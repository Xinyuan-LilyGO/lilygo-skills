// CLI contract-parity test for the JS thin core.
//
// Walks eval/fixtures/cli-contract.json (a structural snapshot of the Rust CLI)
// and, for every source-query / context entry, runs the JS command and asserts:
//   (a) the process exit code matches the recorded contract,
//   (b) the top-level JSON key set matches the snapshot exactly,
//   (c) source-query: every returned fact's key->value equals the committed
//       fact pack (the anti-fabrication "value parity" gate).
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const BIN = join(ROOT, "bin");
const contract = JSON.parse(readFileSync(join(ROOT, "eval/fixtures/cli-contract.json"), "utf8"));
const packIndex = JSON.parse(readFileSync(join(ROOT, "data/facts/board-fact-packs.json"), "utf8"));

/**
 * Run a bin/*.mjs entrypoint with argv and capture stdout + exit code.
 * @param {string} script
 * @param {string[]} argv
 * @returns {{ code: number, stdout: string }}
 */
function runJs(script, argv) {
  try {
    const stdout = execFileSync(process.execPath, [join(BIN, script), ...argv], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    return { code: 0, stdout };
  } catch (error) {
    const e = /** @type {{ status?: number, stdout?: string }} */ (error);
    return { code: typeof e.status === "number" ? e.status : 1, stdout: e.stdout ?? "" };
  }
}

/**
 * key+value set across every fact in a board's committed pack.
 * @param {string} boardId
 * @returns {Set<string>}
 */
function packValueSet(boardId) {
  const pack = packIndex.packs.find((/** @type {FactPack} */ p) => p.board_id === boardId);
  assert.ok(pack, `fact pack missing for ${boardId}`);
  const set = new Set();
  const tables = [pack.pin_matrix, pack.bus_matrix, pack.expander_matrix, pack.connector_matrix, pack.peripheral_table];
  for (const table of tables) {
    for (const fact of table) set.add(`${fact.key} ${fact.value}`);
  }
  return set;
}

const sourceQueryEntries = contract.entries.filter(
  (/** @type {{ label: string }} */ e) => e.label.startsWith("source-query:"),
);
assert.ok(sourceQueryEntries.length >= 7, "expected the source-query contract entries");

for (const entry of sourceQueryEntries) {
  test(`contract ${entry.label}`, () => {
    const { code, stdout } = runJs("query.mjs", entry.args);
    assert.equal(code, entry.exit_code, `exit code for ${entry.label}`);
    const json = JSON.parse(stdout);
    assert.deepEqual(Object.keys(json), entry.output_shape.top_level_keys, `top-level keys for ${entry.label}`);
    const values = packValueSet(json.board_id);
    assert.ok(json.facts.length >= 1, `${entry.label} returned no facts`);
    for (const fact of json.facts) {
      assert.ok(
        values.has(`${fact.key} ${fact.value}`),
        `fact ${fact.key}=${fact.value} not found in ${json.board_id} fact pack (fabrication?)`,
      );
    }
  });
}
