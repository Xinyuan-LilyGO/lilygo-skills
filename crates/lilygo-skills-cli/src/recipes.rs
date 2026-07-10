//! Source-backed recipe registry for safe goal planning across build, flash,
//! serial, LVGL, OTA, and BSP-oriented workflows.
//!
//! Both the recipe definitions and the trigger rules that decide which recipes
//! fire for a prompt live in JSON (`data/recipes/recipes.json` and
//! `data/recipes/recipe-triggers.json`). This module is a thin reader: it
//! computes the two route-derived flags (`has_board`, `pure_fact_lookup`) and
//! hands the rest to the generic [`crate::selection`] engine.
use crate::model::{GoalRoute, Recipe, RecipeRegistry};
use crate::selection::{SelectionConfig, SelectionInput};
use std::collections::{BTreeMap, BTreeSet};

const RECIPE_REGISTRY_JSON: &str = include_str!("../../../data/recipes/recipes.json");
const RECIPE_TRIGGERS_JSON: &str = include_str!("../../../data/recipes/recipe-triggers.json");

pub fn recipe_registry() -> RecipeRegistry {
    serde_json::from_str(RECIPE_REGISTRY_JSON)
        .expect("embedded data/recipes/recipes.json must be valid RecipeRegistry")
}

fn recipe_triggers() -> SelectionConfig {
    serde_json::from_str(RECIPE_TRIGGERS_JSON)
        .expect("embedded data/recipes/recipe-triggers.json must be valid SelectionConfig")
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

pub fn selected_recipe_ids(prompt: &str, route: &GoalRoute) -> BTreeSet<String> {
    let has_board = route.board.is_some();
    let pure_fact_lookup = crate::facts::is_fact_prompt(prompt)
        && !crate::facts::is_implementation_or_debug_prompt(prompt);

    let mut flags = BTreeMap::new();
    flags.insert("has_board", has_board);
    flags.insert("pure_fact_lookup", pure_fact_lookup);

    let mut lists = BTreeMap::new();
    lists.insert("peripherals", route.peripherals.clone());
    lists.insert("chips", route.chips.clone());
    lists.insert("features", route.features.clone());

    let input = SelectionInput {
        prompt: prompt.to_lowercase(),
        flags,
        lists,
    };
    crate::selection::evaluate(&recipe_triggers(), input)
        .into_iter()
        .collect()
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
}
