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
    let cache_path = cache_dir.join(format!(
        "{}.txt",
        crate::facts::stable_hash(&(host, session_id.as_str(), env!("CARGO_PKG_VERSION")))
    ));
    let unchanged = cache_fresh(&cache_path)
        && fs::read_to_string(&cache_path)
            .ok()
            .is_some_and(|cached| cached == signature);
    if fs::create_dir_all(&cache_dir).is_ok() {
        let _ = fs::write(&cache_path, signature);
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
        SESSION_KEYS.split('|').find_map(|key| {
            value
                .get(key)?
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
    })
}

fn cache_dir() -> Option<PathBuf> {
    env_value("LILYGO_SKILLS_CACHE_DIR")
        .map(PathBuf::from)
        .map(|path| path.join("session-context"))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".cache/lilygo-skills/session-context"))
        })
}

fn ttl_seconds() -> u64 {
    env_value("LILYGO_SKILLS_INCREMENTAL_TTL_SECONDS")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_TTL_SECONDS)
}

fn compact_context(plan: &GoalPlan) -> String {
    let mut critical = Vec::new();
    for fact in plan.context_capsule.critical_facts.iter().take(2) {
        let value = fact.value.rsplit('=').next().unwrap_or(fact.value.as_str());
        critical.push(format!("{}={value}", fact.key));
    }
    let mut next = Vec::new();
    for action in &plan.context_capsule.next_actions {
        if action.id == "source-query-io" || action.id.starts_with("source-query-") {
            next.push(format!("{}:{}", action.id, action.permission));
        }
        if next.len() == 2 {
            break;
        }
    }
    format!(
        "LilyGO incremental: critical=[{}]; next=[{}]; expand=goal plan; evidence_boundary={}/hardware_verified={}",
        critical.join(","),
        next.join(","),
        plan.context_capsule.boundary.verification_level,
        plan.context_capsule.boundary.hardware_verified
    )
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
        let plan = plan(prompt);
        let full = crate::goal::render_hook_goal_summary(&plan);
        let input = r#"{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor","session_id":"session-a"}"#;
        let first = maybe_compact_hook_context("claude", input, full.clone(), Some(&plan));
        let second = maybe_compact_hook_context("claude", input, full.clone(), Some(&plan));
        assert_eq!(first, full);
        assert!(second.contains("LilyGO incremental"));
        assert!(second.contains("pin.i2c.sda"));
        assert!(second.contains("source-query-i2c:none"));
        assert!(second.contains("evidence_boundary=V3/hardware_verified=false"));
        assert!(
            second.len() * 5 <= full.len(),
            "second={second}\nfull={full}"
        );
        let _ = fs::remove_dir_all(&temp);
    }
}
