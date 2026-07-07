#!/usr/bin/env bash
set -euo pipefail

if [[ $# -gt 0 && "${1:-}" != "--dry-run" ]]; then
  echo "unknown argument: $1" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" goal plan --json "T-Display-S3 的 I2C 引脚和外设地址有哪些?" \
  >.tmp/pure-query-plan.json
"$BIN" goal plan --json "T-Display-S3 which pins are used by the screen?" \
  >.tmp/pure-query-plan-english-screen.json
"$BIN" goal plan --json "T-Display-S3 read pinout docs" \
  >.tmp/pure-query-plan-read-pinout-docs.json
"$BIN" goal plan --json "T-Display-S3 哪些引脚被屏幕占用了?" \
  >.tmp/pure-query-plan-chinese-screen.json
printf '{"prompt":"T-Display-S3 的 I2C 引脚和外设地址有哪些?"}' \
  | "$BIN" hook claude >.tmp/pure-query-hook.json

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
const plan = read(".tmp/pure-query-plan.json");
const englishScreen = read(".tmp/pure-query-plan-english-screen.json");
const readPinoutDocs = read(".tmp/pure-query-plan-read-pinout-docs.json");
const chineseScreen = read(".tmp/pure-query-plan-chinese-screen.json");
const hook = read(".tmp/pure-query-hook.json");
const lookupPlans = [plan, englishScreen, readPinoutDocs, chineseScreen];
const forbiddenIds = /^(goal-plan-bridge|goal-start-dry-run|goal-build|goal-flash|goal-serial|goal-ota)/;
for (const current of lookupPlans) {
  const capsule = current.context_capsule;
  const actions = capsule.next_actions || [];
  check("lookup has source-query actions", actions.some((action) => action.id.startsWith("source-query-")), actions);
  check("lookup actions are read-only", actions.every((action) => action.permission === "none"), actions);
  check("lookup has no mutating action ids", actions.every((action) => !forbiddenIds.test(action.id)), actions);
  check("lookup has no demos", (capsule.demo_refs || []).length === 0, capsule.demo_refs);
  check("lookup has no recipes", (current.recipe_ids || []).length === 0, current.recipe_ids);
  check("lookup has no implementation start", !capsule.implementation_start, capsule.implementation_start);
  check("lookup has no critical facts block", (capsule.critical_facts || []).length === 0, capsule.critical_facts);
  check("lookup keeps expansion path", (capsule.fact_tables || []).some((table) => /source query/.test(table.query_command)), capsule.fact_tables);
}
const context = hook.hookSpecificOutput?.additionalContext || "";
check("hook does not expose demo path", !context.includes("examples/tft/tft.ino"), context);
check("hook does not expose source recovery block", !context.includes("official-demo-first"), context);
process.stdout.write(JSON.stringify({
  status: "PASS",
  checked_prompts: lookupPlans.length,
  action_ids: lookupPlans.flatMap((current) => (current.context_capsule.next_actions || []).map((action) => action.id)),
  hook_context_bytes: Buffer.byteLength(context)
}, null, 2) + "\n");
NODE
