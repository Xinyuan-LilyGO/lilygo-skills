//! Intent-ranked official demo selection for goal capsules.
use super::*;

pub(super) fn sorted_demo_refs(
    board: &BoardRecord,
    route: &GoalRoute,
    prompt: &str,
) -> Vec<GoalDemoRef> {
    if !wants_demo_refs(prompt) {
        return Vec::new();
    }
    let mut demos = board.demo_refs.clone();
    demos.sort_by_key(|demo| std::cmp::Reverse(demo_score(demo, route, prompt)));
    demos.into_iter().map(goal_demo_ref).collect()
}

fn demo_score(demo: &DemoRef, route: &GoalRoute, prompt: &str) -> i32 {
    let normalized = prompt.to_lowercase();
    let target = demo.target.to_lowercase();
    let path = demo.path.to_lowercase();
    let mut score = 0;
    if route.framework.as_deref() == Some("fw-arduino") && demo.framework == "arduino" {
        score += 10;
    }
    for intent in &demo.intents {
        if intent_matches_prompt(intent, &normalized) {
            score += 45;
        }
    }
    for preferred in &demo.preferred_for {
        if normalized.contains(&preferred.to_lowercase()) {
            score += 30;
        }
    }
    for avoided in &demo.avoid_for {
        if intent_matches_prompt(avoided, &normalized) {
            score -= 45;
        }
    }
    if demo.complexity.as_deref() == Some("minimal") && is_first_run_display_prompt(&normalized) {
        score += 100;
    }
    if target.contains("factory") {
        score += if is_factory_prompt(&normalized) {
            100
        } else {
            -35
        };
    }
    if target == "imu" && contains_any(&normalized, &["imu", "bhi260ap", "gesture", "抬腕"]) {
        score += 50;
    }
    if target == "nfc" && contains_any(&normalized, &["nfc", "st25r3916"]) {
        score += 50;
    }
    if (target.contains("tft") || path.contains("/tft/"))
        && contains_any(
            &normalized,
            &["tft", "tft_espi", "tft-espi", "tftespi", "tft_e"],
        )
    {
        score += 80;
    }
    if target.contains("lvgl") && contains_any(&normalized, &["lvgl", "touch", "display"]) {
        score += 45;
    }
    score
}

fn wants_demo_refs(prompt: &str) -> bool {
    if crate::facts::is_fact_prompt(prompt)
        && !crate::facts::is_implementation_or_debug_prompt(prompt)
    {
        return contains_any(&prompt.to_lowercase(), &["demo", "example", "示例", "例程"]);
    }
    crate::facts::is_implementation_or_debug_prompt(prompt)
        || contains_any(
            &prompt.to_lowercase(),
            &[
                "demo",
                "example",
                "factory",
                "first",
                "quickstart",
                "上手",
                "示例",
                "例程",
            ],
        )
}

fn intent_matches_prompt(intent: &str, prompt: &str) -> bool {
    match intent {
        "minimal-display" | "first-run" => is_first_run_display_prompt(prompt),
        "full-factory" | "factory" => is_factory_prompt(prompt),
        "lvgl" => contains_any(prompt, &["lvgl", "touch"]),
        "ota" => contains_any(prompt, &["ota", "over the air", "无线更新"]),
        other => prompt.contains(&other.replace('-', " ")),
    }
}

fn is_first_run_display_prompt(prompt: &str) -> bool {
    contains_any(
        prompt,
        &[
            "first",
            "first screen",
            "bring-up",
            "quickstart",
            "hello",
            "tft",
            "display",
            "screen",
            "点亮",
            "上手",
            "第一个",
        ],
    ) && !is_factory_prompt(prompt)
}

fn is_factory_prompt(prompt: &str) -> bool {
    contains_any(
        prompt,
        &[
            "factory",
            "full factory",
            "all peripheral",
            "all peripherals",
            "full test",
            "出厂",
            "全功能",
        ],
    )
}

fn goal_demo_ref(demo: DemoRef) -> GoalDemoRef {
    GoalDemoRef {
        framework: demo.framework,
        target: demo.target,
        path: demo.path,
        source_url: demo.source_url,
        evidence_level: demo.evidence_level,
        stale: demo.stale,
        intents: demo.intents,
        complexity: demo.complexity,
    }
}
