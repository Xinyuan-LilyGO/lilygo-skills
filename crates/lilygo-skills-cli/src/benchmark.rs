//! Route and goal benchmark harness used as a completeness gate for skill
//! injection, negative routing, generated registries, and goal capsules.
use crate::goal::{GoalCompleteOptions, GoalStartOptions, complete_goal, plan_goal};
use crate::model::{GoalPlan, Registry, RouteFixture};
use crate::router::route_prompt;
use serde::Serialize;
use std::collections::BTreeSet;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub status: String,
    pub build_profile: String,
    pub iterations: usize,
    pub case_count: usize,
    pub baseline_comparison: BenchmarkBaselineComparison,
    pub total_routes: usize,
    pub elapsed_ns: u128,
    pub ns_per_route: f64,
    pub routes_per_second: f64,
    pub checksum: usize,
    pub coverage: BenchmarkCoverage,
    pub correctness: BenchmarkCorrectness,
    pub goal_capsules: GoalBenchmark,
    pub goal_complete: GoalBenchmark,
    pub playbook_quality: PlaybookQualityBenchmark,
    pub performance_budget: Option<PerformanceBudget>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkCoverage {
    pub registered_skill_count: usize,
    pub covered_skill_count: usize,
    pub missing_skills: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkBaselineComparison {
    pub baseline: String,
    pub baseline_skill_count: usize,
    pub baseline_case_count: usize,
    pub added_case_count: usize,
    pub required_added_cases: usize,
    pub status: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkCorrectness {
    pub status: String,
    pub positive_case_count: usize,
    pub fixture_case_count: usize,
    pub negative_case_count: usize,
    pub failures: Vec<BenchmarkFailure>,
}
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkFailure {
    pub case: String,
    pub prompt: String,
    pub reason: String,
    pub expected_skills: Vec<String>,
    pub forbidden_skills: Vec<String>,
    pub actual_decision: String,
    pub actual_skills: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct GoalBenchmark {
    pub status: String,
    pub case_count: usize,
    pub failures: Vec<GoalBenchmarkFailure>,
}
#[derive(Debug, Clone, Serialize)]
pub struct GoalBenchmarkFailure {
    pub case: String,
    pub prompt: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct PlaybookQualityBenchmark {
    pub status: String,
    pub case_count: usize,
    pub failures: Vec<PlaybookQualityFailure>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PlaybookQualityFailure {
    pub case: String,
    pub prompt: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceBudget {
    pub max_ns_per_route: u128,
    pub status: String,
}
struct BenchmarkCase {
    name: String,
    prompt: String,
    expected_decision: String,
    expected_skills: Vec<String>,
    forbidden_skills: Vec<String>,
    expected_level: Option<String>,
    case_kind: CaseKind,
}
#[derive(Clone, Copy)]
enum CaseKind {
    Positive,
    Fixture,
    Negative,
}

pub fn run_benchmark(
    root: &Path,
    registry: &Registry,
    iterations: usize,
    max_ns_per_route: Option<u128>,
) -> BenchmarkReport {
    let iterations = iterations.max(1);
    let cases = benchmark_cases(registry);
    let failures = validate_cases(registry, &cases);
    let coverage = coverage(registry, &cases, &failures);
    let goal_capsules = validate_goal_capsules(root, registry);
    let goal_complete = validate_goal_complete(root, registry);
    let playbook_quality = validate_playbook_quality(root, registry);
    let stats = measure_routes(registry, &cases, iterations);
    let baseline_comparison = baseline_comparison(cases.len());
    let performance_budget = performance_budget(max_ns_per_route, stats.ns_per_route);
    let correctness_status = if failures.is_empty() { "PASS" } else { "FAIL" };
    let status = overall_status(
        correctness_status,
        goal_capsules.status.as_str(),
        goal_complete.status.as_str(),
        playbook_quality.status.as_str(),
        &performance_budget,
        &baseline_comparison,
    );
    let warnings = warnings();

    BenchmarkReport {
        status,
        build_profile: build_profile().to_string(),
        iterations,
        case_count: cases.len(),
        baseline_comparison,
        total_routes: stats.total_routes,
        elapsed_ns: stats.elapsed_ns,
        ns_per_route: stats.ns_per_route,
        routes_per_second: stats.routes_per_second,
        checksum: stats.checksum,
        coverage,
        correctness: BenchmarkCorrectness {
            status: correctness_status.to_string(),
            positive_case_count: count_kind(&cases, CaseKind::Positive),
            fixture_case_count: count_kind(&cases, CaseKind::Fixture),
            negative_case_count: count_kind(&cases, CaseKind::Negative),
            failures,
        },
        goal_capsules,
        goal_complete,
        playbook_quality,
        performance_budget,
        warnings,
    }
}
fn benchmark_cases(registry: &Registry) -> Vec<BenchmarkCase> {
    let mut cases = Vec::new();
    for skill in &registry.skills {
        let trigger = skill
            .triggers
            .first()
            .or_else(|| skill.aliases.first())
            .map(String::as_str)
            .unwrap_or("lilygo");
        cases.push(BenchmarkCase {
            name: format!("positive:{}", skill.id),
            prompt: positive_prompt(&skill.id, trigger),
            expected_decision: "inject".to_string(),
            expected_skills: vec![skill.id.clone()],
            forbidden_skills: Vec::new(),
            expected_level: Some("context-injection".to_string()),
            case_kind: CaseKind::Positive,
        });
    }
    cases.extend(registry.route_fixtures.iter().map(fixture_case));
    cases.extend(negative_cases());
    cases
}

fn positive_prompt(skill_id: &str, trigger: &str) -> String {
    if skill_id.starts_with("playbook-") {
        return format!("LilyGO {trigger} debug implementation benchmark validation");
    }
    if skill_id.starts_with("feature-") {
        return format!("T-Watch Ultra IMU {trigger} benchmark validation");
    }
    if skill_id == "chip-bhi260ap" || skill_id == "periph-imu" {
        return format!("T-Watch Ultra {trigger} benchmark validation");
    }
    format!("LilyGO {trigger} benchmark validation")
}
fn fixture_case(fixture: &RouteFixture) -> BenchmarkCase {
    BenchmarkCase {
        name: format!("fixture:{}", fixture.id),
        prompt: fixture.prompt.clone(),
        expected_decision: fixture.expect_decision.clone(),
        expected_skills: fixture.expect_skills.clone(),
        forbidden_skills: Vec::new(),
        expected_level: None,
        case_kind: CaseKind::Fixture,
    }
}
fn negative_cases() -> Vec<BenchmarkCase> {
    vec![
        negative_decision(
            "negative:non-lilygo",
            "Generic ESP32 LVGL screen is blank",
            "no-op",
            None,
        ),
        negative_decision(
            "negative:unsupported-rp2040",
            "LilyGO RP2040 display example",
            "no-op",
            Some("unsupported"),
        ),
        forbid(
            "negative:gpio-not-pio",
            "LilyGO T-Display-S3 GPIO pinout",
            &["fw-platformio", "tool-platformio-cli", "fw-arduino"],
        ),
        expect_and_forbid(
            "negative:platformio-idf",
            "LilyGO ESP32-S3 PlatformIO ESP-IDF upload and monitor",
            &["fw-platformio", "tool-platformio-cli", "fw-esp-idf"],
            &["fw-arduino", "tool-serial-debug"],
        ),
        forbid(
            "negative:export-not-port",
            "LilyGO T-Display-S3 export GPIO matrix",
            &["tool-serial-debug", "fw-platformio", "tool-platformio-cli"],
        ),
        expect_and_forbid(
            "negative:lora-gps-split",
            "LilyGO T-Beam LoRa GPS example",
            &["periph-lora", "periph-gps"],
            &["periph-lora-gps"],
        ),
        expect_and_forbid(
            "negative:watch-display-not-imu",
            "T-Watch Ultra AMOLED brightness",
            &["board-t-watch-ultra", "periph-display"],
            &["periph-imu", "chip-bhi260ap", "feature-raise-to-wake"],
        ),
        expect_and_forbid(
            "negative:watch-nfc-not-imu-feature",
            "T-Watch Ultra ST25R3916 NFC reader Arduino example",
            &["board-t-watch-ultra", "fw-arduino", "chip-st25r3916"],
            &["periph-imu", "chip-bhi260ap", "feature-raise-to-wake"],
        ),
        expect_and_forbid(
            "positive:watch-ultra-xl9555-fact-route",
            "T-Watch Ultra XL9555 GPIO expander 哪些口连接按键和外设?",
            &["board-t-watch-ultra", "chip-xl9555"],
            &["feature-raise-to-wake"],
        ),
        expect_and_forbid(
            "negative:watch-ultra-io-fact-lookup",
            "T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?",
            &["board-t-watch-ultra", "fw-arduino"],
            &[
                "tool-serial-debug",
                "tool-platformio-cli",
                "feature-raise-to-wake",
            ],
        ),
        negative_decision(
            "negative:missing-board-clarification",
            "Arduino IMU 抬腕检测怎么做",
            "needs_clarification",
            Some("none"),
        ),
        expect_and_forbid(
            "positive:exact-t-display-s3-completeness-route",
            "T-Display-S3 Arduino LVGL display demo",
            &[
                "board-t-display-s3",
                "fw-arduino",
                "fw-lvgl",
                "periph-display",
            ],
            &["board-t-display"],
        ),
        expect_and_forbid(
            "positive:cjk-adjacent-t-display-s3-flash",
            "T-Display-S3烧录失败",
            &[
                "board-t-display-s3",
                "periph-display",
                "playbook-source-discovery",
                "playbook-build-flash-serial",
            ],
            &["board-t-display", "playbook-ota-debug"],
        ),
        expect_and_forbid(
            "positive:cjk-adjacent-watch-ultra-imu",
            "t-watch ultra imu抬腕检测怎么做",
            &[
                "board-t-watch-ultra",
                "periph-imu",
                "chip-bhi260ap",
                "feature-raise-to-wake",
                "playbook-source-discovery",
            ],
            &["board-t-watch", "periph-display"],
        ),
    ]
}

fn negative_decision(
    name: &str,
    prompt: &str,
    expected_decision: &str,
    expected_level: Option<&str>,
) -> BenchmarkCase {
    BenchmarkCase {
        name: name.to_string(),
        prompt: prompt.to_string(),
        expected_decision: expected_decision.to_string(),
        expected_skills: Vec::new(),
        forbidden_skills: Vec::new(),
        expected_level: expected_level.map(str::to_string),
        case_kind: CaseKind::Negative,
    }
}

fn forbid(name: &str, prompt: &str, forbidden_skills: &[&str]) -> BenchmarkCase {
    expect_and_forbid(name, prompt, &[], forbidden_skills)
}

fn expect_and_forbid(
    name: &str,
    prompt: &str,
    expected_skills: &[&str],
    forbidden_skills: &[&str],
) -> BenchmarkCase {
    BenchmarkCase {
        name: name.to_string(),
        prompt: prompt.to_string(),
        expected_decision: "inject".to_string(),
        expected_skills: expected_skills
            .iter()
            .map(|skill| skill.to_string())
            .collect(),
        forbidden_skills: forbidden_skills
            .iter()
            .map(|skill| skill.to_string())
            .collect(),
        expected_level: Some("context-injection".to_string()),
        case_kind: CaseKind::Negative,
    }
}
fn validate_cases(registry: &Registry, cases: &[BenchmarkCase]) -> Vec<BenchmarkFailure> {
    let mut failures = Vec::new();
    for case in cases {
        let result = route_prompt(registry, &case.prompt);
        let actual: BTreeSet<&str> = result.skills.iter().map(String::as_str).collect();
        let expected: BTreeSet<&str> = case.expected_skills.iter().map(String::as_str).collect();
        let forbidden: BTreeSet<&str> = case.forbidden_skills.iter().map(String::as_str).collect();
        let missing: Vec<&str> = expected.difference(&actual).copied().collect();
        let forbidden_present: Vec<&str> = forbidden.intersection(&actual).copied().collect();
        let decision_bad = result.decision != case.expected_decision;
        let level_bad = case
            .expected_level
            .as_ref()
            .is_some_and(|level| result.verification_level != *level);
        if missing.is_empty() && forbidden_present.is_empty() && !decision_bad && !level_bad {
            continue;
        }
        failures.push(failure(case, &result, missing, forbidden_present));
    }
    failures
}
fn failure(
    case: &BenchmarkCase,
    result: &crate::model::RouteResult,
    missing: Vec<&str>,
    forbidden_present: Vec<&str>,
) -> BenchmarkFailure {
    let reason = if result.decision != case.expected_decision {
        format!("expected decision {}", case.expected_decision)
    } else if case
        .expected_level
        .as_ref()
        .is_some_and(|level| result.verification_level != *level)
    {
        format!("expected level {:?}", case.expected_level)
    } else if !missing.is_empty() {
        format!("missing expected skills: {}", missing.join(","))
    } else {
        format!("forbidden skills present: {}", forbidden_present.join(","))
    };
    BenchmarkFailure {
        case: case.name.clone(),
        prompt: case.prompt.clone(),
        reason,
        expected_skills: case.expected_skills.clone(),
        forbidden_skills: case.forbidden_skills.clone(),
        actual_decision: result.decision.clone(),
        actual_skills: result.skills.clone(),
    }
}
fn coverage(
    registry: &Registry,
    cases: &[BenchmarkCase],
    failures: &[BenchmarkFailure],
) -> BenchmarkCoverage {
    let registered: BTreeSet<&str> = registry
        .skills
        .iter()
        .map(|skill| skill.id.as_str())
        .collect();
    let failed_cases: BTreeSet<&str> = failures
        .iter()
        .map(|failure| failure.case.as_str())
        .collect();
    let mut covered = BTreeSet::new();
    for case in cases {
        if matches!(case.case_kind, CaseKind::Positive)
            && !failed_cases.contains(case.name.as_str())
        {
            for skill in &case.expected_skills {
                covered.insert(skill.as_str());
            }
        }
    }
    let missing_skills = registered
        .difference(&covered)
        .map(|skill| skill.to_string())
        .collect::<Vec<_>>();
    BenchmarkCoverage {
        registered_skill_count: registered.len(),
        covered_skill_count: covered.len(),
        missing_skills,
    }
}
struct TimingStats {
    total_routes: usize,
    elapsed_ns: u128,
    ns_per_route: f64,
    routes_per_second: f64,
    checksum: usize,
}
fn measure_routes(registry: &Registry, cases: &[BenchmarkCase], iterations: usize) -> TimingStats {
    let total_routes = iterations.saturating_mul(cases.len());
    let started = Instant::now();
    let mut checksum = 0usize;
    for _ in 0..iterations {
        for case in cases {
            let result = route_prompt(black_box(registry), black_box(&case.prompt));
            checksum = checksum.wrapping_add(result.skills.len());
        }
    }
    let elapsed = started.elapsed();
    let elapsed_ns = elapsed.as_nanos();
    let ns_per_route = elapsed_ns as f64 / total_routes.max(1) as f64;
    let routes_per_second = total_routes as f64 / elapsed.as_secs_f64().max(f64::EPSILON);
    TimingStats {
        total_routes,
        elapsed_ns,
        ns_per_route,
        routes_per_second,
        checksum,
    }
}
fn performance_budget(
    max_ns_per_route: Option<u128>,
    ns_per_route: f64,
) -> Option<PerformanceBudget> {
    max_ns_per_route.map(|max| PerformanceBudget {
        max_ns_per_route: max,
        status: if ns_per_route <= max as f64 {
            "PASS"
        } else {
            "FAIL"
        }
        .to_string(),
    })
}
fn baseline_comparison(case_count: usize) -> BenchmarkBaselineComparison {
    let baseline_case_count = 63;
    let added_case_count = case_count.saturating_sub(baseline_case_count);
    BenchmarkBaselineComparison {
        baseline: "legacy benchmark baseline: 51 skills, 63 cases".to_string(),
        baseline_skill_count: 51,
        baseline_case_count,
        added_case_count,
        required_added_cases: 12,
        status: if added_case_count >= 12 {
            "PASS"
        } else {
            "FAIL"
        }
        .to_string(),
    }
}
fn overall_status(
    correctness_status: &str,
    goal_status: &str,
    goal_complete_status: &str,
    playbook_status: &str,
    performance_budget: &Option<PerformanceBudget>,
    baseline_comparison: &BenchmarkBaselineComparison,
) -> String {
    if correctness_status != "PASS" {
        return "FAIL".to_string();
    }
    if goal_status != "PASS" {
        return "FAIL".to_string();
    }
    if goal_complete_status != "PASS" {
        return "FAIL".to_string();
    }
    if playbook_status != "PASS" {
        return "FAIL".to_string();
    }
    if baseline_comparison.status != "PASS" {
        return "FAIL".to_string();
    }
    if performance_budget
        .as_ref()
        .is_some_and(|budget| budget.status != "PASS")
    {
        return "FAIL".to_string();
    }
    "PASS".to_string()
}

fn validate_playbook_quality(root: &Path, registry: &Registry) -> PlaybookQualityBenchmark {
    let cases = playbook_quality_cases();
    let mut failures = Vec::new();
    for case in &cases {
        let route = route_prompt(registry, case.prompt);
        match plan_goal(root, registry, case.prompt, &route) {
            Ok(plan) => failures.extend(validate_playbook_quality_case(case, &plan)),
            Err(error) => failures.push(PlaybookQualityFailure {
                case: case.name.to_string(),
                prompt: case.prompt.to_string(),
                reason: error,
            }),
        }
    }
    PlaybookQualityBenchmark {
        status: if failures.is_empty() { "PASS" } else { "FAIL" }.to_string(),
        case_count: cases.len(),
        failures,
    }
}

struct PlaybookQualityCase {
    name: &'static str,
    prompt: &'static str,
    required_playbooks: &'static [&'static str],
    forbidden_playbooks: &'static [&'static str],
    evidence_terms: &'static [&'static str],
    anti_claim_terms: &'static [&'static str],
}

fn playbook_quality_cases() -> Vec<PlaybookQualityCase> {
    vec![
        PlaybookQualityCase {
            name: "playbook-quality:lvgl",
            prompt: "T-Watch Ultra LVGL blank screen touch debug",
            required_playbooks: &["playbook-source-discovery", "playbook-lvgl-debug"],
            forbidden_playbooks: &["playbook-ota-debug"],
            evidence_terms: &["source facts", "LVGL", "simulator"],
            anti_claim_terms: &["cannot prove", "pixels"],
        },
        PlaybookQualityCase {
            name: "playbook-quality:ota",
            prompt: "T-Watch Ultra ESP-IDF OTA rollback manifest debug",
            required_playbooks: &["playbook-ota-debug", "playbook-build-flash-serial"],
            forbidden_playbooks: &["playbook-lvgl-debug"],
            evidence_terms: &["manifest", "rollback", "serial"],
            anti_claim_terms: &["planning evidence", "credentials"],
        },
        PlaybookQualityCase {
            name: "playbook-quality:cjk-flash",
            prompt: "T-Display-S3烧录失败",
            required_playbooks: &["playbook-source-discovery", "playbook-build-flash-serial"],
            forbidden_playbooks: &["playbook-ota-debug"],
            evidence_terms: &["build command", "serial"],
            anti_claim_terms: &["flash success"],
        },
        PlaybookQualityCase {
            name: "playbook-quality:bsp",
            prompt: "T-Watch Ultra add display driver BSP status action smoke",
            required_playbooks: &["playbook-source-discovery", "playbook-bsp-driver"],
            forbidden_playbooks: &["playbook-ota-debug"],
            evidence_terms: &["status", "action", "smoke"],
            anti_claim_terms: &["missing pin", "peripheral works"],
        },
        PlaybookQualityCase {
            name: "playbook-quality:no-op",
            prompt: "what is the weather today",
            required_playbooks: &[],
            forbidden_playbooks: &[
                "playbook-source-discovery",
                "playbook-lvgl-debug",
                "playbook-ota-debug",
            ],
            evidence_terms: &[],
            anti_claim_terms: &[],
        },
    ]
}

fn validate_playbook_quality_case(
    case: &PlaybookQualityCase,
    plan: &GoalPlan,
) -> Vec<PlaybookQualityFailure> {
    let mut failures = Vec::new();
    let actual = plan
        .context_capsule
        .playbook_hints
        .iter()
        .map(|hint| hint.playbook_id.as_str())
        .collect::<BTreeSet<_>>();
    for id in case.required_playbooks {
        if !actual.contains(id) {
            failures.push(playbook_failure(case, format!("missing playbook {id}")));
        }
    }
    for id in case.forbidden_playbooks {
        if actual.contains(id) {
            failures.push(playbook_failure(case, format!("forbidden playbook {id}")));
        }
    }
    let evidence = plan
        .context_capsule
        .playbook_hints
        .iter()
        .flat_map(|hint| hint.evidence_targets.iter())
        .map(|value| value.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    for term in case.evidence_terms {
        if !evidence.contains(&term.to_lowercase()) {
            failures.push(playbook_failure(
                case,
                format!("missing evidence term {term}"),
            ));
        }
    }
    let anti_claims = plan
        .context_capsule
        .playbook_hints
        .iter()
        .flat_map(|hint| hint.anti_claims.iter())
        .map(|value| value.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    for term in case.anti_claim_terms {
        if !anti_claims.contains(&term.to_lowercase()) {
            failures.push(playbook_failure(
                case,
                format!("missing anti-claim term {term}"),
            ));
        }
    }
    failures
}

fn playbook_failure(case: &PlaybookQualityCase, reason: String) -> PlaybookQualityFailure {
    PlaybookQualityFailure {
        case: case.name.to_string(),
        prompt: case.prompt.to_string(),
        reason,
    }
}

fn validate_goal_capsules(root: &Path, registry: &Registry) -> GoalBenchmark {
    let cases = goal_cases();
    let mut failures = Vec::new();
    for case in &cases {
        let route = route_prompt(registry, case.prompt);
        match plan_goal(root, registry, case.prompt, &route) {
            Ok(plan) => failures.extend(validate_goal_case(case, &plan)),
            Err(error) => failures.push(GoalBenchmarkFailure {
                case: case.name.to_string(),
                prompt: case.prompt.to_string(),
                reason: error,
            }),
        }
    }
    GoalBenchmark {
        status: if failures.is_empty() { "PASS" } else { "FAIL" }.to_string(),
        case_count: cases.len(),
        failures,
    }
}

fn validate_goal_complete(root: &Path, registry: &Registry) -> GoalBenchmark {
    let cases = [
        (
            "complete:t-display-s3-permission",
            "T-Display-S3 Arduino LVGL display demo",
            "needs_permission",
        ),
        (
            "complete:t-beam-source",
            "T-Beam LoRa GNSS debug",
            "needs_source_ingestion",
        ),
        (
            "complete:missing-board",
            "Arduino LVGL display demo",
            "needs_clarification",
        ),
        ("complete:no-op", "how do I prune tomato plants", "no_op"),
    ];
    let case_count = cases.len();
    let mut failures = Vec::new();
    for (name, prompt, expected_status) in cases {
        let route = route_prompt(registry, prompt);
        let options = goal_complete_options();
        match complete_goal(root, registry, prompt, &route, options) {
            Ok(result) if result["status"].as_str() == Some(expected_status) => {}
            Ok(result) => failures.push(GoalBenchmarkFailure {
                case: name.to_string(),
                prompt: prompt.to_string(),
                reason: format!(
                    "expected status {expected_status}, got {}",
                    result["status"]
                ),
            }),
            Err(error) => failures.push(GoalBenchmarkFailure {
                case: name.to_string(),
                prompt: prompt.to_string(),
                reason: error,
            }),
        }
    }
    GoalBenchmark {
        status: if failures.is_empty() { "PASS" } else { "FAIL" }.to_string(),
        case_count,
        failures,
    }
}

fn goal_complete_options() -> GoalCompleteOptions {
    let project = std::env::temp_dir().join("lilygo-skills-benchmark-complete");
    GoalCompleteOptions {
        project_root: project.clone(),
        project_start: project.clone(),
        generated_root: None,
        allow_generate: false,
        start_options: GoalStartOptions {
            project_root: project,
            dry_run: true,
            allow_build: false,
            allow_flash: false,
            allow_serial: false,
            allow_network: false,
            allow_ota: false,
            allow_simulator: false,
            port: None,
            source_root: None,
        },
    }
}

struct GoalCase {
    name: &'static str,
    prompt: &'static str,
    expected_board: Option<&'static str>,
    expected_completeness: &'static [(&'static str, &'static str)],
    required_facts: &'static [&'static str],
    required_fact_tables: &'static [&'static str],
    required_demos: &'static [&'static str],
    required_recipes: &'static [&'static str],
    expect_no_preferences: bool,
    expect_no_reference_hints: bool,
}

fn goal_cases() -> Vec<GoalCase> {
    vec![
        GoalCase {
            name: "goal:t-watch-ultra-raise-to-wake",
            prompt: "T-Watch Ultra Arduino IMU 抬腕检测怎么做",
            expected_board: Some("board-t-watch-ultra"),
            expected_completeness: &[("imu", "complete")],
            required_facts: &["Bosch BHI260AP", "I2C 0x28", "SensorBHI260AP"],
            required_fact_tables: &[],
            required_demos: &["examples/sensor/BHI260AP_6DoF/BHI260AP_6DoF.ino"],
            required_recipes: &[
                "recipe-run-official-demo",
                "recipe-build-upload-monitor",
                "recipe-serial-debug",
            ],
            expect_no_preferences: false,
            expect_no_reference_hints: false,
        },
        GoalCase {
            name: "goal:t-watch-ultra-lvgl-touch",
            prompt: "T-Watch Ultra Arduino LVGL touch does not move",
            expected_board: Some("board-t-watch-ultra"),
            expected_completeness: &[("display", "complete"), ("input", "complete")],
            required_facts: &["CO5300", "CST9217"],
            required_fact_tables: &[],
            required_demos: &["examples/lvgl/get_started/get_started.ino"],
            required_recipes: &["recipe-lvgl-simulator"],
            expect_no_preferences: false,
            expect_no_reference_hints: false,
        },
        GoalCase {
            name: "goal:t-watch-ultra-ota",
            prompt: "T-Watch Ultra OTA manifest downloaded then rebooted",
            expected_board: Some("board-t-watch-ultra"),
            expected_completeness: &[],
            required_facts: &["16MB flash + 8MB PSRAM"],
            required_fact_tables: &[],
            required_demos: &["examples/factory/factory.ino"],
            required_recipes: &["recipe-ota-debug", "recipe-serial-debug"],
            expect_no_preferences: false,
            expect_no_reference_hints: false,
        },
        GoalCase {
            name: "goal:t-watch-ultra-nfc-demo",
            prompt: "T-Watch Ultra run official NFC demo",
            expected_board: Some("board-t-watch-ultra"),
            expected_completeness: &[],
            required_facts: &["ST25R3916"],
            required_fact_tables: &[],
            required_demos: &["examples/peripheral/NFC_Reader/NFC_Reader.ino"],
            required_recipes: &["recipe-run-official-demo", "recipe-build-upload-monitor"],
            expect_no_preferences: false,
            expect_no_reference_hints: false,
        },
        GoalCase {
            name: "goal:t-watch-ultra-io-facts",
            prompt: "T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?",
            expected_board: Some("board-t-watch-ultra"),
            expected_completeness: &[],
            required_facts: &[],
            required_fact_tables: &[
                "pin_matrix",
                "bus_matrix",
                "expander_matrix",
                "peripheral_table",
            ],
            required_demos: &[],
            required_recipes: &[],
            expect_no_preferences: true,
            expect_no_reference_hints: true,
        },
        GoalCase {
            name: "goal:t-display-s3-completeness",
            prompt: "T-Display-S3 Arduino LVGL display demo",
            expected_board: Some("board-t-display-s3"),
            expected_completeness: &[("display", "complete")],
            required_facts: &[
                "ST7789 170x320 TFT",
                "8-bit parallel display bus",
                "GPIO38 backlight; GPIO15 screen power",
            ],
            required_fact_tables: &[],
            required_demos: &["examples/factory/factory.ino", "examples/tft/tft.ino"],
            required_recipes: &[
                "recipe-run-official-demo",
                "recipe-build-upload-monitor",
                "recipe-lvgl-simulator",
            ],
            expect_no_preferences: false,
            expect_no_reference_hints: false,
        },
    ]
}

fn validate_goal_case(case: &GoalCase, plan: &GoalPlan) -> Vec<GoalBenchmarkFailure> {
    let facts = plan
        .context_capsule
        .facts
        .iter()
        .map(|fact| fact.value.as_str())
        .collect::<BTreeSet<_>>();
    let demos = plan
        .context_capsule
        .demo_refs
        .iter()
        .map(|demo| demo.path.as_str())
        .collect::<BTreeSet<_>>();
    let recipes = plan
        .recipe_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let fact_tables = plan
        .context_capsule
        .fact_tables
        .iter()
        .map(|table| table.table.as_str())
        .collect::<BTreeSet<_>>();
    let mut failures = Vec::new();
    if case.expected_board != plan.route.board.as_deref() {
        failures.push(goal_failure(
            case,
            format!(
                "expected board {:?}, got {:?}",
                case.expected_board, plan.route.board
            ),
        ));
    }
    for (topic, expected) in case.expected_completeness {
        let actual = plan
            .context_capsule
            .completeness
            .get(*topic)
            .map(String::as_str);
        if actual != Some(*expected) {
            failures.push(goal_failure(
                case,
                format!("expected completeness {topic}={expected}, got {actual:?}"),
            ));
        }
    }
    for fact in case.required_facts {
        if !facts.contains(fact) {
            failures.push(goal_failure(case, format!("missing fact {fact}")));
        }
    }
    for demo in case.required_demos {
        if !demos.contains(demo) {
            failures.push(goal_failure(case, format!("missing demo {demo}")));
        }
    }
    for recipe in case.required_recipes {
        if !recipes.contains(recipe) {
            failures.push(goal_failure(case, format!("missing recipe {recipe}")));
        }
    }
    for table in case.required_fact_tables {
        if !fact_tables.contains(table) {
            failures.push(goal_failure(case, format!("missing fact table {table}")));
        }
    }
    if case.expect_no_preferences && !plan.context_capsule.preferences.is_empty() {
        failures.push(goal_failure(
            case,
            "unexpected preferences for fact lookup".to_string(),
        ));
    }
    if case.expect_no_reference_hints && !plan.context_capsule.reference_hints.is_empty() {
        failures.push(goal_failure(
            case,
            "unexpected reference hints for fact lookup".to_string(),
        ));
    }
    failures
}

fn goal_failure(case: &GoalCase, reason: String) -> GoalBenchmarkFailure {
    GoalBenchmarkFailure {
        case: case.name.to_string(),
        prompt: case.prompt.to_string(),
        reason,
    }
}
fn count_kind(cases: &[BenchmarkCase], kind: CaseKind) -> usize {
    cases
        .iter()
        .filter(|case| std::mem::discriminant(&case.case_kind) == std::mem::discriminant(&kind))
        .count()
}
fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
fn warnings() -> Vec<String> {
    if cfg!(debug_assertions) {
        vec![
            "benchmark ran in debug build; use cargo run --release for performance numbers"
                .to_string(),
        ]
    } else {
        Vec::new()
    }
}
