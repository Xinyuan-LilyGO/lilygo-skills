//! Source-backed board fact and readiness DTOs shared by source query,
//! enrichment, route readiness, goal capsules, and generated skill checks.
use super::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceFact {
    pub schema_version: u32,
    pub board_id: String,
    pub topic: String,
    pub key: String,
    pub value: String,
    pub claim: String,
    pub source: SourceFactSource,
    pub authority_rank: u32,
    pub evidence_level: String,
    pub stale: bool,
    pub confidence: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SourceFactSource {
    pub kind: String,
    pub path_or_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_range: Option<String>,
    pub hash: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoardFactPack {
    pub schema_version: u32,
    pub board_id: String,
    pub mcu_family: String,
    pub supported: bool,
    pub pin_matrix: Vec<SourceFact>,
    pub bus_matrix: Vec<SourceFact>,
    pub expander_matrix: Vec<SourceFact>,
    pub connector_matrix: Vec<SourceFact>,
    pub peripheral_table: Vec<SourceFact>,
    pub source_refs: Vec<SourceFactSource>,
    pub conflicts: Vec<FactConflict>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoardFactPackIndex {
    pub schema_version: u32,
    pub packs: Vec<BoardFactPack>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FactConflict {
    pub key: String,
    pub selected: String,
    pub rejected: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FactQueryReport {
    pub status: String,
    pub board_id: String,
    pub topic: String,
    pub supported: bool,
    pub fact_pack: BoardFactPack,
    pub facts: Vec<SourceFact>,
    pub unknowns: Vec<SourceFact>,
    pub conflicts: Vec<FactConflict>,
    pub source_refs: Vec<SourceFactSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completeness: Option<CompletenessSignal>,
    pub discovery_hints: Vec<DiscoveryHint>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompletenessSignal {
    pub board_id: String,
    pub topic: String,
    pub completeness: String,
    pub evidence_level: String,
    pub source_query_command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompletenessNextAction {
    pub kind: String,
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompletenessReport {
    pub schema_version: u32,
    pub status: String,
    pub board_id: String,
    pub topic: String,
    pub completeness: String,
    pub required_present: Vec<String>,
    pub required_missing: Vec<String>,
    pub preferred_present: Vec<String>,
    pub preferred_missing: Vec<String>,
    pub facts: Vec<SourceFact>,
    pub source_refs: Vec<SourceFactSource>,
    pub next_actions: Vec<CompletenessNextAction>,
    pub evidence_level: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnrichmentRead {
    pub adapter: String,
    pub authority_rank: u32,
    pub path_or_url: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnrichmentValidation {
    pub contract_status_after_apply: String,
    pub required_present: Vec<String>,
    pub required_missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoardFactEnrichmentReport {
    pub schema_version: u32,
    pub status: String,
    pub dry_run: bool,
    pub board_id: String,
    pub topic: String,
    pub source_adapters: Vec<String>,
    pub planned_reads: Vec<EnrichmentRead>,
    pub parsed_facts: Vec<SourceFact>,
    pub planned_writes: Vec<String>,
    pub writes: Vec<String>,
    pub source_hashes: BTreeMap<String, String>,
    pub validation: EnrichmentValidation,
    pub validation_commands: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FactPackUpdateReport {
    pub status: String,
    pub dry_run: bool,
    pub fact_pack_count: usize,
    pub stale_fact_packs: Vec<String>,
    pub source_hashes: BTreeMap<String, String>,
    pub planned_writes: Vec<String>,
    pub writes: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FactTablePreview {
    pub table: String,
    pub rows: Vec<SourceFact>,
    pub preview_count: usize,
    pub overflow_count: usize,
    pub query_command: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PreferenceConfig {
    pub schema_version: u32,
    pub framework_order: Vec<String>,
    pub debug_tools: Vec<String>,
    pub code_limits: CodeLimits,
    pub hardware_safety: HardwareSafety,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodeLimits {
    pub max_function_lines: u32,
    pub max_file_lines: u32,
    pub max_nesting: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HardwareSafety {
    pub prefer_dry_run: bool,
    pub require_explicit_flash: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedPreferences {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    pub sources: Vec<String>,
    pub effective: PreferenceConfig,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PreferenceHint {
    pub key: String,
    pub value: String,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReferenceEntry {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub applies_to: Vec<String>,
    pub path_or_url: String,
    pub authority: String,
    pub summary: String,
    pub read_when: String,
    pub inject_triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReferenceCatalogReport {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    pub entries: Vec<ReferenceEntry>,
    pub source_health: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReferenceHint {
    pub reference_id: String,
    pub title: String,
    pub path_or_url: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolchainPlan {
    pub id: String,
    pub required_for: Vec<String>,
    pub check: String,
    pub install_hint: String,
    pub mutates: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SetupPlan {
    pub schema_version: u32,
    pub framework: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub status: String,
    pub dry_run: bool,
    pub no_mutation: bool,
    pub host_requirements: Vec<String>,
    pub toolchains: Vec<ToolchainPlan>,
    pub next_commands: Vec<String>,
    pub private_inputs_needed: Vec<String>,
    pub writes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiscoveryHint {
    pub when: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextBudget {
    pub max_fact_rows_per_table: usize,
    pub max_source_refs_inline: usize,
    pub max_discovery_hints_inline: usize,
    pub max_reference_hints_inline: usize,
    pub max_playbook_hints_inline: usize,
    pub overflow_count: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_fact_rows_per_table: 8,
            max_source_refs_inline: 5,
            max_discovery_hints_inline: 5,
            max_reference_hints_inline: 3,
            max_playbook_hints_inline: 3,
            overflow_count: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalBoundary {
    pub verification_level: String,
    pub hardware_verified: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Recipe {
    pub id: String,
    pub purpose: String,
    pub applies_to: Vec<String>,
    pub steps: Vec<RecipeStep>,
    pub expected_observations: Vec<String>,
    pub failure_patterns: Vec<FailurePattern>,
    pub artifacts: Vec<String>,
    pub required_permissions: Vec<String>,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecipeRegistry {
    pub schema_version: u32,
    pub recipes: Vec<Recipe>,
    pub source_packs: Vec<RecipeSourcePack>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecipeSourcePack {
    pub id: String,
    pub recipe_ids: Vec<String>,
    // Operating-pattern refs: below official code/header/example/docs authority.
    pub source_refs: Vec<String>,
    // Official upstream docs/examples that outrank operating-pattern refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub official_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source_hashes: BTreeMap<String, String>,
    pub authority: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecipeStep {
    pub id: String,
    pub title: String,
    pub command: String,
    pub permission: String,
    pub evidence_level: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FailurePattern {
    pub class: String,
    pub signals: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalPrivacy {
    pub committed_state: Vec<String>,
    pub local_state: Vec<String>,
}

pub fn context_injection() -> String {
    "context-injection".to_string()
}
