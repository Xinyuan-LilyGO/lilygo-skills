//! Shared CLI plumbing for option parsing, JSON rendering, help text, route
//! readiness decoration, and installed-runtime root discovery.
use super::*;

pub(crate) fn prompt_arg(args: &[String]) -> Result<String, String> {
    let mut parts = Vec::new();
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--project" {
            skip_next = true;
            continue;
        }
        if arg != "--json" {
            parts.push(arg.as_str());
        }
    }
    if parts.is_empty() {
        Err("missing prompt".to_string())
    } else {
        Ok(parts.join(" "))
    }
}

pub(crate) fn goal_complete_prompt_arg(args: &[String]) -> Result<String, String> {
    let value_options = ["--project", "--generated-root", "--source-root", "--port"];
    let flag_options = [
        "--json",
        "--dry-run",
        "--allow-generate",
        "--allow-build",
        "--allow-flash",
        "--allow-serial",
        "--allow-network",
        "--allow-ota",
        "--allow-simulator",
    ];
    let mut parts = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if value_options.contains(&arg.as_str()) {
            if index + 1 >= args.len() || args[index + 1].starts_with("--") {
                return Err(format!("{arg} requires a value"));
            }
            index += 2;
            continue;
        }
        if flag_options.contains(&arg.as_str()) {
            index += 1;
            continue;
        }
        if arg.starts_with("--") {
            return Err(format!("unknown option: {arg}"));
        }
        parts.push(arg.as_str());
        index += 1;
    }
    if parts.is_empty() {
        Err("missing prompt".to_string())
    } else {
        Ok(parts.join(" "))
    }
}

pub(crate) fn require_json(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--json") {
        Ok(())
    } else {
        Err("--json is required for this command".to_string())
    }
}

pub(crate) fn profile_arg(args: &[String]) -> Result<PathBuf, String> {
    args.windows(2)
        .find(|pair| pair[0] == "--profile")
        .map(|pair| PathBuf::from(&pair[1]))
        .ok_or_else(|| "--profile <file> is required".to_string())
}

pub(crate) fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

pub(crate) fn option_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}

pub(crate) fn option_values(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .collect()
}

pub(crate) fn output_path_arg(args: &[String], name: &str) -> Result<Option<PathBuf>, String> {
    option_value(args, name)
        .map(|value| {
            let path = Path::new(value);
            if path.is_absolute() {
                Ok(path.to_path_buf())
            } else {
                current_dir().map(|cwd| cwd.join(path))
            }
        })
        .transpose()
}

pub(crate) fn project_start_arg(args: &[String]) -> Result<PathBuf, String> {
    option_value(args, "--project")
        .map(PathBuf::from)
        .unwrap_or(current_dir()?)
        .canonicalize()
        .map_err(|error| format!("failed to resolve project directory: {error}"))
}

pub(crate) fn project_init_arg(args: &[String]) -> Result<PathBuf, String> {
    let raw = option_value(args, "--project")
        .map(PathBuf::from)
        .unwrap_or(current_dir()?);
    let path = if raw.is_absolute() {
        raw
    } else {
        current_dir()?.join(raw)
    };
    fs::create_dir_all(&path).map_err(|error| {
        format!(
            "failed to create project directory {}: {error}",
            path.display()
        )
    })?;
    path.canonicalize()
        .map_err(|error| format!("failed to resolve project directory: {error}"))
}

pub(crate) fn optional_project_arg(args: &[String]) -> Result<Option<PathBuf>, String> {
    if args.iter().any(|arg| arg == "--project") {
        return project_start_arg(args).map(Some);
    }
    Ok(None)
}

pub(crate) fn current_dir() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|error| format!("cwd failed: {error}"))
}

pub(crate) fn usize_option(args: &[String], name: &str, default: usize) -> Result<usize, String> {
    option_value(args, name)
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|error| format!("invalid {name}: {error}"))
        })
        .unwrap_or(Ok(default))
}

pub(crate) fn optional_u128(args: &[String], name: &str) -> Result<Option<u128>, String> {
    option_value(args, name)
        .map(|value| {
            value
                .parse::<u128>()
                .map(Some)
                .map_err(|error| format!("invalid {name}: {error}"))
        })
        .unwrap_or(Ok(None))
}

pub(crate) fn extract_prompt(input: &str) -> String {
    if input.trim().is_empty() {
        return String::new();
    }
    match serde_json::from_str::<serde_json::Value>(input) {
        Ok(value) => value
            .get("prompt")
            .or_else(|| value.get("input"))
            .or_else(|| value.get("text"))
            .and_then(|value| value.as_str())
            .unwrap_or(input)
            .to_string(),
        Err(_) => input.to_string(),
    }
}

pub(crate) fn load_profile(root: &Path) -> Option<ActiveProfile> {
    fs::read_to_string(root.join(PROFILE_PATH))
        .ok()
        .and_then(|data| serde_json::from_str::<ActiveProfile>(&data).ok())
}

pub(crate) fn render_context(route: &crate::model::RouteResult) -> String {
    if route.decision != "inject" {
        return String::new();
    }
    let mut context = format!(
        "LilyGO context injection: skills=[{}]; verification_level={}; hardware_verified=false",
        route.skills.join(","),
        route.verification_level
    );
    if !route.readiness.is_empty() {
        let readiness = route
            .readiness
            .iter()
            .map(|signal| format!("{}={}", signal.topic, signal.completeness))
            .collect::<Vec<_>>()
            .join(",");
        let expansion = route
            .readiness
            .iter()
            .filter_map(|signal| {
                signal
                    .update_command
                    .as_deref()
                    .or(Some(signal.source_query_command.as_str()))
            })
            .take(2)
            .collect::<Vec<_>>()
            .join(" | ");
        context.push_str(&format!("; readiness=[{readiness}]; expand=[{expansion}]"));
    }
    context
}

pub(crate) fn route_with_profile_or_clarification(
    registry: &crate::model::Registry,
    prompt: &str,
    profile: Option<&ActiveProfile>,
) -> RouteResult {
    if let Some(profile) = profile
        && project_context_needs_framework(prompt, profile)
    {
        return framework_clarification_result();
    }
    let mut route = route_prompt_with_profile(registry, prompt, profile);
    if route.decision == "inject"
        && let Some(profile) = profile
        && crate::router::profile_framework_question_applies(prompt, profile)
    {
        route
            .questions
            .push(crate::router::framework_clarification_question());
    }
    route
}

pub(crate) fn attach_route_readiness(
    root: &Path,
    registry: &crate::model::Registry,
    prompt: &str,
    route: &mut RouteResult,
) {
    if route.decision != "inject" {
        return;
    }
    let board = primary_board_id(registry, route);
    route.readiness = completeness_signals_for_prompt(root, board.as_deref(), prompt);
    for signal in &route.readiness {
        if signal.completeness == "needs_source_ingestion" {
            route.notes.push(format!(
                "{} {} needs_source_ingestion; run {}",
                signal.board_id,
                signal.topic,
                signal
                    .update_command
                    .as_deref()
                    .unwrap_or(&signal.source_query_command)
            ));
        }
    }
}

pub(crate) fn primary_board_id(
    registry: &crate::model::Registry,
    route: &RouteResult,
) -> Option<String> {
    route.skills.iter().find_map(|skill_id| {
        registry
            .skills
            .iter()
            .any(|skill| skill.id == *skill_id && skill.kind == SkillKind::Board)
            .then(|| skill_id.clone())
    })
}

pub(crate) fn print_json(value: &impl Serialize) -> Result<(), String> {
    let rendered = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize JSON: {error}"))?;
    println!("{rendered}");
    Ok(())
}

// CLI help text is embedded from data/help so command documentation can be
// reviewed as content instead of being interleaved with command dispatch code.
pub(crate) fn print_help() {
    print!("{}", include_str!("../../../../data/help/main.txt"));
}

pub(crate) fn print_goal_help() {
    print!("{}", include_str!("../../../../data/help/goal.txt"));
}

pub(crate) fn print_profile_help() {
    print!("{}", include_str!("../../../../data/help/profile.txt"));
}

pub(crate) fn print_source_help() {
    print!("{}", include_str!("../../../../data/help/source.txt"));
}

pub(crate) fn print_update_help() {
    print!("{}", include_str!("../../../../data/help/update.txt"));
}

pub(crate) fn print_preference_help() {
    print!("{}", include_str!("../../../../data/help/preference.txt"));
}

pub(crate) fn print_reference_help() {
    print!("{}", include_str!("../../../../data/help/reference.txt"));
}

pub(crate) fn print_setup_help() {
    print!("{}", include_str!("../../../../data/help/setup.txt"));
}

pub(crate) fn print_index_help() {
    print!("{}", include_str!("../../../../data/help/index.txt"));
}

pub(crate) fn print_generate_help() {
    print!("{}", include_str!("../../../../data/help/generate.txt"));
}

pub(crate) fn find_root() -> Result<std::path::PathBuf, String> {
    let cwd = current_dir()?;
    if let Some(root) = find_root_from(cwd.as_path()) {
        return Ok(root);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
        && let Some(root) = find_root_from(parent)
    {
        return Ok(root);
    }
    Err("could not find index/routes.json from current directory or executable path".to_string())
}

pub(crate) fn find_root_from(start: &Path) -> Option<PathBuf> {
    let mut cwd = start.to_path_buf();
    loop {
        if cwd.join("index/routes.json").is_file() {
            return Some(cwd);
        }
        if !cwd.pop() {
            return None;
        }
    }
}
