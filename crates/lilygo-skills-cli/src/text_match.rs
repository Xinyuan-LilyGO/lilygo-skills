//! Prompt matching helpers shared by routing, playbooks, and benchmarks.

pub(crate) fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| contains_word(haystack, needle))
}

/// Lowercase a value and collapse every run of non-alphanumeric characters into
/// a single `-`, trimming leading/trailing separators. Shared by fact-pack keys
/// and generated peripheral/chip/feature skill ids so both stay byte-identical.
pub(crate) fn slug(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub(crate) fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    if !needle.is_ascii() {
        return haystack.contains(needle);
    }
    contains_ascii_word(haystack, needle)
}

fn contains_ascii_word(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut start = 0;
    while let Some(offset) = haystack[start..].find(needle) {
        let begin = start + offset;
        let end = begin + needle.len();
        let before_ok = begin == 0 || !bytes[begin - 1].is_ascii_alphanumeric();
        let after_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        start = next_char_boundary(haystack, begin);
        if start >= haystack.len() {
            break;
        }
    }
    false
}

fn next_char_boundary(value: &str, index: usize) -> usize {
    value[index..]
        .chars()
        .next()
        .map(|ch| index + ch.len_utf8())
        .unwrap_or(value.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_cjk_needles_match_when_adjacent_to_ascii() {
        assert!(contains_word("t-display-s3烧录失败", "烧录"));
        assert!(contains_word("t-watch ultra imu抬腕检测怎么做", "抬腕"));
    }

    #[test]
    fn utf8_ascii_needles_scan_past_cjk_without_panicking() {
        assert!(!contains_word("t-display-s3烧录失败", "displayx"));
        assert!(contains_word("t-display-s3烧录失败", "t-display-s3"));
    }

    #[test]
    fn ascii_short_triggers_keep_word_boundaries() {
        assert!(!contains_word("gpio", "pio"));
        assert!(!contains_word("platformio", "tf"));
        assert!(!contains_word("export", "port"));
        assert!(contains_word("pio run", "pio"));
        assert!(contains_word("tf card", "tf"));
        assert!(contains_word("serial port", "port"));
    }
}
