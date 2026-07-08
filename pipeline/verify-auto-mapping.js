#!/usr/bin/env node
// Equivalence gate for the naming-convention auto-mapper: for every manifest
// source that carries a hand-written pins array, fetch its official source,
// auto-map the block's macros, and assert the auto-mapped (key -> GPIO value)
// set reproduces the hand-authored pins exactly. If the convention table
// cannot reproduce the human mappings, this fails — the framework is only
// allowed to grow itself where it provably matches human work.

const path = require("path");
const { execFileSync } = require("child_process");
const fs = require("fs");
const { autoMapPins } = require("./auto-map-pins");

const ROOT = path.join(__dirname, "..");
const MANIFEST = path.join(ROOT, "pipeline/source-manifest.json");

function fetchText(url) {
  return execFileSync("curl", ["-sfL", "--max-time", "30", url], {
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
}

function sliceRange(text, range) {
  const [a, b] = range.split("-").map((n) => parseInt(n, 10));
  return text.split("\n").slice(a - 1, b).join("\n");
}

function extractMacros(block) {
  const map = {};
  const re = /^\s*#define\s+([A-Z0-9_]+)\s+(\d+)\b/gm;
  let m;
  while ((m = re.exec(block)) !== null) {
    if (!(m[1] in map)) map[m[1]] = m[2];
  }
  return map;
}

const manifest = JSON.parse(fs.readFileSync(MANIFEST, "utf8"));
const results = [];
let failed = false;

for (const source of manifest.sources) {
  if (!Array.isArray(source.pins) || !source.pins.length) continue;
  const block = sliceRange(fetchText(source.url), source.line_range);
  const macros = extractMacros(block);
  const auto = new Map(
    autoMapPins(macros).map((p) => [p.key, `${p.macro}=GPIO${p.value_num}`])
  );
  const mismatches = [];
  for (const pin of source.pins) {
    const expected = `${pin.macro}=GPIO${macros[pin.macro]}`;
    const got = auto.get(pin.key);
    if (got !== expected) {
      mismatches.push({ key: pin.key, hand: expected, auto: got || null });
    }
  }
  // Also flag hand keys the auto-mapper missed entirely.
  const handKeys = new Set(source.pins.map((p) => p.key));
  const extraAuto = [...auto.keys()].filter((k) => !handKeys.has(k));
  if (mismatches.length) failed = true;
  results.push({
    board_id: source.board_id,
    hand_pins: source.pins.length,
    auto_reproduced: source.pins.length - mismatches.length,
    mismatches,
    auto_only_keys: extraAuto,
  });
}

const report = { status: failed ? "FAIL" : "PASS", boards: results };
process.stdout.write(JSON.stringify(report, null, 2) + "\n");
process.exit(failed ? 1 : 0);
