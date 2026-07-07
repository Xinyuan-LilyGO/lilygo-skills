//! Prompt-safety checks for public project-ledger records.
use super::CapabilityEntry;

pub(super) fn validate_public_entry(entry: &CapabilityEntry) -> Result<(), String> {
    let rendered = serde_json::to_string(entry)
        .map_err(|error| format!("failed to inspect ledger privacy: {error}"))?;
    if let Some(pattern) = private_pattern(&rendered) {
        return Err(format!(
            "project ledger contains private pattern: {pattern}"
        ));
    }
    Ok(())
}

fn private_pattern(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let markers = [
        ("/dev/cu", "serial_port"),
        ("/dev/tty", "serial_port"),
        ("wifi_password", "credential"),
        ("wireless_key", "credential"),
        ("access_token", "credential"),
        ("auth_token", "credential"),
        ("bearer ", "credential"),
        ("private_key", "credential"),
        ("192.168.", "private_ipv4"),
        ("169.254.", "private_ipv4"),
        ("mdns", "mdns"),
    ];
    for (needle, name) in markers {
        if lower.contains(needle) {
            return Some(name);
        }
    }
    if contains_private_ipv4(&lower) {
        return Some("private_ipv4");
    }
    if contains_mac_address(&lower) {
        return Some("mac_address");
    }
    None
}

fn contains_private_ipv4(text: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .filter(|part| part.matches('.').count() == 3)
        .any(|part| {
            let nums = part
                .split('.')
                .filter_map(|chunk| chunk.parse::<u8>().ok())
                .collect::<Vec<_>>();
            nums.len() == 4
                && (nums[0] == 10
                    || (nums[0] == 172 && (16..=31).contains(&nums[1]))
                    || (nums[0] == 192 && nums[1] == 168)
                    || (nums[0] == 169 && nums[1] == 254))
        })
}

fn contains_mac_address(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let clean =
            token.trim_matches(|ch: char| !ch.is_ascii_hexdigit() && ch != ':' && ch != '-');
        let sep = if clean.contains(':') { ':' } else { '-' };
        let parts = clean.split(sep).collect::<Vec<_>>();
        parts.len() == 6
            && parts
                .iter()
                .all(|part| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_hexdigit()))
    })
}
