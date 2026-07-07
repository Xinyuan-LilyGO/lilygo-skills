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
CACHE_DIR="$ROOT/.tmp/context-budget-cache"
rm -rf "$CACHE_DIR"

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
printf '{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor","session_id":"m23-budget-session"}' \
  | LILYGO_SKILLS_CACHE_DIR="$CACHE_DIR" "$BIN" hook claude >.tmp/context-budget-session-full-hook.json
printf '{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor","session_id":"m23-budget-session"}' \
  | LILYGO_SKILLS_CACHE_DIR="$CACHE_DIR" "$BIN" hook claude >.tmp/context-budget-session-incremental-hook.json
printf '{"prompt":"T-Display-S3 debug an SPI sensor and UART module","session_id":"m23-budget-session"}' \
  | LILYGO_SKILLS_CACHE_DIR="$CACHE_DIR" "$BIN" hook claude >.tmp/context-budget-session-spi-uart-full-hook.json
printf '{"prompt":"T-Display-S3 debug an SPI sensor and UART module","session_id":"m23-budget-session"}' \
  | LILYGO_SKILLS_CACHE_DIR="$CACHE_DIR" "$BIN" hook claude >.tmp/context-budget-session-spi-uart-incremental-hook.json
printf '{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor","session_id":"m23-budget-session"}' \
  | LILYGO_SKILLS_CACHE_DIR="$CACHE_DIR" "$BIN" hook claude >.tmp/context-budget-session-return-hook.json

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
const sessionFullHook = read(".tmp/context-budget-session-full-hook.json").hookSpecificOutput?.additionalContext || "";
const sessionIncrementalHook = read(".tmp/context-budget-session-incremental-hook.json").hookSpecificOutput?.additionalContext || "";
const sessionSpiUartFullHook = read(".tmp/context-budget-session-spi-uart-full-hook.json").hookSpecificOutput?.additionalContext || "";
const sessionSpiUartIncrementalHook = read(".tmp/context-budget-session-spi-uart-incremental-hook.json").hookSpecificOutput?.additionalContext || "";
const sessionReturnHook = read(".tmp/context-budget-session-return-hook.json").hookSpecificOutput?.additionalContext || "";
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
check("first same-session hook is full", sessionFullHook.includes("goal-plan-bridge:none"), sessionFullHook);
check("second same-session hook is incremental", sessionIncrementalHook.includes("LilyGO incremental"), sessionIncrementalHook);
check("incremental keeps critical facts", sessionIncrementalHook.includes("pin.i2c.sda"), sessionIncrementalHook);
check("incremental keeps source expansion", sessionIncrementalHook.includes("source-query-i2c:none"), sessionIncrementalHook);
check("incremental keeps evidence boundary", sessionIncrementalHook.includes("evidence_boundary=V3/hardware_verified=false"), sessionIncrementalHook);
check("incremental hook is at most twenty percent of full hook", bytes(sessionIncrementalHook) * 5 <= bytes(sessionFullHook), {
  full: bytes(sessionFullHook),
  incremental: bytes(sessionIncrementalHook),
  sessionIncrementalHook
});
check("different same-session signature gets a first full hook", sessionSpiUartFullHook.includes("goal-plan-bridge:none"), sessionSpiUartFullHook);
check("multi-bus repeat keeps all source expansions", sessionSpiUartIncrementalHook.includes("source-query-spi:none") && sessionSpiUartIncrementalHook.includes("source-query-uart:none"), sessionSpiUartIncrementalHook);
check("multi-bus repeat does not keep unrelated i2c critical fact", !sessionSpiUartIncrementalHook.includes("pin.i2c.sda"), sessionSpiUartIncrementalHook);
check("returning to an older same-session signature compacts", sessionReturnHook.includes("LilyGO incremental"), sessionReturnHook);
process.stdout.write(JSON.stringify({
  status: "PASS",
  bytes: {
    lookup_capsule: lookupCapsuleBytes,
    implementation_capsule: implCapsuleBytes,
    lookup_hook: bytes(lookupHook),
    implementation_hook: bytes(implHook),
    session_full_hook: bytes(sessionFullHook),
    session_incremental_hook: bytes(sessionIncrementalHook),
    session_spi_uart_full_hook: bytes(sessionSpiUartFullHook),
    session_spi_uart_incremental_hook: bytes(sessionSpiUartIncrementalHook),
    session_return_hook: bytes(sessionReturnHook)
  },
  repeated_actions: repeatActionIds
}, null, 2) + "\n");
NODE
