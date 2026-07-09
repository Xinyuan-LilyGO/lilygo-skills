//! Safe goal command planning and execution with explicit permission gates for
//! build, flash, serial, simulator, network, and OTA actions.
use super::GoalStartOptions;
use super::observation::{observation_excerpt, observation_payload_seen};
use crate::model::{GoalCommandEvidence, GoalCommandPlan, GoalPlan, Recipe, RecipeStep};
use crate::project_context::LOCAL_FILE;
use crate::recipes::recipe_registry;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 300;
const OBSERVATION_COMMAND_TIMEOUT_SECS: u64 = 10;
const SERIAL_READ_LOOP: &str = "i=0; while [ $i -lt 8 ]; do serial-mcp-server read --port \"$1\" --baud 115200 --timeout-ms 1000 --json; i=$((i+1)); sleep 0.3; done";
const PRIVATE_TARGET_WORDS: &str = "/dev/cu|/dev/tty|usbmodem|usbserial|usb id|vid:|pid:|.local";
const SENSITIVE_OUTPUT_WORDS: &str = "access_token|auth_token|authorization|bearer |token=|password|passwd|psk|private_key|secret|ssid|wifi_ssid|wifi_password";

#[derive(serde::Deserialize)]
struct LocalOtaCommands {
    #[serde(default)]
    ota_manifest_argv: Vec<String>,
    #[serde(default)]
    ota_observe_argv: Vec<String>,
}

pub(super) fn planned_commands(
    plan: &GoalPlan,
    options: &GoalStartOptions,
) -> Vec<GoalCommandPlan> {
    trusted_recipes(plan)
        .iter()
        .flat_map(|recipe| {
            recipe.steps.iter().map(|step| {
                let argv = argv_for_step(plan, recipe, step, options);
                let command = if argv.is_empty() || local_private_step(&step.id) {
                    manual_command_for_step(plan, recipe, step)
                } else {
                    display_command(&argv)
                };
                GoalCommandPlan {
                    recipe_id: recipe.id.clone(),
                    step_id: step.id.clone(),
                    command,
                    argv,
                    permission: step.permission.clone(),
                    evidence_level: step.evidence_level.clone(),
                    working_dir: command_working_dir(options),
                }
            })
        })
        .collect()
}

fn trusted_recipes(plan: &GoalPlan) -> Vec<Recipe> {
    let selected = plan
        .recipe_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    recipe_registry()
        .recipes
        .into_iter()
        .filter(|recipe| selected.contains(recipe.id.as_str()))
        .collect()
}

fn manual_command_for_step(plan: &GoalPlan, recipe: &Recipe, step: &RecipeStep) -> String {
    match step.id.as_str() {
        "partition-check" => partition_command(plan),
        "manifest-check" => "resolve the project OTA manifest runner from source, scripts, references, or ignored local state; ask only for private details that cannot be inferred".to_string(),
        "ota-observe" => "resolve the project OTA transport and observation runner, then capture bounded evidence with private values kept local".to_string(),
        _ => {
            if recipe.id == "recipe-lvgl-simulator" {
                "run LVGL page-data or simulator harness for this project".to_string()
            } else {
                step.command.clone()
            }
        }
    }
}

fn argv_for_step(
    plan: &GoalPlan,
    recipe: &Recipe,
    step: &RecipeStep,
    options: &GoalStartOptions,
) -> Vec<String> {
    match step.id.as_str() {
        "check-toolchain" => toolchain_argv(plan),
        "build" => build_argv(plan, options),
        "upload" => upload_argv(plan, options),
        "monitor" | "capture-log" => serial_argv(options),
        "list-ports" => strings(&["espflash", "list-ports", "--skip-update-check"]),
        "partition-check" => partition_argv(plan),
        "manifest-check" => local_ota_argv(options, true),
        "ota-observe" => local_ota_argv(options, false),
        "page-data" => lvgl_page_data_argv(options),
        "simulator-render" => lvgl_simulator_argv(options),
        _ => {
            if recipe.id == "recipe-lvgl-simulator" {
                Vec::new()
            } else {
                step.command
                    .split_whitespace()
                    .map(str::to_string)
                    .collect()
            }
        }
    }
}

fn display_command(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| {
            if arg.chars().all(|ch| {
                ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '=')
            }) {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn command_working_dir(options: &GoalStartOptions) -> Option<String> {
    options
        .source_root
        .as_ref()
        .unwrap_or(&options.project_root)
        .to_str()
        .map(str::to_string)
}

fn toolchain_argv(plan: &GoalPlan) -> Vec<String> {
    match plan.route.framework.as_deref() {
        Some("fw-esp-idf") => strings(&["idf.py", "--version"]),
        Some("fw-platformio") => strings(&["pio", "--version"]),
        Some("fw-rust") => strings(&["cargo", "--version"]),
        _ => strings(&["arduino-cli", "version"]),
    }
}

fn build_argv(plan: &GoalPlan, options: &GoalStartOptions) -> Vec<String> {
    match plan.route.framework.as_deref() {
        Some("fw-esp-idf") => strings(&["idf.py", "build"]),
        Some("fw-platformio") => strings(&["pio", "run"]),
        Some("fw-rust") => strings(&["cargo", "build", "--release"]),
        _ => {
            let sketch = selected_demo_sketch(plan, options)
                .unwrap_or_else(|| "<LilyGoLib>/examples/<selected-demo>".to_string());
            let mut argv = vec![
                "arduino-cli".to_string(),
                "compile".to_string(),
                "--fqbn".to_string(),
                arduino_fqbn(plan),
            ];
            argv.extend(arduino_library_args(plan, options));
            argv.push(sketch);
            argv
        }
    }
}

fn upload_argv(plan: &GoalPlan, options: &GoalStartOptions) -> Vec<String> {
    let port = options.port.as_deref().unwrap_or("<port>").to_string();
    match plan.route.framework.as_deref() {
        Some("fw-esp-idf") => vec![
            "idf.py".to_string(),
            "-p".to_string(),
            port,
            "flash".to_string(),
        ],
        Some("fw-platformio") => vec![
            "pio".to_string(),
            "run".to_string(),
            "--target".to_string(),
            "upload".to_string(),
            format!("--upload-port={port}"),
        ],
        Some("fw-rust") => vec![
            "espflash".to_string(),
            "flash".to_string(),
            format!("--port={port}"),
            "<firmware>".to_string(),
        ],
        _ => {
            let sketch = selected_demo_sketch(plan, options)
                .unwrap_or_else(|| "<LilyGoLib>/examples/<selected-demo>".to_string());
            vec![
                "arduino-cli".to_string(),
                "upload".to_string(),
                format!("-p={port}"),
                "--fqbn".to_string(),
                arduino_fqbn(plan),
                sketch,
            ]
        }
    }
}

fn serial_argv(options: &GoalStartOptions) -> Vec<String> {
    // Runtime observation should read the app's CDC/UART stream directly.
    // `espflash monitor` first synchronizes with the ROM bootloader, which is
    // useful before flashing but unreliable after Arduino has already booted.
    let mut argv = strings(&["sh", "-c", SERIAL_READ_LOOP, "serial-read"]);
    argv.push(options.port.as_deref().unwrap_or("<port>").to_string());
    argv
}

fn partition_argv(plan: &GoalPlan) -> Vec<String> {
    match plan.route.framework.as_deref() {
        Some("fw-esp-idf") => strings(&["idf.py", "partition-table"]),
        Some("fw-platformio") => strings(&["pio", "run", "--target", "buildfs"]),
        _ => Vec::new(),
    }
}

fn partition_command(plan: &GoalPlan) -> String {
    match plan.route.framework.as_deref() {
        Some("fw-esp-idf") => "idf.py partition-table".to_string(),
        Some("fw-platformio") => "pio run --target buildfs".to_string(),
        _ => "inspect Arduino partition scheme and rollback settings".to_string(),
    }
}

fn local_ota_argv(options: &GoalStartOptions, manifest: bool) -> Vec<String> {
    let path = options.project_root.join(LOCAL_FILE);
    let Ok(data) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(local) = serde_json::from_str::<LocalOtaCommands>(&data) else {
        return Vec::new();
    };
    if manifest {
        local.ota_manifest_argv
    } else {
        local.ota_observe_argv
    }
}

fn lvgl_page_data_argv(options: &GoalStartOptions) -> Vec<String> {
    vec![
        "lvgl-page-data".to_string(),
        "--project".to_string(),
        command_root(options),
    ]
}

fn lvgl_simulator_argv(options: &GoalStartOptions) -> Vec<String> {
    vec![
        "lvgl-simulator".to_string(),
        "--project".to_string(),
        command_root(options),
        "--render".to_string(),
    ]
}

fn command_root(options: &GoalStartOptions) -> String {
    options
        .source_root
        .as_ref()
        .unwrap_or(&options.project_root)
        .display()
        .to_string()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn arduino_fqbn(plan: &GoalPlan) -> String {
    fact_value(plan, "arduino.fqbn")
        .unwrap_or_else(|| "<arduino-fqbn-from-board-source>".to_string())
}

fn arduino_library_args(plan: &GoalPlan, options: &GoalStartOptions) -> Vec<String> {
    let Some(value) = fact_value(plan, "arduino.library_roots") else {
        return Vec::new();
    };
    value
        .split(',')
        .filter(|root| !root.trim().is_empty())
        .flat_map(|root| {
            [
                "--libraries".to_string(),
                arduino_library_path(root.trim(), options),
            ]
        })
        .collect()
}

fn arduino_library_path(root: &str, options: &GoalStartOptions) -> String {
    let Some(source_root) = &options.source_root else {
        return format!("<source-root>/{root}");
    };
    if root == "." {
        return source_root.display().to_string();
    }
    source_root.join(root).display().to_string()
}

fn fact_value(plan: &GoalPlan, key: &str) -> Option<String> {
    plan.context_capsule
        .facts
        .iter()
        .find(|fact| fact.key == key)
        .map(|fact| fact.value.clone())
}

fn selected_demo_sketch(plan: &GoalPlan, options: &GoalStartOptions) -> Option<String> {
    let source_root = options.source_root.as_ref()?;
    let demo = plan.context_capsule.demo_refs.first()?;
    let path = Path::new(&demo.path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    let sketch_dir = if path.extension().is_some() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    Some(source_root.join(sketch_dir).display().to_string())
}

pub(super) fn blocked_permissions(
    commands: &[GoalCommandPlan],
    options: &GoalStartOptions,
) -> Vec<String> {
    let mut blocked = BTreeSet::new();
    for command in commands {
        collect_missing_permissions(&command.permission, options, &mut blocked);
    }
    blocked.into_iter().collect()
}

fn collect_missing_permissions(
    permission_spec: &str,
    options: &GoalStartOptions,
    blocked: &mut BTreeSet<String>,
) {
    for permission in permission_spec.split('+') {
        match permission {
            "read-only" => {}
            "allow-build" => insert_missing(blocked, !options.allow_build, "allow-build"),
            "allow-flash:port" => {
                insert_missing(blocked, !options.allow_flash, "allow-flash");
                insert_missing(blocked, options.port.is_none(), "port");
            }
            "allow-serial:port" => {
                insert_missing(blocked, !options.allow_serial, "allow-serial");
                insert_missing(blocked, options.port.is_none(), "port");
            }
            "allow-network" => insert_missing(blocked, !options.allow_network, "allow-network"),
            "allow-ota" => insert_missing(blocked, !options.allow_ota, "allow-ota"),
            "allow-simulator" => {
                insert_missing(blocked, !options.allow_simulator, "allow-simulator")
            }
            _ => {}
        }
    }
}

fn insert_missing(blocked: &mut BTreeSet<String>, missing: bool, name: &str) {
    if missing {
        blocked.insert(name.to_string());
    }
}

pub(super) fn executable_commands(commands: &[GoalCommandPlan]) -> Vec<GoalCommandPlan> {
    commands
        .iter()
        .filter(|command| is_executable_step(&command.step_id) && !command.argv.is_empty())
        .cloned()
        .collect()
}

fn is_executable_step(step_id: &str) -> bool {
    matches!(
        step_id,
        "check-toolchain"
            | "build"
            | "upload"
            | "monitor"
            | "capture-log"
            | "list-ports"
            | "partition-check"
            | "manifest-check"
            | "ota-observe"
            | "page-data"
            | "simulator-render"
    )
}

pub(super) fn allowed_commands(
    commands: &[GoalCommandPlan],
    options: &GoalStartOptions,
) -> Vec<GoalCommandPlan> {
    commands
        .iter()
        .filter(|command| permission_allowed(&command.permission, options))
        .cloned()
        .collect()
}

fn permission_allowed(permission_spec: &str, options: &GoalStartOptions) -> bool {
    let mut missing = BTreeSet::new();
    collect_missing_permissions(permission_spec, options, &mut missing);
    missing.is_empty()
}

pub(super) fn run_command(
    command: &GoalCommandPlan,
    options: &GoalStartOptions,
) -> Result<GoalCommandEvidence, String> {
    let Some(binary) = command.argv.first() else {
        return Err("empty command".to_string());
    };
    if command.argv.iter().any(|part| part.contains('<')) {
        return Err(format!("command is not concrete: {}", command.command));
    }
    let mut process = Command::new(binary);
    process.args(&command.argv[1..]);
    if let Some(working_dir) = &command.working_dir {
        process.current_dir(working_dir);
    }
    let (output, timed_out) = run_with_timeout(process, command_timeout(command))
        .map_err(|error| format!("failed to run {}: {error}", command.command))?;
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let status = command_status(&command.step_id, output.status.success(), timed_out, &text);
    Ok(GoalCommandEvidence {
        recipe_id: command.recipe_id.clone(),
        step_id: command.step_id.clone(),
        command: redact_sensitive(&command.command, options),
        status,
        exit_code: output.status.code(),
        output_excerpt: output_excerpt(command, &text, options),
    })
}

pub(super) fn command_status(
    step_id: &str,
    exit_success: bool,
    timed_out: bool,
    text: &str,
) -> String {
    let serial = matches!(step_id, "monitor" | "capture-log");
    let observed = observation_payload_seen(text);
    let pass = (serial && observed)
        || (!serial && (exit_success || (timed_out && observation_step(step_id) && observed)));
    if pass {
        "PASS".to_string()
    } else {
        "FAIL".to_string()
    }
}

fn observation_step(step_id: &str) -> bool {
    matches!(step_id, "monitor" | "capture-log" | "ota-observe")
}

fn output_excerpt(command: &GoalCommandPlan, text: &str, options: &GoalStartOptions) -> String {
    if local_private_step(&command.step_id) {
        return "[private local OTA command output omitted; exit code recorded]".to_string();
    }
    let redacted = redact_sensitive(text, options);
    if observation_step(&command.step_id) {
        observation_excerpt(&redacted)
    } else {
        excerpt(&redacted)
    }
}

fn local_private_step(step_id: &str) -> bool {
    matches!(step_id, "manifest-check" | "ota-observe")
}

fn command_timeout(command: &GoalCommandPlan) -> Duration {
    let seconds = match command.step_id.as_str() {
        "monitor" | "capture-log" | "ota-observe" => OBSERVATION_COMMAND_TIMEOUT_SECS,
        _ => DEFAULT_COMMAND_TIMEOUT_SECS,
    };
    Duration::from_secs(seconds)
}

fn run_with_timeout(mut process: Command, timeout: Duration) -> Result<(Output, bool), String> {
    let start = Instant::now();
    let mut child = process
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    // Dedicated reader threads drain both pipes while the child runs; polling
    // try_wait() with full pipes deadlocks once a child writes more than the
    // OS pipe buffer (~64KB), and killing at timeout would truncate output.
    let stdout_reader = spawn_pipe_reader(child.stdout.take());
    let stderr_reader = spawn_pipe_reader(child.stderr.take());
    let (status, timed_out) = loop {
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(status) => break (status, false),
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let status = child.wait().map_err(|error| error.to_string())?;
                    break (status, true);
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    };
    // After the child exits (or is killed), grandchildren can keep the pipe
    // write-ends open indefinitely; the join is bounded so a lingering
    // grandchild cannot stall the CLI, and partial output is preserved.
    let drain = if timed_out {
        Duration::from_secs(2)
    } else {
        Duration::from_secs(5)
    };
    // One shared deadline across both pipes so the worst-case stall stays at
    // `drain`, not double.
    let deadline = Instant::now() + drain;
    let stdout = join_pipe_reader(stdout_reader, deadline);
    let mut stderr = join_pipe_reader(stderr_reader, deadline);
    if timed_out {
        stderr.extend_from_slice(
            format!("\ncommand timed out after {}s", timeout.as_secs()).as_bytes(),
        );
    }
    Ok((
        Output {
            status,
            stdout,
            stderr,
        },
        timed_out,
    ))
}

struct PipeReader {
    buffer: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    handle: thread::JoinHandle<()>,
}

fn spawn_pipe_reader<R: std::io::Read + Send + 'static>(pipe: Option<R>) -> Option<PipeReader> {
    let mut source = pipe?;
    let buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&buffer);
    let handle = thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match source.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    if let Ok(mut locked) = sink.lock() {
                        locked.extend_from_slice(&chunk[..count]);
                    }
                }
            }
        }
    });
    Some(PipeReader { buffer, handle })
}

fn join_pipe_reader(reader: Option<PipeReader>, deadline: Instant) -> Vec<u8> {
    let Some(reader) = reader else {
        return Vec::new();
    };
    while !reader.handle.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if reader.handle.is_finished() {
        let _ = reader.handle.join();
    }
    match reader.buffer.lock() {
        Ok(locked) => locked.clone(),
        // A poisoned lock still holds the captured evidence; keep it.
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

pub(super) fn command_error_evidence(
    command: &GoalCommandPlan,
    error: &str,
    options: &GoalStartOptions,
) -> GoalCommandEvidence {
    GoalCommandEvidence {
        recipe_id: command.recipe_id.clone(),
        step_id: command.step_id.clone(),
        command: redact_sensitive(&command.command, options),
        status: "BLOCKED".to_string(),
        exit_code: None,
        output_excerpt: output_excerpt(command, error, options),
    }
}

pub(super) fn command_failure_summary(command: &GoalCommandEvidence) -> String {
    if command.status == "PASS" {
        return String::new();
    }
    format!(
        "{}:{} status={} exit={:?}\n{}",
        command.recipe_id,
        command.step_id,
        command.status,
        command.exit_code,
        command.output_excerpt
    )
}

pub(super) fn highest_level(commands: &[GoalCommandEvidence]) -> String {
    if commands
        .iter()
        .any(|command| command.status == "PASS" && level_rank(&command.step_id) >= 5)
    {
        return "V5".to_string();
    }
    if commands
        .iter()
        .any(|command| command.status == "PASS" && level_rank(&command.step_id) >= 4)
    {
        return "V4".to_string();
    }
    "V3".to_string()
}

fn level_rank(step_id: &str) -> u8 {
    match step_id {
        "upload" | "monitor" | "capture-log" | "ota-observe" => 5,
        "build" | "page-data" | "simulator-render" | "partition-check" | "manifest-check" => 4,
        _ => 3,
    }
}

pub(super) fn next_action(status: &str) -> Option<String> {
    match status {
        "complete" => {
            Some("inspect evidence and decide whether V4/V5 proof is sufficient".to_string())
        }
        "partial" => Some(
            "review collected evidence, then approve the next explicit permission if needed"
                .to_string(),
        ),
        _ => Some("resolve blocker, patch, or reduce the goal before rerun".to_string()),
    }
}

fn excerpt(value: &str) -> String {
    value
        .lines()
        .take(20)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(4000)
        .collect()
}

pub(super) fn redact_sensitive(value: &str, options: &GoalStartOptions) -> String {
    let mut redacted = value.to_string();
    let project_root = options.project_root.display().to_string();
    let source_root = options
        .source_root
        .as_ref()
        .map_or(String::new(), |path| path.display().to_string());
    for (needle, replacement) in [
        (project_root, "<redacted-project-root>"),
        (source_root, "<redacted-source-root>"),
        (options.port.clone().unwrap_or_default(), "<redacted-port>"),
        (env::var("HOME").unwrap_or_default(), "<redacted-home>"),
    ] {
        if !needle.is_empty() {
            redacted = redacted.replace(&needle, replacement);
        }
    }
    redacted
        .lines()
        .map(|line| {
            let lower = line.to_lowercase();
            if contains_any(&lower, SENSITIVE_OUTPUT_WORDS) {
                "[redacted sensitive output line]".to_string()
            } else if contains_private_target(&lower) {
                "[redacted private output line]".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn contains_any(value: &str, needles: &str) -> bool {
    needles.split('|').any(|needle| value.contains(needle))
}

fn contains_private_target(value: &str) -> bool {
    contains_any(value, PRIVATE_TARGET_WORDS)
        || contains_private_ipv4(value)
        || contains_mac_address(value)
}

fn contains_private_ipv4(value: &str) -> bool {
    value
        .split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .any(|candidate| {
            candidate
                .parse::<std::net::Ipv4Addr>()
                .is_ok_and(|ip| ip.is_private() || ip.is_link_local())
        })
}

fn contains_mac_address(value: &str) -> bool {
    value
        .split(|ch: char| !(ch.is_ascii_hexdigit() || ch == ':' || ch == '-'))
        .any(is_mac_address)
}

fn is_mac_address(candidate: &str) -> bool {
    let parts = candidate.split([':', '-']).collect::<Vec<_>>();
    (candidate.contains(':') || candidate.contains('-'))
        && parts.len() == 6
        && parts
            .iter()
            .all(|part| part.len() == 2 && part.chars().all(|ch| ch.is_ascii_hexdigit()))
}

#[cfg(test)]
mod pipe_tests {
    use super::*;

    // A child writing more than the OS pipe buffer (~64KB) on both streams
    // must complete promptly with full output instead of deadlocking until
    // the timeout kill truncates it.
    #[test]
    fn runner_large_output_no_deadlock() {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(
            "head -c 131072 /dev/zero | tr '\\0' 'a'; \
             head -c 131072 /dev/zero | tr '\\0' 'b' >&2",
        );
        let started = Instant::now();
        let (output, timed_out) = run_with_timeout(command, Duration::from_secs(30)).expect("run");
        assert!(!timed_out, "large-output child must not hit the timeout");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "child with >64KB output must finish promptly"
        );
        assert_eq!(output.stdout.len(), 131072, "stdout must be complete");
        assert_eq!(output.stderr.len(), 131072, "stderr must be complete");
        assert!(output.status.success());
    }

    #[test]
    fn runner_timeout_keeps_partial_output_and_marker() {
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("printf partial; printf diag >&2; sleep 30");
        let (output, timed_out) = run_with_timeout(command, Duration::from_secs(1)).expect("run");
        assert!(timed_out);
        assert_eq!(output.stdout, b"partial");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("diag"));
        assert!(stderr.contains("command timed out after 1s"));
    }
}
