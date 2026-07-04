//! Data-backed embedded playbooks.
//!
//! Playbooks are generated runtime skills plus compact route/goal hints. They
//! describe how an agent should investigate and collect evidence; they never
//! replace source-backed board facts.

use crate::model::{Playbook, PlaybookCatalog, PlaybookHint, Registry, Skill, SkillKind};
use crate::templates::render_template;
use crate::text_match::contains_word;
use std::collections::{BTreeMap, BTreeSet};

const PLAYBOOKS_JSON: &str = include_str!("../../../data/playbooks/playbooks.json");
const PLAYBOOK_TEMPLATE: &str = include_str!("../../../templates/skills/playbook.md");

pub fn playbook_catalog() -> PlaybookCatalog {
    serde_json::from_str(PLAYBOOKS_JSON).expect("embedded playbook catalog must be valid JSON")
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

pub fn playbook_skill_files() -> Vec<(String, String)> {
    playbook_catalog()
        .playbooks
        .into_iter()
        .map(|playbook| (playbook.id.clone(), render_playbook_skill(&playbook)))
        .collect()
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
    let normalized = normalize(prompt);
    if !is_embedded_work_prompt(&normalized, route_skills) {
        return Vec::new();
    }
    let mut ids = BTreeSet::new();
    let action = has_action_intent(&normalized);
    let source = has_source_intent(&normalized);
    if action || source {
        ids.insert("playbook-source-discovery".to_string());
    }
    if action
        && matches_any(
            &normalized,
            &[
                "build", "flash", "upload", "serial", "monitor", "boot log", "烧录", "上传", "串口",
            ],
        )
    {
        ids.insert("playbook-build-flash-serial".to_string());
    }
    if action
        && matches_any(
            &normalized,
            &[
                "lvgl",
                "display",
                "screen",
                "touch",
                "flush",
                "blank",
                "page-data",
            ],
        )
    {
        ids.insert("playbook-lvgl-debug".to_string());
    }
    if action
        && matches_any(
            &normalized,
            &[
                "ota",
                "rollback",
                "manifest",
                "partition",
                "firmware update",
            ],
        )
    {
        ids.insert("playbook-ota-debug".to_string());
        ids.insert("playbook-build-flash-serial".to_string());
    }
    if action
        && matches_any(
            &normalized,
            &[
                "bsp",
                "driver",
                "peripheral driver",
                "status action smoke",
                "capability",
            ],
        )
    {
        ids.insert("playbook-bsp-driver".to_string());
    }
    if action && matches_any(&normalized, &["lora", "gnss", "gps", "radio", "meshtastic"]) {
        ids.insert("playbook-radio-gnss".to_string());
    }
    if action
        && matches_any(
            &normalized,
            &[
                "setup",
                "install",
                "toolchain",
                "blank machine",
                "rustup",
                "node",
            ],
        )
    {
        ids.insert("playbook-setup-toolchain".to_string());
    }
    order_playbook_ids(ids)
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

fn render_playbook_skill(playbook: &Playbook) -> String {
    render_template(
        PLAYBOOK_TEMPLATE,
        &[
            ("id", playbook.id.clone()),
            ("title", playbook.title.clone()),
            ("summary", playbook.summary.clone()),
            ("load_when", playbook.load_when.clone()),
            ("sources", bullet_list(&playbook.source_refs)),
            ("facts", bullet_list(&playbook.required_board_facts)),
            ("axes", bullet_list(&playbook.diagnostic_axes)),
            ("steps", bullet_list(&playbook.steps)),
            ("failures", bullet_list(&playbook.failure_classes)),
            ("evidence", bullet_list(&playbook.evidence_targets)),
            ("evidence_boundary", bullet_list(&playbook.anti_claims)),
            ("resources", bullet_list(&playbook.resource_hints)),
        ],
    )
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

fn bullet_list(items: &[String]) -> String {
    if items.is_empty() {
        return "- none".to_string();
    }
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn order_playbook_ids(ids: BTreeSet<String>) -> Vec<String> {
    let order = [
        "playbook-source-discovery",
        "playbook-setup-toolchain",
        "playbook-build-flash-serial",
        "playbook-lvgl-debug",
        "playbook-ota-debug",
        "playbook-bsp-driver",
        "playbook-radio-gnss",
    ];
    order
        .iter()
        .filter(|id| ids.contains(**id))
        .map(|id| (*id).to_string())
        .collect()
}

fn is_embedded_work_prompt(prompt: &str, route_skills: &[String]) -> bool {
    route_skills
        .iter()
        .any(|skill| skill == "lilygo-router" || skill.starts_with("board-"))
        || matches_any(
            prompt,
            &[
                "lilygo",
                "t-display",
                "t-watch",
                "t-beam",
                "esp32",
                "arduino",
                "esp-idf",
                "platformio",
                "lvgl",
                "ota",
                "lora",
                "gnss",
            ],
        )
}

fn has_action_intent(prompt: &str) -> bool {
    matches_any(
        prompt,
        &[
            "add",
            "implement",
            "debug",
            "fix",
            "build",
            "flash",
            "upload",
            "monitor",
            "setup",
            "install",
            "run",
            "demo",
            "example",
            "fails",
            "failure",
            "blank",
            "does not",
            "怎么做",
            "实现",
            "调试",
            "安装",
            "烧录",
        ],
    )
}

fn has_source_intent(prompt: &str) -> bool {
    matches_any(
        prompt,
        &[
            "source",
            "official",
            "datasheet",
            "pinout",
            "which repo",
            "where to find",
            "资料",
            "官方",
            "数据手册",
        ],
    )
}

fn matches_any(prompt: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| contains_word(prompt, needle))
}

fn normalize(value: &str) -> String {
    value.to_lowercase().replace('_', "-")
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
    fn generated_playbook_skills() {
        let files = playbook_skill_files();
        assert_eq!(files.len(), required_playbook_ids().len());
        let lvgl = files
            .iter()
            .find(|(id, _)| id == "playbook-lvgl-debug")
            .map(|(_, content)| content)
            .expect("lvgl playbook");
        assert!(lvgl.contains("Diagnostic Axes"));
        assert!(lvgl.contains("Evidence Targets"));
        assert!(lvgl.contains("Evidence Boundary"));
        assert!(lvgl.contains("Generation Contract: templates/skills/playbook.md"));
        assert!(!lvgl.contains("{{"));
        assert!(!lvgl.contains("/Users/"));
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
