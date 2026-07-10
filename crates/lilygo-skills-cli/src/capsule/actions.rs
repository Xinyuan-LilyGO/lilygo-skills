//! Compact, permission-aware next-action hints for goal capsules.
use super::*;

pub(super) fn add_project_skill_hints(hints: &mut Vec<GoalInternalSkillHint>, route: &RouteResult) {
    for skill_id in route
        .skills
        .iter()
        .filter(|skill_id| skill_id.starts_with("project-"))
    {
        let Some(path) = route.paths.get(skill_id) else {
            continue;
        };
        hints.push(GoalInternalSkillHint {
            skill_id: skill_id.clone(),
            kind: "project-custom-skill".to_string(),
            expand_command: format!("read {path}"),
            reason: "Project-local operating pattern matched; read after official board facts."
                .to_string(),
        });
    }
    hints.truncate(6);
}

pub(super) fn next_actions_for_goal(
    board_id: &str,
    prompt: &str,
    demo_refs: &[GoalDemoRef],
    fact_tables: &[FactTablePreview],
    route: &RouteResult,
) -> Vec<GoalNextAction> {
    if route.decision != "inject" {
        return Vec::new();
    }
    let normalized = prompt.to_lowercase();
    let implementation_or_debug = crate::facts::is_implementation_or_debug_prompt(prompt);
    let fact_only = crate::facts::is_fact_prompt(prompt) && !implementation_or_debug;
    let mut actions = Vec::new();
    if implementation_or_debug {
        actions.push(next_action(
            "goal-plan-bridge",
            "Read the compact goal plan",
            format!("lilygo-skills goal plan --json {}", shell_quote(prompt)),
            "none",
            "Use the planner as the next read-only step before editing firmware or touching hardware.",
        ));
    }
    if fact_only || needs_io_expansion(&normalized, fact_tables) {
        actions.push(next_action(
            "source-query-io",
            "Check board IO facts",
            format!("lilygo-skills source query --board {board_id} --topic io --json"),
            "none",
            "Read exact pins, buses, expanders, connectors, and source refs before assigning GPIO.",
        ));
    }
    for topic in crate::facts::bus_topics_for_prompt(prompt) {
        actions.push(next_action(
            &format!("source-query-{topic}"),
            &format!("Check {topic} bus facts"),
            format!("lilygo-skills source query --board {board_id} --topic {topic} --json"),
            "none",
            "Sensor and bus prompts need the bus-specific source view before code.",
        ));
    }
    for topic in crate::facts::topics_for_prompt(prompt) {
        if !crate::facts::is_readiness_topic(&topic) {
            continue;
        }
        actions.push(next_action(
            &format!("source-query-{topic}"),
            &format!("Check {topic} source facts"),
            format!("lilygo-skills source query --board {board_id} --topic {topic} --json"),
            "none",
            "Matched peripheral prompts should expose the narrow official source slice on demand.",
        ));
    }
    if fact_only {
        return dedup_next_actions(actions, 6);
    }
    if let Some(demo) = demo_refs.first() {
        actions.push(next_action(
            "expand-board-source",
            "Open selected board source refs",
            format!("lilygo-skills index query {board_id} --json"),
            "none",
            format!(
                "The closest official demo is {}; read its source before adapting it.",
                demo.path
            ),
        ));
    }
    actions.push(next_action(
        "goal-start-dry-run",
        "Preview build/upload/monitor plan",
        format!(
            "lilygo-skills goal complete --dry-run --json {}",
            shell_quote(prompt)
        ),
        "none",
        "Confirm the execution plan and required permissions before mutating a project or device.",
    ));
    if implementation_or_debug {
        actions.push(next_action(
            "goal-build",
            "Run the build step after approval",
            format!(
                "lilygo-skills goal complete --allow-build --json {}",
                shell_quote(prompt)
            ),
            "allow-build",
            "A compiled artifact is the first evidence level above source/context planning.",
        ));
    }
    if contains_any(
        &normalized,
        &["upload", "flash", "monitor", "serial", "串口", "烧录"],
    ) {
        actions.push(next_action(
            "goal-flash-monitor",
            "Flash and observe after approval",
            format!(
                "lilygo-skills goal complete --allow-build --allow-flash --allow-serial --json {}",
                shell_quote(prompt)
            ),
            "allow-flash",
            "Device mutation and serial observation require explicit user permission.",
        ));
    }
    dedup_next_actions(actions, 8)
}

fn next_action(
    id: &str,
    label: &str,
    command: impl Into<String>,
    permission: &str,
    reason: impl Into<String>,
) -> GoalNextAction {
    GoalNextAction {
        id: id.to_string(),
        label: label.to_string(),
        command: command.into(),
        permission: permission.to_string(),
        reason: reason.into(),
    }
}

fn dedup_next_actions(actions: Vec<GoalNextAction>, max: usize) -> Vec<GoalNextAction> {
    let mut seen = BTreeSet::new();
    actions
        .into_iter()
        .filter(|action| seen.insert(action.id.clone()))
        .take(max)
        .collect()
}

fn needs_io_expansion(normalized: &str, fact_tables: &[FactTablePreview]) -> bool {
    !fact_tables.is_empty()
        || !crate::facts::bus_topics_for_prompt(normalized).is_empty()
        || contains_any(normalized, &["sensor", "sensors", "bus", "传感器"])
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
