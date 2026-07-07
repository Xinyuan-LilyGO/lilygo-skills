#!/usr/bin/env node
const {
  GENERATED_PATH,
  FACT_PACK_PATH,
  GOLD_BOARDS,
  allAcceptedFieldsHaveSource,
  generatePacks,
  knownPitfallsHaveIssueSource,
  writeJson
} = require("./official-source-lib");

const args = process.argv.slice(2);
const goldOnly = args.includes("--gold-only");
const allBoards = args.includes("--all-boards");
const write = args.includes("--write");

if (!goldOnly && !allBoards) {
  console.error("usage: node pipeline/run-official-source-pipeline.js --gold-only|--all-boards [--write] [--json]");
  process.exit(1);
}

const generated = generatePacks({ goldOnly });
const missing = generated.packs.flatMap(allAcceptedFieldsHaveSource);
const badPitfalls = generated.packs.flatMap(knownPitfallsHaveIssueSource);
if (missing.length) {
  console.error(JSON.stringify({ status: "FAIL", reason: "accepted_fields_missing_source", missing }, null, 2));
  process.exit(1);
}
if (badPitfalls.length) {
  console.error(JSON.stringify({ status: "FAIL", reason: "known_pitfall_requires_issue_source", bad_pitfalls: badPitfalls }, null, 2));
  process.exit(1);
}

writeJson(GENERATED_PATH, generated);
const writes = [".tmp/pipeline/board-fact-packs.generated.json"];
if (write && allBoards) {
  writeJson(FACT_PACK_PATH, generated);
  writes.push("data/facts/board-fact-packs.json");
}

const exactBoards = generated.packs.filter((pack) => {
  const facts = [
    ...(pack.pin_matrix || []),
    ...(pack.bus_matrix || []),
    ...(pack.expander_matrix || []),
    ...(pack.connector_matrix || []),
    ...(pack.peripheral_table || [])
  ];
  return facts.some((fact) => fact.confidence === "exact");
}).length;
const unknownWithSourcesBoards = generated.packs.filter((pack) => {
  const facts = [
    ...(pack.pin_matrix || []),
    ...(pack.bus_matrix || []),
    ...(pack.expander_matrix || []),
    ...(pack.connector_matrix || []),
    ...(pack.peripheral_table || [])
  ];
  return facts.some((fact) => fact.confidence === "unknown_with_sources");
}).length;

console.log(JSON.stringify({
  status: "PASS",
  mode: goldOnly ? "gold-only" : "all-boards",
  boards_total: generated.packs.length,
  gold_boards: GOLD_BOARDS,
  exact_boards: exactBoards,
  unknown_with_sources_boards: unknownWithSourcesBoards,
  fields_missing_source: 0,
  known_pitfalls_with_issue_source: badPitfalls.length === 0,
  writes
}, null, 2));
