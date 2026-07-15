import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const AUTO_BOARDS = [
  "board-t-rgb",
  "board-t-2can",
  "board-t-circle",
  "board-t-embed-cc1101",
  "board-t-lora-dual",
  "board-t-nixietube",
  "board-t-touchbar-amoled",
  "board-t-wristband",
];

/** @param {string} relative @returns {Promise<any>} */
async function readJson(relative) {
  return JSON.parse(await readFile(join(ROOT, relative), "utf8"));
}

test("all eight auto-clean boards are committed with gated provenance and reproducible manifest sources", async () => {
  const boards = /** @type {BoardIndex} */ (await readJson("data/boards.json"));
  const facts = /** @type {FactPackIndex} */ (await readJson("data/facts/board-fact-packs.json"));
  const manifest = /** @type {{ sources: { board_id: string; auto_pins?: boolean; url: string }[] }} */ (
    await readJson("pipeline/source-manifest.json")
  );
  const boardIds = new Set(boards.boards.map((board) => board.id));
  const packs = new Map(facts.packs.map((pack) => [pack.board_id, pack]));
  const sources = new Map(manifest.sources.map((source) => [source.board_id, source]));

  assert.equal(boards.boards.length, 34);
  assert.equal(facts.packs.length, 34);
  for (const id of AUTO_BOARDS) {
    assert.equal(boardIds.has(id), true, `${id} board record`);
    const pack = packs.get(id);
    assert.ok(pack, `${id} fact pack`);
    assert.ok(pack.pin_matrix.length >= 2, `${id} mapped pin count`);
    assert.ok(pack.pin_matrix.every((fact) => fact.confidence === "exact"));
    assert.ok(pack.pin_matrix.every((fact) => /^https:\/\/github\.com\/Xinyuan-LilyGO\/.+\/blob\/[0-9a-f]{40}\//.test(fact.source.path_or_url)));
    assert.ok(pack.pin_matrix.every((fact) => /^\d+-\d+$/.test(fact.source.line_range || "")));
    assert.ok(pack.pin_matrix.every((fact) => /^sha256:[0-9a-f]{64}$/.test(fact.source.hash)));
    const source = sources.get(id);
    assert.equal(source?.auto_pins, true);
    assert.match(source?.url || "", /^https:\/\/raw\.githubusercontent\.com\/Xinyuan-LilyGO\//);
  }
});
