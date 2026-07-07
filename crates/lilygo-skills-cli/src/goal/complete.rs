//! Completion-state coordinator for agents.
//!
//! `goal complete` is read-only until explicit permissions allow the existing
//! goal runner to execute build, flash, serial, network, OTA, or simulator
//! steps. This module only composes readiness and redacted public JSON.

use super::{GoalStartOptions, plan_goal_with_project, start_goal};
use crate::generate::{generate_skills, verify_generated_root};
use crate::model::{GoalPlan, GoalStartResult, Registry, RouteResult, SetupPlan};
use crate::project_ledger::{hints_for_route, record_goal_capabilities};
use crate::setup_plan::setup_plan;
use crate::text_match::contains_any;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub type GoalCompleteResult = Value;

#[derive(Debug, Clone)]
pub struct GoalCompleteOptions {
    pub project_root: PathBuf,
    pub project_start: PathBuf,
    pub generated_root: Option<PathBuf>,
    pub allow_generate: bool,
    pub start_options: GoalStartOptions,
}

struct Stage {
    status: &'static str,
    summary: String,
    missing: Vec<String>,
    commands: Vec<String>,
}

pub fn complete_goal(
    root: &Path,
    registry: &Registry,
    prompt: &str,
    route: &RouteResult,
    options: GoalCompleteOptions,
) -> Result<GoalCompleteResult, String> {
    let private = options
        .project_root
        .join(".lilygo-skills/local.json")
        .is_file();
    if route.decision != "inject" {
        return Ok(non_injected(registry, prompt, route, private));
    }

    let plan = plan_goal_with_project(
        root,
        registry,
        prompt,
        route,
        Some(options.project_start.as_path()),
    )?;
    let generated = generated_stage(root, &options, prompt)?;
    let source = source_stage(&plan);
    let setup = setup_stage(&plan, options.project_start.as_path());
    let project = project_stage(&plan);
    let missing_permissions = missing_permissions(&plan, &options.start_options);
    let mut actions = stage_actions(&generated, &source, &setup);
    let mut execution = json!({"attempted": false, "steps": []});
    let mut evidence = plan_evidence(&plan);
    let mut ledger_writes = Vec::new();
    let ledger_hints = hints_for_route(options.project_root.as_path(), route, prompt);
    let status = if plan.decision == "needs_clarification" || !plan.missing.is_empty() {
        for question in &plan.questions {
            actions.push(action(
                "ask_user",
                &question.prompt,
                &question.examples.join(", "),
            ));
        }
        "needs_clarification"
    } else if generated.status == "missing" {
        "needs_generation"
    } else if source.status == "needs_source_ingestion" {
        "needs_source_ingestion"
    } else if setup.status == "missing" {
        "needs_setup"
    } else if !missing_permissions.is_empty() {
        actions.push(action(
            "request_permission",
            &missing_permissions.join(" "),
            "execution requires explicit permission",
        ));
        "needs_permission"
    } else if options.start_options.dry_run || plan.permissions_required.is_empty() {
        "planned"
    } else {
        let result = start_goal(&plan, &options.start_options)?;
        execution = start_execution(&result);
        evidence = start_evidence(&result);
        if result.status == "PASS" {
            ledger_writes = record_goal_capabilities(
                options.project_root.as_path(),
                prompt,
                &plan,
                &result.highest_verification_level,
                result.hardware_verified,
                result.evidence_path.as_deref(),
            )?;
        }
        start_status(&result)
    };

    Ok(json!({
        "schema_version": 1,
        "status": status,
        "prompt": prompt,
        "route": plan.route,
        "readiness": {
            "generated_skills": stage_json(generated),
            "source": stage_json(source),
            "setup": stage_json(setup),
            "project": stage_json(project)
        },
        "plan": {
            "goal_id": plan.goal_id,
            "recipe_ids": plan.recipe_ids,
            "required_permissions": plan.permissions_required,
            "planned_artifacts": plan.planned_artifacts
        },
        "execution": execution,
        "evidence": evidence,
        "project_ledger": {
            "read": ledger_hints,
            "writes": ledger_writes
        },
        "next_actions": actions,
        "privacy": privacy(private)
    }))
}

fn non_injected(
    registry: &Registry,
    prompt: &str,
    route: &RouteResult,
    private: bool,
) -> GoalCompleteResult {
    let clarify = route.decision == "needs_clarification" || embedded_like(prompt);
    let mut actions = Vec::new();
    if clarify && route.questions.is_empty() {
        actions.push(action(
            "ask_user",
            "Which LilyGO ESP32-family board are you using?",
            "board identity is required before planning",
        ));
    }
    let project = if clarify {
        Stage {
            status: "needs_clarification",
            summary: "board identity is missing".to_string(),
            missing: vec!["board".to_string()],
            commands: board_examples(registry),
        }
    } else {
        skipped("not_applicable", "no project context needed")
    };
    json!({
        "schema_version": 1,
        "status": if clarify { "needs_clarification" } else { "no_op" },
        "prompt": prompt,
        "route": {"skills": route.skills},
        "readiness": {
            "generated_skills": stage_json(skipped("not_checked", "route did not require generated skills")),
            "source": stage_json(skipped("not_applicable", "no LilyGO source topic selected")),
            "setup": stage_json(skipped("not_checked", "no framework selected")),
            "project": stage_json(project)
        },
        "plan": Value::Null,
        "execution": {"attempted": false, "steps": []},
        "evidence": {"highest_verification_level": "V0", "hardware_verified": false, "artifacts": []},
        "next_actions": actions,
        "privacy": privacy(private)
    })
}

fn generated_stage(
    root: &Path,
    options: &GoalCompleteOptions,
    prompt: &str,
) -> Result<Stage, String> {
    let requested = generated_skill_intent(prompt);
    let default_root = requested.then(|| {
        options
            .project_root
            .join(crate::generate::GENERATED_CACHE_DIR)
    });
    let generated_root = options
        .generated_root
        .as_deref()
        .or(default_root.as_deref());
    let Some(generated_root) = generated_root else {
        return Ok(skipped(
            "not_checked",
            "no generated root supplied; using source-tree registry",
        ));
    };
    if options.allow_generate {
        generate_skills(root, generated_root)?;
    }
    let report = verify_generated_root(root, generated_root);
    if report.status == "PASS" {
        return Ok(Stage {
            status: "verified",
            summary: format!("{} generated skills verified", report.present),
            missing: Vec::new(),
            commands: Vec::new(),
        });
    }
    Ok(Stage {
        status: "missing",
        summary: "generated root is missing routed support files".to_string(),
        missing: report
            .missing
            .into_iter()
            .chain(report.reference_skills_missing)
            .collect(),
        commands: generated_commands(generated_root),
    })
}

fn generated_skill_intent(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    let mentions_skill_cache = contains_any(
        &lower,
        &[
            "skill",
            "skills",
            "generated",
            "生成",
            "重新生成",
            "项目 cache",
            "项目缓存",
        ],
    );
    let asks_generation = contains_any(
        &lower,
        &[
            "generate",
            "regenerate",
            "refresh",
            "update",
            "verify",
            "check",
            "生成",
            "重新生成",
            "更新",
            "检查",
            "完整",
        ],
    );
    mentions_skill_cache && asks_generation
}

fn generated_commands(generated_root: &Path) -> Vec<String> {
    let root = generated_root.display();
    vec![
        format!("lilygo-skills generate skills --out {root} --json"),
        format!("lilygo-skills verify --generated-root {root} --json"),
    ]
}

fn source_stage(plan: &GoalPlan) -> Stage {
    let signals = &plan.context_capsule.readiness;
    if signals.is_empty() {
        return source_fact_fallback_stage(plan);
    }
    let status = if signals
        .iter()
        .any(|signal| signal.completeness == "needs_source_ingestion")
    {
        "needs_source_ingestion"
    } else if signals
        .iter()
        .any(|signal| signal.completeness == "partial")
    {
        "partial"
    } else {
        "complete"
    };
    Stage {
        status,
        summary: signals
            .iter()
            .map(|signal| format!("{}={}", signal.topic, signal.completeness))
            .collect::<Vec<_>>()
            .join(","),
        missing: signals
            .iter()
            .flat_map(|signal| signal.required_missing.clone())
            .collect(),
        commands: signals
            .iter()
            .map(|signal| {
                signal
                    .update_command
                    .clone()
                    .unwrap_or_else(|| signal.source_query_command.clone())
            })
            .collect(),
    }
}

fn source_fact_fallback_stage(plan: &GoalPlan) -> Stage {
    let fact_count = plan.context_capsule.facts.len();
    let demo_count = plan.context_capsule.demo_refs.len();
    if fact_count == 0 && demo_count == 0 {
        return skipped("not_applicable", "no source completeness topic selected");
    }
    let mut commands = Vec::new();
    if let Some(board) = plan.route.board.as_deref() {
        let topic = fallback_source_topic(plan);
        commands.push(format!(
            "lilygo-skills source query --board {board} --topic {topic} --json"
        ));
    }
    Stage {
        status: "source_backed",
        summary: format!("source-backed facts={fact_count},demo_refs={demo_count}"),
        missing: Vec::new(),
        commands,
    }
}

fn fallback_source_topic(plan: &GoalPlan) -> &'static str {
    if plan
        .context_capsule
        .facts
        .iter()
        .any(|fact| fact.key == "peripheral")
    {
        return "peripheral";
    }
    "pinout"
}

fn setup_stage(plan: &GoalPlan, project: &Path) -> Stage {
    let Some(framework) = plan.route.framework.as_deref() else {
        return skipped("not_checked", "no primary framework selected");
    };
    match setup_plan(framework, Some(project)) {
        Ok(setup) => setup_from_plan(&setup, setup_intent(&plan.prompt)),
        Err(error) => Stage {
            status: "missing",
            summary: error,
            missing: vec![framework.to_string()],
            commands: Vec::new(),
        },
    }
}

fn setup_from_plan(setup: &SetupPlan, block: bool) -> Stage {
    Stage {
        status: if block { "missing" } else { "not_checked" },
        summary: format!("{} setup plan is no-mutation", setup.framework),
        missing: if block {
            setup.host_requirements.clone()
        } else {
            Vec::new()
        },
        commands: setup.next_commands.iter().take(4).cloned().collect(),
    }
}

fn project_stage(plan: &GoalPlan) -> Stage {
    if !plan.missing.is_empty() {
        return Stage {
            status: "needs_clarification",
            summary: "project context is missing required values".to_string(),
            missing: plan.missing.clone(),
            commands: Vec::new(),
        };
    }
    if plan.route.board.is_some() || plan.route.framework.is_some() {
        skipped("resolved", "board/framework context resolved")
    } else {
        skipped("not_applicable", "no board/framework context selected")
    }
}

fn missing_permissions(plan: &GoalPlan, options: &GoalStartOptions) -> Vec<String> {
    if options.dry_run {
        return plan.permissions_required.clone();
    }
    plan.permissions_required
        .iter()
        .filter(|permission| !permission_satisfied(permission, options))
        .cloned()
        .collect()
}

fn permission_satisfied(permission: &str, options: &GoalStartOptions) -> bool {
    match permission {
        "allow-build" => options.allow_build,
        "allow-flash" => options.allow_flash,
        "allow-flash:port" => options.allow_flash && options.port.is_some(),
        "allow-serial" => options.allow_serial,
        "allow-serial:port" => options.allow_serial && options.port.is_some(),
        "allow-network" => options.allow_network,
        "allow-ota" => options.allow_ota,
        "allow-simulator" => options.allow_simulator,
        other if other.contains('+') => other
            .split('+')
            .all(|part| permission_satisfied(part, options)),
        _ => false,
    }
}

fn start_status(result: &GoalStartResult) -> &'static str {
    if !result.blocked_permissions.is_empty() {
        "needs_permission"
    } else if result.status == "PASS" {
        "complete"
    } else if result.status == "BLOCKED" {
        "blocked"
    } else {
        "failed"
    }
}

fn stage_actions(generated: &Stage, source: &Stage, setup: &Stage) -> Vec<Value> {
    let mut actions = Vec::new();
    for (kind, stage) in [
        ("generate", generated),
        ("source", source),
        ("setup", setup),
    ] {
        for command in stage.commands.iter().take(3) {
            actions.push(action(
                "run_command",
                command,
                &format!("{kind}: {}", stage.summary),
            ));
        }
    }
    actions
}

fn stage_json(stage: Stage) -> Value {
    json!({
        "status": stage.status,
        "summary": stage.summary,
        "missing": stage.missing,
        "commands": stage.commands
    })
}

fn action(kind: &str, command: &str, reason: &str) -> Value {
    json!({"kind": kind, "command": command, "reason": reason})
}

fn plan_evidence(plan: &GoalPlan) -> Value {
    json!({
        "highest_verification_level": plan.context_capsule.boundary.verification_level,
        "hardware_verified": plan.context_capsule.boundary.hardware_verified,
        "artifacts": plan.planned_artifacts
    })
}

fn start_execution(result: &GoalStartResult) -> Value {
    json!({
        "attempted": true,
        "steps": result.ran_commands,
        "failure_class": result.failure_class
    })
}

fn start_evidence(result: &GoalStartResult) -> Value {
    json!({
        "highest_verification_level": result.highest_verification_level,
        "hardware_verified": result.hardware_verified,
        "artifacts": result.planned_artifacts
    })
}

fn skipped(status: &'static str, summary: &str) -> Stage {
    Stage {
        status,
        summary: summary.to_string(),
        missing: Vec::new(),
        commands: Vec::new(),
    }
}

fn board_examples(registry: &Registry) -> Vec<String> {
    registry
        .skills
        .iter()
        .filter(|skill| skill.kind == crate::model::SkillKind::Board)
        .take(5)
        .map(|skill| skill.id.clone())
        .collect()
}

fn embedded_like(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    [
        "arduino",
        "esp-idf",
        "platformio",
        "lvgl",
        "display",
        "imu",
        "lora",
        "ota",
        "firmware",
    ]
    .iter()
    .any(|term| lower.contains(term))
}

fn setup_intent(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    ["setup", "install", "toolchain", "环境", "安装"]
        .iter()
        .any(|term| lower.contains(term))
}

fn privacy(private: bool) -> Value {
    json!({"public_output_redacted": true, "private_state_used": private})
}
