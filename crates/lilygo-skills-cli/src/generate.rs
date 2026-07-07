//! Runtime skill generation and generated-root verification for install/cache
//! artifacts derived from the meta skill, registry, recipes, and source packs.
use crate::peripheral_source::generated_skill_files;
use crate::playbooks::playbook_skill_files;
use crate::recipes::recipe_registry;
use crate::registry::load_registry;
use crate::source::write_if_changed;
use crate::source_generation::board_skill_files;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

const REFERENCE_INDEX_PATH: &str = "data/skills/reference/index.json";
const PERIPHERAL_PACK_PATH: &str = "data/peripherals/source-packs.json";
const STATIC_REFERENCES_DIR: &str = "skills/references";
const SKILL_TEMPLATES_DIR: &str = "templates/skills";
pub const GENERATED_CACHE_DIR: &str = ".lilygo-skills/generated-skills";

#[derive(Debug, Deserialize)]
struct ReferenceSkillIndex {
    skills: Vec<ReferenceSkillEntry>,
}

#[derive(Debug, Deserialize)]
struct ReferenceSkillEntry {
    id: String,
    kind: String,
    source_path: String,
}

#[derive(Debug, Serialize)]
pub struct GenerateReport {
    pub schema_version: u32,
    pub status: String,
    pub out_root: String,
    pub skill_count: usize,
    pub board_skills: usize,
    pub peripheral_skills: usize,
    pub reference_skills: usize,
    pub playbook_skills: usize,
    pub source_pack_ids: Vec<String>,
    pub source_hashes: BTreeMap<String, String>,
    pub warnings: Vec<String>,
    pub verification_hints: Vec<String>,
}

pub fn generated_cache_root(root: &Path) -> PathBuf {
    root.join(GENERATED_CACHE_DIR)
}

pub fn default_generated_cache_writes() -> Vec<String> {
    vec![
        format!("{GENERATED_CACHE_DIR}/skills"),
        format!("{GENERATED_CACHE_DIR}/skills/references"),
        format!("{GENERATED_CACHE_DIR}/templates/skills"),
        format!("{GENERATED_CACHE_DIR}/index/routes.json"),
    ]
}

pub const GENERATED_MARKER_FILE: &str = ".lilygo-skills-generated";

/// Generation deletes `<out>/skills`, `<out>/skills/references`, and
/// `<out>/templates/skills` before rewriting them. Those subtrees may only be
/// cleared when the target is provably a generated root: marker-carrying, or
/// matching the pre-marker generated shape (which must include at least one
/// generated `board-*` skill, so a source clone — router plus references only
/// — or a user's own `~/skills` can never qualify). Targets that do not yet
/// contain those subtrees have nothing to clear and are always safe;
/// `generate --out ~` must never delete user data.
fn ensure_clearable_generation_target(out: &Path) -> Result<(), String> {
    if !out.exists() {
        return Ok(());
    }
    if !out.is_dir() {
        return Err(format!(
            "generation target {} is not a directory",
            out.display()
        ));
    }
    let clears_existing = out.join("skills").exists() || out.join(SKILL_TEMPLATES_DIR).exists();
    if !clears_existing {
        return Ok(());
    }
    if out.join(GENERATED_MARKER_FILE).is_file() {
        return Ok(());
    }
    let legacy_shape = out.join("index/routes.json").is_file()
        && out.join(SKILL_TEMPLATES_DIR).is_dir()
        && has_generated_board_skill(&out.join("skills"));
    if legacy_shape {
        return Ok(());
    }
    Err(format!(
        "refusing to clear {}: existing skills/templates content without the \
         {GENERATED_MARKER_FILE} marker or a generated layout",
        out.display()
    ))
}

fn has_generated_board_skill(skills_root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(skills_root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry.file_name().to_string_lossy().starts_with("board-")
            && entry.path().join("SKILL.md").is_file()
    })
}

pub fn generate_skills(root: &Path, out: &Path) -> Result<GenerateReport, String> {
    let out = out.to_path_buf();
    if is_source_tree_generation_target(root, &out) {
        return Err("refusing to generate into the committed source tree".to_string());
    }
    ensure_clearable_generation_target(&out)?;
    // Mark the target as generated before clearing/writing: an interrupted
    // generation must stay retryable instead of tripping the guard forever.
    write_if_changed(
        &out.join(GENERATED_MARKER_FILE),
        b"generated skills root; safe for lilygo-skills generate to clear\n",
    )?;
    let skills_root = out.join("skills");
    if skills_root.exists() {
        fs::remove_dir_all(&skills_root)
            .map_err(|error| format!("failed to clear {}: {error}", skills_root.display()))?;
    }

    let mut warnings = Vec::new();

    let boards = board_skill_files(root)?;
    let board_count = boards.len();
    for (id, content) in &boards {
        write_skill(&skills_root, id, content)?;
    }

    let peripherals = generated_skill_files(root)?;
    let peripheral_count = peripherals.len();
    for (id, content) in &peripherals {
        write_skill(&skills_root, id, content)?;
    }

    let references = reference_skill_files(root)?;
    let reference_count = references.len();
    for (id, content) in &references {
        write_skill(&skills_root, id, content)?;
    }

    let playbooks = playbook_skill_files();
    let playbook_count = playbooks.len();
    for (id, content) in &playbooks {
        write_skill(&skills_root, id, content)?;
    }

    let router_src = root.join("skills/lilygo-router/SKILL.md");
    if let Ok(content) = fs::read_to_string(&router_src) {
        write_skill(&skills_root, "lilygo-router", &content)?;
    } else {
        warnings.push("committed meta router skills/lilygo-router/SKILL.md not found".to_string());
    }

    copy_support_tree(
        &root.join(STATIC_REFERENCES_DIR),
        &out.join(STATIC_REFERENCES_DIR),
    )?;
    copy_support_tree(
        &root.join(SKILL_TEMPLATES_DIR),
        &out.join(SKILL_TEMPLATES_DIR),
    )?;

    let registry = load_registry(root)?;
    let index_root = out.join("index");
    fs::create_dir_all(&index_root)
        .map_err(|error| format!("failed to create {}: {error}", index_root.display()))?;
    let rendered = serde_json::to_string_pretty(&registry)
        .map_err(|error| format!("failed to render registry: {error}"))?
        + "\n";
    write_if_changed(&index_root.join("routes.json"), rendered.as_bytes())?;

    for skill in &registry.skills {
        let path = skills_root.join(&skill.id).join("SKILL.md");
        if !path.is_file() {
            warnings.push(format!(
                "registry skill {} has no generated SKILL.md",
                skill.id
            ));
        }
    }

    let (source_pack_ids, source_hashes) = source_pack_summary(root)?;
    let skill_count = registry.skills.len();
    let out_display = public_path(out.as_path());

    Ok(GenerateReport {
        schema_version: 1,
        status: if warnings.is_empty() {
            "PASS".to_string()
        } else {
            "WARN".to_string()
        },
        out_root: out_display.clone(),
        skill_count,
        board_skills: board_count,
        peripheral_skills: peripheral_count,
        reference_skills: reference_count,
        playbook_skills: playbook_count,
        source_pack_ids,
        source_hashes,
        warnings,
        verification_hints: vec![
            format!(
                "verify with: lilygo-skills verify --generated-root {} --json",
                out_display
            ),
            "generated skills stay at context-injection; V4/V5 needs evidence smokes".to_string(),
        ],
    })
}

fn is_source_tree_generation_target(root: &Path, out: &Path) -> bool {
    let root = normalize_boundary_path(root);
    let out = normalize_boundary_path(out);
    out == root
        || protected_source_dirs()
            .iter()
            .any(|part| out.starts_with(root.join(part)))
}

fn protected_source_dirs() -> &'static [&'static str] {
    &[
        "crates",
        "data",
        "doc",
        "docs",
        "index",
        "installer",
        "scripts",
        "schemas",
        "skills",
        "templates",
    ]
}

fn normalize_boundary_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return normalize_path(&canonical);
    }
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return normalize_path(path);
    };
    let mut normalized = normalize_boundary_path(parent);
    normalized.push(name);
    normalize_path(&normalized)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[derive(Debug, Serialize)]
pub struct GeneratedVerifyReport {
    pub schema_version: u32,
    pub status: String,
    pub generated_root: String,
    pub skill_count: usize,
    pub present: usize,
    pub missing: Vec<String>,
    pub extra: Vec<String>,
    pub extra_count: usize,
    pub reference_skills_present: usize,
    pub reference_skills_missing: Vec<String>,
    pub evidence_boundary_ok: bool,
    pub evidence_violations: Vec<String>,
    pub source_coverage_delta: i64,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn verify_generated_root(source_root: &Path, generated_root: &Path) -> GeneratedVerifyReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let registry = match load_registry(generated_root) {
        Ok(registry) => registry,
        Err(error) => {
            return GeneratedVerifyReport {
                schema_version: 1,
                status: "FAIL".to_string(),
                generated_root: public_path(generated_root),
                skill_count: 0,
                present: 0,
                missing: Vec::new(),
                extra: Vec::new(),
                extra_count: 0,
                reference_skills_present: 0,
                reference_skills_missing: Vec::new(),
                evidence_boundary_ok: false,
                evidence_violations: Vec::new(),
                source_coverage_delta: 0,
                errors: vec![format!("missing generated index/routes.json: {error}")],
                warnings,
            };
        }
    };

    let skills_root = generated_root.join("skills");
    let registry_ids = registry
        .skills
        .iter()
        .map(|skill| skill.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut present = 0usize;
    let mut missing = Vec::new();
    let mut evidence_violations = Vec::new();
    for skill in &registry.skills {
        let path = skills_root.join(&skill.id).join("SKILL.md");
        match fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => {
                present += 1;
                if let Some(violation) = evidence_boundary_violation(&skill.id, &content) {
                    evidence_violations.push(violation);
                }
            }
            _ => missing.push(skill.id.clone()),
        }
    }
    let extra = generated_skill_dirs(&skills_root)
        .into_iter()
        .filter(|id| !registry_ids.contains(id))
        .collect::<Vec<_>>();

    let required = crate::registry::required_reference_skills();
    let mut reference_present = 0usize;
    let mut reference_missing = Vec::new();
    for id in required {
        if skills_root.join(id).join("SKILL.md").is_file() {
            reference_present += 1;
        } else {
            reference_missing.push(id.to_string());
        }
    }

    let source_delta = match load_registry(source_root) {
        Ok(source_registry) => registry.skills.len() as i64 - source_registry.skills.len() as i64,
        Err(error) => {
            warnings.push(format!("could not load source registry for delta: {error}"));
            0
        }
    };

    if !missing.is_empty() {
        errors.push(format!("{} routed skills missing in cache", missing.len()));
    }
    if !reference_missing.is_empty() {
        errors.push(format!(
            "{} required reference skills missing",
            reference_missing.len()
        ));
    }
    if !evidence_violations.is_empty() {
        errors.push(format!(
            "{} generated skills claim unverified hardware success",
            evidence_violations.len()
        ));
    }
    if !extra.is_empty() {
        errors.push(format!(
            "{} generated skills are not registered",
            extra.len()
        ));
    }

    let status = if errors.is_empty() {
        "PASS".to_string()
    } else {
        "FAIL".to_string()
    };
    GeneratedVerifyReport {
        schema_version: 1,
        status,
        generated_root: public_path(generated_root),
        skill_count: registry.skills.len(),
        present,
        missing,
        extra_count: extra.len(),
        extra,
        reference_skills_present: reference_present,
        reference_skills_missing: reference_missing,
        evidence_boundary_ok: evidence_violations.is_empty(),
        evidence_violations,
        source_coverage_delta: source_delta,
        errors,
        warnings,
    }
}

fn public_path(path: &Path) -> String {
    let normalized = normalize_path(path);
    if let Ok(cwd) = std::env::current_dir() {
        let cwd = normalize_path(cwd.as_path());
        if let Ok(relative) = normalized.strip_prefix(cwd) {
            return path_display_or_dot(relative);
        }
    }
    if normalized.is_absolute() {
        return normalized
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("<redacted-path>/{name}"))
            .unwrap_or_else(|| "<redacted-path>".to_string());
    }
    path_display_or_dot(normalized.as_path())
}

fn path_display_or_dot(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        path.display().to_string()
    }
}

fn generated_skill_dirs(skills_root: &Path) -> Vec<String> {
    let mut ids = fs::read_dir(skills_root)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let path = entry.path();
                    if path.join("SKILL.md").is_file() {
                        entry.file_name().to_str().map(str::to_string)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

fn evidence_boundary_violation(id: &str, content: &str) -> Option<String> {
    let lower = content.to_lowercase();
    const FORBIDDEN: &[&str] = &[
        "pixels rendered",
        "ota succeeded",
        "flash verified on hardware",
        "lora link established",
        "gnss fix acquired",
        concat!("hardware verified", ": true"),
    ];
    for needle in FORBIDDEN {
        if contains_unqualified_claim(&lower, needle) {
            return Some(format!("{id}: '{needle}'"));
        }
    }
    None
}

fn contains_unqualified_claim(content: &str, needle: &str) -> bool {
    if !content.contains(needle) {
        return false;
    }
    let anti_claims = [
        format!("cannot prove {needle}"),
        format!("do not claim {needle}"),
        format!("without {needle} evidence"),
    ];
    !anti_claims.iter().any(|claim| content.contains(claim))
}

pub(crate) fn generatable_skill_ids(
    root: &Path,
) -> Result<std::collections::BTreeSet<String>, String> {
    let mut ids = std::collections::BTreeSet::new();
    ids.insert("lilygo-router".to_string());
    for (id, _) in board_skill_files(root)? {
        ids.insert(id);
    }
    for (id, _) in generated_skill_files(root)? {
        ids.insert(id);
    }
    for (id, _) in reference_skill_files(root)? {
        ids.insert(id);
    }
    for (id, _) in playbook_skill_files() {
        ids.insert(id);
    }
    Ok(ids)
}

fn write_skill(skills_root: &Path, id: &str, content: &str) -> Result<(), String> {
    let dir = skills_root.join(id);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create {}: {error}", dir.display()))?;
    let content = ensure_skill_frontmatter(id, content);
    write_if_changed(&dir.join("SKILL.md"), content.as_bytes())?;
    Ok(())
}

/// Skill hosts only index `SKILL.md` files that start with YAML frontmatter
/// carrying `name`/`description`; templates without their own frontmatter get
/// one synthesized from the skill id and the first prose line.
fn ensure_skill_frontmatter(id: &str, content: &str) -> String {
    if content.starts_with("---\n") || content.starts_with("---\r\n") {
        return content.to_string();
    }
    let description = frontmatter_description(id, content);
    format!("---\nname: {id}\ndescription: \"{description}\"\n---\n\n{content}")
}

fn frontmatter_description(id: &str, content: &str) -> String {
    let mut heading = String::new();
    let mut body = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(stripped) = trimmed.strip_prefix('#') {
            if heading.is_empty() {
                heading = stripped.trim_start_matches('#').trim().to_string();
            }
            continue;
        }
        body = trimmed.trim_start_matches('-').trim().to_string();
        break;
    }
    let mut description = if body.is_empty() { heading } else { body };
    if description.is_empty() {
        description = format!("LilyGO context skill {id}");
    }
    description = description.replace('\\', "/").replace('"', "'");
    if description.chars().count() > 240 {
        description = description.chars().take(240).collect::<String>() + "...";
    }
    description
}

fn copy_support_tree(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("support directory missing: {}", src.display()));
    }
    if dst.exists() {
        fs::remove_dir_all(dst)
            .map_err(|error| format!("failed to clear {}: {error}", dst.display()))?;
    }
    copy_support_entries(src, dst)
}

fn copy_support_entries(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|error| format!("failed to create {}: {error}", dst.display()))?;
    for entry in
        fs::read_dir(src).map_err(|error| format!("failed to read {}: {error}", src.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read {} entry: {error}", src.display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_support_entries(&from, &to)?;
        } else if from.is_file() {
            fs::copy(&from, &to).map_err(|error| {
                format!(
                    "failed to copy support file {} to {}: {error}",
                    from.display(),
                    to.display()
                )
            })?;
        }
    }
    Ok(())
}

fn reference_skill_files(root: &Path) -> Result<Vec<(String, String)>, String> {
    let index_path = root.join(REFERENCE_INDEX_PATH);
    let data = fs::read_to_string(&index_path)
        .map_err(|error| format!("failed to read {}: {error}", index_path.display()))?;
    let index: ReferenceSkillIndex = serde_json::from_str(&data)
        .map_err(|error| format!("invalid {REFERENCE_INDEX_PATH}: {error}"))?;
    let mut files = Vec::new();
    for entry in index.skills {
        let source = root.join(&entry.source_path);
        let content = fs::read_to_string(&source).map_err(|error| {
            format!(
                "failed to read reference skill {} ({}): {error}",
                entry.id,
                source.display()
            )
        })?;
        let _ = &entry.kind;
        files.push((entry.id, content));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    // Runtime skill generation must be self-contained so install/update can
    // materialize a fresh cache and verify it without committed snapshots.
    #[test]
    fn generated_skill_output_root() {
        let root = root();
        let out = std::env::temp_dir().join(format!("lilygo-generate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        let report = generate_skills(root.as_path(), &out).expect("generate");
        let registry = load_registry(root.as_path()).expect("source registry");
        assert_eq!(report.status, "PASS", "warnings: {:?}", report.warnings);
        assert_eq!(report.skill_count, registry.skills.len());
        assert!(report.board_skills > 0 && report.reference_skills > 0);
        assert_eq!(
            report.playbook_skills,
            crate::playbooks::required_playbook_ids().len()
        );
        assert!(!report.out_root.contains("/Users/"));
        assert!(
            report
                .verification_hints
                .iter()
                .all(|hint| !hint.contains("/Users/"))
        );

        // Every routed skill materialized under <out>/skills and nowhere else.
        for skill in &registry.skills {
            let path = out.join("skills").join(&skill.id).join("SKILL.md");
            assert!(path.is_file(), "missing generated skill {}", skill.id);
        }
        assert!(out.join("index/routes.json").is_file());
        assert!(
            out.join("skills/references/generation-contract.md")
                .is_file()
        );
        assert!(out.join("templates/skills/playbook.md").is_file());
        assert!(!out.join("skills/references/SKILL.md").exists());

        // Generating must never write into committed source-owned trees.
        assert!(generate_skills(root.as_path(), &root.join("skills")).is_err());
        assert!(generate_skills(root.as_path(), &root.join("templates")).is_err());
        assert!(generate_skills(root.as_path(), &root.join("templates/skills")).is_err());
        assert!(generate_skills(root.as_path(), &root.join("data")).is_err());
        assert!(generate_skills(root.as_path(), root.as_path()).is_err());

        let verify = verify_generated_root(root.as_path(), &out);
        assert_eq!(verify.status, "PASS", "verify errors: {:?}", verify.errors);
        assert!(verify.missing.is_empty());
        assert!(verify.extra.is_empty());
        assert!(verify.reference_skills_missing.is_empty());
        assert!(verify.evidence_boundary_ok);
        assert!(!verify.generated_root.contains("/Users/"));
        assert!(
            crate::playbooks::required_playbook_ids()
                .iter()
                .all(|id| out.join("skills").join(id).join("SKILL.md").is_file())
        );

        let _ = fs::remove_dir_all(&out);
    }

    #[cfg(unix)]
    #[test]
    fn source_tree_generation_rejects_symlinked_root() {
        let root = root();
        let link =
            std::env::temp_dir().join(format!("lilygo-source-root-link-{}", std::process::id()));
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&root, &link).expect("symlink source root");

        assert!(is_source_tree_generation_target(root.as_path(), &link));
        assert!(is_source_tree_generation_target(
            root.as_path(),
            &link.join("skills")
        ));
        assert!(is_source_tree_generation_target(
            root.as_path(),
            &link.join("index/routes.json")
        ));
        assert!(is_source_tree_generation_target(
            root.as_path(),
            &link.join("templates")
        ));

        let _ = fs::remove_file(&link);
    }
}

fn source_pack_summary(root: &Path) -> Result<(Vec<String>, BTreeMap<String, String>), String> {
    let mut ids = Vec::new();
    let mut hashes = BTreeMap::new();
    for pack in recipe_registry().source_packs {
        ids.push(pack.id);
        for (reference, hash) in pack.source_hashes {
            hashes.insert(reference, hash);
        }
    }
    let peripheral_path = root.join(PERIPHERAL_PACK_PATH);
    if let Ok(data) = fs::read_to_string(&peripheral_path) {
        let value: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
        let packs = value
            .get("packs")
            .and_then(|packs| packs.as_array())
            .cloned()
            .unwrap_or_default();
        for pack in packs {
            if let Some(id) = pack.get("id").and_then(|id| id.as_str()) {
                ids.push(id.to_string());
            }
        }
    }
    ids.sort();
    ids.dedup();
    Ok((ids, hashes))
}

#[cfg(test)]
mod out_guard_tests {
    use super::*;

    fn source_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    // `generate --out <dir>` must never clear a directory that is not
    // provably a generated root; `--out ~` used to delete `~/skills`.
    #[test]
    fn generate_out_guard_refuses_non_generated_dir() {
        let temp =
            std::env::temp_dir().join(format!("lilygo-skills-out-guard-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join("skills/precious")).unwrap();
        fs::write(temp.join("skills/precious/notes.md"), "user data").unwrap();

        let result = generate_skills(source_root().as_path(), &temp);
        assert!(result.is_err(), "non-generated dir must refuse");
        assert!(
            temp.join("skills/precious/notes.md").is_file(),
            "user data must survive"
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn generate_out_guard_allows_empty_marker_and_legacy_roots() {
        let temp =
            std::env::temp_dir().join(format!("lilygo-skills-out-ok-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        // Empty dir generates and gains the marker.
        generate_skills(source_root().as_path(), &temp).expect("empty dir generates");
        assert!(temp.join(GENERATED_MARKER_FILE).is_file());

        // Marker root regenerates.
        generate_skills(source_root().as_path(), &temp).expect("marker root regenerates");

        // Legacy root (pre-marker layout) migrates: remove the marker only.
        fs::remove_file(temp.join(GENERATED_MARKER_FILE)).unwrap();
        generate_skills(source_root().as_path(), &temp).expect("legacy root regenerates");
        assert!(
            temp.join(GENERATED_MARKER_FILE).is_file(),
            "marker restored"
        );

        let _ = fs::remove_dir_all(&temp);
    }
}
