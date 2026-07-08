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

"$BIN" source query --board board-t-beam --topic lora --json >.tmp/board-data-beam-lora.json
"$BIN" source query --board board-t-beam --topic spi --json >.tmp/board-data-beam-spi.json
"$BIN" source query --board board-t-deck --topic display --json >.tmp/board-data-deck-display.json
"$BIN" source query --board board-t-display-s3-amoled --topic input --json >.tmp/board-data-amoled-input.json
"$BIN" source completeness --board board-t-beam --topic lora --json >.tmp/board-data-beam-lora-completeness.json
"$BIN" source completeness --board board-t-deck --topic input --json >.tmp/board-data-deck-input-completeness.json
"$BIN" route --json "T-Beam SX1262 LoRa send packet" >.tmp/board-data-tbeam-route.json

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
function hasSource(report, text) {
  return (report.source_refs || []).some((source) => source.path_or_url.includes(text));
}
function hasUnknown(report) {
  return (report.facts || []).some((fact) => fact.confidence === "unknown_with_sources");
}
const beamLora = read(".tmp/board-data-beam-lora.json");
const beamSpi = read(".tmp/board-data-beam-spi.json");
const deckDisplay = read(".tmp/board-data-deck-display.json");
const amoledInput = read(".tmp/board-data-amoled-input.json");
const beamLoraCompleteness = read(".tmp/board-data-beam-lora-completeness.json");
const deckInputCompleteness = read(".tmp/board-data-deck-input-completeness.json");
const tbeamRoute = read(".tmp/board-data-tbeam-route.json");
check("T-Beam route still matches SX1262 LoRa", tbeamRoute.skills.includes("board-t-beam") && tbeamRoute.skills.includes("periph-lora"), tbeamRoute);
check("T-Beam lora source refs include official repo", hasSource(beamLora, "LilyGo-LoRa-Series"), beamLora);
check("T-Beam lora stays unknown_with_sources", hasUnknown(beamLora), beamLora.facts);
check("T-Beam spi is source-backed exact after ingestion", hasSource(beamSpi, "LilyGo-LoRa-Series") && (beamSpi.facts || []).some((fact) => fact.confidence === "exact"), beamSpi.facts);
check("T-Deck display source refs include official repo", hasSource(deckDisplay, "T-Deck"), deckDisplay);
check("T-Deck display stays unknown_with_sources", hasUnknown(deckDisplay), deckDisplay.facts);
check("AMOLED input stays unknown_with_sources", hasUnknown(amoledInput), amoledInput.facts);
check("T-Beam lora completeness never hides residual unknowns", beamLoraCompleteness.completeness !== "complete" || hasUnknown(beamLoraCompleteness), beamLoraCompleteness);
check("T-Deck input is not falsely complete", deckInputCompleteness.completeness !== "complete", deckInputCompleteness);
process.stdout.write(JSON.stringify({
  status: "PASS",
  checked_boards: ["board-t-beam", "board-t-deck", "board-t-display-s3-amoled"],
  route_skills: tbeamRoute.skills,
  unknown_with_sources: {
    beam_lora: hasUnknown(beamLora),
    beam_spi: hasUnknown(beamSpi),
    deck_display: hasUnknown(deckDisplay),
    amoled_input: hasUnknown(amoledInput)
  }
}, null, 2) + "\n");
NODE
