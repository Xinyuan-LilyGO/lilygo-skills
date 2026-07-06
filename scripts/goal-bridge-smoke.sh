#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "goal-bridge-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor" \
  >.tmp/goal-bridge-impl.json
"$BIN" goal plan --json "T-Display-S3 的 I2C 引脚和外设地址有哪些?" \
  >.tmp/goal-bridge-lookup.json
printf '{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor"}' \
  | "$BIN" hook claude >.tmp/goal-bridge-hook.json

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
const impl = read(".tmp/goal-bridge-impl.json");
const lookup = read(".tmp/goal-bridge-lookup.json");
const hook = read(".tmp/goal-bridge-hook.json");
const implActions = impl.context_capsule.next_actions || [];
const lookupActions = lookup.context_capsule.next_actions || [];
const actionIds = new Set(implActions.map((action) => action.id));
for (const id of ["goal-plan-bridge", "goal-start-dry-run", "source-query-io", "source-query-i2c"]) {
  check(`implementation action ${id}`, actionIds.has(id), implActions);
}
const bridge = implActions.find((action) => action.id === "goal-plan-bridge");
const dryRun = implActions.find((action) => action.id === "goal-start-dry-run");
check("bridge is read-only", bridge.permission === "none", bridge);
check("bridge points to goal plan", /goal plan --json/.test(bridge.command), bridge);
check("dry-run is read-only", dryRun.permission === "none", dryRun);
check("dry-run is directly executable", /goal complete --dry-run --json/.test(dryRun.command), dryRun);
check("dry-run has no saved-plan placeholder", !dryRun.command.includes("<saved-plan.json>"), dryRun);
check("lookup has no goal bridge", !lookupActions.some((action) => action.id === "goal-plan-bridge"), lookupActions);
check("lookup has no dry-run start", !lookupActions.some((action) => action.id === "goal-start-dry-run"), lookupActions);
const context = hook.hookSpecificOutput?.additionalContext || "";
check("hook exposes bridge", context.includes("goal-plan-bridge:none"), context);
check("hook exposes i2c source query", context.includes("source-query-i2c:none"), context);
process.stdout.write(JSON.stringify({
  status: "PASS",
  implementation_actions: [...actionIds],
  lookup_actions: lookupActions.map((action) => action.id),
  hook_context_bytes: Buffer.byteLength(context)
}, null, 2) + "\n");
NODE
