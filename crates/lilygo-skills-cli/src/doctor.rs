//! Install/runtime health checks for the context-injection harness.
use crate::commands::{attach_route_readiness, render_context};
use crate::model::{DoctorCheck, DoctorReport, DoctorSampleInjection};
use crate::registry::{ensure_skill_files, load_registry};
use crate::router::route_prompt;
use std::fs;
use std::path::{Path, PathBuf};

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
    if let Some(home) = home {
        checks.extend(host_checks(home));
    } else {
        checks.push(check(
            "host-install",
            "WARN",
            "host integration not checked; pass --home <dir> to validate installed hooks",
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
        optional_path_check(
            "codex-agents",
            home.join(".codex/AGENTS.md"),
            "Codex AGENTS.md integration file is present",
        ),
        optional_path_check(
            "claude-skill",
            home.join(".claude/skills/lilygo-skills/SKILL.md"),
            "Claude router skill is present",
        ),
        claude_settings_check(home),
    ]
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
    let has_hook = value
        .pointer("/hooks/UserPromptSubmit")
        .and_then(|value| value.as_array())
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry
                    .get("hooks")
                    .and_then(|hooks| hooks.as_array())
                    .is_some_and(|hooks| {
                        hooks.iter().any(|hook| {
                            hook.get("command")
                                .and_then(|command| command.as_str())
                                .is_some_and(|command| {
                                    command.contains("lilygo-skills")
                                        && command.contains("hook claude")
                                })
                        })
                    })
            })
        });
    if has_hook {
        check(
            "claude-hook",
            "PASS",
            "Claude UserPromptSubmit hook is wired",
        )
    } else {
        check(
            "claude-hook",
            "FAIL",
            "Claude UserPromptSubmit hook is not wired",
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
        let report = doctor_report(root().as_path(), None);
        assert_eq!(report.status, "PASS");
        assert_eq!(report.sample_injection.status, "PASS");
        assert!(
            report
                .sample_injection
                .matched_skills
                .iter()
                .any(|skill| skill == "board-t-display-s3")
        );
    }
}
