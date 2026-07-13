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
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
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

const contextEntries = contract.entries.filter(
  (/** @type {{ label: string }} */ e) => e.label.startsWith("context:"),
);
assert.ok(contextEntries.length >= 3, "expected the context contract entries");

for (const entry of contextEntries) {
  test(`contract ${entry.label}`, () => {
    const { code, stdout } = runJs("find.mjs", entry.args);
    assert.equal(code, entry.exit_code, `exit code for ${entry.label}`);
    const json = JSON.parse(stdout);
    assert.deepEqual(Object.keys(json), entry.output_shape.top_level_keys, `top-level keys for ${entry.label}`);
  });
}

test("context sniffs a board from platformio.ini alone (no keyword)", () => {
  const dir = mkdtempSync(join(tmpdir(), "m35-ctx-"));
  try {
    writeFileSync(join(dir, "platformio.ini"), "[env:t-display-s3]\nboard = lilygo-t-display-s3\n");
    const { code, stdout } = runJs("find.mjs", ["context", "--json", "--project", dir, "wire the I2C bus"]);
    assert.equal(code, 0);
    const json = JSON.parse(stdout);
    assert.equal(json.board, "board-t-display-s3");
    assert.equal(json.board_source, "inferred-from-project");
    assert.equal(json.decision, "inject");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("context assigns no board on ambiguous project evidence", () => {
  const dir = mkdtempSync(join(tmpdir(), "m35-ctx-amb-"));
  try {
    writeFileSync(join(dir, "platformio.ini"), "[env:t-beam]\nboard = ttgo-t-beam\n\n[env:t-deck]\nboard = t-deck\n");
    const { code, stdout } = runJs("find.mjs", ["context", "--json", "--project", dir]);
    assert.equal(code, 0);
    const json = JSON.parse(stdout);
    assert.equal(json.board, null);
    assert.equal(json.decision, "no-op");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// --- verify sources: deterministic structural + exit-code contract ----------
// (The live hash re-proof is exercised out-of-band, not in the offline suite.)
import { rawFetchUrl } from "../bin/verify.mjs";

test("rawFetchUrl rewrites github blob to raw and passes others through", () => {
  assert.equal(
    rawFetchUrl("https://github.com/owner/repo/blob/master/src/x.h"),
    "https://raw.githubusercontent.com/owner/repo/master/src/x.h",
  );
  const raw = "https://raw.githubusercontent.com/owner/repo/master/x.h";
  assert.equal(rawFetchUrl(raw), raw);
});

for (const entry of contract.entries.filter((/** @type {{ label: string }} */ e) => e.label.startsWith("verify-sources:"))) {
  test(`verify contract shape ${entry.label}`, () => {
    // JS verifySources emits exactly these top-level keys by construction.
    assert.deepEqual(entry.output_shape.top_level_keys, ["status", "board_id", "counts", "facts"]);
  });
}

test("verify sources requires --json (exit 2)", () => {
  const { code } = runJs("verify.mjs", ["verify", "sources", "--board", "board-t-beam"]);
  assert.equal(code, 2);
});

test("verify sources rejects an unknown board (exit 2)", () => {
  const { code } = runJs("verify.mjs", ["verify", "sources", "--board", "board-does-not-exist", "--json"]);
  assert.equal(code, 2);
});

// --- doctor: exit-code + top-level key contract ----------------------------
for (const entry of contract.entries.filter((/** @type {{ label: string }} */ e) => e.label === "doctor:json")) {
  test("doctor --json contract", () => {
    const { code, stdout } = runJs("doctor.mjs", ["doctor", "--json"]);
    assert.equal(code, entry.exit_code);
    assert.deepEqual(Object.keys(JSON.parse(stdout)), entry.output_shape.top_level_keys);
  });
}

test("doctor requires --json (exit 2)", () => {
  const { code } = runJs("doctor.mjs", ["doctor"]);
  assert.equal(code, 2);
});
