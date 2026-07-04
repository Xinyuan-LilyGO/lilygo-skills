//! Resolves global and project-scoped preferences into compact hints that guide
//! agents without exposing private local evidence.
use crate::facts;
use crate::model::{
    CodeLimits, HardwareSafety, PreferenceConfig, PreferenceHint, ResolvedPreferences,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const PROJECT_PREFERENCES_PATH: &str = ".lilygo-skills/preferences.json";
const DEFAULTS_PATH: &str = "data/preferences/defaults.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreferencePatch {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    framework_order: Option<Vec<String>>,
    #[serde(default)]
    debug_tools: Option<Vec<String>>,
    #[serde(default)]
    code_limits: Option<CodeLimitPatch>,
    #[serde(default)]
    hardware_safety: Option<HardwareSafetyPatch>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeLimitPatch {
    #[serde(default)]
    max_function_lines: Option<u32>,
    #[serde(default)]
    max_file_lines: Option<u32>,
    #[serde(default)]
    max_nesting: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HardwareSafetyPatch {
    #[serde(default)]
    prefer_dry_run: Option<bool>,
    #[serde(default)]
    require_explicit_flash: Option<bool>,
}

pub(crate) fn resolve_preferences(
    root: &Path,
    project_start: Option<&Path>,
) -> Result<ResolvedPreferences, String> {
    let mut effective = default_preferences();
    let mut sources = vec!["built-in defaults".to_string()];
    let repo_defaults = root.join(DEFAULTS_PATH);
    if repo_defaults.is_file() {
        apply_preference_file(&repo_defaults, &mut effective)?;
        sources.push(DEFAULTS_PATH.to_string());
    }
    let project_file = project_start.and_then(find_project_preferences);
    if let Some(path) = &project_file {
        apply_preference_file(path, &mut effective)?;
        sources.push(PROJECT_PREFERENCES_PATH.to_string());
    }
    Ok(ResolvedPreferences {
        status: "PASS".to_string(),
        project_root: project_file
            .as_ref()
            .and_then(|path| path.parent())
            .and_then(|path| path.parent())
            .map(|path| path.display().to_string()),
        sources,
        effective,
        warnings: vec![
            "preferences guide tool/style/safety behavior only; source facts remain authoritative".to_string(),
            ".lilygo-skills/local.json is the private place for ports, credentials, raw logs, and evidence paths".to_string(),
        ],
    })
}

pub(crate) fn preference_hints_for_prompt(
    root: &Path,
    project_start: Option<&Path>,
    prompt: &str,
) -> Vec<PreferenceHint> {
    if facts::is_fact_prompt(prompt) && !explicit_behavior_prompt(prompt) {
        return Vec::new();
    }
    if !facts::is_implementation_or_debug_prompt(prompt) && !explicit_behavior_prompt(prompt) {
        return Vec::new();
    }
    let Ok(resolved) = resolve_preferences(root, project_start) else {
        return Vec::new();
    };
    let source = resolved.sources.join(" > ");
    let mut hints = vec![
        PreferenceHint {
            key: "framework_order".to_string(),
            value: resolved.effective.framework_order.join(","),
            source: source.clone(),
        },
        PreferenceHint {
            key: "debug_tools".to_string(),
            value: resolved.effective.debug_tools.join(","),
            source: source.clone(),
        },
        PreferenceHint {
            key: "code_limits".to_string(),
            value: format!(
                "max_function_lines={},max_file_lines={},max_nesting={}",
                resolved.effective.code_limits.max_function_lines,
                resolved.effective.code_limits.max_file_lines,
                resolved.effective.code_limits.max_nesting
            ),
            source: source.clone(),
        },
        PreferenceHint {
            key: "hardware_safety".to_string(),
            value: format!(
                "prefer_dry_run={},require_explicit_flash={}",
                resolved.effective.hardware_safety.prefer_dry_run,
                resolved.effective.hardware_safety.require_explicit_flash
            ),
            source,
        },
    ];
    hints.truncate(4);
    hints
}

fn default_preferences() -> PreferenceConfig {
    PreferenceConfig {
        schema_version: 1,
        framework_order: vec![
            "arduino".to_string(),
            "esp-idf".to_string(),
            "platformio".to_string(),
            "rust".to_string(),
        ],
        debug_tools: vec![
            "serial-mcp-server".to_string(),
            "espflash".to_string(),
            "binflow".to_string(),
        ],
        code_limits: CodeLimits {
            max_function_lines: 60,
            max_file_lines: 500,
            max_nesting: 3,
        },
        hardware_safety: HardwareSafety {
            prefer_dry_run: true,
            require_explicit_flash: true,
        },
    }
}

fn apply_preference_file(path: &Path, effective: &mut PreferenceConfig) -> Result<(), String> {
    let data = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    reject_private_fields(&data, path)?;
    let patch: PreferencePatch =
        serde_json::from_str(&data).map_err(|error| format!("invalid preferences: {error}"))?;
    if let Some(version) = patch.schema_version
        && version != 1
    {
        return Err(format!("unsupported preference schema_version: {version}"));
    }
    if let Some(framework_order) = patch.framework_order {
        effective.framework_order = framework_order;
    }
    if let Some(debug_tools) = patch.debug_tools {
        effective.debug_tools = debug_tools;
    }
    if let Some(code_limits) = patch.code_limits {
        if let Some(value) = code_limits.max_function_lines {
            effective.code_limits.max_function_lines = value;
        }
        if let Some(value) = code_limits.max_file_lines {
            effective.code_limits.max_file_lines = value;
        }
        if let Some(value) = code_limits.max_nesting {
            effective.code_limits.max_nesting = value;
        }
    }
    if let Some(hardware_safety) = patch.hardware_safety {
        if let Some(value) = hardware_safety.prefer_dry_run {
            effective.hardware_safety.prefer_dry_run = value;
        }
        if let Some(value) = hardware_safety.require_explicit_flash {
            effective.hardware_safety.require_explicit_flash = value;
        }
    }
    Ok(())
}

fn reject_private_fields(data: &str, _path: &Path) -> Result<(), String> {
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|error| format!("invalid JSON: {error}"))?;
    let mut findings = Vec::new();
    collect_private_findings(&value, "$", &mut findings);
    if findings.is_empty() {
        return Ok(());
    }
    Err(format!(
        "preferences file contains private preference fields or values: {}",
        findings.join(",")
    ))
}

fn collect_private_findings(
    value: &serde_json::Value,
    json_path: &str,
    findings: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let normalized = key.to_lowercase().replace('-', "_");
                if is_private_key(&normalized) {
                    findings.push(format!("{json_path}.{key}:private-key"));
                }
                collect_private_findings(value, &format!("{json_path}.{key}"), findings);
            }
        }
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                collect_private_findings(value, &format!("{json_path}[{index}]"), findings);
            }
        }
        serde_json::Value::String(value) => {
            if let Some(kind) = private_value_kind(value) {
                findings.push(format!("{json_path}:{kind}"));
            }
        }
        _ => {}
    }
}

fn is_private_key(key: &str) -> bool {
    matches!(
        key,
        "port"
            | "serial_port"
            | "wifi"
            | "wifi_ssid"
            | "wifi_password"
            | "password"
            | "token"
            | "secret"
            | "ota_host"
            | "local_evidence"
            | "raw_logs"
            | "evidence_path"
            | "evidence_artifact"
    )
}

fn private_value_kind(value: &str) -> Option<&'static str> {
    let trimmed = value.trim();
    let lower = trimmed.to_lowercase();
    if lower.is_empty() {
        return None;
    }
    if contains_credential_assignment(&lower) {
        return Some("credential-value");
    }
    if contains_serial_device(&lower) {
        return Some("serial-device");
    }
    if contains_private_local_path(&lower) {
        return Some("local-path");
    }
    if contains_private_ipv4(&lower) {
        return Some("private-network-target");
    }
    if lower.contains(".local") {
        return Some("mdns-target");
    }
    if lower.contains("raw_log") || lower.contains("raw log") {
        return Some("raw-log");
    }
    None
}

fn contains_credential_assignment(value: &str) -> bool {
    [
        "token=",
        "access_token=",
        "auth_token=",
        "password=",
        "passwd=",
        "secret=",
        "ssid=",
        "wifi_ssid=",
        "wifi_password=",
        "ota_host=",
        "bearer ",
        "private_key=",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

fn contains_serial_device(value: &str) -> bool {
    value.contains("/dev/cu")
        || value.contains("/dev/tty")
        || value.contains("/dev/serial")
        || is_windows_com_port(value)
}

fn is_windows_com_port(value: &str) -> bool {
    let upper = value.trim().to_ascii_uppercase();
    let Some(rest) = upper.strip_prefix("COM") else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|character| character.is_ascii_digit())
}

fn contains_private_local_path(value: &str) -> bool {
    value.starts_with("/users/")
        || value.starts_with("/home/")
        || value.starts_with("/private/")
        || value.starts_with("/tmp/")
        || value.starts_with("/var/")
        || value.starts_with("file:/")
        || value.contains(".lilygo-skills/evidence")
}

fn contains_private_ipv4(value: &str) -> bool {
    value
        .split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .filter(|part| part.contains('.'))
        .any(is_private_ipv4)
}

fn is_private_ipv4(candidate: &str) -> bool {
    let octets = candidate
        .split('.')
        .filter_map(|part| part.parse::<u8>().ok())
        .collect::<Vec<_>>();
    if octets.len() != 4 {
        return false;
    }
    octets[0] == 10
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
}

fn find_project_preferences(start: &Path) -> Option<PathBuf> {
    let mut path = start.to_path_buf();
    loop {
        let candidate = path.join(PROJECT_PREFERENCES_PATH);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !path.pop() {
            return None;
        }
    }
}

fn explicit_behavior_prompt(prompt: &str) -> bool {
    let normalized = prompt.to_lowercase();
    ["binflow", "serial", "debug", "调试", "style", "preference"]
        .iter()
        .any(|needle| normalized.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn preference_schema() {
        let prefs = resolve_preferences(root().as_path(), None).expect("preferences");
        assert_eq!(prefs.status, "PASS");
        assert!(
            prefs
                .effective
                .framework_order
                .contains(&"arduino".to_string())
        );
        assert!(prefs.effective.debug_tools.contains(&"binflow".to_string()));
        assert!(prefs.effective.hardware_safety.prefer_dry_run);
    }

    #[test]
    fn preference_precedence() {
        let project = std::env::temp_dir().join(format!(
            "lilygo-preference-precedence-{}",
            std::process::id()
        ));
        let prefs_dir = project.join(".lilygo-skills");
        let _ = fs::remove_dir_all(&project);
        fs::create_dir_all(&prefs_dir).expect("prefs dir");
        fs::write(
            prefs_dir.join("preferences.json"),
            r#"{
              "schema_version": 1,
              "framework_order": ["platformio", "arduino"],
              "debug_tools": ["binflow", "serial-mcp-server"],
              "code_limits": {"max_function_lines": 42},
              "hardware_safety": {"require_explicit_flash": true}
            }"#,
        )
        .expect("prefs file");
        let resolved =
            resolve_preferences(root().as_path(), Some(project.as_path())).expect("resolved");
        assert_eq!(resolved.effective.framework_order[0], "platformio");
        assert_eq!(resolved.effective.code_limits.max_function_lines, 42);
        assert!(
            resolved
                .sources
                .contains(&PROJECT_PREFERENCES_PATH.to_string())
        );
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn preference_privacy_boundary() {
        let project =
            std::env::temp_dir().join(format!("lilygo-preference-privacy-{}", std::process::id()));
        let prefs_dir = project.join(".lilygo-skills");
        let _ = fs::remove_dir_all(&project);
        fs::create_dir_all(&prefs_dir).expect("prefs dir");
        fs::write(
            prefs_dir.join("preferences.json"),
            r#"{"schema_version":1,"serial_port":"private-device"}"#,
        )
        .expect("private prefs");
        let error = resolve_preferences(root().as_path(), Some(project.as_path()))
            .expect_err("private preferences must fail");
        assert!(error.contains("private preference fields"));
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn preference_privacy_boundary_rejects_private_values() {
        let project = std::env::temp_dir().join(format!(
            "lilygo-preference-private-values-{}",
            std::process::id()
        ));
        let prefs_dir = project.join(".lilygo-skills");
        let _ = fs::remove_dir_all(&project);
        fs::create_dir_all(&prefs_dir).expect("prefs dir");
        fs::write(
            prefs_dir.join("preferences.json"),
            r#"{
              "schema_version": 1,
              "framework_order": ["arduino"],
              "debug_tools": [
                "/dev/cu.usbmodem-private",
                "token=abc123",
                "host=192.168.1.40",
                "watch.local",
                "/private/source",
                ".lilygo-skills/evidence/raw-log.txt"
              ]
            }"#,
        )
        .expect("private value prefs");
        let error = resolve_preferences(root().as_path(), Some(project.as_path()))
            .expect_err("private preference values must fail");
        assert!(error.contains("private preference fields or values"));
        assert!(error.contains("$.debug_tools[0]:serial-device"));
        assert!(error.contains("$.debug_tools[1]:credential-value"));
        assert!(error.contains("$.debug_tools[2]:private-network-target"));
        assert!(error.contains("$.debug_tools[3]:mdns-target"));
        assert!(error.contains("$.debug_tools[4]:local-path"));
        assert!(error.contains("$.debug_tools[5]:local-path"));
        assert!(!error.contains("abc123"));
        assert!(!error.contains("/dev/cu.usbmodem-private"));
        let _ = fs::remove_dir_all(&project);
    }
}
