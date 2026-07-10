//! Best-effort board sniffing for projects that have no
//! `.lilygo-skills/project.json` profile.
//!
//! The `context` command wants to know which LilyGO board a project targets
//! even before the user has run `project init`. This module reads the common
//! build-config files a firmware project already carries and, if the evidence
//! unambiguously names a board that is KNOWN to the registry, returns that
//! board's skill id. It NEVER guesses: when the evidence is missing, unknown,
//! or points at more than one board, it returns `None` so the caller injects no
//! board context rather than a fabricated one (honesty over coverage).
//!
//! Detection order (highest-signal source first):
//!   (a) `platformio.ini`        -- `board = ...` values and `[env:...]` names
//!   (b) `sdkconfig` / `sdkconfig.defaults` -- ESP-IDF config lines
//!   (c) `*.ino` and `src/*.{cpp,h}` -- `#include` lines and comments
//!
//! Matching is deliberately conservative. Each source contributes candidate
//! tokens; every token is normalized (lowercased, non-alphanumerics stripped)
//! and tested against the normalized `id`/`triggers`/`aliases` of every Board
//! skill. To avoid short, generic aliases (e.g. the `s3` alias on the
//! esp32-s3 *series* board) swallowing everything, a match only counts when the
//! matched alias is at least `MIN_ALIAS_LEN` chars, and when several boards
//! match we keep only the one whose matched alias is strictly the longest
//! (most specific). If two different boards tie on specificity, or nothing
//! matches, the result is ambiguous and we assign nothing.
//!
//! Limits (documented on purpose): we only read files at the project root (plus
//! `src/`), cap how many/large files we read, and rely on the registry alias
//! lists -- a board with no distinctive alias, or a project whose only signal is
//! a bare chip target like `esp32s3`, will correctly resolve to "unknown".

use crate::model::{Registry, SkillKind};
use std::fs;
use std::path::Path;

/// Shortest matched alias length that is allowed to identify a board. Filters
/// out generic two/three-char aliases (e.g. `s3`) that would match unrelated
/// project strings.
const MIN_ALIAS_LEN: usize = 4;
/// Cap on bytes read from any single sniffed file. Build configs and headers
/// are small; this only bounds pathological inputs.
const MAX_FILE_BYTES: usize = 64 * 1024;
/// Cap on how many source files (*.ino / src/*.{cpp,h}) we scan.
const MAX_SOURCE_FILES: usize = 16;

/// Detect the board skill id for `project_root`, or `None` when the evidence is
/// absent or ambiguous. `project_root` is the directory `context` was asked to
/// inspect (typically the firmware project root).
pub(crate) fn sniff_board(project_root: &Path, registry: &Registry) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    collect_platformio(project_root, &mut candidates);
    collect_sdkconfig(project_root, &mut candidates);
    collect_sources(project_root, &mut candidates);
    resolve_board(&candidates, registry)
}

/// (a) platformio.ini: harvest `board = <value>` right-hand sides and the
/// `[env:<name>]` section names -- both routinely encode the board.
fn collect_platformio(project_root: &Path, out: &mut Vec<String>) {
    let Some(text) = read_capped(&project_root.join("platformio.ini")) else {
        return;
    };
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(value) = line.strip_prefix("board").and_then(|rest| {
            // Accept `board = x` and `board=x`; reject `board_build.mcu = x` etc.
            let rest = rest.trim_start();
            rest.strip_prefix('=').map(str::trim)
        }) {
            out.push(value.to_string());
        }
        if let Some(env) = line
            .strip_prefix("[env:")
            .and_then(|rest| rest.strip_suffix(']'))
        {
            out.push(env.to_string());
        }
    }
}

/// (b) sdkconfig / sdkconfig.defaults: ESP-IDF projects rarely name the LilyGO
/// board directly, but some carry a board hint in a comment or a custom
/// `CONFIG_..._BOARD` symbol. We add every config token and let the strict
/// alias matcher decide -- unknown targets simply resolve to `None`.
fn collect_sdkconfig(project_root: &Path, out: &mut Vec<String>) {
    for name in ["sdkconfig", "sdkconfig.defaults"] {
        let Some(text) = read_capped(&project_root.join(name)) else {
            continue;
        };
        for raw in text.lines() {
            let line = raw.trim();
            // Comments frequently name the target board ("# T-Display-S3").
            if let Some(comment) = line.strip_prefix('#') {
                out.push(comment.trim().to_string());
            }
            if let Some((key, value)) = line.split_once('=')
                && key.contains("BOARD")
            {
                out.push(value.trim().trim_matches('"').to_string());
            }
        }
    }
}

/// (c) *.ino / src/*.{cpp,h}: include lines and comments sometimes name the
/// board (e.g. `#include "LilyGo_T_Display_S3.h"` or `// Board: T-Beam`). Only
/// `#include`/comment lines are considered so ordinary code identifiers do not
/// leak in as candidates.
fn collect_sources(project_root: &Path, out: &mut Vec<String>) {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_source_paths(project_root, &mut files);
    collect_source_paths(&project_root.join("src"), &mut files);
    for path in files.into_iter().take(MAX_SOURCE_FILES) {
        let Some(text) = read_capped(&path) else {
            continue;
        };
        for raw in text.lines() {
            let line = raw.trim();
            if line.starts_with("#include") || line.starts_with("//") || line.starts_with("/*") {
                out.push(line.to_string());
            }
        }
    }
}

fn collect_source_paths(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(ext, "ino" | "cpp" | "h") {
            out.push(path);
        }
    }
}

fn read_capped(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let slice = &bytes[..bytes.len().min(MAX_FILE_BYTES)];
    Some(String::from_utf8_lossy(slice).into_owned())
}

/// Reduce the collected candidate strings to at most one KNOWN board id.
///
/// For each Board skill we compute its best specificity score: the length of
/// the longest of its aliases that any candidate string contains. The winner is
/// the board with the strictly-highest score; if two boards share the top score
/// the signal is ambiguous and we return `None` (never guess). Order of the
/// candidates does not affect the outcome.
fn resolve_board(candidates: &[String], registry: &Registry) -> Option<String> {
    let normalized: Vec<String> = candidates
        .iter()
        .map(|candidate| normalize(candidate))
        .filter(|candidate| !candidate.is_empty())
        .collect();
    // Highest alias-length score achieved by each matching board id.
    let mut scores: Vec<(&str, usize)> = Vec::new();
    for skill in &registry.skills {
        if skill.kind != SkillKind::Board {
            continue;
        }
        // The alias set includes the id and triggers as well as aliases.
        let score = std::iter::once(skill.id.as_str())
            .chain(skill.triggers.iter().map(String::as_str))
            .chain(skill.aliases.iter().map(String::as_str))
            .map(normalize)
            .filter(|alias| {
                alias.len() >= MIN_ALIAS_LEN
                    && normalized
                        .iter()
                        .any(|candidate| candidate.contains(alias.as_str()))
            })
            .map(|alias| alias.len())
            .max();
        if let Some(score) = score {
            scores.push((skill.id.as_str(), score));
        }
    }
    let top = scores.iter().map(|(_, score)| *score).max()?;
    let mut winners = scores.iter().filter(|(_, score)| *score == top);
    let winner = winners.next()?.0;
    // A single board must own the top score; a tie is ambiguous -> assign none.
    if winners.next().is_some() {
        return None;
    }
    Some(winner.to_string())
}

/// Lowercase and drop every non-alphanumeric char so `T-Display-S3`,
/// `t display s3`, and `lilygo-t-display-s3` all collapse to comparable forms.
fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::load_registry;
    use std::path::PathBuf;

    fn registry() -> Registry {
        load_registry(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
        )
        .unwrap()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lilygo-sniff-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn platformio_board_value_resolves_known_board() {
        let dir = temp_dir("pio-board");
        fs::write(
            dir.join("platformio.ini"),
            "[env:t-display-s3]\nplatform = espressif32\nboard = lilygo-t-display-s3\n",
        )
        .unwrap();
        assert_eq!(
            sniff_board(&dir, &registry()).as_deref(),
            Some("board-t-display-s3")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ambiguous_multiple_boards_resolve_to_none() {
        let dir = temp_dir("pio-ambiguous");
        // Two env sections naming two distinct boards -> no honest single answer.
        fs::write(
            dir.join("platformio.ini"),
            "[env:t-beam]\nboard = ttgo-t-beam\n\n[env:t-deck]\nboard = t-deck\n",
        )
        .unwrap();
        assert_eq!(sniff_board(&dir, &registry()), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_board_token_resolves_to_none() {
        let dir = temp_dir("pio-unknown");
        fs::write(
            dir.join("platformio.ini"),
            "[env:generic]\nboard = esp32dev\n",
        )
        .unwrap();
        assert_eq!(sniff_board(&dir, &registry()), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_config_files_resolve_to_none() {
        let dir = temp_dir("empty");
        assert_eq!(sniff_board(&dir, &registry()), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
