use super::*;
use crate::model::ActiveProfile;
use crate::registry::load_registry;
use crate::router::{route_prompt, route_prompt_with_profile};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn plan(prompt: &str) -> GoalPlan {
    let root = root();
    let registry = load_registry(root.as_path()).expect("registry");
    let route = route_prompt(&registry, prompt);
    plan_goal(root.as_path(), &registry, prompt, &route).expect("goal plan")
}

fn plan_with_profile(prompt: &str, profile: &ActiveProfile) -> GoalPlan {
    let root = root();
    let registry = load_registry(root.as_path()).expect("registry");
    let route = route_prompt_with_profile(&registry, prompt, Some(profile));
    plan_goal(root.as_path(), &registry, prompt, &route).expect("goal plan")
}

fn plan_for_project(prompt: &str, project: &Path) -> GoalPlan {
    let root = root();
    let registry = load_registry(root.as_path()).expect("registry");
    let route = route_prompt(&registry, prompt);
    plan_goal_with_project(root.as_path(), &registry, prompt, &route, Some(project))
        .expect("goal plan with project")
}

#[test]
fn goal_plan_schema() {
    let plan = plan("T-Watch Ultra Arduino IMU 抬腕检测怎么做");
    assert_eq!(plan.schema_version, 1);
    assert_eq!(plan.status, "PASS");
    assert_eq!(plan.decision, "planned");
    assert!(plan.goal_id.starts_with("goal-"));
    let value = serde_json::to_value(&plan).expect("json value");
    assert!(value.get("context_capsule").is_some());
    assert!(value.get("recipes").is_some());
}

#[test]
fn context_composer_capsule() {
    let plan = plan("T-Watch Ultra Arduino IMU 抬腕检测怎么做");
    assert_eq!(plan.route.board.as_deref(), Some("board-t-watch-ultra"));
    assert_eq!(plan.route.framework.as_deref(), Some("fw-arduino"));
    assert!(plan.route.peripherals.contains(&"periph-imu".to_string()));
    assert!(plan.route.chips.contains(&"chip-bhi260ap".to_string()));
    assert!(
        plan.context_capsule
            .facts
            .iter()
            .any(|fact| fact.key == "chip" && fact.value == "Bosch BHI260AP")
    );
    assert!(
        plan.context_capsule
            .facts
            .iter()
            .any(|fact| fact.key == "bus" && fact.value == "I2C 0x28")
    );
    assert!(
        plan.context_capsule
            .facts
            .iter()
            .any(|fact| fact.key == "driver" && fact.value == "SensorBHI260AP")
    );
    assert!(
        plan.context_capsule
            .demo_refs
            .iter()
            .any(|demo| { demo.path == "examples/sensor/BHI260AP_6DoF/BHI260AP_6DoF.ino" })
    );
    assert!(plan.context_capsule.facts.iter().any(|fact| {
        fact.key == "arduino.fqbn"
            && fact.value.contains("esp32:esp32:twatch_ultra")
            && fact.value.contains("CDCOnBoot=default")
    }));
    assert!(plan.context_capsule.facts.iter().any(|fact| {
        fact.key == "arduino.library_roots" && fact.value.contains("../LilyGoLib-ThirdParty")
    }));
    for id in [
        "recipe-run-official-demo",
        "recipe-build-upload-monitor",
        "recipe-serial-debug",
    ] {
        assert!(plan.recipe_ids.contains(&id.to_string()), "missing {id}");
    }
    assert_eq!(plan.context_capsule.boundary.verification_level, "V3");
    assert!(!plan.context_capsule.boundary.hardware_verified);
}

#[test]
fn goal_main_board_precedence() {
    let plan = plan("T-Display-S3 Arduino LVGL display demo");
    assert_eq!(plan.route.board.as_deref(), Some("board-t-display-s3"));
    assert!(
        !plan.route.skills.contains(&"board-t-display".to_string()),
        "{:?}",
        plan.route.skills
    );
}

#[test]
fn goal_discovery_enrichment_hints() {
    let plan = plan("T-Display-S3 Arduino LVGL display demo");
    assert_eq!(
        plan.context_capsule
            .completeness
            .get("display")
            .map(String::as_str),
        Some("complete")
    );
    assert!(
        plan.context_capsule
            .readiness
            .iter()
            .any(|signal| { signal.topic == "display" && signal.update_command.is_none() })
    );
    assert!(plan.context_capsule.discovery_hints.iter().any(|hint| {
        hint.command
            .as_deref()
            .is_some_and(|command| command.contains("source query"))
    }));
    for expected in ["ST7789 170x320 TFT", "8-bit parallel display bus"] {
        assert!(
            plan.context_capsule
                .facts
                .iter()
                .any(|fact| fact.value == expected),
            "missing {expected}"
        );
    }
    assert!(
        plan.context_capsule
            .facts
            .iter()
            .any(|fact| { fact.value.contains("GPIO38") && fact.value.contains("GPIO15") })
    );
}

#[test]
fn source_recovery_capsule_t_display_s3() {
    let plan = plan("T-Display-S3 PlatformIO Arduino TFT_eSPI I2C sensor screen");
    let capsule = &plan.context_capsule;
    let rendered = serde_json::to_string(capsule).expect("capsule json");

    assert_eq!(plan.route.board.as_deref(), Some("board-t-display-s3"));
    assert!(
        capsule
            .demo_refs
            .iter()
            .any(|demo| demo.path == "examples/tft/tft.ino")
    );
    assert!(rendered.contains("implementation_start"));
    assert!(rendered.contains("official-demo-first"));
    assert!(rendered.contains("Setup206_LilyGo_T_Display_S3.h"));
    assert!(rendered.contains("pin_config.h"));
    assert!(rendered.contains("critical_facts"));
    assert!(rendered.contains("PIN_IIC_SDA=GPIO18"));
    assert!(rendered.contains("PIN_IIC_SCL=GPIO17"));
    assert!(rendered.contains("recovery_actions"));
    assert!(rendered.contains("source query --board board-t-display-s3 --topic io --json"));
    assert!(rendered.contains("internal_skill_hints"));
    assert!(rendered.contains("playbook-source-discovery"));
}

#[test]
fn capsule_drops_internal_bookkeeping_noise() {
    // Lock: the injected capsule must never carry the internal-bookkeeping
    // fields with near-zero answer value. Two shapes exercised: an implementation
    // prompt (recipes present, so kept) and a pure fact lookup (recipes empty).
    for prompt in [
        "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor",
        "T-Display-S3 的 I2C 引脚和屏幕占用了哪些 GPIO?",
    ] {
        let summary = render_hook_goal_summary(&plan(prompt));
        // Random goal_id hash: gone.
        assert!(
            !summary.contains("goal_id="),
            "goal_id must not be injected: {summary}"
        );
        // Empty recipe/playbook arrays: not rendered as noise.
        assert!(
            !summary.contains("recipes=[]"),
            "empty recipes must not render: {summary}"
        );
        assert!(
            !summary.contains("playbooks=[]"),
            "empty playbooks must not render: {summary}"
        );
        // Pure counts: dropped.
        assert!(
            !summary.contains("fact_tables="),
            "fact_tables count must not render: {summary}"
        );
        assert!(
            !summary.contains("discovery_hints="),
            "discovery_hints count must not render: {summary}"
        );
        // completeness=[..] duplicated the route prefix's readiness=[..]; dropped.
        assert!(
            !summary.contains("completeness="),
            "completeness duplicate must not render: {summary}"
        );
        // Honesty markers stay: the model must never see a capsule without them.
        assert!(
            summary.contains("evidence_boundary=V3/hardware_verified=false"),
            "honesty markers must remain: {summary}"
        );
    }
}

#[test]
fn capsule_carries_fetch_before_claim_guidance() {
    // Every board goal capsule must carry the fetch-before-claim guidance line,
    // placed right next to the honesty markers. NOTE: this only proves the line
    // is present -- whether it changes live-model behavior (fewer invented pins)
    // needs a real A/B and is not asserted here.
    let prompts = [
        "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor",
        "T-Display-S3 的 I2C 引脚和屏幕占用了哪些 GPIO?",
        "T-Watch Ultra Arduino IMU 抬腕检测怎么做",
    ];
    let mut total = 0usize;
    for prompt in prompts {
        let summary = render_hook_goal_summary(&plan(prompt));
        assert!(
            summary.contains("do not invent pin numbers"),
            "guidance line missing: {summary}"
        );
        assert!(
            summary.contains("lilygo-skills source query"),
            "guidance must point at the source query: {summary}"
        );
        // Guidance sits just before the honesty markers, which must remain.
        assert!(
            summary.contains(
                "do not invent pin numbers; evidence_boundary=V3/hardware_verified=false"
            ),
            "guidance must be adjacent to intact honesty markers: {summary}"
        );
        total += summary.len();
    }
    // The average board capsule stays under the 1024 B budget even with the
    // guidance line added (WP1 de-noise freed the space). The fleet-wide average
    // measured by eval/coverage-gate.js is lower still.
    let avg = total / prompts.len();
    assert!(avg < 1024, "avg capsule too large: {avg}");

    // A no-board prompt injects no capsule at all -- the guidance never leaks
    // into an off-topic or board-less context.
    let none = render_hook_goal_summary(&plan("what is the weather today"));
    assert!(!none.contains("do not invent pin numbers"), "{none}");
}

#[test]
fn source_recovery_hook_summary_t_display_s3() {
    let plan = plan("T-Display-S3 PlatformIO Arduino TFT_eSPI I2C sensor screen");
    let summary = render_hook_goal_summary(&plan);

    // lean-capsule refactor: the implementation-prompt source-recovery segment keeps only
    // the concrete demo entry point and the critical source-backed pins inline.
    // The former headers=[..], recovery=[..], and internal=[..] operating detail
    // was dropped from the push capsule -- it is recovered on the pull side via
    // `source query` / `verify sources` -- so those substrings must be absent.
    assert!(summary.contains("examples/tft/tft.ino"));
    assert!(summary.contains("PIN_IIC_SDA=GPIO18"));
    assert!(summary.contains("PIN_IIC_SCL=GPIO17"));
    assert!(
        !summary.contains("headers=["),
        "headers must be dropped: {summary}"
    );
    assert!(
        !summary.contains("recovery=["),
        "recovery must be dropped: {summary}"
    );
    assert!(
        !summary.contains("internal=["),
        "internal must be dropped: {summary}"
    );
    assert!(
        !summary.contains("Setup206_LilyGo_T_Display_S3.h"),
        "header detail must be dropped: {summary}"
    );
}

#[test]
fn project_board_readiness() {
    let profile = ActiveProfile {
        board: "board-t-display-s3".to_string(),
        framework: Some("fw-arduino".to_string()),
        features: Vec::new(),
    };
    let plan = plan_with_profile("LVGL 显示 demo 怎么做", &profile);
    assert_eq!(plan.route.board.as_deref(), Some("board-t-display-s3"));
    assert_eq!(plan.route.framework.as_deref(), Some("fw-arduino"));
    assert_eq!(
        plan.context_capsule
            .completeness
            .get("display")
            .map(String::as_str),
        Some("complete")
    );
}

#[test]
fn blank_project_getting_started() {
    let profile = ActiveProfile {
        board: "board-t-display-s3".to_string(),
        framework: Some("fw-arduino".to_string()),
        features: Vec::new(),
    };
    let plan = plan_with_profile("LVGL 显示 demo 怎么做", &profile);
    assert!(
        plan.context_capsule
            .demo_refs
            .iter()
            .any(|demo| demo.source_url.contains("T-Display-S3"))
    );
    assert!(
        plan.context_capsule
            .source_refs
            .iter()
            .any(|source| source.url.contains("T-Display-S3"))
    );
    assert!(plan.context_capsule.discovery_hints.iter().any(|hint| {
        hint.command
            .as_deref()
            .is_some_and(|command| command.contains("source query"))
    }));
}

#[test]
fn io_prompt_fact_expansion() {
    let plan = plan("T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?");
    assert_eq!(plan.route.board.as_deref(), Some("board-t-watch-ultra"));
    assert_eq!(plan.route.framework.as_deref(), Some("fw-arduino"));
    let tables = plan
        .context_capsule
        .fact_tables
        .iter()
        .map(|table| table.table.as_str())
        .collect::<BTreeSet<_>>();
    for table in [
        "pin_matrix",
        "bus_matrix",
        "expander_matrix",
        "peripheral_table",
    ] {
        assert!(tables.contains(table), "missing {table}");
    }
    assert!(
        plan.context_capsule
            .fact_tables
            .iter()
            .flat_map(|table| table.rows.iter())
            .any(|fact| fact.key == "expander.xl9555.channel-map"
                && fact.value == "unknown_with_sources")
    );
}

#[test]
fn discovery_hints_for_missing_facts() {
    let plan = plan("T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?");
    assert!(plan.context_capsule.discovery_hints.iter().any(|hint| {
        hint.command.as_deref().is_some_and(|command| {
            command.contains("source query --board board-t-watch-ultra --topic io")
        })
    }));
    assert!(
        plan.context_capsule
            .discovery_hints
            .iter()
            .any(|hint| hint.reason.contains("XL9555"))
    );
}

#[test]
fn no_over_injection_for_fact_lookup() {
    let plan = plan("T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?");
    assert!(plan.context_capsule.preferences.is_empty());
    assert!(plan.context_capsule.reference_hints.is_empty());
    for forbidden in [
        "tool-serial-debug",
        "tool-platformio-cli",
        "feature-raise-to-wake",
    ] {
        assert!(
            !plan.route.skills.contains(&forbidden.to_string()),
            "fact lookup over-injected {forbidden}"
        );
    }
}

#[test]
fn fact_lookup_does_not_emit_implementation_recovery_context() {
    let plan = plan("T-Display-S3 Arduino IO口怎么用? 哪些GPIO接了外设?");
    assert_eq!(plan.route.board.as_deref(), Some("board-t-display-s3"));
    assert!(
        plan.context_capsule
            .fact_tables
            .iter()
            .any(|table| { table.table == "pin_matrix" || table.table == "peripheral_table" })
    );
    assert!(plan.context_capsule.implementation_start.is_none());
    assert!(plan.context_capsule.critical_facts.is_empty());
    assert!(plan.context_capsule.recovery_actions.is_empty());
    assert!(plan.context_capsule.internal_skill_hints.is_empty());
    for forbidden in [
        "recipe-run-official-demo",
        "recipe-build-upload-monitor",
        "recipe-lvgl-simulator",
    ] {
        assert!(
            !plan.recipe_ids.contains(&forbidden.to_string()),
            "fact lookup selected {forbidden}"
        );
    }
    let summary = render_hook_goal_summary(&plan);
    assert!(!summary.contains("examples/tft/tft.ino"));
    assert!(!summary.contains("source_recovery="));
}

#[test]
fn pure_query_capsules_trim_mutating_context() {
    let plan = plan("T-Display-S3 的 I2C 引脚和外设地址有哪些?");
    let action_ids = plan
        .context_capsule
        .next_actions
        .iter()
        .map(|action| action.id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(action_ids.contains("source-query-io"));
    assert!(action_ids.contains("source-query-i2c"));
    assert!(!action_ids.contains("goal-plan-bridge"));
    assert!(!action_ids.contains("goal-start-dry-run"));
    assert!(plan.context_capsule.demo_refs.is_empty());
    assert!(plan.recipe_ids.is_empty());
    assert!(plan.context_capsule.implementation_start.is_none());
    assert!(plan.context_capsule.critical_facts.is_empty());
    assert!(
        plan.context_capsule
            .next_actions
            .iter()
            .all(|action| action.permission == "none")
    );
}

#[test]
fn context_budget_caps() {
    let plan = plan("T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?");
    let budget = &plan.context_capsule.budget;
    assert!(plan.context_capsule.source_refs.len() <= budget.max_source_refs_inline);
    assert!(plan.context_capsule.discovery_hints.len() <= budget.max_discovery_hints_inline);
    assert!(plan.context_capsule.reference_hints.len() <= budget.max_reference_hints_inline);
    assert!(plan.context_capsule.playbook_hints.len() <= budget.max_playbook_hints_inline);
    assert!(
        plan.context_capsule
            .fact_tables
            .iter()
            .all(|table| table.rows.len() <= budget.max_fact_rows_per_table)
    );
}

#[test]
fn playbook_goal_capsule_budget() {
    let plan = plan("T-Watch Ultra LVGL blank screen touch debug");
    let ids = plan
        .context_capsule
        .playbook_hints
        .iter()
        .map(|hint| hint.playbook_id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(ids.contains("playbook-source-discovery"));
    assert!(ids.contains("playbook-lvgl-debug"));
    assert!(
        plan.route
            .playbooks
            .contains(&"playbook-lvgl-debug".to_string())
    );
    assert!(
        plan.context_capsule.playbook_hints.len()
            <= plan.context_capsule.budget.max_playbook_hints_inline
    );
    assert!(plan.context_capsule.playbook_hints.iter().all(|hint| {
        hint.expand_command
            .starts_with("lilygo-skills index query playbook-")
    }));
}

#[test]
fn playbook_evidence_boundaries() {
    let lvgl = plan("T-Watch Ultra LVGL blank screen touch debug");
    assert_eq!(lvgl.context_capsule.boundary.verification_level, "V3");
    assert!(!lvgl.context_capsule.boundary.hardware_verified);
    let lvgl_claims = lvgl
        .context_capsule
        .playbook_hints
        .iter()
        .flat_map(|hint| hint.anti_claims.iter())
        .map(|claim| claim.to_lowercase())
        .collect::<Vec<_>>();
    assert!(
        lvgl_claims
            .iter()
            .any(|claim| claim.contains("cannot prove"))
    );

    let ota = plan("T-Watch Ultra ESP-IDF OTA rollback manifest debug");
    let ota_ids = ota
        .context_capsule
        .playbook_hints
        .iter()
        .map(|hint| hint.playbook_id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(ota_ids.contains("playbook-ota-debug"));
    assert!(ota_ids.contains("playbook-build-flash-serial"));
    assert!(
        ota.context_capsule
            .playbook_hints
            .iter()
            .flat_map(|hint| hint.anti_claims.iter())
            .any(|claim| claim.contains("planning evidence") || claim.contains("credentials"))
    );
}

#[test]
fn playbook_board_fact_precedence() {
    let plan = plan("T-Watch Ultra add display driver BSP status action smoke");
    assert!(
        plan.context_capsule
            .playbook_hints
            .iter()
            .any(|hint| hint.playbook_id == "playbook-bsp-driver")
    );
    assert!(
        plan.context_capsule
            .facts
            .iter()
            .any(|fact| fact.key == "chip" && fact.value == "CO5300")
    );
    assert!(
        plan.context_capsule
            .facts
            .iter()
            .any(|fact| fact.key == "driver" && fact.value.contains("display"))
    );
}

#[test]
fn playbook_reference_hints() {
    let temp =
        std::env::temp_dir().join(format!("lilygo-playbook-reference-{}", std::process::id()));
    let refs_dir = temp.join(".lilygo-skills");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&refs_dir).expect("refs dir");
    fs::write(
        refs_dir.join("references.json"),
        r#"{
          "schema_version": 1,
          "entries": [{
            "id": "project-serial-debug",
            "title": "Project serial debug tool",
            "kind": "tool",
            "applies_to": ["serial", "debug"],
            "path_or_url": "https://github.com/Adancurusul/serial-mcp-server",
            "authority": "operating-pattern",
            "summary": "Use serial-mcp-server for bounded UART observation.",
            "read_when": "Serial debug requested.",
            "inject_triggers": ["serial", "debug", "log"]
          }]
        }"#,
    )
    .expect("references file");
    let plan = plan_for_project("T-Watch Ultra serial boot log debug", &temp);
    assert!(
        plan.context_capsule
            .reference_hints
            .iter()
            .any(|hint| hint.reference_id == "project-serial-debug")
    );
    assert!(
        plan.context_capsule
            .playbook_hints
            .iter()
            .any(|hint| hint.playbook_id == "playbook-build-flash-serial")
    );
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn playbook_preference_hints() {
    let temp =
        std::env::temp_dir().join(format!("lilygo-playbook-preference-{}", std::process::id()));
    let prefs_dir = temp.join(".lilygo-skills");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&prefs_dir).expect("prefs dir");
    fs::write(
        prefs_dir.join("preferences.json"),
        r#"{
          "schema_version": 1,
          "debug_tools": ["serial-mcp-server"]
        }"#,
    )
    .expect("preferences file");
    let plan = plan_for_project("T-Watch Ultra serial debug with logs", &temp);
    assert!(
        plan.context_capsule
            .preferences
            .iter()
            .any(|hint| { hint.key == "debug_tools" && hint.value.contains("serial-mcp-server") })
    );
    assert!(
        plan.context_capsule
            .playbook_hints
            .iter()
            .any(|hint| hint.playbook_id == "playbook-build-flash-serial")
    );
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn playbook_privacy_boundary() {
    let temp = std::env::temp_dir().join(format!("lilygo-playbook-privacy-{}", std::process::id()));
    let local_dir = temp.join(".lilygo-skills");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&local_dir).expect("local dir");
    fs::write(
        local_dir.join("local.json"),
        r#"{
          "schema_version": 1,
          "serial_port": "/dev/cu.usbmodem101",
          "ota_manifest_url": "http://192.168.0.2:8080/manifest.json"
        }"#,
    )
    .expect("local file");
    let plan = plan_for_project("T-Watch Ultra OTA manifest serial debug", &temp);
    let rendered = serde_json::to_string(&plan).expect("plan json");
    assert!(!rendered.contains("/dev/cu"));
    assert!(!rendered.contains("192.168."));
    assert!(rendered.contains("private.local_state"));
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn compact_injection_overflow_refs() {
    let plan = plan("T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?");
    assert!(plan.context_capsule.budget.overflow_count > 0);
    assert!(
        plan.context_capsule
            .fact_tables
            .iter()
            .any(|table| table.overflow_count > 0 && table.query_command.contains("source query"))
    );
}

#[test]
fn context_budget_dedupes_repeated_capsules() {
    let plan = plan("T-Display-S3 debug I2C I2C I2C sensor screen screen screen");
    let action_ids = plan
        .context_capsule
        .next_actions
        .iter()
        .map(|action| action.id.as_str())
        .collect::<Vec<_>>();
    let unique = action_ids.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(action_ids.len(), unique.len());
    assert!(
        plan.context_capsule
            .next_actions
            .iter()
            .any(|action| action.id == "source-query-i2c")
    );
    assert!(
        plan.context_capsule
            .fact_tables
            .iter()
            .any(|table| table.query_command.contains("source query"))
    );
    assert!(render_hook_goal_summary(&plan).len() < 1400);
}

#[test]
fn lookup_capsule_injects_display_and_i2c_pins() {
    // A pin/bus lookup prompt must surface the concrete GPIO occupancy inline
    // (the great-effect injection lever), not only an expand pointer. The full
    // parallel-bus data pins land, and each display GPIO is spelled out so a
    // reader sees D0..D7 individually rather than a "/"-joined shorthand.
    let summary = render_hook_goal_summary(&plan("T-Display-S3 的 I2C 引脚和屏幕占用了哪些 GPIO?"));
    assert!(
        summary.contains("pins=["),
        "missing pins segment: {summary}"
    );
    for gpio in [
        "GPIO39", "GPIO40", "GPIO41", "GPIO42", "GPIO45", "GPIO46", "GPIO47", "GPIO48",
    ] {
        assert!(
            summary.contains(gpio),
            "display pin {gpio} not injected: {summary}"
        );
    }
    assert!(
        summary.contains("GPIO18") && summary.contains("GPIO17"),
        "i2c pins missing"
    );
    // The injected capsule must stay small even with the pins surfaced.
    assert!(summary.len() < 1400, "capsule too large: {}", summary.len());
}

#[test]
fn lookup_capsule_injects_power_and_haptic_chip() {
    // Naming the power/haptic subsystem must surface that peripheral's exact
    // chip and I2C address instead of leaving it behind the expand pointer.
    let power = render_hook_goal_summary(&plan("T-Watch Ultra 电源管理芯片是什么?"));
    assert!(
        power.contains("AXP2101") && power.contains("0x34"),
        "power chip missing: {power}"
    );
    let haptic = render_hook_goal_summary(&plan("T-Watch Ultra 震动马达怎么控制?"));
    assert!(
        haptic.contains("DRV2605") && haptic.contains("0x5A"),
        "haptic chip missing: {haptic}"
    );
}

#[test]
fn project_local_private_state_hint_is_presence_only() {
    let prompt = "T-Watch Ultra OTA over WiFi then serial monitor";
    let temp =
        std::env::temp_dir().join(format!("lilygo-goal-private-local-{}", std::process::id()));
    let child = temp.join("firmware/src");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(temp.join(".lilygo-skills")).expect("local dir");
    fs::create_dir_all(&child).expect("child dir");
    fs::write(
        temp.join(".lilygo-skills/local.json"),
        r#"{"wireless_name":"SyntheticLocalNetwork","wireless_key":"SyntheticLocalKey","ota_host":"SyntheticLocalTarget","serial_port":"/dev/cu.usbmodem-private"}"#,
    )
    .expect("local config");
    let root = root();
    let registry = load_registry(root.as_path()).expect("registry");
    let profile = ActiveProfile {
        board: "board-t-watch-ultra".to_string(),
        framework: Some("fw-arduino".to_string()),
        features: Vec::new(),
    };
    let route = route_prompt_with_profile(&registry, prompt, Some(&profile));
    let plan = plan_goal_with_project(root.as_path(), &registry, prompt, &route, Some(&child))
        .expect("goal plan");
    assert!(
        plan.context_capsule
            .facts
            .iter()
            .any(|fact| { fact.key == "private.local_state" && fact.value.contains("present") })
    );
    let rendered = serde_json::to_string(&plan).expect("json");
    for private in [
        "SyntheticLocalNetwork",
        "SyntheticLocalKey",
        "SyntheticLocalTarget",
        "/dev/cu.usbmodem-private",
    ] {
        assert!(!rendered.contains(private), "leaked {private}");
    }
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_lvgl_recipe_selection() {
    let plan = plan("T-Watch Ultra Arduino LVGL touch does not move");
    assert!(
        plan.recipe_ids
            .contains(&"recipe-lvgl-simulator".to_string())
    );
    assert!(plan.recipes.iter().any(|recipe| {
        recipe.id == "recipe-lvgl-simulator"
            && recipe.steps.iter().any(|step| step.id == "page-data")
            && recipe
                .steps
                .iter()
                .any(|step| step.id == "simulator-render")
    }));
}

#[test]
fn goal_ota_recipe_selection() {
    let plan = plan("T-Watch Ultra OTA manifest downloaded then rebooted");
    assert!(plan.recipe_ids.contains(&"recipe-ota-debug".to_string()));
    let ota = plan
        .recipes
        .iter()
        .find(|recipe| recipe.id == "recipe-ota-debug")
        .expect("ota recipe");
    for step in ["partition-check", "manifest-check", "ota-observe"] {
        assert!(
            ota.steps.iter().any(|item| item.id == step),
            "missing {step}"
        );
    }
    assert!(
        ota.expected_observations
            .iter()
            .any(|observation| observation.contains("rollback"))
    );
}

#[test]
fn goal_privacy_boundary() {
    let plan = plan("T-Watch Ultra serial boot log unreadable");
    assert!(
        plan.privacy
            .local_state
            .contains(&".lilygo-skills/evidence/".to_string())
    );
    assert!(plan.privacy.committed_state.iter().all(|item| {
        !item.contains("port") && !item.contains("ssid") && !item.contains("password")
    }));
    let rendered = serde_json::to_string(&plan).expect("render");
    assert!(!rendered.contains("/dev/"));
    assert!(!rendered.contains("192.168."));
}

#[test]
fn goal_source_authority() {
    let plan = plan("T-Watch Ultra Arduino IMU 抬腕检测怎么做");
    let docs = plan
        .context_capsule
        .source_refs
        .iter()
        .find(|source| source.url == DOCUMENTATION_REPO)
        .expect("documentation repo source");
    let official = plan
        .context_capsule
        .source_refs
        .iter()
        .find(|source| source.kind == "driver-header" || source.kind == "chip-vendor")
        .expect("official source");
    assert!(official.authority_rank > docs.authority_rank);
}

#[test]
fn public_recipe_source_pack() {
    let registry = crate::recipes::recipe_registry();
    assert!(registry.source_packs.iter().any(|pack| {
        pack.id == "recipe-pack-bsp-chip-driver"
            && !pack.source_refs.is_empty()
            && pack
                .source_refs
                .iter()
                .all(|source| source.starts_with("https://"))
    }));
}

#[test]
fn demo_intent_minimal_display_demo() {
    let display_plan =
        plan("T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor");
    let demos = &display_plan.context_capsule.demo_refs;
    assert_eq!(
        demos.first().map(|demo| demo.path.as_str()),
        Some("examples/tft/tft.ino")
    );
    assert!(
        demos
            .iter()
            .any(|demo| demo.path == "examples/factory/factory.ino")
    );

    let factory = plan("T-Display-S3 Arduino factory full peripheral test");
    assert_eq!(
        factory
            .context_capsule
            .demo_refs
            .first()
            .map(|demo| demo.path.as_str()),
        Some("examples/factory/factory.ino")
    );
}

#[test]
fn demo_intent_chinese_minimal_display_demo() {
    let display_plan = plan("T-Display-S3 Arduino 帮我让屏幕先亮起来，跑个最简单的显示例程");
    assert_eq!(
        display_plan
            .context_capsule
            .demo_refs
            .first()
            .map(|demo| demo.path.as_str()),
        Some("examples/tft/tft.ino")
    );
    assert!(
        display_plan
            .context_capsule
            .next_actions
            .iter()
            .any(|action| action.id == "goal-plan-bridge")
    );

    let factory_plan = plan("T-Display-S3 Arduino 跑完整出厂测试");
    assert_eq!(
        factory_plan
            .context_capsule
            .demo_refs
            .first()
            .map(|demo| demo.path.as_str()),
        Some("examples/factory/factory.ino")
    );
}

#[test]
fn intent_classification_lookup_prompts_are_read_only() {
    for prompt in [
        "T-Display-S3 which pins are used by the screen?",
        "T-Display-S3 read pinout docs",
        "T-Display-S3 inspect docs",
        "T-Display-S3 locate docs",
        "T-Display-S3 哪些引脚被屏幕占用了?",
        "T-Display-S3 先看一下屏幕占用了哪些 IO",
    ] {
        let plan = plan(prompt);
        let actions = &plan.context_capsule.next_actions;
        assert!(
            actions.iter().all(|action| !action.id.starts_with("goal-")),
            "{prompt}: {actions:?}"
        );
        assert!(
            actions.iter().all(|action| action.permission == "none"),
            "{prompt}: {actions:?}"
        );
        assert!(
            plan.context_capsule.demo_refs.is_empty(),
            "{prompt}: {:?}",
            plan.context_capsule.demo_refs
        );
        assert!(
            plan.recipe_ids.is_empty(),
            "{prompt}: {:?}",
            plan.recipe_ids
        );
        assert!(
            plan.context_capsule.implementation_start.is_none(),
            "{prompt}: {:?}",
            plan.context_capsule.implementation_start
        );
        assert!(
            plan.context_capsule
                .fact_tables
                .iter()
                .any(|table| table.query_command.contains("source query")),
            "{prompt}: {:?}",
            plan.context_capsule.fact_tables
        );
    }

    let watch_s3 = plan("LilyGO T-Watch S3 屏幕和触摸占用了哪些引脚?");
    let action_ids = watch_s3
        .context_capsule
        .next_actions
        .iter()
        .map(|action| action.id.as_str())
        .collect::<Vec<_>>();
    assert!(
        action_ids.contains(&"source-query-display"),
        "{action_ids:?}"
    );
    assert!(action_ids.contains(&"source-query-input"), "{action_ids:?}");
    assert_eq!(
        watch_s3.context_capsule.completeness.get("display"),
        Some(&"complete".to_string())
    );
    assert_eq!(
        watch_s3.context_capsule.completeness.get("input"),
        Some(&"complete".to_string())
    );
    assert!(
        watch_s3
            .context_capsule
            .next_actions
            .iter()
            .all(|action| action.permission == "none"),
        "{:?}",
        watch_s3.context_capsule.next_actions
    );
}

#[test]
fn intent_classification_mixed_prompt_prefers_action() {
    let mixed_plan = plan("T-Display-S3 查一下引脚，然后帮我点亮屏幕");
    let actions = &mixed_plan.context_capsule.next_actions;
    assert!(
        actions.iter().any(|action| action.id == "goal-plan-bridge"),
        "{actions:?}"
    );
    assert!(
        actions.iter().any(|action| action.id == "source-query-io"),
        "{actions:?}"
    );
    assert!(
        mixed_plan
            .context_capsule
            .demo_refs
            .iter()
            .any(|demo| demo.path == "examples/tft/tft.ino"),
        "{:?}",
        mixed_plan.context_capsule.demo_refs
    );

    let read_sensor_plan = plan("T-Display-S3 read I2C sensor data");
    assert!(
        read_sensor_plan
            .context_capsule
            .next_actions
            .iter()
            .any(|action| action.id == "goal-plan-bridge"),
        "{:?}",
        read_sensor_plan.context_capsule.next_actions
    );
}

#[test]
fn goal_next_actions_are_permission_aware() {
    let implementation_plan =
        plan("T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor");
    let actions = &implementation_plan.context_capsule.next_actions;
    assert!(actions.iter().any(|action| action.id == "goal-plan-bridge"));
    assert!(actions.iter().any(|action| action.id == "source-query-io"));
    assert!(actions.iter().any(|action| action.id == "source-query-i2c"));
    // Execution next-actions (goal-start-dry-run / goal-build / goal-flash-monitor)
    // were dropped with the goal command surface; every retained next-action is a
    // read-only, permission=none pointer.
    assert!(actions.iter().all(|action| action.permission == "none"));
    assert!(
        actions
            .iter()
            .all(|action| !action.command.contains("/Users/"))
    );

    let multi_bus_plan = plan("T-Display-S3 debug an SPI sensor and UART module");
    let multi_bus_actions = &multi_bus_plan.context_capsule.next_actions;
    assert!(
        multi_bus_actions
            .iter()
            .any(|action| action.id == "source-query-spi")
    );
    assert!(
        multi_bus_actions
            .iter()
            .any(|action| action.id == "source-query-uart")
    );

    let fact_lookup = plan("T-Display-S3 Arduino IO口怎么用? 哪些GPIO接了外设?");
    assert!(
        fact_lookup
            .context_capsule
            .next_actions
            .iter()
            .all(|action| action.permission == "none")
    );
    assert!(fact_lookup.context_capsule.demo_refs.is_empty());
}

#[test]
fn goal_bridge_actions_for_implementation_prompts() {
    let plan = plan("T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor");
    // The read-only bridge now points at the retained `context --plan` view
    // (the goal command surface is gone); the capsule token id stays stable so
    // the graded `next=[goal-plan-bridge:none,..]` string is unchanged.
    let bridge = plan
        .context_capsule
        .next_actions
        .iter()
        .find(|action| action.id == "goal-plan-bridge")
        .expect("capsule-plan bridge action");
    assert_eq!(bridge.permission, "none");
    assert!(bridge.command.contains("context --plan --json"));
    assert!(bridge.command.contains("T-Display-S3"));
    let summary = render_hook_goal_summary(&plan);
    assert!(summary.contains("goal-plan-bridge:none"));
    assert!(summary.contains("source-query-i2c:none"));
}

#[test]
fn starter_board_data_is_source_backed() {
    let root = root();
    let beam_lora =
        crate::facts::source_query(root.as_path(), "board-t-beam", "lora").expect("beam lora");
    assert!(
        beam_lora
            .source_refs
            .iter()
            .any(|source| source.path_or_url.contains("LilyGo-LoRa-Series"))
    );
    assert!(
        beam_lora
            .facts
            .iter()
            .any(|fact| fact.confidence == "unknown_with_sources")
    );
    // Ingested official pins make lora required-complete; the residual radio
    // chip variant stays unknown_with_sources (asserted above), so completeness
    // reports complete without hiding the remaining gap.
    assert_eq!(
        crate::facts::source_completeness(root.as_path(), "board-t-beam", "lora")
            .expect("beam lora completeness")
            .completeness,
        "complete"
    );
    let deck_display = crate::facts::source_query(root.as_path(), "board-t-deck", "display")
        .expect("deck display");
    assert!(
        deck_display
            .source_refs
            .iter()
            .any(|source| source.path_or_url.contains("T-Deck"))
    );
    assert!(
        deck_display
            .facts
            .iter()
            .any(|fact| fact.confidence == "unknown_with_sources")
    );
    assert_ne!(
        crate::facts::source_completeness(root.as_path(), "board-t-deck", "input")
            .expect("deck input completeness")
            .completeness,
        "complete"
    );
    let amoled_input =
        crate::facts::source_query(root.as_path(), "board-t-display-s3-amoled", "input")
            .expect("amoled input");
    assert!(
        amoled_input
            .facts
            .iter()
            .any(|fact| fact.confidence == "unknown_with_sources")
    );
}
