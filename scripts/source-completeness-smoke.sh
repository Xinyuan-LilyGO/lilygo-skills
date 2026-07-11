#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "source-completeness-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

before_hash="$(shasum -a 256 data/facts/board-fact-packs.json | awk '{print $1}')"

"$BIN" source completeness --board board-t-display-s3 --topic display --json \
  >.tmp/source-completeness-display.json
"$BIN" source query --board board-t-display-s3 --topic display --json \
  >.tmp/source-completeness-query-display.json
"$BIN" update board-facts --board board-t-display-s3 --topic display --dry-run --json \
  >.tmp/source-completeness-update-dry-run.json
"$BIN" update board-facts --board future-rp2040-product --topic display --dry-run --json \
  >.tmp/source-completeness-unsupported-dry-run.json
set +e
"$BIN" update board-facts --board future-rp2040-product --topic display --json \
  >.tmp/source-completeness-unsupported-apply.json 2>&1
unsupported_apply_exit=$?
set -e
if [[ "$unsupported_apply_exit" -eq 0 ]]; then
  echo "FAIL unsupported board-facts apply unexpectedly succeeded" >&2
  exit 1
fi
"$BIN" route --json "T-Display-S3 Arduino LVGL display demo" \
  >.tmp/source-completeness-route.json
printf '{"prompt":"T-Display-S3 Arduino LVGL display demo"}' | "$BIN" hook codex \
  >.tmp/source-completeness-hook.json

after_hash="$(shasum -a 256 data/facts/board-fact-packs.json | awk '{print $1}')"
export unsupported_apply_exit

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
const completeness = read(".tmp/source-completeness-display.json");
const query = read(".tmp/source-completeness-query-display.json");
const update = read(".tmp/source-completeness-update-dry-run.json");
const unsupportedDryRun = read(".tmp/source-completeness-unsupported-dry-run.json");
const route = read(".tmp/source-completeness-route.json");
const hook = read(".tmp/source-completeness-hook.json");

check("display completeness complete",
  completeness.status === "PASS" &&
  completeness.completeness === "complete" &&
  completeness.required_missing.length === 0 &&
  completeness.required_present.includes("display.panel_or_chip") &&
  completeness.required_present.includes("display.bus_or_interface") &&
  completeness.required_present.includes("display.backlight_or_power") &&
  completeness.facts.some((fact) => fact.value.includes("ST7789")) &&
  completeness.facts.some((fact) => fact.value.includes("GPIO38")),
  completeness);
check("source query carries completeness metadata",
  query.status === "PASS" &&
  query.completeness &&
  query.completeness.completeness === "complete" &&
  query.facts.some((fact) => fact.value.includes("8-bit parallel")),
  query);
check("update board-facts dry-run no writes",
  update.status === "PASS" &&
  update.dry_run === true &&
  update.writes.length === 0 &&
  update.planned_writes.includes("data/facts/board-fact-packs.json") &&
  update.source_adapters.includes("official-code") &&
  update.validation.contract_status_after_apply === "complete",
  update);
check("route has compact readiness",
  route.decision === "inject" &&
  route.skills.includes("board-t-display-s3") &&
  !route.skills.includes("board-t-display") &&
  !route.matches.some((match) => match.skill === "board-t-display") &&
  route.readiness.some((signal) =>
    signal.topic === "display" && signal.completeness === "complete"),
  route);
check("unsupported enrichment dry-run has no planned writes",
  unsupportedDryRun.status === "PASS" &&
  unsupportedDryRun.validation.contract_status_after_apply === "unsupported" &&
  unsupportedDryRun.planned_writes.length === 0 &&
  unsupportedDryRun.writes.length === 0,
  unsupportedDryRun);
check("hook compact no-write context",
  hook.decision === "inject" &&
  // Lean capsule: the push side drops the `readiness=[topic=complete]` status
  // list; the `expand=[..]` pull pointer (the source query that fetches the
  // source-backed detail) is what stays.
  hook.context.includes("expand=[") &&
  hook.context.includes("source query") &&
  !hook.context.includes("readiness=[") &&
  !hook.context.includes("update board-facts") &&
  hook.context.length < 1200,
  hook);

process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: [
    "source completeness display",
    "source query completeness metadata",
    "update board-facts dry-run",
    "unsupported board-facts dry-run/apply boundary",
    "route readiness",
    "hook compact readiness"
  ],
  completeness: completeness.completeness,
  route_skills: route.skills,
  hook_context_length: hook.context.length,
  writes: update.writes,
  unsupported_apply_exit: Number(process.env.unsupported_apply_exit || 1)
}, null, 2) + "\n");
NODE

if [[ "$before_hash" != "$after_hash" ]]; then
  echo "FAIL source-completeness-smoke mutated data/facts/board-fact-packs.json" >&2
  exit 1
fi
