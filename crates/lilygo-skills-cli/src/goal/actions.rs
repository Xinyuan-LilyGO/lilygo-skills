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
    if fact_only || needs_io_expansion(&normalized, fact_tables) {
        actions.push(next_action(
            "source-query-io",
            "Check board IO facts",
            format!("lilygo-skills source query --board {board_id} --topic io --json"),
            "none",
            "Read exact pins, buses, expanders, connectors, and source refs before assigning GPIO.",
        ));
    }
    if let Some(topic) = bus_topic_for_prompt(&normalized) {
        actions.push(next_action(
            &format!("source-query-{topic}"),
            &format!("Check {topic} bus facts"),
            format!("lilygo-skills source query --board {board_id} --topic {topic} --json"),
            "none",
            "Sensor and bus prompts need the bus-specific source view before code.",
        ));
    }
    if fact_only {
        return dedup_next_actions(actions, 4);
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
        "lilygo-skills goal start --plan <saved-plan.json> --dry-run --json",
        "none",
        "Confirm the execution plan and required permissions before mutating a project or device.",
    ));
    if implementation_or_debug {
        actions.push(next_action(
            "goal-build",
            "Run the build step after approval",
            "lilygo-skills goal start --plan <saved-plan.json> --allow-build --json",
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
            "lilygo-skills goal start --plan <saved-plan.json> --allow-build --allow-flash --allow-serial --json",
            "allow-flash",
            "Device mutation and serial observation require explicit user permission.",
        ));
    }
    dedup_next_actions(actions, 6)
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
        || contains_any(
            normalized,
            &[
                "sensor",
                "sensors",
                "i2c",
                "spi",
                "uart",
                "i2s",
                "gpio",
                "pin",
                "io",
                "bus",
                "传感器",
                "引脚",
                "外设",
            ],
        )
}

fn bus_topic_for_prompt(normalized: &str) -> Option<&'static str> {
    [
        ("i2c", ["i2c", "iic", "qwiic", "stemma"].as_slice()),
        ("spi", ["spi", "mosi", "miso", "sclk"].as_slice()),
        ("uart", ["uart", "serial", "tx", "rx", "串口"].as_slice()),
        ("i2s", ["i2s", "audio", "bclk", "lrclk"].as_slice()),
        ("gpio", ["gpio", "pin", "io", "引脚", "外设"].as_slice()),
    ]
    .into_iter()
    .find_map(|(topic, needles)| contains_any(normalized, needles).then_some(topic))
}
