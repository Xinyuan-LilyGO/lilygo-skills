// `verify sources --board <id> [--topic <t>] --json`: live re-proof of every
// line-anchored fact. Re-fetches each fact's raw source, recomputes sha256, and
// classifies OK / DRIFT / UNREACHABLE. Offline/rate-limited is graceful
// (UNREACHABLE, never a crash). Mirrors Rust verify_sources.rs.
import { getPack, sha256Hex } from "./lib.mjs";

const FETCH_TIMEOUT_MS = 30_000;
const MAX_CONCURRENCY = 2;
const MAX_ATTEMPTS = 3;

/**
 * A fact is live-verifiable only with the full triple: fetchable http url,
 * recorded line_range, and a sha256 hash.
 * @param {Fact} fact
 * @returns {boolean}
 */
function isVerifiable(fact) {
  const s = fact.source;
  return s.path_or_url.startsWith("http") && typeof s.line_range === "string" && s.hash.startsWith("sha256:");
}

/**
 * github blob URL -> raw.githubusercontent form (raw/non-github pass through).
 * @param {string} url
 * @returns {string}
 */
export function rawFetchUrl(url) {
  const prefix = "https://github.com/";
  if (url.startsWith(prefix)) {
    const rest = url.slice(prefix.length);
    const idx = rest.indexOf("/blob/");
    if (idx >= 0) return `https://raw.githubusercontent.com/${rest.slice(0, idx)}/${rest.slice(idx + "/blob/".length)}`;
  }
  return url;
}

/**
 * Inclusive 1-based line slice (mirrors verify_sources::slice_range); null on a
 * malformed range.
 * @param {string} text
 * @param {string} range
 * @returns {string | null}
 */
function sliceRange(text, range) {
  const dash = range.indexOf("-");
  if (dash < 0) return null;
  const start = Number.parseInt(range.slice(0, dash).trim(), 10);
  const end = Number.parseInt(range.slice(dash + 1).trim(), 10);
  if (!Number.isInteger(start) || !Number.isInteger(end) || start === 0 || end < start) return null;
  const lines = text.split("\n");
  const hi = Math.min(end, lines.length);
  return lines.slice(start - 1, hi).join("\n");
}

/**
 * Fetch raw text with a timeout, optional GITHUB_TOKEN, and backoff retries on
 * transient failures. Returns the body or throws the final error.
 * @param {string} url
 * @returns {Promise<string>}
 */
async function fetchText(url) {
  /** @type {Record<string, string>} */
  const headers = {};
  const token = process.env["GITHUB_TOKEN"];
  if (token && url.includes("githubusercontent.com")) headers["Authorization"] = `Bearer ${token}`;
  let lastError = new Error("unreachable");
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
    try {
      const response = await fetch(url, { headers, signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) });
      if (response.ok) return await response.text();
      // Rate-limit / server errors are retryable; hard 4xx are not.
      if (response.status === 429 || response.status >= 500) {
        lastError = new Error(`http ${response.status}`);
      } else {
        throw new Error(`http ${response.status}`);
      }
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
    }
    if (attempt < MAX_ATTEMPTS) await sleep(400 * attempt);
  }
  throw lastError;
}

/**
 * @param {number} ms
 * @returns {Promise<void>}
 */
function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * @param {Fact} fact
 * @param {{ ok: true, text: string } | { ok: false, error: string }} fetched
 * @returns {VerifyFact}
 */
function classify(fact, fetched) {
  const s = fact.source;
  const hasRange = typeof s.line_range === "string";
  let verdict;
  /** @type {string | undefined} */
  let liveHash;
  /** @type {string | undefined} */
  let detail;
  if (fetched.ok) {
    liveHash = sha256Hex(fetched.text);
    if (liveHash === s.hash) {
      verdict = "OK";
      if (hasRange && sliceRange(fetched.text, /** @type {string} */ (s.line_range)) !== null) {
        detail = "line_range re-sliced";
      }
    } else {
      verdict = "DRIFT";
      detail = "fetched file hash differs from stored hash";
    }
  } else {
    verdict = "UNREACHABLE";
    detail = fetched.error;
  }
  return {
    key: fact.key,
    topic: fact.topic,
    fetch_url: rawFetchUrl(s.path_or_url),
    ...(hasRange ? { line_range: s.line_range } : {}),
    stored_hash: s.hash,
    ...(liveHash !== undefined ? { live_hash: liveHash } : {}),
    verdict,
    ...(detail !== undefined ? { detail } : {}),
  };
}

/**
 * Run tasks with bounded concurrency, preserving input order in the results.
 * @template T
 * @param {Array<() => Promise<T>>} tasks
 * @param {number} limit
 * @returns {Promise<T[]>}
 */
async function pooled(tasks, limit) {
  /** @type {T[]} */
  const results = new Array(tasks.length);
  let next = 0;
  async function worker() {
    while (next < tasks.length) {
      const i = next++;
      results[i] = await /** @type {() => Promise<T>} */ (tasks[i])();
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, tasks.length) }, worker));
  return results;
}

/**
 * @param {string} boardId
 * @param {string | undefined} topic
 * @returns {Promise<VerifyReport>}
 */
export async function verifySources(boardId, topic) {
  const pack = getPack(boardId);
  if (!pack) throw new Error(`unknown board fact pack: ${boardId}`);
  const candidates = [...pack.pin_matrix, ...pack.bus_matrix]
    .filter(isVerifiable)
    .filter((fact) => topic === undefined || fact.topic === topic);
  const facts = await pooled(
    candidates.map((fact) => async () => {
      const url = rawFetchUrl(fact.source.path_or_url);
      try {
        return classify(fact, { ok: true, text: await fetchText(url) });
      } catch (error) {
        return classify(fact, { ok: false, error: error instanceof Error ? error.message : String(error) });
      }
    }),
    MAX_CONCURRENCY,
  );
  const counts = { total: facts.length, ok: 0, drift: 0, unreachable: 0 };
  for (const fact of facts) {
    if (fact.verdict === "OK") counts.ok++;
    else if (fact.verdict === "DRIFT") counts.drift++;
    else counts.unreachable++;
  }
  return {
    status: counts.drift > 0 ? "DRIFT" : "PASS",
    board_id: boardId,
    ...(topic !== undefined ? { topic } : {}),
    counts,
    facts,
  };
}

// --- CLI -------------------------------------------------------------------

/**
 * @param {string[]} argv
 * @returns {Promise<number>} exit code
 */
export async function runVerify(argv) {
  const args = argv[0] === "verify" && argv[1] === "sources" ? argv.slice(2)
    : argv[0] === "sources" ? argv.slice(1) : argv;
  if (!args.includes("--json")) {
    process.stderr.write("--json is required for this command\n");
    return 2;
  }
  let board;
  let topic;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--board") board = args[++i];
    else if (args[i] === "--topic") topic = args[++i];
  }
  if (!board) {
    process.stderr.write("--board <board-id> is required\n");
    return 2;
  }
  let report;
  try {
    report = await verifySources(board, topic);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return 2;
  }
  process.stdout.write(JSON.stringify(report, null, 2) + "\n");
  // DRIFT is a real integrity signal -> non-zero; UNREACHABLE stays PASS.
  return report.status === "PASS" ? 0 : 2;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  runVerify(process.argv.slice(2)).then((code) => process.exit(code));
}
