//! Progressive-disclosure router that maps prompts and project profiles to the
//! smallest useful LilyGO skill set plus clarification and evidence boundaries.
use crate::model::{
    ActiveProfile, ClarificationQuestion, MatchReason, MatchedTerm, Registry, RouteResult, Skill,
    SkillKind,
};
use crate::playbooks::selected_playbook_ids;
use crate::text_match::{contains_any, contains_word};
use std::collections::{BTreeMap, BTreeSet};

const ROUTER_SKILL: &str = "lilygo-router";

pub fn route_prompt(registry: &Registry, prompt: &str) -> RouteResult {
    route_prompt_with_profile(registry, prompt, None)
}

pub fn route_prompt_with_profile(
    registry: &Registry,
    prompt: &str,
    profile: Option<&ActiveProfile>,
) -> RouteResult {
    let normalized = normalize(prompt);
    let has_signal = has_lilygo_signal(registry, &normalized);
    let profile_can_apply = profile.is_some_and(|_| is_embedded_prompt(&normalized));
    if !has_signal && !profile_can_apply {
        if needs_board_clarification(&normalized) {
            return clarification_result(
                "board",
                "Which LilyGO board is this project using?",
                board_examples(registry),
            );
        }
        return noop_result(
            "none",
            false,
            vec!["No LilyGO ESP32-family signal detected.".to_string()],
        );
    }
    if is_unsupported_non_esp32(&normalized) {
        return noop_result(
            "unsupported",
            true,
            vec![
                "Unsupported LilyGO product boundary: first implementation only supports ESP32-family boards."
                    .to_string(),
            ],
        );
    }

    let mut selected = BTreeSet::new();
    let mut matches = Vec::new();
    add_skill(
        registry,
        ROUTER_SKILL,
        "signal",
        "LilyGO/ESP32-family",
        &mut selected,
        &mut matches,
    );

    let mut explicit_boards = BTreeSet::new();
    for skill in registry
        .skills
        .iter()
        .filter(|skill| skill.id != ROUTER_SKILL)
        .filter(|skill| skill.kind != SkillKind::Feature)
        .filter(|skill| skill.kind != SkillKind::Playbook)
    {
        if let Some(value) = first_match(skill, &normalized) {
            if skill.kind == SkillKind::Board {
                explicit_boards.insert(skill.id.clone());
            }
            add_skill(
                registry,
                &skill.id,
                "keyword",
                &value,
                &mut selected,
                &mut matches,
            );
        }
    }

    add_profile_context(
        registry,
        profile,
        &explicit_boards,
        &mut selected,
        &mut matches,
    );
    add_derived_context(registry, &normalized, &mut selected, &mut matches);
    add_peripheral_feature_context(registry, &normalized, &mut selected, &mut matches);
    add_playbook_context(registry, &normalized, &mut selected, &mut matches);
    suppress_family_fallbacks(registry, &mut selected);
    suppress_exact_product_prefix_fallbacks(registry, &normalized, &mut selected);
    matches.retain(|matched| selected.contains(&matched.skill));

    // Context fallback: the primary board context is inferred (not user-named)
    // when no board/keyword signal matched the prompt yet an active profile
    // board still supplied the capsule. This only fires on the profile-fallback
    // path; keyword/board-name routes keep board_source = None (unchanged).
    let board_source =
        if !has_signal && profile.is_some_and(|profile| selected.contains(&profile.board)) {
            Some("inferred-from-project".to_string())
        } else {
            None
        };

    let skills = ordered_skills(registry, &selected);
    let paths = registry
        .skills
        .iter()
        .filter(|skill| selected.contains(skill.id.as_str()))
        .map(|skill| (skill.id.clone(), skill.path.clone()))
        .collect();

    RouteResult {
        decision: "inject".to_string(),
        skills,
        matches,
        paths,
        readiness: Vec::new(),
        missing: Vec::new(),
        questions: Vec::new(),
        verification_level: "context-injection".to_string(),
        hardware_verified: false,
        hardware_verification_boundary: needs_hardware_boundary(&normalized),
        notes: route_notes(&normalized),
        truncated: false,
        board_source,
    }
}

fn add_playbook_context(
    registry: &Registry,
    prompt: &str,
    selected: &mut BTreeSet<String>,
    matches: &mut Vec<MatchReason>,
) {
    let selected_skills = selected.iter().cloned().collect::<Vec<_>>();
    for playbook in selected_playbook_ids(prompt, &selected_skills) {
        add_skill(
            registry,
            &playbook,
            "playbook",
            "embedded playbook hint",
            selected,
            matches,
        );
    }
}

pub fn project_context_needs_framework(prompt: &str, profile: &ActiveProfile) -> bool {
    profile.framework.is_none()
        && !has_explicit_framework(prompt)
        && needs_framework_clarification(&normalize(prompt))
}

pub fn framework_clarification_result() -> RouteResult {
    clarification_result(
        "framework",
        "Which framework/toolchain should this LilyGO project use?",
        framework_examples(),
    )
}

/// Lightweight queries keep their injected context, but when an active
/// profile lacks a framework and the prompt names none, the route carries the
/// framework question alongside the context.
pub fn profile_framework_question_applies(prompt: &str, profile: &ActiveProfile) -> bool {
    profile.framework.is_none() && !has_explicit_framework(prompt)
}

pub fn framework_clarification_question() -> ClarificationQuestion {
    ClarificationQuestion {
        id: "framework".to_string(),
        prompt: "Which framework/toolchain should this LilyGO project use?".to_string(),
        examples: framework_examples(),
    }
}

fn framework_examples() -> Vec<String> {
    vec![
        "fw-arduino".to_string(),
        "fw-esp-idf".to_string(),
        "fw-platformio".to_string(),
        "fw-rust".to_string(),
    ]
}

fn noop_result(
    verification_level: &str,
    hardware_verification_boundary: bool,
    notes: Vec<String>,
) -> RouteResult {
    RouteResult {
        decision: "no-op".to_string(),
        skills: Vec::new(),
        matches: Vec::new(),
        paths: BTreeMap::new(),
        readiness: Vec::new(),
        missing: Vec::new(),
        questions: Vec::new(),
        verification_level: verification_level.to_string(),
        hardware_verified: false,
        hardware_verification_boundary,
        notes,
        truncated: false,
        board_source: None,
    }
}

fn clarification_result(id: &str, prompt: &str, examples: Vec<String>) -> RouteResult {
    RouteResult {
        decision: "needs_clarification".to_string(),
        skills: Vec::new(),
        matches: Vec::new(),
        paths: BTreeMap::new(),
        readiness: Vec::new(),
        missing: vec![id.to_string()],
        questions: vec![ClarificationQuestion {
            id: id.to_string(),
            prompt: prompt.to_string(),
            examples,
        }],
        verification_level: "none".to_string(),
        hardware_verified: false,
        hardware_verification_boundary: false,
        notes: Vec::new(),
        truncated: false,
        board_source: None,
    }
}

fn has_lilygo_signal(registry: &Registry, prompt: &str) -> bool {
    contains_any(
        prompt,
        &[
            "lilygo",
            "t-display",
            "tdisplay",
            "t display",
            "t-beam",
            "tbeam",
            "t beam",
            "t-deck",
            "tdeck",
            "t deck",
            "t-watch",
            "twatch",
            "t watch",
        ],
    ) || has_registry_board_signal(registry, prompt)
}

fn has_registry_board_signal(registry: &Registry, prompt: &str) -> bool {
    registry.skills.iter().any(|skill| {
        skill.kind == SkillKind::Board
            && skill
                .triggers
                .iter()
                .chain(skill.aliases.iter())
                .filter(|trigger| normalize(trigger).len() >= 5)
                .any(|trigger| contains_word(prompt, &normalize(trigger)))
    })
}

fn is_unsupported_non_esp32(prompt: &str) -> bool {
    contains_any(prompt, &["rp2040", "nrf52", "stm32", "risc-v", "rp2350"])
        && !contains_any(prompt, &["esp32", "esp-idf", "arduino-esp32"])
}

// Derived application/feature route triggers live in data so routing policy can
// be reviewed and updated without scattering domain terms through Rust code.
const DERIVED_CONTEXT_JSON: &str = include_str!("../../../../data/router/derived-context.json");

#[derive(serde::Deserialize)]
pub(crate) struct DerivedContextFile {
    pub entries: Vec<OwnedDerivedSpec>,
}

#[derive(serde::Deserialize)]
pub(crate) struct OwnedDerivedSpec {
    pub skill_id: String,
    pub kind: String,
    pub value: String,
    pub needles: Vec<String>,
}

pub(crate) fn derived_context_specs() -> Vec<OwnedDerivedSpec> {
    let file: DerivedContextFile = serde_json::from_str(DERIVED_CONTEXT_JSON)
        .expect("embedded data/router/derived-context.json must be valid");
    file.entries
}

// Router keyword rules live in data so the classification vocabulary (embedded-
// prompt gate + critical-fact keywords) can be reviewed and extended -- e.g.
// adding CJK hardware terms -- without editing Rust. Rust only reads the data.
const KEYWORD_RULES_JSON: &str = include_str!("../../../../data/router/keyword-rules.json");

#[derive(serde::Deserialize)]
pub(crate) struct KeywordRules {
    pub embedded_prompt: Vec<String>,
    pub critical_text: CriticalTextRules,
}

#[derive(serde::Deserialize)]
pub(crate) struct CriticalTextRules {
    pub key_prefixes: Vec<String>,
    pub keywords: Vec<String>,
}

pub(crate) fn keyword_rules() -> KeywordRules {
    serde_json::from_str(KEYWORD_RULES_JSON)
        .expect("embedded data/router/keyword-rules.json must be valid")
}

fn add_derived_context(
    registry: &Registry,
    prompt: &str,
    selected: &mut BTreeSet<String>,
    matches: &mut Vec<MatchReason>,
) {
    for spec in derived_context_specs() {
        add_when_any(registry, prompt, selected, matches, &spec);
    }
    add_runner_and_tool_context(registry, prompt, selected, matches);
}

fn add_peripheral_feature_context(
    registry: &Registry,
    prompt: &str,
    selected: &mut BTreeSet<String>,
    matches: &mut Vec<MatchReason>,
) {
    let imu_requested = contains_any(
        prompt,
        &[
            "imu",
            "bhi260ap",
            "6dof",
            "accelerometer",
            "gyroscope",
            "gesture",
            "raise-to-wake",
            "tilt-to-wake",
            "wrist raise",
            "抬腕",
        ],
    );
    let watch_ultra_selected = selected.contains("board-t-watch-ultra");
    if imu_requested && watch_ultra_selected {
        add_skill(
            registry,
            "periph-imu",
            "source-pack",
            "T-Watch Ultra IMU source pack",
            selected,
            matches,
        );
        add_skill(
            registry,
            "chip-bhi260ap",
            "source-pack",
            "Bosch BHI260AP source pack",
            selected,
            matches,
        );
    }
    let has_compatible_imu = selected.contains("periph-imu") || selected.contains("chip-bhi260ap");
    if has_compatible_imu && is_raise_to_wake_intent(prompt) {
        add_skill(
            registry,
            "feature-raise-to-wake",
            "feature",
            "raise-to-wake requires IMU context",
            selected,
            matches,
        );
    }
}

fn add_profile_context(
    registry: &Registry,
    profile: Option<&ActiveProfile>,
    explicit_boards: &BTreeSet<String>,
    selected: &mut BTreeSet<String>,
    matches: &mut Vec<MatchReason>,
) {
    let Some(profile) = profile else {
        return;
    };
    if explicit_board_overrides_profile(registry, explicit_boards, profile) {
        return;
    }
    add_skill(
        registry,
        &profile.board,
        "profile",
        "active board profile",
        selected,
        matches,
    );
    if let Some(framework) = &profile.framework {
        add_skill(
            registry,
            framework,
            "profile",
            "active framework profile",
            selected,
            matches,
        );
    }
    for feature in &profile.features {
        add_skill(
            registry,
            feature,
            "profile",
            "active feature profile",
            selected,
            matches,
        );
    }
}

fn explicit_board_overrides_profile(
    registry: &Registry,
    explicit_boards: &BTreeSet<String>,
    profile: &ActiveProfile,
) -> bool {
    explicit_boards.iter().any(|board_id| {
        if board_id == &profile.board {
            return false;
        }
        let Some(profile_skill) = registry
            .skills
            .iter()
            .find(|skill| skill.id == profile.board)
        else {
            return true;
        };
        profile_skill.family_id.as_ref() != Some(board_id)
    })
}

fn suppress_family_fallbacks(registry: &Registry, selected: &mut BTreeSet<String>) {
    let family_ids: Vec<String> = registry
        .skills
        .iter()
        .filter(|skill| skill.product && selected.contains(skill.id.as_str()))
        .filter_map(|skill| skill.family_id.clone())
        .collect();
    for family_id in family_ids {
        selected.remove(&family_id);
    }
}

fn suppress_exact_product_prefix_fallbacks(
    registry: &Registry,
    prompt: &str,
    selected: &mut BTreeSet<String>,
) {
    let selected_boards = registry
        .skills
        .iter()
        .filter(|skill| skill.kind == SkillKind::Board && selected.contains(skill.id.as_str()))
        .map(|skill| skill.id.clone())
        .collect::<Vec<_>>();
    for board_id in &selected_boards {
        let exact_more_specific = selected_boards
            .iter()
            .any(|other| other != board_id && exact_suffix_is_present(board_id, other, prompt));
        if exact_more_specific {
            selected.remove(board_id);
        }
    }
}

fn exact_suffix_is_present(generic: &str, specific: &str, prompt: &str) -> bool {
    let Some(suffix) = specific.strip_prefix(&format!("{generic}-")) else {
        return false;
    };
    suffix
        .split('-')
        .filter(|part| !part.is_empty())
        .any(|part| contains_word(prompt, part))
        || prompt.contains(&suffix.replace('-', ""))
}

mod tools;
pub(crate) use tools::*;

fn first_match(skill: &Skill, prompt: &str) -> Option<String> {
    skill
        .triggers
        .iter()
        .chain(skill.aliases.iter())
        .find(|trigger| contains_word(prompt, &normalize(trigger)))
        .cloned()
}

fn add_skill(
    registry: &Registry,
    skill_id: &str,
    kind: &str,
    value: &str,
    selected: &mut BTreeSet<String>,
    matches: &mut Vec<MatchReason>,
) {
    if registry.skills.iter().any(|skill| skill.id == skill_id)
        && selected.insert(skill_id.to_string())
    {
        matches.push(MatchReason {
            skill: skill_id.to_string(),
            matched: MatchedTerm {
                kind: kind.to_string(),
                value: value.to_string(),
            },
        });
    }
}

fn ordered_skills(registry: &Registry, selected: &BTreeSet<String>) -> Vec<String> {
    let mut skills: Vec<_> = registry
        .skills
        .iter()
        .filter(|skill| selected.contains(skill.id.as_str()))
        .collect();
    skills.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
    });
    skills.into_iter().map(|skill| skill.id.clone()).collect()
}

fn route_notes(prompt: &str) -> Vec<String> {
    let mut notes = vec!["Verification level is context injection only.".to_string()];
    if needs_hardware_boundary(prompt) {
        notes.push(
            "hardware-verification boundary: no hardware, flash, serial, OTA, LVGL rendering, RF link, or GNSS fix success is claimed."
                .to_string(),
        );
    }
    notes
}

fn needs_hardware_boundary(prompt: &str) -> bool {
    contains_any(
        prompt,
        &[
            "ota",
            "lvgl",
            "flash",
            "upload",
            "serial",
            "real",
            "hardware",
            "render",
            "simulator",
            "validate",
            "imu",
            "gesture",
            "raise-to-wake",
            "tilt-to-wake",
            "lora",
            "gnss",
            "gps",
            "rf",
            "radio",
            "telemetry",
            "抬腕",
        ],
    )
}

fn is_embedded_prompt(prompt: &str) -> bool {
    let terms = keyword_rules().embedded_prompt;
    let needles = terms.iter().map(String::as_str).collect::<Vec<_>>();
    contains_any(prompt, &needles)
}

fn needs_board_clarification(prompt: &str) -> bool {
    is_embedded_prompt(prompt)
        && contains_any(
            prompt,
            &[
                "imu",
                "bhi260ap",
                "gesture",
                "raise-to-wake",
                "tilt-to-wake",
                "wrist raise",
                "抬腕",
            ],
        )
}

// Only build-intent prompts require a framework before routing. Domain
// keywords alone (lvgl, watch ui) are lightweight fact/debug queries; with an
// active board profile they must keep normal routing instead of dropping all
// context behind a framework clarification.
fn needs_framework_clarification(prompt: &str) -> bool {
    contains_any(prompt, &["demo", "example", "build", "upload", "install"])
}

fn has_explicit_framework(prompt: &str) -> bool {
    let normalized = normalize(prompt);
    contains_any(
        &normalized,
        &[
            "arduino",
            "esp-idf",
            "idf.py",
            "platformio",
            "pio",
            "rust",
            "esp-rs",
        ],
    )
}

fn board_examples(registry: &Registry) -> Vec<String> {
    let preferred = [
        "board-t-watch-ultra",
        "board-t-watch-s3",
        "board-t-display-s3",
    ];
    preferred
        .iter()
        .filter(|id| registry.skills.iter().any(|skill| skill.id == **id))
        .map(|id| id.to_string())
        .collect()
}

fn is_raise_to_wake_intent(prompt: &str) -> bool {
    contains_any(
        prompt,
        &[
            "raise-to-wake",
            "tilt-to-wake",
            "wrist raise",
            "gesture",
            "抬腕",
        ],
    )
}

fn normalize(value: &str) -> String {
    value.to_lowercase().replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::load_registry;
    use std::path::Path;

    fn registry() -> Registry {
        load_registry(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
        )
        .unwrap()
    }

    fn repo_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    // Route triggers and recipe catalogs live in data/** so product vocabulary
    // can evolve without scattering domain terms through Rust code.
    #[test]
    fn data_backed_route_recipe_catalogs() {
        let root = repo_root();

        // Route derived-context triggers are a data file the router loads.
        let derived_path = root.join("data/router/derived-context.json");
        assert!(derived_path.is_file(), "route trigger catalog must be data");
        let derived: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&derived_path).unwrap()).unwrap();
        let derived_entries = derived["entries"].as_array().expect("entries array");
        assert!(
            derived_entries.len() >= 6,
            "expected data-backed route triggers"
        );
        // The router actually consumes the data file (not just a stray asset).
        let specs = derived_context_specs();
        assert_eq!(specs.len(), derived_entries.len());
        assert!(specs.iter().any(|spec| spec.skill_id == "periph-display"));

        // Recipe catalog + source packs live in a data file the runtime loads.
        let recipes_path = root.join("data/recipes/recipes.json");
        assert!(recipes_path.is_file(), "recipe catalog must be data");
        let recipes_file: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&recipes_path).unwrap()).unwrap();
        assert!(recipes_file["recipes"].as_array().unwrap().len() >= 7);
        let registry = crate::recipes::recipe_registry();
        assert!(
            registry
                .recipes
                .iter()
                .any(|recipe| recipe.id == "recipe-lora-gnss-source")
        );

        // Fact prompt keywords and topic fields are data-backed too.
        assert!(root.join("data/facts/prompt-keywords.json").is_file());
        assert!(root.join("data/facts/topic-fields.json").is_file());
    }

    #[test]
    fn keyword_rules_are_data_backed_and_reproduce_english_behavior() {
        // The router keyword vocabulary must live in data (not hardcoded in Rust)
        // and the data-driven `is_embedded_prompt` must reproduce every prior
        // hardcoded English term (zero regression on the existing hit set).
        let path = repo_root().join("data/router/keyword-rules.json");
        assert!(path.is_file(), "keyword rules must be a data file");
        let rules = keyword_rules();
        assert!(!rules.embedded_prompt.is_empty());
        assert!(!rules.critical_text.keywords.is_empty());

        // Exact prior hardcoded English embedded-prompt needles.
        const OLD_ENGLISH_EMBEDDED: &[&str] = &[
            "arduino",
            "esp-idf",
            "idf.py",
            "rust",
            "esp-rs",
            "platformio",
            "pio",
            "lvgl",
            "display",
            "screen",
            "touch",
            "serial",
            "flash",
            "upload",
            "ota",
            "firmware",
            "lora",
            "gps",
            "gnss",
            "nfc",
            "battery",
            "pmu",
            "power",
            "sensor",
            "imu",
            "i2c",
            "spi",
            "gpio",
            "pinout",
            "button",
            "sd",
            "tf",
            "audio",
            "speaker",
            "microphone",
            "haptic",
            "gesture",
            "raise-to-wake",
            "tilt-to-wake",
            "wrist raise",
            "抬腕",
            "debug",
            "build",
            "setup",
            "install",
        ];
        for term in OLD_ENGLISH_EMBEDDED {
            assert!(
                is_embedded_prompt(term),
                "data-driven is_embedded_prompt regressed on old term {term}"
            );
        }
        // A plainly non-embedded prompt still does not match (no over-broadening).
        assert!(!is_embedded_prompt("how do i prune tomatoes"));
    }

    #[test]
    fn cjk_hardware_prompt_recalls_inferred_board() {
        // With an active board profile, a Chinese hardware prompt that names no
        // board must still inject the board capsule (inferred-from-project),
        // isomorphic to the English context-fallback test. This is what the new
        // CJK embedded terms (屏/点亮/显示/触摸/串口/固件/烧录/引脚) unlock.
        let registry = registry();
        let profile = ActiveProfile {
            board: "board-t-display-s3".to_string(),
            framework: None,
            features: Vec::new(),
        };
        for prompt in ["这个屏怎么点亮", "固件烧录后串口没有输出"] {
            let route = route_prompt_with_profile(&registry, prompt, Some(&profile));
            assert_eq!(route.decision, "inject", "prompt: {prompt}");
            assert!(
                route.skills.contains(&"board-t-display-s3".to_string()),
                "prompt {prompt} must inject the active board: {:?}",
                route.skills
            );
            assert_eq!(
                route.board_source.as_deref(),
                Some("inferred-from-project"),
                "prompt {prompt} must be marked inferred"
            );
        }
        // A 显示/触摸 prompt also carries the board's display facts, matching the
        // existing English fallback test's peripheral assertion.
        let display = route_prompt_with_profile(&registry, "显示和触摸怎么调", Some(&profile));
        assert_eq!(display.decision, "inject");
        assert!(display.skills.contains(&"periph-display".to_string()));

        // Guardrail: the same CJK hardware prompt with NO active board stays at
        // zero injection -- the CJK terms never invent a board out of nothing.
        let no_board = route_prompt(&registry, "这个屏怎么点亮");
        assert_eq!(no_board.decision, "no-op");
        assert!(no_board.skills.is_empty());
        assert!(no_board.board_source.is_none());
    }

    #[test]
    fn routing_fixtures() {
        let registry = registry();
        let display = route_prompt(&registry, "T-Display-S3 ESP-IDF LVGL screen is blank");
        assert_eq!(display.decision, "inject");
        assert!(display.skills.contains(&"board-t-display-s3".to_string()));
        assert!(display.skills.contains(&"fw-esp-idf".to_string()));
        assert!(display.skills.contains(&"fw-lvgl".to_string()));
        assert!(display.skills.contains(&"periph-display".to_string()));

        let beam = route_prompt(&registry, "LilyGO T-Beam Arduino LoRa GPS example");
        assert!(beam.skills.contains(&"board-t-beam".to_string()));
        assert!(beam.skills.contains(&"fw-arduino".to_string()));
        assert!(beam.skills.contains(&"periph-lora".to_string()));
        assert!(beam.skills.contains(&"periph-gps".to_string()));

        let none = route_prompt(&registry, "Python script for a desktop CSV report");
        assert_eq!(none.decision, "no-op");
        assert!(none.skills.is_empty());

        let unsupported = route_prompt(&registry, "LilyGO RP2040 display example");
        assert_eq!(unsupported.decision, "no-op");
        assert_eq!(unsupported.verification_level, "unsupported");
    }

    #[test]
    fn unsupported_family_boundary() {
        let registry = registry();
        let unsupported = route_prompt(&registry, "LilyGO RP2040 display implementation demo");
        assert_eq!(unsupported.decision, "no-op");
        assert_eq!(unsupported.verification_level, "unsupported");
        assert!(unsupported.hardware_verification_boundary);
        assert!(
            unsupported
                .notes
                .iter()
                .any(|note| note.contains("only supports ESP32-family"))
        );
        assert!(unsupported.skills.is_empty());
    }

    #[test]
    fn framework_route_fixtures() {
        let registry = registry();
        let rust = route_prompt(&registry, "LilyGO T-Deck Rust esp-rs display input");
        assert!(rust.skills.contains(&"fw-rust".to_string()));
        assert!(rust.skills.contains(&"board-t-deck".to_string()));

        let idf = route_prompt(&registry, "T-Watch ESP-IDF idf.py install flow");
        assert!(idf.skills.contains(&"fw-esp-idf".to_string()));
        assert!(!idf.skills.contains(&"fw-arduino".to_string()));
    }

    #[test]
    fn peripheral_route_fixtures() {
        let registry = registry();
        let ota = route_prompt(
            &registry,
            "T-Watch Ultra OTA update fails after manifest download",
        );
        assert!(ota.skills.contains(&"app-ota".to_string()));
        assert!(ota.skills.contains(&"debug-flash-serial".to_string()));

        let lvgl = route_prompt(&registry, "LilyGO T-Display-S3 LVGL display driver debug");
        assert!(lvgl.skills.contains(&"fw-lvgl".to_string()));
        assert!(lvgl.skills.contains(&"periph-display".to_string()));
        assert!(lvgl.skills.contains(&"debug-lvgl-loop".to_string()));

        let lora = route_prompt(&registry, "T-Beam SX1262 send packet");
        assert!(lora.skills.contains(&"board-t-beam".to_string()));
        assert!(lora.skills.contains(&"periph-lora".to_string()));
        assert!(!lora.skills.contains(&"chip-sx1262-or-sx1280".to_string()));
    }

    #[test]
    fn playbook_route_matrix() {
        let registry = registry();
        let lvgl = route_prompt(&registry, "T-Watch Ultra LVGL blank screen touch debug");
        assert!(
            lvgl.skills
                .contains(&"playbook-source-discovery".to_string())
        );
        assert!(lvgl.skills.contains(&"playbook-lvgl-debug".to_string()));
        assert!(!lvgl.skills.contains(&"playbook-ota-debug".to_string()));

        let ota = route_prompt(
            &registry,
            "T-Watch Ultra ESP-IDF OTA rollback manifest debug",
        );
        assert!(ota.skills.contains(&"playbook-ota-debug".to_string()));
        assert!(
            ota.skills
                .contains(&"playbook-build-flash-serial".to_string())
        );

        let bsp = route_prompt(
            &registry,
            "T-Watch Ultra add display driver BSP status action smoke",
        );
        assert!(bsp.skills.contains(&"playbook-bsp-driver".to_string()));
    }

    #[test]
    fn utf8_cjk_adjacent_route_prompts_do_not_panic() {
        let registry = registry();
        let display = route_prompt(&registry, "T-Display-S3烧录失败");
        assert_eq!(display.decision, "inject");
        assert!(display.skills.contains(&"board-t-display-s3".to_string()));
        assert!(
            display
                .skills
                .contains(&"playbook-build-flash-serial".to_string())
        );

        let watch = route_prompt(&registry, "t-watch ultra imu抬腕检测怎么做");
        assert_eq!(watch.decision, "inject");
        assert!(watch.skills.contains(&"board-t-watch-ultra".to_string()));
        assert!(watch.skills.contains(&"periph-imu".to_string()));
        assert!(watch.skills.contains(&"feature-raise-to-wake".to_string()));

        let watch_s3_pins = route_prompt(&registry, "LilyGO T-Watch S3 屏幕和触摸占用了哪些引脚?");
        assert_eq!(watch_s3_pins.decision, "inject");
        assert!(
            watch_s3_pins
                .skills
                .contains(&"board-t-watch-s3".to_string())
        );
        assert!(watch_s3_pins.skills.contains(&"periph-display".to_string()));
        assert!(watch_s3_pins.skills.contains(&"periph-input".to_string()));
    }

    #[test]
    fn playbook_no_over_injection() {
        let registry = registry();
        let weather = route_prompt(&registry, "what is the weather today");
        assert_eq!(weather.decision, "no-op");
        assert!(
            weather
                .skills
                .iter()
                .all(|skill| !skill.starts_with("playbook-"))
        );

        let facts = route_prompt(
            &registry,
            "T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?",
        );
        assert!(facts.skills.contains(&"board-t-watch-ultra".to_string()));
        assert!(
            facts
                .skills
                .iter()
                .all(|skill| !skill.starts_with("playbook-"))
        );
    }

    #[test]
    fn xl9555_fact_route() {
        let registry = registry();
        let route = route_prompt(
            &registry,
            "T-Watch Ultra XL9555 GPIO expander 哪些口连接按键和外设?",
        );
        assert_eq!(route.decision, "inject");
        assert!(route.skills.contains(&"board-t-watch-ultra".to_string()));
        assert!(route.skills.contains(&"chip-xl9555".to_string()));
        assert!(!route.skills.contains(&"feature-raise-to-wake".to_string()));
    }

    #[test]
    fn verification_level_fixtures() {
        let registry = registry();
        let ota = route_prompt(&registry, "T-Watch OTA update");
        assert_eq!(ota.verification_level, "context-injection");
        assert!(!ota.hardware_verified);
        assert!(ota.hardware_verification_boundary);
        assert!(
            ota.notes
                .iter()
                .any(|note| note.contains("hardware-verification boundary"))
        );

        let lvgl = route_prompt(
            &registry,
            "T-Display-S3 Arduino LVGL touch input does not move",
        );
        assert_eq!(lvgl.verification_level, "context-injection");
        assert!(!lvgl.hardware_verified);
        assert!(lvgl.hardware_verification_boundary);
        assert!(lvgl.skills.contains(&"debug-lvgl-loop".to_string()));

        let real = route_prompt(&registry, "LilyGO T-Display-S3 real LVGL board validation");
        assert!(real.hardware_verification_boundary);
        assert!(!real.hardware_verified);

        let generic = route_prompt(&registry, "LilyGO ESP32-S3 LVGL touch input does not move");
        assert!(generic.skills.contains(&"series-esp32-s3".to_string()));
        assert!(generic.skills.contains(&"fw-lvgl".to_string()));
        assert!(generic.skills.contains(&"periph-display".to_string()));
        assert!(generic.skills.contains(&"debug-lvgl-loop".to_string()));
    }

    #[test]
    fn reference_source_integration() {
        let registry = registry();
        let route = route_prompt(
            &registry,
            "LilyGO T-Watch watch UI LVGL page loop OTA debug",
        );
        assert!(route.skills.contains(&"app-watch-ui-lvgl".to_string()));
        assert!(route.skills.contains(&"app-ota".to_string()));
        assert!(route.skills.contains(&"debug-lvgl-loop".to_string()));
    }

    #[test]
    fn auxiliary_tool_route_fixtures() {
        let registry = registry();
        let serial = route_prompt(&registry, "T-Display-S3 serial port boot log is unreadable");
        assert!(serial.skills.contains(&"debug-flash-serial".to_string()));
        assert!(serial.skills.contains(&"tool-serial-debug".to_string()));

        let install = route_prompt(&registry, "install Arduino CLI for LilyGO T-Beam upload");
        assert!(install.skills.contains(&"fw-arduino".to_string()));
        assert!(install.skills.contains(&"tool-arduino-cli".to_string()));

        let pinout = route_prompt(&registry, "T-Display-S3 pinout");
        assert!(pinout.skills.contains(&"board-t-display-s3".to_string()));
        assert!(!pinout.skills.contains(&"tool-serial-debug".to_string()));
    }

    #[test]
    fn platformio_does_not_substring_inject_storage() {
        // Regression: trigger "tf" (TF card) must not match inside "pla[tf]ormio".
        let registry = registry();
        let route = route_prompt(&registry, "LilyGO ESP32-S3 PlatformIO upload and monitor");
        assert!(route.skills.contains(&"fw-platformio".to_string()));
        assert!(route.skills.contains(&"tool-platformio-cli".to_string()));
        assert!(
            !route.skills.contains(&"periph-storage".to_string()),
            "storage must not be injected from the 'tf' substring of platformio"
        );
        // A real TF/SD storage signal still routes periph-storage.
        let storage = route_prompt(&registry, "LilyGO T-Display-S3 TF card SPIFFS example");
        assert!(storage.skills.contains(&"periph-storage".to_string()));
    }

    #[test]
    fn platformio_does_not_default_to_arduino_framework() {
        let registry = registry();
        // Bare PlatformIO prompt: no framework named -> do not assume Arduino.
        let bare = route_prompt(&registry, "LilyGO ESP32-S3 PlatformIO upload and monitor");
        assert!(!bare.skills.contains(&"fw-arduino".to_string()));
        // Explicit ESP-IDF under PlatformIO: keep ESP-IDF, never add conflicting Arduino.
        let idf = route_prompt(&registry, "LilyGO T-Display-S3 PlatformIO ESP-IDF build");
        assert!(idf.skills.contains(&"fw-esp-idf".to_string()));
        assert!(!idf.skills.contains(&"fw-arduino".to_string()));
        // Explicit Arduino under PlatformIO: framework comes from the keyword pass.
        let arduino = route_prompt(&registry, "LilyGO T-Display-S3 PlatformIO Arduino build");
        assert!(arduino.skills.contains(&"fw-arduino".to_string()));
    }

    #[test]
    fn ordinary_fact_does_not_substring_inject_tools() {
        // Regression: intent needle "port" must not match inside "ex[port]".
        let registry = registry();
        let route = route_prompt(
            &registry,
            "T-Display-S3 frustrated with rust toolchain export",
        );
        assert!(
            !route.skills.contains(&"tool-serial-debug".to_string()),
            "serial tool must not be injected from the 'port' substring of export"
        );
        // A genuine serial intent still gates the serial tool in.
        let serial = route_prompt(&registry, "T-Display-S3 serial port boot log is unreadable");
        assert!(serial.skills.contains(&"tool-serial-debug".to_string()));
    }

    #[test]
    fn progressive_disclosure_negative_route_matrix() {
        let registry = registry();
        let gpio = route_prompt(&registry, "T-Display-S3 GPIO pinout");
        assert!(gpio.skills.contains(&"board-t-display-s3".to_string()));
        assert!(!gpio.skills.contains(&"fw-platformio".to_string()));
        assert!(!gpio.skills.contains(&"fw-arduino".to_string()));
        assert!(!gpio.skills.contains(&"tool-platformio-cli".to_string()));
        assert!(!gpio.skills.contains(&"tool-serial-debug".to_string()));

        let pio_idf = route_prompt(
            &registry,
            "LilyGO ESP32-S3 PlatformIO ESP-IDF upload and monitor",
        );
        assert!(pio_idf.skills.contains(&"fw-platformio".to_string()));
        assert!(pio_idf.skills.contains(&"tool-platformio-cli".to_string()));
        assert!(pio_idf.skills.contains(&"fw-esp-idf".to_string()));
        assert!(!pio_idf.skills.contains(&"fw-arduino".to_string()));
        assert!(!pio_idf.skills.contains(&"tool-serial-debug".to_string()));
    }

    #[test]
    fn platformio_language_preference_matrix() {
        let registry = registry();
        let arduino = route_prompt(&registry, "LilyGO T-Display-S3 PlatformIO Arduino upload");
        assert!(arduino.skills.contains(&"fw-platformio".to_string()));
        assert!(arduino.skills.contains(&"tool-platformio-cli".to_string()));
        assert!(arduino.skills.contains(&"fw-arduino".to_string()));

        let pio = route_prompt(&registry, "LilyGO ESP32-S3 pio run upload");
        assert!(pio.skills.contains(&"fw-platformio".to_string()));
        assert!(pio.skills.contains(&"tool-platformio-cli".to_string()));
        assert!(!pio.skills.contains(&"fw-arduino".to_string()));
    }

    #[test]
    fn generated_route_integration() {
        let registry = registry();
        let route = route_prompt(&registry, "LilyGO T-Dongle-S3 Arduino USB display");
        assert!(route.skills.contains(&"board-t-dongle-s3".to_string()));
        assert!(route.skills.contains(&"fw-arduino".to_string()));
    }

    #[test]
    fn product_route_integration() {
        let registry = registry();
        let route = route_prompt(&registry, "T-Watch Ultra ESP-IDF LVGL serial demo");
        assert!(route.skills.contains(&"board-t-watch-ultra".to_string()));
        assert!(!route.skills.contains(&"board-t-watch".to_string()));
        assert!(route.skills.contains(&"fw-esp-idf".to_string()));
        assert!(route.skills.contains(&"fw-lvgl".to_string()));
        assert!(route.skills.contains(&"periph-display".to_string()));
        assert!(route.hardware_verification_boundary);
    }

    #[test]
    fn exact_board_precedence() {
        let registry = registry();
        let exact = route_prompt(&registry, "T-Display-S3 Arduino LVGL display demo");
        assert!(exact.skills.contains(&"board-t-display-s3".to_string()));
        assert!(
            !exact.skills.contains(&"board-t-display".to_string()),
            "{:?}",
            exact.skills
        );
        assert!(
            !exact
                .matches
                .iter()
                .any(|matched| matched.skill == "board-t-display"),
            "{:?}",
            exact.matches
        );

        let generic = route_prompt(&registry, "LilyGO tdisplay benchmark validation");
        assert!(generic.skills.contains(&"board-t-display".to_string()));
    }

    #[test]
    fn active_board_profile_routing() {
        let registry = registry();
        let profile = ActiveProfile {
            board: "board-t-watch-ultra".to_string(),
            framework: None,
            features: Vec::new(),
        };
        let short = route_prompt_with_profile(&registry, "LVGL screen is blank", Some(&profile));
        assert!(short.skills.contains(&"board-t-watch-ultra".to_string()));
        assert!(short.skills.contains(&"fw-lvgl".to_string()));
        assert!(short.skills.contains(&"periph-display".to_string()));

        let unrelated =
            route_prompt_with_profile(&registry, "how do I prune tomatoes", Some(&profile));
        assert_eq!(unrelated.decision, "no-op");
        assert!(unrelated.skills.is_empty());

        let other_board = route_prompt_with_profile(
            &registry,
            "T-Display-S3 LVGL screen is blank",
            Some(&profile),
        );
        assert!(
            other_board
                .skills
                .contains(&"board-t-display-s3".to_string())
        );
        assert!(
            !other_board
                .skills
                .contains(&"board-t-watch-ultra".to_string())
        );
    }

    #[test]
    fn context_fallback_inferred_board_injection() {
        // Positive: an active board profile + a board-relevant prompt that never
        // names the board must still inject that board capsule, marked inferred.
        let registry = registry();
        let profile = ActiveProfile {
            board: "board-t-display-s3".to_string(),
            framework: None,
            features: Vec::new(),
        };
        for prompt in [
            "how do I light up the screen",
            "the display shows nothing after flash",
        ] {
            let route = route_prompt_with_profile(&registry, prompt, Some(&profile));
            assert_eq!(route.decision, "inject", "prompt: {prompt}");
            assert!(
                route.skills.contains(&"board-t-display-s3".to_string()),
                "prompt {prompt} must inject the active board: {:?}",
                route.skills
            );
            assert!(
                route.skills.contains(&"periph-display".to_string()),
                "prompt {prompt} should carry the board's display facts"
            );
            assert_eq!(
                route.board_source.as_deref(),
                Some("inferred-from-project"),
                "prompt {prompt} must mark the board as inferred"
            );
        }
    }

    #[test]
    fn context_fallback_unrelated_prompt_with_active_board_stays_empty() {
        // Negative A: an active board is set but the prompt is totally unrelated
        // (a cooking question). Chosen + locked behavior: inject nothing, so no
        // wrong board facts leak into an off-topic prompt.
        let registry = registry();
        let profile = ActiveProfile {
            board: "board-t-display-s3".to_string(),
            framework: None,
            features: Vec::new(),
        };
        for prompt in ["红烧肉怎么做", "how do I braise pork belly"] {
            let route = route_prompt_with_profile(&registry, prompt, Some(&profile));
            assert_eq!(route.decision, "no-op", "prompt: {prompt}");
            assert!(
                route.skills.is_empty(),
                "prompt {prompt}: {:?}",
                route.skills
            );
            assert!(route.board_source.is_none());
        }
    }

    #[test]
    fn context_fallback_no_active_board_stays_empty() {
        // Negative B: no active board + a prompt that names no board must inject
        // zero bytes -- the fallback never invents a board out of nothing.
        let registry = registry();
        let route = route_prompt(&registry, "how do I light up the screen");
        assert_eq!(route.decision, "no-op");
        assert!(route.skills.is_empty());
        assert!(route.board_source.is_none());
    }

    #[test]
    fn context_fallback_named_board_is_not_marked_inferred() {
        // A prompt that names its own board is the keyword path, not a fallback:
        // board_source stays None even when a matching profile is present.
        let registry = registry();
        let profile = ActiveProfile {
            board: "board-t-display-s3".to_string(),
            framework: None,
            features: Vec::new(),
        };
        let route = route_prompt_with_profile(
            &registry,
            "T-Display-S3 LVGL screen is blank",
            Some(&profile),
        );
        assert_eq!(route.decision, "inject");
        assert!(route.skills.contains(&"board-t-display-s3".to_string()));
        assert!(
            route.board_source.is_none(),
            "an explicitly named board must not be flagged as inferred"
        );
    }

    #[test]
    fn needs_clarification_board() {
        let registry = registry();
        let route = route_prompt(&registry, "Arduino IMU 抬腕检测怎么做");
        assert_eq!(route.decision, "needs_clarification");
        assert_eq!(route.missing, vec!["board".to_string()]);
        assert!(route.skills.is_empty());
        assert!(route.questions.iter().any(|question| {
            question.id == "board"
                && question
                    .examples
                    .contains(&"board-t-watch-ultra".to_string())
        }));
        assert_eq!(route.verification_level, "none");
        assert!(!route.hardware_verified);
    }

    #[test]
    fn needs_clarification_framework() {
        let profile = ActiveProfile {
            board: "board-t-watch-ultra".to_string(),
            framework: None,
            features: Vec::new(),
        };
        assert!(project_context_needs_framework(
            "LVGL watch UI demo 怎么写",
            &profile
        ));
        let route = framework_clarification_result();
        assert_eq!(route.decision, "needs_clarification");
        assert_eq!(route.missing, vec!["framework".to_string()]);
        assert!(
            route.questions[0]
                .examples
                .contains(&"fw-arduino".to_string())
        );
        assert!(
            route.questions[0]
                .examples
                .contains(&"fw-esp-idf".to_string())
        );

        let explicit = ActiveProfile {
            board: "board-t-watch-ultra".to_string(),
            framework: None,
            features: Vec::new(),
        };
        assert!(!project_context_needs_framework(
            "Arduino LVGL watch UI demo",
            &explicit
        ));
    }

    #[test]
    fn project_context_route() {
        let registry = registry();
        let profile = ActiveProfile {
            board: "board-t-watch-ultra".to_string(),
            framework: Some("fw-arduino".to_string()),
            features: vec!["feature-raise-to-wake".to_string()],
        };
        let short = route_prompt_with_profile(&registry, "抬腕检测怎么做", Some(&profile));
        for skill in [
            "board-t-watch-ultra",
            "periph-imu",
            "chip-bhi260ap",
            "fw-arduino",
            "feature-raise-to-wake",
        ] {
            assert!(short.skills.contains(&skill.to_string()), "missing {skill}");
        }
    }

    #[test]
    fn project_context_precedence() {
        let registry = registry();
        let profile = ActiveProfile {
            board: "board-t-watch-ultra".to_string(),
            framework: Some("fw-arduino".to_string()),
            features: vec!["feature-raise-to-wake".to_string()],
        };
        let explicit = route_prompt_with_profile(
            &registry,
            "T-Display-S3 Arduino LVGL screen is blank",
            Some(&profile),
        );
        assert!(explicit.skills.contains(&"board-t-display-s3".to_string()));
        assert!(!explicit.skills.contains(&"board-t-watch-ultra".to_string()));
    }

    #[test]
    fn project_context_noop() {
        let registry = registry();
        let profile = ActiveProfile {
            board: "board-t-watch-ultra".to_string(),
            framework: Some("fw-arduino".to_string()),
            features: Vec::new(),
        };
        let route = route_prompt_with_profile(&registry, "how do I prune tomatoes", Some(&profile));
        assert_eq!(route.decision, "no-op");
        assert!(route.skills.is_empty());
    }

    #[test]
    fn peripheral_subroute_matrix() {
        let registry = registry();
        let imu = route_prompt(&registry, "T-Watch Ultra Arduino IMU driver source");
        assert!(imu.skills.contains(&"board-t-watch-ultra".to_string()));
        assert!(imu.skills.contains(&"fw-arduino".to_string()));
        assert!(imu.skills.contains(&"periph-imu".to_string()));
        assert!(imu.skills.contains(&"chip-bhi260ap".to_string()));

        let display = route_prompt(&registry, "T-Watch Ultra display brightness Arduino");
        assert!(display.skills.contains(&"periph-display".to_string()));
        assert!(!display.skills.contains(&"periph-imu".to_string()));
        assert!(
            !display
                .skills
                .contains(&"feature-raise-to-wake".to_string())
        );
    }

    #[test]
    fn raise_to_wake_route_fixture() {
        let registry = registry();
        let route = route_prompt(&registry, "T-Watch Ultra Arduino IMU 抬腕检测怎么做");
        for skill in [
            "lilygo-router",
            "board-t-watch-ultra",
            "periph-imu",
            "chip-bhi260ap",
            "fw-arduino",
            "feature-raise-to-wake",
        ] {
            assert!(route.skills.contains(&skill.to_string()), "missing {skill}");
        }
        assert_eq!(route.verification_level, "context-injection");
        assert!(!route.hardware_verified);
        assert!(route.hardware_verification_boundary);
    }

    #[test]
    fn over_injection_regression() {
        let registry = registry();
        let unrelated = route_prompt(&registry, "how do I prune tomatoes");
        assert_eq!(unrelated.decision, "no-op");

        let gpio = route_prompt(&registry, "T-Display-S3 GPIO pinout");
        assert!(!gpio.skills.contains(&"fw-platformio".to_string()));
        assert!(!gpio.skills.contains(&"tool-platformio-cli".to_string()));

        let watch_display = route_prompt(&registry, "T-Watch Ultra AMOLED brightness");
        assert!(
            !watch_display
                .skills
                .contains(&"feature-raise-to-wake".to_string())
        );
        assert!(!watch_display.skills.contains(&"chip-bhi260ap".to_string()));

        let profile = ActiveProfile {
            board: "board-t-watch-ultra".to_string(),
            framework: Some("fw-arduino".to_string()),
            features: Vec::new(),
        };
        let no_op = route_prompt_with_profile(&registry, "how do I prune tomatoes", Some(&profile));
        assert_eq!(no_op.decision, "no-op");
    }

    #[test]
    fn peripheral_evidence_boundary() {
        let registry = registry();
        let route = route_prompt(&registry, "T-Watch Ultra BHI260AP raise-to-wake real test");
        assert_eq!(route.verification_level, "context-injection");
        assert!(!route.hardware_verified);
        assert!(route.hardware_verification_boundary);
        assert!(route.notes.iter().any(|note| note.contains("no hardware")));
    }

    #[test]
    fn lora_gnss_route_evidence_boundary() {
        let registry = registry();
        let route = route_prompt(&registry, "T-Beam LoRa GNSS telemetry");
        assert_eq!(route.verification_level, "context-injection");
        assert!(!route.hardware_verified);
        assert!(route.hardware_verification_boundary);
        assert!(route.notes.iter().any(|note| note.contains("RF link")));
    }

    #[test]
    fn sx1262_lora_model_route() {
        let registry = registry();
        let route = route_prompt(&registry, "LilyGO T-Beam SX1262 range test");
        assert_eq!(route.decision, "inject");
        assert!(route.skills.contains(&"board-t-beam".to_string()));
        assert!(route.skills.contains(&"periph-lora".to_string()));
        assert!(!route.skills.contains(&"chip-sx1262-or-sx1280".to_string()));
    }
}
