//! Session-scoped hook context compaction with safe full-context fallback.
use crate::model::GoalPlan;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const DEFAULT_TTL_SECONDS: u64 = 6 * 60 * 60;
const SESSION_KEYS: &str = "session_id|sessionId|conversation_id|conversationId|thread_id|threadId";

pub(crate) fn maybe_compact_hook_context(
    host: &str,
    input: &str,
    full_context: String,
    plan: Option<&GoalPlan>,
) -> String {
    if full_context.is_empty() || incremental_disabled() {
        return full_context;
    }
    let (Some(session_id), Some(cache_dir)) = (session_id(input), cache_dir()) else {
        return full_context;
    };
    let signature = crate::facts::stable_hash(&full_context);
    let cache_key = [host, &session_id, env!("CARGO_PKG_VERSION"), &signature].join("|");
    let cache_path = cache_dir.join(format!("{}.txt", crate::facts::stable_hash(&cache_key)));
    let unchanged = cache_fresh(&cache_path);
    if fs::create_dir_all(&cache_dir).is_ok() {
        let _ = fs::write(&cache_path, "seen");
    }
    if unchanged && let Some(plan) = plan {
        compact_context(plan)
    } else {
        full_context
    }
}

fn cache_fresh(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age.as_secs() < ttl_seconds())
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn incremental_disabled() -> bool {
    env_value("LILYGO_SKILLS_DISABLE_INCREMENTAL")
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn session_id(input: &str) -> Option<String> {
    env_value("LILYGO_SKILLS_SESSION_ID").or_else(|| {
        let value = serde_json::from_str::<serde_json::Value>(input).ok()?;
        SESSION_KEYS
            .split('|')
            .filter_map(|key| value.get(key)?.as_str())
            .map(str::trim)
            .find(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn cache_dir() -> Option<PathBuf> {
    if let Some(path) = env_value("LILYGO_SKILLS_CACHE_DIR") {
        return Some(PathBuf::from(path).join("session-context"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".cache/lilygo-skills/session-context"))
}

fn ttl_seconds() -> u64 {
    env_value("LILYGO_SKILLS_INCREMENTAL_TTL_SECONDS")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_TTL_SECONDS)
}

fn compact_context(plan: &GoalPlan) -> String {
    let topics = plan
        .context_capsule
        .next_actions
        .iter()
        .filter_map(|action| action.id.strip_prefix("source-query-"))
        .filter(|topic| *topic != "io")
        .collect::<Vec<_>>();
    let critical = compact_critical_lines(&plan.context_capsule.critical_facts, &topics);
    let next = plan
        .context_capsule
        .next_actions
        .iter()
        .filter(|action| action.id == "source-query-io" || action.id.starts_with("source-query-"))
        .map(|action| format!("{}:{}", action.id, action.permission))
        .take(3)
        .collect::<Vec<_>>();
    format!(
        "LilyGO incremental: critical=[{}]; next=[{}]; expand=goal plan; evidence_boundary={}/hardware_verified={}",
        critical.join(","),
        next.join(","),
        plan.context_capsule.boundary.verification_level,
        plan.context_capsule.boundary.hardware_verified
    )
}

/// Critical facts must never vanish solely because the topic filter found no
/// match: when the capsule carries critical facts but none share a key with
/// the routed topics, fall back to the board's top facts instead of emitting
/// an empty critical list.
fn compact_critical_lines(
    critical_facts: &[crate::model::GoalCriticalFact],
    topics: &[&str],
) -> Vec<String> {
    let render = |fact: &crate::model::GoalCriticalFact| {
        let value = fact.value.rsplit('=').next().unwrap_or(fact.value.as_str());
        format!("{}={value}", fact.key)
    };
    let filtered = critical_facts
        .iter()
        .filter(|fact| topics.is_empty() || topics.iter().any(|topic| fact.key.contains(*topic)))
        .take(2)
        .map(render)
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        critical_facts.iter().take(2).map(render).collect()
    } else {
        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal::plan_goal;
    use crate::registry::load_registry;
    use crate::router::route_prompt;
    use std::path::Path;

    fn plan(prompt: &str) -> GoalPlan {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = load_registry(root.as_path()).expect("registry");
        let route = route_prompt(&registry, prompt);
        plan_goal(root.as_path(), &registry, prompt, &route).expect("plan")
    }

    #[test]
    fn incremental_context_keeps_critical_facts_and_shrinks_repeats() {
        let temp =
            std::env::temp_dir().join(format!("lilygo-session-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        unsafe {
            std::env::set_var("LILYGO_SKILLS_CACHE_DIR", &temp);
            std::env::remove_var("LILYGO_SKILLS_DISABLE_INCREMENTAL");
        }
        let prompt = "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor";
        let first_plan = plan(prompt);
        let full = crate::goal::render_hook_goal_summary(&first_plan);
        let spi_uart = plan("T-Display-S3 debug an SPI sensor and UART module");
        let spi_uart_full = crate::goal::render_hook_goal_summary(&spi_uart);
        let input = r#"{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor","session_id":"session-a"}"#;
        let spi_uart_input = r#"{"prompt":"T-Display-S3 debug an SPI sensor and UART module","session_id":"session-a"}"#;
        let first = maybe_compact_hook_context("claude", input, full.clone(), Some(&first_plan));
        let second = maybe_compact_hook_context("claude", input, full.clone(), Some(&first_plan));
        let first_spi_uart = maybe_compact_hook_context(
            "claude",
            spi_uart_input,
            spi_uart_full.clone(),
            Some(&spi_uart),
        );
        let second_spi_uart = maybe_compact_hook_context(
            "claude",
            spi_uart_input,
            spi_uart_full.clone(),
            Some(&spi_uart),
        );
        let third = maybe_compact_hook_context("claude", input, full.clone(), Some(&first_plan));
        assert_eq!(first, full);
        assert_eq!(first_spi_uart, spi_uart_full);
        assert!(second.contains("LilyGO incremental"));
        assert!(second.contains("pin.i2c.sda"));
        assert!(second.contains("source-query-i2c:none"));
        assert!(second.contains("evidence_boundary=V3/hardware_verified=false"));
        assert!(second_spi_uart.contains("LilyGO incremental"));
        assert!(second_spi_uart.contains("source-query-spi:none"));
        assert!(second_spi_uart.contains("source-query-uart:none"));
        assert!(!second_spi_uart.contains("pin.i2c.sda"));
        assert!(third.contains("LilyGO incremental"));
        // Efficiency guard (not an honesty/coverage assertion): the incremental
        // repeat must stay far smaller than the full capsule. Injection de-noise
        // shrank the full capsule (dropped goal_id/completeness/fact_tables/
        // discovery_hints counts), so the fixed ratio was retuned from 5x to 4x;
        // it still proves >4x compaction of the repeated context.
        assert!(
            second.len() * 4 <= full.len(),
            "second={second}\nfull={full}"
        );
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn compact_critical_lines_fall_back_when_topics_match_nothing() {
        let fact = |key: &str, value: &str| crate::model::GoalCriticalFact {
            key: key.to_string(),
            value: value.to_string(),
            source: "test://pin_config.h".to_string(),
            evidence_level: "V3-source-reference".to_string(),
        };
        let facts = vec![
            fact("pin.i2c.sda", "PIN_IIC_SDA=GPIO18"),
            fact("pin.i2c.scl", "PIN_IIC_SCL=GPIO17"),
            fact("pin.touch.int", "PIN_TOUCH_INT=GPIO16"),
        ];
        // Topics overlap fact keys: relevance filter applies.
        let matched = compact_critical_lines(&facts, &["i2c"]);
        assert_eq!(matched, vec!["pin.i2c.sda=GPIO18", "pin.i2c.scl=GPIO17"]);
        // Topics match no fact key: must fall back to top facts, never render
        // an empty critical list while the capsule still carries facts.
        let fallback = compact_critical_lines(&facts, &["uart"]);
        assert_eq!(fallback, vec!["pin.i2c.sda=GPIO18", "pin.i2c.scl=GPIO17"]);
        // Empty capsule stays empty: the fallback invents nothing.
        assert!(compact_critical_lines(&[], &["uart"]).is_empty());
    }
}
