#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "goal-safety-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

PROJECT_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-safety.XXXXXX")"
OTA_RUNNER_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-ota-runner.XXXXXX")"
VALID_SOURCE="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-valid-source.XXXXXX")"
INVALID_SOURCE="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-invalid-source.XXXXXX")"
trap 'rm -rf "$PROJECT_ROOT" "$OTA_RUNNER_ROOT" "$VALID_SOURCE" "$INVALID_SOURCE"' EXIT

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" goal plan --json "T-Watch Ultra Arduino IMU 抬腕检测怎么做" \
  >.tmp/goal-safety-plan.json
"$BIN" goal plan --json "T-Watch Ultra Rust build firmware" \
  >.tmp/goal-safety-rust-plan.json
"$BIN" goal plan --json "T-Watch Ultra OTA manifest downloaded then rebooted" \
  >.tmp/goal-safety-ota-plan.json
"$BIN" goal start --plan .tmp/goal-safety-plan.json --dry-run --json \
  >.tmp/goal-safety-dry-run.json
"$BIN" goal start --plan .tmp/goal-safety-plan.json --project "$PROJECT_ROOT" --json \
  >.tmp/goal-safety-default-dry-run.json
"$BIN" goal start --plan .tmp/goal-safety-ota-plan.json --dry-run --json \
  >.tmp/goal-safety-ota-dry-run.json

node <<'NODE'
const fs = require("fs");
const plan = JSON.parse(fs.readFileSync(".tmp/goal-safety-plan.json", "utf8"));
if (plan.recipes?.[0]?.steps?.[0]) {
  plan.recipes[0].steps[0].id = "check-toolchain";
  plan.recipes[0].steps[0].permission = "read-only";
  plan.recipes[0].steps[0].command = "rm -rf /tmp/lilygo-should-not-run";
}
fs.writeFileSync(".tmp/goal-safety-malicious-plan.json", JSON.stringify(plan, null, 2));
const ota = JSON.parse(fs.readFileSync(".tmp/goal-safety-ota-plan.json", "utf8"));
ota.recipe_ids = ["recipe-ota-debug"];
ota.recipes = (ota.recipes || []).filter((recipe) => recipe.id === "recipe-ota-debug");
fs.writeFileSync(".tmp/goal-safety-ota-runner-plan.json", JSON.stringify(ota, null, 2));
NODE
"$BIN" goal start --plan .tmp/goal-safety-malicious-plan.json --dry-run --json \
  >.tmp/goal-safety-malicious-dry-run.json

mkdir -p "$OTA_RUNNER_ROOT/.lilygo-skills"
cat >"$OTA_RUNNER_ROOT/.lilygo-skills/local.json" <<'JSON'
{
  "ota_manifest_argv": ["sh", "-c", "printf 'manifest ExamplePrivateKey SyntheticLocalTarget'"],
  "ota_observe_argv": ["sh", "-c", "printf 'observe ExamplePrivateKey SyntheticLocalTarget'"]
}
JSON
"$BIN" goal start \
  --plan .tmp/goal-safety-ota-runner-plan.json \
  --project "$OTA_RUNNER_ROOT" \
  --allow-network \
  --allow-ota \
  --allow-serial \
  --port /tmp/lilygo-invalid-serial \
  --json \
  >.tmp/goal-safety-ota-runner.json

mkdir -p "$VALID_SOURCE/src" "$INVALID_SOURCE/src"
printf '[package]\nname = "lilygo_goal_valid"\nversion = "0.1.0"\nedition = "2021"\n\n' \
  >"$VALID_SOURCE/Cargo.toml"
printf 'fn main() {}\n' >"$VALID_SOURCE/src/main.rs"
printf '[package]\nname = "lilygo_goal_invalid"\nversion = "0.1.0"\nedition = "2021"\n\n' \
  >"$INVALID_SOURCE/Cargo.toml"
printf 'fn main() { let value: u32 = "bad"; let _ = value; }\n' \
  >"$INVALID_SOURCE/src/main.rs"

"$BIN" goal start \
  --plan .tmp/goal-safety-rust-plan.json \
  --project "$PROJECT_ROOT" \
  --allow-build \
  --source-root "$VALID_SOURCE" \
  --json \
  >.tmp/goal-safety-build-only.json

set +e
"$BIN" goal start \
  --plan .tmp/goal-safety-rust-plan.json \
  --project "$PROJECT_ROOT" \
  --allow-build \
  --allow-flash \
  --source-root "$VALID_SOURCE" \
  --port /tmp/lilygo-invalid-serial \
  --json \
  >.tmp/goal-safety-build-then-blocked.json \
  2>.tmp/goal-safety-build-then-blocked.stderr
BUILD_THEN_BLOCKED_EXIT=$?
"$BIN" goal start \
  --plan .tmp/goal-safety-rust-plan.json \
  --project "$PROJECT_ROOT" \
  --allow-build \
  --source-root "$INVALID_SOURCE" \
  --json \
  >.tmp/goal-safety-failed-build.json \
  2>.tmp/goal-safety-failed-build.stderr
FAILED_BUILD_EXIT=$?
"$BIN" goal start \
  --plan .tmp/goal-safety-rust-plan.json \
  --project "$PROJECT_ROOT" \
  --allow-build \
  --source-root "$INVALID_SOURCE" \
  --json \
  >.tmp/goal-safety-repeated-failed-build.json \
  2>.tmp/goal-safety-repeated-failed-build.stderr
REPEATED_FAILED_BUILD_EXIT=$?
"$BIN" goal status --id '../../outside' --project "$PROJECT_ROOT" --json \
  >.tmp/goal-safety-traversal.json \
  2>.tmp/goal-safety-traversal.stderr
TRAVERSAL_EXIT=$?
"$BIN" goal start \
  --plan .tmp/goal-safety-ota-plan.json \
  --project "$PROJECT_ROOT" \
  --allow-build \
  --allow-network \
  --allow-ota \
  --allow-serial \
  --port /tmp/lilygo-invalid-serial \
  --json \
  >.tmp/goal-safety-ota-blocked.json \
  2>.tmp/goal-safety-ota-blocked.stderr
OTA_BLOCKED_EXIT=$?
set -e

BUILD_THEN_BLOCKED_EXIT="$BUILD_THEN_BLOCKED_EXIT" FAILED_BUILD_EXIT="$FAILED_BUILD_EXIT" REPEATED_FAILED_BUILD_EXIT="$REPEATED_FAILED_BUILD_EXIT" TRAVERSAL_EXIT="$TRAVERSAL_EXIT" OTA_BLOCKED_EXIT="$OTA_BLOCKED_EXIT" node <<'NODE'
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
const dry = read(".tmp/goal-safety-dry-run.json");
const defaultDry = read(".tmp/goal-safety-default-dry-run.json");
const otaDry = read(".tmp/goal-safety-ota-dry-run.json");
const otaRunner = read(".tmp/goal-safety-ota-runner.json");
const malicious = read(".tmp/goal-safety-malicious-dry-run.json");
const buildOnly = read(".tmp/goal-safety-build-only.json");
const buildThenBlocked = read(".tmp/goal-safety-build-then-blocked.json");
const failedBuild = read(".tmp/goal-safety-failed-build.json");
const repeatedFailedBuild = read(".tmp/goal-safety-repeated-failed-build.json");
const otaBlocked = read(".tmp/goal-safety-ota-blocked.json");
const buildThenBlockedExit = Number(process.env.BUILD_THEN_BLOCKED_EXIT);
const failedBuildExit = Number(process.env.FAILED_BUILD_EXIT);
const repeatedFailedBuildExit = Number(process.env.REPEATED_FAILED_BUILD_EXIT);
const traversalExit = Number(process.env.TRAVERSAL_EXIT);
const otaBlockedExit = Number(process.env.OTA_BLOCKED_EXIT);
check("dry-run pass", dry.status === "PASS" && dry.dry_run === true, dry);
check("dry-run no writes", Array.isArray(dry.writes) && dry.writes.length === 0, dry);
check("dry-run no commands", Array.isArray(dry.ran_commands) && dry.ran_commands.length === 0, dry);
check("dry-run includes required permissions", Array.isArray(dry.required_permissions) && dry.required_permissions.includes("allow-build") && dry.required_permissions.includes("allow-flash:port"), dry);
check("dry-run includes planned artifacts", Array.isArray(dry.planned_artifacts) && dry.planned_artifacts.length > 0, dry);
check("dry-run plans build", dry.planned_commands.some((command) => command.permission === "allow-build"), dry.planned_commands);
check("dry-run plans flash gate", dry.planned_commands.some((command) => command.permission === "allow-flash:port"), dry.planned_commands);
check("dry-run plans serial gate", dry.planned_commands.some((command) => command.permission === "allow-serial:port"), dry.planned_commands);
check("default start is dry-run", defaultDry.status === "PASS" && defaultDry.dry_run === true, defaultDry);
check("default start has no writes", Array.isArray(defaultDry.writes) && defaultDry.writes.length === 0, defaultDry);
check("default start runs no commands", Array.isArray(defaultDry.ran_commands) && defaultDry.ran_commands.length === 0, defaultDry);
check("default start includes planned artifacts", Array.isArray(defaultDry.planned_artifacts) && defaultDry.planned_artifacts.length > 0, defaultDry);
check("ota dry-run has no generic ota binary", otaDry.planned_commands.every((command) => !command.command.includes("lilygo-ota")), otaDry.planned_commands);
check("ota manifest resolves project runner", otaDry.planned_commands.some((command) => command.step_id === "manifest-check" && (command.argv || []).length === 0 && command.command.includes("project OTA manifest runner")), otaDry.planned_commands);
check("ota observe resolves project runner", otaDry.planned_commands.some((command) => command.step_id === "ota-observe" && (command.argv || []).length === 0 && command.command.includes("project OTA transport")), otaDry.planned_commands);
check("local ota runner pass", otaRunner.status === "PASS" && otaRunner.highest_verification_level === "V5", otaRunner);
check("local ota runner output omitted", JSON.stringify(otaRunner).includes("private local OTA command output omitted"), otaRunner);
check("local ota runner hides private values", !JSON.stringify(otaRunner).includes("ExamplePrivateKey") && !JSON.stringify(otaRunner).includes("SyntheticLocalTarget"), otaRunner);
check("malicious plan command ignored", malicious.planned_commands.every((command) => !command.command.includes("rm -rf")), malicious.planned_commands);
check("build-only pass", buildOnly.status === "PASS" && buildOnly.highest_verification_level === "V4", buildOnly);
check("build-only evidence path is relative", typeof buildOnly.evidence_path === "string" && buildOnly.evidence_path.startsWith(".lilygo-skills/evidence/"), buildOnly);
check("build-only did not edit gitignore", !buildOnly.writes.includes(".gitignore"), buildOnly);
check("build-only does not flash", buildOnly.ran_commands.every((command) => command.step_id !== "upload" && command.step_id !== "monitor"), buildOnly.ran_commands);
check("blocked after build exits nonzero", buildThenBlockedExit !== 0, buildThenBlockedExit);
check("blocked after build keeps V4", buildThenBlocked.status === "BLOCKED" && buildThenBlocked.highest_verification_level === "V4", buildThenBlocked);
check("blocked after build has build pass", buildThenBlocked.ran_commands.some((command) => command.step_id === "build" && command.status === "PASS"), buildThenBlocked.ran_commands);
check("blocked after build does not claim hardware", buildThenBlocked.hardware_verified === false, buildThenBlocked);
check("failed build exits nonzero", failedBuildExit !== 0, failedBuildExit);
check("failed build blocked", failedBuild.status === "BLOCKED" && failedBuild.failure_class === "build-failure", failedBuild);
check("failed build has retry state", failedBuild.repeated_failure_count === 1 && failedBuild.retry_limit === 1, failedBuild);
check("failed build stays V3", failedBuild.highest_verification_level === "V3" && failedBuild.hardware_verified === false, failedBuild);
check("failed build recorded FAIL", failedBuild.ran_commands.some((command) => command.step_id === "build" && command.status === "FAIL"), failedBuild.ran_commands);
check("repeated failed build exits nonzero", repeatedFailedBuildExit !== 0, repeatedFailedBuildExit);
check("repeated failed build routes problem-solving", repeatedFailedBuild.status === "BLOCKED" && repeatedFailedBuild.repeated_failure_count === 2 && /problem-solving/.test(repeatedFailedBuild.next_action || ""), repeatedFailedBuild);
check("goal id traversal rejected", traversalExit !== 0, traversalExit);
check("ota without runner exits nonzero", otaBlockedExit !== 0, otaBlockedExit);
check("ota without runner blocked", otaBlocked.status === "BLOCKED" && otaBlocked.highest_verification_level === "V3", otaBlocked);
process.stdout.write(JSON.stringify({
  status: "PASS",
  dry_run: true,
  dry_run_writes: dry.writes,
  dry_run_ran_commands: dry.ran_commands,
  default_start_dry_run: true,
  default_start_writes: defaultDry.writes,
  ota_generic_binary_absent: true,
  local_ota_runner_status: otaRunner.status,
  malicious_plan_ignored: true,
  build_only_status: buildOnly.status,
  build_then_blocked_level: buildThenBlocked.highest_verification_level,
  failed_build_status: failedBuild.status,
  failed_build_class: failedBuild.failure_class,
  repeated_failed_build_action: repeatedFailedBuild.next_action,
  traversal_rejected: true,
  ota_without_runner_status: otaBlocked.status,
  highest_verification_level: defaultDry.highest_verification_level,
  hardware_verified: defaultDry.hardware_verified
}, null, 2) + "\n");
NODE
