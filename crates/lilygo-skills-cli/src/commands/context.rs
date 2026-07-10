//! Shared capsule assembly and the `context` command.
//!
//! `assemble_capsule` is the ONE path that both `hook <host>` and `context`
//! use to build the injected capsule, so the two surfaces never drift. The
//! `context` command exposes that capsule as a first-class command and adds
//! best-effort board sniffing (via [`crate::board_sniff`]) for projects with no
//! `.lilygo-skills/project.json` profile.
use super::*;

/// Board-agnostic capsule assembly shared by `hook <host>` and `context`.
///
/// Given a `prompt` evaluated from `start_dir`, it resolves the active board
/// (project profile first; then -- only when `sniff` -- best-effort board
/// sniffing; then the runtime default profile), routes deterministically, and
/// returns the routed result plus the fully-assembled injected capsule:
/// critical facts + pins + expand pointers + goal guidance + honesty markers.
/// Both callers share this ONE path so the two surfaces never drift. `hook`
/// passes `sniff = false`, so its output is unchanged; `context` passes
/// `sniff = true` to gain project-file board detection.
pub(super) fn assemble_capsule(
    root: &Path,
    registry: &crate::model::Registry,
    prompt: &str,
    start_dir: &Path,
    host: &str,
    input: &str,
    sniff: bool,
) -> Result<(RouteResult, String), String> {
    let project = resolve_project_context(start_dir)?;
    let route_registry = match project.as_ref() {
        Some(project) => {
            registry_with_project_skills(registry, Some(project.project_root.as_path()))?
        }
        None => registry.clone(),
    };
    let profile = match project.as_ref() {
        Some(project) => Some(project.context.active_profile()),
        // No project.json: fall back to sniffing (context only), then to the
        // runtime default profile. Sniffing assigns a board only on unambiguous,
        // registry-known evidence, so this never fabricates a board.
        None => sniff
            .then(|| crate::board_sniff::sniff_board(start_dir, registry))
            .flatten()
            .map(|board| ActiveProfile {
                board,
                framework: None,
                features: Vec::new(),
            })
            .or_else(|| load_profile(root)),
    };
    let mut route = route_with_profile_or_clarification(&route_registry, prompt, profile.as_ref());
    attach_route_readiness(root, &route_registry, prompt, &mut route);
    let mut content = render_context(&route);
    let ledger_hints = project
        .as_ref()
        .map(|project| hints_for_route(project.project_root.as_path(), &route, prompt));
    if let Some(hints) = &ledger_hints {
        content.push_str(&render_hook_ledger_context(hints));
    }
    let mut goal_plan = None;
    if let Ok(plan) = plan_goal_with_project(root, &route_registry, prompt, &route, Some(start_dir))
    {
        content.push_str(&render_hook_goal_summary(&plan));
        goal_plan = Some(plan);
    }
    let content = if let (Some(project), Some(hints)) = (project.as_ref(), ledger_hints.as_ref()) {
        maybe_compact_project_hook_context(
            project.project_root.as_path(),
            prompt,
            &route,
            content,
            goal_plan.as_ref(),
            hints,
        )
    } else {
        content
    };
    let content = crate::session_context::maybe_compact_hook_context(
        host,
        input,
        content,
        goal_plan.as_ref(),
    );
    Ok((route, content))
}

/// `context [--project <dir>] [--json] [prompt]` exposes the hook's capsule
/// assembly as a first-class command: it resolves the active board for the
/// directory (project profile or best-effort sniffing) and prints the same
/// capsule the hook injects. With no explicit prompt it synthesizes one from the
/// resolved board so the output is the board's own capsule (CWD -> board ->
/// capsule).
pub(super) fn context_command(root: &Path, args: &[String]) -> Result<(), String> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        println!("Usage: lilygo-skills context [--project <dir>] [--json] [prompt]");
        return Ok(());
    }
    let start_dir = project_start_arg(args)?;
    let registry = load_registry(root)?;
    ensure_skill_files(root, &registry)?;
    let board = resolve_board_id(root, &registry, &start_dir, true);
    // An explicit prompt routes exactly like the hook; otherwise synthesize a
    // prompt from the resolved board so `context` returns that board's capsule.
    let prompt = match context_prompt_arg(args) {
        Some(prompt) => prompt,
        None => board
            .as_deref()
            .map(|board| board_synthetic_prompt(&registry, board))
            .unwrap_or_default(),
    };
    let (route, content) =
        assemble_capsule(root, &registry, &prompt, &start_dir, "claude", "", true)?;
    if has_flag(args, "--json") {
        return print_json(&serde_json::json!({
            "board": board,
            "board_source": route.board_source,
            "decision": route.decision,
            "skills": route.skills,
            "verification_level": route.verification_level,
            "context": content,
        }));
    }
    println!("{content}");
    Ok(())
}

/// Resolve the active board id for `start_dir` the same way `assemble_capsule`
/// does: project profile first, then (when `sniff`) best-effort sniffing, then
/// the runtime default profile.
fn resolve_board_id(
    root: &Path,
    registry: &crate::model::Registry,
    start_dir: &Path,
    sniff: bool,
) -> Option<String> {
    if let Ok(Some(project)) = resolve_project_context(start_dir) {
        return Some(project.context.board);
    }
    if sniff && let Some(board) = crate::board_sniff::sniff_board(start_dir, registry) {
        return Some(board);
    }
    load_profile(root).map(|profile| profile.board)
}

/// Build a routing prompt that resolves to `board_id`. Prefer the board's
/// longest registry trigger (a real, matchable alias); fall back to the id with
/// its `board-` prefix stripped and dashes turned into spaces.
fn board_synthetic_prompt(registry: &crate::model::Registry, board_id: &str) -> String {
    registry
        .skills
        .iter()
        .find(|skill| skill.id == board_id && skill.kind == SkillKind::Board)
        .and_then(|skill| {
            skill
                .triggers
                .iter()
                .max_by_key(|trigger| trigger.len())
                .cloned()
        })
        .unwrap_or_else(|| {
            board_id
                .strip_prefix("board-")
                .unwrap_or(board_id)
                .replace('-', " ")
        })
}

/// Optional trailing prompt for `context` (unlike the hook, a prompt is not
/// required). Returns `None` when only flags/options were supplied.
fn context_prompt_arg(args: &[String]) -> Option<String> {
    prompt_arg(args).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::load_registry;
    use std::fs;
    use std::path::Path;

    #[test]
    fn context_capsule_on_project_fixture_carries_facts_and_honesty() {
        // `context` on a project pinned via project.json must reproduce the
        // hook's board capsule: critical facts/pins plus the honesty markers.
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = load_registry(source_root.as_path()).expect("registry");
        let dir = std::env::temp_dir().join(format!("lilygo-context-proj-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let context =
            new_project_context(&registry, "board-t-display-s3", None, Vec::new()).unwrap();
        write_project_context(&dir, &context).unwrap();

        let board = resolve_board_id(source_root.as_path(), &registry, &dir, true);
        assert_eq!(board.as_deref(), Some("board-t-display-s3"));
        let prompt = board_synthetic_prompt(&registry, board.as_deref().unwrap());
        let (route, capsule) = assemble_capsule(
            source_root.as_path(),
            &registry,
            &prompt,
            &dir,
            "claude",
            "",
            true,
        )
        .expect("assemble");
        assert_eq!(route.decision, "inject");
        assert!(capsule.contains("board-t-display-s3"), "{capsule}");
        // Critical pin facts are present (T-Display-S3 backlight/power pins).
        assert!(capsule.contains("PIN_LCD_BL"), "{capsule}");
        // Honesty markers must never be dropped from an injected capsule.
        assert!(capsule.contains("hardware_verified=false"), "{capsule}");
        assert!(capsule.contains("evidence_boundary=V3"), "{capsule}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn context_sniffs_board_only_when_enabled() {
        // With no project.json, `context` (sniff=true) detects the board from
        // platformio.ini; the hook path (sniff=false) must ignore it so hook
        // output stays unchanged.
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = load_registry(source_root.as_path()).expect("registry");
        let dir = std::env::temp_dir().join(format!("lilygo-context-sniff-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("platformio.ini"),
            "[env:t-display-s3]\nboard = lilygo-t-display-s3\n",
        )
        .unwrap();
        assert_eq!(
            resolve_board_id(source_root.as_path(), &registry, &dir, true).as_deref(),
            Some("board-t-display-s3")
        );
        assert!(
            resolve_board_id(source_root.as_path(), &registry, &dir, false).is_none(),
            "sniff=false (hook path) must not detect a board from project files"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
