#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "context-budget-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor" \
  >.tmp/context-budget-impl.json
"$BIN" goal plan --json "T-Display-S3 的 I2C 引脚和外设地址有哪些?" \
  >.tmp/context-budget-lookup.json
"$BIN" goal plan --json "T-Display-S3 debug I2C I2C I2C sensor screen screen screen" \
  >.tmp/context-budget-repeat.json
printf '{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor"}' \
  | "$BIN" hook claude >.tmp/context-budget-impl-hook.json
printf '{"prompt":"T-Display-S3 的 I2C 引脚和外设地址有哪些?"}' \
  | "$BIN" hook claude >.tmp/context-budget-lookup-hook.json

node <<'NODE'
const fs = require("fs");
function read(path) {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}
function bytes(value) {
  return Buffer.byteLength(typeof value === "string" ? value : JSON.stringify(value));
}
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(JSON.stringify(detail, null, 2));
    process.exit(1);
  }
}
const impl = read(".tmp/context-budget-impl.json");
const lookup = read(".tmp/context-budget-lookup.json");
const repeat = read(".tmp/context-budget-repeat.json");
const implHook = read(".tmp/context-budget-impl-hook.json").hookSpecificOutput?.additionalContext || "";
const lookupHook = read(".tmp/context-budget-lookup-hook.json").hookSpecificOutput?.additionalContext || "";
const lookupCapsuleBytes = bytes(lookup.context_capsule);
const implCapsuleBytes = bytes(impl.context_capsule);
const repeatActionIds = (repeat.context_capsule.next_actions || []).map((action) => action.id);
const repeatUnique = new Set(repeatActionIds);
check("lookup hook stays compact", bytes(lookupHook) < 1800, { bytes: bytes(lookupHook), lookupHook });
check("implementation hook stays bounded", bytes(implHook) < 2600, { bytes: bytes(implHook), implHook });
check("lookup hook is smaller than implementation hook", bytes(lookupHook) < bytes(implHook), {
  lookup: bytes(lookupHook),
  implementation: bytes(implHook)
});
check("lookup capsule keeps expansion refs", (lookup.context_capsule.fact_tables || []).some((table) => /source query/.test(table.query_command)), lookup.context_capsule);
check("repeated prompt action ids are deduped", repeatActionIds.length === repeatUnique.size, repeatActionIds);
check("inline source refs honor budget", (impl.context_capsule.source_refs || []).length <= impl.context_capsule.budget.max_source_refs_inline, impl.context_capsule.source_refs);
process.stdout.write(JSON.stringify({
  status: "PASS",
  bytes: {
    lookup_capsule: lookupCapsuleBytes,
    implementation_capsule: implCapsuleBytes,
    lookup_hook: bytes(lookupHook),
    implementation_hook: bytes(implHook)
  },
  repeated_actions: repeatActionIds
}, null, 2) + "\n");
NODE
