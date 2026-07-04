//! Reference catalog resolver that combines built-in and project references
//! into compact implementation/debug hints with source-authority boundaries.
use crate::facts;
use crate::model::{ReferenceCatalogReport, ReferenceEntry, ReferenceHint};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const PROJECT_REFERENCE_PATH: &str = ".lilygo-skills/references.json";

const BUILT_IN_REFERENCES_JSON: &str = include_str!("../../../data/references/built-in.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceFile {
    schema_version: u32,
    entries: Vec<ReferenceEntry>,
}

pub(crate) fn list_references(
    _root: &Path,
    project_start: Option<&Path>,
) -> Result<ReferenceCatalogReport, String> {
    let mut entries = built_in_entries();
    if let Some(path) = project_start.and_then(find_project_references) {
        let mut project_entries = load_reference_file(&path)?;
        entries.append(&mut project_entries);
    }
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    entries.dedup_by(|left, right| left.id == right.id);
    let source_health = entries
        .iter()
        .map(|entry| source_health(_root, entry))
        .collect();
    Ok(ReferenceCatalogReport {
        status: "PASS".to_string(),
        project_root: project_start.map(|path| path.display().to_string()),
        entries,
        source_health,
        warnings: vec![
            "reference entries are read hints and operating patterns; official source facts outrank them".to_string(),
        ],
    })
}

pub(crate) fn reference_hints_for_prompt(
    root: &Path,
    project_start: Option<&Path>,
    prompt: &str,
) -> Vec<ReferenceHint> {
    if facts::is_fact_prompt(prompt) && !implementation_reference_prompt(prompt) {
        return Vec::new();
    }
    if !implementation_reference_prompt(prompt) {
        return Vec::new();
    }
    let Ok(catalog) = list_references(root, project_start) else {
        return Vec::new();
    };
    let normalized = prompt.to_lowercase();
    let mut hints = catalog
        .entries
        .iter()
        .filter(|entry| {
            entry
                .inject_triggers
                .iter()
                .any(|trigger| normalized.contains(&trigger.to_lowercase()))
        })
        .map(|entry| ReferenceHint {
            reference_id: entry.id.clone(),
            title: entry.title.clone(),
            path_or_url: entry.path_or_url.clone(),
            reason: entry.read_when.clone(),
        })
        .collect::<Vec<_>>();
    hints.truncate(3);
    hints
}

fn built_in_entries() -> Vec<ReferenceEntry> {
    let file: ReferenceFile = serde_json::from_str(BUILT_IN_REFERENCES_JSON)
        .expect("embedded data/references/built-in.json must be valid");
    file.entries
}

fn load_reference_file(path: &Path) -> Result<Vec<ReferenceEntry>, String> {
    let data = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let file: ReferenceFile =
        serde_json::from_str(&data).map_err(|error| format!("invalid references: {error}"))?;
    if file.schema_version != 1 {
        return Err(format!(
            "unsupported reference schema_version: {}",
            file.schema_version
        ));
    }
    for entry in &file.entries {
        validate_reference_entry(entry)?;
    }
    Ok(file.entries)
}

fn validate_reference_entry(entry: &ReferenceEntry) -> Result<(), String> {
    if matches!(
        entry.authority.as_str(),
        "board-fact" | "source-fact" | "official-fact"
    ) {
        return Err(format!(
            "reference {} cannot override official board facts",
            entry.id
        ));
    }
    if is_private_reference_target(&entry.path_or_url) {
        return Err(format!(
            "reference {} uses a private local path; use a relative path or URL",
            entry.id
        ));
    }
    Ok(())
}

fn source_health(_root: &Path, entry: &ReferenceEntry) -> String {
    if entry.path_or_url.starts_with("http") || entry.path_or_url.starts_with("binflow://") {
        return format!("{}:recorded", entry.id);
    }
    if Path::new(&entry.path_or_url).is_file() {
        format!("{}:present", entry.id)
    } else {
        format!("{}:missing-local-reference", entry.id)
    }
}

fn is_private_reference_target(path_or_url: &str) -> bool {
    let lower = path_or_url.trim().to_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("binflow://")
    {
        return false;
    }
    lower.starts_with("file:/")
        || Path::new(path_or_url).is_absolute()
        || lower.contains(".lilygo-skills/evidence")
        || lower.contains("/dev/cu")
        || lower.contains("/dev/tty")
        || lower.contains("token=")
        || lower.contains("password=")
        || lower.contains("secret=")
}

fn find_project_references(start: &Path) -> Option<PathBuf> {
    let mut path = start.to_path_buf();
    loop {
        let candidate = path.join(PROJECT_REFERENCE_PATH);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !path.pop() {
            return None;
        }
    }
}

fn implementation_reference_prompt(prompt: &str) -> bool {
    let normalized = prompt.to_lowercase();
    [
        "implement",
        "debug",
        "driver",
        "setup",
        "install",
        "binflow",
        "serial",
        "lvgl",
        "ota",
        "调试",
        "实现",
        "传输",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;

    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn reference_catalog() {
        let catalog = list_references(root().as_path(), None).expect("catalog");
        assert_eq!(catalog.status, "PASS");
        assert!(
            catalog
                .entries
                .iter()
                .any(|entry| entry.id == "ref-lilygo-documentation-repo")
        );
        let rendered = serde_json::to_string(&catalog).expect("catalog json");
        assert!(!rendered.contains("/Users/"));
    }

    // The public built-in catalog must be fully resolvable for a public clone:
    // every entry is an official/public URL scheme, none references a private
    // practice layer, and none reports missing-local-reference health noise.
    #[test]
    fn reference_catalog_publicly_resolvable() {
        let catalog = list_references(root().as_path(), None).expect("catalog");
        assert!(!catalog.entries.is_empty());
        for entry in &catalog.entries {
            assert!(
                entry.path_or_url.starts_with("https://")
                    || entry.path_or_url.starts_with("http://")
                    || entry.path_or_url.starts_with("binflow://"),
                "built-in reference {} must use a public URL scheme: {}",
                entry.id,
                entry.path_or_url
            );
        }
        for health in &catalog.source_health {
            assert!(
                health.ends_with(":recorded"),
                "built-in reference health must be recorded for public clones: {health}"
            );
        }
        let rendered = serde_json::to_string(&catalog).expect("catalog json");
        let private_layer = concat!("vil", "ya");
        assert!(!rendered.to_lowercase().contains(private_layer));
    }

    #[test]
    fn reference_injection_triggers() {
        let hints = reference_hints_for_prompt(
            root().as_path(),
            None,
            "T-Watch Ultra 用 binflow 传输并串口调试",
        );
        let ids = hints
            .iter()
            .map(|hint| hint.reference_id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(ids.contains("ref-binflow-transfer"));
        assert!(ids.contains("ref-serial-mcp-server"));
        assert!(hints.len() <= 3);
        let rendered = serde_json::to_string(&hints).expect("hints json");
        assert!(!rendered.contains("/Users/"));

        let fact_only = reference_hints_for_prompt(
            root().as_path(),
            None,
            "T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?",
        );
        assert!(fact_only.is_empty());
    }

    #[test]
    fn reference_authority_boundary() {
        let project =
            std::env::temp_dir().join(format!("lilygo-reference-authority-{}", std::process::id()));
        let refs_dir = project.join(".lilygo-skills");
        let _ = fs::remove_dir_all(&project);
        fs::create_dir_all(&refs_dir).expect("refs dir");
        fs::write(
            refs_dir.join("references.json"),
            r#"{
              "schema_version": 1,
              "entries": [{
                "id": "bad-board-fact-reference",
                "title": "Bad board fact override",
                "kind": "local-doc",
                "applies_to": ["io"],
                "path_or_url": "local.md",
                "authority": "board-fact",
                "summary": "invalid",
                "read_when": "never",
                "inject_triggers": ["io"]
              }]
            }"#,
        )
        .expect("bad ref");
        let error = list_references(root().as_path(), Some(project.as_path()))
            .expect_err("fact override must fail");
        assert!(error.contains("cannot override official board facts"));
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn reference_rejects_private_local_paths() {
        let project =
            std::env::temp_dir().join(format!("lilygo-reference-private-{}", std::process::id()));
        let refs_dir = project.join(".lilygo-skills");
        let _ = fs::remove_dir_all(&project);
        fs::create_dir_all(&refs_dir).expect("refs dir");
        fs::write(
            refs_dir.join("references.json"),
            r#"{
              "schema_version": 1,
              "entries": [{
                "id": "private-local-reference",
                "title": "Private local reference",
                "kind": "local-doc",
                "applies_to": ["debug"],
                "path_or_url": "/private/debug.md",
                "authority": "operating-pattern",
                "summary": "invalid",
                "read_when": "never",
                "inject_triggers": ["debug"]
              }]
            }"#,
        )
        .expect("private ref");
        let error = list_references(root().as_path(), Some(project.as_path()))
            .expect_err("private local reference must fail");
        assert!(error.contains("private local path"));
        assert!(!error.contains("/private"));
        let _ = fs::remove_dir_all(&project);
    }
}
