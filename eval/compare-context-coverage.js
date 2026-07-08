#!/usr/bin/env node
// Context-coverage ablation for the "three-source" simplification thesis.
//
// Question: how much of each eval task's winning answer is ALREADY carried by
// the pure structured facts (the three sources: pins/wiring + capability +
// demo/peripheral), versus requiring the extra guidance/recipe/honesty layers?
// The more the pure facts cover, the more of the surrounding machinery is
// redundant for these tasks and safe to sunset. What the facts DON'T cover is
// exactly what earns its keep. This is an information-coverage comparison over
// real eval ground truth — NOT a live-model A/B (that needs the private runner).
//
// Usage: node eval/compare-context-coverage.js [--json]

const fs = require("fs");
const path = require("path");
const root = path.resolve(__dirname, "..");
const jsonOut = process.argv.includes("--json");

const tasks = JSON.parse(fs.readFileSync(path.join(root, "eval/tasks.json"), "utf8")).tasks;
const packs = JSON.parse(fs.readFileSync(path.join(root, "data/facts/board-fact-packs.json"), "utf8")).packs;
const byId = new Map(packs.map((p) => [p.board_id, p]));

// The "three-source facts" text for a board: everything structured in its pack.
function factsText(pack) {
  if (!pack) return "";
  const parts = [];
  const arrays = ["pin_matrix", "bus_matrix", "expander_matrix", "connector_matrix", "peripheral_table"];
  for (const a of arrays) for (const e of pack[a] || []) parts.push([e.key, e.value, e.claim].filter(Boolean).join(" "));
  for (const s of pack.source_refs || []) parts.push([s.kind, s.path_or_url, s.line_range].filter(Boolean).join(" "));
  for (const c of pack.conflicts || []) parts.push(JSON.stringify(c));
  return parts.join("\n");
}
// Honesty markers a board carries (these are what keep the model from over-claiming).
function honestyMarkers(pack) {
  const t = factsText(pack).toLowerCase();
  return {
    gpio_free_unknown: t.includes("gpio.free") && t.includes("unknown_with_sources"),
    unknown_with_sources: t.includes("unknown_with_sources"),
  };
}
const norm = (s) => s.toLowerCase().replace(/\s+/g, " ").trim();
function covers(hay, needle) {
  const h = norm(hay), n = norm(needle);
  if (h.includes(n)) return true;
  // token-subset fallback for multi-word expected facts (e.g. "8-bit parallel")
  const toks = n.split(" ").filter((w) => w.length > 2);
  return toks.length > 1 && toks.every((w) => h.includes(w));
}

let expTotal = 0, expCovered = 0;
const uncovered = [];
const perBoard = {};
for (const task of tasks) {
  const pack = byId.get(task.board_id);
  const ft = factsText(pack);
  const b = (perBoard[task.board_id] = perBoard[task.board_id] || { exp: 0, cov: 0, tasks: 0 });
  b.tasks++;
  for (const ef of task.expected_facts || []) {
    expTotal++; b.exp++;
    if (covers(ft, ef)) { expCovered++; b.cov++; }
    else uncovered.push({ task: task.id, category: task.category, fact: ef });
  }
}

// Classify uncovered facts by what layer would supply them.
const layerOf = (f) => {
  const s = f.toLowerCase();
  if (/\.ino|example|factory|demo|setup\d|tft_espi/.test(s)) return "L3-demo/example";
  if (/evidence|verified|serial log|unverified|boundary|before scl|free gpio/.test(s)) return "honesty/guidance";
  if (/\d+\.\d+|arduino-esp32|library|core|pitfall|version/.test(s)) return "pitfall/version";
  return "other";
};
const byLayer = {};
for (const u of uncovered) byLayer[layerOf(u.fact)] = (byLayer[layerOf(u.fact)] || 0) + 1;

const report = {
  status: "PASS",
  thesis: "how much of the answer is pure three-source facts vs. needs extra layers",
  eval_tasks: tasks.length,
  expected_facts_total: expTotal,
  covered_by_pure_facts: expCovered,
  coverage_pct: Math.round((expCovered / expTotal) * 1000) / 10,
  uncovered_count: uncovered.length,
  uncovered_by_layer: byLayer,
  honesty_markers_present: Object.fromEntries(
    [...new Set(tasks.map((t) => t.board_id))].map((id) => [id, honestyMarkers(byId.get(id))])
  ),
  per_board: perBoard,
  uncovered_sample: uncovered.slice(0, 14),
};
process.stdout.write(JSON.stringify(report, null, jsonOut ? 2 : 2) + "\n");
