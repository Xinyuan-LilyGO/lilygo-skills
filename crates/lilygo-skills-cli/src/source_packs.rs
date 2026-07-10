//! Read-only access to the peripheral source-pack index
//! (`data/peripherals/source-packs.json`) and its source-authority ranking.
//!
//! The index is generated offline by the JS pipeline and committed; the capsule
//! assembly reads it to attach ranked peripheral source refs. Retained after the
//! generation stack (which used to build this index in-process) was removed.

use crate::model::PeripheralSourcePackIndex;
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

/// Load the committed peripheral source-pack index. A missing file yields an
/// empty index (the capsule simply attaches no extra peripheral source refs)
/// rather than an error, matching the previous non-fatal behavior.
pub(crate) fn load_source_pack_index(root: &Path) -> Result<PeripheralSourcePackIndex, String> {
    let path = root.join(SOURCE_PACK_INDEX_PATH);
    if !path.is_file() {
        return Ok(PeripheralSourcePackIndex {
            schema_version: 1,
            packs: Vec::new(),
        });
    }
    let data = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&data).map_err(|error| format!("invalid {}: {error}", path.display()))
}
