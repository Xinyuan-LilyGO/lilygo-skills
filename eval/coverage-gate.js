#!/usr/bin/env node
// P0 coverage gate. Grades the REAL injected capsule that the model actually
// receives -- `lilygo-skills hook claude` for each eval task's prompt -- against
// the task's expected_facts. This is stronger than compare-context-coverage.js
// (which reads only the static fact matrices): the live capsule also carries the
// chosen demo, header names, and critical pins, so this measures true coverage.
//
// It also enforces a baseline that the great-effect refactor must never regress
// below (prove-then-sunset): cutting scaffolding is only allowed if coverage of
// the real capsule holds. Honesty markers (hardware_verified=false, V3 evidence
// boundary) must stay present in every injected capsule.
//
// Usage:
//   node eval/coverage-gate.js                 # gate: fail if below baseline
//   node eval/coverage-gate.js --json          # machine-readable
//   node eval/coverage-gate.js --update-baseline   # record current as baseline

const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..");
const BIN = path.join(ROOT, "target/release/lilygo-skills");
const BASELINE_PATH = path.join(ROOT, "eval/coverage-baseline.json");
const args = process.argv.slice(2);
const jsonOut = args.includes("--json");
const updateBaseline = args.includes("--update-baseline");

const tasks = JSON.parse(fs.readFileSync(path.join(ROOT, "eval/tasks.json"), "utf8")).tasks;

function hookContext(prompt) {
  const out = execFileSync(BIN, ["hook", "claude"], {
    input: JSON.stringify({ prompt }),
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
  try {
    const j = JSON.parse(out);
    return j.hookSpecificOutput?.additionalContext || out;
  } catch {
    return out;
  }
}

const norm = (s) => s.toLowerCase().replace(/\s+/g, " ").trim();
function covers(hay, needle) {
  const h = norm(hay), n = norm(needle);
  if (h.includes(n)) return true;
  const toks = n.split(" ").filter((w) => w.length > 2);
  return toks.length > 1 && toks.every((w) => h.includes(w));
}

let expTotal = 0, expCovered = 0, honestyOk = 0, ctxBytes = 0;
const perBoard = {};
const uncovered = [];
for (const task of tasks) {
  const ctx = hookContext(task.prompt);
  ctxBytes += Buffer.byteLength(ctx);
  const low = ctx.toLowerCase();
  // Honesty markers: every capsule must keep the V3 evidence boundary and the
  // not-hardware-verified flag so the model never claims hardware success.
  if (low.includes("hardware_verified=false") && low.includes("evidence_boundary=v3")) honestyOk++;
  const b = (perBoard[task.board_id] = perBoard[task.board_id] || { exp: 0, cov: 0 });
  for (const ef of task.expected_facts || []) {
    expTotal++; b.exp++;
    if (covers(ctx, ef)) { expCovered++; b.cov++; }
    else uncovered.push({ task: task.id, fact: ef });
  }
}

const pct = Math.round((expCovered / expTotal) * 1000) / 10;
const report = {
  measured: {
    eval_tasks: tasks.length,
    expected_facts_total: expTotal,
    covered: expCovered,
    coverage_pct: pct,
    honesty_markers_ok: `${honestyOk}/${tasks.length}`,
    avg_capsule_bytes: Math.round(ctxBytes / tasks.length),
    per_board: perBoard,
    uncovered_sample: uncovered.slice(0, 12),
  },
};

if (updateBaseline) {
  fs.writeFileSync(
    BASELINE_PATH,
    JSON.stringify({ min_covered: expCovered, min_coverage_pct: pct, honesty_markers_required: honestyOk, note: "P0 baseline for the great-effect refactor; injected-capsule coverage and honesty markers must never regress below this. honesty_markers_required is the current measured floor (a capsule with no board data legitimately carries no marker). Update only with an explicit, reviewed reason." }, null, 2) + "\n"
  );
  report.status = "BASELINE_WRITTEN";
  process.stdout.write(JSON.stringify(report, null, 2) + "\n");
  process.exit(0);
}

const baseline = fs.existsSync(BASELINE_PATH) ? JSON.parse(fs.readFileSync(BASELINE_PATH, "utf8")) : null;
const failures = [];
if (baseline) {
  if (expCovered < baseline.min_covered) failures.push(`coverage regressed: ${expCovered} < baseline ${baseline.min_covered}`);
  if (honestyOk < (baseline.honesty_markers_required || tasks.length)) failures.push(`honesty markers dropped: ${honestyOk} < ${baseline.honesty_markers_required || tasks.length}`);
} else {
  failures.push("no baseline recorded; run with --update-baseline first");
}
report.status = failures.length ? "FAIL" : "PASS";
report.baseline = baseline;
if (failures.length) report.failures = failures;
process.stdout.write(JSON.stringify(report, null, jsonOut ? 2 : 2) + "\n");
process.exit(failures.length ? 1 : 0);
