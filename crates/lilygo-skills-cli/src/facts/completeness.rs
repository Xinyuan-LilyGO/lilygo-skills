//! Topic-readiness evaluation for board facts; references alone return
//! ingestion actions until required source-backed facts are present.
use super::*;

pub(crate) fn unsupported_completeness(board_id: &str, topic: &str) -> CompletenessReport {
    CompletenessReport {
        schema_version: 1,
        status: "PASS".to_string(),
        board_id: board_id.to_string(),
        topic: topic.to_string(),
        completeness: "unsupported".to_string(),
        required_present: Vec::new(),
        required_missing: required_fields(topic),
        preferred_present: Vec::new(),
        preferred_missing: preferred_fields(topic),
        facts: Vec::new(),
        source_refs: Vec::new(),
        next_actions: Vec::new(),
        evidence_level: "V3".to_string(),
        warnings: vec![
            "unsupported LilyGO product boundary: runnable guidance is limited to ESP32-family boards"
                .to_string(),
        ],
    }
}

pub(crate) fn evaluate_completeness(
    board: &BoardRecord,
    pack: &BoardFactPack,
    topic: &str,
) -> CompletenessReport {
    if !pack.supported {
        return unsupported_completeness(&board.id, topic);
    }
    let facts = completeness_facts(board, pack, topic);
    let required = required_fields(topic);
    let preferred = preferred_fields(topic);
    let present = present_fields(board, pack, topic, &facts);
    let required_present = intersection(&required, &present);
    let required_missing = difference(&required, &present);
    let preferred_present = intersection(&preferred, &present);
    let preferred_missing = difference(&preferred, &present);
    let completeness = completeness_status(&required_missing, &pack.source_refs);
    CompletenessReport {
        schema_version: 1,
        status: "PASS".to_string(),
        board_id: board.id.clone(),
        topic: topic.to_string(),
        completeness: completeness.clone(),
        required_present,
        required_missing: required_missing.clone(),
        preferred_present,
        preferred_missing,
        facts,
        source_refs: pack.source_refs.clone(),
        next_actions: next_actions(&board.id, topic, &completeness, &required_missing),
        evidence_level: "V3".to_string(),
        warnings: completeness_warnings(topic, &completeness),
    }
}

pub(crate) fn is_supported_esp32(board: &BoardRecord) -> bool {
    board.supported && board.mcu.to_lowercase().contains("esp32")
}

pub(crate) fn completeness_status(
    required_missing: &[String],
    sources: &[SourceFactSource],
) -> String {
    if required_missing.is_empty() {
        return "complete".to_string();
    }
    if !sources.is_empty() {
        return "needs_source_ingestion".to_string();
    }
    "partial".to_string()
}

pub(crate) fn completeness_signal(
    root: &Path,
    board_id: &str,
    topic: &str,
) -> Result<CompletenessSignal, String> {
    let report = source_completeness(root, board_id, topic)?;
    Ok(signal_from_report(&report))
}

pub(crate) fn signal_from_report(report: &CompletenessReport) -> CompletenessSignal {
    CompletenessSignal {
        board_id: report.board_id.clone(),
        topic: report.topic.clone(),
        completeness: report.completeness.clone(),
        evidence_level: report.evidence_level.clone(),
        source_query_command: format!(
            "lilygo-skills source query --board {} --topic {} --json",
            report.board_id, report.topic
        ),
        update_command: report
            .next_actions
            .iter()
            .find(|action| action.command.contains("update board-facts"))
            .map(|action| action.command.clone()),
        required_missing: report.required_missing.clone(),
    }
}

pub(crate) fn next_actions(
    board_id: &str,
    topic: &str,
    completeness: &str,
    missing: &[String],
) -> Vec<CompletenessNextAction> {
    let mut actions = Vec::new();
    if completeness == "needs_source_ingestion" {
        actions.push(CompletenessNextAction {
            kind: "run_command".to_string(),
            command: format!(
                "lilygo-skills update board-facts --board {board_id} --topic {topic} --dry-run --json"
            ),
            reason: format!(
                "Parse official refs before quick-start guidance; missing {}.",
                missing.join(",")
            ),
        });
    }
    actions.push(CompletenessNextAction {
        kind: "run_command".to_string(),
        command: format!("lilygo-skills source query --board {board_id} --topic {topic} --json"),
        reason: "Inspect compact source facts and source refs before implementation.".to_string(),
    });
    actions
}

// Topic completeness rules are data-owned so readiness requirements stay
// reviewable next to the source-fact model.
const TOPIC_FIELDS_JSON: &str = include_str!("../../../../data/facts/topic-fields.json");

#[derive(serde::Deserialize)]
struct TopicFieldsFile {
    generic_required: Vec<String>,
    generic_preferred: Vec<String>,
    required: std::collections::BTreeMap<String, Vec<String>>,
    preferred: std::collections::BTreeMap<String, Vec<String>>,
}

fn topic_fields() -> TopicFieldsFile {
    serde_json::from_str(TOPIC_FIELDS_JSON)
        .expect("embedded data/facts/topic-fields.json must be valid")
}

fn expand(template: &[String], topic: &str) -> Vec<String> {
    template
        .iter()
        .map(|field| field.replace("{topic}", topic))
        .collect()
}

pub(crate) fn required_fields(topic: &str) -> Vec<String> {
    let rules = topic_fields();
    rules
        .required
        .get(topic)
        .cloned()
        .unwrap_or_else(|| expand(&rules.generic_required, topic))
}

pub(crate) fn preferred_fields(topic: &str) -> Vec<String> {
    let rules = topic_fields();
    rules
        .preferred
        .get(topic)
        .cloned()
        .unwrap_or_else(|| expand(&rules.generic_preferred, topic))
}

pub(crate) fn present_fields(
    board: &BoardRecord,
    pack: &BoardFactPack,
    topic: &str,
    facts: &[SourceFact],
) -> BTreeSet<String> {
    let mut present = BTreeSet::new();
    if !pack.source_refs.is_empty() {
        present.insert("source_refs".to_string());
    }
    add_demo_and_hint_fields(board, topic, facts, &mut present);
    if topic == "display" {
        add_display_fields(board, facts, &mut present);
    } else {
        add_generic_topic_fields(topic, facts, &mut present);
    }
    present
}

pub(crate) fn add_demo_and_hint_fields(
    board: &BoardRecord,
    topic: &str,
    facts: &[SourceFact],
    present: &mut BTreeSet<String>,
) {
    let has_demo = board
        .demo_refs
        .iter()
        .any(|demo| demo_matches_topic(demo, topic));
    let fact_has_demo = facts
        .iter()
        .any(|fact| fact.key == "framework.demo_refs" && is_known_fact(fact));
    let fact_has_build_hint = facts
        .iter()
        .any(|fact| fact.key == "framework.build_hint" && is_known_fact(fact));
    if has_demo || fact_has_demo {
        present.insert("framework.demo_refs".to_string());
    }
    if (has_demo && !board.frameworks.is_empty()) || fact_has_build_hint {
        present.insert("framework.build_hint".to_string());
    }
    if topic == "display" && !board.source_urls.is_empty() {
        present.insert("debug.blank_screen_hints".to_string());
    }
}

pub(crate) fn add_display_fields(
    board: &BoardRecord,
    facts: &[SourceFact],
    present: &mut BTreeSet<String>,
) {
    let display = board
        .peripheral_matrix
        .iter()
        .find(|peripheral| peripheral.category == "display");
    if let Some(display) = display {
        if !display.chip.is_empty() {
            present.insert("display.panel_or_chip".to_string());
        }
        if !display.bus.is_empty() {
            present.insert("display.bus_or_interface".to_string());
        }
    }
    if board
        .peripheral_matrix
        .iter()
        .any(|peripheral| peripheral.category == "power")
        || board
            .demo_refs
            .iter()
            .any(|demo| demo.target.to_lowercase().contains("brightness"))
    {
        present.insert("display.backlight_or_power".to_string());
    }
    if board
        .peripheral_matrix
        .iter()
        .any(|peripheral| peripheral.category == "touch")
    {
        present.insert("display.touch".to_string());
    }
    for fact in facts.iter().filter(|fact| is_known_fact(fact)) {
        match fact.key.as_str() {
            "display.panel_or_chip" => {
                present.insert("display.panel_or_chip".to_string());
            }
            "display.bus_or_interface" => {
                present.insert("display.bus_or_interface".to_string());
            }
            "display.backlight_or_power" => {
                present.insert("display.backlight_or_power".to_string());
            }
            "display.resolution" => {
                present.insert("display.resolution".to_string());
            }
            "display.touch" => {
                present.insert("display.touch".to_string());
            }
            _ => {}
        }
    }
}

pub(crate) fn add_generic_topic_fields(
    topic: &str,
    facts: &[SourceFact],
    present: &mut BTreeSet<String>,
) {
    if facts.iter().any(|fact| is_known_topic_fact(topic, fact)) {
        present.insert(format!("{topic}.chip"));
        present.insert(format!("{topic}.bus_or_interface"));
    }
}

fn is_known_topic_fact(topic: &str, fact: &SourceFact) -> bool {
    if !is_known_fact(fact) || fact.key.starts_with("framework.") {
        return false;
    }
    let haystack = format!("{} {} {}", fact.topic, fact.key, fact.value).to_lowercase();
    let keywords = prompt_keywords();
    keywords
        .topics
        .get(topic)
        .is_some_and(|needles| needles.iter().any(|needle| haystack.contains(needle)))
        || haystack.contains(topic)
}

pub(crate) fn completeness_facts(
    board: &BoardRecord,
    pack: &BoardFactPack,
    topic: &str,
) -> Vec<SourceFact> {
    let mut facts = facts_for_topic(pack, topic);
    facts.extend(generated_completeness_facts(board, pack, topic));
    facts.sort_by(|left, right| {
        right
            .authority_rank
            .cmp(&left.authority_rank)
            .then_with(|| left.key.cmp(&right.key))
    });
    facts
}

pub(crate) fn generated_completeness_facts(
    board: &BoardRecord,
    pack: &BoardFactPack,
    topic: &str,
) -> Vec<SourceFact> {
    let Some(source) = pack.source_refs.first().cloned() else {
        return Vec::new();
    };
    let mut facts = Vec::new();
    if board
        .demo_refs
        .iter()
        .any(|demo| demo_matches_topic(demo, topic))
    {
        facts.push(fact(
            board,
            topic,
            "framework.demo_refs",
            "official demo refs present",
            "Official demo or example references are available for this topic",
            source.clone(),
            "derived",
        ));
        facts.push(fact(
            board,
            topic,
            "framework.build_hint",
            &format!("frameworks={}", board.frameworks.join(",")),
            "Framework support is recorded for the board and topic demo refs",
            source.clone(),
            "derived",
        ));
    }
    if topic == "display" && !board.source_urls.is_empty() {
        facts.push(fact(
            board,
            topic,
            "debug.blank_screen_hints",
            "check power/backlight, bus init, reset, color order, LVGL tick/flush",
            "Blank-screen debug hints are generated from the display/LVGL contract",
            source,
            "derived",
        ));
    }
    facts
}

pub(crate) fn board_fact_enrichment(
    root: &Path,
    board_id: &str,
    topic: &str,
    dry_run: bool,
) -> Result<BoardFactEnrichmentReport, String> {
    let topic = normalize_completeness_topic(topic)?;
    let boards = load_board_index(root)?;
    let board = boards
        .boards
        .iter()
        .find(|board| board.id == board_id)
        .ok_or_else(|| format!("unknown board: {board_id}"))?;
    if !is_supported_esp32(board) {
        let validation = unsupported_completeness(&board.id, &topic);
        if dry_run {
            return Ok(unsupported_enrichment_report(
                board, &topic, dry_run, validation,
            ));
        }
        return Err(format!(
            "unsupported board for board-facts enrichment: {board_id}; only ESP32-family LilyGO boards can be enriched"
        ));
    }
    let mut index = load_fact_pack_index(root)?;
    ensure_pack(&mut index, board);
    let parsed_facts = enrichment_facts(board, &index, &topic);
    merge_enrichment_facts(&mut index, board_id, parsed_facts.clone());
    let validation = source_completeness_from_index(board, &index, &topic);
    let writes = if dry_run {
        Vec::new()
    } else {
        write_fact_pack_index(root, &index)?
    };
    Ok(enrichment_report(
        board,
        &topic,
        dry_run,
        parsed_facts,
        validation,
        writes,
    ))
}

pub(crate) fn ensure_pack(index: &mut BoardFactPackIndex, board: &BoardRecord) {
    if index.packs.iter().any(|pack| pack.board_id == board.id) {
        return;
    }
    index.packs.push(fact_pack_from_board(board));
}

pub(crate) fn enrichment_facts(
    board: &BoardRecord,
    index: &BoardFactPackIndex,
    topic: &str,
) -> Vec<SourceFact> {
    let pack = index
        .packs
        .iter()
        .find(|pack| pack.board_id == board.id)
        .cloned()
        .unwrap_or_else(|| fact_pack_from_board(board));
    let report = evaluate_completeness(board, &pack, topic);
    report
        .required_missing
        .iter()
        .filter_map(|field| unknown_fact_for_missing(board, &pack, topic, field))
        .collect()
}

pub(crate) fn unknown_fact_for_missing(
    board: &BoardRecord,
    pack: &BoardFactPack,
    topic: &str,
    field: &str,
) -> Option<SourceFact> {
    if matches!(
        field,
        "framework.demo_refs" | "framework.build_hint" | "debug.blank_screen_hints" | "source_refs"
    ) {
        return None;
    }
    let source = pack.source_refs.first().cloned()?;
    Some(fact(
        board,
        topic,
        field,
        "unknown_with_sources",
        "Exact value is missing; official refs must be parsed before quick-start readiness",
        source,
        "unknown_with_sources",
    ))
}

pub(crate) fn merge_enrichment_facts(
    index: &mut BoardFactPackIndex,
    board_id: &str,
    parsed_facts: Vec<SourceFact>,
) {
    let Some(pack) = index
        .packs
        .iter_mut()
        .find(|pack| pack.board_id == board_id)
    else {
        return;
    };
    for fact in parsed_facts {
        push_unique(&mut pack.peripheral_table, fact);
    }
}

pub(crate) fn push_unique(facts: &mut Vec<SourceFact>, fact: SourceFact) {
    if facts.iter().any(|existing| existing.key == fact.key) {
        return;
    }
    facts.push(fact);
}

pub(crate) fn write_fact_pack_index(
    root: &Path,
    index: &BoardFactPackIndex,
) -> Result<Vec<String>, String> {
    let rendered = serde_json::to_string_pretty(index)
        .map_err(|error| format!("failed to render fact packs: {error}"))?
        + "\n";
    if write_if_changed(&root.join(FACT_PACK_INDEX_PATH), rendered.as_bytes())? {
        Ok(vec![FACT_PACK_INDEX_PATH.to_string()])
    } else {
        Ok(Vec::new())
    }
}

pub(crate) fn source_completeness_from_index(
    board: &BoardRecord,
    index: &BoardFactPackIndex,
    topic: &str,
) -> CompletenessReport {
    let pack = index
        .packs
        .iter()
        .find(|pack| pack.board_id == board.id)
        .cloned()
        .unwrap_or_else(|| fact_pack_from_board(board));
    evaluate_completeness(board, &pack, topic)
}

pub(crate) fn enrichment_report(
    board: &BoardRecord,
    topic: &str,
    dry_run: bool,
    parsed_facts: Vec<SourceFact>,
    validation: CompletenessReport,
    writes: Vec<String>,
) -> BoardFactEnrichmentReport {
    enrichment_report_with(
        board,
        topic,
        dry_run,
        EnrichmentReportParts {
            parsed_facts,
            planned_writes: vec![FACT_PACK_INDEX_PATH.to_string()],
            writes,
            validation,
            validation_commands: vec![
            format!("lilygo-skills source completeness --board {} --topic {topic} --json", board.id),
            "cargo test -p lilygo-skills-cli generated_board_fact_packs representative_board_completeness".to_string(),
        ],
            warnings: vec![
                "dry-run writes are previews only; route and hook never mutate fact packs".to_string(),
                "unknown_with_sources is not quick-start readiness until a higher-authority source proves an exact value".to_string(),
            ],
        },
    )
}

pub(crate) fn unsupported_enrichment_report(
    board: &BoardRecord,
    topic: &str,
    dry_run: bool,
    validation: CompletenessReport,
) -> BoardFactEnrichmentReport {
    enrichment_report_with(
        board,
        topic,
        dry_run,
        EnrichmentReportParts {
            parsed_facts: Vec::new(),
            planned_writes: Vec::new(),
            writes: Vec::new(),
            validation,
            validation_commands: vec![format!(
                "lilygo-skills source completeness --board {} --topic {topic} --json",
                board.id
            )],
            warnings: vec![
                "unsupported LilyGO product boundary: board-facts enrichment is limited to ESP32-family boards".to_string(),
                "apply mode fails closed for unsupported boards and must not write fact packs".to_string(),
            ],
        },
    )
}

struct EnrichmentReportParts {
    parsed_facts: Vec<SourceFact>,
    planned_writes: Vec<String>,
    writes: Vec<String>,
    validation: CompletenessReport,
    validation_commands: Vec<String>,
    warnings: Vec<String>,
}

fn enrichment_report_with(
    board: &BoardRecord,
    topic: &str,
    dry_run: bool,
    parts: EnrichmentReportParts,
) -> BoardFactEnrichmentReport {
    let reads = planned_reads(board);
    let mut source_adapters = reads
        .iter()
        .map(|read| read.adapter.clone())
        .collect::<Vec<_>>();
    source_adapters.sort();
    source_adapters.dedup();
    let source_hashes = reads
        .iter()
        .map(|read| {
            (
                format!("{}:{}", read.adapter, read.path_or_url),
                read.hash.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    BoardFactEnrichmentReport {
        schema_version: 1,
        status: "PASS".to_string(),
        dry_run,
        board_id: board.id.clone(),
        topic: topic.to_string(),
        source_adapters,
        planned_reads: reads,
        parsed_facts: parts.parsed_facts,
        planned_writes: parts.planned_writes,
        writes: parts.writes,
        source_hashes,
        validation: EnrichmentValidation {
            contract_status_after_apply: parts.validation.completeness,
            required_present: parts.validation.required_present,
            required_missing: parts.validation.required_missing,
        },
        validation_commands: parts.validation_commands,
        warnings: parts.warnings,
    }
}

pub(crate) fn planned_reads(board: &BoardRecord) -> Vec<EnrichmentRead> {
    let mut reads = board_sources(board)
        .into_iter()
        .map(|source| EnrichmentRead {
            adapter: adapter_for_source(&source.kind),
            authority_rank: source_authority_rank(&source.kind),
            path_or_url: source.path_or_url,
            hash: source.hash,
        })
        .collect::<Vec<_>>();
    for demo in &board.demo_refs {
        reads.push(EnrichmentRead {
            adapter: "official-examples".to_string(),
            authority_rank: 85,
            path_or_url: demo.source_url.clone(),
            hash: format!(
                "sha256:{}",
                stable_hash(&(demo.source_url.as_str(), demo.path.as_str()))
            ),
        });
    }
    reads.sort_by(|left, right| {
        right
            .authority_rank
            .cmp(&left.authority_rank)
            .then_with(|| left.path_or_url.cmp(&right.path_or_url))
    });
    reads.dedup_by(|left, right| left.path_or_url == right.path_or_url);
    reads
}

pub(crate) fn adapter_for_source(kind: &str) -> String {
    match kind {
        "github-repo" | "quick-start" => "official-github-repo",
        "driver-header" | "arduino-pins" | "official-code" => "official-code",
        "hardware-doc" => "official-hardware-doc",
        "documentation-repo" => "documentation-repo",
        "wiki" => "wiki-fallback",
        other => other,
    }
    .to_string()
}

pub(crate) fn intersection(fields: &[String], present: &BTreeSet<String>) -> Vec<String> {
    fields
        .iter()
        .filter(|field| present.contains(field.as_str()))
        .cloned()
        .collect()
}

pub(crate) fn difference(fields: &[String], present: &BTreeSet<String>) -> Vec<String> {
    fields
        .iter()
        .filter(|field| !present.contains(field.as_str()))
        .cloned()
        .collect()
}

pub(crate) fn completeness_warnings(topic: &str, completeness: &str) -> Vec<String> {
    let mut warnings = vec![
        "completeness is V3 source/context evidence, not build, flash, serial, OTA, LVGL pixel, or physical peripheral proof".to_string(),
    ];
    if completeness == "needs_source_ingestion" {
        warnings.push(format!(
            "{topic} quick-start is not ready until missing required facts are parsed from official refs"
        ));
    }
    warnings
}

pub(crate) fn fact_pack_report(
    root: &Path,
    index: &BoardFactPackIndex,
    dry_run: bool,
    writes: Vec<String>,
) -> FactPackUpdateReport {
    let source_hashes = index
        .packs
        .iter()
        .map(|pack| (pack.board_id.clone(), stable_hash(pack)))
        .collect::<BTreeMap<_, _>>();
    FactPackUpdateReport {
        status: "PASS".to_string(),
        dry_run,
        fact_pack_count: index.packs.len(),
        stale_fact_packs: stale_fact_packs(root, index),
        source_hashes,
        planned_writes: vec![FACT_PACK_INDEX_PATH.to_string()],
        writes,
        warnings: vec![
            "fact packs are source/context evidence; build, flash, serial, OTA, LVGL pixels, and physical behavior need later evidence".to_string(),
            "XL9555 channel mappings stay unknown unless a higher-authority source proves exact channel assignments".to_string(),
        ],
    }
}
