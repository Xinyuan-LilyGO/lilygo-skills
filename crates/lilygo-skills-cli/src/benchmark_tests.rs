use crate::benchmark::run_benchmark;
use crate::model::Registry;
use crate::registry::load_registry;
use std::path::Path;

fn registry() -> Registry {
    load_registry(root().as_path()).expect("registry loads")
}

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn benchmark_covers_registered_skills() {
    let registry = registry();
    let root = root();
    let report = run_benchmark(root.as_path(), &registry, 1, None);
    assert_eq!(report.status, "PASS", "{report:?}");
    assert_eq!(report.coverage.missing_skills, Vec::<String>::new());
    assert_eq!(report.correctness.failures.len(), 0);
    assert_eq!(report.goal_capsules.status, "PASS");
    assert_eq!(report.playbook_quality.status, "PASS");
}

#[test]
fn benchmark_budget_can_fail() {
    let registry = registry();
    let root = root();
    let report = run_benchmark(root.as_path(), &registry, 1, Some(0));
    assert_eq!(report.status, "FAIL");
    assert_eq!(
        report
            .performance_budget
            .as_ref()
            .map(|budget| budget.status.as_str()),
        Some("FAIL")
    );
}

#[test]
fn peripheral_benchmark_coverage() {
    let registry = registry();
    let root = root();
    let report = run_benchmark(root.as_path(), &registry, 1, None);
    assert_eq!(report.status, "PASS", "{report:?}");
    assert_eq!(report.baseline_comparison.status, "PASS");
    assert_eq!(report.baseline_comparison.baseline_case_count, 63);
    assert!(report.baseline_comparison.added_case_count >= 12);
    assert!(report.coverage.covered_skill_count >= 67);
}

#[test]
fn playbook_quality_benchmark() {
    let registry = registry();
    let root = root();
    let report = run_benchmark(root.as_path(), &registry, 1, None);
    assert_eq!(report.playbook_quality.status, "PASS", "{report:?}");
    assert!(report.playbook_quality.case_count >= 4);
    assert!(report.coverage.covered_skill_count >= 71);
}
