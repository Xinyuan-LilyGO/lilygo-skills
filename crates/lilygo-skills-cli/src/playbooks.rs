//! Data-backed embedded playbooks.
//!
//! Playbooks are generated runtime skills plus compact route/goal hints. They
//! describe how an agent should investigate and collect evidence; they never
//! replace source-backed board facts.

use crate::model::{Playbook, PlaybookCatalog, PlaybookHint, Registry, Skill, SkillKind};
use crate::selection::{SelectionConfig, SelectionInput};
use std::collections::{BTreeMap, BTreeSet};

const PLAYBOOKS_JSON: &str = include_str!("../../../data/playbooks/playbooks.json");
const PLAYBOOK_TRIGGERS_JSON: &str = include_str!("../../../data/playbooks/playbook-triggers.json");

pub fn playbook_catalog() -> PlaybookCatalog {
    serde_json::from_str(PLAYBOOKS_JSON).expect("embedded playbook catalog must be valid JSON")
}

fn playbook_triggers() -> SelectionConfig {
    serde_json::from_str(PLAYBOOK_TRIGGERS_JSON)
        .expect("embedded data/playbooks/playbook-triggers.json must be valid SelectionConfig")
}

pub fn validate_playbook_catalog(catalog: &PlaybookCatalog) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for playbook in &catalog.playbooks {
        if !seen.insert(playbook.id.as_str()) {
            errors.push(format!("duplicate playbook id {}", playbook.id));
        }
        if !playbook.id.starts_with("playbook-") {
            errors.push(format!("playbook {} must use playbook-* id", playbook.id));
        }
        if playbook.source_refs.is_empty() {
            errors.push(format!("playbook {} has no source_refs", playbook.id));
        }
        if playbook.diagnostic_axes.is_empty() {
            errors.push(format!("playbook {} has no diagnostic_axes", playbook.id));
        }
        if playbook.evidence_targets.is_empty() {
            errors.push(format!("playbook {} has no evidence_targets", playbook.id));
        }
        if playbook.anti_claims.is_empty() {
            errors.push(format!("playbook {} has no anti_claims", playbook.id));
        }
        if playbook.benchmark_prompts.is_empty() {
            errors.push(format!("playbook {} has no benchmark_prompts", playbook.id));
        }
        for source in &playbook.source_refs {
            if source.starts_with('/') || source.contains("/Users/") {
                errors.push(format!(
                    "playbook {} has private or absolute source ref {source}",
                    playbook.id
                ));
            }
        }
    }
    errors
}

#[cfg(test)]
pub fn required_playbook_ids() -> BTreeSet<String> {
    playbook_catalog()
        .playbooks
        .into_iter()
        .map(|playbook| playbook.id)
        .collect()
}

pub fn playbook_by_id(id: &str) -> Option<Playbook> {
    playbook_catalog()
        .playbooks
        .into_iter()
        .find(|playbook| playbook.id == id)
}

pub fn selected_playbook_ids(prompt: &str, route_skills: &[String]) -> Vec<String> {
    let mut lists = BTreeMap::new();
    lists.insert("route_skills", route_skills.to_vec());
    let input = SelectionInput {
        prompt: prompt.to_lowercase(),
        flags: BTreeMap::new(),
        lists,
    };
    crate::selection::evaluate(&playbook_triggers(), input)
}

pub fn playbook_hints_for_prompt(prompt: &str, route_skills: &[String]) -> Vec<PlaybookHint> {
    let catalog = playbook_catalog()
        .playbooks
        .into_iter()
        .map(|playbook| (playbook.id.clone(), playbook))
        .collect::<BTreeMap<_, _>>();
    selected_playbook_ids(prompt, route_skills)
        .into_iter()
        .filter_map(|id| catalog.get(&id).map(playbook_hint))
        .collect()
}

pub fn playbook_registry_skills() -> Vec<Skill> {
    playbook_catalog()
        .playbooks
        .into_iter()
        .map(|playbook| Skill {
            id: playbook.id.clone(),
            kind: SkillKind::Playbook,
            path: format!("skills/{}/SKILL.md", playbook.id),
            summary: playbook.summary.clone(),
            triggers: playbook.trigger_terms.clone(),
            aliases: playbook.domains.clone(),
            priority: 45,
            verification_level: "context-injection".to_string(),
            family_id: None,
            product: false,
        })
        .collect()
}

pub fn registry_with_playbooks(mut registry: Registry) -> Registry {
    let mut existing = registry
        .skills
        .iter()
        .map(|skill| skill.id.clone())
        .collect::<BTreeSet<_>>();
    for skill in playbook_registry_skills() {
        if existing.insert(skill.id.clone()) {
            registry.skills.push(skill);
        }
    }
    registry
}

fn playbook_hint(playbook: &Playbook) -> PlaybookHint {
    PlaybookHint {
        playbook_id: playbook.id.clone(),
        title: playbook.title.clone(),
        reason: playbook.summary.clone(),
        expand_command: format!("lilygo-skills index query {} --json", playbook.id),
        evidence_targets: playbook.evidence_targets.clone(),
        anti_claims: playbook.anti_claims.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playbook_schema() {
        let catalog = playbook_catalog();
        assert_eq!(catalog.schema_version, 1);
        assert!(validate_playbook_catalog(&catalog).is_empty());
        assert_eq!(catalog.playbooks.len(), 7);
        assert!(
            catalog
                .playbooks
                .iter()
                .any(|book| book.id == "playbook-lvgl-debug")
        );
        assert!(
            catalog
                .playbooks
                .iter()
                .any(|book| book.id == "playbook-ota-debug")
        );
    }

    #[test]
    fn embedded_playbook_catalog() {
        let ids = required_playbook_ids();
        for id in [
            "playbook-source-discovery",
            "playbook-build-flash-serial",
            "playbook-lvgl-debug",
            "playbook-ota-debug",
            "playbook-bsp-driver",
            "playbook-radio-gnss",
            "playbook-setup-toolchain",
        ] {
            assert!(ids.contains(id), "missing {id}");
        }
    }

    #[test]
    fn playbook_source_authority() {
        for playbook in playbook_catalog().playbooks {
            assert!(
                playbook.source_refs.iter().any(|source| {
                    source.contains("github.com/Xinyuan-LilyGO")
                        || source.contains("docs.espressif.com")
                        || source.contains("docs.lvgl.io")
                }),
                "{} lacks authoritative source refs",
                playbook.id
            );
            assert!(
                playbook.anti_claims.iter().any(|claim| {
                    let claim = claim.to_lowercase();
                    claim.contains("cannot prove")
                        || claim.contains("do not claim")
                        || claim.contains("planning evidence")
                        || claim.contains("local evidence")
                }),
                "{} lacks evidence-boundary wording",
                playbook.id
            );
        }
    }

    #[test]
    fn playbook_route_selector() {
        let route = vec![
            "lilygo-router".to_string(),
            "board-t-watch-ultra".to_string(),
        ];
        let ids = selected_playbook_ids("T-Watch Ultra LVGL blank screen touch debug", &route);
        assert!(ids.contains(&"playbook-source-discovery".to_string()));
        assert!(ids.contains(&"playbook-lvgl-debug".to_string()));
        assert!(
            !selected_playbook_ids("what is the weather today", &[])
                .contains(&"playbook-lvgl-debug".to_string())
        );
    }

    #[test]
    fn utf8_cjk_adjacent_playbook_prompts_do_not_panic() {
        let route = vec![
            "lilygo-router".to_string(),
            "board-t-display-s3".to_string(),
        ];
        let flash = selected_playbook_ids("T-Display-S3烧录失败", &route);
        assert!(flash.contains(&"playbook-source-discovery".to_string()));
        assert!(flash.contains(&"playbook-build-flash-serial".to_string()));

        let watch_route = vec![
            "lilygo-router".to_string(),
            "board-t-watch-ultra".to_string(),
        ];
        let gesture = selected_playbook_ids("t-watch ultra imu抬腕检测怎么做", &watch_route);
        assert!(gesture.contains(&"playbook-source-discovery".to_string()));
    }
}
