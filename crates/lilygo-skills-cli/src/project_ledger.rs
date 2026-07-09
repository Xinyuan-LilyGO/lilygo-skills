//! Project-local prompt-safe memory for repeated board capability context.
mod privacy;
mod signature;
mod staleness;
mod time;

use crate::facts::stable_hash;
use crate::model::{GoalPlan, RouteResult};
use crate::project_context::{PROJECT_FILE, ResolvedProjectContext};
use crate::source::write_if_changed;
use crate::text_match::{contains_any, contains_word};
use privacy::validate_public_entry;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use signature::{
    expansion_commands, hash_project_file, project_code_signature, route_dimensions,
    route_signature, source_signature_for_readiness, source_signature_for_route,
};
use staleness::{digest_stale, entry_stale};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use time::{current_timestamp, timestamp_after};

pub const LEDGER_FILE: &str = ".lilygo-skills/ledger.json";
pub const CONTEXT_DIGEST_FILE: &str = ".lilygo-skills/context-digest.json";
const LEDGER_SCHEMA_VERSION: u32 = 1;
const DIGEST_SCHEMA_VERSION: u32 = 1;
const DIGEST_TTL_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProjectLedger {
    pub schema_version: u32,
    #[serde(default)]
    pub capabilities: Vec<CapabilityEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CapabilityEntry {
    pub entry_id: String,
    pub board_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    pub capability: String,
    pub verification_level: String,
    pub status: String,
    pub summary: String,
    pub source_signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_code_signature: Option<String>,
    pub runtime_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_evidence_hash: Option<String>,
    #[serde(default)]
    pub expand_commands: Vec<String>,
    pub verified_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ContextDigestStore {
    pub schema_version: u32,
    #[serde(default)]
    pub digests: Vec<ContextDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextDigest {
    pub digest_id: String,
    pub route_signature: String,
    pub context_signature: String,
    pub critical_fact_signature: String,
    pub source_signature: String,
    pub runtime_version: String,
    pub capsule_class: String,
    pub emitted_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub expand_commands: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RecordInput {
    kind: String,
    board_id: String,
    #[serde(default)]
    framework: Option<String>,
    capability: String,
    verification_level: String,
    #[serde(default)]
    status: Option<String>,
    summary: String,
    source_signature: String,
    #[serde(default)]
    project_code_signature: Option<String>,
    #[serde(default)]
    public_evidence_hash: Option<String>,
    #[serde(default)]
    expand_commands: Vec<String>,
    #[serde(default)]
    verified_at: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectLedgerHints {
    pub mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<ProjectLedgerEntryHint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_digest: Option<ProjectDigestHint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectLedgerEntryHint {
    pub capability: String,
    pub verification_level: String,
    pub status: String,
    pub stale: bool,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_evidence_hash: Option<String>,
    pub expand_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDigestHint {
    pub mode: String,
    pub route_signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub critical_facts_retained: bool,
    pub full_context_command: String,
}

pub fn disabled() -> bool {
    env_true("LILYGO_SKILLS_DISABLE_PROJECT_LEDGER")
}

pub fn show_project_ledger(
    project: Option<&ResolvedProjectContext>,
    start: &Path,
) -> Result<Value, String> {
    if disabled() {
        return Ok(json!({
            "status": "PASS",
            "mode": "disabled",
            "project_root": null,
            "profile": null,
            "capabilities": [],
            "digests": [],
            "warnings": ["LILYGO_SKILLS_DISABLE_PROJECT_LEDGER is set"]
        }));
    }
    let Some(project) = project else {
        return Ok(json!({
            "status": "PASS",
            "mode": "none",
            "project_root": null,
            "searched_from": redact_path(start),
            "profile": null,
            "capabilities": [],
            "digests": [],
            "warnings": ["no .lilygo-skills/project.json found"]
        }));
    };
    let ledger = read_ledger(project.project_root.as_path())?;
    let digests = read_digest_store(project.project_root.as_path())?;
    let code_signature = project_code_signature(project.project_root.as_path());
    let capabilities = ledger
        .capabilities
        .iter()
        .map(|entry| {
            let stale = entry_stale(entry, code_signature.as_deref(), None);
            json!({
                "entry_id": entry.entry_id,
                "board_id": entry.board_id,
                "framework": entry.framework,
                "capability": entry.capability,
                "verification_level": entry.verification_level,
                "status": if stale { "stale" } else { entry.status.as_str() },
                "stale": stale,
                "summary": entry.summary,
                "source_signature": entry.source_signature,
                "project_code_signature": entry.project_code_signature,
                "runtime_version": entry.runtime_version,
                "public_evidence_hash": entry.public_evidence_hash,
                "expand_commands": entry.expand_commands,
                "verified_at": entry.verified_at,
                "updated_at": entry.updated_at,
                "expires_at": entry.expires_at
            })
        })
        .collect::<Vec<_>>();
    let digest_values = digests
        .digests
        .iter()
        .map(|digest| {
            json!({
                "digest_id": digest.digest_id,
                "route_signature": digest.route_signature,
                "context_signature": digest.context_signature,
                "critical_fact_signature": digest.critical_fact_signature,
                "source_signature": digest.source_signature,
                "runtime_version": digest.runtime_version,
                "capsule_class": digest.capsule_class,
                "emitted_at": digest.emitted_at,
                "expires_at": digest.expires_at,
                "stale": digest_stale(digest),
                "expand_commands": digest.expand_commands
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "status": "PASS",
        "mode": "enabled",
        "project_root": redact_path(project.project_root.as_path()),
        "profile": {
            "board_id": project.context.board,
            "framework": project.context.framework,
            "features": project.context.features,
            "preferred_tools": project.context.preferred_tools,
            "source_signature": project.context.source_signature,
            "updated_at": project.context.updated_at
        },
        "capabilities": capabilities,
        "digests": digest_values,
        "warnings": []
    }))
}

pub fn clear_project_ledger(project_root: &Path) -> Result<Vec<String>, String> {
    let mut writes = Vec::new();
    for file in [LEDGER_FILE, CONTEXT_DIGEST_FILE] {
        let path = project_root.join(file);
        if path.is_file() {
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
            writes.push(file.to_string());
        }
    }
    Ok(writes)
}

pub fn record_from_file(project_root: &Path, input: &Path) -> Result<Value, String> {
    let data = fs::read_to_string(input)
        .map_err(|error| format!("failed to read {}: {error}", input.display()))?;
    let record: RecordInput = serde_json::from_str(&data)
        .map_err(|error| format!("invalid ledger record {}: {error}", input.display()))?;
    let entry = capability_entry_from_record(project_root, record)?;
    let mut ledger = read_ledger(project_root)?;
    merge_capability(&mut ledger, entry.clone());
    write_ledger(project_root, &ledger)?;
    Ok(json!({
        "status": "recorded",
        "entry_id": entry.entry_id,
        "privacy": "pass",
        "writes": [LEDGER_FILE]
    }))
}

pub fn hints_for_route(
    project_root: &Path,
    route: &RouteResult,
    prompt: &str,
) -> ProjectLedgerHints {
    if disabled() {
        return ProjectLedgerHints {
            mode: "disabled".to_string(),
            entries: Vec::new(),
            context_digest: None,
            warnings: vec!["LILYGO_SKILLS_DISABLE_PROJECT_LEDGER is set".to_string()],
        };
    }
    if !project_root.join(PROJECT_FILE).is_file() || route.decision != "inject" {
        return empty_hints("none");
    }
    let redo = redo_prompt(prompt);
    let code_signature = project_code_signature(project_root);
    let mut warnings = Vec::new();
    let entries = read_ledger(project_root)
        .map(|ledger| relevant_entries(&ledger, route, prompt, code_signature.as_deref(), redo))
        .unwrap_or_else(|error| {
            warnings.push(error);
            Vec::new()
        });
    let digest = read_digest_store(project_root)
        .ok()
        .and_then(|store| digest_hit(&store, route, prompt, redo));
    let mode = if redo {
        "bypass"
    } else if digest.is_some() {
        "unchanged"
    } else if entries.is_empty() {
        "miss"
    } else {
        "hit"
    };
    ProjectLedgerHints {
        mode: mode.to_string(),
        entries,
        context_digest: digest,
        warnings,
    }
}

pub fn route_json_with_ledger(route: &RouteResult, hints: ProjectLedgerHints) -> Value {
    let mut value = serde_json::to_value(route).unwrap_or_else(|_| json!({}));
    value["project_ledger"] = serde_json::to_value(hints).unwrap_or_else(|_| json!({}));
    value
}

pub fn render_hook_ledger_context(hints: &ProjectLedgerHints) -> String {
    if hints.mode == "none" || hints.mode == "miss" || hints.mode == "disabled" {
        return String::new();
    }
    let entries = hints
        .entries
        .iter()
        .take(2)
        .map(|entry| {
            format!(
                "{}={} status={} stale={}",
                entry.capability, entry.verification_level, entry.status, entry.stale
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let digest = hints
        .context_digest
        .as_ref()
        .map(|digest| digest.mode.as_str())
        .unwrap_or("none");
    format!(
        "; project_ledger(mode={}, previously_verified=[{}], digest={}, expand=project ledger show)",
        hints.mode, entries, digest
    )
}

pub fn maybe_compact_project_hook_context(
    project_root: &Path,
    prompt: &str,
    route: &RouteResult,
    full_context: String,
    plan: Option<&GoalPlan>,
    hints: &ProjectLedgerHints,
) -> String {
    if full_context.is_empty()
        || disabled()
        || !project_root.join(PROJECT_FILE).is_file()
        || redo_prompt(prompt)
    {
        return full_context;
    }
    if hints.context_digest.is_some()
        && let Some(plan) = plan
    {
        return compact_project_context(plan, hints);
    }
    let _ = record_context_digest(project_root, route, &full_context, plan);
    full_context
}

pub fn record_goal_capabilities(
    project_root: &Path,
    prompt: &str,
    plan: &GoalPlan,
    highest_verification_level: &str,
    hardware_verified: bool,
    evidence_path: Option<&str>,
) -> Result<Vec<String>, String> {
    if disabled() || !verified_evidence_level(highest_verification_level, hardware_verified) {
        return Ok(Vec::new());
    }
    let Some(board_id) = plan.route.board.clone() else {
        return Ok(Vec::new());
    };
    let public_hash = evidence_path
        .and_then(|path| hash_project_file(project_root, path).ok())
        .map(|hash| format!("sha256:{hash}"));
    let mut ledger = read_ledger(project_root)?;
    let source_signature =
        source_signature_for_route(&plan.context_capsule.readiness, &plan.route.skills);
    for capability in capabilities_from_plan(prompt, plan) {
        let now = current_timestamp();
        let entry = CapabilityEntry {
            entry_id: capability_entry_id(
                &board_id,
                plan.route.framework.as_deref(),
                &capability,
                &source_signature,
                public_hash.as_deref(),
            ),
            board_id: board_id.clone(),
            framework: plan.route.framework.clone(),
            capability: capability.clone(),
            verification_level: highest_verification_level.to_string(),
            status: "verified".to_string(),
            summary: format!(
                "{capability} previously reached {highest_verification_level} evidence for {board_id}; re-run goal evidence before claiming current behavior."
            ),
            source_signature: source_signature.clone(),
            project_code_signature: project_code_signature(project_root),
            runtime_version: env!("CARGO_PKG_VERSION").to_string(),
            public_evidence_hash: public_hash.clone(),
            expand_commands: vec![
                "lilygo-skills goal evidence --id <goal-id> --json".to_string(),
                format!(
                    "lilygo-skills source query --board {board_id} --topic {} --json",
                    topic_from_capability(&capability)
                ),
            ],
            verified_at: now.clone(),
            updated_at: now,
            expires_at: None,
        };
        validate_public_entry(&entry)?;
        merge_capability(&mut ledger, entry);
    }
    if ledger.capabilities.is_empty() {
        return Ok(Vec::new());
    }
    write_ledger(project_root, &ledger)?;
    Ok(vec![LEDGER_FILE.to_string()])
}

fn capability_entry_from_record(
    project_root: &Path,
    record: RecordInput,
) -> Result<CapabilityEntry, String> {
    if record.kind != "capability" {
        return Err(format!("unsupported ledger record kind: {}", record.kind));
    }
    let now = current_timestamp();
    let mut status = record.status.unwrap_or_else(|| {
        if record.verification_level.starts_with("V4")
            || record.verification_level.starts_with("V5")
        {
            if record.public_evidence_hash.is_some() {
                "verified"
            } else {
                "stale"
            }
        } else {
            "source_ready"
        }
        .to_string()
    });
    if status == "verified"
        && !(record.public_evidence_hash.is_some()
            && (record.verification_level.starts_with("V4")
                || record.verification_level.starts_with("V5")))
    {
        status = "stale".to_string();
    }
    let entry = CapabilityEntry {
        entry_id: capability_entry_id(
            &record.board_id,
            record.framework.as_deref(),
            &record.capability,
            &record.source_signature,
            record.public_evidence_hash.as_deref(),
        ),
        board_id: record.board_id,
        framework: record.framework,
        capability: record.capability,
        verification_level: record.verification_level,
        status,
        summary: record.summary,
        source_signature: record.source_signature,
        project_code_signature: record
            .project_code_signature
            .or_else(|| project_code_signature(project_root)),
        runtime_version: env!("CARGO_PKG_VERSION").to_string(),
        public_evidence_hash: record.public_evidence_hash,
        expand_commands: record.expand_commands,
        verified_at: record.verified_at.unwrap_or_else(|| now.clone()),
        updated_at: now,
        expires_at: record.expires_at,
    };
    validate_public_entry(&entry)?;
    Ok(entry)
}

fn read_ledger(project_root: &Path) -> Result<ProjectLedger, String> {
    let path = project_root.join(LEDGER_FILE);
    if !path.is_file() {
        return Ok(ProjectLedger {
            schema_version: LEDGER_SCHEMA_VERSION,
            capabilities: Vec::new(),
            updated_at: None,
        });
    }
    let data = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&data).map_err(|error| format!("invalid {}: {error}", path.display()))
}

fn write_ledger(project_root: &Path, ledger: &ProjectLedger) -> Result<(), String> {
    let mut ledger = ledger.clone();
    ledger.schema_version = LEDGER_SCHEMA_VERSION;
    ledger.updated_at = Some(current_timestamp());
    for entry in &ledger.capabilities {
        validate_public_entry(entry)?;
    }
    let path = project_root.join(LEDGER_FILE);
    let rendered = serde_json::to_vec_pretty(&ledger)
        .map_err(|error| format!("failed to render project ledger: {error}"))?;
    write_if_changed(&path, &rendered)?;
    Ok(())
}

fn read_digest_store(project_root: &Path) -> Result<ContextDigestStore, String> {
    let path = project_root.join(CONTEXT_DIGEST_FILE);
    if !path.is_file() {
        return Ok(ContextDigestStore {
            schema_version: DIGEST_SCHEMA_VERSION,
            digests: Vec::new(),
            updated_at: None,
        });
    }
    let data = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&data).map_err(|error| format!("invalid {}: {error}", path.display()))
}

fn write_digest_store(project_root: &Path, store: &ContextDigestStore) -> Result<(), String> {
    let mut store = store.clone();
    store.schema_version = DIGEST_SCHEMA_VERSION;
    store.updated_at = Some(current_timestamp());
    let path = project_root.join(CONTEXT_DIGEST_FILE);
    let rendered = serde_json::to_vec_pretty(&store)
        .map_err(|error| format!("failed to render context digest: {error}"))?;
    write_if_changed(&path, &rendered)?;
    Ok(())
}

fn record_context_digest(
    project_root: &Path,
    route: &RouteResult,
    context: &str,
    plan: Option<&GoalPlan>,
) -> Result<(), String> {
    let route_signature = route_signature(route);
    let source_signature = source_signature_for_readiness(route);
    let critical_fact_signature = plan
        .map(|plan| {
            stable_hash(
                &plan
                    .context_capsule
                    .critical_facts
                    .iter()
                    .map(|fact| format!("{}={}", fact.key, fact.value))
                    .collect::<Vec<_>>()
                    .join("|"),
            )
        })
        .unwrap_or_else(|| stable_hash(&String::new()));
    let digest = ContextDigest {
        digest_id: stable_hash(&format!(
            "{}|{}|{}|{}",
            route_signature,
            source_signature,
            env!("CARGO_PKG_VERSION"),
            stable_hash(&context.to_string())
        )),
        route_signature,
        context_signature: stable_hash(&context.to_string()),
        critical_fact_signature,
        source_signature,
        runtime_version: env!("CARGO_PKG_VERSION").to_string(),
        capsule_class: "hook-full".to_string(),
        emitted_at: current_timestamp(),
        expires_at: timestamp_after(DIGEST_TTL_SECONDS),
        expand_commands: expansion_commands(route),
    };
    let mut store = read_digest_store(project_root)?;
    store
        .digests
        .retain(|existing| existing.route_signature != digest.route_signature);
    store.digests.push(digest);
    store.digests.retain(|digest| !digest_stale(digest));
    write_digest_store(project_root, &store)
}

fn digest_hit(
    store: &ContextDigestStore,
    route: &RouteResult,
    prompt: &str,
    redo: bool,
) -> Option<ProjectDigestHint> {
    if redo || prompt.trim().is_empty() {
        return None;
    }
    let route_signature = route_signature(route);
    let source_signature = source_signature_for_readiness(route);
    store
        .digests
        .iter()
        .find(|digest| {
            digest.runtime_version == env!("CARGO_PKG_VERSION")
                && digest.route_signature == route_signature
                && digest.source_signature == source_signature
                && !digest_stale(digest)
        })
        .map(|digest| ProjectDigestHint {
            mode: "unchanged".to_string(),
            route_signature,
            expires_at: Some(digest.expires_at.clone()),
            critical_facts_retained: true,
            full_context_command: digest
                .expand_commands
                .first()
                .cloned()
                .unwrap_or_else(|| "lilygo-skills source query --json".to_string()),
        })
}

fn relevant_entries(
    ledger: &ProjectLedger,
    route: &RouteResult,
    prompt: &str,
    code_signature: Option<&str>,
    redo: bool,
) -> Vec<ProjectLedgerEntryHint> {
    let dims = route_dimensions(route);
    let source_signature = source_signature_for_readiness(route);
    ledger
        .capabilities
        .iter()
        .filter(|entry| entry_relevant(entry, prompt, &dims))
        .take(4)
        .map(|entry| {
            let stale = entry_stale(entry, code_signature, Some(source_signature.as_str()));
            ProjectLedgerEntryHint {
                capability: entry.capability.clone(),
                verification_level: entry.verification_level.clone(),
                status: if stale {
                    "stale".to_string()
                } else {
                    entry.status.clone()
                },
                stale,
                summary: if redo {
                    format!(
                        "Previously verified {capability}; explicit re-run requested, so use full context and re-verify.",
                        capability = entry.capability
                    )
                } else {
                    entry.summary.clone()
                },
                public_evidence_hash: entry.public_evidence_hash.clone(),
                expand_commands: entry.expand_commands.clone(),
            }
        })
        .collect()
}

fn entry_relevant(entry: &CapabilityEntry, prompt: &str, dims: &BTreeSet<String>) -> bool {
    if !dims.contains(&entry.board_id) {
        return false;
    }
    if let Some(framework) = &entry.framework
        && dims.iter().any(|dim| dim.starts_with("fw-"))
        && !dims.contains(framework)
    {
        return false;
    }
    let prompt = prompt.to_lowercase();
    capability_terms(&entry.capability)
        .iter()
        .any(|term| contains_word(&prompt, term) || dims.contains(term.as_str()))
}

fn capability_terms(capability: &str) -> Vec<String> {
    capability
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| part.len() >= 3)
        .map(|part| part.to_lowercase())
        .collect()
}

fn compact_project_context(plan: &GoalPlan, hints: &ProjectLedgerHints) -> String {
    let critical = plan
        .context_capsule
        .critical_facts
        .iter()
        .take(3)
        .map(|fact| format!("{}={}", fact.key, fact.value))
        .collect::<Vec<_>>()
        .join(",");
    let entries = hints
        .entries
        .iter()
        .take(2)
        .map(|entry| format!("{}:{}", entry.capability, entry.status))
        .collect::<Vec<_>>()
        .join(",");
    let next = plan
        .context_capsule
        .next_actions
        .iter()
        .filter(|action| action.id.starts_with("source-query-"))
        .map(|action| format!("{}:{}", action.id, action.permission))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "LilyGO project ledger: context unchanged; critical=[{}]; previously_verified=[{}]; next=[{}]; expand=project ledger show; evidence_boundary={}/hardware_verified={}",
        critical,
        entries,
        next,
        plan.context_capsule.boundary.verification_level,
        plan.context_capsule.boundary.hardware_verified
    )
}

fn merge_capability(ledger: &mut ProjectLedger, entry: CapabilityEntry) {
    ledger
        .capabilities
        .retain(|existing| existing.entry_id != entry.entry_id);
    ledger.capabilities.push(entry);
    ledger.capabilities.sort_by(|left, right| {
        left.board_id
            .cmp(&right.board_id)
            .then(left.capability.cmp(&right.capability))
    });
}

fn capabilities_from_plan(prompt: &str, plan: &GoalPlan) -> Vec<String> {
    let mut capabilities = BTreeSet::new();
    for chip in &plan.route.chips {
        capabilities.insert(match chip.as_str() {
            "chip-bhi260ap" => "imu.bhi260ap".to_string(),
            "chip-st25r3916" => "nfc.st25r3916".to_string(),
            "chip-sx1262" => "lora.sx1262".to_string(),
            other => other.trim_start_matches("chip-").replace('-', "."),
        });
    }
    for peripheral in &plan.route.peripherals {
        capabilities.insert(peripheral.trim_start_matches("periph-").to_string());
    }
    if capabilities.is_empty() {
        for topic in &["display", "imu", "ota", "lvgl", "lora", "gnss", "nfc"] {
            if contains_word(prompt, topic) {
                capabilities.insert((*topic).to_string());
            }
        }
    }
    capabilities.into_iter().collect()
}

fn topic_from_capability(capability: &str) -> &str {
    capability.split('.').next().unwrap_or(capability)
}

fn verified_evidence_level(level: &str, hardware_verified: bool) -> bool {
    hardware_verified || level.starts_with("V4") || level.starts_with("V5")
}

fn capability_entry_id(
    board_id: &str,
    framework: Option<&str>,
    capability: &str,
    source_signature: &str,
    public_evidence_hash: Option<&str>,
) -> String {
    stable_hash(&format!(
        "{}|{}|{}|{}|{}",
        board_id,
        framework.unwrap_or(""),
        capability,
        source_signature,
        public_evidence_hash.unwrap_or("")
    ))
}

fn redo_prompt(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    contains_any(
        &lower,
        &[
            "re-verify",
            "reverify",
            "re-run",
            "rerun",
            "re-implement",
            "重新验证",
            "重新跑",
            "重新实现",
            "再验证",
        ],
    )
}

fn empty_hints(mode: &str) -> ProjectLedgerHints {
    ProjectLedgerHints {
        mode: mode.to_string(),
        entries: Vec::new(),
        context_digest: None,
        warnings: Vec::new(),
    }
}

fn redact_path(path: &Path) -> String {
    path.file_name()
        .map(|name| format!("<project>/{}", name.to_string_lossy()))
        .unwrap_or_else(|| "<project>".to_string())
}

fn env_true(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RouteResult;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn temp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lilygo-ledger-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".lilygo-skills")).unwrap();
        fs::write(
            dir.join(PROJECT_FILE),
            r#"{"schema_version":1,"board":"board-t-watch-ultra","framework":"fw-arduino","features":[]}"#,
        )
        .unwrap();
        dir
    }

    fn route() -> RouteResult {
        RouteResult {
            decision: "inject".to_string(),
            skills: vec![
                "lilygo-router".to_string(),
                "board-t-watch-ultra".to_string(),
                "fw-arduino".to_string(),
                "periph-imu".to_string(),
            ],
            matches: Vec::new(),
            paths: BTreeMap::new(),
            readiness: Vec::new(),
            missing: Vec::new(),
            questions: Vec::new(),
            verification_level: "context-injection".to_string(),
            hardware_verified: false,
            hardware_verification_boundary: true,
            notes: Vec::new(),
            truncated: false,
            board_source: None,
        }
    }

    fn record(dir: &Path, capability: &str) -> PathBuf {
        let path = dir.join("record.json");
        let source_signature = source_signature_for_readiness(&route());
        fs::write(
            &path,
            format!(
                r#"{{
  "kind":"capability",
  "board_id":"board-t-watch-ultra",
  "framework":"fw-arduino",
  "capability":"{capability}",
  "verification_level":"V5",
  "summary":"{capability} previously reached V5 build/upload/serial evidence on a redacted report.",
  "source_signature":"{source_signature}",
  "public_evidence_hash":"sha256:evidence",
  "expand_commands":["lilygo-skills source query --board board-t-watch-ultra --topic imu --json"]
}}"#
            ),
        )
        .unwrap();
        path
    }

    #[test]
    fn project_ledger_profile_defaults_and_prompt_override() {
        let dir = temp_project("profile-defaults");
        let report = show_project_ledger(None, dir.as_path()).unwrap();
        assert_eq!(report["mode"], "none");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_verified_capability_compacts_hook() {
        let dir = temp_project("verified-hit");
        record_from_file(
            dir.as_path(),
            record(dir.as_path(), "imu.bhi260ap").as_path(),
        )
        .unwrap();
        let hints = hints_for_route(dir.as_path(), &route(), "T-Watch Ultra IMU debug");
        assert_eq!(hints.mode, "hit");
        assert_eq!(hints.entries[0].capability, "imu.bhi260ap");
        assert!(render_hook_ledger_context(&hints).contains("previously"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_context_digest_invalidation() {
        let dir = temp_project("digest");
        let route = route();
        let full = "full LilyGO context with source-query-imu:none";
        record_context_digest(dir.as_path(), &route, full, None).unwrap();
        let first = hints_for_route(dir.as_path(), &route, "T-Watch Ultra IMU debug");
        assert_eq!(first.mode, "unchanged");
        unsafe {
            std::env::set_var("LILYGO_SKILLS_DISABLE_PROJECT_LEDGER", "1");
        }
        let disabled = hints_for_route(dir.as_path(), &route, "T-Watch Ultra IMU debug");
        assert_eq!(disabled.mode, "disabled");
        unsafe {
            std::env::remove_var("LILYGO_SKILLS_DISABLE_PROJECT_LEDGER");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_unknown_capability_not_recorded() {
        let dir = temp_project("unknown");
        let hints = hints_for_route(dir.as_path(), &route(), "T-Watch Ultra barometer");
        assert!(hints.entries.is_empty());
        assert!(!dir.join(LEDGER_FILE).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_budget_and_relevance() {
        let dir = temp_project("budget");
        record_from_file(
            dir.as_path(),
            record(dir.as_path(), "imu.bhi260ap").as_path(),
        )
        .unwrap();
        record_from_file(
            dir.as_path(),
            record(dir.as_path(), "display.st7789").as_path(),
        )
        .unwrap();
        let hints = hints_for_route(dir.as_path(), &route(), "T-Watch Ultra IMU debug");
        assert_eq!(hints.entries.len(), 1);
        assert_eq!(hints.entries[0].capability, "imu.bhi260ap");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_code_drift_marks_stale() {
        let dir = temp_project("drift");
        let path = record(dir.as_path(), "imu.bhi260ap");
        let mut input = fs::read_to_string(&path).unwrap();
        input = input.replace(
            r#""public_evidence_hash":"sha256:evidence","#,
            r#""project_code_signature":"sha256:old","public_evidence_hash":"sha256:evidence","#,
        );
        fs::write(&path, input).unwrap();
        record_from_file(dir.as_path(), path.as_path()).unwrap();
        fs::write(dir.join("main.cpp"), "changed").unwrap();
        let hints = hints_for_route(dir.as_path(), &route(), "T-Watch Ultra IMU debug");
        assert!(hints.entries[0].stale);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_code_drift_uses_file_content() {
        let dir = temp_project("content-drift");
        fs::write(dir.join("main.cpp"), "aaaa").unwrap();
        record_from_file(
            dir.as_path(),
            record(dir.as_path(), "imu.bhi260ap").as_path(),
        )
        .unwrap();
        fs::write(dir.join("main.cpp"), "bbbb").unwrap();
        let hints = hints_for_route(dir.as_path(), &route(), "T-Watch Ultra IMU debug");
        assert!(hints.entries[0].stale);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_source_signature_mismatch_marks_stale() {
        let dir = temp_project("source-drift");
        let path = record(dir.as_path(), "imu.bhi260ap");
        let mut input = fs::read_to_string(&path).unwrap();
        input = input.replace(
            &source_signature_for_readiness(&route()),
            "sha256:obsolete-source",
        );
        fs::write(&path, input).unwrap();
        record_from_file(dir.as_path(), path.as_path()).unwrap();
        let hints = hints_for_route(dir.as_path(), &route(), "T-Watch Ultra IMU debug");
        assert!(hints.entries[0].stale);
        assert_eq!(hints.entries[0].status, "stale");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_record_cannot_force_verified_without_evidence() {
        let dir = temp_project("missing-evidence");
        let path = record(dir.as_path(), "imu.bhi260ap");
        let mut input = fs::read_to_string(&path).unwrap();
        input = input.replace(r#""public_evidence_hash":"sha256:evidence","#, "");
        input = input.replace(
            r#""verification_level":"V5","#,
            r#""verification_level":"V5","status":"verified","#,
        );
        fs::write(&path, input).unwrap();
        record_from_file(dir.as_path(), path.as_path()).unwrap();
        let project = crate::project_context::resolve_project_context(dir.as_path()).unwrap();
        let report = show_project_ledger(project.as_ref(), dir.as_path()).unwrap();
        assert_eq!(report["capabilities"][0]["status"], "stale");
        assert_eq!(report["capabilities"][0]["stale"], true);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_redo_override_bypasses_hit() {
        let dir = temp_project("redo");
        record_from_file(
            dir.as_path(),
            record(dir.as_path(), "imu.bhi260ap").as_path(),
        )
        .unwrap();
        let hints = hints_for_route(dir.as_path(), &route(), "re-verify T-Watch Ultra IMU");
        assert_eq!(hints.mode, "bypass");
        assert!(hints.entries[0].summary.contains("re-run requested"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_suppression_requires_provable_emission() {
        let dir = temp_project("provable");
        let hints = hints_for_route(dir.as_path(), &route(), "T-Watch Ultra IMU debug");
        assert_ne!(hints.mode, "unchanged");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_ledger_privacy_rejects_secret_values() {
        let dir = temp_project("privacy");
        let path = record(dir.as_path(), "imu.bhi260ap");
        let mut input = fs::read_to_string(&path).unwrap();
        input = input.replace("redacted report", "/dev/cu.usbmodem-private");
        fs::write(&path, input).unwrap();
        let err = record_from_file(dir.as_path(), path.as_path()).unwrap_err();
        assert!(err.contains("private pattern"));
        let _ = fs::remove_dir_all(&dir);
    }
}
