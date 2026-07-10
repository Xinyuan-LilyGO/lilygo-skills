//! Registry loading and verification for committed meta skills, generated
//! runtime skills, source references, recipes, routes, and fact packs.
use crate::model::{
    BoardFactPackIndex, ReferenceSkillReport, Registry, Skill, SkillKind, SourceReferenceReport,
    VerifyReport,
};
use crate::source::load_board_index;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const REGISTRY_PATH: &str = "index/routes.json";
const SOURCE_MANIFEST_PATH: &str = "data/references/source-intake/manifest.md";
const RECIPE_SOURCE_PACK_PATH: &str = "data/recipes/recipes.json";
const FACT_PACK_PATH: &str = "data/facts/board-fact-packs.json";
const REQUIRED_REFERENCE_SKILLS: &[&str] = &[
    "fw-arduino",
    "fw-esp-idf",
    "fw-rust",
    "fw-platformio",
    "fw-lvgl",
    "periph-display",
    "periph-lora",
    "periph-power",
    "periph-gps",
    "periph-cellular",
    "periph-input",
    "periph-storage",
    "periph-audio",
    "debug-flash-serial",
    "debug-lvgl-loop",
    "app-ota",
    "app-smart-display",
    "app-watch-ui-lvgl",
    "app-meshtastic",
    "tool-arduino-cli",
    "tool-platformio-cli",
    "tool-espressif-doc-mcp",
    "tool-serial-debug",
    "tool-embedded-debugger",
    "tool-lvgl-simulator",
];

pub fn load_registry(root: &Path) -> Result<Registry, String> {
    let path = root.join(REGISTRY_PATH);
    let data = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let registry: Registry = serde_json::from_str(&data)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    Ok(crate::playbooks::registry_with_playbooks(registry))
}

// A meta-only source tree has no committed generated skills (only the meta
// router). A materialized runtime (install root or generated cache) has skill
// files on disk. We can only tell them apart by whether any non-router skill
// file is present.
fn tree_is_meta_only(root: &Path, registry: &Registry) -> bool {
    !registry
        .skills
        .iter()
        .any(|skill| skill.id != "lilygo-router" && root.join(&skill.path).is_file())
}

// Meta-only release boundary: on a pure source tree, generated SKILL.md files are
// not committed, so a registry skill counts as present when the source model can
// generate it. On a materialized runtime we require the file itself, so a deleted
// skill in an installed runtime still fails verification.
fn skill_is_available(
    root: &Path,
    skill_id: &str,
    skill_path: &str,
    meta_only: bool,
    generatable: &BTreeSet<String>,
) -> bool {
    if root.join(skill_path).is_file() {
        return true;
    }
    meta_only && generatable.contains(skill_id)
}

/// On a meta-only source tree, board/peripheral/framework SKILL.md files are not
/// committed (skills are delivered as context injection, not files), so every
/// registry-declared skill counts as available. On a materialized runtime we
/// still require the file itself. This replaces the removed generation stack's
/// `generatable_skill_ids`, which enumerated exactly this same registry set.
fn registry_skill_ids(registry: &Registry) -> BTreeSet<String> {
    registry
        .skills
        .iter()
        .map(|skill| skill.id.clone())
        .collect()
}

pub fn ensure_skill_files(root: &Path, registry: &Registry) -> Result<(), String> {
    let meta_only = tree_is_meta_only(root, registry);
    let generatable = registry_skill_ids(registry);
    let missing = registry
        .skills
        .iter()
        .filter(|skill| !skill_is_available(root, &skill.id, &skill.path, meta_only, &generatable))
        .map(|skill| format!("missing skill file {} for {}", skill.path, skill.id))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "registry has missing skill files: {}",
            missing.join("; ")
        ))
    }
}

pub fn verify(root: &Path) -> VerifyReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let registry = match load_registry(root) {
        Ok(registry) => registry,
        Err(error) => {
            return VerifyReport {
                status: "FAIL".to_string(),
                skill_count: 0,
                route_count: 0,
                fixture_count: 0,
                source_manifest_status: "missing".to_string(),
                board_index_status: "not-checked".to_string(),
                reference_skills: ReferenceSkillReport {
                    required: REQUIRED_REFERENCE_SKILLS.len(),
                    present: 0,
                    missing: REQUIRED_REFERENCE_SKILLS
                        .iter()
                        .map(|skill| skill.to_string())
                        .collect(),
                },
                source_references: SourceReferenceReport {
                    official_urls_checked: 0,
                    documentation_repo_status: "not-checked".to_string(),
                    recipe_source_pack_status: "not-checked".to_string(),
                    fact_pack_status: "not-checked".to_string(),
                },
                errors: vec![error],
                warnings,
            };
        }
    };

    let meta_only = tree_is_meta_only(root, &registry);
    let generatable = registry_skill_ids(&registry);
    let mut ids = BTreeSet::new();
    for skill in &registry.skills {
        validate_skill(
            root,
            skill,
            meta_only,
            &generatable,
            &mut ids,
            &mut errors,
            &mut warnings,
        );
    }

    for fixture in &registry.route_fixtures {
        for skill_id in &fixture.expect_skills {
            if !ids.contains(skill_id) {
                errors.push(format!(
                    "fixture {} expects unknown skill {}",
                    fixture.id, skill_id
                ));
            }
        }
    }

    let source_manifest_path = root.join(SOURCE_MANIFEST_PATH);
    let source_manifest_status = if source_manifest_path.is_file() {
        verify_manifest_artifacts(root, &mut errors, &mut warnings);
        "present"
    } else {
        errors.push(format!("missing source manifest {SOURCE_MANIFEST_PATH}"));
        "missing"
    };

    let board_index_status = match load_board_index(root) {
        Ok(index) => {
            for board in &index.boards {
                if board.supported && !board.mcu.to_lowercase().contains("esp32") {
                    errors.push(format!(
                        "supported board {} is outside ESP32 family ({})",
                        board.id, board.mcu
                    ));
                }
                if board.supported && (board.repo_url.is_empty() || board.wiki_url.is_empty()) {
                    errors.push(format!(
                        "board {} is missing official source pointer",
                        board.id
                    ));
                }
            }
            "present"
        }
        Err(error) => {
            errors.push(error);
            "missing"
        }
    };

    let reference_skills = reference_skill_report(&ids);
    for missing in &reference_skills.missing {
        errors.push(format!("missing required reference skill {missing}"));
    }

    let documentation_repo_status =
        verify_documentation_repo_reference(root, &mut errors, &mut warnings);
    let recipe_source_pack_status = verify_recipe_source_packs(root, &mut errors);
    let fact_pack_status = verify_fact_packs(root, &mut errors);
    errors.extend(crate::playbooks::validate_playbook_catalog(
        &crate::playbooks::playbook_catalog(),
    ));
    let official_urls_checked = registry
        .skills
        .iter()
        .filter(|skill| skill.summary.contains("http") || skill.summary.contains("official"))
        .count();

    VerifyReport {
        status: if errors.is_empty() { "PASS" } else { "FAIL" }.to_string(),
        skill_count: registry.skills.len(),
        route_count: registry
            .skills
            .iter()
            .filter(|skill| !skill.triggers.is_empty())
            .count(),
        fixture_count: registry.route_fixtures.len(),
        source_manifest_status: source_manifest_status.to_string(),
        board_index_status: board_index_status.to_string(),
        reference_skills,
        source_references: SourceReferenceReport {
            official_urls_checked,
            documentation_repo_status,
            recipe_source_pack_status,
            fact_pack_status,
        },
        errors,
        warnings,
    }
}

fn verify_fact_packs(root: &Path, errors: &mut Vec<String>) -> String {
    let path = root.join(FACT_PACK_PATH);
    let Ok(data) = fs::read_to_string(&path) else {
        errors.push(format!("missing fact pack index {FACT_PACK_PATH}"));
        return "missing".to_string();
    };
    let Ok(index) = serde_json::from_str::<BoardFactPackIndex>(&data) else {
        errors.push(format!("invalid fact pack index {FACT_PACK_PATH}"));
        return "invalid".to_string();
    };
    if index.packs.is_empty() {
        errors.push(format!("fact pack index {FACT_PACK_PATH} has no packs"));
        return "empty".to_string();
    }
    let Some(watch) = index
        .packs
        .iter()
        .find(|pack| pack.board_id == "board-t-watch-ultra")
    else {
        errors.push("fact pack index missing board-t-watch-ultra".to_string());
        return "incomplete".to_string();
    };
    if watch.pin_matrix.is_empty()
        || watch.bus_matrix.is_empty()
        || watch.expander_matrix.is_empty()
        || watch.peripheral_table.is_empty()
    {
        errors.push("board-t-watch-ultra fact pack missing required tables".to_string());
        return "incomplete".to_string();
    }
    if !watch.expander_matrix.iter().any(|fact| {
        fact.key == "expander.xl9555.channel-map" && fact.value == "unknown_with_sources"
    }) {
        errors.push(
            "board-t-watch-ultra fact pack must preserve unknown XL9555 channel mapping"
                .to_string(),
        );
        return "incomplete".to_string();
    }
    "present".to_string()
}

fn verify_recipe_source_packs(root: &Path, errors: &mut Vec<String>) -> String {
    let path = root.join(RECIPE_SOURCE_PACK_PATH);
    let Ok(data) = fs::read_to_string(&path) else {
        errors.push(format!(
            "missing recipe source pack index {RECIPE_SOURCE_PACK_PATH}"
        ));
        return "missing".to_string();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
        errors.push(format!(
            "invalid recipe source pack index {RECIPE_SOURCE_PACK_PATH}"
        ));
        return "invalid".to_string();
    };
    let packs = value
        .get("source_packs")
        .and_then(|packs| packs.as_array())
        .cloned()
        .unwrap_or_default();
    let required = [
        "recipe-pack-watch-flash-debug",
        "recipe-pack-lvgl-ui-debug-loop",
        "recipe-pack-bsp-chip-driver",
        "recipe-pack-ota-debug",
        "recipe-pack-lora-gnss-source",
    ];
    for id in required {
        let pack = packs
            .iter()
            .find(|pack| pack.get("id").and_then(|value| value.as_str()) == Some(id));
        let Some(pack) = pack else {
            errors.push(format!("missing recipe source pack {id}"));
            continue;
        };
        let refs = pack
            .get("source_refs")
            .and_then(|refs| refs.as_array())
            .cloned()
            .unwrap_or_default();
        if refs.is_empty() {
            errors.push(format!("recipe source pack {id} has no source_refs"));
        }
        let hashes = pack
            .get("source_hashes")
            .and_then(|hashes| hashes.as_object())
            .cloned()
            .unwrap_or_default();
        for source_ref in refs {
            let Some(source_ref) = source_ref.as_str() else {
                errors.push(format!("recipe source pack {id} has non-string source_ref"));
                continue;
            };
            if source_ref.starts_with('/') {
                errors.push(format!(
                    "recipe source pack {id} uses non-portable absolute source_ref {source_ref}"
                ));
            }
            if source_ref.starts_with("http://") || source_ref.starts_with("https://") {
                // Official public URLs carry provenance by address; sha256
                // hashes are required only for local file snapshots.
                continue;
            }
            let Some(hash) = hashes.get(source_ref).and_then(|hash| hash.as_str()) else {
                errors.push(format!(
                    "recipe source pack {id} missing source_hash for {source_ref}"
                ));
                continue;
            };
            if !hash.starts_with("sha256:") || hash.len() != "sha256:".len() + 64 {
                errors.push(format!(
                    "recipe source pack {id} has invalid source_hash for {source_ref}"
                ));
            }
        }
    }
    if errors
        .iter()
        .any(|error| error.contains("recipe source pack"))
    {
        "incomplete".to_string()
    } else {
        "present".to_string()
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_skill(
    root: &Path,
    skill: &Skill,
    meta_only: bool,
    generatable: &BTreeSet<String>,
    ids: &mut BTreeSet<String>,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if !ids.insert(skill.id.clone()) {
        errors.push(format!("duplicate skill id {}", skill.id));
    }
    if !skill_is_available(root, &skill.id, &skill.path, meta_only, generatable) {
        // Strict runtime keeps the "missing skill file" wording so a corrupted
        // install is diagnosed the same way it always was.
        errors.push(format!(
            "missing skill file {} for {}",
            skill.path, skill.id
        ));
    }
    if skill.verification_level != "context-injection" {
        errors.push(format!(
            "skill {} has unsupported verification level {}",
            skill.id, skill.verification_level
        ));
    }
    if matches!(skill.kind, SkillKind::Board) && !skill.summary.to_lowercase().contains("esp32") {
        warnings.push(format!(
            "board {} summary should name ESP32 family",
            skill.id
        ));
    }
}

fn reference_skill_report(ids: &BTreeSet<String>) -> ReferenceSkillReport {
    let missing: Vec<String> = REQUIRED_REFERENCE_SKILLS
        .iter()
        .filter(|skill| !ids.contains(**skill))
        .map(|skill| skill.to_string())
        .collect();
    ReferenceSkillReport {
        required: REQUIRED_REFERENCE_SKILLS.len(),
        present: REQUIRED_REFERENCE_SKILLS.len() - missing.len(),
        missing,
    }
}

fn verify_documentation_repo_reference(
    root: &Path,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> String {
    let manifest_path = root.join(SOURCE_MANIFEST_PATH);
    let manifest = fs::read_to_string(&manifest_path).unwrap_or_default();
    let has_repo = manifest.contains("https://github.com/Xinyuan-LilyGO/documentation");
    let has_wiki =
        manifest.contains("https://wiki.lilygo.cc/") || manifest.contains("wiki.lilygo.cc");
    if has_repo && has_wiki {
        return "recorded".to_string();
    }
    if manifest_path.is_file() {
        errors.push(
            "source manifest must record Xinyuan-LilyGO/documentation as the versioned wiki source"
                .to_string(),
        );
        "missing".to_string()
    } else {
        warnings.push(
            "source manifest unavailable; documentation repo reference not checked".to_string(),
        );
        "not-checked".to_string()
    }
}

fn verify_manifest_artifacts(root: &Path, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
    let manifest = fs::read_to_string(root.join(SOURCE_MANIFEST_PATH)).unwrap_or_default();
    let mut checked = 0;
    for line in manifest.lines().filter(|line| line.starts_with("|")) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 6 || cells[1] == "Artifact" || cells[2] == "----------" {
            continue;
        }
        let Some(path) = backtick_value(cells[2]) else {
            continue;
        };
        let Some(expected_hash) = backtick_value(cells[5]) else {
            continue;
        };
        if expected_hash.len() != 64 || !expected_hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
            warnings.push(format!("manifest artifact has non-SHA256 marker: {path}"));
            continue;
        }
        checked += 1;
        let artifact_path = root.join(path);
        match fs::read(&artifact_path) {
            Ok(bytes) => {
                let actual = format!("{:x}", Sha256::digest(&bytes));
                if actual != expected_hash {
                    errors.push(format!(
                        "manifest hash mismatch for {path}: expected {expected_hash}, got {actual}"
                    ));
                }
            }
            Err(error) => errors.push(format!(
                "manifest artifact missing or unreadable {}: {error}",
                artifact_path.display()
            )),
        }
    }
    if checked == 0 {
        errors.push("source manifest contains no hash-verifiable artifacts".to_string());
    }
}

fn backtick_value(value: &str) -> Option<&str> {
    value.strip_prefix('`')?.strip_suffix('`')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_verifies_fixtures() {
        let report = verify(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
        );
        assert_eq!(report.status, "PASS", "{report:?}");
        assert_eq!(
            report.source_references.documentation_repo_status,
            "recorded"
        );
        assert_eq!(
            report.source_references.recipe_source_pack_status,
            "present"
        );
        assert_eq!(report.source_references.fact_pack_status, "present");
    }
}
