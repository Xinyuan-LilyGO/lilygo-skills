//! External source refresh adapters for LilyGO Wiki/GitHub cache snapshots and
//! source manifests used by dry-run and apply update surfaces.
use crate::source::{
    REPO_CACHE_PATH, SOURCE_MANIFEST_PATH, WIKI_PRODUCTS_CACHE_PATH, write_if_changed,
};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub(crate) struct SourceRefresh {
    pub(crate) live: bool,
    pub(crate) writes: Vec<String>,
}

pub(crate) fn refresh_source_cache(
    root: &Path,
    warnings: &mut Vec<String>,
) -> Result<SourceRefresh, String> {
    let repos = match fetch_github_repos_live() {
        Ok(repos) => repos,
        Err(error) => {
            warnings.push(format!(
                "live GitHub refresh unavailable; kept existing cache: {error}"
            ));
            let cache = read_repo_cache_value(root)?;
            let cached_repos = cache
                .get("repos")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            if cached_repos.is_empty() {
                return Err("source cache fallback has no repository records".to_string());
            }
            return Ok(SourceRefresh {
                live: false,
                writes: Vec::new(),
            });
        }
    };

    let payload = json!({
        "fetched_at_unix": unix_timestamp(),
        "count": repos.len(),
        "repos": repos,
    });
    let rendered = serde_json::to_vec_pretty(&payload)
        .map_err(|error| format!("failed to render repo cache: {error}"))?;
    let compressed = gzip_bytes("lilygo-repos.json", &rendered)?;
    let cache_path = root.join(REPO_CACHE_PATH);
    let mut writes = Vec::new();
    if write_if_changed(&cache_path, &compressed)? {
        writes.push(REPO_CACHE_PATH.to_string());
    }
    match fetch_wiki_products() {
        Ok(wiki_payload) => {
            let mut rendered = serde_json::to_vec_pretty(&wiki_payload)
                .map_err(|error| format!("failed to render Wiki product cache: {error}"))?;
            rendered.push(b'\n');
            let wiki_path = root.join(WIKI_PRODUCTS_CACHE_PATH);
            if write_if_changed(&wiki_path, &rendered)? {
                writes.push(WIKI_PRODUCTS_CACHE_PATH.to_string());
            }
        }
        Err(error) => warnings.push(format!(
            "live Wiki product index refresh unavailable; kept existing cache if present: {error}"
        )),
    }
    Ok(SourceRefresh { live: true, writes })
}

pub(crate) fn refresh_manifest_hashes(
    root: &Path,
    touched_paths: &[String],
) -> Result<Vec<String>, String> {
    if touched_paths.is_empty() {
        return Ok(Vec::new());
    }
    let manifest_path = root.join(SOURCE_MANIFEST_PATH);
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let touched: BTreeSet<&str> = touched_paths.iter().map(String::as_str).collect();
    let mut changed = false;
    let mut lines = Vec::new();
    for line in manifest.lines() {
        let Some(updated) = refresh_manifest_line(root, line, &touched)? else {
            lines.push(line.to_string());
            continue;
        };
        changed |= updated != line;
        lines.push(updated);
    }
    if !changed {
        return Ok(Vec::new());
    }
    let rendered = lines.join("\n") + "\n";
    write_if_changed(&manifest_path, rendered.as_bytes())?;
    Ok(vec![SOURCE_MANIFEST_PATH.to_string()])
}

fn fetch_github_repos_live() -> Result<Vec<serde_json::Value>, String> {
    match fetch_github_repos_with_gh() {
        Ok(repos) => Ok(repos),
        Err(gh_error) => match fetch_github_repos_with_curl() {
            Ok(repos) => Ok(repos),
            Err(curl_error) => Err(format!(
                "gh api failed: {gh_error}; curl failed: {curl_error}"
            )),
        },
    }
}

fn fetch_github_repos_with_gh() -> Result<Vec<serde_json::Value>, String> {
    let mut repos = Vec::new();
    let jq_filter = "[.[] | {archived, default_branch, description, forks_count, full_name, html_url, language, name, pushed_at, stargazers_count, updated_at}]";
    for page in 1..=20 {
        let endpoint =
            format!("/orgs/Xinyuan-LilyGO/repos?per_page=50&type=public&sort=updated&page={page}");
        let mut command = Command::new("gh");
        command.args(["api", &endpoint, "--jq", jq_filter]);
        let output = output_with_timeout(&mut command, 20)?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let page_repos: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("invalid gh api JSON: {error}"))?;
        if page_repos.is_empty() {
            break;
        }
        let page_len = page_repos.len();
        repos.extend(page_repos.into_iter().map(normalize_repo_value));
        if page_len < 50 {
            break;
        }
    }
    sort_and_validate_repos(repos)
}

fn fetch_github_repos_with_curl() -> Result<Vec<serde_json::Value>, String> {
    let mut repos = Vec::new();
    for page in 1..=10 {
        let url = format!(
            "https://api.github.com/orgs/Xinyuan-LilyGO/repos?per_page=100&page={page}&sort=updated"
        );
        let output = Command::new("curl")
            .args([
                "-fsSL",
                "--max-time",
                "20",
                "-H",
                "User-Agent: lilygo-skills",
                &url,
            ])
            .output()
            .map_err(|error| format!("failed to run curl: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let page_repos: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("invalid GitHub repo JSON: {error}"))?;
        if page_repos.is_empty() {
            break;
        }
        let page_len = page_repos.len();
        repos.extend(page_repos.into_iter().map(normalize_repo_value));
        if page_len < 100 {
            break;
        }
    }
    sort_and_validate_repos(repos)
}

fn fetch_wiki_products() -> Result<serde_json::Value, String> {
    let url = "https://wiki.lilygo.cc/products/";
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", "20", url])
        .output()
        .map_err(|error| format!("failed to run curl for Wiki index: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let html = String::from_utf8(output.stdout)
        .map_err(|error| format!("Wiki index was not UTF-8: {error}"))?;
    let links = parse_wiki_product_links(&html);
    if links.is_empty() {
        return Err("Wiki index contained no product links".to_string());
    }
    Ok(json!({
        "fetched_at_unix": unix_timestamp(),
        "source": url,
        "count": links.len(),
        "products": links,
    }))
}

fn parse_wiki_product_links(html: &str) -> Vec<serde_json::Value> {
    let mut links = BTreeSet::new();
    for chunk in html.split("href=\"").skip(1) {
        let Some(link) = chunk.split('"').next() else {
            continue;
        };
        if link.starts_with("/products/") && link.ends_with('/') {
            links.insert(link.to_string());
        }
    }
    links
        .into_iter()
        .filter_map(|link| {
            let slug = link
                .trim_matches('/')
                .split('/')
                .next_back()
                .unwrap_or("")
                .to_string();
            if slug.is_empty() {
                return None;
            }
            Some(json!({
                "slug": slug,
                "link": link,
                "url": format!("https://wiki.lilygo.cc{link}"),
            }))
        })
        .collect()
}

fn sort_and_validate_repos(
    mut repos: Vec<serde_json::Value>,
) -> Result<Vec<serde_json::Value>, String> {
    if repos.is_empty() {
        Err("GitHub API returned no repositories".to_string())
    } else {
        repos.sort_by(|left, right| {
            repo_stars(right)
                .cmp(&repo_stars(left))
                .then_with(|| repo_name(left).cmp(&repo_name(right)))
        });
        Ok(repos)
    }
}

fn normalize_repo_value(repo: serde_json::Value) -> serde_json::Value {
    json!({
        "archived": repo.get("archived").and_then(|value| value.as_bool()).unwrap_or(false),
        "default_branch": repo.get("default_branch").and_then(|value| value.as_str()).unwrap_or(""),
        "description": repo.get("description").cloned().unwrap_or(serde_json::Value::Null),
        "forks_count": repo.get("forks_count").and_then(|value| value.as_u64()).unwrap_or(0),
        "full_name": repo.get("full_name").and_then(|value| value.as_str()).unwrap_or(""),
        "html_url": repo.get("html_url").and_then(|value| value.as_str()).unwrap_or(""),
        "language": repo.get("language").cloned().unwrap_or(serde_json::Value::Null),
        "name": repo.get("name").and_then(|value| value.as_str()).unwrap_or(""),
        "pushed_at": repo.get("pushed_at").and_then(|value| value.as_str()).unwrap_or(""),
        "stargazers_count": repo.get("stargazers_count").and_then(|value| value.as_u64()).unwrap_or(0),
        "updated_at": repo.get("updated_at").and_then(|value| value.as_str()).unwrap_or("")
    })
}

fn repo_stars(repo: &serde_json::Value) -> u64 {
    repo.get("stargazers_count")
        .and_then(|value| value.as_u64())
        .unwrap_or(0)
}

fn repo_name(repo: &serde_json::Value) -> String {
    repo.get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string()
}

fn read_repo_cache_value(root: &Path) -> Result<serde_json::Value, String> {
    let path = root.join(REPO_CACHE_PATH);
    let file =
        File::open(&path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut decoder = GzDecoder::new(file);
    let mut data = String::new();
    decoder
        .read_to_string(&mut data)
        .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;
    serde_json::from_str(&data).map_err(|error| format!("invalid repo cache: {error}"))
}

fn gzip_bytes(filename: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .map_err(|error| format!("failed to gzip {filename}: {error}"))?;
    encoder
        .finish()
        .map_err(|error| format!("failed to finish gzip {filename}: {error}"))
}

fn refresh_manifest_line(
    root: &Path,
    line: &str,
    touched_paths: &BTreeSet<&str>,
) -> Result<Option<String>, String> {
    if !line.starts_with('|') {
        return Ok(None);
    }
    let mut cells: Vec<String> = line
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect();
    if cells.len() < 6 {
        return Ok(None);
    }
    let Some(path) = backtick_value(&cells[2]).map(str::to_string) else {
        return Ok(None);
    };
    if !touched_paths.contains(path.as_str()) {
        return Ok(None);
    }
    let hash = sha256_file(&root.join(&path))?;
    cells[5] = format!("`{hash}`");
    Ok(Some(format!(
        "| {} |",
        cells[1..cells.len() - 1].join(" | ")
    )))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn backtick_value(value: &str) -> Option<&str> {
    value.strip_prefix('`')?.strip_suffix('`')
}

fn output_with_timeout(command: &mut Command, seconds: u64) -> Result<Output, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to spawn command: {error}"))?;
    // Reader threads drain both pipes while polling; a child writing more
    // than the OS pipe buffer would otherwise block forever under try_wait.
    let stdout_reader = spawn_pipe_reader(child.stdout.take());
    let stderr_reader = spawn_pipe_reader(child.stderr.take());
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let drain = Instant::now() + Duration::from_secs(2);
                    let _ = join_pipe_reader(stdout_reader, drain);
                    let _ = join_pipe_reader(stderr_reader, drain);
                    return Err(format!("command timed out after {seconds}s"));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(format!("failed to poll command: {error}")),
        }
    };
    let drain = Instant::now() + Duration::from_secs(5);
    Ok(Output {
        status,
        stdout: join_pipe_reader(stdout_reader, drain),
        stderr: join_pipe_reader(stderr_reader, drain),
    })
}

struct PipeReader {
    buffer: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    handle: std::thread::JoinHandle<()>,
}

fn spawn_pipe_reader<R: std::io::Read + Send + 'static>(pipe: Option<R>) -> Option<PipeReader> {
    let mut source = pipe?;
    let buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&buffer);
    let handle = std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match source.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    if let Ok(mut locked) = sink.lock() {
                        locked.extend_from_slice(&chunk[..count]);
                    }
                }
            }
        }
    });
    Some(PipeReader { buffer, handle })
}

fn join_pipe_reader(reader: Option<PipeReader>, deadline: Instant) -> Vec<u8> {
    let Some(reader) = reader else {
        return Vec::new();
    };
    while !reader.handle.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if reader.handle.is_finished() {
        let _ = reader.handle.join();
    }
    match reader.buffer.lock() {
        Ok(locked) => locked.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
