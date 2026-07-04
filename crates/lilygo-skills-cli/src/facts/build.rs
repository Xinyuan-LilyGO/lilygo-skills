//! Builds source-backed fact packs from board records while preserving
//! `unknown_with_sources` for unproven pin and peripheral details.
use super::*;

pub(crate) fn fact_pack_from_board(board: &BoardRecord) -> BoardFactPack {
    let source_refs = board_sources(board);
    let mut pin_matrix = Vec::new();
    let mut bus_matrix = Vec::new();
    let mut expander_matrix = Vec::new();
    let mut connector_matrix = Vec::new();
    let mut peripheral_table = Vec::new();

    if board.supported && board.mcu.to_lowercase().contains("esp32") {
        let source = best_board_source(board, "arduino-pins")
            .or_else(|| best_board_source(board, "driver-header"))
            .unwrap_or_else(|| source_refs[0].clone());
        pin_matrix.push(fact(
            board,
            "pinout",
            "mcu.family",
            &board.mcu,
            "MCU family for runnable support boundary",
            source.clone(),
            "exact",
        ));
        pin_matrix.push(fact(
            board,
            "pinout",
            "frameworks.supported",
            &board.frameworks.join(","),
            "Frameworks recorded for this board",
            source.clone(),
            "derived",
        ));
        pin_matrix.push(fact(
            board,
            "pinout",
            "gpio.free",
            "unknown_with_sources",
            "Free GPIO cannot be inferred without complete official pin assignment proof",
            source.clone(),
            "unknown_with_sources",
        ));
    }

    for peripheral in &board.peripheral_matrix {
        let source = peripheral_source(peripheral);
        bus_matrix.push(peripheral_fact(
            board,
            peripheral,
            "bus",
            &format!("bus.{}.{}", slug(&peripheral.bus), slug(&peripheral.chip)),
            &format!("{} uses {}", peripheral.chip, peripheral.bus),
            "Peripheral bus assignment from board source",
            source.clone(),
            "exact",
        ));
        peripheral_table.push(peripheral_fact(
            board,
            peripheral,
            "peripheral",
            &format!(
                "peripheral.{}.{}",
                peripheral.category,
                slug(&peripheral.chip)
            ),
            &format!(
                "{} | {} | {} | {}",
                peripheral.name, peripheral.chip, peripheral.bus, peripheral.driver
            ),
            "Board peripheral chip, bus, and driver record",
            source.clone(),
            "exact",
        ));
        if is_expander(peripheral) {
            expander_matrix.push(peripheral_fact(
                board,
                peripheral,
                "expander",
                "expander.xl9555.bus",
                &peripheral.bus,
                "XL9555 expander bus address",
                source.clone(),
                "exact",
            ));
            expander_matrix.push(peripheral_fact(
                board,
                peripheral,
                "expander",
                "expander.xl9555.channel-map",
                "unknown_with_sources",
                "Exact XL9555 channel-to-function mapping is not proven by the current source cache",
                source.clone(),
                "unknown_with_sources",
            ));
        }
        if is_connector(peripheral) {
            connector_matrix.push(peripheral_fact(
                board,
                peripheral,
                "connector",
                &format!("connector.{}", slug(&peripheral.name)),
                &format!("{} uses {}", peripheral.name, peripheral.bus),
                "Board connector or socket integration record",
                source,
                "exact",
            ));
        }
    }

    BoardFactPack {
        schema_version: 1,
        board_id: board.id.clone(),
        mcu_family: board.mcu.clone(),
        supported: is_supported_esp32(board),
        pin_matrix,
        bus_matrix,
        expander_matrix,
        connector_matrix,
        peripheral_table,
        source_refs,
        conflicts: Vec::new(),
    }
}

pub(crate) fn board_sources(board: &BoardRecord) -> Vec<SourceFactSource> {
    let mut sources = board
        .source_urls
        .iter()
        .map(source_from_board_url)
        .collect::<Vec<_>>();
    sources.push(source_ref("documentation-repo", DOCUMENTATION_REPO));
    sources.sort_by(|left, right| {
        source_authority_rank(&right.kind)
            .cmp(&source_authority_rank(&left.kind))
            .then_with(|| left.path_or_url.cmp(&right.path_or_url))
    });
    sources
        .dedup_by(|left, right| left.kind == right.kind && left.path_or_url == right.path_or_url);
    sources
}

pub(crate) fn source_from_board_url(source: &SourceUrl) -> SourceFactSource {
    source_ref(&source.kind, &source.url)
}

pub(crate) fn best_board_source(board: &BoardRecord, kind: &str) -> Option<SourceFactSource> {
    board
        .source_urls
        .iter()
        .find(|source| source.kind == kind)
        .map(source_from_board_url)
}

pub(crate) fn peripheral_source(peripheral: &PeripheralRecord) -> SourceFactSource {
    let kind = if peripheral.source_url.contains("/src/") {
        "driver-header"
    } else if peripheral.source_url.contains("/docs/hardware/") {
        "hardware-doc"
    } else {
        "official-code"
    };
    source_ref(kind, &peripheral.source_url)
}

pub(crate) fn source_ref(kind: &str, path_or_url: &str) -> SourceFactSource {
    SourceFactSource {
        kind: kind.to_string(),
        path_or_url: path_or_url.to_string(),
        line_range: None,
        hash: format!("sha256:{}", stable_hash(&(kind, path_or_url))),
    }
}

pub(crate) fn fact(
    board: &BoardRecord,
    topic: &str,
    key: &str,
    value: &str,
    claim: &str,
    source: SourceFactSource,
    confidence: &str,
) -> SourceFact {
    SourceFact {
        schema_version: 1,
        board_id: board.id.clone(),
        topic: topic.to_string(),
        key: key.to_string(),
        value: value.to_string(),
        claim: claim.to_string(),
        authority_rank: source_authority_rank(&source.kind),
        evidence_level: "V3-source-reference".to_string(),
        stale: false,
        confidence: confidence.to_string(),
        source,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn peripheral_fact(
    board: &BoardRecord,
    _peripheral: &PeripheralRecord,
    topic: &str,
    key: &str,
    value: &str,
    claim: &str,
    source: SourceFactSource,
    confidence: &str,
) -> SourceFact {
    fact(board, topic, key, value, claim, source, confidence)
}

pub(crate) fn facts_for_topic(pack: &BoardFactPack, topic: &str) -> Vec<SourceFact> {
    let mut facts = match topic {
        "io" => [
            pack.pin_matrix.as_slice(),
            pack.bus_matrix.as_slice(),
            pack.expander_matrix.as_slice(),
            pack.connector_matrix.as_slice(),
            pack.peripheral_table.as_slice(),
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect(),
        "pinout" => pack.pin_matrix.clone(),
        "bus" => pack.bus_matrix.clone(),
        "expander" => pack.expander_matrix.clone(),
        "connector" => pack.connector_matrix.clone(),
        "peripheral" => pack.peripheral_table.clone(),
        "display" => topic_facts(pack, &["display", "touch"]),
        "imu" => topic_facts(pack, &["imu", "sensor", "bhi260ap"]),
        "power" => topic_facts(pack, &["power", "pmu", "axp"]),
        "lora" => topic_facts(
            pack,
            &[
                "lora", "radio", "sx1262", "sx1268", "sx1276", "sx1278", "sx1280",
            ],
        ),
        "gnss" => topic_facts(pack, &["gnss", "gps", "mia-m10"]),
        "input" => topic_facts(pack, &["input", "keyboard", "button", "xl9555"]),
        _ => Vec::new(),
    };
    facts.sort_by(|left, right| {
        right
            .authority_rank
            .cmp(&left.authority_rank)
            .then_with(|| left.key.cmp(&right.key))
    });
    facts
}

pub(crate) fn topic_facts(pack: &BoardFactPack, needles: &[&str]) -> Vec<SourceFact> {
    pack.peripheral_table
        .iter()
        .chain(pack.bus_matrix.iter())
        .chain(pack.expander_matrix.iter())
        .filter(|fact| {
            let value = format!("{} {} {}", fact.topic, fact.key, fact.value).to_lowercase();
            needles.iter().any(|needle| value.contains(needle))
        })
        .cloned()
        .collect()
}

pub(crate) fn table_preview(
    board_id: &str,
    table: &str,
    rows: &[SourceFact],
    topic: &str,
    budget: &ContextBudget,
) -> FactTablePreview {
    let rows_preview = rows
        .iter()
        .take(budget.max_fact_rows_per_table)
        .cloned()
        .collect::<Vec<_>>();
    FactTablePreview {
        table: table.to_string(),
        preview_count: rows_preview.len(),
        overflow_count: rows.len().saturating_sub(rows_preview.len()),
        rows: rows_preview,
        query_command: format!(
            "lilygo-skills source query --board {board_id} --topic {topic} --json"
        ),
    }
}

pub(crate) fn normalize_topic(topic: &str) -> Result<&'static str, String> {
    match topic {
        "io" | "gpio" => Ok("io"),
        "pinout" | "pin" | "pins" => Ok("pinout"),
        "bus" | "i2c" | "spi" | "uart" => Ok("bus"),
        "expander" | "xl9555" => Ok("expander"),
        "connector" | "socket" => Ok("connector"),
        "peripheral" | "peripherals" => Ok("peripheral"),
        "display" | "lvgl" | "screen" | "lcd" | "amoled" => Ok("display"),
        "imu" | "sensor" | "gesture" => Ok("imu"),
        "power" | "pmu" | "battery" => Ok("power"),
        "lora" | "radio" => Ok("lora"),
        "gnss" | "gps" => Ok("gnss"),
        "input" | "keyboard" | "button" | "touch" => Ok("input"),
        other => Err(format!("unsupported source topic: {other}")),
    }
}

pub(crate) fn normalize_completeness_topic(topic: &str) -> Result<&'static str, String> {
    normalize_topic(topic)
}

pub(crate) fn is_readiness_topic(topic: &str) -> bool {
    matches!(
        topic,
        "display" | "imu" | "power" | "lora" | "gnss" | "input"
    )
}

pub(crate) fn topics_for_prompt(prompt: &str) -> Vec<String> {
    let normalized = prompt.to_lowercase();
    let keywords = prompt_keywords();
    let mut topics: Vec<String> = keywords
        .topic_order
        .iter()
        .filter(|topic| {
            keywords
                .topics
                .get(*topic)
                .is_some_and(|needles| needles.iter().any(|needle| normalized.contains(needle)))
        })
        .cloned()
        .collect();
    topics.sort();
    topics.dedup();
    topics.truncate(ContextBudget::default().max_discovery_hints_inline);
    topics
}

pub(crate) fn demo_matches_topic(demo: &crate::model::DemoRef, topic: &str) -> bool {
    let target = format!("{} {}", demo.target, demo.path).to_lowercase();
    match topic {
        "display" => contains_any(&target, &["display", "lvgl", "screen", "factory"]),
        "imu" => contains_any(&target, &["imu", "bhi260", "sensor", "factory"]),
        "power" => contains_any(&target, &["power", "battery", "factory"]),
        "lora" => contains_any(
            &target,
            &[
                "lora", "radio", "sx1262", "sx1268", "sx1276", "sx1278", "sx1280", "factory",
            ],
        ),
        "gnss" => contains_any(&target, &["gnss", "gps", "mia-m10", "factory"]),
        "input" => contains_any(
            &target,
            &["input", "keyboard", "button", "touch", "factory"],
        ),
        _ => contains_any(&target, &[topic, "factory"]),
    }
}

pub(crate) fn is_known_fact(fact: &SourceFact) -> bool {
    fact.confidence != "unknown_with_sources" && fact.value != "unknown_with_sources"
}

pub(crate) fn discovery_hints(
    board_id: &str,
    topic: &str,
    include_unknown_hint: bool,
) -> Vec<DiscoveryHint> {
    let mut hints = vec![DiscoveryHint {
        when: "need source-backed board facts before writing firmware".to_string(),
        action: "run_command".to_string(),
        command: Some(format!(
            "lilygo-skills source query --board {board_id} --topic {topic} --json"
        )),
        reference_id: None,
        reason: "Fetch the full fact pack on demand instead of inlining every table.".to_string(),
    }];
    if include_unknown_hint {
        hints.push(DiscoveryHint {
            when: "a fact is unknown or ambiguous".to_string(),
            action: "run_command".to_string(),
            command: Some(format!(
                "lilygo-skills source query --board {board_id} --topic expander --json"
            )),
            reference_id: None,
            reason: "Check the expander table and source refs before assigning XL9555 channels."
                .to_string(),
        });
    }
    hints.truncate(ContextBudget::default().max_discovery_hints_inline);
    hints
}

pub(crate) fn query_warnings(pack: &BoardFactPack) -> Vec<String> {
    if !pack.supported {
        return vec![
            "unsupported LilyGO product boundary: runnable guidance is limited to ESP32-family boards".to_string(),
        ];
    }
    vec![
        "source query returns V3 source/context evidence, not a successful firmware run".to_string(),
        "unknown_with_sources means the current source cache has pointers but no exact actionable value".to_string(),
    ]
}

pub(crate) fn stale_fact_packs(root: &Path, index: &BoardFactPackIndex) -> Vec<String> {
    let path = root.join(FACT_PACK_INDEX_PATH);
    let Ok(existing) = fs::read_to_string(path) else {
        return index
            .packs
            .iter()
            .map(|pack| pack.board_id.clone())
            .collect();
    };
    let Ok(previous) = serde_json::from_str::<BoardFactPackIndex>(&existing) else {
        return index
            .packs
            .iter()
            .map(|pack| pack.board_id.clone())
            .collect();
    };
    let previous_hashes = previous
        .packs
        .iter()
        .map(|pack| (pack.board_id.as_str(), stable_hash(pack)))
        .collect::<BTreeMap<_, _>>();
    index
        .packs
        .iter()
        .filter(|pack| previous_hashes.get(pack.board_id.as_str()) != Some(&stable_hash(pack)))
        .map(|pack| pack.board_id.clone())
        .collect()
}

pub(crate) fn is_expander(peripheral: &PeripheralRecord) -> bool {
    peripheral.category == "io" || peripheral.chip.to_lowercase().contains("xl9555")
}

pub(crate) fn is_connector(peripheral: &PeripheralRecord) -> bool {
    matches!(peripheral.category.as_str(), "storage" | "radio" | "gnss")
}

pub(crate) fn is_embedded_fact_or_impl_prompt(prompt: &str) -> bool {
    is_fact_prompt(prompt) || is_implementation_or_debug_prompt(prompt)
}

pub(crate) fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

pub(crate) fn slug(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub(crate) fn stable_hash(value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
