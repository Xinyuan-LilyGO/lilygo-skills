#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "cjk-prompt-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" route --json "T-Display-S3烧录失败" >.tmp/cjk-route-display-flash.json
"$BIN" route --json "t-watch ultra imu抬腕检测怎么做" >.tmp/cjk-route-watch-imu.json
printf '{"prompt":"T-Display-S3烧录失败怎么办"}\n' \
  | "$BIN" hook claude >.tmp/cjk-hook-display-flash.json
"$BIN" goal plan --json "t-watch ultra imu抬腕检测怎么做" >.tmp/cjk-goal-watch-imu.json
"$BIN" benchmark --json --iterations 100 >.tmp/cjk-benchmark.json

node <<'NODE'
const fs = require("fs");
function read(path) {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(JSON.stringify(detail, null, 2));
    process.exit(1);
  }
}
const display = read(".tmp/cjk-route-display-flash.json");
const watch = read(".tmp/cjk-route-watch-imu.json");
const hook = read(".tmp/cjk-hook-display-flash.json");
const goal = read(".tmp/cjk-goal-watch-imu.json");
const benchmark = read(".tmp/cjk-benchmark.json");

check("display route injects flash playbook",
  display.decision === "inject" &&
  display.skills.includes("board-t-display-s3") &&
  display.skills.includes("playbook-build-flash-serial") &&
  !display.skills.includes("playbook-ota-debug"),
  display);
check("watch IMU route injects raise-to-wake context",
  watch.decision === "inject" &&
  watch.skills.includes("board-t-watch-ultra") &&
  watch.skills.includes("periph-imu") &&
  watch.skills.includes("feature-raise-to-wake"),
  watch);
check("hook returns valid claude envelope",
  hook.hookSpecificOutput &&
  hook.hookSpecificOutput.hookEventName === "UserPromptSubmit" &&
  typeof hook.hookSpecificOutput.additionalContext === "string" &&
  hook.hookSpecificOutput.additionalContext.includes("board-t-display-s3") &&
  hook.hookSpecificOutput.additionalContext.includes("playbook-build-flash-serial"),
  hook);
check("goal keeps source-backed IMU facts",
  goal.status === "PASS" &&
  goal.route.board === "board-t-watch-ultra" &&
  goal.context_capsule.completeness.imu === "complete" &&
  goal.context_capsule.facts.some((fact) => fact.value === "Bosch BHI260AP"),
  goal.context_capsule);
check("benchmark includes and passes CJK cases",
  benchmark.status === "PASS" &&
  benchmark.coverage.missing_skills.length === 0 &&
  benchmark.playbook_quality.case_count >= 5 &&
  benchmark.correctness.failures.length === 0 &&
  benchmark.playbook_quality.failures.length === 0,
  benchmark);

process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: [
    "route CJK display flash",
    "route CJK watch IMU",
    "hook CJK JSON envelope",
    "goal CJK IMU facts",
    "benchmark CJK fixtures"
  ],
  route_skills: display.skills,
  goal_board: goal.route.board,
  benchmark_case_count: benchmark.case_count,
  playbook_quality_cases: benchmark.playbook_quality.case_count
}, null, 2) + "\n");
NODE
