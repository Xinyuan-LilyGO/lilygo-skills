//! Live re-proof of source-backed facts: `verify sources` re-fetches each
//! pin/bus fact's official source over the ambient http(s) proxy, recomputes
//! the sha256 of the fetched file, and compares it to the stored hash. This is
//! the runnable form of our one edge — verifiability — turning the stored
//! `source` triple into an on-demand OK / DRIFT / UNREACHABLE verdict.
//!
//! The stored `hash` is the sha256 of the *whole* fetched file (matching how
//! `pipeline/ingest-from-manifest.js` records it), so the verdict is driven by
//! the file hash; `line_range` is re-sliced only to surface the anchored block.
//! Offline is graceful: a failed fetch is an honest `UNREACHABLE` verdict, not
//! a crash.
use crate::facts::load_fact_pack_index;
use crate::model::SourceFact;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Serialize)]
pub(crate) struct SourceVerifyReport {
    pub status: String,
    pub board_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    pub counts: SourceVerifyCounts,
    pub facts: Vec<SourceVerifyFact>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SourceVerifyCounts {
    pub total: usize,
    pub ok: usize,
    pub drift: usize,
    pub unreachable: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct SourceVerifyFact {
    pub key: String,
    pub topic: String,
    pub fetch_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_range: Option<String>,
    pub stored_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_hash: Option<String>,
    pub verdict: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A verifiable pin/bus fact: one that carries the full source triple
/// (a fetchable http url, a recorded `line_range`, and a `sha256:` hash). Facts
/// without a `line_range` are repo/reference-tier and are not re-fetchable as a
/// single file, so they are out of scope for the live hash re-proof.
fn is_verifiable(fact: &SourceFact) -> bool {
    let source = &fact.source;
    source.path_or_url.starts_with("http")
        && source.line_range.is_some()
        && source.hash.starts_with("sha256:")
}

/// Convert a github `blob` URL into its `raw.githubusercontent.com` form so the
/// recorded source can be fetched as raw file bytes. Raw urls and non-github
/// urls pass through unchanged.
pub(crate) fn raw_fetch_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        // OWNER/REPO/blob/REF/PATH -> raw.githubusercontent.com/OWNER/REPO/REF/PATH
        if let Some((repo, tail)) = rest.split_once("/blob/") {
            return format!("https://raw.githubusercontent.com/{repo}/{tail}");
        }
    }
    url.to_string()
}

fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Fetch raw text over curl, honoring the ambient http(s) proxy exactly as the
/// node ingest path does (`curl -sfL`). Any non-success is surfaced as an error
/// so the caller can classify it as `UNREACHABLE`.
fn curl_fetch(url: &str) -> Result<String, String> {
    let output = std::process::Command::new("curl")
        .args(["-sfL", "--max-time", "30", url])
        .output()
        .map_err(|error| format!("curl unavailable: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "curl failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("non-utf8 response: {error}"))
}

/// Slice an inclusive, 1-based line range (mirrors `pipeline/source-io.js`).
fn slice_range(text: &str, range: &str) -> Option<String> {
    let (a, b) = range.split_once('-')?;
    let start: usize = a.trim().parse().ok()?;
    let end: usize = b.trim().parse().ok()?;
    if start == 0 || end < start {
        return None;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let hi = end.min(lines.len());
    Some(lines[start - 1..hi].join("\n"))
}

/// Classify a single fact against a fetch result. Pure so it is unit-testable
/// without network: fetch `Ok` with a matching hash → OK, a mismatch → DRIFT,
/// a fetch `Err` → UNREACHABLE.
fn classify(fact: &SourceFact, fetched: Result<String, String>) -> SourceVerifyFact {
    let source = &fact.source;
    let fetch_url = raw_fetch_url(&source.path_or_url);
    let (verdict, live_hash, detail) = match fetched {
        Ok(text) => {
            let live = sha256_hex(&text);
            if live == source.hash {
                let detail = source
                    .line_range
                    .as_deref()
                    .and_then(|range| slice_range(&text, range))
                    .map(|_| "line_range re-sliced".to_string());
                ("OK".to_string(), Some(live), detail)
            } else {
                (
                    "DRIFT".to_string(),
                    Some(live),
                    Some("fetched file hash differs from stored hash".to_string()),
                )
            }
        }
        Err(error) => ("UNREACHABLE".to_string(), None, Some(error)),
    };
    SourceVerifyFact {
        key: fact.key.clone(),
        topic: fact.topic.clone(),
        fetch_url,
        line_range: source.line_range.clone(),
        stored_hash: source.hash.clone(),
        live_hash,
        verdict,
        detail,
    }
}

pub(crate) fn verify_sources(
    root: &Path,
    board_id: &str,
    topic: Option<&str>,
) -> Result<SourceVerifyReport, String> {
    verify_sources_with(root, board_id, topic, curl_fetch)
}

/// Core builder parameterized by the fetcher so tests can inject a fixture
/// source without touching the network.
fn verify_sources_with(
    root: &Path,
    board_id: &str,
    topic: Option<&str>,
    fetch: impl Fn(&str) -> Result<String, String>,
) -> Result<SourceVerifyReport, String> {
    let index = load_fact_pack_index(root)?;
    let pack = index
        .packs
        .into_iter()
        .find(|pack| pack.board_id == board_id)
        .ok_or_else(|| format!("unknown board fact pack: {board_id}"))?;

    let facts: Vec<SourceVerifyFact> = pack
        .pin_matrix
        .iter()
        .chain(pack.bus_matrix.iter())
        .filter(|fact| is_verifiable(fact))
        .filter(|fact| topic.is_none_or(|topic| fact.topic == topic))
        .map(|fact| {
            let url = raw_fetch_url(&fact.source.path_or_url);
            classify(fact, fetch(&url))
        })
        .collect();

    let mut counts = SourceVerifyCounts {
        total: facts.len(),
        ok: 0,
        drift: 0,
        unreachable: 0,
    };
    for fact in &facts {
        match fact.verdict.as_str() {
            "OK" => counts.ok += 1,
            "DRIFT" => counts.drift += 1,
            _ => counts.unreachable += 1,
        }
    }
    // DRIFT means an upstream source changed under a fact we still assert — a
    // real integrity signal, so the command exits non-zero on any drift.
    // UNREACHABLE (offline / 404) is an honest verdict, never a failure.
    let status = if counts.drift > 0 { "DRIFT" } else { "PASS" };
    Ok(SourceVerifyReport {
        status: status.to_string(),
        board_id: board_id.to_string(),
        topic: topic.map(str::to_string),
        counts,
        facts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SourceFact, SourceFactSource};

    fn fixture_fact(hash: &str) -> SourceFact {
        SourceFact {
            schema_version: 1,
            board_id: "board-fixture".to_string(),
            topic: "pinout".to_string(),
            key: "pin.i2c.sda".to_string(),
            value: "I2C_SDA=GPIO21".to_string(),
            claim: "fixture".to_string(),
            source: SourceFactSource {
                kind: "arduino-pins".to_string(),
                path_or_url: "https://github.com/Xinyuan-LilyGO/LilyGo-LoRa-Series/blob/master/x.h"
                    .to_string(),
                line_range: Some("1-2".to_string()),
                hash: hash.to_string(),
            },
            authority_rank: 90,
            evidence_level: "V3-source-reference".to_string(),
            stale: false,
            confidence: "exact".to_string(),
        }
    }

    #[test]
    fn blob_url_becomes_raw() {
        assert_eq!(
            raw_fetch_url("https://github.com/owner/repo/blob/master/src/x.h"),
            "https://raw.githubusercontent.com/owner/repo/master/src/x.h"
        );
        // Already-raw and non-github urls are unchanged.
        let raw = "https://raw.githubusercontent.com/owner/repo/master/x.h";
        assert_eq!(raw_fetch_url(raw), raw);
    }

    #[test]
    fn matching_hash_is_ok() {
        let body = "line one\nline two\nline three";
        let stored = sha256_hex(body);
        let fact = fixture_fact(&stored);
        let result = classify(&fact, Ok(body.to_string()));
        assert_eq!(result.verdict, "OK");
        assert_eq!(result.live_hash.as_deref(), Some(stored.as_str()));
    }

    #[test]
    fn tampered_hash_is_drift() {
        // The stored hash no longer matches the (fixture) fetched file → DRIFT.
        let body = "line one\nline two\nline three";
        let fact =
            fixture_fact("sha256:0000000000000000000000000000000000000000000000000000000000000000");
        let result = classify(&fact, Ok(body.to_string()));
        assert_eq!(result.verdict, "DRIFT");
        assert_eq!(result.live_hash.as_deref(), Some(sha256_hex(body).as_str()));
    }

    #[test]
    fn fetch_error_is_unreachable() {
        let fact = fixture_fact("sha256:abc");
        let result = classify(&fact, Err("no network".to_string()));
        assert_eq!(result.verdict, "UNREACHABLE");
        assert!(result.live_hash.is_none());
    }

    #[test]
    fn report_counts_and_topic_filter() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // Offline-deterministic: inject a fetcher that echoes a fixed body, so
        // every verifiable fact resolves to a stable DRIFT (its stored hash is
        // the real upstream hash, not our fixed body). This exercises the pack
        // load, filter, and counting without touching the network.
        let report = verify_sources_with(root.as_path(), "board-t-beam", None, |_url| {
            Ok("fixed body".to_string())
        })
        .expect("verify report");
        assert!(report.counts.total > 0);
        assert_eq!(
            report.counts.total,
            report.counts.ok + report.counts.drift + report.counts.unreachable
        );
        assert!(report.facts.iter().all(|fact| fact.line_range.is_some()));

        let offline = verify_sources_with(root.as_path(), "board-t-beam", Some("pinout"), |_| {
            Err("offline".to_string())
        })
        .expect("offline report");
        assert_eq!(offline.status, "PASS", "offline must not be a hard failure");
        assert_eq!(offline.counts.unreachable, offline.counts.total);
        assert!(offline.facts.iter().all(|fact| fact.topic == "pinout"));
    }
}
