#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--dry-run" ]]; then
  shift
fi
if [[ "$#" -ne 0 ]]; then
  echo "usage: full-evidence-smoke.sh [--dry-run]" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

bash scripts/system-smoke.sh >.tmp/full-system-smoke.json
bash scripts/ota-smoke.sh --dry-run >.tmp/full-ota-smoke.json
bash scripts/lvgl-smoke.sh --dry-run >.tmp/full-lvgl-smoke.json
bash scripts/runtime-degradation-smoke.sh >.tmp/full-runtime-degradation-smoke.json
bash scripts/peripheral-source-smoke.sh --dry-run >.tmp/full-peripheral-source-smoke.json
bash scripts/goal-context-smoke.sh --dry-run >.tmp/full-goal-context-smoke.json
bash scripts/goal-safety-smoke.sh --dry-run >.tmp/full-goal-safety-smoke.json
bash scripts/goal-privacy-smoke.sh --dry-run >.tmp/full-goal-privacy-smoke.json
bash scripts/goal-complete-smoke.sh --dry-run >.tmp/full-goal-complete-smoke.json
bash scripts/goal-complete-permission-smoke.sh --dry-run >.tmp/full-goal-complete-permission-smoke.json
bash scripts/source-fact-smoke.sh --dry-run >.tmp/full-source-fact-smoke.json
bash scripts/source-completeness-smoke.sh --dry-run >.tmp/full-source-completeness-smoke.json
bash scripts/board-completeness-smoke.sh --dry-run >.tmp/full-board-completeness-smoke.json
bash scripts/preference-reference-smoke.sh --dry-run >.tmp/full-preference-reference-smoke.json
bash scripts/setup-plan-smoke.sh --dry-run >.tmp/full-setup-plan-smoke.json
bash scripts/generated-cache-boundary-smoke.sh >.tmp/full-generated-cache-boundary-smoke.json
cargo run -p lilygo-skills-cli -- sync-boards --dry-run --json >.tmp/full-sync.json

node <<'NODE'
const fs = require("fs");
const system = JSON.parse(fs.readFileSync(".tmp/full-system-smoke.json", "utf8"));
const ota = JSON.parse(fs.readFileSync(".tmp/full-ota-smoke.json", "utf8"));
const lvgl = JSON.parse(fs.readFileSync(".tmp/full-lvgl-smoke.json", "utf8"));
const runtimeDegradation = JSON.parse(
  fs.readFileSync(".tmp/full-runtime-degradation-smoke.json", "utf8")
);
const peripheralSource = JSON.parse(
  fs.readFileSync(".tmp/full-peripheral-source-smoke.json", "utf8")
);
const goalContext = JSON.parse(fs.readFileSync(".tmp/full-goal-context-smoke.json", "utf8"));
const goalSafety = JSON.parse(fs.readFileSync(".tmp/full-goal-safety-smoke.json", "utf8"));
const goalPrivacy = JSON.parse(fs.readFileSync(".tmp/full-goal-privacy-smoke.json", "utf8"));
const goalComplete = JSON.parse(fs.readFileSync(".tmp/full-goal-complete-smoke.json", "utf8"));
const goalCompletePermission = JSON.parse(
  fs.readFileSync(".tmp/full-goal-complete-permission-smoke.json", "utf8")
);
const sourceFact = JSON.parse(fs.readFileSync(".tmp/full-source-fact-smoke.json", "utf8"));
const sourceCompleteness = JSON.parse(fs.readFileSync(".tmp/full-source-completeness-smoke.json", "utf8"));
const boardCompleteness = JSON.parse(fs.readFileSync(".tmp/full-board-completeness-smoke.json", "utf8"));
const preferenceReference = JSON.parse(fs.readFileSync(".tmp/full-preference-reference-smoke.json", "utf8"));
const setupPlan = JSON.parse(fs.readFileSync(".tmp/full-setup-plan-smoke.json", "utf8"));
const generatedCache = JSON.parse(
  fs.readFileSync(".tmp/full-generated-cache-boundary-smoke.json", "utf8")
);
const sync = JSON.parse(fs.readFileSync(".tmp/full-sync.json", "utf8"));
const ok =
  system.status === "PASS" &&
  ota.status === "BOUNDARY" &&
  lvgl.status === "BOUNDARY" &&
  runtimeDegradation.status === "PASS" &&
  peripheralSource.status === "PASS" &&
  goalContext.status === "PASS" &&
  goalSafety.status === "PASS" &&
  goalPrivacy.status === "PASS" &&
  goalComplete.status === "PASS" &&
  goalCompletePermission.status === "PASS" &&
  sourceFact.status === "PASS" &&
  sourceCompleteness.status === "PASS" &&
  boardCompleteness.status === "PASS" &&
  preferenceReference.status === "PASS" &&
  setupPlan.status === "PASS" &&
  generatedCache.status === "PASS" &&
  sync.status === "PASS";
process.stdout.write(JSON.stringify({
  status: ok ? "PASS" : "FAIL",
  dry_run: true,
  route_and_install: system.status,
  ota: ota.status,
  lvgl: lvgl.status,
  runtime_degradation: runtimeDegradation.status,
  peripheral_source: peripheralSource.status,
  goal_context: goalContext.status,
  goal_safety: goalSafety.status,
  goal_privacy: goalPrivacy.status,
  goal_complete: goalComplete.status,
  goal_complete_permission: goalCompletePermission.status,
  source_fact: sourceFact.status,
  source_completeness: sourceCompleteness.status,
  board_completeness: boardCompleteness.status,
  preference_reference: preferenceReference.status,
  setup_plan: setupPlan.status,
  generated_cache_boundary: generatedCache.status,
  runtime_degradation_checks: runtimeDegradation.checks.length,
  runtime_degradation_commands: runtimeDegradation.command_count,
  runtime_install_verified_writes: runtimeDegradation.install_verified_writes,
  source_candidate_count: sync.generated_candidate_count,
  source_pack_count: peripheralSource.source_pack_count,
  source_fact_io_count: sourceFact.io_fact_count,
  source_completeness_status: sourceCompleteness.completeness,
  board_completeness_cases: boardCompleteness.checked.length,
  goal_case_count: goalContext.goal_cases,
  goal_blocked_permissions: goalSafety.blocked_permissions,
  highest_verification_level: "V3",
  hardware_verified: false,
  boundaries: [...ota.boundaries, ...lvgl.boundaries],
  writes: []
}, null, 2) + "\n");
process.exit(ok ? 0 : 2);
NODE
