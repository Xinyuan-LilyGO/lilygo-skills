#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--dry-run" ]]; then
  shift
fi
if [[ "$#" -ne 0 ]]; then
  echo "usage: goal-complete-smoke.sh [--dry-run]" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp
GENERATED_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-complete-generated.XXXXXX")"
trap 'rm -rf "$GENERATED_ROOT"' EXIT

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" goal complete --dry-run --json "T-Display-S3 Arduino LVGL display demo" \
  >.tmp/goal-complete-display.json
"$BIN" goal complete --dry-run --json "Arduino LVGL display demo" \
  >.tmp/goal-complete-clarification.json
"$BIN" goal complete --dry-run --json "T-Beam LoRa GNSS debug" \
  >.tmp/goal-complete-source.json
"$BIN" goal complete --dry-run --generated-root "$GENERATED_ROOT" --json "T-Display-S3 Arduino LVGL display demo" \
  >.tmp/goal-complete-generated-missing.json
"$BIN" goal complete --dry-run --json "how do I prune tomato plants" \
  >.tmp/goal-complete-noop.json

node <<'NODE'
const fs = require("fs");
const files = {
  display: ".tmp/goal-complete-display.json",
  clarification: ".tmp/goal-complete-clarification.json",
  source: ".tmp/goal-complete-source.json",
  generated: ".tmp/goal-complete-generated-missing.json",
  noop: ".tmp/goal-complete-noop.json"
};
const read = (name) => JSON.parse(fs.readFileSync(files[name], "utf8"));
const display = read("display");
const clarification = read("clarification");
const source = read("source");
const generated = read("generated");
const noop = read("noop");
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(JSON.stringify(detail, null, 2));
    process.exit(1);
  }
}
check("display needs permission",
  display.status === "needs_permission" &&
  display.route.board === "board-t-display-s3" &&
  display.route.framework === "fw-arduino" &&
  display.route.skills.includes("fw-lvgl") &&
  display.readiness.source.status === "complete" &&
  display.readiness.generated_skills.status === "not_checked" &&
  display.execution.attempted === false,
  display);
check("clarification asks board",
  clarification.status === "needs_clarification" &&
  clarification.readiness.project.missing.includes("board") &&
  clarification.next_actions.some((action) => action.kind === "ask_user"),
  clarification);
check("source ingestion blocks before permissions",
  source.status === "needs_source_ingestion" &&
  source.readiness.source.status === "needs_source_ingestion" &&
  source.next_actions.some((action) => action.command.includes("update board-facts")),
  source);
check("generated root missing is distinct",
  generated.status === "needs_generation" &&
  generated.readiness.generated_skills.status === "missing" &&
  generated.next_actions.some((action) => action.command.includes("generate skills")),
  generated);
check("unrelated prompt no-op",
  noop.status === "no_op" &&
  noop.route.skills.length === 0 &&
  noop.execution.attempted === false,
  noop);
for (const [name, file] of Object.entries(files)) {
  const text = fs.readFileSync(file, "utf8");
  check(`${name} public output redacted`,
    !/\/Users\/[A-Za-z0-9._-]+\//.test(text) &&
    !/\/dev\/(?:cu|tty)\./.test(text) &&
    !/192\.168\./.test(text),
    text);
}
process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: Object.keys(files),
  display_status: display.status,
  clarification_status: clarification.status,
  source_status: source.status,
  generated_status: generated.status,
  noop_status: noop.status
}, null, 2) + "\n");
NODE
