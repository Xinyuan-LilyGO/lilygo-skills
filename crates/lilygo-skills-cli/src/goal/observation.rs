//! Runtime observation-line detection for goal command evidence: decides
//! whether serial/monitor output contains the target payload markers.
pub(super) fn observation_payload_seen(text: &str) -> bool {
    text.lines().any(target_observation_line)
}

fn target_observation_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || host_observation_line(trimmed) {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    observation_signal_lower(&lower)
        || contains_any(
            &lower,
            &[
                "esp-rom",
                "rst:",
                "boot:",
                "load:",
                "entry ",
                "guru meditation",
                "[t:",
                " i (",
                " w (",
                " e (",
            ],
        )
}

fn host_observation_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    line.contains("command timed out after")
        || line.contains(" INFO ]")
        || line.contains(" WARN ]")
        || contains_any(
            &lower,
            &[
                "serial port:",
                "connecting",
                "using flash stub",
                "chip type:",
                "crystal is",
                "features:",
                "mac:",
                "stub running",
                "hard resetting",
                "leaving",
                "error:",
            ],
        )
}

pub(super) fn observation_excerpt(value: &str) -> String {
    let mut lines = value
        .lines()
        .take(20)
        .map(str::to_string)
        .collect::<Vec<_>>();
    for line in value.lines().filter(|line| observation_signal_line(line)) {
        if !lines.iter().any(|existing| existing == line) {
            lines.push(line.to_string());
        }
        if lines.len() >= 60 {
            break;
        }
    }
    lines.join("\n").chars().take(6000).collect()
}

fn observation_signal_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    observation_signal_lower(&lower)
}

fn observation_signal_lower(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "product id",
            "kernel version",
            "boot status",
            "host interface",
            "feature status",
            "sensor id",
            "accelerometer",
            "gyroscope",
            "wake gesture",
            "wrist tilt",
            "ax:",
            " ax:",
            " gx:",
            "ay:",
            "gy:",
        ],
    )
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}
