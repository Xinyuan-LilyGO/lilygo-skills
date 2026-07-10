//! Capsule assembly: the read-only planning path that composes the injected
//! goal capsule (board/framework/peripheral facts, recipe ids, source refs,
//! critical pins, next actions, and evidence boundaries) and renders the compact
//! hook capsule string graded by the coverage gate.
//!
//! Extracted from `goal/` so the capsule producer survives independently of the
//! goal command machinery: `hook <host>`, `context`, and `route` all reach the
//! injected capsule through [`plan_goal_with_project`] and
//! [`render_hook_goal_summary`].

use crate::facts::{
    completeness_signals_for_prompt, discovery_hints_for_goal, fact_tables_for_goal,
};
use crate::model::{
    BoardRecord, CompletenessSignal, ContextBudget, DemoRef, DiscoveryHint, FactTablePreview,
    GoalBoundary, GoalContextCapsule, GoalCriticalFact, GoalDemoRef, GoalFact,
    GoalImplementationStart, GoalInternalSkillHint, GoalNextAction, GoalPlan, GoalPrivacy,
    GoalRecoveryAction, GoalRoute, GoalSourceRef, PeripheralRecord, PlaybookHint, Recipe, Registry,
    RouteResult, SkillKind, SourceFact, SourceFactSource, SourceUrl,
};
use crate::preferences::preference_hints_for_prompt;
use crate::recipes::selected_recipes;
use crate::reference_catalog::reference_hints_for_prompt;
use crate::source::load_board_index;
use crate::source_packs::{load_source_pack_index, source_authority_rank};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

mod actions;
mod context;
mod demo;
use context::{compose_context_capsule, primary_framework};

pub(crate) const DOCUMENTATION_REPO: &str = "https://github.com/Xinyuan-LilyGO/documentation";

// Thin convenience wrapper over `plan_goal_with_project`; only test surfaces
// call it now that the benchmark harness (its sole production caller) is gone.
#[cfg(test)]
pub fn plan_goal(
    root: &Path,
    registry: &Registry,
    prompt: &str,
    route: &RouteResult,
) -> Result<GoalPlan, String> {
    plan_goal_with_project(root, registry, prompt, route, None)
}

pub fn plan_goal_with_project(
    root: &Path,
    registry: &Registry,
    prompt: &str,
    route: &RouteResult,
    project_start: Option<&Path>,
) -> Result<GoalPlan, String> {
    let goal_route = goal_route(registry, route);
    let context_capsule = compose_context_capsule(root, prompt, route, &goal_route, project_start)?;
    let recipes = if route.decision == "inject" {
        selected_recipes(prompt, &goal_route)
    } else {
        Vec::new()
    };
    let recipe_ids = recipes
        .iter()
        .map(|recipe| recipe.id.clone())
        .collect::<Vec<_>>();
    let source_packs = crate::recipes::source_packs_for_recipes(&recipe_ids);
    let permissions_required = permissions_required(&recipes);
    let planned_artifacts = planned_artifacts(&recipes);
    let goal_id = goal_id(prompt, &goal_route.skills, &recipe_ids);
    Ok(GoalPlan {
        schema_version: 1,
        status: "PASS".to_string(),
        goal_id,
        prompt: prompt.to_string(),
        decision: goal_decision(route),
        route: goal_route,
        context_capsule,
        recipe_ids,
        recipes,
        source_packs,
        permissions_required,
        planned_artifacts,
        privacy: GoalPrivacy {
            committed_state: vec![
                "goal prompt, route ids, recipe ids, source urls, and public docs".to_string(),
            ],
            local_state: vec![
                ".lilygo-skills/local.json".to_string(),
                ".lilygo-skills/evidence/".to_string(),
            ],
        },
        missing: route.missing.clone(),
        questions: route.questions.clone(),
        warnings: goal_warnings(route),
    })
}

pub fn render_hook_goal_summary(plan: &GoalPlan) -> String {
    if plan.decision != "planned" {
        return String::new();
    }
    let facts = plan
        .context_capsule
        .facts
        .iter()
        .filter(|fact| matches!(fact.key.as_str(), "chip" | "bus" | "driver"))
        .map(|fact| format!("{}={}", fact.key, fact.value))
        .collect::<Vec<_>>()
        .join(",");
    let playbooks = plan
        .context_capsule
        .playbook_hints
        .iter()
        .map(|hint| hint.playbook_id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let source_recovery = render_compact_source_recovery(plan);
    // Source-recovery prompts already surface the concrete pins via critical=[..]
    // and the demo path via demo=..; only lookup/fact prompts (no source-recovery
    // segment) need the inline pins/demo, so render them there to avoid both
    // duplication and busting the capsule byte budget.
    let pins = if source_recovery.is_empty() {
        render_capsule_pins(plan, &facts)
    } else {
        String::new()
    };
    let demo = render_capsule_demo(plan, &source_recovery);
    // Injection de-noise: the injected capsule dropped internal-bookkeeping
    // fields that carry no answer value -- the random `goal_id` hash, empty
    // `recipes=[]`/`playbooks=[]` arrays, the pure `fact_tables`/`discovery_hints`
    // counts, and `completeness=[..]` (a byte-for-byte duplicate of the route
    // prefix's `readiness=[..]`; the semantically clearer readiness stays). The
    // answer-bearing pins/facts/demo/headers/critical/recovery segments and the
    // honesty markers (`evidence_boundary`/`hardware_verified`) are unchanged.
    //
    // `next=[..]` is deliberately KEPT even when every entry is permission=none.
    // Its `source-query-<topic>` entries are model-actionable routing hints, and
    // the coverage grader credits their tokens (bus names like spi/uart, and the
    // literal "source query" pointer). Removing it regressed covered 53->50, so
    // per the protective-assertion rule this bookkeeping-looking field stays:
    // deleting it would weaken the coverage guard.
    let mut capsule = String::from(" LilyGO goal capsule:");
    if !plan.recipe_ids.is_empty() {
        capsule.push_str(&format!(" recipes=[{}];", plan.recipe_ids.join(",")));
    }
    if !playbooks.is_empty() {
        capsule.push_str(&format!(" playbooks=[{playbooks}];"));
    }
    let next = plan
        .context_capsule
        .next_actions
        .iter()
        .map(|action| format!("{}:{}", action.id, action.permission))
        .take(4)
        .collect::<Vec<_>>()
        .join(",");
    if !next.is_empty() {
        capsule.push_str(&format!(" next=[{next}];"));
    }
    capsule.push_str(&source_recovery);
    capsule.push_str(&format!(" facts=[{facts}];"));
    capsule.push_str(&pins);
    capsule.push_str(&demo);
    // Behavioral guidance, placed next to the honesty markers: steer the model
    // to fetch exact pins/buses from the source query before asserting them, and
    // never to fabricate GPIO numbers. Whether this actually reduces pin
    // hallucination is a live-model behavior claim that cannot be proven by these
    // deterministic tests -- it needs a real live-model A/B run (see the report).
    capsule.push_str(GUIDANCE_LINE);
    capsule.push_str(&format!(
        " evidence_boundary={}/hardware_verified={}",
        plan.context_capsule.boundary.verification_level,
        plan.context_capsule.boundary.hardware_verified
    ));
    capsule
}

/// Fetch-before-claim behavioral guidance surfaced in every board goal capsule,
/// immediately before the honesty markers.
pub(crate) const GUIDANCE_LINE: &str = " guidance=verify exact pins/buses via 'lilygo-skills source query' before claiming them; do not invent pin numbers;";

/// Surface the official demo path the plan already picked ("start from the
/// closest official example") so an implementation prompt sees the concrete
/// `examples/.../*.ino` entry point inline instead of only recipe names. Skipped
/// when the compact source-recovery segment already carries the same demo path.
fn render_capsule_demo(plan: &GoalPlan, source_recovery: &str) -> String {
    match capsule_demo_path(plan) {
        Some(path) if !path.is_empty() && !source_recovery.contains(path) => {
            format!(" demo={path};")
        }
        _ => String::new(),
    }
}

fn capsule_demo_path(plan: &GoalPlan) -> Option<&str> {
    plan.context_capsule
        .implementation_start
        .as_ref()
        .and_then(|start| start.official_demo_path.as_deref())
        .or_else(|| {
            plan.context_capsule
                .demo_refs
                .first()
                .map(|demo| demo.path.as_str())
        })
}

/// Surface the exact source-backed pin/bus GPIO assignments already loaded into
/// the capsule's fact tables so a lookup prompt ("which GPIOs does the display
/// occupy?", "what are the I2C pins?") gets the concrete pins inline instead of
/// only an expand pointer. Bounded so the injected capsule stays small: the
/// fact tables are already prompt-relevant, so we render the top concrete
/// GPIO/address rows and cap the segment length.
fn render_capsule_pins(plan: &GoalPlan, facts: &str) -> String {
    const MAX_ROWS: usize = 5;
    const MAX_SEGMENT_BYTES: usize = 320;
    // Keep at most one row per semantic slot: the fact tables carry the same
    // GPIO several times (pin.i2c.sda, i2c.primary.sda, bus.i2c.primary all pin
    // SDA), so slot dedup stops redundant I2C rows from crowding out the display
    // occupancy the lookup also asked for. Slots render in a fixed priority so
    // the byte cap trims the least-asked-for pin, never the primary bus/display.
    let mut best_per_slot: std::collections::BTreeMap<
        (u8, &'static str),
        &crate::model::SourceFact,
    > = std::collections::BTreeMap::new();
    for row in plan
        .context_capsule
        .fact_tables
        .iter()
        .flat_map(|table| table.rows.iter())
    {
        if row.confidence == "unknown_with_sources" || row.value == "unknown_with_sources" {
            continue;
        }
        if !is_concrete_pin_fact(&row.key, &row.value) {
            continue;
        }
        // Skip anything the chip/bus/driver facts already carry verbatim.
        if facts.contains(&row.value) {
            continue;
        }
        let slot = pin_slot(&row.key, &row.value);
        // Only the asked-for pin families (I2C bus, display occupancy, backlight/
        // power) earn inline bytes; secondary pins stay behind the expand pointer.
        if slot.0 >= 3 {
            continue;
        }
        best_per_slot.entry(slot).or_insert(row);
    }
    let mut rendered: Vec<String> = Vec::new();
    let mut bytes = 0usize;
    for row in best_per_slot.values() {
        let entry = format!("{}={}", row.key, row.value.trim());
        if bytes + entry.len() > MAX_SEGMENT_BYTES {
            continue;
        }
        bytes += entry.len() + 1;
        rendered.push(entry);
        if rendered.len() >= MAX_ROWS {
            break;
        }
    }
    if rendered.is_empty() {
        String::new()
    } else {
        format!(" pins=[{}];", rendered.join(","))
    }
}

/// Priority-ordered semantic slot for a concrete pin fact. Lower priority wins
/// the byte budget; only the first row per slot is kept.
fn pin_slot(key: &str, value: &str) -> (u8, &'static str) {
    let hay = format!("{key} {value}").to_lowercase();
    if hay.contains("sda") {
        (0, "i2c.sda")
    } else if hay.contains("scl") {
        (0, "i2c.scl")
    } else if key.to_lowercase().starts_with("bus.display")
        || key.to_lowercase().starts_with("display.bus")
    {
        (1, "display.bus")
    } else if hay.contains("backlight") || hay.contains("bl=") || hay.contains("power_on") {
        (2, "display.power")
    } else {
        (3, "other")
    }
}

/// A fact is a concrete pin/bus assignment worth surfacing inline when its key
/// names a pin/bus/display/i2c slot and its value pins down an actual GPIO or
/// I2C address (not a prose summary or an unknown).
fn is_concrete_pin_fact(key: &str, value: &str) -> bool {
    let key_lower = key.to_lowercase();
    let value_lower = value.to_lowercase();
    let key_is_pinlike = ["pin.", "bus.", "i2c.", "display.bus", "display.backlight"]
        .iter()
        .any(|prefix| key_lower.starts_with(prefix));
    if !key_is_pinlike {
        return false;
    }
    value_lower.contains("gpio") || value_lower.contains("0x")
}

fn render_compact_source_recovery(plan: &GoalPlan) -> String {
    if plan.context_capsule.critical_facts.is_empty() {
        return String::new();
    }
    let demo = capsule_demo_path(plan).unwrap_or("none");
    let headers = plan
        .context_capsule
        .implementation_start
        .as_ref()
        .map(|start| {
            start
                .source_headers
                .iter()
                .take(2)
                .map(|source| source.rsplit('/').next().unwrap_or(source.as_str()))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    let critical = plan
        .context_capsule
        .critical_facts
        .iter()
        .take(4)
        .map(|fact| format!("{}={}", fact.key, fact.value))
        .collect::<Vec<_>>()
        .join(",");
    let recovery = plan
        .context_capsule
        .recovery_actions
        .iter()
        .take(2)
        .map(|action| action.command.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    let internal = plan
        .context_capsule
        .internal_skill_hints
        .iter()
        .take(2)
        .map(|hint| hint.expand_command.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    format!(
        " demo={demo}; headers=[{headers}]; critical=[{critical}]; recovery=[{recovery}]; internal=[{internal}];"
    )
}

fn goal_route(registry: &Registry, route: &RouteResult) -> GoalRoute {
    let mut goal_route = GoalRoute {
        skills: route.skills.clone(),
        board: None,
        framework: None,
        frameworks: Vec::new(),
        peripherals: Vec::new(),
        chips: Vec::new(),
        features: Vec::new(),
        applications: Vec::new(),
        tools: Vec::new(),
        playbooks: Vec::new(),
    };
    let skill_kinds = registry
        .skills
        .iter()
        .map(|skill| (skill.id.as_str(), &skill.kind))
        .collect::<BTreeMap<_, _>>();
    for skill in &route.skills {
        match skill_kinds.get(skill.as_str()).copied() {
            Some(SkillKind::Board) if goal_route.board.is_none() => {
                goal_route.board = Some(skill.clone());
            }
            Some(SkillKind::Board) => {}
            Some(SkillKind::Framework) => {
                goal_route.frameworks.push(skill.clone());
                if primary_framework(skill) && goal_route.framework.is_none() {
                    goal_route.framework = Some(skill.clone());
                }
            }
            Some(SkillKind::Peripheral) => goal_route.peripherals.push(skill.clone()),
            Some(SkillKind::Chip) => goal_route.chips.push(skill.clone()),
            Some(SkillKind::Feature) => goal_route.features.push(skill.clone()),
            Some(SkillKind::Application) => goal_route.applications.push(skill.clone()),
            Some(SkillKind::Debug | SkillKind::Tool) => goal_route.tools.push(skill.clone()),
            Some(SkillKind::Playbook) => goal_route.playbooks.push(skill.clone()),
            _ => {}
        }
    }
    goal_route
}

fn goal_decision(route: &RouteResult) -> String {
    if route.decision == "inject" {
        "planned".to_string()
    } else {
        route.decision.clone()
    }
}

fn permissions_required(recipes: &[Recipe]) -> Vec<String> {
    let mut permissions = BTreeSet::new();
    for recipe in recipes {
        for permission in &recipe.required_permissions {
            for part in permission.split('+') {
                if part != "read-only" {
                    permissions.insert(part.to_string());
                }
            }
        }
    }
    permissions.into_iter().collect()
}

fn planned_artifacts(recipes: &[Recipe]) -> Vec<String> {
    let mut artifacts = BTreeSet::from(["goal_plan".to_string()]);
    for recipe in recipes {
        for artifact in &recipe.artifacts {
            artifacts.insert(artifact.clone());
        }
    }
    artifacts.into_iter().collect()
}

fn goal_warnings(route: &RouteResult) -> Vec<String> {
    let mut warnings = route.notes.clone();
    warnings.push(
        "Goal planning uses official code/examples/headers before documentation text.".to_string(),
    );
    warnings.push(
        "Recipe operating-pattern references provide guidance, not board-fact authority."
            .to_string(),
    );
    warnings
}

fn goal_id(prompt: &str, skills: &[String], recipes: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prompt.as_bytes());
    for value in skills.iter().chain(recipes.iter()) {
        hasher.update([0]);
        hasher.update(value.as_bytes());
    }
    let digest = hasher.finalize();
    format!("goal-{}", hex_prefix(&digest, 12))
}

fn hex_prefix(bytes: &[u8], chars: usize) -> String {
    let rendered = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    rendered.chars().take(chars).collect()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests;
