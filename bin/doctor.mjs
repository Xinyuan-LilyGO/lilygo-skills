// `doctor --json`: data-integrity self-check for the JS thin core (board count,
// fact-pack V3 coverage, required data files, sniff-rules parseable, sample
// injection). Exit-code semantics mirror Rust doctor: `doctor` without `--json`
// is a usage error (exit 2); a non-PASS report also exits 2.
import { existsSync } from "node:fs";
import { loadBoards, loadFactPacks, dataPath, isMain } from "./lib.mjs";
import { buildContext, loadSniffRules } from "./find.mjs";

/**
 * Run a loader and swallow any failure (a bad/absent file is reported by the
 * data-files check, not a crash).
 * @template T
 * @param {() => T} load
 * @returns {T | null}
 */
function tryLoad(load) {
  try {
    return load();
  } catch {
    return null;
  }
}

/**
 * @typedef {{ id: string, status: string, summary: string }} DoctorCheck
 */

/**
 * @returns {{ schema_version: number, status: string, runtime_mode: string, checks: DoctorCheck[], sample_injection: unknown, warnings: string[] }}
 */
export function doctorReport() {
  /** @type {DoctorCheck[]} */
  const checks = [];
  const files = [
    "data/boards.json", "data/facts/board-fact-packs.json",
    "data/facts/prompt-keywords.json", "data/facts/topic-fields.json", "data/sniff-rules.json",
  ];
  const missing = files.filter((rel) => !existsSync(dataPath(rel)));
  checks.push(check("data-files", missing.length === 0, missing.length === 0
    ? `all ${files.length} runtime data files present`
    : `missing: ${missing.join(", ")}`));

  const boards = tryLoad(loadBoards);
  checks.push(check("boards", (boards?.boards.length ?? 0) > 0, `${boards?.boards.length ?? 0} boards in registry`));

  const index = tryLoad(loadFactPacks);
  const packs = index?.packs ?? [];
  checks.push(check("fact-packs", packs.length > 0, `${packs.length} board fact packs`));

  let total = 0;
  let v3 = 0;
  for (const pack of packs) {
    for (const table of [pack.pin_matrix, pack.bus_matrix, pack.expander_matrix, pack.connector_matrix, pack.peripheral_table]) {
      for (const fact of table) {
        total++;
        if (typeof fact.evidence_level === "string" && fact.evidence_level.startsWith("V3")) v3++;
      }
    }
  }
  checks.push(check("v3-coverage", total > 0 && v3 === total, `${v3}/${total} facts carry V3 evidence`));

  const rules = tryLoad(loadSniffRules);
  checks.push(check("sniff-rules", (rules?.boards.length ?? 0) > 0, `${rules?.boards.length ?? 0} board sniff matchers`));

  const status = checks.every((c) => c.status === "PASS") ? "PASS" : "FAIL";
  const sample = buildContext({ prompt: "T-Display-S3 pinout" });
  return {
    schema_version: 1,
    status,
    runtime_mode: "js-thin-core",
    checks,
    sample_injection: { board: sample.board, decision: sample.decision, context: sample.context },
    warnings: [],
  };
}

/**
 * @param {string} id
 * @param {boolean} ok
 * @param {string} summary
 * @returns {DoctorCheck}
 */
function check(id, ok, summary) {
  return { id, status: ok ? "PASS" : "FAIL", summary };
}

/**
 * @param {string[]} argv
 * @returns {number} exit code
 */
export function runDoctor(argv) {
  const args = argv[0] === "doctor" ? argv.slice(1) : argv;
  if (!args.includes("--json")) {
    process.stderr.write("--json is required for this command\n");
    return 2;
  }
  const report = doctorReport();
  process.stdout.write(JSON.stringify(report, null, 2) + "\n");
  return report.status === "PASS" ? 0 : 2;
}

if (isMain(import.meta.url)) {
  process.exit(runDoctor(process.argv.slice(2)));
}
