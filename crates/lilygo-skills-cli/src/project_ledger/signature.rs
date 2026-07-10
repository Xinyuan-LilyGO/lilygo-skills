//! Stable signatures used to invalidate stale project-ledger memory.
use crate::facts::stable_hash;
use crate::model::{CompletenessSignal, RouteResult};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(super) fn route_dimensions(route: &RouteResult) -> BTreeSet<String> {
    let mut dims = route.skills.iter().cloned().collect::<BTreeSet<_>>();
    for skill in &route.skills {
        for prefix in ["periph-", "chip-", "feature-", "app-", "fw-"] {
            if let Some(value) = skill.strip_prefix(prefix) {
                dims.insert(value.replace('-', "."));
                dims.insert(value.to_string());
            }
        }
    }
    for signal in &route.readiness {
        dims.insert(signal.board_id.clone());
        dims.insert(signal.topic.clone());
    }
    dims
}

pub(super) fn route_signature(route: &RouteResult) -> String {
    let mut dims = route_dimensions(route).into_iter().collect::<Vec<_>>();
    dims.sort();
    stable_hash(&dims.join("|"))
}

pub(super) fn source_signature_for_readiness(route: &RouteResult) -> String {
    source_signature_for_route(&route.readiness, &route.skills)
}

pub(super) fn source_signature_for_route(
    readiness: &[CompletenessSignal],
    skills: &[String],
) -> String {
    if readiness.is_empty() {
        return stable_hash(&skills.join("|"));
    }
    stable_hash(
        &readiness
            .iter()
            .map(|signal| {
                format!(
                    "{}:{}:{}:{}",
                    signal.board_id, signal.topic, signal.completeness, signal.evidence_level
                )
            })
            .collect::<Vec<_>>()
            .join("|"),
    )
}

pub(super) fn expansion_commands(route: &RouteResult) -> Vec<String> {
    route
        .readiness
        .iter()
        .map(|signal| signal.source_query_command.clone())
        .take(4)
        .collect()
}

pub(super) fn project_code_signature(project_root: &Path) -> Option<String> {
    let mut parts = Vec::new();
    collect_project_files(project_root, project_root, &mut parts).ok()?;
    if parts.is_empty() {
        return None;
    }
    parts.sort();
    Some(stable_hash(&parts.join("|")))
}

fn collect_project_files(
    root: &Path,
    current: &Path,
    parts: &mut Vec<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(current)
        .map_err(|error| format!("failed to read {}: {error}", current.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read directory entry: {error}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == ".lilygo-skills" || name == "target" || name == "node_modules"
        {
            continue;
        }
        if path.is_dir() {
            collect_project_files(root, &path, parts)?;
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let data = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        parts.push(format!(
            "{}:{}",
            relative.display(),
            stable_hash(&String::from_utf8_lossy(&data))
        ));
    }
    Ok(())
}
