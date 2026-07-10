//! Goal execution facade.
//!
//! The read-only capsule/planning path now lives in [`crate::capsule`]; this
//! module owns the permission-gated execution half: starting a goal only runs
//! build/flash/serial/network/OTA/simulator steps under explicit allow flags and
//! writes only project-local evidence.

use crate::model::{GoalCommandEvidence, GoalCommandPlan, GoalEvidence, GoalPlan, GoalStartResult};
use crate::recipes::classify_failure;
pub use evidence::{cancel_goal, load_goal_evidence};
use evidence::{validate_goal_id, write_goal_evidence};
use runner::{
    allowed_commands, blocked_permissions, command_error_evidence, command_failure_summary,
    executable_commands, highest_level, next_action, planned_commands, redact_sensitive,
    run_command,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

mod complete;
mod evidence;
mod observation;
mod runner;
pub use complete::{GoalCompleteOptions, complete_goal};

// The capsule producer moved to `crate::capsule`; re-export it so the existing
// `crate::goal::{...}` call sites (commands, session_context, complete) keep
// resolving while the goal command surface is still present.
#[cfg(test)]
pub(crate) use crate::capsule::{DOCUMENTATION_REPO, plan_goal};
pub(crate) use crate::capsule::{plan_goal_with_project, render_hook_goal_summary};

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

pub fn read_plan(path: &Path) -> Result<GoalPlan, String> {
    let data = std::fs::read_to_string(path)
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
    let highest = highest_level(&ran_commands);
    let evidence_blockers = if status == "partial" {
        blocked_permissions
            .iter()
            .map(|permission| format!("pending permission: {permission}"))
            .collect()
    } else {
        blockers
    };
    let signature = failure_signature(status, failure_class.as_deref(), &ran_commands);
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
    commands: &[GoalCommandPlan],
    options: &GoalStartOptions,
) -> Vec<GoalCommandPlan> {
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
    commands: &[GoalCommandEvidence],
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

#[cfg(test)]
mod tests;
