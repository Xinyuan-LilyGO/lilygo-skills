#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--dry-run" ]]; then
  shift
fi
if [[ "$#" -ne 0 ]]; then
  echo "usage: board-completeness-smoke.sh [--dry-run]" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" source completeness --board board-t-watch-ultra --topic display --json \
  >.tmp/board-completeness-watch-display.json
"$BIN" source completeness --board board-t-watch-ultra --topic imu --json \
  >.tmp/board-completeness-watch-imu.json
"$BIN" source completeness --board board-t-watch-ultra --topic power --json \
  >.tmp/board-completeness-watch-power.json
"$BIN" source completeness --board board-t-display-s3 --topic display --json \
  >.tmp/board-completeness-display-s3.json
"$BIN" source completeness --board board-t-beam --topic lora --json \
  >.tmp/board-completeness-beam-lora.json
"$BIN" source completeness --board board-t-beam --topic gnss --json \
  >.tmp/board-completeness-beam-gnss.json
"$BIN" source completeness --board board-t-deck --topic display --json \
  >.tmp/board-completeness-deck-display.json
"$BIN" source completeness --board board-t-deck --topic input --json \
  >.tmp/board-completeness-deck-input.json
"$BIN" source completeness --board board-rp2040 --topic display --json \
  >.tmp/board-completeness-unsupported.json

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
const cases = {
  watchDisplay: read(".tmp/board-completeness-watch-display.json"),
  watchImu: read(".tmp/board-completeness-watch-imu.json"),
  watchPower: read(".tmp/board-completeness-watch-power.json"),
  displayS3: read(".tmp/board-completeness-display-s3.json"),
  beamLora: read(".tmp/board-completeness-beam-lora.json"),
  beamGnss: read(".tmp/board-completeness-beam-gnss.json"),
  deckDisplay: read(".tmp/board-completeness-deck-display.json"),
  deckInput: read(".tmp/board-completeness-deck-input.json"),
  unsupported: read(".tmp/board-completeness-unsupported.json")
};
for (const [name, report] of Object.entries({
  watchDisplay: cases.watchDisplay,
  watchImu: cases.watchImu,
  watchPower: cases.watchPower
})) {
  check(`${name} complete`, report.completeness === "complete" && report.required_missing.length === 0, report);
}
for (const [name, report] of Object.entries({
  displayS3: cases.displayS3,
  beamLora: cases.beamLora,
  beamGnss: cases.beamGnss,
  deckDisplay: cases.deckDisplay,
  deckInput: cases.deckInput
})) {
  check(`${name} visible readiness`,
    ["complete", "partial", "needs_source_ingestion"].includes(report.completeness), report);
  check(`${name} actionable when incomplete`,
    report.completeness === "complete" ||
    report.next_actions.some((action) => action.command.includes("update board-facts") ||
      action.command.includes("source query")),
    report);
}
check("unsupported boundary", cases.unsupported.completeness === "unsupported", cases.unsupported);

process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: Object.keys(cases),
  readiness: Object.fromEntries(Object.entries(cases).map(([name, report]) => [
    name,
    {
      board: report.board_id,
      topic: report.topic,
      completeness: report.completeness,
      missing: report.required_missing
    }
  ]))
}, null, 2) + "\n");
NODE
