//! Goal evidence persistence and privacy-safe path handling for local project
//! `.lilygo-skills` state.
use crate::model::{GoalEvidence, GoalStartResult};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(super) struct EvidenceWrite {
    pub(super) writes: Vec<String>,
    pub(super) evidence_path: String,
}

pub fn load_goal_evidence(project_root: &Path, goal_id: &str) -> Result<GoalEvidence, String> {
    let path = evidence_path(project_root, goal_id)?;
    if !path.is_file() {
        return Ok(GoalEvidence {
            schema_version: 1,
            goal_id: goal_id.to_string(),
            status: "not-found".to_string(),
            highest_verification_level: "V3".to_string(),
            hardware_verified: false,
            commands: Vec::new(),
            artifacts: Vec::new(),
            blockers: vec!["no local goal evidence found".to_string()],
            failure_class: None,
            failure_signature: None,
            repeated_failure_count: None,
            retry_limit: None,
            next_action: Some("run goal start with a plan to create local evidence".to_string()),
        });
    }
    let data = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&data).map_err(|error| format!("invalid {}: {error}", path.display()))
}

pub fn cancel_goal(project_root: &Path, goal_id: &str) -> Result<GoalStartResult, String> {
    let mut evidence = load_goal_evidence(project_root, goal_id)?;
    if evidence.status == "not-found" {
        return Err(format!("cannot cancel missing goal evidence for {goal_id}"));
    }
    evidence.status = "interrupted".to_string();
    evidence.next_action = Some("resume by starting from the original plan".to_string());
    let write = write_goal_evidence(project_root, &evidence)?;
    Ok(GoalStartResult {
        status: "PASS".to_string(),
        goal_id: goal_id.to_string(),
        dry_run: false,
        planned_commands: Vec::new(),
        required_permissions: Vec::new(),
        planned_artifacts: Vec::new(),
        ran_commands: Vec::new(),
        blocked_permissions: Vec::new(),
        writes: write.writes,
        evidence_path: Some(write.evidence_path),
        highest_verification_level: evidence.highest_verification_level,
        hardware_verified: evidence.hardware_verified,
        failure_class: evidence.failure_class,
        failure_signature: evidence.failure_signature,
        repeated_failure_count: evidence.repeated_failure_count,
        retry_limit: evidence.retry_limit,
        next_action: evidence.next_action,
    })
}

pub(super) fn write_goal_evidence(
    project_root: &Path,
    evidence: &GoalEvidence,
) -> Result<EvidenceWrite, String> {
    let path = evidence_path(project_root, &evidence.goal_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let rendered = serde_json::to_vec_pretty(evidence)
        .map_err(|error| format!("failed to render goal evidence: {error}"))?;
    fs::write(&path, rendered)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    let writes = vec![relative_path(project_root, &path)];
    Ok(EvidenceWrite {
        writes,
        evidence_path: relative_path(project_root, &path),
    })
}

fn evidence_path(project_root: &Path, goal_id: &str) -> Result<PathBuf, String> {
    validate_goal_id(goal_id)?;
    Ok(project_root
        .join(".lilygo-skills")
        .join("evidence")
        .join(goal_id)
        .join("evidence.json"))
}

pub(super) fn validate_goal_id(goal_id: &str) -> Result<(), String> {
    let Some(suffix) = goal_id.strip_prefix("goal-") else {
        return Err(format!("invalid goal id: {goal_id}"));
    };
    if suffix.len() != 12 || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid goal id: {goal_id}"));
    }
    if Path::new(goal_id)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("invalid goal id: {goal_id}"));
    }
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
