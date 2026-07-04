#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "project-context-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

PROJECT_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-project-context.XXXXXX")"
mkdir -p "$PROJECT_ROOT/firmware/src"

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" project init \
  --project "$PROJECT_ROOT" \
  --board board-t-watch-ultra \
  --framework fw-arduino \
  --feature feature-raise-to-wake \
  --json >.tmp/project-context-init.json
"$BIN" verify --generated-root "$PROJECT_ROOT/.lilygo-skills/generated-skills" --json \
  >.tmp/project-context-generated-verify.json
"$BIN" project show --project "$PROJECT_ROOT/firmware/src" --json \
  >.tmp/project-context-show.json
"$BIN" route --project "$PROJECT_ROOT" --json "抬腕检测怎么做" \
  >.tmp/project-context-route.json
"$BIN" route --project "$PROJECT_ROOT" --json "T-Display-S3 Arduino LVGL screen is blank" \
  >.tmp/project-context-explicit.json
"$BIN" route --json "Arduino IMU 抬腕检测怎么做" \
  >.tmp/project-context-missing-board.json

BOARD_ONLY="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-board-only.XXXXXX")"
"$BIN" project init --project "$BOARD_ONLY" --board board-t-watch-ultra --json \
  >.tmp/project-context-board-only-init.json
"$BIN" route --project "$BOARD_ONLY" --json "LVGL watch UI demo 怎么写" \
  >.tmp/project-context-missing-framework.json

DISPLAY_PROJECT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-display-project.XXXXXX")"
"$BIN" project init \
  --project "$DISPLAY_PROJECT" \
  --board board-t-display-s3 \
  --framework fw-arduino \
  --json >.tmp/project-context-display-init.json
"$BIN" route --project "$DISPLAY_PROJECT" --json "LVGL 显示 demo 怎么做" \
  >.tmp/project-context-display-route.json
"$BIN" goal plan --project "$DISPLAY_PROJECT" --json "LVGL 显示 demo 怎么做" \
  >.tmp/project-context-display-goal.json

(
  cd "$PROJECT_ROOT/firmware/src"
  "$BIN" route --json "抬腕检测怎么做" >"$ROOT/.tmp/project-context-cwd-route.json"
)

"$BIN" route --project "$PROJECT_ROOT" --json "how do I prune tomatoes" \
  >.tmp/project-context-noop.json
"$BIN" project clear --project "$PROJECT_ROOT" --json \
  >.tmp/project-context-clear.json

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
const init = read(".tmp/project-context-init.json");
const generatedVerify = read(".tmp/project-context-generated-verify.json");
const show = read(".tmp/project-context-show.json");
const route = read(".tmp/project-context-route.json");
const explicit = read(".tmp/project-context-explicit.json");
const missingBoard = read(".tmp/project-context-missing-board.json");
const missingFramework = read(".tmp/project-context-missing-framework.json");
const displayInit = read(".tmp/project-context-display-init.json");
const displayRoute = read(".tmp/project-context-display-route.json");
const displayGoal = read(".tmp/project-context-display-goal.json");
const cwdRoute = read(".tmp/project-context-cwd-route.json");
const noop = read(".tmp/project-context-noop.json");
const clear = read(".tmp/project-context-clear.json");

check("project init writes project config", init.status === "PASS" && init.writes.includes(".lilygo-skills/project.json"), init);
check("local config ignored", init.writes.includes(".gitignore"), init);
check("project init generates cache",
  init.generated_cache &&
  init.generated_cache.verify_status === "PASS" &&
  init.generated_cache.skill_count >= 60 &&
  init.writes.some((write) => write.includes(".lilygo-skills/generated-skills/skills")),
  init);
check("project generated cache verifies",
  generatedVerify.status === "PASS" &&
  generatedVerify.missing.length === 0 &&
  generatedVerify.extra.length === 0,
  generatedVerify);
check("project show walks upward", show.context_source === "project" && show.board === "board-t-watch-ultra", show);
check("project route injects", route.decision === "inject" &&
  ["board-t-watch-ultra", "periph-imu", "chip-bhi260ap", "fw-arduino", "feature-raise-to-wake"].every((skill) => route.skills.includes(skill)), route);
check("explicit prompt overrides project", explicit.skills.includes("board-t-display-s3") && !explicit.skills.includes("board-t-watch-ultra"), explicit);
check("missing board clarifies", missingBoard.decision === "needs_clarification" && missingBoard.missing.includes("board"), missingBoard);
check("missing framework clarifies", missingFramework.decision === "needs_clarification" && missingFramework.missing.includes("framework"), missingFramework);
check("display project init", displayInit.status === "PASS" && displayInit.context.board === "board-t-display-s3", displayInit);
check("display project short route readiness",
  displayRoute.skills.includes("board-t-display-s3") &&
  displayRoute.skills.includes("fw-arduino") &&
  displayRoute.readiness.some((signal) => signal.topic === "display" &&
    signal.completeness === "complete"),
  displayRoute);
check("display project short goal readiness",
  displayGoal.route.board === "board-t-display-s3" &&
  displayGoal.route.framework === "fw-arduino" &&
  displayGoal.context_capsule.completeness.display === "complete" &&
  displayGoal.context_capsule.discovery_hints.some((hint) =>
    hint.command && hint.command.includes("source query")),
  displayGoal);
check("cwd project discovery injects", cwdRoute.skills.includes("board-t-watch-ultra") && cwdRoute.skills.includes("feature-raise-to-wake"), cwdRoute);
check("unrelated prompt remains no-op", noop.decision === "no-op" && noop.skills.length === 0, noop);
check("project clear leaves local boundary untouched", clear.status === "PASS" && clear.writes.includes(".lilygo-skills/project.json"), clear);

process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: [
    "project init",
    "project init generated cache",
    "project show upward discovery",
    "route --project",
    "cwd discovery",
    "explicit override",
    "needs_clarification board",
    "needs_clarification framework",
    "display project readiness",
    "no-op",
    "clear"
  ],
  route_skills: route.skills,
  cwd_route_skills: cwdRoute.skills,
  missing_board: missingBoard.questions,
  missing_framework: missingFramework.questions,
  display_project_completeness: displayGoal.context_capsule.completeness
}, null, 2) + "\n");
NODE
