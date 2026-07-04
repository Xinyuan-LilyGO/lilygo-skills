//! Project-local board, framework, feature, generated-cache, and private
//! evidence boundary management under `.lilygo-skills`.
use crate::generate::GENERATED_CACHE_DIR;
use crate::model::{ActiveProfile, ProjectContext, Registry, SkillKind};
use crate::source::write_if_changed;
use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECT_FILE: &str = ".lilygo-skills/project.json";
pub const LOCAL_FILE: &str = ".lilygo-skills/local.json";
pub const EVIDENCE_DIR: &str = ".lilygo-skills/evidence/";

#[derive(Debug, Clone)]
pub struct ResolvedProjectContext {
    pub project_root: PathBuf,
    pub context: ProjectContext,
    pub local_evidence_present: bool,
}

impl ProjectContext {
    pub fn active_profile(&self) -> ActiveProfile {
        ActiveProfile {
            board: self.board.clone(),
            framework: self.framework.clone(),
            features: self.features.clone(),
        }
    }
}

pub fn new_project_context(
    registry: &Registry,
    board: &str,
    framework: Option<&str>,
    features: Vec<String>,
) -> Result<ProjectContext, String> {
    validate_skill(registry, board, SkillKind::Board, "board")?;
    if let Some(framework_id) = framework {
        validate_framework(registry, framework_id)?;
    }
    for feature in &features {
        validate_skill(registry, feature, SkillKind::Feature, "feature")?;
    }
    Ok(ProjectContext {
        schema_version: 1,
        board: board.to_string(),
        framework: framework.map(str::to_string),
        features,
        notes: Some("Project defaults only; no machine-specific evidence.".to_string()),
    })
}

pub fn write_project_context(
    project_root: &Path,
    context: &ProjectContext,
) -> Result<Vec<String>, String> {
    fs::create_dir_all(project_root)
        .map_err(|error| format!("failed to create {}: {error}", project_root.display()))?;
    let project_path = project_root.join(PROJECT_FILE);
    let rendered = serde_json::to_vec_pretty(context)
        .map_err(|error| format!("failed to render project context: {error}"))?;
    let mut writes = Vec::new();
    if write_if_changed(&project_path, &rendered)? {
        writes.push(PROJECT_FILE.to_string());
    }
    if ensure_local_ignore(project_root)? {
        writes.push(".gitignore".to_string());
    }
    Ok(writes)
}

pub fn clear_project_context(project_root: &Path) -> Result<Vec<String>, String> {
    let path = project_root.join(PROJECT_FILE);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    fs::remove_file(&path)
        .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
    Ok(vec![PROJECT_FILE.to_string()])
}

pub fn resolve_project_context(start: &Path) -> Result<Option<ResolvedProjectContext>, String> {
    let mut cursor = start.to_path_buf();
    loop {
        let path = cursor.join(PROJECT_FILE);
        if path.is_file() {
            let context = read_project_context(&path)?;
            return Ok(Some(ResolvedProjectContext {
                local_evidence_present: cursor.join(LOCAL_FILE).is_file(),
                project_root: cursor,
                context,
            }));
        }
        if !cursor.pop() {
            return Ok(None);
        }
    }
}

pub fn read_project_context(path: &Path) -> Result<ProjectContext, String> {
    let data = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str::<ProjectContext>(&data)
        .map_err(|error| format!("invalid project context {}: {error}", path.display()))
}

pub fn ensure_local_ignore(project_root: &Path) -> Result<bool, String> {
    let path = project_root.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let required = [
        LOCAL_FILE.to_string(),
        EVIDENCE_DIR.to_string(),
        format!("{GENERATED_CACHE_DIR}/"),
    ];
    let existing_lines = existing
        .lines()
        .map(str::trim)
        .collect::<std::collections::BTreeSet<_>>();
    let missing = required
        .iter()
        .filter(|line| !existing_lines.contains(line.as_str()))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(false);
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    for line in missing {
        updated.push_str(line);
        updated.push('\n');
    }
    fs::write(&path, updated)
        .map_err(|error| format!("failed to update {}: {error}", path.display()))?;
    Ok(true)
}

fn validate_framework(registry: &Registry, framework: &str) -> Result<(), String> {
    let exists = registry.skills.iter().any(|skill| {
        skill.id == framework && matches!(skill.kind, SkillKind::Framework | SkillKind::Tool)
    });
    if exists {
        Ok(())
    } else {
        Err(format!("unknown framework skill: {framework}"))
    }
}

fn validate_skill(
    registry: &Registry,
    skill_id: &str,
    expected: SkillKind,
    label: &str,
) -> Result<(), String> {
    let exists = registry
        .skills
        .iter()
        .any(|skill| skill.id == skill_id && skill.kind == expected);
    if exists {
        Ok(())
    } else {
        Err(format!("unknown {label} skill: {skill_id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::load_registry;

    fn registry() -> Registry {
        load_registry(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
        )
        .unwrap()
    }

    fn temp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lilygo-skills-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("firmware/src")).unwrap();
        dir
    }

    #[test]
    fn project_config_schema() {
        let registry = registry();
        let dir = temp_project("project-config");
        let context = new_project_context(
            &registry,
            "board-t-watch-ultra",
            Some("fw-arduino"),
            vec!["feature-raise-to-wake".to_string()],
        )
        .unwrap();
        let writes = write_project_context(&dir, &context).unwrap();
        assert!(writes.contains(&PROJECT_FILE.to_string()));
        assert!(writes.contains(&".gitignore".to_string()));

        let stored = fs::read_to_string(dir.join(PROJECT_FILE)).unwrap();
        assert!(stored.contains("board-t-watch-ultra"));
        assert!(!stored.contains("serial_port"));
        assert!(!stored.contains("wifi_password"));
        assert!(
            fs::read_to_string(dir.join(".gitignore"))
                .unwrap()
                .contains(LOCAL_FILE)
        );
        assert!(
            fs::read_to_string(dir.join(".gitignore"))
                .unwrap()
                .contains(GENERATED_CACHE_DIR)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_config_init_creates_project_root() {
        let registry = registry();
        let dir =
            std::env::temp_dir().join(format!("lilygo-skills-missing-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let context =
            new_project_context(&registry, "board-t-watch-ultra", None, Vec::new()).unwrap();
        let writes = write_project_context(&dir, &context).unwrap();
        assert!(dir.is_dir());
        assert!(dir.join(PROJECT_FILE).is_file());
        assert!(writes.contains(&PROJECT_FILE.to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_config_private_boundary() {
        let registry = registry();
        let dir = temp_project("local-private");
        let context =
            new_project_context(&registry, "board-t-watch-ultra", None, Vec::new()).unwrap();
        write_project_context(&dir, &context).unwrap();
        fs::write(
            dir.join(LOCAL_FILE),
            r#"{"schema_version":1,"serial_port":"/dev/cu.synthetic-private"}"#,
        )
        .unwrap();
        let resolved = resolve_project_context(&dir).unwrap().expect("context");
        assert!(resolved.local_evidence_present);
        assert!(
            fs::read_to_string(dir.join(".gitignore"))
                .unwrap()
                .lines()
                .any(|line| line.trim() == LOCAL_FILE)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_context_walks_upward() {
        let registry = registry();
        let dir = temp_project("walk-up");
        let context =
            new_project_context(&registry, "board-t-watch-ultra", None, Vec::new()).unwrap();
        write_project_context(&dir, &context).unwrap();

        let resolved = resolve_project_context(&dir.join("firmware/src"))
            .unwrap()
            .expect("context");
        assert_eq!(resolved.project_root, dir);
        assert_eq!(resolved.context.board, "board-t-watch-ultra");
        assert!(!resolved.local_evidence_present);
        let _ = fs::remove_dir_all(resolved.project_root);
    }

    #[test]
    fn project_context_rejects_private_fields() {
        let dir = temp_project("private-fields");
        let context_dir = dir.join(".lilygo-skills");
        fs::create_dir_all(&context_dir).unwrap();
        fs::write(
            context_dir.join("project.json"),
            r#"{"schema_version":1,"board":"board-t-watch-ultra","serial_port":"/dev/cu.synthetic-private"}"#,
        )
        .unwrap();
        let error = resolve_project_context(&dir).unwrap_err();
        assert!(error.contains("unknown field"));
        let _ = fs::remove_dir_all(&dir);
    }
}
