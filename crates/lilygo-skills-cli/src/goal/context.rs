//! Builds compact goal capsules from facts, source refs, readiness,
//! preferences, and lookup commands while keeping full sources behind queries.
use super::*;

pub(super) fn compose_context_capsule(
    root: &Path,
    prompt: &str,
    route: &RouteResult,
    goal_route: &GoalRoute,
    project_start: Option<&Path>,
) -> Result<GoalContextCapsule, String> {
    let mut budget = ContextBudget::default();
    let mut facts = Vec::new();
    let fact_tables = Vec::new();
    let mut demo_refs = Vec::new();
    let mut source_refs = Vec::new();
    let preferences = preference_hints_for_prompt(root, project_start, prompt);
    let reference_hints = reference_hints_for_prompt(root, project_start, prompt);
    let mut playbook_hints = crate::playbooks::playbook_hints_for_prompt(prompt, &route.skills);
    let Some(board_id) = &goal_route.board else {
        budget.overflow_count += playbook_hints
            .len()
            .saturating_sub(budget.max_playbook_hints_inline);
        playbook_hints.truncate(budget.max_playbook_hints_inline);
        return Ok(GoalContextCapsule {
            summary: context_summary(route, goal_route),
            facts,
            next_actions: Vec::new(),
            implementation_start: None,
            critical_facts: Vec::new(),
            recovery_actions: Vec::new(),
            internal_skill_hints: Vec::new(),
            fact_tables,
            completeness: BTreeMap::new(),
            readiness: Vec::new(),
            demo_refs,
            source_refs,
            preferences,
            reference_hints,
            playbook_hints,
            discovery_hints: discovery_hints_for_goal(None, prompt),
            budget,
            boundary: boundary(route, "No board-specific evidence was composed."),
        });
    };
    let board_index = load_board_index(root)?;
    let board = board_index
        .boards
        .iter()
        .find(|board| board.id == *board_id)
        .ok_or_else(|| format!("board record not found for {board_id}"))?;
    add_fact(&mut facts, "board", &board.display_name, board_id);
    add_fact(&mut facts, "mcu", &board.mcu, board_id);
    add_fact(
        &mut facts,
        "frameworks",
        &board.frameworks.join(","),
        board_id,
    );
    add_arduino_toolchain_facts(&mut facts, board, goal_route);
    add_private_local_state_hint(&mut facts, project_start, prompt);
    add_board_sources(&mut source_refs, board);
    add_documentation_repo(&mut source_refs);
    add_relevant_peripherals(
        &mut facts,
        &mut source_refs,
        board,
        goal_route,
        prompt,
        root,
    )?;
    demo_refs.extend(super::demo::sorted_demo_refs(board, goal_route, prompt));
    let fact_tables = fact_tables_for_goal(root, board_id, prompt)?;
    add_fact_table_sources(&mut source_refs, &fact_tables);
    dedup_sources(&mut source_refs);
    budget.overflow_count += source_refs
        .len()
        .saturating_sub(budget.max_source_refs_inline);
    cap_source_refs(&mut source_refs, budget.max_source_refs_inline);
    budget.overflow_count += fact_tables
        .iter()
        .map(|table| table.overflow_count)
        .sum::<usize>();
    let readiness = completeness_signals_for_prompt(root, Some(board_id), prompt);
    let completeness = readiness
        .iter()
        .map(|signal| (signal.topic.clone(), signal.completeness.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut discovery_hints = discovery_hints_for_goal(Some(board_id), prompt);
    discovery_hints.extend(readiness.iter().filter_map(readiness_discovery_hint));
    budget.overflow_count += discovery_hints
        .len()
        .saturating_sub(budget.max_discovery_hints_inline);
    discovery_hints.truncate(budget.max_discovery_hints_inline);
    let mut reference_hints = reference_hints;
    budget.overflow_count += reference_hints
        .len()
        .saturating_sub(budget.max_reference_hints_inline);
    reference_hints.truncate(budget.max_reference_hints_inline);
    budget.overflow_count += playbook_hints
        .len()
        .saturating_sub(budget.max_playbook_hints_inline);
    playbook_hints.truncate(budget.max_playbook_hints_inline);
    let implementation_start =
        implementation_start_for_goal(prompt, &demo_refs, &source_refs, &fact_tables);
    let critical_facts = critical_facts_for_goal(prompt, &fact_tables);
    let recovery_actions = recovery_actions_for_goal(board_id, prompt, &fact_tables);
    let mut internal_skill_hints = internal_skill_hints_for_goal(prompt, &playbook_hints);
    super::actions::add_project_skill_hints(&mut internal_skill_hints, route);
    let next_actions =
        super::actions::next_actions_for_goal(board_id, prompt, &demo_refs, &fact_tables, route);
    Ok(GoalContextCapsule {
        summary: context_summary(route, goal_route),
        facts,
        next_actions,
        implementation_start,
        critical_facts,
        recovery_actions,
        internal_skill_hints,
        fact_tables,
        completeness,
        readiness,
        demo_refs,
        source_refs,
        preferences,
        reference_hints,
        playbook_hints,
        discovery_hints,
        budget,
        boundary: boundary(
            route,
            "Goal planning is source/context evidence only until build, simulator, or hardware commands run.",
        ),
    })
}

fn add_arduino_toolchain_facts(
    facts: &mut Vec<GoalFact>,
    board: &BoardRecord,
    goal_route: &GoalRoute,
) {
    if goal_route.framework.as_deref() != Some("fw-arduino") {
        return;
    }
    if board.id == "board-t-watch-ultra" {
        // The T-Watch Ultra Arduino menu uses board-specific option names; the
        // generated command must mirror `arduino-cli board details`.
        add_fact(
            facts,
            "arduino.fqbn",
            "esp32:esp32:twatch_ultra:UploadSpeed=921600,USBMode=hwcdc,CDCOnBoot=default,UploadMode=default,CPUFreq=240,PartitionScheme=app3M_fat9M_16MB,LoopCore=1,EventsCore=1,Revision=Radio_SX1262",
            "arduino-cli board details esp32:esp32:twatch_ultra",
        );
        add_fact(
            facts,
            "arduino.library_roots",
            ".,../LilyGoLib-ThirdParty",
            "official LilyGoLib checkout layout",
        );
    }
}

fn add_private_local_state_hint(
    facts: &mut Vec<GoalFact>,
    project_start: Option<&Path>,
    prompt: &str,
) {
    let prompt = prompt.to_lowercase();
    if !contains_any(
        &prompt,
        &[
            "ota", "wifi", "wi-fi", "serial", "flash", "monitor", "upload", "network", "port",
            "串口", "无线",
        ],
    ) || !has_private_local_config(project_start)
    {
        return;
    }
    add_fact(
        facts,
        "private.local_state",
        "present; read ignored .lilygo-skills/local.json only at execution time for ports, Wi-Fi, OTA targets, and evidence paths; never quote values",
        crate::project_context::LOCAL_FILE,
    );
}

fn has_private_local_config(project_start: Option<&Path>) -> bool {
    let Some(mut cursor) = project_start.map(Path::to_path_buf) else {
        return false;
    };
    loop {
        if cursor.join(crate::project_context::LOCAL_FILE).is_file() {
            return true;
        }
        if !cursor.pop() {
            return false;
        }
    }
}

fn add_relevant_peripherals(
    facts: &mut Vec<GoalFact>,
    source_refs: &mut Vec<GoalSourceRef>,
    board: &BoardRecord,
    goal_route: &GoalRoute,
    prompt: &str,
    root: &Path,
) -> Result<(), String> {
    let requested = requested_peripherals(goal_route, prompt);
    for peripheral in &board.peripheral_matrix {
        let normalized = normalized_peripheral(peripheral);
        if !requested.contains(normalized) {
            continue;
        }
        add_fact(facts, "peripheral", normalized, &peripheral.source_url);
        add_fact(facts, "chip", &peripheral.chip, &peripheral.source_url);
        add_fact(facts, "bus", &peripheral.bus, &peripheral.source_url);
        add_fact(facts, "driver", &peripheral.driver, &peripheral.source_url);
        source_refs.push(source_ref(
            "lilygo-hardware",
            source_authority_rank("lilygo-hardware"),
            &peripheral.source_url,
            &peripheral.source_status,
            false,
        ));
    }
    let packs = load_source_pack_index(root)?;
    for pack in packs
        .packs
        .iter()
        .filter(|pack| pack.board_id == board.id)
        .filter(|pack| requested.contains(pack.peripheral.as_str()))
    {
        for source in &pack.sources {
            source_refs.push(GoalSourceRef {
                kind: source.kind.clone(),
                authority_rank: source.authority_rank,
                url: source.url.clone(),
                status: source.status.clone(),
                stale: source.stale,
                evidence_level: source.evidence_level.clone(),
            });
        }
    }
    Ok(())
}

fn requested_peripherals(route: &GoalRoute, prompt: &str) -> BTreeSet<&'static str> {
    let normalized = prompt.to_lowercase();
    let mut requested = BTreeSet::new();
    for skill in &route.peripherals {
        match skill.as_str() {
            "periph-imu" => {
                requested.insert("imu");
            }
            "periph-display" => {
                requested.insert("display");
                requested.insert("touch");
            }
            "periph-input" => {
                requested.insert("input");
                requested.insert("touch");
            }
            "periph-power" => {
                requested.insert("power");
            }
            "periph-storage" => {
                requested.insert("storage");
                requested.insert("memory");
            }
            "periph-lora" => {
                requested.insert("lora");
            }
            "periph-gps" => {
                requested.insert("gnss");
            }
            _ => {}
        }
    }
    if route.chips.iter().any(|chip| chip == "chip-bhi260ap")
        || contains_any(&normalized, &["imu", "bhi260ap", "gesture", "抬腕"])
    {
        requested.insert("imu");
    }
    if route.chips.iter().any(|chip| chip == "chip-st25r3916")
        || contains_any(&normalized, &["nfc", "st25r3916"])
    {
        requested.insert("nfc");
    }
    if contains_any(&normalized, &["lvgl", "touch", "display", "screen"]) {
        requested.insert("display");
        requested.insert("touch");
        requested.insert("power");
    }
    if contains_any(&normalized, &["ota", "flash", "partition", "manifest"]) {
        requested.insert("memory");
        requested.insert("storage");
    }
    // A prompt that names the power/PMIC or haptic subsystem should surface that
    // peripheral's exact chip, bus, and driver -- otherwise "which power chip?"
    // returns only the expand pointer while the AXP2101/DRV2605 fact stays hidden.
    if contains_any(
        &normalized,
        &[
            "power", "pmic", "axp", "battery", "charge", "电源", "电池", "充电",
        ],
    ) {
        requested.insert("power");
    }
    if contains_any(
        &normalized,
        &[
            "haptic", "vibrat", "motor", "drv2605", "震动", "马达", "振动",
        ],
    ) {
        requested.insert("haptic");
    }
    if route.chips.iter().any(|chip| chip == "chip-xl9555")
        || contains_any(
            &normalized,
            &["xl9555", "gpio", "io", "pinout", "引脚", "外设"],
        )
    {
        requested.insert("input");
    }
    requested
}

fn add_board_sources(source_refs: &mut Vec<GoalSourceRef>, board: &BoardRecord) {
    for source in &board.source_urls {
        source_refs.push(source_ref_from_board(source));
    }
}

fn add_documentation_repo(source_refs: &mut Vec<GoalSourceRef>) {
    // Search target, ranked below board headers, hardware docs, and examples.
    source_refs.push(source_ref(
        "documentation-repo",
        65,
        DOCUMENTATION_REPO,
        "versioned-wiki-source",
        false,
    ));
}

fn source_ref_from_board(source: &SourceUrl) -> GoalSourceRef {
    source_ref(
        source.kind.as_str(),
        board_source_rank(source.kind.as_str()),
        source.url.as_str(),
        source.status.as_str(),
        false,
    )
}

fn source_ref(
    kind: &str,
    authority_rank: u32,
    url: &str,
    status: &str,
    stale: bool,
) -> GoalSourceRef {
    GoalSourceRef {
        kind: kind.to_string(),
        authority_rank,
        url: url.to_string(),
        status: status.to_string(),
        stale,
        evidence_level: "V3-source-reference".to_string(),
    }
}

fn board_source_rank(kind: &str) -> u32 {
    match kind {
        "driver-header" | "arduino-pins" => 95,
        "hardware-doc" | "quick-start" | "github-repo" => 90,
        "wiki" => 55,
        _ => 50,
    }
}

fn dedup_sources(source_refs: &mut Vec<GoalSourceRef>) {
    // Authority rank decides which refs survive compact context caps.
    source_refs.sort_by(|left, right| {
        right
            .authority_rank
            .cmp(&left.authority_rank)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.url.cmp(&right.url))
    });
    let mut seen = BTreeSet::new();
    source_refs.retain(|source| seen.insert((source.kind.clone(), source.url.clone())));
}

fn add_fact_table_sources(source_refs: &mut Vec<GoalSourceRef>, tables: &[FactTablePreview]) {
    for source in tables
        .iter()
        .flat_map(|table| table.rows.iter().map(|fact| &fact.source))
    {
        source_refs.push(goal_source_ref_from_fact_source(source));
    }
}

fn readiness_discovery_hint(signal: &CompletenessSignal) -> Option<DiscoveryHint> {
    if signal.completeness == "complete" {
        return None;
    }
    Some(DiscoveryHint {
        when: format!(
            "{} {} completeness is {}",
            signal.board_id, signal.topic, signal.completeness
        ),
        action: "run_command".to_string(),
        command: signal
            .update_command
            .clone()
            .or_else(|| Some(signal.source_query_command.clone())),
        reference_id: None,
        reason: "Resolve topic readiness before claiming quick-start implementation details."
            .to_string(),
    })
}

fn cap_source_refs(source_refs: &mut Vec<GoalSourceRef>, max_inline: usize) {
    if source_refs.len() <= max_inline {
        return;
    }
    // Preserve one documentation repo pointer as the next-search breadcrumb.
    let documentation_repo = source_refs
        .iter()
        .find(|source| source.url == DOCUMENTATION_REPO)
        .cloned();
    source_refs.truncate(max_inline);
    if let Some(documentation_repo) = documentation_repo
        && !source_refs
            .iter()
            .any(|source| source.url == DOCUMENTATION_REPO)
        && !source_refs.is_empty()
    {
        source_refs.pop();
        source_refs.push(documentation_repo);
    }
}

fn implementation_start_for_goal(
    prompt: &str,
    demo_refs: &[GoalDemoRef],
    source_refs: &[GoalSourceRef],
    fact_tables: &[FactTablePreview],
) -> Option<GoalImplementationStart> {
    if !is_source_recovery_prompt(prompt, fact_tables) {
        return None;
    }
    let official_demo = demo_refs.first();
    let source_headers = source_refs
        .iter()
        .filter(|source| {
            matches!(
                source.kind.as_str(),
                "driver-header" | "arduino-pins" | "official-code" | "hardware-doc"
            )
        })
        .map(|source| source.url.clone())
        .take(4)
        .collect::<Vec<_>>();
    Some(GoalImplementationStart {
        strategy: "official-demo-first".to_string(),
        reason: "Start from the closest official example and verify board pins in source headers before writing custom code.".to_string(),
        official_demo_path: official_demo.map(|demo| demo.path.clone()),
        official_demo_url: official_demo.map(|demo| demo.source_url.clone()),
        source_headers,
        next_steps: vec![
            "read the official demo path first".to_string(),
            "check driver headers and pin_config before assigning GPIO".to_string(),
            "query source facts again when a pin, bus, or driver is missing".to_string(),
        ],
    })
}

fn critical_facts_for_goal(
    prompt: &str,
    fact_tables: &[FactTablePreview],
) -> Vec<GoalCriticalFact> {
    if !is_source_recovery_prompt(prompt, fact_tables) {
        return Vec::new();
    }
    let mut critical = Vec::new();
    let mut seen = BTreeSet::new();
    for fact in fact_tables.iter().flat_map(|table| table.rows.iter()) {
        if !is_critical_source_fact(fact) {
            continue;
        }
        push_critical_fact(
            &mut critical,
            &mut seen,
            &fact.key,
            &fact.value,
            &fact.source.path_or_url,
            &fact.evidence_level,
        );
    }
    critical.truncate(8);
    critical
}

fn recovery_actions_for_goal(
    board_id: &str,
    prompt: &str,
    fact_tables: &[FactTablePreview],
) -> Vec<GoalRecoveryAction> {
    if !is_source_recovery_prompt(prompt, fact_tables) {
        return Vec::new();
    }
    vec![
        recovery_action(
            "source-query",
            format!("lilygo-skills source query --board {board_id} --topic io --json"),
            "Expand exact board pins, buses, connectors, and known unknowns before assigning GPIO.",
        ),
        recovery_action(
            "playbook",
            "lilygo-skills index query playbook-source-discovery --json",
            "Load the source-discovery playbook when official facts are absent or ambiguous.",
        ),
    ]
}

fn internal_skill_hints_for_goal(
    prompt: &str,
    playbook_hints: &[PlaybookHint],
) -> Vec<GoalInternalSkillHint> {
    if !is_source_recovery_prompt(prompt, &[]) {
        return Vec::new();
    }
    let mut hints = playbook_hints
        .iter()
        .map(|hint| GoalInternalSkillHint {
            skill_id: hint.playbook_id.clone(),
            kind: "playbook".to_string(),
            expand_command: hint.expand_command.clone(),
            reason: hint.reason.clone(),
        })
        .collect::<Vec<_>>();
    if !hints
        .iter()
        .any(|hint| hint.skill_id == "playbook-source-discovery")
    {
        hints.push(GoalInternalSkillHint {
            skill_id: "playbook-source-discovery".to_string(),
            kind: "playbook".to_string(),
            expand_command: "lilygo-skills index query playbook-source-discovery --json"
                .to_string(),
            reason: "Use the source model before guessing pins, drivers, demos, or setup files."
                .to_string(),
        });
    }
    hints.truncate(5);
    hints
}

fn is_source_recovery_prompt(prompt: &str, fact_tables: &[FactTablePreview]) -> bool {
    let has_source_facts = !fact_tables.is_empty();
    let implementation_or_debug = crate::facts::is_implementation_or_debug_prompt(prompt);
    if has_source_facts && crate::facts::is_fact_prompt(prompt) && !implementation_or_debug {
        return false;
    }
    implementation_or_debug
}

fn is_critical_source_fact(fact: &SourceFact) -> bool {
    fact.confidence != "unknown_with_sources"
        && fact.value != "unknown_with_sources"
        && is_critical_text(&fact.key, &fact.value)
}

fn is_critical_text(key: &str, value: &str) -> bool {
    let key_lower = key.to_lowercase();
    // Structural rule (additive, board-class agnostic): concrete pin/bus/
    // peripheral assignments are critical, so radio (LoRa/GNSS), IMU, UART and
    // SPI boards get their critical pins too, not just display boards.
    if ["pin.", "bus.", "display.", "expander.", "connector."]
        .iter()
        .any(|prefix| key_lower.starts_with(prefix))
    {
        return true;
    }
    // Any fact whose value names a concrete GPIO assignment.
    if value.to_lowercase().contains("gpio") {
        return true;
    }
    // Union with the prior keyword coverage (extended beyond display) so no
    // board loses critical facts it already surfaced. Measured regression
    // guard: replacing the keywords outright dropped coverage 14->3 boards.
    let haystack = format!("{key} {value}").to_lowercase();
    contains_any(
        &haystack,
        &[
            "pin_iic", "iic", "i2c", "spi", "uart", "i2s", "lora", "gnss", "gps", "imu", "radio",
            "sx126", "sx127", "sx128", "display", "tft", "lcd", "amoled", "epaper", "e-paper",
            "touch", "button", "bat_volt", "sd_", "pmic", "axp",
        ],
    )
}

fn push_critical_fact(
    facts: &mut Vec<GoalCriticalFact>,
    seen: &mut BTreeSet<(String, String)>,
    key: &str,
    value: &str,
    source: &str,
    evidence_level: &str,
) {
    if seen.insert((key.to_string(), value.to_string())) {
        facts.push(GoalCriticalFact {
            key: key.to_string(),
            value: value.to_string(),
            source: source.to_string(),
            evidence_level: evidence_level.to_string(),
        });
    }
}

fn recovery_action(kind: &str, command: impl Into<String>, reason: &str) -> GoalRecoveryAction {
    GoalRecoveryAction {
        kind: kind.to_string(),
        command: command.into(),
        reason: reason.to_string(),
    }
}

fn goal_source_ref_from_fact_source(source: &SourceFactSource) -> GoalSourceRef {
    GoalSourceRef {
        kind: source.kind.clone(),
        authority_rank: board_source_rank(&source.kind),
        url: source.path_or_url.clone(),
        status: source.hash.clone(),
        stale: false,
        evidence_level: "V3-source-reference".to_string(),
    }
}

fn add_fact(facts: &mut Vec<GoalFact>, key: &str, value: &str, source: &str) {
    if value.is_empty() {
        return;
    }
    let exists = facts
        .iter()
        .any(|fact| fact.key == key && fact.value == value && fact.source == source);
    if !exists {
        facts.push(GoalFact {
            key: key.to_string(),
            value: value.to_string(),
            source: source.to_string(),
            evidence_level: "V3-source-reference".to_string(),
        });
    }
}

fn normalized_peripheral(peripheral: &PeripheralRecord) -> &'static str {
    let chip = peripheral.chip.to_lowercase();
    let name = peripheral.name.to_lowercase();
    if chip.contains("bhi260ap") || name.contains("imu") {
        "imu"
    } else if chip.contains("st25r3916") || peripheral.category == "nfc" {
        "nfc"
    } else if peripheral.category == "radio" {
        "lora"
    } else if peripheral.category == "gnss" {
        "gnss"
    } else if peripheral.category == "io" {
        "input"
    } else if peripheral.category == "touch" {
        "touch"
    } else if peripheral.category == "display" {
        "display"
    } else if peripheral.category == "memory" {
        "memory"
    } else if peripheral.category == "storage" {
        "storage"
    } else if peripheral.category == "power" {
        "power"
    } else if peripheral.category == "haptic" {
        "haptic"
    } else {
        "other"
    }
}

fn boundary(route: &RouteResult, reason: &str) -> GoalBoundary {
    GoalBoundary {
        verification_level: if route.decision == "inject" {
            "V3".to_string()
        } else {
            "none".to_string()
        },
        hardware_verified: false,
        reason: reason.to_string(),
    }
}

fn context_summary(route: &RouteResult, goal_route: &GoalRoute) -> String {
    if route.decision != "inject" {
        return route.notes.join(" ");
    }
    format!(
        "Goal capsule for board={}, framework={}, skills=[{}]",
        goal_route.board.as_deref().unwrap_or("unknown"),
        goal_route.framework.as_deref().unwrap_or("unspecified"),
        goal_route.skills.join(",")
    )
}

pub(super) fn primary_framework(skill: &str) -> bool {
    matches!(
        skill,
        "fw-arduino" | "fw-esp-idf" | "fw-platformio" | "fw-rust"
    )
}

#[cfg(test)]
mod critical_text_tests {
    use super::is_critical_text;

    #[test]
    fn structural_rule_covers_non_display_peripherals() {
        // Radio / GNSS / IMU / UART / SPI pins are critical, not just display.
        assert!(is_critical_text("pin.lora.sck", "LORA_SCK=GPIO5"));
        assert!(is_critical_text("pin.gnss.rx", "GPS_RX=GPIO34"));
        assert!(is_critical_text("pin.imu.int", "BHI_INT=GPIO37"));
        assert!(is_critical_text("bus.spi.radio", "SX1262 on SPI"));
        // Display facts stay critical (no regression vs the old allowlist).
        assert!(is_critical_text("pin.i2c.sda", "PIN_IIC_SDA=GPIO18"));
        assert!(is_critical_text(
            "display.panel_or_chip",
            "ST7789 170x320 TFT"
        ));
        // Value-level GPIO fallback still catches unprefixed assignments.
        assert!(is_critical_text("backlight", "PIN_LCD_BL=GPIO38"));
        // Non-pin metadata is not critical.
        assert!(!is_critical_text("mcu.family", "esp32-s3"));
        assert!(!is_critical_text(
            "frameworks.supported",
            "arduino,platformio"
        ));
    }
}
