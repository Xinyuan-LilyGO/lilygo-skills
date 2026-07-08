#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--dry-run" ]]; then
  shift
fi
if [[ "$#" -ne 0 ]]; then
  echo "usage: source-fact-smoke.sh [--dry-run]" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" source query --board board-t-watch-ultra --topic io --json \
  >.tmp/source-fact-io.json
"$BIN" source query --board board-t-watch-ultra --topic expander --json \
  >.tmp/source-fact-expander.json
"$BIN" update fact-packs --dry-run --json \
  >.tmp/source-fact-update-dry-run.json
"$BIN" goal plan --json "T-Watch Ultra Arduino IO口怎么用? 哪些GPIO接了外设?" \
  >.tmp/source-fact-goal.json

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
const io = read(".tmp/source-fact-io.json");
const expander = read(".tmp/source-fact-expander.json");
const update = read(".tmp/source-fact-update-dry-run.json");
const goal = read(".tmp/source-fact-goal.json");

check("source query io pass", io.status === "PASS" && io.supported, io);
check("io has source-backed tables",
  io.fact_pack.pin_matrix.length > 0 &&
  io.fact_pack.bus_matrix.length > 0 &&
  io.fact_pack.expander_matrix.length > 0 &&
  io.fact_pack.peripheral_table.length > 0, io.fact_pack);
check("expander unknown is not guessed",
  expander.facts.some((fact) => fact.key === "expander.xl9555.channel-map" &&
    fact.value === "unknown_with_sources" &&
    fact.confidence === "unknown_with_sources"), expander.facts);
check("fact update dry-run writes empty",
  update.status === "PASS" && update.dry_run === true && Array.isArray(update.writes) && update.writes.length === 0,
  update);
check("goal expands compact fact tables",
  goal.context_capsule.fact_tables.length >= 4 &&
  goal.context_capsule.source_refs.length <= goal.context_capsule.budget.max_source_refs_inline &&
  goal.context_capsule.discovery_hints.length > 0,
  goal.context_capsule);
const preferences = goal.context_capsule.preferences || [];
const referenceHints = goal.context_capsule.reference_hints || [];
check("fact lookup avoids preference/reference over-injection",
  preferences.length === 0 &&
  referenceHints.length === 0,
  goal.context_capsule);
check("fact lookup avoids implementation recipe over-injection",
  (goal.recipe_ids || []).every((id) => ![
    "recipe-run-official-demo",
    "recipe-build-upload-monitor",
    "recipe-lvgl-simulator"
  ].includes(id)) &&
  !goal.context_capsule.implementation_start &&
  (goal.context_capsule.critical_facts || []).length === 0 &&
  (goal.context_capsule.recovery_actions || []).length === 0,
  { recipe_ids: goal.recipe_ids, context_capsule: goal.context_capsule });

process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: [
    "source query io",
    "source query expander unknown_with_sources",
    "update fact-packs dry-run",
    "goal compact fact expansion",
    "no fact-prompt over-injection"
  ],
  io_fact_count: io.facts.length,
  goal_fact_tables: goal.context_capsule.fact_tables.map((table) => ({
    table: table.table,
    preview_count: table.preview_count,
    overflow_count: table.overflow_count
  })),
  discovery_hints: goal.context_capsule.discovery_hints
}, null, 2) + "\n");
NODE
