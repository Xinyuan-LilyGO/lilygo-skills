//! Source-backed recipe registry for safe goal planning across build, flash,
//! serial, LVGL, OTA, and BSP-oriented workflows.
use crate::model::{GoalRoute, Recipe, RecipeRegistry};
use crate::text_match::contains_any;
use std::collections::BTreeSet;

const RECIPE_REGISTRY_JSON: &str = include_str!("../../../data/recipes/recipes.json");

pub fn recipe_registry() -> RecipeRegistry {
    serde_json::from_str(RECIPE_REGISTRY_JSON)
        .expect("embedded data/recipes/recipes.json must be valid RecipeRegistry")
}

pub fn selected_recipes(prompt: &str, route: &GoalRoute) -> Vec<Recipe> {
    let selected = selected_recipe_ids(prompt, route);
    recipe_registry()
        .recipes
        .into_iter()
        .filter(|recipe| selected.contains(recipe.id.as_str()))
        .collect()
}

pub fn source_packs_for_recipes(recipe_ids: &[String]) -> Vec<crate::model::RecipeSourcePack> {
    let selected: BTreeSet<&str> = recipe_ids.iter().map(String::as_str).collect();
    recipe_registry()
        .source_packs
        .into_iter()
        .filter(|pack| {
            pack.recipe_ids
                .iter()
                .any(|id| selected.contains(id.as_str()))
        })
        .collect()
}

pub fn selected_recipe_ids(prompt: &str, route: &GoalRoute) -> BTreeSet<&'static str> {
    let normalized = prompt.to_lowercase();
    let mut selected = BTreeSet::new();
    let has_board = route.board.is_some();
    let has_demo_target = has_board
        && (contains_any(
            &normalized,
            &["demo", "example", "official", "示例", "例程"],
        ) || !route.peripherals.is_empty()
            || !route.chips.is_empty()
            || !route.features.is_empty());

    if has_demo_target {
        selected.insert("recipe-run-official-demo");
    }
    if has_board
        && (contains_any(
            &normalized,
            &[
                "arduino",
                "esp-idf",
                "platformio",
                "rust",
                "build",
                "upload",
                "flash",
                "烧录",
                "编译",
            ],
        ) || has_demo_target)
    {
        selected.insert("recipe-build-upload-monitor");
    }
    if has_board
        && (contains_any(
            &normalized,
            &[
                "serial",
                "boot log",
                "monitor",
                "unreadable",
                "upload",
                "flash",
                "ota",
                "imu",
                "gesture",
                "抬腕",
            ],
        ) || route
            .features
            .iter()
            .any(|feature| feature == "feature-raise-to-wake"))
    {
        selected.insert("recipe-serial-debug");
    }
    if contains_any(
        &normalized,
        &["lvgl", "touch", "page-data", "screen", "display"],
    ) {
        selected.insert("recipe-lvgl-simulator");
    }
    if contains_any(
        &normalized,
        &["ota", "manifest", "partition", "rollback", "rebooted"],
    ) {
        selected.insert("recipe-ota-debug");
    }
    if contains_any(
        &normalized,
        &[
            "lora",
            "gnss",
            "gps",
            "radio",
            "meshtastic",
            "telemetry",
            "radiolib",
        ],
    ) || route.peripherals.iter().any(|peripheral| {
        peripheral == "periph-lora" || peripheral == "periph-lora-gps" || peripheral == "periph-gps"
    }) {
        selected.insert("recipe-lora-gnss-source");
    }
    let asks_for_bsp_source = contains_any(
        &normalized,
        &["bsp", "driver", "source", "header", "chip", "datasheet"],
    );
    let routed_chip_source_request =
        !route.chips.is_empty() && contains_any(&normalized, &["driver", "source", "datasheet"]);
    if asks_for_bsp_source || routed_chip_source_request {
        selected.insert("recipe-bsp-chip-driver");
    }
    selected
}

pub fn classify_failure(output: &str) -> Option<String> {
    let normalized = output.to_lowercase();
    if contains_any(
        &normalized,
        &[
            "no such file",
            "not found",
            "command not found",
            "not concrete",
            "no executable goal steps",
            "failed to run",
        ],
    ) {
        return Some("missing-tool-or-source".to_string());
    }
    if contains_any(
        &normalized,
        &["failed to connect", "no serial", "permission denied"],
    ) {
        return Some("port-or-permission".to_string());
    }
    if contains_any(
        &normalized,
        &["timed out", "timeout", "no frames", "no data"],
    ) {
        return Some("runtime-timeout-no-observation".to_string());
    }
    if contains_any(&normalized, &["manifest", "rollback", "partition"]) {
        return Some("ota-partition-manifest".to_string());
    }
    if contains_any(
        &normalized,
        &["undefined reference", "compile", "compilation", "error:"],
    ) {
        return Some("build-failure".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GoalRoute;

    fn route(skills: &[&str]) -> GoalRoute {
        GoalRoute {
            skills: skills.iter().map(|skill| skill.to_string()).collect(),
            board: Some("board-t-watch-ultra".to_string()),
            framework: Some("fw-arduino".to_string()),
            frameworks: vec!["fw-arduino".to_string()],
            peripherals: skills
                .iter()
                .filter(|skill| skill.starts_with("periph-"))
                .map(|skill| skill.to_string())
                .collect(),
            chips: skills
                .iter()
                .filter(|skill| skill.starts_with("chip-"))
                .map(|skill| skill.to_string())
                .collect(),
            features: skills
                .iter()
                .filter(|skill| skill.starts_with("feature-"))
                .map(|skill| skill.to_string())
                .collect(),
            applications: Vec::new(),
            tools: Vec::new(),
            playbooks: Vec::new(),
        }
    }

    #[test]
    fn recipe_registry() {
        let registry = super::recipe_registry();
        let ids = registry
            .recipes
            .iter()
            .map(|recipe| recipe.id.as_str())
            .collect::<BTreeSet<_>>();
        for id in [
            "recipe-run-official-demo",
            "recipe-build-upload-monitor",
            "recipe-serial-debug",
            "recipe-lvgl-simulator",
            "recipe-ota-debug",
            "recipe-lora-gnss-source",
            "recipe-bsp-chip-driver",
        ] {
            assert!(ids.contains(id), "missing recipe {id}");
        }
        assert!(
            registry
                .source_packs
                .iter()
                .any(|pack| pack.id == "recipe-pack-lvgl-ui-debug-loop")
        );
    }

    // OTA/LVGL/LoRa recipe packs must cite public upstream references; private
    // practice repositories are never part of the runtime contract.
    #[test]
    fn recipe_source_pack_authority() {
        let registry = super::recipe_registry();
        for pack_id in [
            "recipe-pack-ota-debug",
            "recipe-pack-lvgl-ui-debug-loop",
            "recipe-pack-lora-gnss-source",
        ] {
            let pack = registry
                .source_packs
                .iter()
                .find(|pack| pack.id == pack_id)
                .unwrap_or_else(|| panic!("missing source pack {pack_id}"));
            assert!(
                !pack.official_refs.is_empty(),
                "{pack_id} must cite official upstream refs"
            );
            assert!(
                pack.official_refs
                    .iter()
                    .all(|reference| reference.starts_with("https://")),
                "{pack_id} official refs must be https upstream links"
            );
            assert!(
                !pack.source_refs.is_empty()
                    && pack
                        .source_refs
                        .iter()
                        .all(|reference| reference.starts_with("https://")),
                "{pack_id} source refs must be public https references"
            );
            assert_eq!(
                pack.authority, "official-docs-over-operating-recipe",
                "{pack_id} must rank official docs above operating recipe"
            );
        }
    }

    #[test]
    fn recipe_selection_matrix() {
        let imu = route(&["periph-imu", "chip-bhi260ap", "feature-raise-to-wake"]);
        let selected = selected_recipe_ids("T-Watch Ultra Arduino IMU 抬腕检测怎么做", &imu);
        assert!(selected.contains("recipe-run-official-demo"));
        assert!(selected.contains("recipe-build-upload-monitor"));
        assert!(selected.contains("recipe-serial-debug"));
        assert!(!selected.contains("recipe-ota-debug"));

        let lvgl = route(&["periph-display"]);
        let selected = selected_recipe_ids("T-Watch Ultra Arduino LVGL touch does not move", &lvgl);
        assert!(selected.contains("recipe-lvgl-simulator"));

        let ota = route(&["app-ota"]);
        let selected =
            selected_recipe_ids("T-Watch Ultra OTA manifest downloaded then rebooted", &ota);
        assert!(selected.contains("recipe-ota-debug"));
        assert!(selected.contains("recipe-serial-debug"));

        let lora = route(&["periph-lora", "periph-gps"]);
        let selected = selected_recipe_ids("T-Beam LoRa GNSS telemetry", &lora);
        assert!(selected.contains("recipe-lora-gnss-source"));

        let selected = selected_recipe_ids("T-Watch Ultra rotation display issue", &lvgl);
        assert!(selected.contains("recipe-lvgl-simulator"));
        assert!(!selected.contains("recipe-ota-debug"));
    }

    #[test]
    fn goal_failure_classification() {
        assert_eq!(
            classify_failure("Compilation error: undefined reference").as_deref(),
            Some("build-failure")
        );
        assert_eq!(
            classify_failure("OTA manifest digest mismatch then rollback").as_deref(),
            Some("ota-partition-manifest")
        );
        assert_eq!(
            classify_failure("failed to connect to serial port").as_deref(),
            Some("port-or-permission")
        );
    }
}
