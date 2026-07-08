//! Goal planning and execution facade.
//!
//! Planning is read-only: it composes board/framework/peripheral facts, recipe
//! ids, source references, preferences, and evidence boundaries. Execution
//! requires explicit permission flags and writes only project-local evidence.

use crate::facts::{
    completeness_signals_for_prompt, discovery_hints_for_goal, fact_tables_for_goal,
};
use crate::model::{
    BoardRecord, CompletenessSignal, ContextBudget, DemoRef, DiscoveryHint, FactTablePreview,
    GoalBoundary, GoalContextCapsule, GoalCriticalFact, GoalDemoRef, GoalEvidence, GoalFact,
    GoalImplementationStart, GoalInternalSkillHint, GoalNextAction, GoalPlan, GoalPrivacy,
    GoalRecoveryAction, GoalRoute, GoalSourceRef, GoalStartResult, PeripheralRecord, PlaybookHint,
    Recipe, Registry, RouteResult, SkillKind, SourceFact, SourceFactSource, SourceUrl,
};
use crate::peripheral_source::{load_source_pack_index, source_authority_rank};
use crate::preferences::preference_hints_for_prompt;
use crate::recipes::{classify_failure, selected_recipes};
use crate::reference_catalog::reference_hints_for_prompt;
use crate::source::load_board_index;
pub use evidence::{cancel_goal, load_goal_evidence};
use evidence::{validate_goal_id, write_goal_evidence};
use runner::{
    allowed_commands, blocked_permissions, command_error_evidence, command_failure_summary,
    executable_commands, highest_level, next_action, planned_commands, redact_sensitive,
    run_command,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

mod actions;
mod complete;
mod demo;
mod evidence;
mod observation;
mod runner;
pub use complete::{GoalCompleteOptions, complete_goal};

const DOCUMENTATION_REPO: &str = "https://github.com/Xinyuan-LilyGO/documentation";
const SAME_FAILURE_RETRY_LIMIT: u32 = 1;

#[derive(Debug, Clone)]
pub struct GoalStartOptions {
    pub project_root: PathBuf,
    pub dry_run: bool,
    pub allow_build: bool,
    pub allow_flash: bool,
    pub allow_serial: bool,
    pub allow_network: bool,
    pub allow_ota: bool,
    pub allow_simulator: bool,
    pub port: Option<String>,
    pub source_root: Option<PathBuf>,
}

impl GoalStartOptions {
    fn has_execution_permission(&self) -> bool {
        self.allow_build
            || self.allow_flash
            || self.allow_serial
            || self.allow_network
            || self.allow_ota
            || self.allow_simulator
    }
}

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

pub fn read_plan(path: &Path) -> Result<GoalPlan, String> {
    let data = fs::read_to_string(path)
        .map_err(|error| format!("failed to read goal plan {}: {error}", path.display()))?;
    serde_json::from_str(&data).map_err(|error| format!("invalid goal plan: {error}"))
}

pub fn start_goal(plan: &GoalPlan, options: &GoalStartOptions) -> Result<GoalStartResult, String> {
    validate_goal_id(&plan.goal_id)?;
    let planned_commands = planned_commands(plan, options);
    let public_planned_commands = public_planned_commands(&planned_commands, options);
    if options.dry_run || !options.has_execution_permission() {
        // Starting a goal without explicit allow flags is a no-write preview.
        // This prevents an agent from accidentally building, flashing, opening
        // serial, touching the network, or running OTA/simulator steps when the
        // user only asked for guidance.
        return Ok(GoalStartResult {
            status: "PASS".to_string(),
            goal_id: plan.goal_id.clone(),
            dry_run: true,
            planned_commands: public_planned_commands,
            required_permissions: plan.permissions_required.clone(),
            planned_artifacts: plan.planned_artifacts.clone(),
            ran_commands: Vec::new(),
            blocked_permissions: Vec::new(),
            writes: Vec::new(),
            evidence_path: None,
            highest_verification_level: "V3".to_string(),
            hardware_verified: false,
            failure_class: None,
            failure_signature: None,
            repeated_failure_count: None,
            retry_limit: None,
            next_action: Some("approve explicit permissions to advance beyond V3".to_string()),
        });
    }

    let executable_commands = executable_commands(&planned_commands);
    let blocked_permissions = blocked_permissions(&executable_commands, options);
    let allowed_commands = allowed_commands(&executable_commands, options);
    let runnable_permissioned = allowed_commands
        .iter()
        .any(|command| command.permission != "read-only");
    if !blocked_permissions.is_empty() && !runnable_permissioned {
        let evidence = GoalEvidence {
            schema_version: 1,
            goal_id: plan.goal_id.clone(),
            status: "blocked".to_string(),
            highest_verification_level: "V3".to_string(),
            hardware_verified: false,
            commands: Vec::new(),
            artifacts: plan.planned_artifacts.clone(),
            blockers: blocked_permissions.clone(),
            failure_class: Some("blocked-for-permission".to_string()),
            failure_signature: None,
            repeated_failure_count: None,
            retry_limit: None,
            next_action: Some("rerun with explicit allow flags and target details".to_string()),
        };
        let write = write_goal_evidence(options.project_root.as_path(), &evidence)?;
        return Ok(GoalStartResult {
            status: "BLOCKED".to_string(),
            goal_id: plan.goal_id.clone(),
            dry_run: false,
            planned_commands: public_planned_commands,
            required_permissions: plan.permissions_required.clone(),
            planned_artifacts: plan.planned_artifacts.clone(),
            ran_commands: Vec::new(),
            blocked_permissions,
            writes: write.writes,
            evidence_path: Some(write.evidence_path),
            highest_verification_level: "V3".to_string(),
            hardware_verified: false,
            failure_class: Some("blocked-for-permission".to_string()),
            failure_signature: None,
            repeated_failure_count: None,
            retry_limit: None,
            next_action: Some("rerun with explicit allow flags and target details".to_string()),
        });
    }

    let mut ran_commands = Vec::new();
    let mut blockers = Vec::new();
    for command in allowed_commands {
        match run_command(&command, options) {
            Ok(evidence) => {
                if evidence.status == "PASS" {
                    ran_commands.push(evidence);
                    continue;
                }
                blockers.push(command_failure_summary(&evidence));
                ran_commands.push(evidence);
                break;
            }
            Err(error) => {
                let redacted = redact_sensitive(&error, options);
                blockers.push(redacted.clone());
                ran_commands.push(command_error_evidence(&command, &redacted, options));
                break;
            }
        }
    }
    if ran_commands.is_empty() && blocked_permissions.is_empty() {
        blockers.push("no executable goal steps were selected for this plan".to_string());
    }

    let failure_text = format!(
        "{}\n{}",
        blockers.join("\n"),
        ran_commands
            .iter()
            .map(command_failure_summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
    let failure_class = classify_failure(&failure_text);
    let status = if !blockers.is_empty() {
        "blocked"
    } else if !blocked_permissions.is_empty() {
        "partial"
    } else {
        "complete"
    };
    let highest = highest_level(status, &ran_commands);
    let evidence_blockers = if status == "partial" {
        blocked_permissions
            .iter()
            .map(|permission| format!("pending permission: {permission}"))
            .collect()
    } else {
        blockers
    };
    let signature = failure_signature(
        status,
        failure_class.as_deref(),
        &evidence_blockers,
        &ran_commands,
    );
    let retry = retry_state(
        options.project_root.as_path(),
        &plan.goal_id,
        signature.as_deref(),
    );
    let result_next_action = failure_next_action(status, failure_class.as_deref(), &retry)
        .or_else(|| next_action(status));
    let evidence = GoalEvidence {
        schema_version: 1,
        goal_id: plan.goal_id.clone(),
        status: status.to_string(),
        highest_verification_level: highest.clone(),
        hardware_verified: highest == "V5",
        commands: ran_commands.clone(),
        artifacts: plan.planned_artifacts.clone(),
        blockers: evidence_blockers,
        failure_class: failure_class.clone(),
        failure_signature: signature.clone(),
        repeated_failure_count: retry.repeated_failure_count,
        retry_limit: retry.retry_limit,
        next_action: result_next_action.clone(),
    };
    let write = write_goal_evidence(options.project_root.as_path(), &evidence)?;
    Ok(GoalStartResult {
        status: if status == "complete" || status == "partial" {
            "PASS"
        } else {
            "BLOCKED"
        }
        .to_string(),
        goal_id: plan.goal_id.clone(),
        dry_run: false,
        planned_commands: public_planned_commands,
        required_permissions: plan.permissions_required.clone(),
        planned_artifacts: plan.planned_artifacts.clone(),
        ran_commands,
        blocked_permissions,
        writes: write.writes,
        evidence_path: Some(write.evidence_path),
        highest_verification_level: evidence.highest_verification_level,
        hardware_verified: evidence.hardware_verified,
        failure_class,
        failure_signature: signature,
        repeated_failure_count: retry.repeated_failure_count,
        retry_limit: retry.retry_limit,
        next_action: evidence.next_action,
    })
}

fn public_planned_commands(
    commands: &[crate::model::GoalCommandPlan],
    options: &GoalStartOptions,
) -> Vec<crate::model::GoalCommandPlan> {
    commands
        .iter()
        .map(|command| {
            let mut public = command.clone();
            public.command = redact_sensitive(&public.command, options);
            public.working_dir = public
                .working_dir
                .as_ref()
                .map(|working_dir| redact_sensitive(working_dir, options));
            public.argv = public
                .argv
                .iter()
                .map(|arg| redact_sensitive(arg, options))
                .collect();
            public
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct RetryState {
    repeated_failure_count: Option<u32>,
    retry_limit: Option<u32>,
}

fn failure_signature(
    status: &str,
    failure_class: Option<&str>,
    _blockers: &[String],
    commands: &[crate::model::GoalCommandEvidence],
) -> Option<String> {
    if status != "blocked" {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(failure_class.unwrap_or("unclassified"));
    for command in commands {
        if command.status == "PASS" {
            continue;
        }
        hasher.update(b"\ncommand:");
        hasher.update(command.recipe_id.as_bytes());
        hasher.update(b":");
        hasher.update(command.step_id.as_bytes());
        hasher.update(b":");
        hasher.update(command.status.as_bytes());
        hasher.update(format!("{:?}", command.exit_code).as_bytes());
    }
    Some(format!("{:x}", hasher.finalize())[..12].to_string())
}

fn retry_state(project_root: &Path, goal_id: &str, signature: Option<&str>) -> RetryState {
    let Some(signature) = signature else {
        return RetryState {
            repeated_failure_count: None,
            retry_limit: None,
        };
    };
    let previous = load_goal_evidence(project_root, goal_id).ok();
    let repeated_failure_count = previous
        .as_ref()
        .filter(|evidence| evidence.failure_signature.as_deref() == Some(signature))
        .and_then(|evidence| evidence.repeated_failure_count)
        .unwrap_or(0)
        + 1;
    RetryState {
        repeated_failure_count: Some(repeated_failure_count),
        retry_limit: Some(SAME_FAILURE_RETRY_LIMIT),
    }
}

fn failure_next_action(
    status: &str,
    failure_class: Option<&str>,
    retry: &RetryState,
) -> Option<String> {
    if status != "blocked" {
        return None;
    }
    let count = retry.repeated_failure_count?;
    let limit = retry.retry_limit?;
    if count > limit {
        return Some(format!(
            "route to problem-solving: repeated identical {} failure exceeded retry_limit={limit}",
            failure_class.unwrap_or("unclassified")
        ));
    }
    let class = failure_class.unwrap_or("unclassified");
    if class == "runtime-timeout-no-observation" {
        return Some("add boot/status serial markers or choose firmware that emits logs before rerunning bounded serial/OTA observation; the next identical failure routes to problem-solving".to_string());
    }
    if class == "ota-partition-manifest" {
        return Some("inspect partition table, manifest version/digest, rollback state, and private OTA target config before rerun; the next identical failure routes to problem-solving".to_string());
    }
    Some(format!(
        "fix the {class} blocker before rerun; the next identical failure routes to problem-solving"
    ))
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
    let completeness = plan
        .context_capsule
        .completeness
        .iter()
        .map(|(topic, status)| format!("{topic}={status}"))
        .collect::<Vec<_>>()
        .join(",");
    let playbooks = plan
        .context_capsule
        .playbook_hints
        .iter()
        .map(|hint| hint.playbook_id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let next = plan
        .context_capsule
        .next_actions
        .iter()
        .map(|action| format!("{}:{}", action.id, action.permission))
        .take(4)
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
    format!(
        " LilyGO goal capsule: goal_id={}; recipes=[{}]; playbooks=[{}]; next=[{}];{} facts=[{}];{}{} completeness=[{}]; fact_tables={}; discovery_hints={}; evidence_boundary={}/hardware_verified={}",
        plan.goal_id,
        plan.recipe_ids.join(","),
        playbooks,
        next,
        source_recovery,
        facts,
        pins,
        demo,
        completeness,
        plan.context_capsule.fact_tables.len(),
        plan.context_capsule.discovery_hints.len(),
        plan.context_capsule.boundary.verification_level,
        plan.context_capsule.boundary.hardware_verified
    )
}

/// Surface the official demo path the plan already picked ("start from the
/// closest official example") so an implementation prompt sees the concrete
/// `examples/.../*.ino` entry point inline instead of only recipe names. Skipped
/// when the compact source-recovery segment already carries the same demo path.
fn render_capsule_demo(plan: &GoalPlan, source_recovery: &str) -> String {
    let path = plan
        .context_capsule
        .implementation_start
        .as_ref()
        .and_then(|start| start.official_demo_path.as_deref())
        .or_else(|| {
            plan.context_capsule
                .demo_refs
                .first()
                .map(|demo| demo.path.as_str())
        });
    match path {
        Some(path) if !path.is_empty() && !source_recovery.contains(path) => {
            format!(" demo={path};")
        }
        _ => String::new(),
    }
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
    let demo = plan
        .context_capsule
        .implementation_start
        .as_ref()
        .and_then(|start| start.official_demo_path.as_deref())
        .or_else(|| {
            plan.context_capsule
                .demo_refs
                .first()
                .map(|demo| demo.path.as_str())
        })
        .unwrap_or("none");
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

mod context;
use context::{compose_context_capsule, primary_framework};

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
