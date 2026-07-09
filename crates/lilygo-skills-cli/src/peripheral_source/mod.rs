//! Peripheral source-pack indexing and generated peripheral/chip/feature skill
//! registration from board matrices and ranked source evidence.
use crate::generate::{generate_skills, generated_cache_root};
use crate::model::{
    BoardRecord, FeatureRef, FrameworkRef, PeripheralSkillUpdateReport, PeripheralSourcePack,
    PeripheralSourcePackIndex, Registry, RouteFixture, Skill, SkillKind, SourcePackSource,
    SourcePackSummary, SourcePackUpdateReport,
};
use crate::registry::load_registry;
use crate::source::{generated_skill_writes, load_board_index, write_if_changed};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(crate) const SOURCE_PACK_INDEX_PATH: &str = "data/peripherals/source-packs.json";

pub(crate) fn source_authority_rank(kind: &str) -> u32 {
    match kind {
        "chip-vendor" => 100,
        "lilygo-hardware" => 90,
        "lilygo-driver" => 85,
        "arduino-example" | "lilygo-example" => 80,
        "framework-official" => 70,
        "local-reference" => 60,
        "vetted-open-source" => 50,
        _ => 0,
    }
}

pub(crate) fn build_source_pack_index(root: &Path) -> Result<PeripheralSourcePackIndex, String> {
    let boards = load_board_index(root)?;
    let mut packs = Vec::new();
    for board in boards
        .boards
        .iter()
        .filter(|board| board.supported && board.product)
    {
        for peripheral in &board.peripheral_matrix {
            packs.push(pack_from_board_peripheral(board, peripheral));
        }
    }
    packs.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(PeripheralSourcePackIndex {
        schema_version: 1,
        packs,
    })
}

pub(crate) fn load_source_pack_index(root: &Path) -> Result<PeripheralSourcePackIndex, String> {
    let path = root.join(SOURCE_PACK_INDEX_PATH);
    if path.is_file() {
        let data = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        return serde_json::from_str(&data)
            .map_err(|error| format!("invalid {}: {error}", path.display()));
    }
    build_source_pack_index(root)
}

pub(crate) fn source_pack_preview(root: &Path) -> Result<SourcePackUpdateReport, String> {
    let index = build_source_pack_index(root)?;
    let stale = stale_source_pack_ids(root, &index);
    Ok(source_pack_report(index, true, stale, Vec::new()))
}

pub(crate) fn source_pack_apply(root: &Path) -> Result<SourcePackUpdateReport, String> {
    let index = build_source_pack_index(root)?;
    let stale = stale_source_pack_ids(root, &index);
    let rendered = serde_json::to_string_pretty(&index)
        .map_err(|error| format!("failed to render source packs: {error}"))?
        + "\n";
    let writes = if write_if_changed(&root.join(SOURCE_PACK_INDEX_PATH), rendered.as_bytes())? {
        vec![SOURCE_PACK_INDEX_PATH.to_string()]
    } else {
        Vec::new()
    };
    Ok(source_pack_report(index, false, stale, writes))
}

pub(crate) fn peripheral_skill_preview(
    root: &Path,
    generated_out: Option<&Path>,
) -> Result<PeripheralSkillUpdateReport, String> {
    let index = load_source_pack_index(root)?;
    let registry = update_registry_with_source_packs(load_registry(root)?, &index);
    Ok(peripheral_skill_report(
        root,
        &index,
        &registry,
        true,
        Vec::new(),
        generated_out,
    ))
}

pub(crate) fn peripheral_skill_apply(
    root: &Path,
    generated_out: Option<&Path>,
) -> Result<PeripheralSkillUpdateReport, String> {
    let index = load_source_pack_index(root)?;
    let registry = update_registry_with_source_packs(load_registry(root)?, &index);
    let out = generated_out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| generated_cache_root(root));
    let generated = generate_skills(root, &out)?;
    let mut warnings = shared_warnings();
    warnings.extend(generated.warnings);
    Ok(peripheral_skill_report_with_warnings(
        root,
        &index,
        &registry,
        false,
        generated_skill_writes(root, &out),
        Some(&out),
        warnings,
    ))
}

pub(crate) fn update_registry_with_source_packs(
    mut registry: Registry,
    index: &PeripheralSourcePackIndex,
) -> Registry {
    let generated_ids = generated_skills(index)
        .into_iter()
        .map(|skill| skill.id)
        .collect::<BTreeSet<_>>();
    registry
        .skills
        .retain(|skill| !is_generated_source_skill(skill, &generated_ids));
    ensure_canonical_peripheral_skills(&mut registry);
    registry.skills.extend(generated_skills(index));
    ensure_source_route_fixtures(&mut registry);
    registry
}

fn pack_from_board_peripheral(
    board: &BoardRecord,
    peripheral: &crate::model::PeripheralRecord,
) -> PeripheralSourcePack {
    let normalized_peripheral = normalized_peripheral(peripheral);
    let chip_slug = chip_slug(&peripheral.chip);
    let sources = ranked_sources(board, peripheral, &normalized_peripheral);
    let feature_refs = feature_refs(&normalized_peripheral, &peripheral.chip);
    PeripheralSourcePack {
        id: format!(
            "periph-pack-{}-{}-{}",
            board.id.trim_start_matches("board-"),
            normalized_peripheral,
            chip_slug
        ),
        board_id: board.id.clone(),
        peripheral: normalized_peripheral,
        chip: peripheral.chip.clone(),
        aliases: aliases(peripheral),
        sources,
        framework_refs: framework_refs(board, peripheral),
        feature_refs,
        warnings: pack_warnings(peripheral),
    }
}

fn ranked_sources(
    board: &BoardRecord,
    peripheral: &crate::model::PeripheralRecord,
    normalized_peripheral: &str,
) -> Vec<SourcePackSource> {
    let mut sources = Vec::new();
    if let Some(url) = chip_vendor_url(&peripheral.chip) {
        sources.push(source(
            "chip-vendor",
            url,
            "official-vendor-reference",
            false,
        ));
    }
    sources.push(source(
        "lilygo-hardware",
        &peripheral.source_url,
        &peripheral.source_status,
        false,
    ));
    if !peripheral.driver.is_empty() {
        let driver_url = driver_url(board).unwrap_or(peripheral.source_url.as_str());
        sources.push(source(
            "lilygo-driver",
            driver_url,
            &peripheral.source_status,
            false,
        ));
    }
    if let Some(demo) = preferred_arduino_demo(board, peripheral) {
        sources.push(source(
            "arduino-example",
            &demo.source_url,
            &demo.source_status,
            demo.stale,
        ));
    }
    if normalized_peripheral == "imu" {
        sources.push(source(
            "framework-official",
            "https://docs.espressif.com/projects/arduino-esp32/en/latest/",
            "official-framework-reference",
            false,
        ));
    }
    sources.sort_by_key(|source| Reverse(source.authority_rank));
    sources.dedup_by(|left, right| left.kind == right.kind && left.url == right.url);
    sources
}

fn preferred_arduino_demo<'a>(
    board: &'a BoardRecord,
    peripheral: &crate::model::PeripheralRecord,
) -> Option<&'a crate::model::DemoRef> {
    board
        .demo_refs
        .iter()
        .filter(|demo| demo.framework == "arduino")
        .filter(|demo| demo_matches_peripheral(&demo.target, peripheral))
        .min_by_key(|demo| {
            if demo.target == "factory-peripheral-test" {
                1
            } else {
                0
            }
        })
}

fn source(kind: &str, url: &str, status: &str, stale: bool) -> SourcePackSource {
    SourcePackSource {
        kind: kind.to_string(),
        authority_rank: source_authority_rank(kind),
        url: url.to_string(),
        evidence_level: "V3-source-reference".to_string(),
        stale,
        status: status.to_string(),
    }
}

fn framework_refs(
    board: &BoardRecord,
    peripheral: &crate::model::PeripheralRecord,
) -> Vec<FrameworkRef> {
    board
        .demo_refs
        .iter()
        .filter(|demo| demo_matches_peripheral(demo.target.as_str(), peripheral))
        .map(|demo| FrameworkRef {
            framework: demo.framework.clone(),
            target: demo.target.clone(),
            path: demo.path.clone(),
            source_url: demo.source_url.clone(),
            evidence_level: demo.evidence_level.clone(),
            stale: demo.stale,
        })
        .collect()
}

fn demo_matches_peripheral(target: &str, peripheral: &crate::model::PeripheralRecord) -> bool {
    let target = target.to_lowercase();
    let category = normalized_peripheral(peripheral);
    target.contains(&category)
        || target.contains(&peripheral.category.to_lowercase())
        || target.contains(&slug(&peripheral.chip))
        || target.contains("factory-peripheral-test")
}

fn feature_refs(peripheral: &str, chip: &str) -> Vec<FeatureRef> {
    if peripheral != "imu" || !chip.to_lowercase().contains("bhi260ap") {
        return Vec::new();
    }
    vec![FeatureRef {
        feature: "raise-to-wake".to_string(),
        guidance_level: "algorithmic-starting-point".to_string(),
        requires_calibration: true,
        hardware_verified: false,
        evidence_level: "V3-source-reference".to_string(),
    }]
}

fn pack_warnings(peripheral: &crate::model::PeripheralRecord) -> Vec<String> {
    let mut warnings = vec![
        "Source pack is context evidence, not build, flash, serial, OTA, LVGL render, or physical behavior evidence.".to_string(),
    ];
    if chip_vendor_url(&peripheral.chip).is_none() {
        warnings.push(format!(
            "No chip-vendor source registered for {}; LilyGO source remains authoritative for board integration.",
            peripheral.chip
        ));
    }
    warnings
}

fn chip_vendor_url(chip: &str) -> Option<&'static str> {
    let normalized = chip.to_lowercase();
    if normalized.contains("bhi260ap") {
        return Some("https://www.bosch-sensortec.com/products/smart-sensor-systems/bhi260ap/");
    }
    None
}

fn driver_url(board: &BoardRecord) -> Option<&str> {
    board
        .source_urls
        .iter()
        .find(|source| source.kind == "driver-header")
        .map(|source| source.url.as_str())
}

fn normalized_peripheral(peripheral: &crate::model::PeripheralRecord) -> String {
    let chip = peripheral.chip.to_lowercase();
    if chip.contains("bhi260ap") || peripheral.name.to_lowercase().contains("imu") {
        "imu".to_string()
    } else if peripheral.category == "radio" {
        "lora".to_string()
    } else if peripheral.category == "io" {
        "input".to_string()
    } else {
        peripheral.category.clone()
    }
}

fn aliases(peripheral: &crate::model::PeripheralRecord) -> Vec<String> {
    let mut aliases = BTreeSet::new();
    aliases.insert(normalized_peripheral(peripheral));
    aliases.insert(peripheral.category.to_lowercase());
    aliases.insert(peripheral.name.to_lowercase());
    aliases.insert(peripheral.chip.to_lowercase());
    aliases.insert(chip_slug(&peripheral.chip));
    aliases.insert(peripheral.driver.to_lowercase());
    if peripheral.chip.to_lowercase().contains("bhi260ap") {
        for alias in [
            "imu",
            "bhi260ap",
            "6dof",
            "gesture",
            "raise-to-wake",
            "tilt-to-wake",
            "抬腕",
        ] {
            aliases.insert(alias.to_string());
        }
    }
    aliases
        .into_iter()
        .filter(|alias| !alias.is_empty())
        .collect()
}

fn generated_skills(index: &PeripheralSourcePackIndex) -> Vec<Skill> {
    let mut skills = BTreeMap::new();
    for pack in &index.packs {
        if should_generate_peripheral_skill(pack) {
            skills.insert(peripheral_skill_id(pack), peripheral_skill(pack));
        }
        if should_generate_chip_skill(pack) {
            skills.insert(chip_skill_id(pack), chip_skill(pack));
        }
        for feature in &pack.feature_refs {
            skills.insert(feature_skill_id(feature), feature_skill(pack, feature));
        }
    }
    skills.into_values().collect()
}

fn peripheral_skill(pack: &PeripheralSourcePack) -> Skill {
    source_skill(
        peripheral_skill_id(pack),
        SkillKind::Peripheral,
        format!(
            "{} peripheral source-pack context with ranked board, chip, and framework references.",
            pack.peripheral
        ),
        peripheral_triggers(pack),
        74,
    )
}

fn chip_skill(pack: &PeripheralSourcePack) -> Skill {
    source_skill(
        chip_skill_id(pack),
        SkillKind::Chip,
        format!("{} chip context from source pack {}.", pack.chip, pack.id),
        chip_triggers(pack),
        73,
    )
}

fn feature_skill(pack: &PeripheralSourcePack, feature: &FeatureRef) -> Skill {
    source_skill(
        feature_skill_id(feature),
        SkillKind::Feature,
        format!(
            "{} feature guidance for {} using {} context.",
            feature.feature, pack.board_id, pack.chip
        ),
        feature_triggers(feature),
        64,
    )
}

fn source_skill(
    id: String,
    kind: SkillKind,
    summary: String,
    triggers: Vec<String>,
    priority: i32,
) -> Skill {
    Skill {
        path: format!("skills/{id}/SKILL.md"),
        id,
        kind,
        summary,
        triggers: dedup_preserve_order(triggers),
        aliases: Vec::new(),
        priority,
        verification_level: "context-injection".to_string(),
        family_id: None,
        product: false,
    }
}

fn should_generate_chip_skill(pack: &PeripheralSourcePack) -> bool {
    let chip = pack.chip.to_lowercase();
    if pack.peripheral == "memory" || pack.peripheral == "storage" {
        return false;
    }
    !chip.contains(" or ")
        && !chip.contains('+')
        && !chip.contains("up to")
        && !chip.contains("flash")
        && !chip.contains("psram")
        && !chip.contains("microsd")
}

fn dedup_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn peripheral_triggers(pack: &PeripheralSourcePack) -> Vec<String> {
    match pack.peripheral.as_str() {
        "imu" => vec!["imu", "6dof", "accelerometer", "gyroscope", "gesture"],
        other => vec![other],
    }
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn chip_triggers(pack: &PeripheralSourcePack) -> Vec<String> {
    vec![pack.chip.to_lowercase(), chip_slug(&pack.chip)]
}

fn feature_triggers(feature: &FeatureRef) -> Vec<String> {
    if feature.feature == "raise-to-wake" {
        return ["raise-to-wake", "tilt-to-wake", "wrist raise", "抬腕"]
            .into_iter()
            .map(str::to_string)
            .collect();
    }
    vec![feature.feature.clone()]
}

// Rendered (skill_id, SKILL.md) pairs for every generated peripheral/chip/feature
// skill, used by `generate skills --out <dir>` to write into a generated cache.
pub(crate) fn generated_skill_files(root: &Path) -> Result<Vec<(String, String)>, String> {
    let index = load_source_pack_index(root)?;
    Ok(generated_skills(&index)
        .into_iter()
        .map(|skill| {
            let content = render_generated_skill(&skill, &index);
            (skill.id.clone(), content)
        })
        .collect())
}

mod render;
pub(crate) use render::*;

fn is_generated_source_skill(skill: &Skill, generated_ids: &BTreeSet<String>) -> bool {
    generated_ids.contains(&skill.id)
        || matches!(skill.kind, SkillKind::Chip | SkillKind::Feature)
        || skill.id == "periph-imu"
        || (skill.kind == SkillKind::Peripheral && skill.summary.contains("source-pack context"))
}

fn should_generate_peripheral_skill(pack: &PeripheralSourcePack) -> bool {
    pack.peripheral == "imu"
}

fn ensure_canonical_peripheral_skills(registry: &mut Registry) {
    for skill in canonical_peripheral_skills() {
        if let Some(existing) = registry
            .skills
            .iter_mut()
            .find(|existing| existing.id == skill.id)
        {
            *existing = skill;
        } else {
            registry.skills.push(skill);
        }
    }
}

fn canonical_peripheral_skills() -> Vec<Skill> {
    vec![
        canonical_peripheral(
            "periph-display",
            "Display, backlight, panel, touch, and blank-screen context with official board source pointers.",
            &[
                "display",
                "screen",
                "lcd",
                "touch",
                "blank",
                "backlight",
                "amoled",
            ],
            60,
        ),
        canonical_peripheral(
            "periph-lora",
            "LoRa, SX126x/SX127x, region, antenna, and Meshtastic-style context.",
            &[
                "lora",
                "sx1262",
                "sx1268",
                "sx1276",
                "sx1278",
                "sx1280",
                "meshtastic",
            ],
            60,
        ),
        canonical_peripheral(
            "periph-gps",
            "GPS/GNSS fix, UART, antenna, and serial evidence context.",
            &["gps", "gnss", "ublox", "nmea"],
            60,
        ),
        canonical_peripheral(
            "periph-power",
            "Battery, PMU, charging, sleep, and power rail context for LilyGO boards.",
            &["battery", "pmu", "power", "charging", "sleep"],
            58,
        ),
        canonical_peripheral(
            "periph-cellular",
            "Cellular modem, SIM, PPP, LTE, and AT command context for LilyGO boards.",
            &["cellular", "lte", "sim", "ppp", "modem", "at command"],
            58,
        ),
        canonical_peripheral(
            "periph-input",
            "Touch, buttons, keyboard, encoder, and input event context.",
            &["touch", "button", "keyboard", "encoder", "input"],
            58,
        ),
        canonical_peripheral(
            "periph-storage",
            "SD, SPIFFS, LittleFS, partition, and asset storage context.",
            &[
                "sd",
                "tf",
                "spiffs",
                "littlefs",
                "storage",
                "partition",
                "存储",
            ],
            58,
        ),
        canonical_peripheral(
            "periph-audio",
            "I2S microphone, speaker, codec, and audio pipeline context.",
            &["audio", "i2s", "microphone", "speaker", "codec"],
            58,
        ),
    ]
}

fn canonical_peripheral(id: &str, summary: &str, triggers: &[&str], priority: i32) -> Skill {
    source_skill(
        id.to_string(),
        SkillKind::Peripheral,
        summary.to_string(),
        strings(triggers),
        priority,
    )
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn peripheral_skill_report(
    root: &Path,
    index: &PeripheralSourcePackIndex,
    registry: &Registry,
    dry_run: bool,
    writes: Vec<String>,
    generated_out: Option<&Path>,
) -> PeripheralSkillUpdateReport {
    peripheral_skill_report_with_warnings(
        root,
        index,
        registry,
        dry_run,
        writes,
        generated_out,
        shared_warnings(),
    )
}

fn peripheral_skill_report_with_warnings(
    root: &Path,
    index: &PeripheralSourcePackIndex,
    registry: &Registry,
    dry_run: bool,
    writes: Vec<String>,
    generated_out: Option<&Path>,
    warnings: Vec<String>,
) -> PeripheralSkillUpdateReport {
    let skill_ids = generated_skills(index)
        .into_iter()
        .map(|skill| skill.id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let route_fixture_ids = registry
        .route_fixtures
        .iter()
        .filter(|fixture| fixture.id.contains("raise-to-wake") || fixture.id.contains("bhi260ap"))
        .map(|fixture| fixture.id.clone())
        .collect();
    PeripheralSkillUpdateReport {
        status: "PASS".to_string(),
        dry_run,
        source_pack_count: index.packs.len(),
        generated_skill_count: skill_ids.len(),
        generated_route_count: skill_ids.len(),
        stale_source_packs: stale_source_pack_ids(root, index),
        planned_writes: if dry_run {
            generated_out
                .map(|out| generated_skill_writes(root, out))
                .unwrap_or_else(crate::generate::default_generated_cache_writes)
        } else {
            Vec::new()
        },
        writes,
        skill_ids,
        route_fixture_ids,
        warnings,
    }
}

fn source_pack_report(
    index: PeripheralSourcePackIndex,
    dry_run: bool,
    stale: Vec<String>,
    writes: Vec<String>,
) -> SourcePackUpdateReport {
    SourcePackUpdateReport {
        status: "PASS".to_string(),
        dry_run,
        source_pack_count: index.packs.len(),
        stale_source_packs: stale,
        planned_writes: if dry_run {
            vec![SOURCE_PACK_INDEX_PATH.to_string()]
        } else {
            Vec::new()
        },
        writes,
        packs: index.packs.iter().map(pack_summary).collect(),
        warnings: shared_warnings(),
    }
}

fn pack_summary(pack: &PeripheralSourcePack) -> SourcePackSummary {
    SourcePackSummary {
        id: pack.id.clone(),
        board_id: pack.board_id.clone(),
        peripheral: pack.peripheral.clone(),
        chip: pack.chip.clone(),
        source_dimensions: pack
            .sources
            .iter()
            .map(|source| source.kind.clone())
            .collect(),
    }
}

fn stale_source_pack_ids(root: &Path, generated: &PeripheralSourcePackIndex) -> Vec<String> {
    let path = root.join(SOURCE_PACK_INDEX_PATH);
    let Ok(data) = fs::read_to_string(path) else {
        return generated.packs.iter().map(|pack| pack.id.clone()).collect();
    };
    let Ok(existing) = serde_json::from_str::<PeripheralSourcePackIndex>(&data) else {
        return generated.packs.iter().map(|pack| pack.id.clone()).collect();
    };
    let existing = existing
        .packs
        .into_iter()
        .map(|pack| (pack.id.clone(), pack))
        .collect::<BTreeMap<_, _>>();
    generated
        .packs
        .iter()
        .filter(|pack| {
            existing
                .get(&pack.id)
                .is_none_or(|old| !pack_equal(old, pack))
        })
        .map(|pack| pack.id.clone())
        .collect()
}

fn pack_equal(left: &PeripheralSourcePack, right: &PeripheralSourcePack) -> bool {
    serde_json::to_value(left).ok() == serde_json::to_value(right).ok()
}

fn shared_warnings() -> Vec<String> {
    vec![
        "Source/demo links are V3 context evidence only.".to_string(),
        "hardware_verified remains false until simulator or real-device evidence is attached."
            .to_string(),
    ]
}

fn peripheral_skill_id(pack: &PeripheralSourcePack) -> String {
    format!("periph-{}", pack.peripheral)
}

fn chip_skill_id(pack: &PeripheralSourcePack) -> String {
    format!("chip-{}", chip_slug(&pack.chip))
}

fn feature_skill_id(feature: &FeatureRef) -> String {
    format!("feature-{}", slug(&feature.feature))
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn chip_slug(value: &str) -> String {
    let normalized = value
        .to_lowercase()
        .replace("bosch ", "")
        .replace("u-blox ", "")
        .replace("espressif ", "");
    slug(&normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn peripheral_source_pack_schema() {
        let index = build_source_pack_index(root().as_path()).expect("source packs");
        let pack = index
            .packs
            .iter()
            .find(|pack| pack.id == "periph-pack-t-watch-ultra-imu-bhi260ap")
            .expect("BHI260AP pack");
        assert_eq!(pack.board_id, "board-t-watch-ultra");
        assert_eq!(pack.peripheral, "imu");
        assert!(pack.aliases.iter().any(|alias| alias == "抬腕"));
        assert!(pack.framework_refs.iter().any(|reference| {
            reference.framework == "arduino" && reference.path.contains("BHI260AP_6DoF")
        }));
        assert!(
            pack.feature_refs
                .iter()
                .any(|feature| feature.feature == "raise-to-wake" && !feature.hardware_verified)
        );
    }

    #[test]
    fn source_authority_scoring() {
        assert!(source_authority_rank("chip-vendor") > source_authority_rank("lilygo-driver"));
        assert!(source_authority_rank("lilygo-driver") > source_authority_rank("lilygo-example"));
        let index = build_source_pack_index(root().as_path()).expect("source packs");
        let pack = index
            .packs
            .iter()
            .find(|pack| pack.id == "periph-pack-t-watch-ultra-imu-bhi260ap")
            .expect("BHI260AP pack");
        let dimensions = pack
            .sources
            .iter()
            .map(|source| source.kind.as_str())
            .collect::<BTreeSet<_>>();
        // Source packs must cite public authority dimensions so generated
        // skills can be redistributed without private repository context.
        for required in [
            "chip-vendor",
            "lilygo-hardware",
            "lilygo-driver",
            "framework-official",
        ] {
            assert!(dimensions.contains(required), "missing {required}");
        }
    }

    #[test]
    fn generated_peripheral_subskills() {
        let index = build_source_pack_index(root().as_path()).expect("source packs");
        let registry =
            update_registry_with_source_packs(load_registry(root().as_path()).unwrap(), &index);
        for skill_id in ["periph-imu", "chip-bhi260ap"] {
            assert!(
                registry.skills.iter().any(|skill| skill.id == skill_id),
                "{skill_id}"
            );
        }
    }

    #[test]
    fn generated_chip_skills_are_real_chips() {
        let index = build_source_pack_index(root().as_path()).expect("source packs");
        let skills = generated_skills(&index);
        let ids = skills
            .iter()
            .map(|skill| skill.id.as_str())
            .collect::<BTreeSet<_>>();
        for pseudo in [
            "chip-16mb-flash-8mb-psram",
            "chip-microsd-up-to-32gb-fat32",
            "chip-sx1262-or-sx1280",
        ] {
            assert!(!ids.contains(pseudo), "pseudo chip skill leaked: {pseudo}");
        }
        for real in ["chip-bhi260ap", "chip-st25r3916", "chip-axp2101"] {
            assert!(ids.contains(real), "missing real chip skill: {real}");
        }
        for skill in skills {
            let unique = skill.triggers.iter().collect::<BTreeSet<_>>();
            assert_eq!(
                unique.len(),
                skill.triggers.len(),
                "duplicate triggers for {}",
                skill.id
            );
        }
    }

    #[test]
    fn template_peripheral_skill_contract_marker() {
        let files = generated_skill_files(root().as_path()).expect("generated source skills");
        let (_, content) = files
            .iter()
            .find(|(id, _)| id == "periph-imu")
            .expect("imu generated skill");
        assert!(content.contains("Generation Contract: templates/skills/peripheral.md"));
        assert!(!content.contains("{{"));
    }

    #[test]
    fn feature_skill_generation() {
        let index = build_source_pack_index(root().as_path()).expect("source packs");
        let registry =
            update_registry_with_source_packs(load_registry(root().as_path()).unwrap(), &index);
        let skill = registry
            .skills
            .iter()
            .find(|skill| skill.id == "feature-raise-to-wake")
            .expect("feature skill");
        assert_eq!(skill.kind, SkillKind::Feature);
        assert!(skill.triggers.iter().any(|trigger| trigger == "抬腕"));
    }
}
