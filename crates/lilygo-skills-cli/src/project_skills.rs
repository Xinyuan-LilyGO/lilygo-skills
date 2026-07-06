//! Project-local custom skill loading for per-firmware operating patterns.
use crate::model::{ProjectSkillEntry, ProjectSkillIndex, Registry, Skill, SkillKind};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) const PROJECT_SKILL_INDEX: &str = ".lilygo-skills/skills/index.json";
const PROJECT_SKILL_ROOT: &str = ".lilygo-skills/skills";

pub(crate) fn registry_with_project_skills(
    registry: &Registry,
    project_root: Option<&Path>,
) -> Result<Registry, String> {
    let Some(project_root) = project_root else {
        return Ok(registry.clone());
    };
    let index_path = project_root.join(PROJECT_SKILL_INDEX);
    if !index_path.is_file() {
        return Ok(registry.clone());
    }
    let data = fs::read_to_string(&index_path)
        .map_err(|error| format!("failed to read project skill index: {error}"))?;
    let index: ProjectSkillIndex = serde_json::from_str(&data)
        .map_err(|error| format!("invalid project skill index: {error}"))?;
    if index.schema_version != 1 {
        return Err("project skill index schema_version must be 1".to_string());
    }
    validate_registered_files(project_root, &index)?;
    let existing = registry
        .skills
        .iter()
        .map(|skill| skill.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut merged = registry.clone();
    for entry in index.skills {
        validate_entry(project_root, &entry, &existing)?;
        merged.skills.push(project_skill(entry));
    }
    Ok(merged)
}

fn project_skill(entry: ProjectSkillEntry) -> Skill {
    Skill {
        id: entry.id,
        kind: parse_kind(&entry.kind),
        path: entry.path,
        summary: entry.summary,
        triggers: entry.triggers,
        aliases: Vec::new(),
        priority: 70,
        verification_level: "project-context".to_string(),
        family_id: None,
        product: false,
    }
}

fn parse_kind(kind: &str) -> SkillKind {
    match kind {
        "application" => SkillKind::Application,
        "tool" => SkillKind::Tool,
        "peripheral" => SkillKind::Peripheral,
        "feature" => SkillKind::Feature,
        "playbook" => SkillKind::Playbook,
        _ => SkillKind::Debug,
    }
}

fn validate_entry(
    project_root: &Path,
    entry: &ProjectSkillEntry,
    existing: &BTreeSet<&str>,
) -> Result<(), String> {
    if !entry.id.starts_with("project-") {
        return Err(format!(
            "project skill id must start with project-: {}",
            entry.id
        ));
    }
    if existing.contains(entry.id.as_str()) {
        return Err(format!(
            "project skill id collides with built-in skill: {}",
            entry.id
        ));
    }
    if entry.summary.trim().is_empty() || entry.triggers.is_empty() {
        return Err(format!(
            "project skill {} requires summary and at least one trigger",
            entry.id
        ));
    }
    let relative = safe_project_skill_path(&entry.path)?;
    let skill_path = project_root.join(&relative);
    let content = fs::read_to_string(&skill_path).map_err(|error| {
        format!(
            "failed to read project skill {} at {}: {error}",
            entry.id,
            relative.display()
        )
    })?;
    if !content.starts_with("---") {
        return Err(format!(
            "project skill {} must start with frontmatter",
            entry.id
        ));
    }
    reject_private_content(&entry.id, &entry.path, &content)?;
    Ok(())
}

fn safe_project_skill_path(path: &str) -> Result<PathBuf, String> {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(format!("project skill path must be relative: {path}"));
    }
    if !path.starts_with(PROJECT_SKILL_ROOT) || !path.ends_with("/SKILL.md") {
        return Err(format!(
            "project skill path must stay under {PROJECT_SKILL_ROOT}/<id>/SKILL.md: {path}"
        ));
    }
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!("project skill path escapes the project: {path}"));
    }
    Ok(candidate.to_path_buf())
}

fn validate_registered_files(project_root: &Path, index: &ProjectSkillIndex) -> Result<(), String> {
    let root = project_root.join(PROJECT_SKILL_ROOT);
    if !root.is_dir() {
        return Ok(());
    }
    let registered = index
        .skills
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();
    for path in skill_files(&root)? {
        let relative = path
            .strip_prefix(project_root)
            .map_err(|error| format!("failed to relativize project skill: {error}"))?
            .to_string_lossy()
            .replace('\\', "/");
        if relative == PROJECT_SKILL_INDEX {
            continue;
        }
        if !registered.contains(relative.as_str()) {
            return Err(format!("unregistered project skill file: {relative}"));
        }
    }
    Ok(())
}

fn skill_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in
        fs::read_dir(root).map_err(|error| format!("failed to read project skills: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read project skill entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            let skill = path.join("SKILL.md");
            if skill.is_file() {
                files.push(skill);
            }
        }
    }
    Ok(files)
}

fn reject_private_content(id: &str, index_path: &str, content: &str) -> Result<(), String> {
    let lower = format!("{index_path}\n{content}").to_lowercase();
    let forbidden = [
        "/users/", "/dev/cu", "/dev/tty", "password", "passwd", "secret", "token", "ssid",
        "192.168.", "10.0.", "172.16.",
    ];
    if let Some(pattern) = forbidden.iter().find(|pattern| lower.contains(**pattern)) {
        return Err(format!(
            "project skill {id} contains private or machine-local content matching {pattern}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::load_registry;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn temp_project(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("lilygo-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join(".lilygo-skills/skills/project-lvgl-loop")).expect("project");
        path
    }

    #[test]
    fn project_custom_skill_route_and_validate() {
        let project = temp_project("project-skill");
        fs::write(
            project.join(PROJECT_SKILL_INDEX),
            r#"{
  "schema_version": 1,
  "skills": [{
    "id": "project-lvgl-loop",
    "kind": "debug",
    "path": ".lilygo-skills/skills/project-lvgl-loop/SKILL.md",
    "summary": "Project LVGL loop checklist.",
    "triggers": ["lvgl", "touch"]
  }]
}"#,
        )
        .expect("index");
        fs::write(
            project.join(".lilygo-skills/skills/project-lvgl-loop/SKILL.md"),
            "---\nname: project-lvgl-loop\n---\n# Project LVGL loop\n",
        )
        .expect("skill");
        let registry = load_registry(root().as_path()).expect("registry");
        let merged =
            registry_with_project_skills(&registry, Some(project.as_path())).expect("merge");
        assert!(
            merged
                .skills
                .iter()
                .any(|skill| skill.id == "project-lvgl-loop")
        );
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn project_custom_skill_rejects_private_content() {
        let project = temp_project("project-skill-private");
        fs::write(
            project.join(PROJECT_SKILL_INDEX),
            r#"{"schema_version":1,"skills":[{"id":"project-debug","kind":"debug","path":".lilygo-skills/skills/project-lvgl-loop/SKILL.md","summary":"debug","triggers":["lvgl"]}]}"#,
        )
        .expect("index");
        fs::write(
            project.join(".lilygo-skills/skills/project-lvgl-loop/SKILL.md"),
            "---\nname: project-debug\n---\n/private path /Users/adan\n",
        )
        .expect("skill");
        let registry = load_registry(root().as_path()).expect("registry");
        let error = registry_with_project_skills(&registry, Some(project.as_path()))
            .expect_err("private content rejected");
        assert!(error.contains("private"));
        let _ = fs::remove_dir_all(project);
    }
}
