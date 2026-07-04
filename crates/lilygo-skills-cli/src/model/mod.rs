//! Public data contracts for registry routing, generated skills, source facts,
//! project context, goal planning, setup, references, and verification reports.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Registry {
    pub schema_version: u32,
    pub skills: Vec<Skill>,
    pub route_fixtures: Vec<RouteFixture>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Skill {
    pub id: String,
    pub kind: SkillKind,
    pub path: String,
    pub summary: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default = "context_injection")]
    pub verification_level: String,
    #[serde(default)]
    pub family_id: Option<String>,
    #[serde(default)]
    pub product: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SkillKind {
    Router,
    Board,
    Framework,
    Peripheral,
    Chip,
    Feature,
    Debug,
    Application,
    Tool,
    Playbook,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteFixture {
    pub id: String,
    pub prompt: String,
    pub expect_decision: String,
    #[serde(default)]
    pub expect_skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchReason {
    pub skill: String,
    pub matched: MatchedTerm,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchedTerm {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteResult {
    pub decision: String,
    pub skills: Vec<String>,
    pub matches: Vec<MatchReason>,
    pub paths: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readiness: Vec<CompletenessSignal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<ClarificationQuestion>,
    pub verification_level: String,
    pub hardware_verified: bool,
    pub hardware_verification_boundary: bool,
    pub notes: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClarificationQuestion {
    pub id: String,
    pub prompt: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    pub status: String,
    pub skill_count: usize,
    pub route_count: usize,
    pub fixture_count: usize,
    pub source_manifest_status: String,
    pub board_index_status: String,
    pub reference_skills: ReferenceSkillReport,
    pub source_references: SourceReferenceReport,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReferenceSkillReport {
    pub required: usize,
    pub present: usize,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceReferenceReport {
    pub official_urls_checked: usize,
    pub documentation_repo_status: String,
    pub recipe_source_pack_status: String,
    pub fact_pack_status: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoardIndex {
    pub schema_version: u32,
    pub boards: Vec<BoardRecord>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoardRecord {
    pub id: String,
    #[serde(default)]
    pub family_id: Option<String>,
    #[serde(default)]
    pub product: bool,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub mcu: String,
    pub supported: bool,
    pub frameworks: Vec<String>,
    pub peripherals: Vec<String>,
    pub repo_url: String,
    pub wiki_url: String,
    pub source_status: String,
    #[serde(default)]
    pub source_urls: Vec<SourceUrl>,
    #[serde(default)]
    pub source_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub peripheral_matrix: Vec<PeripheralRecord>,
    #[serde(default)]
    pub demo_refs: Vec<DemoRef>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceUrl {
    pub kind: String,
    pub url: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeripheralRecord {
    pub category: String,
    pub name: String,
    pub chip: String,
    pub bus: String,
    pub driver: String,
    pub source_url: String,
    pub source_status: String,
    pub evidence_level: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DemoRef {
    pub framework: String,
    pub target: String,
    pub source_url: String,
    pub path: String,
    pub stale: bool,
    pub source_status: String,
    pub evidence_level: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActiveProfile {
    pub board: String,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectContext {
    pub schema_version: u32,
    pub board: String,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductCandidate {
    pub id: String,
    pub family_id: Option<String>,
    pub slug: String,
    pub wiki_url: String,
    pub repo_url: String,
    pub supported: bool,
    pub source_status: String,
    pub stale: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceModeReport {
    pub github_org: String,
    pub wiki: String,
    pub documentation_repo: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncPreview {
    pub status: String,
    pub mode: String,
    pub dry_run: bool,
    pub sources: SourceModeReport,
    pub source_count: usize,
    pub repo_count: usize,
    pub wiki_page_count: usize,
    pub generated_candidate_count: usize,
    pub product_candidate_count: usize,
    pub unsupported_count: usize,
    pub candidate_route_ids: Vec<String>,
    pub candidates: Vec<BoardRecord>,
    pub product_candidates: Vec<ProductCandidate>,
    pub planned_writes: Vec<String>,
    pub writes: Vec<String>,
    pub warnings: Vec<String>,
    pub source_manifest: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdatePreview {
    pub status: String,
    pub target: String,
    pub dry_run: bool,
    pub source_families: Vec<String>,
    pub cache_status: String,
    pub stale_status: String,
    pub source_count: usize,
    pub board_count: usize,
    pub generated_candidate_count: usize,
    pub product_candidate_count: usize,
    pub unsupported_count: usize,
    pub product_candidates: Vec<ProductCandidate>,
    pub stale_product_records: Vec<String>,
    pub planned_fetches: Vec<String>,
    pub planned_writes: Vec<String>,
    pub writes: Vec<String>,
    pub warnings: Vec<String>,
    pub compatibility_notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeripheralSourcePackIndex {
    pub schema_version: u32,
    pub packs: Vec<PeripheralSourcePack>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeripheralSourcePack {
    pub id: String,
    pub board_id: String,
    pub peripheral: String,
    pub chip: String,
    pub aliases: Vec<String>,
    pub sources: Vec<SourcePackSource>,
    pub framework_refs: Vec<FrameworkRef>,
    pub feature_refs: Vec<FeatureRef>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourcePackSource {
    pub kind: String,
    pub authority_rank: u32,
    pub url: String,
    pub evidence_level: String,
    pub stale: bool,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FrameworkRef {
    pub framework: String,
    pub target: String,
    pub path: String,
    pub source_url: String,
    pub evidence_level: String,
    pub stale: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeatureRef {
    pub feature: String,
    pub guidance_level: String,
    pub requires_calibration: bool,
    pub hardware_verified: bool,
    pub evidence_level: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcePackUpdateReport {
    pub status: String,
    pub dry_run: bool,
    pub source_pack_count: usize,
    pub stale_source_packs: Vec<String>,
    pub planned_writes: Vec<String>,
    pub writes: Vec<String>,
    pub packs: Vec<SourcePackSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcePackSummary {
    pub id: String,
    pub board_id: String,
    pub peripheral: String,
    pub chip: String,
    pub source_dimensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeripheralSkillUpdateReport {
    pub status: String,
    pub dry_run: bool,
    pub source_pack_count: usize,
    pub generated_skill_count: usize,
    pub generated_route_count: usize,
    pub stale_source_packs: Vec<String>,
    pub planned_writes: Vec<String>,
    pub writes: Vec<String>,
    pub skill_ids: Vec<String>,
    pub route_fixture_ids: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HardwareProfile {
    pub board: String,
    pub framework: String,
    #[serde(default)]
    pub port: Option<String>,
    #[serde(default)]
    pub simulator: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub verification_level: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HardwareVerifyReport {
    pub status: String,
    pub verification_level: String,
    pub profile: String,
    pub board: String,
    pub framework: String,
    pub capabilities: Vec<String>,
    pub boundaries: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalPlan {
    pub schema_version: u32,
    pub status: String,
    pub goal_id: String,
    pub prompt: String,
    pub decision: String,
    pub route: GoalRoute,
    pub context_capsule: GoalContextCapsule,
    pub recipe_ids: Vec<String>,
    pub recipes: Vec<Recipe>,
    // Source packs backing the selected recipes: official upstream refs plus
    // official operating patterns, so OTA/LVGL/LoRa plans cite real sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_packs: Vec<RecipeSourcePack>,
    pub permissions_required: Vec<String>,
    pub planned_artifacts: Vec<String>,
    pub privacy: GoalPrivacy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<ClarificationQuestion>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalRoute {
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub peripherals: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chips: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applications: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub playbooks: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalContextCapsule {
    pub summary: String,
    pub facts: Vec<GoalFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fact_tables: Vec<FactTablePreview>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub completeness: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readiness: Vec<CompletenessSignal>,
    pub demo_refs: Vec<GoalDemoRef>,
    pub source_refs: Vec<GoalSourceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferences: Vec<PreferenceHint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_hints: Vec<ReferenceHint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub playbook_hints: Vec<PlaybookHint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discovery_hints: Vec<DiscoveryHint>,
    #[serde(default)]
    pub budget: ContextBudget,
    pub boundary: GoalBoundary,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalFact {
    pub key: String,
    pub value: String,
    pub source: String,
    pub evidence_level: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlaybookCatalog {
    pub schema_version: u32,
    pub playbooks: Vec<Playbook>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Playbook {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub domains: Vec<String>,
    pub applies_to: Vec<String>,
    pub trigger_terms: Vec<String>,
    pub load_when: String,
    pub authority: String,
    pub source_refs: Vec<String>,
    pub required_board_facts: Vec<String>,
    pub diagnostic_axes: Vec<String>,
    pub steps: Vec<String>,
    pub failure_classes: Vec<String>,
    pub evidence_targets: Vec<String>,
    pub anti_claims: Vec<String>,
    pub resource_hints: Vec<String>,
    pub benchmark_prompts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlaybookHint {
    pub playbook_id: String,
    pub title: String,
    pub reason: String,
    pub expand_command: String,
    pub evidence_targets: Vec<String>,
    pub anti_claims: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalDemoRef {
    pub framework: String,
    pub target: String,
    pub path: String,
    pub source_url: String,
    pub evidence_level: String,
    pub stale: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalSourceRef {
    pub kind: String,
    pub authority_rank: u32,
    pub url: String,
    pub status: String,
    pub stale: bool,
    pub evidence_level: String,
}

mod detail;
pub use detail::*;
