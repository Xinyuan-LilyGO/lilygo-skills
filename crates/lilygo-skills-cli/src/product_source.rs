//! Product-source adapter that merges cached LilyGO Wiki and GitHub metadata
//! into supported ESP32-family board records and source-backed matrices.
use crate::model::{
    BoardIndex, BoardRecord, DemoRef, PeripheralRecord, ProductCandidate, SourceUrl,
};
use crate::source::{REPO_CACHE_PATH, WIKI_PRODUCTS_CACHE_PATH};
use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::Path;

const LILYGOLIB_BLOB: &str = "https://github.com/Xinyuan-LilyGO/LilyGoLib/blob/master";
const ARDUINO_PINS_URL: &str = "https://github.com/espressif/arduino-esp32/blob/master/variants/lilygo_twatch_ultra/pins_arduino.h";

#[derive(Debug, Deserialize)]
struct WikiProducts {
    products: Vec<WikiProduct>,
}

#[derive(Debug, Deserialize)]
struct WikiProduct {
    slug: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct RepoCache {
    #[serde(default)]
    repos: Vec<RepoRecord>,
}

#[derive(Debug, Clone, Deserialize)]
struct RepoRecord {
    name: String,
    html_url: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    pushed_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

/// Fail-closed merge: pruning committed product boards is only allowed when
/// both source caches are readable and actually yield product records. An
/// unreadable, corrupt, or empty cache must never empty `boards.json`.
pub(crate) fn merge_product_records(
    root: &Path,
    mut board_index: BoardIndex,
) -> Result<BoardIndex, String> {
    let products = try_read_wiki_products(root)?;
    let repos = try_read_repo_cache(root)?;
    let records: Vec<BoardRecord> = products
        .iter()
        .filter_map(|product| product_record_from_source(product, &repos))
        .collect();
    if records.is_empty() {
        return Err(
            "product source caches yielded no product records; refusing to prune boards.json \
             (fail-closed). Refresh data/references/source-intake/raw caches and retry."
                .to_string(),
        );
    }
    let generated_ids: BTreeSet<String> = records.iter().map(|record| record.id.clone()).collect();
    board_index
        .boards
        .retain(|board| !board.product || generated_ids.contains(&board.id));
    for record in records {
        if let Some(existing) = board_index
            .boards
            .iter_mut()
            .find(|board| board.id == record.id)
        {
            *existing = merge_existing_product(existing, record);
        } else {
            board_index.boards.push(record);
        }
    }
    sort_board_index(&mut board_index);
    Ok(board_index)
}

pub(crate) fn product_candidates(root: &Path, board_index: &BoardIndex) -> Vec<ProductCandidate> {
    let products = read_wiki_products(root);
    let repos = read_repo_cache(root);
    products
        .iter()
        .map(|product| {
            let generated = product_record_from_source(product, &repos);
            let existing = board_index
                .boards
                .iter()
                .find(|board| board.id == board_id_for_slug(&product.slug));
            candidate_from_product(product, generated.as_ref(), existing)
        })
        .collect()
}

pub(crate) fn stale_product_record_ids(root: &Path, board_index: &BoardIndex) -> Vec<String> {
    product_records(root)
        .into_iter()
        .filter(|record| product_record_is_stale(record, board_index))
        .map(|record| record.id)
        .collect()
}

pub(crate) fn product_records(root: &Path) -> Vec<BoardRecord> {
    let products = read_wiki_products(root);
    let repos = read_repo_cache(root);
    products
        .iter()
        .filter_map(|product| product_record_from_source(product, &repos))
        .collect()
}

fn candidate_from_product(
    product: &WikiProduct,
    generated: Option<&BoardRecord>,
    existing: Option<&BoardRecord>,
) -> ProductCandidate {
    let generated_or_existing = generated.or(existing);
    ProductCandidate {
        id: board_id_for_slug(&product.slug),
        family_id: generated_or_existing.and_then(|record| record.family_id.clone()),
        slug: product.slug.clone(),
        wiki_url: product.url.clone(),
        repo_url: generated_or_existing
            .map(|record| record.repo_url.clone())
            .unwrap_or_default(),
        supported: generated_or_existing.is_some_and(|record| record.supported),
        source_status: generated_or_existing
            .map(|record| record.source_status.clone())
            .unwrap_or_else(|| "wiki-cache-only".to_string()),
        stale: generated
            .zip(existing)
            .is_some_and(|(generated, existing)| generated.source_hashes != existing.source_hashes)
            || generated.is_some() && existing.is_none(),
        warnings: generated_or_existing
            .map(|record| record.warnings.clone())
            .unwrap_or_else(|| {
                vec![
                    "Wiki product exists, but cached repo evidence is not enough to generate an active ESP32 product skill."
                        .to_string(),
                ]
            }),
    }
}

fn product_record_is_stale(record: &BoardRecord, board_index: &BoardIndex) -> bool {
    board_index
        .boards
        .iter()
        .find(|board| board.id == record.id)
        .map(|existing| existing.source_hashes != record.source_hashes || existing.stale)
        .unwrap_or(true)
}

fn product_record_from_source(product: &WikiProduct, repos: &[RepoRecord]) -> Option<BoardRecord> {
    let repo = repo_for_slug(&product.slug, repos)?;
    let mcu = infer_mcu(&product.slug, repo)?;
    if !is_supported_mcu(&mcu) {
        return None;
    }

    let mut record = BoardRecord {
        id: board_id_for_slug(&product.slug),
        family_id: family_for_slug(&product.slug),
        product: true,
        display_name: display_name_for_slug(&product.slug),
        aliases: aliases_for_slug(&product.slug),
        mcu,
        supported: true,
        frameworks: default_frameworks(),
        peripherals: inferred_peripherals(&product.slug),
        repo_url: repo.html_url.clone(),
        wiki_url: product.url.clone(),
        source_status: "github-cache+wiki-cache".to_string(),
        source_urls: source_urls(product, repo),
        source_hashes: source_hashes(product, repo),
        stale: false,
        peripheral_matrix: Vec::new(),
        demo_refs: generic_demo_refs(&product.slug, repo),
        warnings: vec![
            "Generated from cached Wiki product URL plus GitHub repository metadata; pin-level claims require a product matrix or source lookup."
                .to_string(),
        ],
    };

    if product.slug == "t-watch-ultra" {
        record.peripherals = t_watch_ultra_peripherals();
        record.peripheral_matrix = t_watch_ultra_matrix();
        record.demo_refs = t_watch_ultra_demo_refs();
        record.warnings = vec![
            "LVGL and OTA are cross-board framework/application layers; this product skill exposes hardware capabilities and official source/demo references only."
                .to_string(),
            "Demo links are context/source evidence, not build, flash, serial, OTA, or rendered UI success."
                .to_string(),
        ];
    }

    Some(record)
}

fn merge_existing_product(existing: &BoardRecord, mut generated: BoardRecord) -> BoardRecord {
    generated.aliases = merged_strings(&generated.aliases, &existing.aliases);
    if !existing.peripheral_matrix.is_empty() && generated.peripheral_matrix.is_empty() {
        generated.peripheral_matrix = existing.peripheral_matrix.clone();
    }
    if !existing.demo_refs.is_empty() && generated.demo_refs.is_empty() {
        generated.demo_refs = existing.demo_refs.clone();
    }
    generated.warnings = merged_warnings(&generated.warnings, &existing.warnings);
    generated
}

fn merged_warnings(generated: &[String], existing: &[String]) -> Vec<String> {
    merged_strings(generated, existing)
}

fn merged_strings(generated: &[String], existing: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    generated
        .iter()
        .chain(existing.iter())
        .filter(|warning| seen.insert((*warning).clone()))
        .cloned()
        .collect()
}

fn sort_board_index(board_index: &mut BoardIndex) {
    board_index.boards.sort_by(|left, right| {
        left.id
            .starts_with("future-")
            .cmp(&right.id.starts_with("future-"))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn try_read_wiki_products(root: &Path) -> Result<Vec<WikiProduct>, String> {
    let path = root.join(WIKI_PRODUCTS_CACHE_PATH);
    let data = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str::<WikiProducts>(&data)
        .map(|cache| cache.products)
        .map_err(|error| format!("invalid {}: {error}", path.display()))
}

fn try_read_repo_cache(root: &Path) -> Result<Vec<RepoRecord>, String> {
    let path = root.join(REPO_CACHE_PATH);
    let file = fs::File::open(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut decoder = GzDecoder::new(file);
    let mut data = String::new();
    decoder
        .read_to_string(&mut data)
        .map_err(|error| format!("corrupt gzip cache {}: {error}", path.display()))?;
    serde_json::from_str::<RepoCache>(&data)
        .map(|cache| cache.repos)
        .map_err(|error| format!("invalid {}: {error}", path.display()))
}

// Candidate/staleness listings tolerate missing caches (read-only views);
// only the pruning merge above is fail-closed.
fn read_wiki_products(root: &Path) -> Vec<WikiProduct> {
    try_read_wiki_products(root).unwrap_or_default()
}

fn read_repo_cache(root: &Path) -> Vec<RepoRecord> {
    try_read_repo_cache(root).unwrap_or_default()
}

fn repo_for_slug<'a>(slug: &str, repos: &'a [RepoRecord]) -> Option<&'a RepoRecord> {
    let key = normalized_key(slug);
    if matches!(
        slug,
        "t-watch-ultra" | "t-watch-s3" | "t-watch-s3-plus" | "t-lora-pager"
    ) {
        return repos.iter().find(|repo| {
            repo.name == "LilyGoLib"
                && repo
                    .description
                    .as_deref()
                    .map(|description| normalized_key(description).contains(&key))
                    .unwrap_or(false)
        });
    }

    repos.iter().find(|repo| {
        repo_keys(repo)
            .iter()
            .any(|repo_key| repo_key == &key || repo_key.ends_with(&key))
    })
}

fn repo_keys(repo: &RepoRecord) -> Vec<String> {
    let name = repo.name.trim();
    let without_lilygo = name
        .strip_prefix("LilyGO-")
        .or_else(|| name.strip_prefix("LilyGo-"))
        .or_else(|| name.strip_prefix("Lilygo-"))
        .or_else(|| name.strip_prefix("LilyGo"))
        .unwrap_or(name);
    let without_ttgo = without_lilygo
        .strip_prefix("TTGO-")
        .or_else(|| without_lilygo.strip_prefix("TTGO_"))
        .unwrap_or(without_lilygo);
    vec![normalized_key(name), normalized_key(without_ttgo)]
}

fn infer_mcu(slug: &str, repo: &RepoRecord) -> Option<String> {
    let source =
        format!("{slug} {}", repo.description.as_deref().unwrap_or_default()).to_lowercase();
    if source.contains("esp32-c5")
        || source.contains("esp32c5")
        || source.contains("esp32-c6")
        || source.contains("esp32c6")
        || slug.contains("-c5")
        || slug.contains("-c6")
    {
        return None;
    }
    if source.contains("esp32-s3") || source.contains("esp32s3") || slug.contains("-s3") {
        return Some("esp32-s3".to_string());
    }
    if source.contains("esp32-s2") || source.contains("esp32s2") || slug.contains("-s2") {
        return Some("esp32-s2".to_string());
    }
    if source.contains("esp32-c3") || source.contains("esp32c3") || slug.contains("-c3") {
        return Some("esp32-c3".to_string());
    }
    if source.contains("esp32-p4") || source.contains("esp32p4") || slug.contains("-p4") {
        return Some("esp32-p4".to_string());
    }
    if matches!(
        slug,
        "t-display" | "t-watch-2019" | "t-watch-2021" | "t-beam"
    ) || source.contains("esp32")
    {
        return Some("esp32".to_string());
    }
    if slug == "t-watch-ultra" || slug == "t-lora-pager" {
        return Some("esp32-s3".to_string());
    }
    None
}

fn is_supported_mcu(mcu: &str) -> bool {
    matches!(
        mcu,
        "esp32" | "esp32-s2" | "esp32-s3" | "esp32-c3" | "esp32-p4"
    )
}

fn source_urls(product: &WikiProduct, repo: &RepoRecord) -> Vec<SourceUrl> {
    let mut urls = vec![
        source_url("wiki", &product.url, "wiki-cache"),
        source_url("github-repo", &repo.html_url, "github-cache"),
    ];
    if product.slug == "t-watch-ultra" {
        for (kind, url, status) in [
            (
                "hardware-doc",
                format!("{LILYGOLIB_BLOB}/docs/hardware/lilygo-t-watch-ultra.md"),
                "github-live-verified",
            ),
            (
                "quick-start",
                format!("{LILYGOLIB_BLOB}/docs/lilygo-t-watch-ultra.md"),
                "github-live-verified",
            ),
            (
                "driver-header",
                format!("{LILYGOLIB_BLOB}/src/LilyGoWatchUltra.h"),
                "github-live-verified",
            ),
            (
                "arduino-pins",
                ARDUINO_PINS_URL.to_string(),
                "official-github",
            ),
        ] {
            urls.push(source_url(kind, &url, status));
        }
    }
    urls
}

fn source_hashes(product: &WikiProduct, repo: &RepoRecord) -> BTreeMap<String, String> {
    let mut hashes = BTreeMap::new();
    hashes.insert("wiki".to_string(), source_hash(&product.url));
    hashes.insert(
        "repo".to_string(),
        source_hash(&format!(
            "{}|{}|{}|{}",
            repo.html_url,
            repo.pushed_at.as_deref().unwrap_or_default(),
            repo.updated_at.as_deref().unwrap_or_default(),
            repo.description.as_deref().unwrap_or_default()
        )),
    );
    hashes.insert(
        "product".to_string(),
        source_hash(&format!(
            "{}|{}|{}",
            product.slug, product.url, repo.html_url
        )),
    );
    hashes
}

fn source_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn source_url(kind: &str, url: &str, status: &str) -> SourceUrl {
    SourceUrl {
        kind: kind.to_string(),
        url: url.to_string(),
        status: status.to_string(),
    }
}

const T_WATCH_ULTRA_MATRIX_JSON: &str =
    include_str!("../../../data/product/t-watch-ultra-matrix.json");

#[derive(serde::Deserialize)]
struct MatrixFile {
    entries: Vec<MatrixEntry>,
}

#[derive(serde::Deserialize)]
struct MatrixEntry {
    category: String,
    name: String,
    chip: String,
    bus: String,
    driver: String,
    source: String,
}

fn t_watch_ultra_matrix() -> Vec<PeripheralRecord> {
    let hardware = format!("{LILYGOLIB_BLOB}/docs/hardware/lilygo-t-watch-ultra.md");
    let header = format!("{LILYGOLIB_BLOB}/src/LilyGoWatchUltra.h");
    let file: MatrixFile = serde_json::from_str(T_WATCH_ULTRA_MATRIX_JSON)
        .expect("embedded data/product/t-watch-ultra-matrix.json must be valid");
    file.entries
        .into_iter()
        .map(|entry| {
            let source_url = if entry.source == "header" {
                &header
            } else {
                &hardware
            };
            peripheral(
                &entry.category,
                &entry.name,
                &entry.chip,
                &entry.bus,
                &entry.driver,
                source_url,
            )
        })
        .collect()
}

fn peripheral(
    category: &str,
    name: &str,
    chip: &str,
    bus: &str,
    driver: &str,
    source_url: &str,
) -> PeripheralRecord {
    PeripheralRecord {
        category: category.to_string(),
        name: name.to_string(),
        chip: chip.to_string(),
        bus: bus.to_string(),
        driver: driver.to_string(),
        source_url: source_url.to_string(),
        source_status: "github-live-verified".to_string(),
        evidence_level: "V3-source-reference".to_string(),
    }
}

fn t_watch_ultra_demo_refs() -> Vec<DemoRef> {
    let mut demos = [
        ("hello-world", "examples/helloworld/helloworld.ino"),
        ("factory-peripheral-test", "examples/factory/factory.ino"),
        (
            "lvgl-get-started",
            "examples/lvgl/get_started/get_started.ino",
        ),
        (
            "display-brightness",
            "examples/peripheral/DisplayBrightness/DisplayBrightness.ino",
        ),
        (
            "gnss",
            "examples/peripheral/GPSFullExample/GPSFullExample.ino",
        ),
        ("nfc", "examples/peripheral/NFC_Reader/NFC_Reader.ino"),
        ("sd-card", "examples/peripheral/SD_Test/SD_Test.ino"),
        (
            "power-monitor",
            "examples/power/PowerManageMonitor/PowerManageMonitor.ino",
        ),
        ("imu", "examples/sensor/BHI260AP_6DoF/BHI260AP_6DoF.ino"),
        ("radio-factory", "examples/factory/hw_sx1262.cpp"),
    ]
    .into_iter()
    .map(|(target, path)| demo("arduino", target, path))
    .collect::<Vec<_>>();
    demos.push(DemoRef {
        framework: "platformio".to_string(),
        target: "platformio-port".to_string(),
        source_url: "https://github.com/Xinyuan-LilyGO/LilyGoLib-PlatformIO".to_string(),
        path: "LilyGoLib-PlatformIO".to_string(),
        stale: false,
        source_status: "github-cache".to_string(),
        evidence_level: "V3-source-reference".to_string(),
        intents: Vec::new(),
        complexity: None,
        dependencies: Vec::new(),
        preferred_for: Vec::new(),
        avoid_for: Vec::new(),
    });
    demos
}

fn generic_demo_refs(slug: &str, repo: &RepoRecord) -> Vec<DemoRef> {
    let target = if slug.contains("display") {
        "display-examples"
    } else if slug.contains("lora") || slug.contains("beam") || slug.contains("t3") {
        "radio-examples"
    } else {
        "repository-examples"
    };
    vec![DemoRef {
        framework: "source".to_string(),
        target: target.to_string(),
        source_url: repo.html_url.clone(),
        path: "examples/ or docs/ in official repository".to_string(),
        stale: false,
        source_status: "github-cache".to_string(),
        evidence_level: "V3-source-reference".to_string(),
        intents: Vec::new(),
        complexity: None,
        dependencies: Vec::new(),
        preferred_for: Vec::new(),
        avoid_for: Vec::new(),
    }]
}

fn demo(framework: &str, target: &str, path: &str) -> DemoRef {
    DemoRef {
        framework: framework.to_string(),
        target: target.to_string(),
        source_url: format!("{LILYGOLIB_BLOB}/{path}"),
        path: path.to_string(),
        stale: false,
        source_status: "github-live-verified".to_string(),
        evidence_level: "V3-source-reference".to_string(),
        intents: Vec::new(),
        complexity: None,
        dependencies: Vec::new(),
        preferred_for: Vec::new(),
        avoid_for: Vec::new(),
    }
}

fn t_watch_ultra_peripherals() -> Vec<String> {
    strings(&[
        "display", "touch", "lora", "gnss", "nfc", "sensor", "power", "rtc", "audio", "haptic",
        "input", "storage", "memory",
    ])
}

fn inferred_peripherals(slug: &str) -> Vec<String> {
    const RULES: &[(&str, &[&str])] = &[
        (
            "display",
            &[
                "display", "watch", "deck", "e-paper", "epaper", "qt", "dongle",
            ],
        ),
        (
            "input",
            &[
                "watch", "display", "touch", "keyboard", "encoder", "knob", "deck",
            ],
        ),
        ("lora", &["lora", "beam", "t3", "echo"]),
        ("gps", &["gps", "gnss", "beam", "sim", "a7670"]),
        ("power", &["watch", "beam", "power", "solar"]),
        ("storage", &["sd", "display", "watch", "deck", "p4", "s3"]),
        ("audio", &["audio", "speaker", "watch", "twr"]),
    ];
    RULES
        .iter()
        .filter(|(_, needles)| contains_any(slug, needles))
        .map(|(name, _)| name.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn default_frameworks() -> Vec<String> {
    strings(&["arduino", "esp-idf", "rust", "platformio"])
}

fn family_for_slug(slug: &str) -> Option<String> {
    let family = if slug.starts_with("t-watch-") {
        "board-t-watch"
    } else if slug.starts_with("t-display-s3-") {
        "board-t-display-s3"
    } else if slug.starts_with("t-beam-") {
        "board-t-beam"
    } else if slug.starts_with("t-deck-") {
        "board-t-deck"
    } else if slug.starts_with("t-dongle-s3-") {
        "board-t-dongle-s3"
    } else {
        return None;
    };
    Some(family.to_string())
}

fn display_name_for_slug(slug: &str) -> String {
    format!("LilyGO {}", title_from_slug(slug))
}

fn title_from_slug(slug: &str) -> String {
    slug.split('-')
        .map(|part| match part {
            "t" => "T".to_string(),
            "s2" => "S2".to_string(),
            "s3" => "S3".to_string(),
            "c3" => "C3".to_string(),
            "p4" => "P4".to_string(),
            "gps" => "GPS".to_string(),
            "nfc" => "NFC".to_string(),
            "lora" => "LoRa".to_string(),
            "amoled" => "AMOLED".to_string(),
            "epaper" => "ePaper".to_string(),
            "id" => "ID".to_string(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn aliases_for_slug(slug: &str) -> Vec<String> {
    let spaced = slug.replace('-', " ");
    let compact = slug.replace('-', "");
    let mut aliases = vec![slug.to_string(), spaced, compact];
    if slug == "t-display-s3" {
        aliases.extend(strings(&["t-display", "t display", "tdisplay"]));
    }
    if slug == "t-dongle-s3" {
        aliases.extend(strings(&["t-dongle", "t dongle", "tdongle"]));
    }
    if slug == "t-watch-ultra" {
        aliases.push("twatch ultra".to_string());
        aliases.push("t watch ultra".to_string());
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn board_id_for_slug(slug: &str) -> String {
    format!("board-{slug}")
}

fn normalized_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn product_source_ingestion() {
        let root = root();
        let index = crate::source::load_board_index(root.as_path()).expect("board index");
        let candidates = product_candidates(root.as_path(), &index);
        assert!(candidates.len() >= 100);
        let ultra = candidates
            .iter()
            .find(|candidate| candidate.id == "board-t-watch-ultra")
            .expect("T-Watch Ultra candidate");
        assert!(ultra.supported);
        assert_eq!(ultra.family_id.as_deref(), Some("board-t-watch"));
        assert!(ultra.wiki_url.contains("/t-watch-ultra/"));
        assert!(ultra.repo_url.contains("LilyGoLib"));

        let records = product_records(root.as_path());
        let record = records
            .iter()
            .find(|record| record.id == "board-t-watch-ultra")
            .expect("T-Watch Ultra record");
        assert_eq!(record.mcu, "esp32-s3");
        assert!(!record.source_hashes.is_empty());
        assert!(
            record
                .source_urls
                .iter()
                .any(|source| source.kind == "hardware-doc")
        );

        let mut merged = merge_product_records(
            root.as_path(),
            crate::source::load_board_index(root.as_path()).expect("board index"),
        )
        .expect("merge with intact caches");
        let ultra = merged
            .boards
            .iter_mut()
            .find(|board| board.id == "board-t-watch-ultra")
            .expect("merged Ultra");
        ultra
            .source_hashes
            .insert("wiki".to_string(), "stale".to_string());
        let stale = stale_product_record_ids(root.as_path(), &merged);
        assert!(stale.contains(&"board-t-watch-ultra".to_string()));
    }

    #[test]
    fn product_matrix_generation() {
        let root = root();
        let record = product_records(root.as_path())
            .into_iter()
            .find(|record| record.id == "board-t-watch-ultra")
            .expect("T-Watch Ultra record");
        assert!(record.peripheral_matrix.len() >= 10);
        assert!(record.demo_refs.len() >= 8);
        assert!(
            record
                .peripheral_matrix
                .iter()
                .any(|entry| entry.chip.contains("MIA-M10Q"))
        );
        assert!(
            record
                .peripheral_matrix
                .iter()
                .any(|entry| entry.chip.contains("ST25R3916"))
        );
        assert!(
            record
                .demo_refs
                .iter()
                .any(|demo| demo.target == "lvgl-get-started")
        );
    }

    #[test]
    fn product_claim_guard() {
        let root = root();
        for record in product_records(root.as_path()) {
            assert!(
                !record
                    .peripherals
                    .iter()
                    .any(|peripheral| peripheral == "lvgl" || peripheral == "ota"),
                "{} must not treat LVGL or OTA as board peripherals",
                record.id
            );
            for entry in &record.peripheral_matrix {
                assert!(!entry.source_url.is_empty(), "{} matrix source", record.id);
                assert!(
                    !entry.source_status.is_empty(),
                    "{} matrix status",
                    record.id
                );
                assert!(
                    !entry.evidence_level.is_empty(),
                    "{} matrix evidence",
                    record.id
                );
            }
            for demo in &record.demo_refs {
                assert!(!demo.source_url.is_empty(), "{} demo source", record.id);
                assert!(!demo.source_status.is_empty(), "{} demo status", record.id);
                assert!(
                    !demo.evidence_level.is_empty(),
                    "{} demo evidence",
                    record.id
                );
            }
        }
    }
}
