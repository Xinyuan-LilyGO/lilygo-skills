use super::complete::GoalCompleteResult;
use super::*;
use crate::model::ActiveProfile;
use crate::registry::load_registry;
use crate::router::{route_prompt, route_prompt_with_profile};

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

fn complete(prompt: &str, options: GoalCompleteOptions) -> GoalCompleteResult {
    let root = root();
    let registry = load_registry(root.as_path()).expect("registry");
    let route = route_prompt(&registry, prompt);
    complete_goal(root.as_path(), &registry, prompt, &route, options).expect("goal complete")
}

fn complete_options(project_root: PathBuf) -> GoalCompleteOptions {
    GoalCompleteOptions {
        project_root: project_root.clone(),
        project_start: project_root.clone(),
        generated_root: None,
        allow_generate: false,
        start_options: options(project_root, false),
    }
}

fn json_str<'a>(value: &'a GoalCompleteResult, pointer: &str) -> &'a str {
    value.pointer(pointer).and_then(|v| v.as_str()).unwrap()
}

fn json_bool(value: &GoalCompleteResult, pointer: &str) -> bool {
    value.pointer(pointer).and_then(|v| v.as_bool()).unwrap()
}

fn json_array_has(value: &GoalCompleteResult, pointer: &str, needle: &str) -> bool {
    value
        .pointer(pointer)
        .and_then(|v| v.as_array())
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(needle)))
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
fn goal_complete_schema() {
    let project = std::env::temp_dir().join(format!(
        "lilygo-goal-complete-schema-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).expect("project");
    let result = complete(
        "T-Display-S3 Arduino LVGL display demo",
        complete_options(project.clone()),
    );
    assert_eq!(result["schema_version"], 1);
    assert_eq!(json_str(&result, "/status"), "needs_permission");
    assert_eq!(json_str(&result, "/route/board"), "board-t-display-s3");
    assert_eq!(json_str(&result, "/route/framework"), "fw-arduino");
    assert_eq!(json_str(&result, "/readiness/source/status"), "complete");
    assert_eq!(
        json_str(&result, "/readiness/generated_skills/status"),
        "not_checked"
    );
    assert!(!json_bool(&result, "/execution/attempted"));
    assert!(json_bool(&result, "/privacy/public_output_redacted"));
    assert!(json_array_has(
        &result,
        "/plan/required_permissions",
        "allow-build"
    ));
    let _ = fs::remove_dir_all(&project);
}

#[test]
fn goal_complete_clarification() {
    let project = std::env::temp_dir().join(format!(
        "lilygo-goal-complete-clarification-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).expect("project");
    let result = complete(
        "Arduino LVGL display demo",
        complete_options(project.clone()),
    );
    assert_eq!(json_str(&result, "/status"), "needs_clarification");
    assert_eq!(
        json_str(&result, "/readiness/project/status"),
        "needs_clarification"
    );
    assert!(
        result
            .pointer("/readiness/project/missing")
            .and_then(|value| value.as_array())
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("board")))
    );
    assert!(
        result
            .pointer("/next_actions")
            .and_then(|value| value.as_array())
            .is_some_and(|items| items.iter().any(|item| item["kind"] == "ask_user"))
    );
    let _ = fs::remove_dir_all(&project);
}

#[test]
fn goal_complete_readiness() {
    let project = std::env::temp_dir().join(format!(
        "lilygo-goal-complete-readiness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).expect("project");
    let result = complete("T-Beam LoRa GNSS debug", complete_options(project.clone()));
    assert_eq!(json_str(&result, "/status"), "needs_source_ingestion");
    assert_eq!(
        json_str(&result, "/readiness/source/status"),
        "needs_source_ingestion"
    );
    assert!(
        result
            .pointer("/next_actions")
            .and_then(|value| value.as_array())
            .is_some_and(|items| items.iter().any(|item| item["command"]
                .as_str()
                .is_some_and(|command| command.contains("update board-facts"))))
    );
    let _ = fs::remove_dir_all(&project);
}

#[test]
fn goal_complete_nfc_source_fallback() {
    let project = std::env::temp_dir().join(format!(
        "lilygo-goal-complete-nfc-source-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).expect("project");
    let result = complete(
        "T-Watch Ultra run official NFC demo",
        complete_options(project.clone()),
    );
    assert_eq!(json_str(&result, "/status"), "needs_permission");
    assert_eq!(json_str(&result, "/route/board"), "board-t-watch-ultra");
    assert_eq!(
        json_str(&result, "/readiness/source/status"),
        "source_backed"
    );
    assert!(
        json_str(&result, "/readiness/source/summary").contains("facts="),
        "{}",
        result["readiness"]["source"]
    );
    assert!(
        result
            .pointer("/readiness/source/commands")
            .and_then(|value| value.as_array())
            .is_some_and(|items| items
                .iter()
                .any(|item| item.as_str().is_some_and(|command| command
                    .contains("source query --board board-t-watch-ultra --topic peripheral"))))
    );
    let _ = fs::remove_dir_all(&project);
}

#[test]
fn goal_complete_generation() {
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-complete-generation-{}",
        std::process::id()
    ));
    let project = temp.join("project");
    let generated = temp.join("generated");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&project).expect("project");
    fs::create_dir_all(&generated).expect("generated");
    let mut options = complete_options(project);
    options.generated_root = Some(generated);
    let result = complete("T-Display-S3 Arduino LVGL display demo", options);
    assert_eq!(json_str(&result, "/status"), "needs_generation");
    assert_eq!(
        json_str(&result, "/readiness/generated_skills/status"),
        "missing"
    );
    assert!(
        result
            .pointer("/next_actions")
            .and_then(|value| value.as_array())
            .is_some_and(|items| items.iter().any(|item| item["command"]
                .as_str()
                .is_some_and(|command| command.contains("generate skills"))))
    );
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_complete_generation_intent_defaults_to_project_cache() {
    let project = std::env::temp_dir().join(format!(
        "lilygo-goal-complete-generation-intent-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).expect("project");
    let result = complete(
        "重新生成这个项目的 LilyGO skills，并检查是否完整",
        complete_options(project.clone()),
    );
    assert_eq!(json_str(&result, "/status"), "needs_generation");
    assert_eq!(
        json_str(&result, "/readiness/generated_skills/status"),
        "missing"
    );
    assert!(
        result
            .pointer("/next_actions")
            .and_then(|value| value.as_array())
            .is_some_and(|items| items.iter().any(|item| item["command"]
                .as_str()
                .is_some_and(|command| command.contains(".lilygo-skills/generated-skills"))))
    );
    let _ = fs::remove_dir_all(&project);
}

#[test]
fn goal_complete_execution() {
    let plan = rust_build_plan();
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-complete-execution-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    let source = temp.join("ok-crate");
    let project = temp.join("project");
    let _ = fs::remove_dir_all(&temp);
    write_rust_crate(&source, "fn main() {}\n");
    fs::create_dir_all(&project).expect("project");
    let mut start_options = allow_build_options(project.clone(), source);
    start_options.allow_flash = true;
    start_options.allow_serial = true;
    start_options.port = Some("/tmp/lilygo-invalid-serial".to_string());
    let options = GoalCompleteOptions {
        project_root: project.clone(),
        project_start: project.clone(),
        generated_root: None,
        allow_generate: false,
        start_options,
    };
    let root = root();
    let registry = load_registry(root.as_path()).expect("registry");
    let route = route_prompt(&registry, "T-Watch Ultra Rust build firmware");
    let result = complete_goal(
        root.as_path(),
        &registry,
        "T-Watch Ultra Rust build firmware",
        &route,
        options,
    )
    .expect("complete");
    assert!(json_bool(&result, "/execution/attempted"));
    assert_eq!(
        json_str(&result, "/evidence/highest_verification_level"),
        "V4"
    );
    assert_eq!(json_str(&result, "/status"), "blocked");
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_complete_privacy() {
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-complete-privacy-{}",
        std::process::id()
    ));
    let local_dir = temp.join(".lilygo-skills");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&local_dir).expect("local dir");
    fs::write(
        local_dir.join("local.json"),
        r#"{"schema_version":1,"serial_port":"/dev/cu.private","ota_host":"192.168.0.9"}"#,
    )
    .expect("local");
    let result = complete(
        "T-Watch Ultra Arduino OTA serial debug",
        complete_options(temp.clone()),
    );
    let rendered = serde_json::to_string(&result).expect("json");
    assert!(json_bool(&result, "/privacy/private_state_used"));
    assert!(!rendered.contains("/dev/cu.private"));
    assert!(!rendered.contains("192.168.0.9"));
    let _ = fs::remove_dir_all(&temp);
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
fn source_recovery_hook_summary_t_display_s3() {
    let plan = plan("T-Display-S3 PlatformIO Arduino TFT_eSPI I2C sensor screen");
    let summary = render_hook_goal_summary(&plan);

    assert!(summary.contains("examples/tft/tft.ino"));
    assert!(summary.contains("Setup206_LilyGo_T_Display_S3.h"));
    assert!(summary.contains("pin_config.h"));
    assert!(summary.contains("PIN_IIC_SDA=GPIO18"));
    assert!(summary.contains("PIN_IIC_SCL=GPIO17"));
    assert!(summary.contains("index query playbook-source-discovery --json"));
    assert!(summary.contains("source query --board board-t-display-s3 --topic io --json"));
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
fn goal_start_safety() {
    let plan = plan("T-Watch Ultra Arduino IMU 抬腕检测怎么做");
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-safety-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).expect("temp");
    let dry = start_goal(&plan, &options(temp.clone(), true)).expect("dry-run");
    assert_eq!(dry.status, "PASS");
    assert!(dry.ran_commands.is_empty());
    assert!(dry.writes.is_empty());
    assert_eq!(dry.required_permissions, plan.permissions_required);
    assert_eq!(dry.planned_artifacts, plan.planned_artifacts);
    assert!(
        dry.planned_commands
            .iter()
            .any(|command| command.permission == "allow-flash:port")
    );
    let default = start_goal(&plan, &options(temp.clone(), false)).expect("default dry-run");
    assert_eq!(default.status, "PASS");
    assert!(default.dry_run);
    assert!(default.ran_commands.is_empty());
    assert!(default.writes.is_empty());
    assert!(!temp.join(".gitignore").exists());
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_start_public_output_redacts_paths_and_includes_artifacts() {
    let plan = plan("T-Watch Ultra Arduino IMU 抬腕检测怎么做");
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-public-output-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    let source = temp.join("source root --flag");
    let project = temp.join("project root --flag");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&source).expect("source root");
    fs::create_dir_all(&project).expect("project root");
    let mut options = options(project.clone(), true);
    options.source_root = Some(source.clone());
    options.port = Some("/dev/cu.synthetic-private".to_string());

    let dry = start_goal(&plan, &options).expect("dry-run");
    assert_eq!(dry.required_permissions, plan.permissions_required);
    assert_eq!(dry.planned_artifacts, plan.planned_artifacts);
    assert!(
        dry.planned_commands
            .iter()
            .any(|command| command.command.contains("<redacted-source-root>"))
    );
    assert!(dry.planned_commands.iter().all(|command| {
        !command.command.contains(source.to_string_lossy().as_ref())
            && !command.command.contains(project.to_string_lossy().as_ref())
            && !command.command.contains("/dev/cu.synthetic-private")
            && command
                .working_dir
                .as_deref()
                .is_none_or(|value| !value.contains(source.to_string_lossy().as_ref()))
    }));
    let rendered = serde_json::to_string(&dry).expect("json");
    assert!(!rendered.contains(source.to_string_lossy().as_ref()));
    assert!(!rendered.contains(project.to_string_lossy().as_ref()));
    assert!(!rendered.contains("/dev/cu.synthetic-private"));
    assert!(rendered.contains("planned_artifacts"));
    assert!(rendered.contains("required_permissions"));
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_status_evidence() {
    let plan = rust_build_plan();
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-evidence-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    let source = temp.join("ok-crate");
    let _ = fs::remove_dir_all(&temp);
    write_rust_crate(&source, "fn main() {}\n");
    let _ = start_goal(&plan, &allow_build_options(temp.clone(), source)).expect("partial");
    let evidence = load_goal_evidence(temp.as_path(), &plan.goal_id).expect("evidence");
    assert_eq!(evidence.status, "partial");
    assert_eq!(evidence.highest_verification_level, "V4");
    let cancel = cancel_goal(temp.as_path(), &plan.goal_id).expect("cancel");
    assert_eq!(cancel.status, "PASS");
    let evidence = load_goal_evidence(temp.as_path(), &plan.goal_id).expect("evidence");
    assert_eq!(evidence.status, "interrupted");
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_failure_classification() {
    let mut plan = rust_build_plan();
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-failure-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    let source = temp.join("bad-crate");
    let _ = fs::remove_dir_all(&temp);
    write_rust_crate(
        &source,
        "fn main() { let value: u32 = \"bad\"; let _ = value; }\n",
    );
    let result =
        start_goal(&plan, &allow_build_options(temp.clone(), source)).expect("failed build result");
    assert_eq!(result.status, "BLOCKED");
    assert_eq!(result.highest_verification_level, "V3");
    assert_eq!(result.failure_class.as_deref(), Some("build-failure"));
    assert_eq!(result.repeated_failure_count, Some(1));
    assert_eq!(result.retry_limit, Some(1));
    assert!(
        result
            .ran_commands
            .iter()
            .any(|command| command.step_id == "build" && command.status == "FAIL")
    );
    let evidence = load_goal_evidence(temp.as_path(), &plan.goal_id).expect("evidence");
    assert_eq!(evidence.status, "blocked");
    assert_eq!(evidence.highest_verification_level, "V3");
    assert!(!evidence.hardware_verified);
    assert_eq!(evidence.repeated_failure_count, Some(1));
    assert_eq!(evidence.retry_limit, Some(1));
    let _ = fs::remove_dir_all(&temp);

    plan.goal_id = "../../outside".to_string();
    let err = start_goal(&plan, &options(std::env::temp_dir(), true)).expect_err("invalid id");
    assert!(err.contains("invalid goal id"));
}

#[test]
fn repeated_goal_failure_routes_to_problem_solving() {
    let plan = rust_build_plan();
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-repeated-failure-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    let source = temp.join("bad-crate");
    let _ = fs::remove_dir_all(&temp);
    write_rust_crate(
        &source,
        "fn main() { let value: u32 = \"bad\"; let _ = value; }\n",
    );
    let options = allow_build_options(temp.clone(), source);
    let first = start_goal(&plan, &options).expect("first failed build");
    assert_eq!(first.status, "BLOCKED");
    assert_eq!(first.repeated_failure_count, Some(1));
    assert!(
        first
            .next_action
            .as_deref()
            .is_some_and(|action| action.contains("next identical failure"))
    );
    let second = start_goal(&plan, &options).expect("second failed build");
    assert_eq!(second.status, "BLOCKED");
    assert_eq!(second.repeated_failure_count, Some(2));
    assert_eq!(second.retry_limit, Some(1));
    assert!(
        second
            .next_action
            .as_deref()
            .is_some_and(|action| action.contains("problem-solving")),
        "{:?}",
        second.next_action
    );
    let evidence = load_goal_evidence(temp.as_path(), &plan.goal_id).expect("evidence");
    assert_eq!(evidence.repeated_failure_count, Some(2));
    assert!(
        evidence
            .next_action
            .as_deref()
            .is_some_and(|action| action.contains("problem-solving"))
    );
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_build_only_can_pass_without_flashing() {
    let plan = rust_build_plan();
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-build-only-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    let source = temp.join("ok-crate");
    let _ = fs::remove_dir_all(&temp);
    write_rust_crate(&source, "fn main() {}\n");
    let result =
        start_goal(&plan, &allow_build_options(temp.clone(), source)).expect("build result");
    assert_eq!(result.status, "PASS");
    assert_eq!(result.highest_verification_level, "V4");
    let expected_evidence_path = format!(".lilygo-skills/evidence/{}/evidence.json", plan.goal_id);
    assert_eq!(
        result.evidence_path.as_deref(),
        Some(expected_evidence_path.as_str())
    );
    assert!(!temp.join(".gitignore").exists());
    assert!(!result.hardware_verified);
    assert!(
        result
            .blocked_permissions
            .contains(&"allow-flash".to_string())
    );
    assert!(
        result
            .ran_commands
            .iter()
            .any(|command| command.step_id == "build" && command.status == "PASS")
    );
    let evidence = load_goal_evidence(temp.as_path(), &plan.goal_id).expect("evidence");
    assert_eq!(evidence.status, "partial");
    assert_eq!(evidence.highest_verification_level, "V4");
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_blocked_after_build_preserves_v4_evidence() {
    let plan = rust_build_plan();
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-build-then-blocked-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    let source = temp.join("ok-crate");
    let _ = fs::remove_dir_all(&temp);
    write_rust_crate(&source, "fn main() {}\n");
    let mut options = allow_build_options(temp.clone(), source);
    options.allow_flash = true;
    options.port = Some("/tmp/lilygo-invalid-serial".to_string());

    let result = start_goal(&plan, &options).expect("blocked after build");
    assert_eq!(result.status, "BLOCKED");
    assert_eq!(result.highest_verification_level, "V4");
    assert!(!result.hardware_verified);
    assert!(
        result
            .ran_commands
            .iter()
            .any(|command| command.step_id == "build" && command.status == "PASS")
    );
    assert!(
        result
            .ran_commands
            .iter()
            .any(|command| command.step_id == "upload" && command.status == "BLOCKED")
    );
    let evidence = load_goal_evidence(temp.as_path(), &plan.goal_id).expect("evidence");
    assert_eq!(evidence.status, "blocked");
    assert_eq!(evidence.highest_verification_level, "V4");
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_command_argv_preserves_user_values() {
    let mut options = options(PathBuf::from("/tmp/project root --flag"), true);
    options.source_root = Some(PathBuf::from("/tmp/source root --flag"));
    options.port = Some("/tmp/serial port --erase".to_string());

    let imu_plan = plan("T-Watch Ultra Arduino IMU 抬腕检测怎么做");
    let commands = super::runner::planned_commands(&imu_plan, &options);
    let monitor = commands
        .iter()
        .find(|command| command.step_id == "monitor")
        .expect("monitor command");
    assert!(
        monitor
            .argv
            .contains(&"--port=/tmp/serial port --erase".to_string()),
        "{:?}",
        monitor.argv
    );
    assert!(!monitor.argv.iter().any(|arg| arg == "--erase"));
    assert!(!monitor.argv.iter().any(|arg| arg == "--no-reset"));

    let lvgl_plan = plan("T-Watch Ultra Arduino LVGL touch does not move");
    let commands = super::runner::planned_commands(&lvgl_plan, &options);
    let page_data = commands
        .iter()
        .find(|command| command.step_id == "page-data")
        .expect("page-data command");
    assert!(
        page_data
            .argv
            .contains(&"/tmp/source root --flag".to_string()),
        "{:?}",
        page_data.argv
    );
}

#[test]
fn arduino_watch_ultra_commands_use_verified_board_profile() {
    let mut options = options(PathBuf::from("/tmp/project"), true);
    options.source_root = Some(PathBuf::from("/tmp/LilyGoLib root"));
    options.port = Some("/tmp/watch port".to_string());
    let plan = plan("T-Watch Ultra Arduino BHI260AP IMU demo build flash monitor");
    let commands = super::runner::planned_commands(&plan, &options);
    let build = commands
        .iter()
        .find(|command| command.step_id == "build")
        .expect("build command");
    let fqbn = "esp32:esp32:twatch_ultra:UploadSpeed=921600,USBMode=hwcdc,CDCOnBoot=default,UploadMode=default,CPUFreq=240,PartitionScheme=app3M_fat9M_16MB,LoopCore=1,EventsCore=1,Revision=Radio_SX1262";
    assert!(build.argv.contains(&fqbn.to_string()), "{:?}", build.argv);
    assert!(
        build.argv.contains(&"/tmp/LilyGoLib root".to_string()),
        "{:?}",
        build.argv
    );
    assert!(
        build
            .argv
            .contains(&"/tmp/LilyGoLib root/../LilyGoLib-ThirdParty".to_string()),
        "{:?}",
        build.argv
    );
    assert!(
        build
            .argv
            .iter()
            .filter(|arg| arg.as_str() == "--libraries")
            .count()
            == 2,
        "{:?}",
        build.argv
    );
    assert!(
        build
            .argv
            .iter()
            .all(|arg| !arg.contains("lilygo_twatch_ultra")),
        "{:?}",
        build.argv
    );

    let upload = commands
        .iter()
        .find(|command| command.step_id == "upload")
        .expect("upload command");
    assert!(upload.argv.contains(&fqbn.to_string()), "{:?}", upload.argv);
    assert!(
        upload
            .argv
            .iter()
            .all(|arg| !arg.contains("lilygo_twatch_ultra")),
        "{:?}",
        upload.argv
    );

    let monitor = commands
        .iter()
        .find(|command| command.step_id == "monitor")
        .expect("monitor command");
    assert!(!monitor.argv.iter().any(|arg| arg == "--no-reset"));
}

#[test]
fn observation_timeout_with_payload_counts_as_pass() {
    assert_eq!(
        super::runner::command_status(
            "monitor",
            false,
            true,
            "ESP-ROM:esp32s3-20210327\n[T: 1.0] AX:+0.24 AY:-0.06 AZ:+1.01\ncommand timed out after 10s",
        ),
        "PASS"
    );
    assert_eq!(
        super::runner::command_status(
            "monitor",
            false,
            true,
            "[2026-07-03T03:19:58Z INFO ] Serial port: '<redacted-port>'\n[2026-07-03T03:19:58Z INFO ] Connecting...\ncommand timed out after 10s",
        ),
        "FAIL"
    );
    assert_eq!(
        super::runner::command_status(
            "monitor",
            false,
            true,
            "Chip type: ESP32-S3\nCrystal is 40 MHz\nFeatures: WiFi, BLE\nError: no app logs\ncommand timed out after 10s",
        ),
        "FAIL"
    );
    assert_eq!(
        super::runner::command_status("build", false, true, "compiler output"),
        "FAIL"
    );
    let excerpt = super::observation::observation_excerpt(
        "ESP-ROM:esp32s3-20210327\n\
         line 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n\
         line 11\nline 12\nline 13\nline 14\nline 15\nline 16\nline 17\nline 18\nline 19\nline 20\n\
         Product ID     : 89\n\
         Sensor ID | Sensor Name\n\
         57 | Wake gesture\n\
         67 | Wrist tilt gesture\n\
         [T: 1.0] AX:+0.24 AY:-0.06 AZ:+1.01 GX:-0.43 GY:+0.49 GZ:+0.06\n",
    );
    assert!(excerpt.contains("Product ID"));
    assert!(excerpt.contains("Wrist tilt gesture"));
    assert!(excerpt.contains("AX:+0.24"));
}

#[test]
fn arduino_partition_check_is_manual_not_executed() {
    let plan = plan("T-Watch Ultra OTA manifest downloaded then rebooted");
    let commands =
        super::runner::planned_commands(&plan, &options(PathBuf::from("/tmp/project"), false));
    let partition = commands
        .iter()
        .find(|command| command.step_id == "partition-check")
        .expect("partition-check command");
    assert!(partition.argv.is_empty(), "{:?}", partition.argv);
    assert!(partition.command.contains("inspect Arduino partition"));
    let executable = super::runner::executable_commands(&commands);
    assert!(
        executable
            .iter()
            .all(|command| command.step_id != "partition-check"),
        "{:?}",
        executable
    );
}

#[test]
fn ota_steps_are_manual_without_project_runner() {
    let plan = plan("T-Watch Ultra OTA manifest downloaded then rebooted");
    let commands =
        super::runner::planned_commands(&plan, &options(PathBuf::from("/tmp/project"), false));
    for step_id in ["manifest-check", "ota-observe"] {
        let step = commands
            .iter()
            .find(|command| command.step_id == step_id)
            .unwrap_or_else(|| panic!("missing {step_id}"));
        assert!(step.argv.is_empty(), "{:?}", step);
        assert!(step.command.contains("project OTA"), "{:?}", step);
        assert!(!step.command.contains("lilygo-ota"), "{:?}", step);
    }
}

#[test]
fn goal_ota_allowed_blocks_without_concrete_evidence() {
    let plan = plan("T-Watch Ultra OTA manifest downloaded then rebooted");
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-ota-blocked-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).expect("temp");
    let mut options = options(temp.clone(), false);
    options.allow_build = true;
    options.allow_network = true;
    options.allow_ota = true;
    options.allow_serial = true;
    options.port = Some("/tmp/lilygo-invalid-serial".to_string());
    let result = start_goal(&plan, &options).expect("ota blocked result");
    assert_eq!(result.status, "BLOCKED");
    assert_eq!(result.highest_verification_level, "V3");
    assert!(!result.hardware_verified);
    assert!(result.failure_class.is_some());
    let evidence = load_goal_evidence(temp.as_path(), &plan.goal_id).expect("evidence");
    assert_eq!(evidence.status, "blocked");
    assert_eq!(evidence.highest_verification_level, "V3");
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn local_ota_runner_executes_private_project_argv() {
    let mut plan = plan("T-Watch Ultra OTA manifest downloaded then rebooted");
    plan.recipe_ids.retain(|id| id == "recipe-ota-debug");
    plan.recipes
        .retain(|recipe| recipe.id == "recipe-ota-debug");
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-ota-local-runner-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(temp.join(".lilygo-skills")).expect("local dir");
    fs::write(
        temp.join(".lilygo-skills/local.json"),
        r#"{
  "ota_manifest_argv": ["sh", "-c", "printf 'manifest SyntheticLocalValue SyntheticLocalTarget'"],
  "ota_observe_argv": ["sh", "-c", "printf 'observe SyntheticLocalValue SyntheticLocalTarget'"]
}"#,
    )
    .expect("local config");
    let mut options = options(temp.clone(), false);
    options.allow_network = true;
    options.allow_ota = true;
    options.allow_serial = true;
    options.port = Some("/tmp/lilygo-invalid-serial".to_string());
    let commands = super::runner::planned_commands(&plan, &options);
    for step_id in ["manifest-check", "ota-observe"] {
        let step = commands
            .iter()
            .find(|command| command.step_id == step_id)
            .unwrap_or_else(|| panic!("missing {step_id}"));
        assert!(!step.argv.is_empty(), "{:?}", step);
        assert!(!step.command.contains("SyntheticLocalValue"), "{:?}", step);
        assert!(step.command.contains("project OTA"), "{:?}", step);
    }
    let result = start_goal(&plan, &options).expect("local ota runner");
    assert_eq!(result.status, "PASS");
    assert_eq!(result.highest_verification_level, "V5");
    let rendered = serde_json::to_string(&result).expect("json");
    assert!(!rendered.contains("SyntheticLocalValue"));
    assert!(!rendered.contains("SyntheticLocalTarget"));
    assert!(rendered.contains("private local OTA command output omitted"));
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_start_ignores_untrusted_plan_recipe_commands() {
    let mut plan = plan("T-Watch Ultra Rust build firmware");
    let temp = std::env::temp_dir().join(format!(
        "lilygo-goal-untrusted-plan-{}-{}",
        std::process::id(),
        plan.goal_id
    ));
    if let Some(recipe) = plan.recipes.first_mut()
        && let Some(step) = recipe.steps.first_mut()
    {
        step.id = "check-toolchain".to_string();
        step.command = "rm -rf /tmp/lilygo-should-not-run".to_string();
        step.permission = "read-only".to_string();
    }
    let dry = start_goal(&plan, &options(temp, true)).expect("dry-run");
    assert!(
        dry.planned_commands
            .iter()
            .all(|command| !command.command.contains("rm -rf")),
        "{:?}",
        dry.planned_commands
    );
}

#[test]
fn goal_id_rejects_path_traversal() {
    let temp = std::env::temp_dir().join(format!("lilygo-goal-traversal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).expect("temp");
    assert!(load_goal_evidence(temp.as_path(), "../../outside").is_err());
    assert!(cancel_goal(temp.as_path(), "../../outside").is_err());
    assert!(!temp.join("outside").exists());
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn goal_evidence_redacts_private_output() {
    let mut options = options(PathBuf::from("/private/project"), true);
    options.source_root = Some(PathBuf::from("/private/source"));
    options.port = Some("/dev/cu.synthetic-private".to_string());
    let redacted = redact_sensitive(
        "access_token=abc\n/dev/cu.synthetic-private\n/private/source/src\nhost=192.168.1.40\nmdns=lilygo-watch.local\nmac=aa:bb:cc:dd:ee:ff\nUSB ID VID:PID 303A:1001",
        &options,
    );
    assert!(redacted.contains("[redacted sensitive output line]"));
    assert!(redacted.contains("[redacted private output line]"));
    assert!(!redacted.contains("abc"));
    assert!(!redacted.contains("/dev/cu.synthetic-private"));
    assert!(!redacted.contains("/private/source"));
    assert!(!redacted.contains("192.168.1.40"));
    assert!(!redacted.contains("lilygo-watch.local"));
    assert!(!redacted.contains("aa:bb:cc:dd:ee:ff"));
    assert!(!redacted.contains("303A:1001"));
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
fn observation_timeout_next_action_keeps_debug_loop_moving() {
    let retry = RetryState {
        repeated_failure_count: Some(1),
        retry_limit: Some(1),
    };
    let action = failure_next_action("blocked", Some("runtime-timeout-no-observation"), &retry)
        .expect("next action");
    assert!(action.contains("boot/status serial markers"), "{action}");
    assert!(
        action.contains("bounded serial/OTA observation"),
        "{action}"
    );
    assert!(action.contains("problem-solving"), "{action}");
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

fn rust_build_plan() -> GoalPlan {
    let mut plan = plan("T-Watch Ultra Rust build firmware");
    plan.route.framework = Some("fw-rust".to_string());
    plan.route.frameworks = vec!["fw-rust".to_string()];
    if !plan
        .recipe_ids
        .contains(&"recipe-build-upload-monitor".to_string())
    {
        plan.recipe_ids
            .push("recipe-build-upload-monitor".to_string());
    }
    plan
}

fn write_rust_crate(root: &Path, main_rs: &str) {
    fs::create_dir_all(root.join("src")).expect("crate src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"lilygo_goal_test_crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n",
    )
    .expect("cargo toml");
    fs::write(root.join("src/main.rs"), main_rs).expect("main rs");
}

fn options(project_root: PathBuf, dry_run: bool) -> GoalStartOptions {
    GoalStartOptions {
        project_root,
        dry_run,
        allow_build: false,
        allow_flash: false,
        allow_serial: false,
        allow_network: false,
        allow_ota: false,
        allow_simulator: false,
        port: None,
        source_root: None,
    }
}

fn allow_build_options(project_root: PathBuf, source_root: PathBuf) -> GoalStartOptions {
    let mut options = options(project_root, false);
    options.allow_build = true;
    options.source_root = Some(source_root);
    options
}
