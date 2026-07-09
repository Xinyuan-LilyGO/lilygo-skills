#!/usr/bin/env node
// Authority-boundary gate. For every manifest source, fetch its official file
// and detect macros that the file defines with more than one value across
// board blocks (multi-variant headers). A source that depends on any such
// macro must carry "variant_confirmed": true — an explicit record that a human
// reviewed which board block the line_range selects. The framework never
// silently resolves a variant conflict; this gate proves it.

const path = require("path");
const fs = require("fs");
const { execFileSync } = require("child_process");
const { autoMapPins } = require("./auto-map-pins");
const { firstDefineMap, allDefineValues } = require("./extract-defines");

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
function extractBlockMacros(block) {
  return firstDefineMap(block);
}
function allDefs(text) {
  return allDefineValues(text);
}

const manifest = JSON.parse(fs.readFileSync(MANIFEST, "utf8"));
const results = [];
let failed = false;

for (const source of manifest.sources) {
  const text = fetchText(source.url);
  const block = sliceRange(text, source.line_range);
  const macros = extractBlockMacros(block);
  const referenced = new Set();
  for (const p of source.pins || []) referenced.add(p.macro);
  if (source.auto_pins) autoMapPins(macros).forEach((p) => referenced.add(p.macro));
  for (const b of source.buses || []) {
    (b.template.match(/\{([A-Z0-9_]+)\}/g) || []).forEach((m) => referenced.add(m.slice(1, -1)));
  }
  const defs = allDefs(text);
  const conflicts = [];
  for (const macro of referenced) {
    const v = defs[macro];
    if (v && v.size > 1) conflicts.push({ macro, chosen: macros[macro], all_values: [...v] });
  }
  const confirmed = source.variant_confirmed === true;
  // Fail if a source depends on a multi-variant macro without explicit human confirmation.
  const ok = conflicts.length === 0 || confirmed;
  if (!ok) failed = true;
  results.push({
    board_id: source.board_id,
    multi_variant_macros: conflicts.length,
    variant_confirmed: confirmed,
    conflicts,
    verdict: ok ? "OK" : "UNCONFIRMED_CONFLICT",
  });
}

process.stdout.write(
  JSON.stringify({ status: failed ? "FAIL" : "PASS", boards: results }, null, 2) + "\n"
);
process.exit(failed ? 1 : 0);
