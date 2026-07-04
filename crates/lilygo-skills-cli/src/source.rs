//! Board/source synchronization and update planning for cached source intake,
//! generated skill cache writes, and board registry previews.
use crate::generate::{default_generated_cache_writes, generate_skills, generated_cache_root};
use crate::model::{BoardIndex, BoardRecord, SourceModeReport, SyncPreview, UpdatePreview};
use crate::product_source::{merge_product_records, product_candidates, stale_product_record_ids};
use crate::source_generation::install_runtime;
use crate::source_refresh::{refresh_manifest_hashes, refresh_source_cache};
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::Path;

pub(crate) const BOARD_INDEX_PATH: &str = "data/boards.json";
pub(crate) const REPO_CACHE_PATH: &str = "data/references/source-intake/raw/lilygo-repos.json.gz";
pub(crate) const WIKI_PRODUCTS_CACHE_PATH: &str =
    "data/references/source-intake/raw/wiki-products.json";
pub(crate) const SOURCE_MANIFEST_PATH: &str = "data/references/source-intake/manifest.md";

#[derive(Debug, Deserialize)]
struct RepoCache {
    count: usize,
}

pub fn load_board_index(root: &Path) -> Result<BoardIndex, String> {
    let path = root.join(BOARD_INDEX_PATH);
    let data = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&data).map_err(|error| format!("invalid {}: {error}", path.display()))
}

pub fn sync_preview(root: &Path) -> SyncPreview {
    let board_index = load_board_index(root).unwrap_or(BoardIndex {
        schema_version: 1,
        boards: Vec::new(),
    });
    sync_report(root, board_index, true, Vec::new())
}

pub fn sync_apply(root: &Path) -> Result<SyncPreview, String> {
    let board_index = merge_product_records(
        root,
        board_index_with_cached_wiki(root, load_board_index(root)?),
    )?;
    let path = root.join(BOARD_INDEX_PATH);
    let content = serde_json::to_string_pretty(&board_index)
        .map_err(|error| format!("failed to render board index: {error}"))?
        + "\n";
    let writes = if write_if_changed(&path, content.as_bytes())? {
        vec![BOARD_INDEX_PATH.to_string()]
    } else {
        Vec::new()
    };
    Ok(sync_report(root, board_index, false, writes))
}

fn board_index_with_cached_wiki(root: &Path, mut board_index: BoardIndex) -> BoardIndex {
    let links = wiki_product_urls(root);
    for board in board_index
        .boards
        .iter_mut()
        .filter(|board| board.supported)
    {
        let Some(url) = wiki_url_for_board(board, &links) else {
            continue;
        };
        board.wiki_url = url;
        board.source_status = board
            .source_status
            .replace("wiki-inferred", "wiki-cache")
            .replace("wiki-index", "wiki-cache");
        board.warnings.retain(|warning| {
            !warning.contains("Wiki page URL is inferred")
                && !warning.contains("Wiki page content not mirrored")
        });
    }
    board_index
}

fn wiki_product_urls(root: &Path) -> Vec<String> {
    let path = root.join(WIKI_PRODUCTS_CACHE_PATH);
    let Ok(data) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
        return Vec::new();
    };
    value
        .get("products")
        .and_then(|products| products.as_array())
        .into_iter()
        .flatten()
        .filter_map(|product| {
            product
                .get("url")
                .and_then(|url| url.as_str())
                .map(str::to_string)
        })
        .collect()
}

fn wiki_url_for_board(board: &BoardRecord, urls: &[String]) -> Option<String> {
    if board.id == "series-esp32-s3" {
        return Some("https://wiki.lilygo.cc/products/".to_string());
    }
    if board.id == "board-t-watch" {
        return urls
            .iter()
            .find(|url| url.contains("/products/t-watch-series/t-watch-2019/"))
            .cloned();
    }
    board
        .aliases
        .iter()
        .map(|alias| alias.replace(' ', "-"))
        .find_map(|slug| {
            urls.iter()
                .find(|url| url.contains(&format!("/{slug}/")))
                .cloned()
        })
}

pub fn update_preview(
    root: &Path,
    target: &str,
    generated_out: Option<&Path>,
) -> Result<UpdatePreview, String> {
    update_report(root, target, true, Vec::new(), Vec::new(), generated_out)
}

pub fn update_apply(
    root: &Path,
    target: &str,
    home: Option<&Path>,
    generated_out: Option<&Path>,
) -> Result<UpdatePreview, String> {
    if target == "boards" {
        let sync = sync_apply(root)?;
        return Ok(update_report_from_sync(sync, false));
    }

    let mut report = update_preview(root, target, generated_out)?;
    report.dry_run = false;
    report.compatibility_notes = vec![
        "apply mode performs the declared write path and reports changed files".to_string(),
        "idempotent apply reports an empty writes list when content is already current".to_string(),
        "hardware, OTA, LVGL, flash, and serial success require V4/V5 evidence".to_string(),
    ];

    match target {
        "sources" => {
            let source_refresh = refresh_source_cache(root, &mut report.warnings)?;
            let mut writes = source_refresh.writes;
            let manifest_writes = refresh_manifest_hashes(root, &writes)?;
            writes.extend(manifest_writes);
            report.writes = writes;
            if source_refresh.live {
                report.cache_status = "cache-refreshed".to_string();
                report.stale_status = "current".to_string();
            } else {
                report.cache_status = "cache-present".to_string();
                report.stale_status = "live-unavailable-cache-fallback".to_string();
            }
        }
        "skills" => {
            let out = generated_out
                .map(Path::to_path_buf)
                .unwrap_or_else(|| generated_cache_root(root));
            let generated = generate_skills(root, &out)?;
            report.cache_status = "generated-cache-written".to_string();
            report.stale_status = generated.status.to_lowercase();
            report.writes = generated_skill_writes(root, &out);
            report.warnings.extend(generated.warnings);
        }
        "runtime" => report.writes = install_runtime(root, home)?,
        other => return Err(format!("unknown update target: {other}")),
    }

    Ok(report)
}

pub fn repo_count_from_cache(root: &Path) -> Result<usize, String> {
    let path = root.join(REPO_CACHE_PATH);
    let file = fs::File::open(&path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut decoder = GzDecoder::new(file);
    let mut data = String::new();
    decoder
        .read_to_string(&mut data)
        .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;
    let cache: RepoCache =
        serde_json::from_str(&data).map_err(|error| format!("invalid repo cache: {error}"))?;
    Ok(cache.count)
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

fn sync_report(
    root: &Path,
    board_index: BoardIndex,
    dry_run: bool,
    writes: Vec<String>,
) -> SyncPreview {
    let repo_count = repo_count_from_cache(root).unwrap_or(0);
    let candidates: Vec<BoardRecord> = board_index
        .boards
        .iter()
        .filter(|board| board.supported)
        .cloned()
        .collect();
    let unsupported_count = board_index
        .boards
        .iter()
        .filter(|board| !board.supported)
        .count();
    let warnings: Vec<String> = board_index
        .boards
        .iter()
        .flat_map(|board| board.warnings.clone())
        .collect();
    let product_candidates = product_candidates(root, &board_index);

    SyncPreview {
        status: "PASS".to_string(),
        mode: if dry_run { "dry-run" } else { "apply" }.to_string(),
        dry_run,
        sources: SourceModeReport {
            github_org: if repo_count > 0 {
                "cache"
            } else {
                "missing-cache"
            }
            .to_string(),
            wiki: "inferred-url-cache".to_string(),
            documentation_repo: "reference-only".to_string(),
        },
        source_count: count_source_artifacts(root),
        repo_count,
        wiki_page_count: board_index
            .boards
            .iter()
            .filter(|board| board.source_status.contains("wiki"))
            .count(),
        generated_candidate_count: candidates.len(),
        product_candidate_count: product_candidates.len(),
        unsupported_count,
        candidate_route_ids: candidates.iter().map(|board| board.id.clone()).collect(),
        candidates,
        product_candidates,
        planned_writes: if dry_run {
            Vec::new()
        } else {
            vec![BOARD_INDEX_PATH.to_string()]
        },
        writes,
        warnings,
        source_manifest: root.join(SOURCE_MANIFEST_PATH).display().to_string(),
    }
}

fn update_report(
    root: &Path,
    target: &str,
    dry_run: bool,
    writes: Vec<String>,
    warnings: Vec<String>,
    generated_out: Option<&Path>,
) -> Result<UpdatePreview, String> {
    let sync = sync_preview(root);
    let stale_product_records = stale_product_record_ids(root, &load_board_index(root)?);
    let planned_fetches = planned_fetches(target)?;
    let planned_writes = planned_writes(root, target, generated_out)?;
    let cache_status = if sync.repo_count > 0 && sync.source_count > 0 {
        "cache-present"
    } else {
        "cache-incomplete"
    };
    let stale_status = if sync.sources.github_org == "cache" {
        "refresh-available"
    } else {
        "source-boundary"
    };

    Ok(UpdatePreview {
        status: "PASS".to_string(),
        target: target.to_string(),
        dry_run,
        source_families: vec![
            "lilygo-github".to_string(),
            "lilygo-wiki".to_string(),
            "lilygo-documentation-repo".to_string(),
            "auxiliary-tool-refs".to_string(),
        ],
        cache_status: cache_status.to_string(),
        stale_status: stale_status.to_string(),
        source_count: sync.source_count,
        board_count: sync.candidates.len() + sync.unsupported_count,
        generated_candidate_count: sync.generated_candidate_count,
        product_candidate_count: sync.product_candidate_count,
        unsupported_count: sync.unsupported_count,
        product_candidates: sync.product_candidates,
        stale_product_records,
        planned_fetches,
        planned_writes,
        writes,
        warnings,
        compatibility_notes: vec![
            "dry-run is non-mutating and reports concrete writes for the matching apply path"
                .to_string(),
            "generated skills are written only to an install root, project cache, or explicit generated output".to_string(),
            "hardware, OTA, LVGL, flash, and serial success require V4/V5 evidence".to_string(),
        ],
    })
}

fn update_report_from_sync(sync: SyncPreview, dry_run: bool) -> UpdatePreview {
    UpdatePreview {
        status: sync.status,
        target: "boards".to_string(),
        dry_run,
        source_families: vec![
            "lilygo-github".to_string(),
            "lilygo-wiki".to_string(),
            "lilygo-documentation-repo".to_string(),
            "auxiliary-tool-refs".to_string(),
        ],
        cache_status: "cache-present".to_string(),
        stale_status: "board-index-applied".to_string(),
        source_count: sync.source_count,
        board_count: sync.candidates.len() + sync.unsupported_count,
        generated_candidate_count: sync.generated_candidate_count,
        product_candidate_count: sync.product_candidate_count,
        unsupported_count: sync.unsupported_count,
        product_candidates: sync.product_candidates,
        stale_product_records: sync
            .candidates
            .iter()
            .filter(|board| board.product && board.stale)
            .map(|board| board.id.clone())
            .collect(),
        planned_fetches: vec![
            "normalize board records inside the current LilyGO support scope".to_string(),
            "exclude unsupported products from active routes".to_string(),
            "compare generated board candidates with route registry".to_string(),
        ],
        planned_writes: sync.planned_writes,
        writes: sync.writes,
        warnings: sync.warnings,
        compatibility_notes: vec![
            "sync-boards and update boards share the same apply path".to_string(),
            "hardware, OTA, LVGL, flash, and serial success require V4/V5 evidence".to_string(),
        ],
    }
}

fn planned_fetches(target: &str) -> Result<Vec<String>, String> {
    let fetches = match target {
        "sources" => vec![
            "fetch LilyGO GitHub org metadata through gh api or curl",
            "check Xinyuan-LilyGO/documentation as the official versioned docs source",
            "check LilyGO Wiki product URL coverage",
            "check auxiliary official reference catalogue",
        ],
        "boards" => vec![
            "normalize board records inside the current LilyGO support scope",
            "exclude unsupported products from active routes",
            "compare generated board candidates with route registry",
        ],
        "skills" => vec![
            "render compact board/series skills from data/boards.json into a generated cache",
            "copy generated route index into the generated cache",
            "render source-packed peripheral/chip/feature skills into the same cache",
            "verify source-intake manifest carries external references",
        ],
        "runtime" => vec![
            "check installed Codex runtime layout",
            "check installed Claude runtime layout",
            "copy source checkout route data into packaged runtimes",
        ],
        other => return Err(format!("unknown update target: {other}")),
    };
    Ok(fetches.into_iter().map(str::to_string).collect())
}

fn planned_writes(
    root: &Path,
    target: &str,
    generated_out: Option<&Path>,
) -> Result<Vec<String>, String> {
    let writes = match target {
        "sources" => vec![
            "data/references/source-intake/raw/lilygo-repos.json.gz",
            "data/references/source-intake/manifest.md",
        ],
        "boards" => vec!["data/boards.json"],
        "skills" => return Ok(generated_skill_plan(root, generated_out)),
        "runtime" => vec!["~/.codex/lilygo-skills", "~/.claude/lilygo-skills"],
        other => return Err(format!("unknown update target: {other}")),
    };
    Ok(writes.into_iter().map(str::to_string).collect())
}

fn generated_skill_plan(root: &Path, generated_out: Option<&Path>) -> Vec<String> {
    generated_out
        .map(|out| generated_skill_writes(root, out))
        .unwrap_or_else(default_generated_cache_writes)
}

pub(crate) fn generated_skill_writes(root: &Path, out: &Path) -> Vec<String> {
    let base = match out.strip_prefix(root) {
        Ok(relative) => relative.display().to_string(),
        Err(_) => out.display().to_string(),
    };
    vec![
        format!("{base}/skills"),
        format!("{base}/skills/references"),
        format!("{base}/templates/skills"),
        format!("{base}/index/routes.json"),
    ]
}

fn count_source_artifacts(root: &Path) -> usize {
    [
        "data/references/source-intake/raw",
        "data/references/source-intake/summaries",
    ]
    .iter()
    .map(|part| count_files(root.join(part).as_path()))
    .sum()
}

fn count_files(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_file())
                .count()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn source_ingestion() {
        let root = root();
        let preview = sync_preview(root.as_path());
        assert_eq!(preview.status, "PASS");
        assert!(preview.repo_count >= 200);
        assert!(preview.generated_candidate_count >= 4);
        assert!(preview.product_candidate_count >= 1);
        assert!(
            preview
                .product_candidates
                .iter()
                .any(|candidate| candidate.id == "board-t-watch-ultra")
        );
        let display = preview
            .candidates
            .iter()
            .find(|board| board.id == "board-t-display-s3")
            .expect("T-Display-S3 candidate");
        assert!(display.aliases.iter().any(|alias| alias == "t-display"));
        assert!(display.repo_url.contains("Xinyuan-LilyGO"));
        assert!(display.wiki_url.contains("wiki.lilygo.cc"));
    }

    #[test]
    fn update_previews_all_targets() {
        let root = root();
        for target in ["sources", "boards", "skills", "runtime"] {
            let preview = update_preview(root.as_path(), target, None).expect("update preview");
            assert_eq!(preview.status, "PASS");
            assert!(preview.dry_run);
            assert!(preview.source_count >= 1);
            assert!(preview.board_count >= 4);
            assert!(preview.product_candidate_count >= 1);
            assert!(!preview.planned_fetches.is_empty());
            assert!(!preview.planned_writes.is_empty());
            if target == "skills" {
                assert!(
                    preview
                        .planned_writes
                        .iter()
                        .all(|write| write.starts_with(".lilygo-skills/generated-skills/"))
                );
            }
            assert!(preview.writes.is_empty());
        }
    }

    #[test]
    fn sync_apply_writes_board_index_in_temp_root() {
        let source_root = root();
        let temp =
            std::env::temp_dir().join(format!("lilygo-skills-sync-apply-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join("data")).unwrap();
        fs::create_dir_all(temp.join("data/references/source-intake/raw")).unwrap();
        fs::create_dir_all(temp.join("data/references/source-intake/summaries")).unwrap();
        fs::copy(
            source_root.join(BOARD_INDEX_PATH),
            temp.join(BOARD_INDEX_PATH),
        )
        .unwrap();
        fs::copy(
            source_root.join(REPO_CACHE_PATH),
            temp.join(REPO_CACHE_PATH),
        )
        .unwrap();
        fs::copy(
            source_root.join(WIKI_PRODUCTS_CACHE_PATH),
            temp.join(WIKI_PRODUCTS_CACHE_PATH),
        )
        .unwrap();

        let report = sync_apply(&temp).expect("sync apply");
        assert_eq!(report.status, "PASS");
        assert!(!report.dry_run);
        assert_eq!(report.planned_writes, vec![BOARD_INDEX_PATH.to_string()]);
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.id == "board-t-watch-ultra")
        );
        assert!(temp.join(BOARD_INDEX_PATH).is_file());
        let _ = fs::remove_dir_all(&temp);
    }

    // A corrupt or missing product cache must abort the sync instead of
    // pruning every product board out of boards.json and reporting PASS.
    #[test]
    fn cache_fail_closed_never_prunes_boards() {
        let source_root = root();
        for (label, prepare) in [
            (
                "corrupt-gzip",
                Box::new(|temp: &std::path::Path| {
                    fs::write(temp.join(REPO_CACHE_PATH), b"not gzip at all").unwrap();
                }) as Box<dyn Fn(&std::path::Path)>,
            ),
            (
                "missing-wiki-cache",
                Box::new(|temp: &std::path::Path| {
                    fs::copy(
                        source_root.join(REPO_CACHE_PATH),
                        temp.join(REPO_CACHE_PATH),
                    )
                    .unwrap();
                    let _ = fs::remove_file(temp.join(WIKI_PRODUCTS_CACHE_PATH));
                }),
            ),
            (
                "empty-wiki-products",
                Box::new(|temp: &std::path::Path| {
                    fs::copy(
                        source_root.join(REPO_CACHE_PATH),
                        temp.join(REPO_CACHE_PATH),
                    )
                    .unwrap();
                    fs::write(
                        temp.join(WIKI_PRODUCTS_CACHE_PATH),
                        b"{\"schema_version\":1,\"products\":[]}",
                    )
                    .unwrap();
                }),
            ),
        ] {
            let temp = std::env::temp_dir().join(format!(
                "lilygo-skills-fail-closed-{label}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&temp);
            fs::create_dir_all(temp.join("data")).unwrap();
            fs::create_dir_all(temp.join("data/references/source-intake/raw")).unwrap();
            fs::create_dir_all(temp.join("data/references/source-intake/summaries")).unwrap();
            fs::copy(
                source_root.join(BOARD_INDEX_PATH),
                temp.join(BOARD_INDEX_PATH),
            )
            .unwrap();
            fs::copy(
                source_root.join(WIKI_PRODUCTS_CACHE_PATH),
                temp.join(WIKI_PRODUCTS_CACHE_PATH),
            )
            .unwrap();
            let before = fs::read_to_string(temp.join(BOARD_INDEX_PATH)).unwrap();
            prepare(&temp);

            let result = sync_apply(&temp);
            assert!(result.is_err(), "{label}: sync_apply must fail closed");
            let after = fs::read_to_string(temp.join(BOARD_INDEX_PATH)).unwrap();
            assert_eq!(before, after, "{label}: boards.json must stay untouched");
            let _ = fs::remove_dir_all(&temp);
        }
    }

    #[test]
    fn product_dry_run_no_write() {
        let root = root();
        let watched = [
            BOARD_INDEX_PATH,
            "index/routes.json",
            "skills/board-t-watch-ultra/SKILL.md",
            "data/profile.json",
        ];
        let before: Vec<_> = watched
            .iter()
            .map(|path| (path, fs::read(root.join(path)).ok()))
            .collect();

        let sync = sync_preview(root.as_path());
        assert!(sync.writes.is_empty());
        for target in ["sources", "boards", "skills", "runtime"] {
            let preview = update_preview(root.as_path(), target, None).expect("update preview");
            assert!(preview.dry_run);
            assert!(preview.writes.is_empty());
            if target == "skills" {
                assert!(
                    preview
                        .product_candidates
                        .iter()
                        .any(|candidate| candidate.id == "board-t-watch-ultra")
                );
            }
        }

        for (path, expected) in before {
            assert_eq!(fs::read(root.join(path)).ok(), expected, "{path} changed");
        }
    }
}
