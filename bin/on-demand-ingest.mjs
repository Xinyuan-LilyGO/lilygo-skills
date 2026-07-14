// Gate-guarded cache-miss pin ingestion from official LilyGO repositories.
// Parsed numbers remain private until a single conventional source passes the
// provenance, authority-conflict, and naming-convention gates. Every failure
// returns a pinless degraded verdict.
import { execFile as execFileCallback } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { promisify } from "node:util";

import { getBoardRegistry, m38CacheRoot, productKey } from "./board-registry.mjs";
import { sha256Hex, slug } from "./lib.mjs";

const require = createRequire(import.meta.url);
/** @type {{ autoMapPins(macros: Record<string, string>): MappedPin[] }} */
const { autoMapPins } = require("../pipeline/auto-map-pins.js");
/** @type {{ firstDefineMap(text: string): Record<string, string>; allDefineValues(text: string): Record<string, Set<string>> }} */
const { firstDefineMap, allDefineValues } = require("../pipeline/extract-defines.js");

const execFile = promisify(execFileCallback);
const INGEST_SCHEMA_VERSION = 1;
const DEFAULT_TIMEOUT_MS = 30_000;
const MAX_GH_OUTPUT_BYTES = 20 * 1024 * 1024;
const MAX_TREE_ENTRIES = 30_000;
const MAX_CANDIDATES = 24;
const MAX_SOURCE_BYTES = 512 * 1024;
const SHA256_RE = /^sha256:[0-9a-f]{64}$/;

/** @typedef {{ key: string; macro: string; value_num: string }} MappedPin */
/**
 * @typedef {object} RepositorySourceFile
 * @property {string} path
 * @property {string} blob_sha
 * @property {string} text
 */
/**
 * @typedef {object} RepositorySnapshot
 * @property {string} commit
 * @property {RepositorySourceFile[]} files
 * @property {string=} discovery_error
 */
/**
 * @typedef {object} IngestSourceState
 * @property {string} repo_url
 * @property {string} path
 * @property {string} commit
 * @property {string} blob_sha
 * @property {string} sha256
 * @property {string} line_range
 * @property {string} url
 * @property {string=} etag
 */
/**
 * @typedef {object} VerifiedIngestVerdict
 * @property {number} schema_version
 * @property {"verified"} status
 * @property {string} board_id
 * @property {string} requested_board
 * @property {string} product_name
 * @property {string} repo_url
 * @property {string} ingested_at
 * @property {string} last_checked
 * @property {IngestSourceState} source
 * @property {{ provenance: "PASS"; source_authority: "PASS"; auto_map_scope: "PASS"; mapped_pins: number; logical_sources: 1 }} gates
 * @property {FactPack} fact_pack
 * @property {"live" | "cache"} cache_status
 */
/**
 * @typedef {object} DegradedIngestVerdict
 * @property {number} schema_version
 * @property {"degraded"} status
 * @property {string} board_id
 * @property {string} requested_board
 * @property {string} product_name
 * @property {string | null} repo_url
 * @property {string} reason
 * @property {string} message
 * @property {string} ingested_at
 * @property {string} last_checked
 * @property {string[]} candidate_paths
 * @property {"live" | "cache"} cache_status
 */
/** @typedef {VerifiedIngestVerdict | DegradedIngestVerdict} IngestVerdict */
/**
 * @typedef {object} OnDemandOptions
 * @property {string=} cacheDir
 * @property {boolean=} enabled
 * @property {Date=} now
 * @property {import("./board-registry.mjs").BoardRegistry=} registry
 * @property {() => Promise<import("./board-registry.mjs").BoardRegistry>=} registryLoader
 * @property {(board: import("./board-registry.mjs").BoardRegistryEntry) => Promise<RepositorySnapshot>=} repositoryFetcher
 * @property {number=} timeoutMs
 * @property {boolean=} forceRefresh
 */

/**
 * Resolve only exact normalized identities. An ambiguous key is not resolved
 * even when one row appears more familiar to the caller.
 * @param {import("./board-registry.mjs").BoardRegistry} registry
 * @param {string} requested
 * @returns {{ status: "resolved"; board: import("./board-registry.mjs").BoardRegistryEntry } | { status: "missing" | "ambiguous" }}
 */
export function resolveRegistryBoard(registry, requested) {
  const requestedKey = productKey(requested);
  const matches = registry.boards.filter((board) => {
    const identities = [board.id, board.product_name, board.repository_name || "", ...board.aliases];
    return identities.some((identity) => productKey(identity) === requestedKey);
  });
  if (matches.length === 0) return { status: "missing" };
  if (matches.length !== 1) return { status: "ambiguous" };
  const board = matches[0];
  return board ? { status: "resolved", board } : { status: "missing" };
}

/**
 * Main cache-miss path. Cached verdicts are returned before any network work;
 * both verified and degraded outcomes are stable until freshness rechecks them.
 * @param {string} requestedBoard
 * @param {OnDemandOptions} [options]
 * @returns {Promise<IngestVerdict>}
 */
export async function ensureOnDemandPinout(requestedBoard, options = {}) {
  const cacheDir = options.cacheDir ?? m38CacheRoot();
  const cached = !options.forceRefresh ? await findCachedVerdict(cacheDir, requestedBoard) : undefined;
  if (cached) return { ...cached, cache_status: "cache" };

  const now = options.now ?? new Date();
  const enabled = options.enabled ?? process.env.LILYGO_SKILLS_ON_DEMAND !== "0";
  if (!enabled) {
    return degradedVerdict({
      requestedBoard,
      boardId: requestedBoard,
      productName: requestedBoard,
      repoUrl: null,
      reason: "on-demand-disabled",
      candidatePaths: [],
      now,
    });
  }

  const registry = options.registry ?? await (options.registryLoader
    ? options.registryLoader()
    : getBoardRegistry({ cacheDir }));
  const resolution = resolveRegistryBoard(registry, requestedBoard);
  if (resolution.status !== "resolved") {
    return degradedVerdict({
      requestedBoard,
      boardId: requestedBoard,
      productName: requestedBoard,
      repoUrl: null,
      reason: resolution.status === "ambiguous" ? "ambiguous-board" : "not-covered",
      candidatePaths: [],
      now,
    });
  }

  const board = resolution.board;
  const boardId = board.id;
  const resolvedCache = !options.forceRefresh ? await readIngestVerdict(cacheDir, boardId) : undefined;
  if (resolvedCache) return { ...resolvedCache, cache_status: "cache" };
  if (!board.official_repo || board.repo_candidates.length !== 1) {
    const verdict = degradedVerdict({
      requestedBoard,
      boardId,
      productName: board.product_name,
      repoUrl: null,
      reason: "ambiguous-board",
      candidatePaths: [],
      now,
    });
    await writeIngestVerdict(cacheDir, verdict);
    return verdict;
  }

  /** @type {RepositorySnapshot} */
  let snapshot;
  try {
    snapshot = await (options.repositoryFetcher
      ? options.repositoryFetcher(board)
      : fetchRepositorySnapshot(board, { timeoutMs: options.timeoutMs }));
  } catch {
    const verdict = degradedVerdict({
      requestedBoard,
      boardId,
      productName: board.product_name,
      repoUrl: board.official_repo,
      reason: "offline",
      candidatePaths: [],
      now,
    });
    await writeIngestVerdict(cacheDir, verdict);
    return verdict;
  }

  const evaluated = evaluateRepositorySnapshot(board, requestedBoard, snapshot, now);
  await writeIngestVerdict(cacheDir, evaluated);
  return evaluated;
}

/**
 * Apply all acceptance gates to already-fetched official source candidates.
 * This pure function is also the boundary used by fixture tests and warm-up.
 * @param {import("./board-registry.mjs").BoardRegistryEntry} board
 * @param {string} requestedBoard
 * @param {RepositorySnapshot} snapshot
 * @param {Date} [now]
 * @returns {IngestVerdict}
 */
export function evaluateRepositorySnapshot(board, requestedBoard, snapshot, now = new Date()) {
  if (!board.official_repo || board.repo_candidates.length !== 1) {
    return degradedVerdict({
      requestedBoard,
      boardId: board.id,
      productName: board.product_name,
      repoUrl: null,
      reason: "ambiguous-board",
      candidatePaths: [],
      now,
    });
  }
  if (snapshot.discovery_error) {
    return degradedVerdict({
      requestedBoard,
      boardId: board.id,
      productName: board.product_name,
      repoUrl: board.official_repo,
      reason: snapshot.discovery_error,
      candidatePaths: snapshot.files.map((file) => file.path),
      now,
    });
  }
  if (snapshot.files.length === 0) {
    return degradedVerdict({
      requestedBoard,
      boardId: board.id,
      productName: board.product_name,
      repoUrl: board.official_repo,
      reason: "no-source",
      candidatePaths: [],
      now,
    });
  }

  const analyzed = snapshot.files.map(analyzeCandidate);
  const conflicting = analyzed.filter((candidate) => candidate.mapped.length >= 2 && candidate.conflicts.length > 0);
  if (conflicting.length > 0) {
    return degradedVerdict({
      requestedBoard,
      boardId: board.id,
      productName: board.product_name,
      repoUrl: board.official_repo,
      reason: "source-conflict",
      candidatePaths: conflicting.map((candidate) => candidate.path),
      now,
    });
  }
  const viable = analyzed.filter((candidate) => candidate.mapped.length >= 2 && candidate.conflicts.length === 0);
  if (viable.length === 0) {
    return degradedVerdict({
      requestedBoard,
      boardId: board.id,
      productName: board.product_name,
      repoUrl: board.official_repo,
      reason: "insufficient-auto-map",
      candidatePaths: snapshot.files.map((file) => file.path),
      now,
    });
  }

  /** @type {Map<string, AnalyzedCandidate[]>} */
  const logicalMaps = new Map();
  for (const candidate of viable) {
    const group = logicalMaps.get(candidate.signature) ?? [];
    group.push(candidate);
    logicalMaps.set(candidate.signature, group);
  }
  if (logicalMaps.size !== 1) {
    return degradedVerdict({
      requestedBoard,
      boardId: board.id,
      productName: board.product_name,
      repoUrl: board.official_repo,
      reason: "multiple-sources",
      candidatePaths: viable.map((candidate) => candidate.path),
      now,
    });
  }

  const equivalent = [...logicalMaps.values()][0] ?? [];
  const selected = equivalent.sort((a, b) => candidateScore(b.path, board) - candidateScore(a.path, board) || a.path.localeCompare(b.path))[0];
  if (!selected) {
    return degradedVerdict({
      requestedBoard,
      boardId: board.id,
      productName: board.product_name,
      repoUrl: board.official_repo,
      reason: "no-source",
      candidatePaths: [],
      now,
    });
  }

  const sourceHash = sha256Hex(selected.text);
  const lineRange = mappedLineRange(selected.text, selected.mapped.map((pin) => pin.macro));
  const sourceUrl = commitBlobUrl(board.official_repo, snapshot.commit, selected.path);
  /** @type {SourceRef} */
  const source = {
    kind: "arduino-pins",
    path_or_url: sourceUrl,
    line_range: lineRange,
    hash: sourceHash,
  };
  const pinMatrix = selected.mapped.map((pin) => makePinFact(board.id, pin, source, selected.path));
  /** @type {FactPack} */
  const factPack = {
    schema_version: 1,
    board_id: board.id,
    mcu_family: inferMcuFamily(board),
    supported: true,
    pin_matrix: pinMatrix,
    bus_matrix: [],
    expander_matrix: [],
    connector_matrix: [],
    peripheral_table: [],
    source_refs: [source],
    conflicts: [],
  };
  if (!provenanceGatePasses(factPack, board.official_repo)) {
    return degradedVerdict({
      requestedBoard,
      boardId: board.id,
      productName: board.product_name,
      repoUrl: board.official_repo,
      reason: "provenance-failed",
      candidatePaths: [selected.path],
      now,
    });
  }

  const timestamp = now.toISOString();
  return {
    schema_version: INGEST_SCHEMA_VERSION,
    status: "verified",
    board_id: board.id,
    requested_board: requestedBoard,
    product_name: board.product_name,
    repo_url: board.official_repo,
    ingested_at: timestamp,
    last_checked: timestamp,
    source: {
      repo_url: board.official_repo,
      path: selected.path,
      commit: snapshot.commit,
      blob_sha: selected.blob_sha,
      sha256: sourceHash,
      line_range: lineRange,
      url: sourceUrl,
    },
    gates: {
      provenance: "PASS",
      source_authority: "PASS",
      auto_map_scope: "PASS",
      mapped_pins: selected.mapped.length,
      logical_sources: 1,
    },
    fact_pack: factPack,
    cache_status: "live",
  };
}

/**
 * Fetch a default-branch commit, its recursive tree, and only the bounded set
 * of conventional source candidates through authenticated `gh api` calls.
 * @param {import("./board-registry.mjs").BoardRegistryEntry} board
 * @param {{ timeoutMs?: number }} [options]
 * @returns {Promise<RepositorySnapshot>}
 */
export async function fetchRepositorySnapshot(board, options = {}) {
  if (!board.official_repo || !board.default_branch) throw new Error("board has no resolved official repository");
  const repo = parseOfficialRepo(board.official_repo);
  const deadline = Date.now() + (options.timeoutMs ?? DEFAULT_TIMEOUT_MS);
  const commitResponse = await ghJson(
    `repos/${repo.owner}/${repo.name}/commits/${encodeURIComponent(board.default_branch)}`,
    deadline,
  );
  const commit = isRecord(commitResponse) && typeof commitResponse.sha === "string" ? commitResponse.sha : "";
  if (!/^[0-9a-f]{40,64}$/i.test(commit)) throw new Error("official repository returned no commit sha");
  const treeResponse = await ghJson(`repos/${repo.owner}/${repo.name}/git/trees/${commit}?recursive=1`, deadline);
  if (!isRecord(treeResponse) || !Array.isArray(treeResponse.tree)) throw new Error("official repository returned no tree");
  if (treeResponse.truncated === true || treeResponse.tree.length > MAX_TREE_ENTRIES) {
    return { commit, files: [], discovery_error: "tree-too-large" };
  }
  const candidateEntries = treeResponse.tree
    .flatMap((entry) => {
      if (!isRecord(entry) || entry.type !== "blob") return [];
      const path = typeof entry.path === "string" ? entry.path : "";
      const blobSha = typeof entry.sha === "string" ? entry.sha : "";
      const size = typeof entry.size === "number" ? entry.size : 0;
      if (!path || !blobSha || size > MAX_SOURCE_BYTES || !isConventionalCandidate(path, board)) return [];
      return [{ path, blob_sha: blobSha }];
    })
    .sort((a, b) => candidateScore(b.path, board) - candidateScore(a.path, board) || a.path.localeCompare(b.path));
  if (candidateEntries.length > MAX_CANDIDATES) {
    return {
      commit,
      files: candidateEntries.slice(0, MAX_CANDIDATES).map((entry) => ({ ...entry, text: "" })),
      discovery_error: "candidate-cap",
    };
  }

  /** @type {RepositorySourceFile[]} */
  const files = [];
  for (const entry of candidateEntries) {
    const blob = await ghJson(`repos/${repo.owner}/${repo.name}/git/blobs/${entry.blob_sha}`, deadline);
    if (!isRecord(blob) || blob.encoding !== "base64" || typeof blob.content !== "string") continue;
    const text = Buffer.from(blob.content.replace(/\n/g, ""), "base64").toString("utf8");
    if (Buffer.byteLength(text, "utf8") > MAX_SOURCE_BYTES) continue;
    files.push({ path: entry.path, blob_sha: entry.blob_sha, text });
  }
  return { commit, files };
}

/** @param {string} cacheDir @param {string} boardId @returns {string} */
export function ingestCacheFile(cacheDir, boardId) {
  return join(cacheDir, "ingest", `${slug(boardId)}.json`);
}

/** @param {string} cacheDir @param {string} boardId @returns {Promise<IngestVerdict | undefined>} */
export async function readIngestVerdict(cacheDir, boardId) {
  try {
    const parsed = JSON.parse(await readFile(ingestCacheFile(cacheDir, boardId), "utf8"));
    if (!isIngestVerdict(parsed) || parsed.board_id !== boardId) return undefined;
    if (parsed.status === "verified" && !provenanceGatePasses(parsed.fact_pack, parsed.repo_url)) return undefined;
    return parsed;
  } catch {
    return undefined;
  }
}

/** @param {string} cacheDir @param {IngestVerdict} verdict @returns {Promise<void>} */
export async function writeIngestVerdict(cacheDir, verdict) {
  const file = ingestCacheFile(cacheDir, verdict.board_id);
  await mkdir(dirname(file), { recursive: true });
  const temporary = `${file}.${process.pid}.${Date.now()}.tmp`;
  await writeFile(temporary, `${JSON.stringify(verdict, null, 2)}\n`, "utf8");
  await rename(temporary, file);
}

/**
 * Offline-equivalent provenance gate for a dynamic pack.
 * @param {FactPack} pack
 * @param {string} repoUrl
 * @returns {boolean}
 */
export function provenanceGatePasses(pack, repoUrl) {
  if (pack.pin_matrix.length < 2 || pack.source_refs.length !== 1) return false;
  return pack.pin_matrix.every((fact) => {
    const source = fact.source;
    return (
      fact.board_id === pack.board_id &&
      fact.evidence_level === "V3-source-reference" &&
      source.path_or_url.startsWith(`${repoUrl}/blob/`) &&
      typeof source.line_range === "string" && /^\d+-\d+$/.test(source.line_range) &&
      SHA256_RE.test(source.hash)
    );
  });
}

/**
 * @typedef {object} AnalyzedCandidate
 * @property {string} path
 * @property {string} blob_sha
 * @property {string} text
 * @property {MappedPin[]} mapped
 * @property {string[]} conflicts
 * @property {string} signature
 */
/** @param {RepositorySourceFile} file @returns {AnalyzedCandidate} */
function analyzeCandidate(file) {
  const macros = firstDefineMap(file.text);
  const mapped = autoMapPins(macros);
  const definitions = /** @type {Record<string, Set<string>>} */ (allDefineValues(file.text));
  const conflicts = mapped
    .filter((pin) => (definitions[pin.macro]?.size ?? 0) > 1)
    .map((pin) => pin.macro);
  const signature = mapped
    .map((pin) => `${pin.key}=${pin.value_num}`)
    .sort()
    .join("|");
  return { ...file, mapped, conflicts, signature };
}

/** @param {string} path @param {import("./board-registry.mjs").BoardRegistryEntry} board @returns {boolean} */
function isConventionalCandidate(path, board) {
  const lower = path.toLowerCase();
  if (/(?:^|\/)(?:node_modules|\.git|build|dist)\//.test(lower)) return false;
  const base = lower.slice(lower.lastIndexOf("/") + 1);
  if (/^setup.*\.h$/.test(base)) {
    return /tft_espi|tft-espi/.test(lower) && setupMatchesBoard(base, board);
  }
  if (/^factory.*\.ino$/.test(base)) return true;
  if (/^(?:pins_config|pin_config|pins_arduino|utilities)\.h$/.test(base)) {
    if (/(?:^|\/)(?:lib|libdeps)\//.test(lower) && !/private_library/.test(lower)) return false;
    return true;
  }
  return /_pins\.h$/.test(base);
}

/** @param {string} base @param {import("./board-registry.mjs").BoardRegistryEntry} board @returns {boolean} */
function setupMatchesBoard(base, board) {
  const setupKey = productKey(base.replace(/^setup\d*[-_]?/i, "").replace(/\.h$/i, ""));
  const boardKeys = [board.id, board.product_name, board.repository_name || "", ...board.aliases]
    .map(productKey)
    .filter((key) => key.length >= 4);
  return boardKeys.some((key) => setupKey.includes(key) || key.includes(setupKey));
}

/** @param {string} path @param {import("./board-registry.mjs").BoardRegistryEntry} board @returns {number} */
function candidateScore(path, board) {
  const lower = path.toLowerCase();
  const base = lower.slice(lower.lastIndexOf("/") + 1);
  let score = 0;
  if (/^(?:pins_config|pin_config|pins_arduino)\.h$/.test(base)) score += 80;
  else if (/^utilities\.h$/.test(base)) score += 70;
  else if (/^setup.*\.h$/.test(base)) score += 60;
  else if (/^factory.*\.ino$/.test(base)) score += 50;
  else if (/_pins\.h$/.test(base)) score += 40;
  if (/private_library/.test(lower)) score += 20;
  if (lower.startsWith("src/")) score += 15;
  if (lower.includes("factory")) score += 10;
  const key = productKey(board.repository_name || board.product_name);
  if (productKey(path).includes(key)) score += 10;
  return score - Math.min(path.split("/").length, 20);
}

/** @param {string} text @param {string[]} macros @returns {string} */
function mappedLineRange(text, macros) {
  const wanted = new Set(macros);
  const lines = text.split("\n");
  const numbers = lines.flatMap((line, index) => {
    const define = line.match(/^[ \t]*#define[ \t]+([A-Z0-9_]+)/);
    const constant = line.match(/^[ \t]*static[ \t]+const[ \t]+u?int\d+_t[ \t]+([A-Z0-9_]+)/);
    const macro = define?.[1] || constant?.[1];
    return macro && wanted.has(macro) ? [index + 1] : [];
  });
  const start = Math.min(...numbers);
  const end = Math.max(...numbers);
  if (!Number.isFinite(start) || !Number.isFinite(end)) throw new Error("mapped pins have no source line anchors");
  return `${start}-${end}`;
}

/** @param {string} repoUrl @param {string} commit @param {string} path @returns {string} */
function commitBlobUrl(repoUrl, commit, path) {
  const encodedPath = path.split("/").map(encodeURIComponent).join("/");
  return `${repoUrl}/blob/${commit}/${encodedPath}`;
}

/** @param {string} boardId @param {MappedPin} pin @param {SourceRef} source @param {string} path @returns {Fact} */
function makePinFact(boardId, pin, source, path) {
  const sourceName = path.slice(path.lastIndexOf("/") + 1);
  return {
    schema_version: 1,
    board_id: boardId,
    topic: "pinout",
    key: pin.key,
    value: `${pin.macro}=GPIO${pin.value_num}`,
    claim: `${pin.key} auto-mapped from ${pin.macro} in official ${sourceName}`,
    source,
    authority_rank: 95,
    evidence_level: "V3-source-reference",
    stale: false,
    confidence: "exact",
  };
}

/** @param {import("./board-registry.mjs").BoardRegistryEntry} board @returns {string} */
function inferMcuFamily(board) {
  const tag = board.tags.find((value) => /^(?:esp32|nrf|rp\d|stm32|k\d)/i.test(value));
  return tag ? slug(tag) : "unknown";
}

/**
 * @param {{ requestedBoard: string; boardId: string; productName: string; repoUrl: string | null; reason: string; candidatePaths: string[]; now: Date }} input
 * @returns {DegradedIngestVerdict}
 */
function degradedVerdict(input) {
  const timestamp = input.now.toISOString();
  const message = input.repoUrl
    ? `board found at ${input.repoUrl}, but no verifiable pinout could be obtained`
    : input.reason === "on-demand-disabled"
      ? "on-demand ingest is disabled; board is not covered locally"
      : "board is not covered by one unambiguous official repository";
  return {
    schema_version: INGEST_SCHEMA_VERSION,
    status: "degraded",
    board_id: input.boardId,
    requested_board: input.requestedBoard,
    product_name: input.productName,
    repo_url: input.repoUrl,
    reason: input.reason,
    message,
    ingested_at: timestamp,
    last_checked: timestamp,
    candidate_paths: [...new Set(input.candidatePaths)].sort(),
    cache_status: "live",
  };
}

/** @param {string} cacheDir @param {string} requestedBoard @returns {Promise<IngestVerdict | undefined>} */
async function findCachedVerdict(cacheDir, requestedBoard) {
  return readIngestVerdict(cacheDir, requestedBoard)
    ?? readIngestVerdict(cacheDir, `board-${productKey(requestedBoard)}`);
}

/** @param {unknown} value @returns {value is IngestVerdict} */
function isIngestVerdict(value) {
  if (!isRecord(value) || value.schema_version !== INGEST_SCHEMA_VERSION) return false;
  if (value.status !== "verified" && value.status !== "degraded") return false;
  return typeof value.board_id === "string" && typeof value.last_checked === "string";
}

/** @param {string} url @returns {{ owner: string; name: string }} */
function parseOfficialRepo(url) {
  const parsed = new URL(url);
  const parts = parsed.pathname.split("/").filter(Boolean);
  if (parsed.hostname !== "github.com" || parts.length !== 2 || parts[0]?.toLowerCase() !== "xinyuan-lilygo") {
    throw new Error("repository is not an official Xinyuan-LilyGO GitHub repository");
  }
  return { owner: /** @type {string} */ (parts[0]), name: /** @type {string} */ (parts[1]) };
}

/** @param {string} endpoint @param {number} deadline @returns {Promise<unknown>} */
async function ghJson(endpoint, deadline) {
  const remaining = deadline - Date.now();
  if (remaining <= 0) throw new Error("official repository crawl timed out");
  const result = await execFile("gh", ["api", endpoint], {
    encoding: "utf8",
    timeout: Math.max(250, remaining),
    maxBuffer: MAX_GH_OUTPUT_BYTES,
  });
  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    throw new Error(`parse gh api response failed: ${error instanceof Error ? error.message : String(error)}`);
  }
}

/** @param {unknown} value @returns {value is Record<string, unknown>} */
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
