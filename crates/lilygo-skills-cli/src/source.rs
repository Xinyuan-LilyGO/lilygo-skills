//! Board index loading and the shared idempotent file writer. Board/source
//! synchronization and the generated-skill cache lived here until the generation
//! stack was removed; the JS pipeline (pipeline/**) now owns data regeneration.
use crate::model::BoardIndex;
use std::fs;
use std::path::Path;

pub(crate) const BOARD_INDEX_PATH: &str = "data/boards.json";

pub fn load_board_index(root: &Path) -> Result<BoardIndex, String> {
    let path = root.join(BOARD_INDEX_PATH);
    let data = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&data).map_err(|error| format!("invalid {}: {error}", path.display()))
}

pub(crate) fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool, String> {
    if fs::read(path)
        .map(|existing| existing == bytes)
        .unwrap_or(false)
    {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn board_index_loads() {
        let index = load_board_index(root().as_path()).expect("board index");
        assert!(
            index
                .boards
                .iter()
                .any(|board| board.id == "board-t-display-s3")
        );
    }
}
