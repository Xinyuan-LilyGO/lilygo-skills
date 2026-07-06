//! Install/runtime health checks for the context-injection harness.
use crate::commands::{attach_route_readiness, render_context};
use crate::model::{DoctorCheck, DoctorReport, DoctorSampleInjection};
use crate::registry::{ensure_skill_files, load_registry};
use crate::router::route_prompt;
use std::fs;
use std::path::{Path, PathBuf};

const AGENTS_SECTION_START: &str = "<!-- lilygo-skills:start -->";
const AGENTS_SECTION_END: &str = "<!-- lilygo-skills:end -->";
const SAMPLE_PROMPT: &str = "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen";
const NO_OP_PROMPT: &str = "tomato soup recipe";

pub(crate) fn doctor_report(root: &Path, home: Option<&Path>) -> DoctorReport {
    let mut checks = Vec::new();
    checks.push(binary_check());
    checks.push(path_check(
        "routes",
        root.join("index/routes.json"),
        "route index is present",
    ));
    checks.push(path_check(
        "board-data",
        root.join("data/boards.json"),
        "board source model is present",
    ));
    let registry = load_registry(root);
    match &registry {
        Ok(registry) => match ensure_skill_files(root, registry) {
            Ok(()) => checks.push(check(
                "skills",
                "PASS",
                "skill files are available or generatable",
            )),
            Err(error) => checks.push(check("skills", "FAIL", sanitize(&error))),
        },
        Err(error) => checks.push(check("skills", "FAIL", sanitize(error))),
    }
    let active_home = home
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from));
    if let Some(home) = active_home.as_deref() {
        let host = host_checks(home);
        checks.push(active_wiring_check(&host));
        checks.extend(host);
    } else {
        checks.push(check(
            "active_wiring",
            "WARN",
            "active host wiring not checked because HOME is unavailable",
        ));
    }
    let sample_injection = sample_injection(root, registry.ok());
    checks.push(check(
        "sample-injection",
        &sample_injection.status,
        "sample route and no-op route were evaluated",
    ));
    let failed = checks.iter().any(|check| check.status == "FAIL");
    let runtime_mode = if root.join("skills/lilygo-router/SKILL.md").is_file() {
        "materialized-or-source"
    } else {
        "source-model"
    };
    DoctorReport {
        schema_version: 1,
        status: if failed { "FAIL" } else { "PASS" }.to_string(),
        runtime_mode: runtime_mode.to_string(),
        checks,
        sample_injection,
        warnings: vec![
            "doctor checks context injection and install wiring; it does not prove hardware, OTA, LVGL, serial, RF, or sensor behavior".to_string(),
        ],
    }
}

fn binary_check() -> DoctorCheck {
    match std::env::current_exe() {
        Ok(path) if path.is_file() => check("binary", "PASS", "runtime binary is executable"),
        Ok(_) => check("binary", "FAIL", "current executable path is not a file"),
        Err(error) => check("binary", "FAIL", sanitize(&error.to_string())),
    }
}

fn path_check(id: &str, path: PathBuf, summary: &str) -> DoctorCheck {
    if path.exists() {
        check(id, "PASS", summary)
    } else {
        check(id, "FAIL", format!("{summary} is missing"))
    }
}

fn host_checks(home: &Path) -> Vec<DoctorCheck> {
    vec![
        codex_agents_check(home),
        optional_path_check(
            "claude-skill",
            home.join(".claude/skills/lilygo-skills/SKILL.md"),
            "Claude router skill is present",
        ),
        claude_settings_check(home),
    ]
}

fn active_wiring_check(host: &[DoctorCheck]) -> DoctorCheck {
    if host.iter().any(|check| check.status == "FAIL") {
        return check(
            "active_wiring",
            "FAIL",
            "active host wiring has malformed LilyGO integration",
        );
    }
    if host.iter().any(|check| check.status == "PASS") {
        return check(
            "active_wiring",
            "PASS",
            "active host wiring includes at least one LilyGO integration",
        );
    }
    check(
        "active_wiring",
        "WARN",
        "active host wiring is not installed; run install.js for Codex or Claude integration",
    )
}

fn codex_agents_check(home: &Path) -> DoctorCheck {
    let path = home.join(".codex/AGENTS.md");
    let Ok(data) = fs::read_to_string(path) else {
        return check(
            "codex-agents",
            "WARN",
            "Codex AGENTS.md integration file is not installed",
        );
    };
    let starts = data.matches(AGENTS_SECTION_START).count();
    let ends = data.matches(AGENTS_SECTION_END).count();
    if starts != ends {
        return check(
            "codex-agents",
            "FAIL",
            "Codex AGENTS.md has unbalanced LilyGO section markers",
        );
    }
    if starts > 0 && data.contains("lilygo-skills") {
        return check(
            "codex-agents",
            "PASS",
            "Codex AGENTS.md LilyGO section is wired",
        );
    }
    check(
        "codex-agents",
        "WARN",
        "Codex AGENTS.md exists but has no LilyGO section",
    )
}

fn claude_settings_check(home: &Path) -> DoctorCheck {
    let path = home.join(".claude/settings.json");
    let Ok(data) = fs::read_to_string(path) else {
        return check(
            "claude-hook",
            "WARN",
            "Claude settings.json is not installed",
        );
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
        return check(
            "claude-hook",
            "FAIL",
            "Claude settings.json is not valid JSON",
        );
    };
    let Some(entries) = value
        .pointer("/hooks/UserPromptSubmit")
        .and_then(|value| value.as_array())
    else {
        return check(
            "claude-hook",
            "WARN",
            "Claude UserPromptSubmit hook is not installed",
        );
    };
    let mut saw_lilygo = false;
    for entry in entries {
        let Some(hooks) = entry.get("hooks").and_then(|hooks| hooks.as_array()) else {
            continue;
        };
        for hook in hooks {
            let Some(command) = hook.get("command").and_then(|command| command.as_str()) else {
                continue;
            };
            if !command.contains("lilygo-skills") {
                continue;
            }
            saw_lilygo = true;
            if command.contains("hook claude") {
                return check(
                    "claude-hook",
                    "PASS",
                    "Claude UserPromptSubmit hook is wired",
                );
            }
        }
    }
    if saw_lilygo {
        check(
            "claude-hook",
            "FAIL",
            "Claude LilyGO hook command is malformed",
        )
    } else {
        check(
            "claude-hook",
            "WARN",
            "Claude UserPromptSubmit hook is not installed",
        )
    }
}

fn optional_path_check(id: &str, path: PathBuf, summary: &str) -> DoctorCheck {
    if path.exists() {
        check(id, "PASS", summary)
    } else {
        check(id, "WARN", format!("{summary} is not installed"))
    }
}

fn sample_injection(
    root: &Path,
    registry: Option<crate::model::Registry>,
) -> DoctorSampleInjection {
    let Some(registry) = registry else {
        return DoctorSampleInjection {
            status: "FAIL".to_string(),
            prompt: SAMPLE_PROMPT.to_string(),
            matched_skills: Vec::new(),
            no_op_status: "not_checked".to_string(),
        };
    };
    let mut route = route_prompt(&registry, SAMPLE_PROMPT);
    attach_route_readiness(root, &registry, SAMPLE_PROMPT, &mut route);
    let context = render_context(&route);
    let no_op = route_prompt(&registry, NO_OP_PROMPT);
    let matched = route.skills.clone();
    let sample_ok = route.decision == "inject"
        && matched.iter().any(|skill| skill == "board-t-display-s3")
        && matched.iter().any(|skill| skill == "periph-display")
        && context.contains("LilyGO context injection");
    let no_op_ok = no_op.decision == "no-op" && no_op.skills.is_empty();
    DoctorSampleInjection {
        status: if sample_ok && no_op_ok {
            "PASS"
        } else {
            "FAIL"
        }
        .to_string(),
        prompt: SAMPLE_PROMPT.to_string(),
        matched_skills: matched,
        no_op_status: no_op.decision,
    }
}

fn check(id: &str, status: &str, summary: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        id: id.to_string(),
        status: status.to_string(),
        summary: summary.into(),
    }
}

fn sanitize(value: &str) -> String {
    value
        .replace(std::env::var("HOME").unwrap_or_default().as_str(), "~")
        .replace("/Users/", "/<home>/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn doctor_reports_installed_injection_health() {
        let temp =
            std::env::temp_dir().join(format!("lilygo-doctor-uninstalled-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("home");
        let report = doctor_report(root().as_path(), Some(temp.as_path()));
        assert_eq!(report.status, "PASS");
        assert_eq!(report.sample_injection.status, "PASS");
        assert!(
            report
                .sample_injection
                .matched_skills
                .iter()
                .any(|skill| skill == "board-t-display-s3")
        );
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.id == "active_wiring" && check.status == "WARN")
        );
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn doctor_reports_active_hook_wiring() {
        let temp = std::env::temp_dir().join(format!("lilygo-doctor-wired-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let codex = temp.join(".codex");
        let claude_skill = temp.join(".claude/skills/lilygo-skills");
        fs::create_dir_all(&codex).expect("codex");
        fs::create_dir_all(&claude_skill).expect("claude skill");
        fs::write(
            codex.join("AGENTS.md"),
            format!("{AGENTS_SECTION_START}\nLilyGO lilygo-skills\n{AGENTS_SECTION_END}\n"),
        )
        .expect("agents");
        fs::write(
            claude_skill.join("SKILL.md"),
            "---\nname: lilygo-skills\n---\n",
        )
        .expect("skill");
        fs::create_dir_all(temp.join(".claude")).expect("claude");
        fs::write(
            temp.join(".claude/settings.json"),
            r#"{"hooks":{"UserPromptSubmit":[{"hooks":[{"type":"command","command":"lilygo-skills hook claude"}]}]}}"#,
        )
        .expect("settings");
        let report = doctor_report(root().as_path(), Some(temp.as_path()));
        assert_eq!(report.status, "PASS");
        for id in [
            "active_wiring",
            "codex-agents",
            "claude-skill",
            "claude-hook",
        ] {
            assert!(
                report
                    .checks
                    .iter()
                    .any(|check| check.id == id && check.status == "PASS"),
                "{id}: {:?}",
                report.checks
            );
        }
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn doctor_fails_malformed_lilygo_hook() {
        let temp =
            std::env::temp_dir().join(format!("lilygo-doctor-malformed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join(".claude")).expect("claude");
        fs::write(
            temp.join(".claude/settings.json"),
            r#"{"hooks":{"UserPromptSubmit":[{"hooks":[{"type":"command","command":"lilygo-skills route"}]}]}}"#,
        )
        .expect("settings");
        let report = doctor_report(root().as_path(), Some(temp.as_path()));
        assert_eq!(report.status, "FAIL");
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.id == "claude-hook" && check.status == "FAIL")
        );
        let _ = fs::remove_dir_all(&temp);
    }
}
