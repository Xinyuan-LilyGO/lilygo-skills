#!/usr/bin/env node
// Generic, manifest-driven board fact ingestion. No per-board pin values are
// hardcoded here: every board's official source URL, line range, and macro
// mapping live in source-manifest.json. This fetches the official source,
// hashes it, extracts the declared #define macros within the declared line
// range, and merges source-backed pin/bus facts into board-fact-packs.json.
//
// Usage:
//   node pipeline/ingest-from-manifest.js --board <board-id> [--write] [--json]
//   node pipeline/ingest-from-manifest.js --all [--write] [--json]
// Fetching uses curl so the ambient http(s)_proxy is honored; without --write
// it only reports the extracted facts (dry-run), never mutating committed data.

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { execFileSync } = require("child_process");
const { autoMapPins } = require("./auto-map-pins");

const ROOT = path.join(__dirname, "..");
const MANIFEST = path.join(ROOT, "pipeline/source-manifest.json");
const FACT_PACK = path.join(ROOT, "data/facts/board-fact-packs.json");

const args = process.argv.slice(2);
const write = args.includes("--write");
const jsonOut = args.includes("--json");
const allBoards = args.includes("--all");
const boardArg = (() => {
  const i = args.indexOf("--board");
  return i >= 0 ? args[i + 1] : null;
})();

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
    if (!(m[1] in map)) map[m[1]] = m[2]; // first definition wins inside the block
  }
  return map;
}

function fillTemplate(tpl, macros) {
  return tpl.replace(/\{([A-Z0-9_]+)\}/g, (_, name) => {
    if (!(name in macros)) throw new Error(`template macro ${name} not found in source block`);
    return macros[name];
  });
}

function entry(source, sourceObj, key, value, claim) {
  return {
    schema_version: 1,
    board_id: source.board_id,
    topic: source.topic,
    key,
    value,
    claim,
    source: sourceObj,
    authority_rank: source.authority_rank,
    evidence_level: "V3-source-reference",
    stale: false,
    confidence: "exact",
  };
}

function ingestSource(source) {
  const text = fetchText(source.url);
  const hash = "sha256:" + crypto.createHash("sha256").update(text).digest("hex");
  const block = sliceRange(text, source.line_range);
  const macros = extractMacros(block);
  const sourceObj = {
    kind: source.source_kind,
    path_or_url: source.url.replace("raw.githubusercontent.com", "github.com").replace("/master/", "/blob/master/"),
    line_range: source.line_range,
    hash,
  };
  const sourceName = source.url.split("/").pop();
  let pins;
  if (source.auto_pins) {
    // Framework-grown board: no hand-written pin mapping. Derive canonical
    // keys from the naming convention; unrecognized macros are skipped.
    pins = autoMapPins(macros).map((p) =>
      entry(
        source,
        sourceObj,
        p.key,
        `${p.macro}=GPIO${p.value_num}`,
        `${p.key} auto-mapped from ${p.macro} in official ${sourceName}`
      )
    );
  } else {
    pins = (source.pins || []).map((p) => {
      if (!(p.macro in macros)) throw new Error(`${source.board_id}: macro ${p.macro} not found in ${source.line_range}`);
      return entry(source, sourceObj, p.key, `${p.macro}=GPIO${macros[p.macro]}`, p.claim);
    });
  }
  const buses = (source.buses || []).map((b) =>
    entry(source, sourceObj, b.key, fillTemplate(b.template, macros), b.claim)
  );
  // Authority boundary: if any macro we depend on is defined with different
  // values elsewhere in the same file (a multi-variant header, e.g. one board
  // block vs another), the chosen line_range is a human judgment that must be
  // explicitly acknowledged. The framework never silently picks a variant.
  const referenced = new Set();
  for (const p of source.pins || []) referenced.add(p.macro);
  if (source.auto_pins) autoMapPins(macros).forEach((p) => referenced.add(p.macro));
  for (const b of source.buses || []) {
    (b.template.match(/\{([A-Z0-9_]+)\}/g) || []).forEach((m) => referenced.add(m.slice(1, -1)));
  }
  const conflicts = detectVariantConflicts(text, referenced, macros);
  if (conflicts.length && source.variant_confirmed !== true) {
    throw new Error(
      `${source.board_id}: multi-variant macros require human review and "variant_confirmed": true in the manifest ` +
        `(this header defines them with different values in different board blocks): ` +
        conflicts.map((c) => `${c.macro}=${c.chosen} vs ${c.all_values.join("/")}`).join("; ")
    );
  }
  return { pins, buses, hash, conflicts_acknowledged: conflicts };
}

// Returns referenced macros that the full file defines with more than one
// distinct value, with the value chosen by the manifest line_range.
function detectVariantConflicts(fullText, referenced, chosenMacros) {
  const defs = {};
  const re = /^\s*#define\s+([A-Z0-9_]+)\s+(\d+)\b/gm;
  let m;
  while ((m = re.exec(fullText)) !== null) {
    (defs[m[1]] = defs[m[1]] || new Set()).add(m[2]);
  }
  const conflicts = [];
  for (const macro of referenced) {
    const values = defs[macro];
    if (values && values.size > 1) {
      conflicts.push({ macro, chosen: chosenMacros[macro], all_values: [...values] });
    }
  }
  return conflicts;
}

function mergeIntoPack(pack, pins, buses) {
  pack.pin_matrix = pack.pin_matrix || [];
  pack.bus_matrix = pack.bus_matrix || [];
  const upsert = (arr, e) => {
    const i = arr.findIndex((x) => x.key === e.key);
    if (i >= 0) arr[i] = e;
    else arr.push(e);
  };
  pins.forEach((e) => upsert(pack.pin_matrix, e));
  buses.forEach((e) => upsert(pack.bus_matrix, e));
}

const manifest = JSON.parse(fs.readFileSync(MANIFEST, "utf8"));
const factData = JSON.parse(fs.readFileSync(FACT_PACK, "utf8"));
const byId = new Map(factData.packs.map((p) => [p.board_id, p]));

const selected = manifest.sources.filter(
  (s) => allBoards || s.board_id === boardArg
);
if (!selected.length) {
  console.error("no matching source; pass --board <id> or --all");
  process.exit(2);
}

const results = [];
for (const source of selected) {
  const { pins, buses, hash } = ingestSource(source);
  const pack = byId.get(source.board_id);
  if (!pack) throw new Error(`fact pack has no board ${source.board_id}`);
  if (write) mergeIntoPack(pack, pins, buses);
  results.push({
    board_id: source.board_id,
    source_hash: hash,
    pins: pins.length,
    buses: buses.length,
    sample: pins.slice(0, 2).map((p) => `${p.key}=${p.value}`),
  });
}

if (write) {
  fs.writeFileSync(FACT_PACK, JSON.stringify(factData, null, 2) + "\n");
}

const report = { status: "PASS", mode: write ? "written" : "dry-run", ingested: results };
process.stdout.write(jsonOut ? JSON.stringify(report, null, 2) + "\n" : report.mode + " " + results.map((r) => r.board_id).join(",") + "\n");
