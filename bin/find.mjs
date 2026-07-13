// Board detection (`context`): keyword + project-file sniffing, plus the thin
// pointer capsule. Matching is data-driven from data/sniff-rules.json (exported
// from the registry) and mirrors the honesty of Rust board_sniff.rs: a board is
// assigned only on unambiguous, registry-known evidence; ties resolve to none.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { readJson } from "./lib.mjs";

/** @type {SniffRules | undefined} */
let rulesCache;
/** @returns {SniffRules} */
export function loadSniffRules() {
  return (rulesCache ??= /** @type {SniffRules} */ (readJson("data/sniff-rules.json")));
}

/**
 * Lowercase and drop every non-alphanumeric char (mirrors board_sniff::normalize).
 * @param {string} value
 * @returns {string}
 */
export function normalizeCandidate(value) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "");
}

/**
 * Reduce candidate strings to at most one KNOWN board id: the board whose
 * longest matched alias (>= min_alias_len) is strictly the most specific. A tie
 * on top specificity is ambiguous -> null (never guess).
 * @param {string[]} candidates
 * @returns {string | null}
 */
export function resolveBoard(candidates) {
  const rules = loadSniffRules();
  const normalized = candidates.map(normalizeCandidate).filter((c) => c !== "");
  /** @type {Array<[string, number]>} */
  const scores = [];
  for (const board of rules.boards) {
    let best = -1;
    for (const alias of board.aliases) {
      if (alias.length >= rules.min_alias_len && normalized.some((c) => c.includes(alias))) {
        best = Math.max(best, alias.length);
      }
    }
    if (best >= 0) scores.push([board.board_id, best]);
  }
  const top = scores.reduce((max, [, score]) => Math.max(max, score), -1);
  if (top < 0) return null;
  const winners = scores.filter(([, score]) => score === top);
  return winners.length === 1 ? /** @type {string} */ (winners[0]?.[0]) : null;
}

/**
 * @param {string} path
 * @returns {string | null}
 */
function readCapped(path) {
  try {
    const bytes = readFileSync(path);
    return bytes.subarray(0, loadSniffRules().max_file_bytes).toString("utf8");
  } catch {
    return null;
  }
}

/**
 * Harvest board-identifying tokens from a project's build config + sources
 * (mirrors board_sniff collect_platformio/sdkconfig/sources).
 * @param {string} projectDir
 * @returns {string[]}
 */
export function collectProjectCandidates(projectDir) {
  /** @type {string[]} */
  const out = [];
  const pio = readCapped(join(projectDir, "platformio.ini"));
  if (pio) {
    for (const raw of pio.split("\n")) {
      const line = raw.trim();
      const board = line.replace(/^board\s*/, "");
      if (board !== line && board.startsWith("=")) out.push(board.slice(1).trim());
      const env = line.match(/^\[env:(.+)\]$/);
      if (env?.[1]) out.push(env[1]);
    }
  }
  for (const name of ["sdkconfig", "sdkconfig.defaults"]) {
    const text = readCapped(join(projectDir, name));
    if (!text) continue;
    for (const raw of text.split("\n")) {
      const line = raw.trim();
      if (line.startsWith("#")) out.push(line.slice(1).trim());
      const eq = line.indexOf("=");
      if (eq > 0 && line.slice(0, eq).includes("BOARD")) {
        out.push(line.slice(eq + 1).trim().replace(/^"|"$/g, ""));
      }
    }
  }
  const rules = loadSniffRules();
  const files = [...sourceFiles(projectDir), ...sourceFiles(join(projectDir, "src"))].slice(0, rules.max_source_files);
  for (const path of files) {
    const text = readCapped(path);
    if (!text) continue;
    for (const raw of text.split("\n")) {
      const line = raw.trim();
      if (line.startsWith("#include") || line.startsWith("//") || line.startsWith("/*")) out.push(line);
    }
  }
  return out;
}

/**
 * @param {string} dir
 * @returns {string[]}
 */
function sourceFiles(dir) {
  try {
    return readdirSync(dir)
      .filter((name) => /\.(ino|cpp|h)$/.test(name))
      .map((name) => join(dir, name));
  } catch {
    return [];
  }
}

/**
 * Resolve the active board: project-file evidence first (most specific), then a
 * keyword match on the prompt.
 * @param {{ prompt?: string, projectDir?: string }} input
 * @returns {{ board: string | null, source: string | null }}
 */
export function detectBoard({ prompt, projectDir }) {
  if (projectDir) {
    const board = resolveBoard(collectProjectCandidates(projectDir));
    if (board) return { board, source: "inferred-from-project" };
  }
  if (prompt) {
    const board = resolveBoard([prompt]);
    if (board) return { board, source: "keyword" };
  }
  return { board: null, source: null };
}

/**
 * Build the thin pointer capsule (`context` report). Keys are emitted in
 * alphabetical order to match the Rust serde_json snapshot.
 * @param {{ prompt?: string, projectDir?: string }} input
 * @returns {ContextReport}
 */
export function buildContext(input) {
  const { board, source } = detectBoard(input);
  if (!board) {
    return {
      board: null,
      board_source: null,
      context: "no LilyGO board detected in the prompt or project; no context injected.",
      decision: "no-op",
      skills: [],
      verification_level: "none",
    };
  }
  const capsule =
    `LilyGO context injection: board=${board}; verification_level=context-injection; ` +
    `hardware_verified=false; before reporting any pin/bus/peripheral run: ` +
    `lilygo-skills source query --board ${board} --topic <topic> --json ` +
    `(topics: pinout/display/lora/gnss/power/i2c/spi/touch); cite the returned official ` +
    `url+line_range+sha256; do not invent pin numbers; evidence_boundary=V3.`;
  return {
    board,
    board_source: source,
    context: capsule,
    decision: "inject",
    skills: [board],
    verification_level: "context-injection",
  };
}

// --- CLI -------------------------------------------------------------------

/**
 * Parse `context [--project <dir>] [--json] [prompt...]`.
 * @param {string[]} argv
 * @returns {{ projectDir?: string, json: boolean, prompt?: string }}
 */
export function parseContextArgs(argv) {
  const args = argv[0] === "context" ? argv.slice(1) : argv;
  /** @type {{ projectDir?: string, json: boolean, prompt?: string }} */
  const out = { json: false };
  /** @type {string[]} */
  const rest = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--project") out.projectDir = args[++i];
    else if (args[i] === "--json") out.json = true;
    else if (args[i] !== undefined) rest.push(/** @type {string} */ (args[i]));
  }
  if (rest.length > 0) out.prompt = rest.join(" ");
  return out;
}

/**
 * @param {string[]} argv
 * @returns {number} exit code
 */
export function runContext(argv) {
  const { projectDir, json, prompt } = parseContextArgs(argv);
  const report = buildContext({ prompt, projectDir });
  if (json) process.stdout.write(JSON.stringify(report, null, 2) + "\n");
  else process.stdout.write(report.context + "\n");
  return 0;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  process.exit(runContext(process.argv.slice(2)));
}
