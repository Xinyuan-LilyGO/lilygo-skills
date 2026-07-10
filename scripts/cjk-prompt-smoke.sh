#!/usr/bin/env bash
set -euo pipefail

if [[ $# -gt 0 && "${1:-}" != "--dry-run" ]]; then
  echo "unknown argument: $1" >&2
  exit 1
fi
if [[ "${1:-}" == "--dry-run" ]]; then
  shift
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" route --json "T-Display-S3烧录失败" >.tmp/cjk-route-display-flash.json
"$BIN" route --json "t-watch ultra imu抬腕检测怎么做" >.tmp/cjk-route-watch-imu.json
"$BIN" route --json "LilyGO T-Watch S3 屏幕和触摸占用了哪些引脚?" \
  >.tmp/cjk-route-watch-s3-display-touch.json
printf '{"prompt":"T-Display-S3烧录失败怎么办"}\n' \
  | "$BIN" hook claude >.tmp/cjk-hook-display-flash.json
"$BIN" context --plan --json "t-watch ultra imu抬腕检测怎么做" >.tmp/cjk-goal-watch-imu.json
"$BIN" context --plan --json "LilyGO T-Watch S3 屏幕和触摸占用了哪些引脚?" \
  >.tmp/cjk-goal-watch-s3-display-touch.json
"$BIN" source query --board board-t-watch-s3 --topic display --json \
  >.tmp/cjk-source-watch-s3-display.json
"$BIN" source query --board board-t-watch-s3 --topic input --json \
  >.tmp/cjk-source-watch-s3-input.json

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
const watchS3DisplayTouch = read(".tmp/cjk-route-watch-s3-display-touch.json");
const hook = read(".tmp/cjk-hook-display-flash.json");
const goal = read(".tmp/cjk-goal-watch-imu.json");
const watchS3Goal = read(".tmp/cjk-goal-watch-s3-display-touch.json");
const watchS3DisplaySource = read(".tmp/cjk-source-watch-s3-display.json");
const watchS3InputSource = read(".tmp/cjk-source-watch-s3-input.json");

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
check("watch S3 CJK display/touch route injects both peripheral topics",
  watchS3DisplayTouch.decision === "inject" &&
  watchS3DisplayTouch.skills.includes("board-t-watch-s3") &&
  watchS3DisplayTouch.skills.includes("periph-display") &&
  watchS3DisplayTouch.skills.includes("periph-input"),
  watchS3DisplayTouch);
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
const watchS3Actions = (watchS3Goal.context_capsule.next_actions || []).map((action) => action.id);
check("watch S3 CJK display/touch goal exposes narrow source queries",
  watchS3Goal.status === "PASS" &&
  watchS3Goal.route.board === "board-t-watch-s3" &&
  watchS3Goal.context_capsule.completeness.display === "complete" &&
  watchS3Goal.context_capsule.completeness.input === "complete" &&
  watchS3Actions.includes("source-query-display") &&
  watchS3Actions.includes("source-query-input") &&
  (watchS3Goal.context_capsule.next_actions || []).every((action) => action.permission === "none"),
  watchS3Goal.context_capsule);
const displayKeys = (watchS3DisplaySource.facts || []).map((fact) => fact.key);
const inputKeys = (watchS3InputSource.facts || []).map((fact) => fact.key);
check("watch S3 display source query is topic narrow",
  displayKeys.includes("display.panel_or_chip") &&
  displayKeys.includes("display.bus_or_interface") &&
  displayKeys.every((key) => key.startsWith("display.") || key.startsWith("bus.display.") || key === "known-pitfall.arduino-esp32-tft-espi" || key === "framework.demo_refs" || key === "framework.build_hint"),
  displayKeys);
check("watch S3 input source query is topic narrow",
  inputKeys.includes("input.chip") &&
  inputKeys.includes("input.bus_or_interface") &&
  inputKeys.includes("input.touch_interrupt") &&
  inputKeys.every((key) => key.startsWith("input.") || key.startsWith("bus.touch.") || key === "framework.demo_refs"),
  inputKeys);

process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: [
    "route CJK display flash",
    "route CJK watch IMU",
    "route CJK watch S3 display/touch",
    "hook CJK JSON envelope",
    "goal CJK IMU facts",
    "goal CJK watch S3 display/touch source queries",
    "source CJK watch S3 display/input narrowness"
  ],
  route_skills: display.skills,
  goal_board: goal.route.board
}, null, 2) + "\n");
NODE
