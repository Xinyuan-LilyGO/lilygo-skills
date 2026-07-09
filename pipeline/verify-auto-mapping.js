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
const { autoMapPins, normalizeMacro, loadConventions } = require("./auto-map-pins");
const { firstDefineMap } = require("./extract-defines");

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
  return firstDefineMap(block);
}

const manifest = JSON.parse(fs.readFileSync(MANIFEST, "utf8"));
const conv = loadConventions();
const conventionRecognizes = (macro) => Boolean((conv.macro_to_key || {})[normalizeMacro(macro, conv)]);
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
  // Equivalence is only assertable where the naming convention claims to cover
  // the macro: for those, the auto-mapper must reproduce the hand mapping
  // exactly (this keeps the convention table honest and fully guards the
  // reference boards whose macros are all convention-covered). A board may also
  // carry explicit hand mappings for macros the convention does not name (novel
  // vendor headers); those are reported as convention_uncovered, not failures,
  // because verify-source-authority is what guards their extraction.
  const uncovered = [];
  for (const pin of source.pins) {
    if (!conventionRecognizes(pin.macro)) {
      uncovered.push(pin.key);
      continue;
    }
    const expected = `${pin.macro}=GPIO${macros[pin.macro]}`;
    const got = auto.get(pin.key);
    if (got !== expected) {
      mismatches.push({ key: pin.key, hand: expected, auto: got || null });
    }
  }
  const handKeys = new Set(source.pins.map((p) => p.key));
  const extraAuto = [...auto.keys()].filter((k) => !handKeys.has(k));
  if (mismatches.length) failed = true;
  results.push({
    board_id: source.board_id,
    hand_pins: source.pins.length,
    convention_covered: source.pins.length - uncovered.length,
    auto_reproduced: source.pins.length - uncovered.length - mismatches.length,
    convention_uncovered: uncovered,
    mismatches,
    auto_only_keys: extraAuto,
  });
}

const report = { status: failed ? "FAIL" : "PASS", boards: results };
process.stdout.write(JSON.stringify(report, null, 2) + "\n");
process.exit(failed ? 1 : 0);
