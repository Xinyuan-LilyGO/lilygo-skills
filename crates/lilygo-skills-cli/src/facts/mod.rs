//! Source-fact query, enrichment, readiness, and compact context-budget facade
//! shared by routing, goals, references, and generated skills.
use crate::model::{
    BoardFactEnrichmentReport, BoardFactPack, BoardFactPackIndex, BoardRecord,
    CompletenessNextAction, CompletenessReport, CompletenessSignal, ContextBudget, DiscoveryHint,
    EnrichmentRead, EnrichmentValidation, FactPackUpdateReport, FactQueryReport, FactTablePreview,
    PeripheralRecord, SourceFact, SourceFactSource, SourceUrl,
};
use crate::source::{load_board_index, write_if_changed};
use crate::text_match::contains_any;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(crate) const FACT_PACK_INDEX_PATH: &str = "data/facts/board-fact-packs.json";
const DOCUMENTATION_REPO: &str = "https://github.com/Xinyuan-LilyGO/documentation";

// Fact/implementation/topic prompt keywords are data-owned so classification
// rules can be reviewed with the fact-pack contracts.
const PROMPT_KEYWORDS_JSON: &str = include_str!("../../../../data/facts/prompt-keywords.json");

#[derive(serde::Deserialize)]
pub(crate) struct PromptKeywords {
    pub fact_prompt: Vec<String>,
    pub implementation_or_debug: Vec<String>,
    pub topic_order: Vec<String>,
    pub topics: BTreeMap<String, Vec<String>>,
}

pub(crate) fn prompt_keywords() -> PromptKeywords {
    serde_json::from_str(PROMPT_KEYWORDS_JSON)
        .expect("embedded data/facts/prompt-keywords.json must be valid")
}

pub(crate) fn source_query(
    root: &Path,
    board_id: &str,
    topic: &str,
) -> Result<FactQueryReport, String> {
    let topic = normalize_topic(topic)?;
    let index = load_fact_pack_index(root)?;
    let pack = index
        .packs
        .into_iter()
        .find(|pack| pack.board_id == board_id)
        .ok_or_else(|| format!("unknown board fact pack: {board_id}"))?;
    let facts = facts_for_topic(&pack, topic);
    let unknowns = facts
        .iter()
        .filter(|fact| fact.confidence == "unknown_with_sources")
        .cloned()
        .collect::<Vec<_>>();
    Ok(FactQueryReport {
        status: if pack.supported {
            "PASS"
        } else {
            "UNSUPPORTED"
        }
        .to_string(),
        board_id: board_id.to_string(),
        topic: topic.to_string(),
        supported: pack.supported,
        fact_pack: pack.clone(),
        facts,
        unknowns,
        conflicts: pack.conflicts.clone(),
        source_refs: pack.source_refs.clone(),
        completeness: if is_readiness_topic(topic) {
            completeness_signal(root, board_id, topic).ok()
        } else {
            None
        },
        discovery_hints: discovery_hints(board_id, topic, true),
        warnings: query_warnings(&pack),
    })
}

pub(crate) fn source_completeness(
    root: &Path,
    board_id: &str,
    topic: &str,
) -> Result<CompletenessReport, String> {
    let topic = normalize_completeness_topic(topic)?;
    let boards = load_board_index(root)?;
    let Some(board) = boards.boards.iter().find(|board| board.id == board_id) else {
        return Ok(unsupported_completeness(board_id, topic));
    };
    let mut pack = load_fact_pack_index(root)?
        .packs
        .into_iter()
        .find(|pack| pack.board_id == board_id)
        .unwrap_or_else(|| fact_pack_from_board(board));
    pack.supported = pack.supported && is_supported_esp32(board);
    Ok(evaluate_completeness(board, &pack, topic))
}

pub(crate) fn board_fact_enrichment_preview(
    root: &Path,
    board_id: &str,
    topic: &str,
) -> Result<BoardFactEnrichmentReport, String> {
    board_fact_enrichment(root, board_id, topic, true)
}

pub(crate) fn board_fact_enrichment_apply(
    root: &Path,
    board_id: &str,
    topic: &str,
) -> Result<BoardFactEnrichmentReport, String> {
    board_fact_enrichment(root, board_id, topic, false)
}

pub(crate) fn completeness_signals_for_prompt(
    root: &Path,
    board_id: Option<&str>,
    prompt: &str,
) -> Vec<CompletenessSignal> {
    let Some(board_id) = board_id else {
        return Vec::new();
    };
    let topics = topics_for_prompt(prompt);
    topics
        .iter()
        .filter_map(|topic| completeness_signal(root, board_id, topic).ok())
        .collect()
}

pub(crate) fn build_fact_pack_index(root: &Path) -> Result<BoardFactPackIndex, String> {
    let boards = load_board_index(root)?;
    let packs = boards.boards.iter().map(fact_pack_from_board).collect();
    Ok(BoardFactPackIndex {
        schema_version: 1,
        packs,
    })
}

pub(crate) fn load_fact_pack_index(root: &Path) -> Result<BoardFactPackIndex, String> {
    let path = root.join(FACT_PACK_INDEX_PATH);
    if path.is_file() {
        let data = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        return serde_json::from_str(&data)
            .map_err(|error| format!("invalid {}: {error}", path.display()));
    }
    build_fact_pack_index(root)
}

pub(crate) fn fact_pack_preview(root: &Path) -> Result<FactPackUpdateReport, String> {
    let index = build_fact_pack_index(root)?;
    Ok(fact_pack_report(root, &index, true, Vec::new()))
}

pub(crate) fn fact_pack_apply(root: &Path) -> Result<FactPackUpdateReport, String> {
    let index = build_fact_pack_index(root)?;
    let rendered = serde_json::to_string_pretty(&index)
        .map_err(|error| format!("failed to render fact packs: {error}"))?
        + "\n";
    let writes = if write_if_changed(&root.join(FACT_PACK_INDEX_PATH), rendered.as_bytes())? {
        vec![FACT_PACK_INDEX_PATH.to_string()]
    } else {
        Vec::new()
    };
    Ok(fact_pack_report(root, &index, false, writes))
}

pub(crate) fn fact_tables_for_goal(
    root: &Path,
    board_id: &str,
    prompt: &str,
) -> Result<Vec<FactTablePreview>, String> {
    if !is_fact_prompt(prompt) {
        return Ok(Vec::new());
    }
    let report = source_query(root, board_id, "io")?;
    let budget = ContextBudget::default();
    Ok([
        (
            "pin_matrix",
            report.fact_pack.pin_matrix.as_slice(),
            "pinout",
        ),
        ("bus_matrix", report.fact_pack.bus_matrix.as_slice(), "bus"),
        (
            "expander_matrix",
            report.fact_pack.expander_matrix.as_slice(),
            "expander",
        ),
        (
            "connector_matrix",
            report.fact_pack.connector_matrix.as_slice(),
            "connector",
        ),
        (
            "peripheral_table",
            report.fact_pack.peripheral_table.as_slice(),
            "peripheral",
        ),
    ]
    .into_iter()
    .filter(|(_, rows, _)| !rows.is_empty())
    .map(|(table, rows, topic)| table_preview(board_id, table, rows, topic, &budget))
    .collect())
}

pub(crate) fn discovery_hints_for_goal(board_id: Option<&str>, prompt: &str) -> Vec<DiscoveryHint> {
    if board_id.is_none() && is_embedded_fact_or_impl_prompt(prompt) {
        return vec![DiscoveryHint {
            when: "board identity is missing".to_string(),
            action: "ask_clarification".to_string(),
            command: None,
            reference_id: None,
            reason: "A source fact query needs an exact LilyGO ESP32-family board id.".to_string(),
        }];
    }
    let Some(board_id) = board_id else {
        return Vec::new();
    };
    if is_fact_prompt(prompt) {
        return discovery_hints(board_id, "io", true);
    }
    if is_implementation_or_debug_prompt(prompt) {
        return discovery_hints(board_id, "peripheral", false);
    }
    Vec::new()
}

pub(crate) fn is_fact_prompt(prompt: &str) -> bool {
    let normalized = prompt.to_lowercase();
    let keywords = prompt_keywords();
    let needles = keywords
        .fact_prompt
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    contains_any(&normalized, &needles)
}

pub(crate) fn is_implementation_or_debug_prompt(prompt: &str) -> bool {
    let normalized = prompt.to_lowercase();
    let keywords = prompt_keywords();
    let needles = keywords
        .implementation_or_debug
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    contains_any(&normalized, &needles)
}

pub(crate) fn source_authority_rank(kind: &str) -> u32 {
    match kind {
        "official-code" => 100,
        "driver-header" | "arduino-pins" => 95,
        "hardware-doc" => 90,
        "github-repo" | "quick-start" => 85,
        "chip-vendor" | "framework-official" => 80,
        "documentation-repo" => 70,
        "wiki" => 55,
        "local-reference" => 45,
        "community" => 20,
        _ => 10,
    }
}

mod build;
mod completeness;
pub(crate) use build::*;
pub(crate) use completeness::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;

    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn temp_fact_root(name: &str) -> std::path::PathBuf {
        let temp = std::env::temp_dir().join(format!("lilygo-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join("data/facts")).expect("temp facts dir");
        fs::copy(
            root().join("data/boards.json"),
            temp.join("data/boards.json"),
        )
        .expect("copy boards");
        fs::copy(
            root().join(FACT_PACK_INDEX_PATH),
            temp.join(FACT_PACK_INDEX_PATH),
        )
        .expect("copy fact packs");
        temp
    }

    #[test]
    fn fact_schema() {
        let index = build_fact_pack_index(root().as_path()).expect("fact packs");
        let pack = index
            .packs
            .iter()
            .find(|pack| pack.board_id == "board-t-watch-ultra")
            .expect("watch ultra pack");
        assert_eq!(pack.schema_version, 1);
        assert!(pack.supported);
        assert!(!pack.pin_matrix.is_empty());
        assert!(!pack.expander_matrix.is_empty());
        assert!(!pack.peripheral_table.is_empty());
        assert!(
            pack.peripheral_table
                .iter()
                .all(|fact| fact.authority_rank > 0 && fact.source.hash.starts_with("sha256:"))
        );
    }

    #[test]
    fn source_query_cli() {
        let report = source_query(root().as_path(), "board-t-watch-ultra", "io").expect("query");
        assert_eq!(report.status, "PASS");
        assert!(report.supported);
        assert!(
            report
                .facts
                .iter()
                .any(|fact| fact.value.contains("Bosch BHI260AP"))
        );
        assert!(
            report
                .unknowns
                .iter()
                .any(|fact| fact.key == "expander.xl9555.channel-map")
        );
        assert!(!report.discovery_hints.is_empty());
    }

    #[test]
    fn fact_source_adapters() {
        let pack = build_fact_pack_index(root().as_path())
            .expect("fact packs")
            .packs
            .into_iter()
            .find(|pack| pack.board_id == "board-t-watch-ultra")
            .expect("watch ultra pack");
        let kinds = pack
            .source_refs
            .iter()
            .map(|source| source.kind.as_str())
            .collect::<BTreeSet<_>>();
        assert!(kinds.contains("arduino-pins"));
        assert!(kinds.contains("driver-header"));
        assert!(kinds.contains("hardware-doc"));
        assert!(kinds.contains("documentation-repo"));
    }

    #[test]
    fn fact_authority_conflicts() {
        assert!(
            source_authority_rank("official-code") > source_authority_rank("documentation-repo")
        );
        assert!(source_authority_rank("driver-header") > source_authority_rank("local-reference"));
        let report = source_query(root().as_path(), "board-t-watch-ultra", "expander")
            .expect("expander query");
        let unknown = report
            .facts
            .iter()
            .find(|fact| fact.key == "expander.xl9555.channel-map")
            .expect("unknown channel mapping");
        assert_eq!(unknown.value, "unknown_with_sources");
        assert_eq!(unknown.confidence, "unknown_with_sources");
    }

    #[test]
    fn io_prompt_source_lookup() {
        let tables = fact_tables_for_goal(
            root().as_path(),
            "board-t-watch-ultra",
            "T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?",
        )
        .expect("goal fact tables");
        assert!(tables.iter().any(|table| table.table == "bus_matrix"));
        assert!(tables.iter().any(|table| table.table == "expander_matrix"));
        assert!(
            tables
                .iter()
                .all(|table| table.query_command.contains("source query"))
        );
    }

    #[test]
    fn completeness_contracts() {
        let report = source_completeness(root().as_path(), "board-t-display-s3", "display")
            .expect("display completeness");
        assert_eq!(report.topic, "display");
        for field in [
            "display.panel_or_chip",
            "display.bus_or_interface",
            "display.backlight_or_power",
            "framework.demo_refs",
            "framework.build_hint",
            "debug.blank_screen_hints",
            "source_refs",
        ] {
            assert!(
                report.required_present.contains(&field.to_string())
                    || report.required_missing.contains(&field.to_string()),
                "contract missing {field}"
            );
        }
    }

    #[test]
    fn display_completeness_contract() {
        let report = source_completeness(root().as_path(), "board-t-display-s3", "display")
            .expect("display completeness");
        assert_eq!(report.completeness, "complete");
        assert!(report.required_missing.is_empty());
        assert!(
            report
                .required_present
                .contains(&"display.panel_or_chip".to_string())
        );
        assert!(
            report
                .required_present
                .contains(&"display.bus_or_interface".to_string())
        );
        assert!(
            report
                .required_present
                .contains(&"display.backlight_or_power".to_string())
        );
        assert!(
            report
                .source_refs
                .iter()
                .any(|source| source.kind == "driver-header")
        );
        assert!(
            report
                .next_actions
                .iter()
                .any(|action| action.command.contains("source query"))
        );
    }

    #[test]
    fn source_completeness_cli() {
        let report = source_completeness(root().as_path(), "board-t-watch-ultra", "display")
            .expect("watch display completeness");
        assert_eq!(report.status, "PASS");
        assert_eq!(report.completeness, "complete");
        assert!(report.required_missing.is_empty());
    }

    #[test]
    fn board_fact_enrichment_adapters() {
        let report =
            board_fact_enrichment_preview(root().as_path(), "board-t-display-s3", "display")
                .expect("enrichment dry-run");
        assert!(report.dry_run);
        assert!(report.writes.is_empty());
        assert!(
            report
                .source_adapters
                .contains(&"official-github-repo".to_string())
        );
        assert!(
            report
                .source_adapters
                .contains(&"documentation-repo".to_string())
        );
        assert!(report.parsed_facts.is_empty());
        assert_eq!(report.validation.contract_status_after_apply, "complete");
        assert!(report.validation.required_missing.is_empty());
    }

    #[test]
    fn board_fact_enrichment_unsupported_apply_no_write() {
        let temp = temp_fact_root("unsupported-board-facts");
        let fact_pack_path = temp.join(FACT_PACK_INDEX_PATH);
        let before = fs::read(&fact_pack_path).expect("fact pack before");
        let dry_run =
            board_fact_enrichment_preview(temp.as_path(), "future-rp2040-product", "display")
                .expect("unsupported dry-run report");
        assert_eq!(
            dry_run.validation.contract_status_after_apply,
            "unsupported"
        );
        assert!(dry_run.planned_writes.is_empty());
        assert!(dry_run.writes.is_empty());

        let apply = board_fact_enrichment_apply(temp.as_path(), "future-rp2040-product", "display");
        assert!(apply.is_err());
        let after = fs::read(&fact_pack_path).expect("fact pack after");
        assert_eq!(before, after);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn documentation_repo_adapter() {
        let report =
            board_fact_enrichment_preview(root().as_path(), "board-t-display-s3", "display")
                .expect("enrichment dry-run");
        let docs = report
            .planned_reads
            .iter()
            .find(|read| read.adapter == "documentation-repo")
            .expect("documentation repo read");
        assert_eq!(docs.path_or_url, DOCUMENTATION_REPO);
        assert!(docs.hash.starts_with("sha256:"));
    }

    #[test]
    fn source_authority_precedence() {
        assert!(
            source_authority_rank("official-code") > source_authority_rank("documentation-repo")
        );
        let report =
            board_fact_enrichment_preview(root().as_path(), "board-t-display-s3", "display")
                .expect("enrichment dry-run");
        let official = report
            .planned_reads
            .iter()
            .find(|read| read.adapter == "official-code")
            .expect("official code read");
        let docs = report
            .planned_reads
            .iter()
            .find(|read| read.adapter == "documentation-repo")
            .expect("documentation read");
        assert!(official.authority_rank > docs.authority_rank);
    }

    #[test]
    fn generated_board_fact_packs() {
        let report =
            board_fact_enrichment_preview(root().as_path(), "board-t-display-s3", "display")
                .expect("enrichment dry-run");
        assert!(
            report
                .planned_writes
                .contains(&FACT_PACK_INDEX_PATH.to_string())
        );
        assert_eq!(report.validation.contract_status_after_apply, "complete");
    }

    #[test]
    fn representative_board_completeness() {
        let root = root();
        let watch_display =
            source_completeness(root.as_path(), "board-t-watch-ultra", "display").unwrap();
        let watch_imu = source_completeness(root.as_path(), "board-t-watch-ultra", "imu").unwrap();
        let watch_power =
            source_completeness(root.as_path(), "board-t-watch-ultra", "power").unwrap();
        assert_eq!(watch_display.completeness, "complete");
        assert_eq!(watch_imu.completeness, "complete");
        assert_eq!(watch_power.completeness, "complete");
        for (board, topic) in [
            ("board-t-display-s3", "display"),
            ("board-t-beam", "lora"),
            ("board-t-beam", "gnss"),
            ("board-t-deck", "display"),
            ("board-t-deck", "input"),
        ] {
            let report = source_completeness(root.as_path(), board, topic).unwrap();
            assert!(
                matches!(
                    report.completeness.as_str(),
                    "needs_source_ingestion" | "partial" | "complete"
                ),
                "{board}/{topic}: {}",
                report.completeness
            );
            assert!(
                !report.next_actions.is_empty() || report.completeness == "complete",
                "{board}/{topic} missing next action"
            );
        }
    }

    #[test]
    fn completeness_privacy_boundary() {
        let rendered = serde_json::to_string(
            &source_completeness(root().as_path(), "board-t-display-s3", "display")
                .expect("display completeness"),
        )
        .expect("json");
        for forbidden in ["/dev/", "192.168.", "password", "token=", "/Users/"] {
            assert!(!rendered.contains(forbidden), "leaked {forbidden}");
        }
        assert!(rendered.contains("V3"));
        assert!(!rendered.contains("hardware_verified\":true"));
    }
}
