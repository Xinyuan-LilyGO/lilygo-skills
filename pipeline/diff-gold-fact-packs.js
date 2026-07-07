#!/usr/bin/env node
const {
  GENERATED_PATH,
  FACT_PACK_PATH,
  GOLD_BOARDS,
  readJson
} = require("./official-source-lib");

function byBoard(index) {
  return new Map(index.packs.map((pack) => [pack.board_id, pack]));
}

function entries(pack) {
  const out = new Map();
  for (const table of ["pin_matrix", "bus_matrix", "expander_matrix", "connector_matrix", "peripheral_table"]) {
    for (const entry of pack[table] || []) {
      out.set(`${table}:${entry.key}`, {
        value: entry.value,
        confidence: entry.confidence,
        source: entry.source?.path_or_url,
        hash: entry.source?.hash
      });
    }
  }
  return out;
}

const current = byBoard(readJson(FACT_PACK_PATH));
const generated = byBoard(readJson(GENERATED_PATH));
const mismatches = [];
const missing = [];

for (const boardId of GOLD_BOARDS) {
  const hand = current.get(boardId);
  const regen = generated.get(boardId);
  if (!hand || !regen) {
    missing.push(boardId);
    continue;
  }
  const handEntries = entries(hand);
  const regenEntries = entries(regen);
  for (const [key, expected] of handEntries.entries()) {
    const actual = regenEntries.get(key);
    if (!actual) {
      mismatches.push({ board_id: boardId, key, reason: "missing_generated_field" });
      continue;
    }
    for (const field of ["value", "confidence", "source", "hash"]) {
      if (expected[field] !== actual[field]) {
        mismatches.push({ board_id: boardId, key, field, expected: expected[field], actual: actual[field] });
      }
    }
  }
}

if (missing.length || mismatches.length) {
  console.error(JSON.stringify({ status: "FAIL", missing, mismatches }, null, 2));
  process.exit(1);
}

console.log(JSON.stringify({
  status: "PASS",
  gold_boards: GOLD_BOARDS,
  compared_boards: GOLD_BOARDS.length,
  mismatches: 0
}, null, 2));
