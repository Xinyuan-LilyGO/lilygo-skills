//! Adds tool contexts that require compound prompt signals while preserving
//! framework boundaries such as PlatformIO wrapping Arduino or ESP-IDF.
use super::*;

pub(crate) fn add_runner_and_tool_context(
    registry: &Registry,
    prompt: &str,
    selected: &mut BTreeSet<String>,
    matches: &mut Vec<MatchReason>,
) {
    if contains_any(prompt, &["platformio", "pio"]) {
        // PlatformIO is a build host, not a framework: it can wrap Arduino-ESP32
        // or ESP-IDF. Defaulting to Arduino would over-inject a framework the
        // prompt never requested (and conflict with an explicit ESP-IDF/Rust
        // ask). The keyword pass already attaches fw-arduino/fw-esp-idf/fw-rust
        // when the prompt names one, so we only add the PlatformIO tooling here.
        add_skill(
            registry,
            "tool-platformio-cli",
            "tool",
            "PlatformIO CLI requested",
            selected,
            matches,
        );
    }
    // Simple single-skill tool triggers are data-backed via the derived-context
    // catalog (kind = "tool"); only compound-condition tools stay in Rust here.
    if contains_any(prompt, &["install", "setup"]) && contains_any(prompt, &["arduino"]) {
        add_skill(
            registry,
            "tool-arduino-cli",
            "tool",
            "Arduino CLI requested",
            selected,
            matches,
        );
    }
    if contains_any(prompt, &["simulator", "page-data", "screenshot"]) && prompt.contains("lvgl") {
        add_skill(
            registry,
            "tool-lvgl-simulator",
            "tool",
            "LVGL simulator evidence requested",
            selected,
            matches,
        );
    }
    if contains_any(prompt, &["docs", "source lookup", "mcp"])
        && contains_any(prompt, &["esp-idf", "espressif"])
    {
        add_skill(
            registry,
            "tool-espressif-doc-mcp",
            "tool",
            "Espressif source lookup requested",
            selected,
            matches,
        );
    }
}

pub(crate) fn add_when_any(
    registry: &Registry,
    prompt: &str,
    selected: &mut BTreeSet<String>,
    matches: &mut Vec<MatchReason>,
    spec: DerivedSpec<'_>,
) {
    if contains_any(prompt, spec.needles) {
        add_skill(
            registry,
            spec.skill_id,
            spec.kind,
            spec.value,
            selected,
            matches,
        );
    }
}
