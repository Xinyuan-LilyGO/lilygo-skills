//! CLI command dispatcher for route, generation, source, project, setup, and
//! goal surfaces; handlers keep public JSON contracts stable.
use crate::capsule::{plan_goal_with_project, render_hook_goal_summary};
use crate::doctor::doctor_report;
use crate::facts::{
    board_fact_enrichment_apply, board_fact_enrichment_preview, completeness_signals_for_prompt,
    fact_pack_apply, fact_pack_preview, source_completeness, source_query,
};
use crate::hardware::verify_hardware_profile;
use crate::model::{ActiveProfile, RouteResult, SkillKind};
use crate::preferences::resolve_preferences;
use crate::project_context::{
    clear_project_context, new_project_context, resolve_project_context, write_project_context,
};
use crate::project_ledger::{
    hints_for_route, maybe_compact_project_hook_context, render_hook_ledger_context,
    route_json_with_ledger,
};
use crate::project_skills::registry_with_project_skills;
use crate::reference_catalog::list_references;
use crate::registry::{ensure_skill_files, load_registry, verify};
use crate::router::{
    framework_clarification_result, project_context_needs_framework, route_prompt_with_profile,
};
use crate::setup_plan::setup_plan;
use crate::source::write_if_changed;
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const PROFILE_PATH: &str = "data/profile.json";

pub fn run(args: impl Iterator<Item = String>, mut stdin: impl Read) -> Result<(), String> {
    let args: Vec<String> = args.collect();
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    if command == "hook" {
        return hook(&args[1..], &mut stdin);
    }

    let root = find_root()?;
    match command {
        "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "route" => route(&root, &args[1..]),
        "context" => context::context_command(&root, &args[1..]),
        "index" => index(&root, &args[1..]),
        "verify" => verify_command(&root, &args[1..]),
        "doctor" => doctor(&root, &args[1..]),
        "source" => source_command(&root, &args[1..]),
        "preference" => preference_command(&root, &args[1..]),
        "reference" => reference_command(&root, &args[1..]),
        "setup" => setup_command(&args[1..]),
        "profile" => profile(&root, &args[1..]),
        "project" => project(&root, &args[1..]),
        "verify-hardware" => verify_hardware(&root, &args[1..]),
        "update" => update(&root, &args[1..]),
        other => Err(format!("unknown command: {other}")),
    }
}

fn route(root: &Path, args: &[String]) -> Result<(), String> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        println!("Usage: lilygo-skills route [--project <dir>] --json <prompt>");
        return Ok(());
    }
    require_json(args)?;
    let prompt = prompt_arg(args)?;
    let registry = load_registry(root)?;
    ensure_skill_files(root, &registry)?;
    let project_start = project_start_arg(args)?;
    if let Some(project) = resolve_project_context(project_start.as_path())? {
        let registry =
            registry_with_project_skills(&registry, Some(project.project_root.as_path()))?;
        let profile = project.context.active_profile();
        let mut route = route_with_profile_or_clarification(&registry, &prompt, Some(&profile));
        attach_route_readiness(root, &registry, &prompt, &mut route);
        let hints = hints_for_route(project.project_root.as_path(), &route, &prompt);
        return print_json(&route_json_with_ledger(&route, hints));
    }
    let profile = load_profile(root);
    let mut route = route_with_profile_or_clarification(&registry, &prompt, profile.as_ref());
    attach_route_readiness(root, &registry, &prompt, &mut route);
    print_json(&route)
}

fn hook(args: &[String], stdin: &mut impl Read) -> Result<(), String> {
    let host = args.first().map(String::as_str).unwrap_or("codex");
    if matches!(host, "--help" | "-h") {
        println!(
            "Usage: lilygo-skills hook <claude|codex>\n\nReads a prompt JSON object ({{\"prompt\":\"...\"}}) from stdin.\nclaude: emits the UserPromptSubmit hookSpecificOutput envelope.\ncodex: emits the diagnostic routing envelope for manual use."
        );
        return Ok(());
    }
    if !matches!(host, "codex" | "claude") {
        return Err(format!("unsupported hook host: {host}"));
    }

    let mut input = String::new();
    if let Err(error) = stdin.read_to_string(&mut input) {
        return print_hook_fail_open(host, &format!("failed to read hook input: {error}"));
    }
    let prompt = extract_prompt(&input);
    let root = match find_root() {
        Ok(root) => root,
        Err(error) => return print_hook_fail_open(host, &error),
    };
    let registry = match load_registry(&root) {
        Ok(registry) => registry,
        Err(error) => return print_hook_fail_open(host, &error),
    };
    if let Err(error) = ensure_skill_files(&root, &registry) {
        return print_hook_fail_open(host, &error);
    }
    let hook_cwd = match current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return print_hook_fail_open(host, &error),
    };
    // Delegate to the shared capsule assembly. `sniff = false` keeps the hook's
    // board resolution exactly as before (project profile, then runtime
    // default), so hook output stays byte-identical; only `context` opts into
    // best-effort board sniffing.
    let (route, content) = match context::assemble_capsule(
        &root, &registry, &prompt, &hook_cwd, host, &input, false,
    ) {
        Ok(result) => result,
        Err(error) => return print_hook_fail_open(host, &error),
    };
    print_json(&hook_envelope(host, &route, content))
}

/// Claude Code only consumes `hookSpecificOutput.additionalContext` from
/// UserPromptSubmit JSON stdout; top-level keys such as `decision` carry host
/// semantics (`"block"`), so the claude envelope stays minimal. Codex keeps
/// the legacy diagnostic envelope for AGENTS.md-driven manual invocation.
fn hook_envelope(host: &str, route: &RouteResult, content: String) -> serde_json::Value {
    if host == "claude" {
        let mut inner = serde_json::json!({ "hookEventName": "UserPromptSubmit" });
        if !content.is_empty() {
            inner["additionalContext"] = serde_json::Value::String(content);
        }
        return serde_json::json!({ "hookSpecificOutput": inner });
    }
    serde_json::json!({
        "host": host,
        "decision": route.decision,
        "skills": route.skills,
        "missing": route.missing,
        "questions": route.questions,
        "context": content,
        "fail_open": true
    })
}

fn print_hook_fail_open(host: &str, error: &str) -> Result<(), String> {
    if host == "claude" {
        eprintln!("lilygo-skills hook claude fail-open: {error}");
        return print_json(&serde_json::json!({
            "hookSpecificOutput": { "hookEventName": "UserPromptSubmit" }
        }));
    }
    print_json(&serde_json::json!({
        "host": host,
        "decision": "no-op",
        "skills": [],
        "context": "",
        "fail_open": true,
        "error": error
    }))
}

fn index(root: &Path, args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_flag(args, "--help") || has_flag(args, "-h") {
        print_index_help();
        return Ok(());
    }
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("missing index subcommand".to_string());
    };
    let registry = load_registry(root)?;
    match subcommand {
        "list" => {
            require_json(&args[1..])?;
            print_json(&registry.skills)
        }
        "query" => {
            require_json(&args[2..])?;
            let id = args.get(1).ok_or("missing skill id for index query")?;
            if id.starts_with("playbook-") {
                let playbook = crate::playbooks::playbook_by_id(id)
                    .ok_or_else(|| format!("unknown playbook id: {id}"))?;
                return print_json(&playbook);
            }
            let skill = registry
                .skills
                .iter()
                .find(|skill| skill.id == *id)
                .ok_or_else(|| format!("unknown skill id: {id}"))?;
            print_json(skill)
        }
        other => Err(format!("unknown index subcommand: {other}")),
    }
}

fn verify_command(root: &Path, args: &[String]) -> Result<(), String> {
    require_json(args)?;
    let report = verify(root);
    print_status_json(&report, &report.status, "verification failed")
}

fn doctor(root: &Path, args: &[String]) -> Result<(), String> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        println!(
            "Usage: lilygo-skills doctor --json [--home <dir>]\n\nChecks runtime data, skill availability, sample injection, no-op routing, and optional host install files."
        );
        return Ok(());
    }
    require_json(args)?;
    let home = option_value(args, "--home").map(PathBuf::from);
    let report = doctor_report(root, home.as_deref());
    print_status_json(&report, &report.status, "doctor check failed")
}

fn source_command(root: &Path, args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        print_source_help();
        return Ok(());
    };
    match subcommand {
        "--help" | "-h" => {
            print_source_help();
            Ok(())
        }
        "query" => source_query_command(root, &args[1..]),
        "completeness" => source_completeness_command(root, &args[1..]),
        other => Err(format!("unknown source subcommand: {other}")),
    }
}

fn source_query_command(root: &Path, args: &[String]) -> Result<(), String> {
    require_json(args)?;
    let (board, topic) = board_topic_args(args, "io")?;
    print_json(&source_query(root, board, topic)?)
}

fn source_completeness_command(root: &Path, args: &[String]) -> Result<(), String> {
    require_json(args)?;
    let (board, topic) = board_topic_args(args, "display")?;
    print_json(&source_completeness(root, board, topic)?)
}

fn preference_command(root: &Path, args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        print_preference_help();
        return Ok(());
    };
    match subcommand {
        "--help" | "-h" => {
            print_preference_help();
            Ok(())
        }
        "show" => {
            require_json(&args[1..])?;
            let project = optional_project_arg(&args[1..])?;
            print_json(&resolve_preferences(root, project.as_deref())?)
        }
        other => Err(format!("unknown preference subcommand: {other}")),
    }
}

fn reference_command(root: &Path, args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        print_reference_help();
        return Ok(());
    };
    match subcommand {
        "--help" | "-h" => {
            print_reference_help();
            Ok(())
        }
        "list" => {
            require_json(&args[1..])?;
            let project = optional_project_arg(&args[1..])?;
            print_json(&list_references(root, project.as_deref())?)
        }
        other => Err(format!("unknown reference subcommand: {other}")),
    }
}

fn setup_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        print_setup_help();
        return Ok(());
    };
    match subcommand {
        "--help" | "-h" => {
            print_setup_help();
            Ok(())
        }
        "plan" => {
            require_json(&args[1..])?;
            let framework = option_value(&args[1..], "--framework").ok_or_else(|| {
                "--framework <arduino|platformio|esp-idf|rust> is required".to_string()
            })?;
            let project = optional_project_arg(&args[1..])?;
            print_json(&setup_plan(framework, project.as_deref())?)
        }
        other => Err(format!("unknown setup subcommand: {other}")),
    }
}

fn profile(root: &Path, args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("missing profile subcommand".to_string());
    };
    match subcommand {
        "--help" | "-h" => {
            print_profile_help();
            Ok(())
        }
        "set" => profile_set(root, &args[1..]),
        "clear" => profile_clear(root, &args[1..]),
        other => Err(format!("unknown profile subcommand: {other}")),
    }
}

fn project(root: &Path, args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("missing project subcommand".to_string());
    };
    match subcommand {
        "--help" | "-h" => {
            print_project_help();
            Ok(())
        }
        "init" => project_init(root, &args[1..]),
        "show" => project_show(&args[1..]),
        "clear" => project_clear(&args[1..]),
        "ledger" => project_ledger_command(&args[1..]),
        other => Err(format!("unknown project subcommand: {other}")),
    }
}

fn project_init(root: &Path, args: &[String]) -> Result<(), String> {
    require_json(args)?;
    let board = option_value(args, "--board")
        .ok_or_else(|| "--board <skill-id> is required".to_string())?;
    let framework = option_value(args, "--framework");
    let features = option_values(args, "--feature");
    let project_root = project_init_arg(args)?;
    let registry = load_registry(root)?;
    let context = new_project_context(&registry, board, framework, features)?;
    let writes = write_project_context(project_root.as_path(), &context)?;
    // Skills are delivered as context injection (via `context`/`hook`), not as a
    // materialized per-project skill cache, so project init only writes the
    // project profile now that the generation stack is gone.
    print_json(&serde_json::json!({
        "status": "PASS",
        "project_root": project_root,
        "context": context,
        "writes": writes
    }))
}

fn project_show(args: &[String]) -> Result<(), String> {
    require_json(args)?;
    let start = project_start_arg(args)?;
    let Some(project) = resolve_project_context(start.as_path())? else {
        return print_json(&serde_json::json!({
            "status": "PASS",
            "context_source": "none",
            "project_root": null,
            "board": null,
            "framework": null,
            "features": [],
            "local_evidence_present": false
        }));
    };
    print_json(&serde_json::json!({
        "status": "PASS",
        "context_source": "project",
        "project_root": project.project_root,
        "board": project.context.board,
        "framework": project.context.framework,
        "features": project.context.features,
        "local_evidence_present": project.local_evidence_present
    }))
}

fn project_clear(args: &[String]) -> Result<(), String> {
    require_json(args)?;
    let project_root = project_start_arg(args)?;
    let writes = clear_project_context(project_root.as_path())?;
    print_json(&serde_json::json!({
        "status": "PASS",
        "project_root": project_root,
        "writes": writes
    }))
}

fn profile_set(root: &Path, args: &[String]) -> Result<(), String> {
    require_json(args)?;
    let board = option_value(args, "--board")
        .ok_or_else(|| "--board <skill-id> is required".to_string())?
        .to_string();
    let framework = option_value(args, "--framework").map(str::to_string);
    let registry = load_registry(root)?;
    let board_exists = registry
        .skills
        .iter()
        .any(|skill| skill.id == board && skill.kind == SkillKind::Board);
    if !board_exists {
        return Err(format!("unknown board skill: {board}"));
    }
    if let Some(framework_id) = &framework {
        let framework_exists = registry.skills.iter().any(|skill| {
            skill.id == *framework_id
                && matches!(skill.kind, SkillKind::Framework | SkillKind::Tool)
        });
        if !framework_exists {
            return Err(format!("unknown framework skill: {framework_id}"));
        }
    }
    let active = ActiveProfile {
        board,
        framework,
        features: Vec::new(),
    };
    let rendered = serde_json::to_vec_pretty(&active)
        .map_err(|error| format!("failed to render profile: {error}"))?;
    let path = root.join(PROFILE_PATH);
    let writes = if write_if_changed(&path, &rendered)? {
        vec![PROFILE_PATH.to_string()]
    } else {
        Vec::new()
    };
    print_json(&serde_json::json!({
        "status": "PASS",
        "profile": active,
        "writes": writes
    }))
}

fn profile_clear(root: &Path, args: &[String]) -> Result<(), String> {
    require_json(args)?;
    let path = root.join(PROFILE_PATH);
    let writes = if path.is_file() {
        fs::remove_file(&path)
            .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        vec![PROFILE_PATH.to_string()]
    } else {
        Vec::new()
    };
    print_json(&serde_json::json!({
        "status": "PASS",
        "profile": null,
        "writes": writes
    }))
}

fn update(root: &Path, args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_flag(args, "--help") {
        print_update_help();
        return Ok(());
    }
    let Some(target) = args.first().map(String::as_str) else {
        return Err("missing update target".to_string());
    };
    require_json(&args[1..])?;
    let rest = &args[1..];
    // Generation-serving update targets (boards/sources/skills/runtime/
    // source-packs/peripheral-skills) were removed with the generation stack;
    // only the source-backed fact-enrichment targets remain.
    match (target, has_flag(rest, "--dry-run")) {
        ("board-facts", dry) if has_flag(rest, "--from-source") => {
            // Explicit, opt-in self-serve ingestion: fetch the board's official
            // source declared in pipeline/source-manifest.json, extract pins,
            // and (unless --dry-run) write source-backed facts. Network only
            // happens under this flag, so offline gates stay offline.
            let (board, _topic) = board_topic_args(rest, "display")?;
            if !manifest_source_board(root, board) {
                return Err(format!(
                    "no source-manifest entry for {board}; add one to pipeline/source-manifest.json"
                ));
            }
            run_manifest_ingest(root, board, !dry)
        }
        ("board-facts", true) => {
            let (board, topic) = board_topic_args(rest, "display")?;
            print_json(&board_fact_enrichment_preview(root, board, topic)?)
        }
        ("board-facts", false) => {
            let (board, topic) = board_topic_args(rest, "display")?;
            print_json(&board_fact_enrichment_apply(root, board, topic)?)
        }
        ("fact-packs", true) => print_json(&fact_pack_preview(root)?),
        ("fact-packs", false) => print_json(&fact_pack_apply(root)?),
        (other, _) => Err(format!("unknown update target: {other}")),
    }
}

fn board_topic_args<'a>(
    args: &'a [String],
    default_topic: &'static str,
) -> Result<(&'a str, &'a str), String> {
    let board = option_value(args, "--board")
        .ok_or_else(|| "--board <board-id> is required".to_string())?;
    Ok((
        board,
        option_value(args, "--topic").unwrap_or(default_topic),
    ))
}

fn verify_hardware(root: &Path, args: &[String]) -> Result<(), String> {
    require_json(args)?;
    let profile = profile_arg(args)?;
    let report = verify_hardware_profile(root, &profile);
    print_json(&report)?;
    if report.status == "FAIL" {
        Err("hardware profile verification failed".to_string())
    } else {
        Ok(())
    }
}

mod context;
mod support;
pub(crate) use support::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RouteResult;
    use crate::registry::load_registry;
    use crate::router::route_prompt;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    #[test]
    fn inferred_board_source_reaches_capsule_text() {
        // The context-fallback provenance must surface in the rendered capsule so
        // the model can tell an inferred board from a user-named one; an
        // unmarked inject stays byte-identical to before.
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = load_registry(source_root.as_path()).expect("registry");
        let profile = ActiveProfile {
            board: "board-t-display-s3".to_string(),
            framework: None,
            features: Vec::new(),
        };
        let inferred = route_with_profile_or_clarification(
            &registry,
            "how do I light up the screen",
            Some(&profile),
        );
        assert_eq!(
            inferred.board_source.as_deref(),
            Some("inferred-from-project")
        );
        let context = render_context(&inferred);
        assert!(
            context.contains("board_source=inferred-from-project"),
            "inferred capsule must state its provenance: {context}"
        );

        let named = route_prompt(&registry, "T-Display-S3 LVGL screen is blank");
        assert!(
            !render_context(&named).contains("board_source"),
            "keyword-matched boards must not emit a board_source marker"
        );
    }

    #[test]
    fn hook_envelopes() {
        let route = RouteResult {
            decision: "inject".to_string(),
            skills: vec!["lilygo-router".to_string(), "fw-lvgl".to_string()],
            matches: Vec::new(),
            paths: BTreeMap::new(),
            readiness: Vec::new(),
            missing: Vec::new(),
            questions: Vec::new(),
            verification_level: "context-injection".to_string(),
            hardware_verified: false,
            hardware_verification_boundary: true,
            notes: Vec::new(),
            truncated: false,
            board_source: None,
        };
        let context = render_context(&route);
        assert!(context.contains("verification_level=context-injection"));
        assert!(context.contains("hardware_verified=false"));

        let claude = hook_envelope("claude", &route, context.clone());
        let inner = claude
            .get("hookSpecificOutput")
            .expect("claude envelope must carry hookSpecificOutput");
        assert_eq!(
            inner.get("hookEventName").and_then(|v| v.as_str()),
            Some("UserPromptSubmit")
        );
        assert_eq!(
            inner.get("additionalContext").and_then(|v| v.as_str()),
            Some(context.as_str())
        );
        assert!(
            claude.get("decision").is_none(),
            "claude envelope must not emit host-semantic top-level keys"
        );

        let codex = hook_envelope("codex", &route, context.clone());
        assert_eq!(
            codex.get("decision").and_then(|v| v.as_str()),
            Some("inject")
        );
        assert_eq!(
            codex.get("context").and_then(|v| v.as_str()),
            Some(context.as_str())
        );
        assert!(codex.get("hookSpecificOutput").is_none());
    }

    #[test]
    fn hook_claude_noop_envelope_has_no_context_key() {
        let route = RouteResult {
            decision: "no-op".to_string(),
            skills: Vec::new(),
            matches: Vec::new(),
            paths: BTreeMap::new(),
            readiness: Vec::new(),
            missing: Vec::new(),
            questions: Vec::new(),
            verification_level: "context-injection".to_string(),
            hardware_verified: false,
            hardware_verification_boundary: true,
            notes: Vec::new(),
            truncated: false,
            board_source: None,
        };
        let context = render_context(&route);
        assert!(context.is_empty(), "no-op routes must not render context");
        let claude = hook_envelope("claude", &route, context);
        let inner = claude.get("hookSpecificOutput").expect("envelope");
        assert!(inner.get("additionalContext").is_none());
    }

    #[test]
    fn route_completeness_signal() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = load_registry(source_root.as_path()).expect("registry");
        let mut route = route_prompt(&registry, "T-Display-S3 Arduino LVGL display demo");
        attach_route_readiness(
            source_root.as_path(),
            &registry,
            "T-Display-S3 Arduino LVGL display demo",
            &mut route,
        );
        assert!(route.readiness.iter().any(|signal| {
            signal.board_id == "board-t-display-s3"
                && signal.topic == "display"
                && signal.completeness == "complete"
        }));
        assert!(
            route
                .notes
                .iter()
                .all(|note| !note.contains("needs_source_ingestion"))
        );
    }

    #[test]
    fn hook_no_write_completeness_signal() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let before =
            fs::read(source_root.join(crate::facts::FACT_PACK_INDEX_PATH)).expect("fact packs");
        let registry = load_registry(source_root.as_path()).expect("registry");
        let mut route = route_prompt(&registry, "T-Display-S3 Arduino LVGL display demo");
        attach_route_readiness(
            source_root.as_path(),
            &registry,
            "T-Display-S3 Arduino LVGL display demo",
            &mut route,
        );
        let context = render_context(&route);
        let after =
            fs::read(source_root.join(crate::facts::FACT_PACK_INDEX_PATH)).expect("fact packs");
        assert_eq!(before, after);
        assert!(context.contains("readiness=[display=complete]"));
        assert!(!context.contains("update board-facts"));
    }

    #[test]
    fn source_completeness_help() {
        print_source_help();
    }

    #[test]
    fn help_surfaces_do_not_require_json() {
        for args in [
            vec!["route", "--help"],
            vec!["hook", "--help"],
            vec!["doctor", "--help"],
        ] {
            let result = run(args.into_iter().map(str::to_string), std::io::empty());
            assert!(result.is_ok());
        }
    }

    #[test]
    fn update_board_facts_help() {
        print_help();
        print_update_help();
    }

    #[test]
    fn global_profile_framework_clarification_matches_project_context() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = load_registry(source_root.as_path()).expect("registry");
        let profile = ActiveProfile {
            board: "board-t-watch-ultra".to_string(),
            framework: None,
            features: Vec::new(),
        };

        let missing =
            route_with_profile_or_clarification(&registry, "LVGL watch UI demo", Some(&profile));
        assert_eq!(missing.decision, "needs_clarification");
        assert_eq!(missing.missing, vec!["framework".to_string()]);
        assert!(missing.skills.is_empty());

        let explicit = route_with_profile_or_clarification(
            &registry,
            "Arduino LVGL watch UI demo",
            Some(&profile),
        );
        assert_eq!(explicit.decision, "inject");
        assert!(explicit.skills.contains(&"board-t-watch-ultra".to_string()));
        assert!(explicit.skills.contains(&"fw-arduino".to_string()));
    }

    // Symptom/fact prompts without build intent must keep normal routing when
    // a board profile is active; the framework clarification gate only
    // applies to build-intent prompts (demo/example/build/upload/install).
    #[test]
    fn profile_lightweight_query_keeps_context_without_framework() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = load_registry(source_root.as_path()).expect("registry");
        let profile = ActiveProfile {
            board: "board-t-watch-ultra".to_string(),
            framework: None,
            features: Vec::new(),
        };

        let symptom =
            route_with_profile_or_clarification(&registry, "LVGL screen is blank", Some(&profile));
        assert_eq!(symptom.decision, "inject");
        assert!(symptom.skills.contains(&"board-t-watch-ultra".to_string()));
        assert!(symptom.skills.contains(&"fw-lvgl".to_string()));

        let noop = route_with_profile_or_clarification(
            &registry,
            "how do I prune tomato plants",
            Some(&profile),
        );
        assert_eq!(noop.decision, "no-op");
        assert!(noop.skills.is_empty());
    }

    #[test]
    fn product_runtime_parity() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp = std::env::temp_dir().join(format!(
            "lilygo-skills-runtime-parity-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        for dir in ["index", "skills", "data"] {
            copy_dir(source_root.join(dir).as_path(), temp.join(dir).as_path());
        }

        let prompt = "T-Watch Ultra ESP-IDF LVGL serial demo";
        let source_registry = load_registry(source_root.as_path()).expect("source registry");
        let runtime_registry = load_registry(temp.as_path()).expect("runtime registry");
        let source_route = route_prompt(&source_registry, prompt);
        let runtime_route = route_prompt(&runtime_registry, prompt);
        assert_eq!(source_route.skills, runtime_route.skills);
        assert!(
            runtime_route
                .skills
                .contains(&"board-t-watch-ultra".to_string())
        );
        let _ = fs::remove_dir_all(&temp);
    }

    fn copy_dir(src: &Path, dst: &Path) {
        fs::create_dir_all(dst).expect("create dst");
        for entry in fs::read_dir(src).expect("read src") {
            let entry = entry.expect("entry");
            let from = entry.path();
            let to = dst.join(entry.file_name());
            if from.is_dir() {
                copy_dir(from.as_path(), to.as_path());
            } else {
                fs::copy(&from, &to).expect("copy file");
            }
        }
    }
}
