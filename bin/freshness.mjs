// Throttled incremental freshness for the dynamic registry and source cache.
// Unchanged repositories cost no source fetch. A changed source is rechecked
// and then sent through the same gated ingest path; failed re-ingest blocks the
// affected board instead of allowing stale pin values to remain authoritative.
import { dirname, join } from "node:path";
import { readdir, readFile, rename, writeFile, mkdir } from "node:fs/promises";

import { getBoardRegistry, m38CacheRoot } from "./board-registry.mjs";
import {
  ensureOnDemandPinout,
  readIngestVerdict,
} from "./on-demand-ingest.mjs";
import { loadFactPacks, sha256Hex } from "./lib.mjs";
import { fetchSource, rawFetchUrl } from "./verify.mjs";

const FRESHNESS_SCHEMA_VERSION = 1;
const DEFAULT_INTERVAL_MS = 24 * 60 * 60 * 1_000;

/**
 * @typedef {object} FreshnessSource
 * @property {string} id
 * @property {"cached" | "committed"} kind
 * @property {string} board_id
 * @property {string} repo_url
 * @property {string} source_url
 * @property {string} path
 * @property {string | null} commit
 * @property {string} sha256
 * @property {string | null} etag
 * @property {string | null} change_token
 */
/**
 * @typedef {object} FreshnessSourceState
 * @property {"cached" | "committed"} kind
 * @property {string} board_id
 * @property {string} repo_url
 * @property {string} source_url
 * @property {string} path
 * @property {string | null} commit
 * @property {string} sha256
 * @property {string | null} etag
 * @property {string | null} change_token
 * @property {string} last_checked
 * @property {"OK" | "DRIFT" | "DEGRADED"} verdict
 */
/**
 * @typedef {object} FreshnessState
 * @property {number} schema_version
 * @property {string} last_checked
 * @property {number} interval_ms
 * @property {number} registry_board_count
 * @property {Record<string, FreshnessSourceState>} sources
 * @property {Record<string, { repo_url: string; reason: string; last_checked: string }>} blocked_boards
 * @property {Record<string, { cache_board_id: string; last_checked: string }>} overrides
 */
/**
 * @typedef {object} SourceCheck
 * @property {"unchanged" | "changed" | "offline"} status
 * @property {string | null} commit
 * @property {string} sha256
 * @property {string | null} etag
 */
/**
 * @typedef {object} FreshnessReport
 * @property {"PASS" | "THROTTLED" | "OFFLINE" | "DISABLED"} status
 * @property {number} checked_sources
 * @property {number} changed_sources
 * @property {number} reingested_sources
 * @property {number} new_boards
 * @property {string[]} blocked_boards
 * @property {string[]} overrides
 * @property {string=} last_checked
 */
/**
 * @typedef {object} FreshnessOptions
 * @property {string=} cacheDir
 * @property {Date=} now
 * @property {number=} intervalMs
 * @property {boolean=} networkEnabled
 * @property {() => Promise<import("./board-registry.mjs").BoardRegistry>=} registryRefresher
 * @property {(registry: import("./board-registry.mjs").BoardRegistry) => Promise<FreshnessSource[]>=} sourceLister
 * @property {(source: FreshnessSource, previous: FreshnessSourceState) => Promise<SourceCheck>=} sourceChecker
 * @property {(source: FreshnessSource, registry: import("./board-registry.mjs").BoardRegistry) => Promise<import("./on-demand-ingest.mjs").IngestVerdict>=} reingestSource
 */

/** @param {string} cacheDir @returns {string} */
export function freshnessStateFile(cacheDir) {
  return join(cacheDir, "freshness-state.json");
}

/**
 * Run at most once per interval. A failed/offline attempt does not advance the
 * throttle timestamp, so a later online call can retry.
 * @param {FreshnessOptions} [options]
 * @returns {Promise<FreshnessReport>}
 */
export async function runDailyFreshness(options = {}) {
  const cacheDir = options.cacheDir ?? m38CacheRoot();
  const now = options.now ?? new Date();
  const intervalMs = options.intervalMs ?? configuredIntervalMs();
  const networkEnabled = options.networkEnabled ?? process.env.LILYGO_SKILLS_FRESHNESS !== "0";
  const previous = await readFreshnessState(cacheDir);
  if (!networkEnabled) return reportFromState("DISABLED", previous);
  if (previous && stateAgeMs(previous, now) < intervalMs) return reportFromState("THROTTLED", previous);

  /** @type {import("./board-registry.mjs").BoardRegistry} */
  let registry;
  try {
    registry = await (options.registryRefresher
      ? options.registryRefresher()
      : getBoardRegistry({ cacheDir, forceRefresh: true }));
  } catch {
    return reportFromState("OFFLINE", previous);
  }
  if (registry.status === "OFFLINE") return reportFromState("OFFLINE", previous);

  /** @type {FreshnessSource[]} */
  let sources;
  try {
    sources = await (options.sourceLister
      ? options.sourceLister(registry)
      : listTrackedSources(cacheDir, registry));
  } catch {
    return reportFromState("OFFLINE", previous);
  }

  const timestamp = now.toISOString();
  /** @type {Record<string, FreshnessSourceState>} */
  const nextSources = {};
  const blockedBoards = { ...(previous?.blocked_boards ?? {}) };
  const overrides = { ...(previous?.overrides ?? {}) };
  let checkedSources = 0;
  let changedSources = 0;
  let reingestedSources = 0;

  for (const source of sources) {
    const old = previous?.sources[source.id];
    if (!old) {
      nextSources[source.id] = sourceState(source, timestamp, "OK");
      continue;
    }
    if (source.sha256 !== old.sha256 && source.kind === "cached") {
      nextSources[source.id] = sourceState(source, timestamp, "OK");
      continue;
    }
    if (source.change_token === old.change_token) {
      nextSources[source.id] = { ...old, last_checked: timestamp };
      continue;
    }

    checkedSources += 1;
    /** @type {SourceCheck} */
    let check;
    try {
      check = await (options.sourceChecker
        ? options.sourceChecker(source, old)
        : checkFreshnessSource(source));
    } catch {
      return reportFromState("OFFLINE", previous);
    }
    if (check.status === "offline") return reportFromState("OFFLINE", previous);
    if (check.status === "unchanged") {
      nextSources[source.id] = {
        ...sourceState(source, timestamp, "OK"),
        commit: check.commit,
        sha256: check.sha256,
        etag: check.etag,
      };
      delete blockedBoards[source.board_id];
      continue;
    }

    changedSources += 1;
    const verdict = await (options.reingestSource
      ? options.reingestSource(source, registry)
      : reingestChangedSource(cacheDir, source, registry));
    reingestedSources += 1;
    if (verdict.status === "verified") {
      nextSources[source.id] = {
        ...sourceState(source, timestamp, "OK"),
        commit: verdict.source.commit,
        sha256: verdict.source.sha256,
        etag: verdict.source.etag ?? check.etag,
        source_url: verdict.source.url,
        path: verdict.source.path,
      };
      overrides[source.board_id] = { cache_board_id: verdict.board_id, last_checked: timestamp };
      delete blockedBoards[source.board_id];
    } else {
      nextSources[source.id] = {
        ...sourceState(source, timestamp, "DEGRADED"),
        commit: check.commit,
        sha256: check.sha256,
        etag: check.etag,
      };
      blockedBoards[source.board_id] = {
        repo_url: source.repo_url,
        reason: verdict.reason,
        last_checked: timestamp,
      };
      delete overrides[source.board_id];
    }
  }

  /** @type {FreshnessState} */
  const next = {
    schema_version: FRESHNESS_SCHEMA_VERSION,
    last_checked: timestamp,
    interval_ms: intervalMs,
    registry_board_count: registry.board_count,
    sources: nextSources,
    blocked_boards: blockedBoards,
    overrides,
  };
  await writeFreshnessState(cacheDir, next);
  return {
    status: "PASS",
    checked_sources: checkedSources,
    changed_sources: changedSources,
    reingested_sources: reingestedSources,
    new_boards: previous ? Math.max(0, registry.board_count - previous.registry_board_count) : 0,
    blocked_boards: Object.keys(blockedBoards).sort(),
    overrides: Object.keys(overrides).sort(),
    last_checked: timestamp,
  };
}

/** @param {string} cacheDir @returns {Promise<FreshnessState | undefined>} */
export async function readFreshnessState(cacheDir) {
  try {
    const parsed = JSON.parse(await readFile(freshnessStateFile(cacheDir), "utf8"));
    if (!isRecord(parsed) || parsed.schema_version !== FRESHNESS_SCHEMA_VERSION) return undefined;
    if (typeof parsed.last_checked !== "string" || !isRecord(parsed.sources)) return undefined;
    return /** @type {FreshnessState} */ (parsed);
  } catch {
    return undefined;
  }
}

/**
 * Build one source row per distinct cached or committed official source.
 * Repository `pushed_at` is the cheap change token; raw bytes are fetched only
 * when that token changes.
 * @param {string} cacheDir
 * @param {import("./board-registry.mjs").BoardRegistry} registry
 * @returns {Promise<FreshnessSource[]>}
 */
export async function listTrackedSources(cacheDir, registry) {
  /** @type {Map<string, { pushed_at: string }>} */
  const byRepo = new Map();
  for (const board of registry.boards) {
    if (board.official_repo) byRepo.set(board.official_repo.toLowerCase(), { pushed_at: board.pushed_at ?? "" });
    for (const repo of board.repo_metadata ?? []) {
      byRepo.set(repo.url.toLowerCase(), { pushed_at: repo.pushed_at });
    }
  }
  /** @type {FreshnessSource[]} */
  const sources = [];

  for (const verdict of await listCachedVerdicts(cacheDir)) {
    if (verdict.status !== "verified") continue;
    const board = byRepo.get(verdict.repo_url.toLowerCase());
    sources.push({
      id: `cached:${verdict.board_id}:${verdict.source.path}`,
      kind: "cached",
      board_id: verdict.board_id,
      repo_url: verdict.repo_url,
      source_url: verdict.source.url,
      path: verdict.source.path,
      commit: verdict.source.commit,
      sha256: verdict.source.sha256,
      etag: verdict.source.etag ?? null,
      change_token: board?.pushed_at || null,
    });
  }

  const seen = new Set();
  for (const pack of loadFactPacks().packs) {
    for (const fact of [...pack.pin_matrix, ...pack.bus_matrix]) {
      const source = fact.source;
      if (!source.line_range || !SHA256_RE.test(source.hash)) continue;
      const parsed = parseGithubBlobUrl(source.path_or_url);
      if (!parsed) continue;
      const id = `committed:${pack.board_id}:${source.path_or_url}`;
      if (seen.has(id)) continue;
      seen.add(id);
      const board = byRepo.get(parsed.repo_url.toLowerCase());
      sources.push({
        id,
        kind: "committed",
        board_id: pack.board_id,
        repo_url: parsed.repo_url,
        source_url: source.path_or_url,
        path: parsed.path,
        commit: parsed.ref,
        sha256: source.hash,
        etag: null,
        change_token: board?.pushed_at || null,
      });
    }
  }
  return sources.sort((a, b) => a.id.localeCompare(b.id));
}

/** @param {FreshnessSource} source @returns {Promise<SourceCheck>} */
async function checkFreshnessSource(source) {
  if (source.kind === "cached") {
    return { status: "changed", commit: source.commit, sha256: source.sha256, etag: source.etag };
  }
  try {
    const fetched = await fetchSource(rawFetchUrl(source.source_url));
    const hash = sha256Hex(fetched.text);
    return {
      status: hash === source.sha256 ? "unchanged" : "changed",
      commit: source.commit,
      sha256: hash,
      etag: fetched.etag,
    };
  } catch {
    return { status: "offline", commit: source.commit, sha256: source.sha256, etag: source.etag };
  }
}

/**
 * @param {string} cacheDir
 * @param {FreshnessSource} source
 * @param {import("./board-registry.mjs").BoardRegistry} registry
 * @returns {Promise<import("./on-demand-ingest.mjs").IngestVerdict>}
 */
async function reingestChangedSource(cacheDir, source, registry) {
  return ensureOnDemandPinout(source.board_id, {
    cacheDir,
    forceRefresh: true,
    registry,
  });
}

/** @param {string} cacheDir @returns {Promise<import("./on-demand-ingest.mjs").IngestVerdict[]>} */
async function listCachedVerdicts(cacheDir) {
  const directory = join(cacheDir, "ingest");
  /** @type {string[]} */
  let names;
  try {
    names = await readdir(directory);
  } catch {
    return [];
  }
  const verdicts = await Promise.all(names
    .filter((name) => name.endsWith(".json"))
    .map((name) => readIngestVerdict(cacheDir, name.slice(0, -5))));
  return verdicts.filter((verdict) => verdict !== undefined);
}

/** @param {FreshnessSource} source @param {string} checked @param {FreshnessSourceState["verdict"]} verdict @returns {FreshnessSourceState} */
function sourceState(source, checked, verdict) {
  return {
    kind: source.kind,
    board_id: source.board_id,
    repo_url: source.repo_url,
    source_url: source.source_url,
    path: source.path,
    commit: source.commit,
    sha256: source.sha256,
    etag: source.etag,
    change_token: source.change_token,
    last_checked: checked,
    verdict,
  };
}

/** @param {string} cacheDir @param {FreshnessState} state @returns {Promise<void>} */
async function writeFreshnessState(cacheDir, state) {
  const file = freshnessStateFile(cacheDir);
  await mkdir(dirname(file), { recursive: true });
  const temporary = `${file}.${process.pid}.${Date.now()}.tmp`;
  await writeFile(temporary, `${JSON.stringify(state, null, 2)}\n`, "utf8");
  await rename(temporary, file);
}

/** @param {"THROTTLED" | "OFFLINE" | "DISABLED"} status @param {FreshnessState | undefined} state @returns {FreshnessReport} */
function reportFromState(status, state) {
  return {
    status,
    checked_sources: 0,
    changed_sources: 0,
    reingested_sources: 0,
    new_boards: 0,
    blocked_boards: Object.keys(state?.blocked_boards ?? {}).sort(),
    overrides: Object.keys(state?.overrides ?? {}).sort(),
    ...(state ? { last_checked: state.last_checked } : {}),
  };
}

/** @param {FreshnessState} state @param {Date} now @returns {number} */
function stateAgeMs(state, now) {
  const checked = Date.parse(state.last_checked);
  return Number.isFinite(checked) ? Math.max(0, now.getTime() - checked) : Number.POSITIVE_INFINITY;
}

/** @returns {number} */
function configuredIntervalMs() {
  const raw = process.env.LILYGO_SKILLS_FRESHNESS_INTERVAL_MS;
  if (!raw) return DEFAULT_INTERVAL_MS;
  const parsed = Number(raw);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : DEFAULT_INTERVAL_MS;
}

/** @param {string} url @returns {{ repo_url: string; ref: string; path: string } | undefined} */
function parseGithubBlobUrl(url) {
  try {
    const parsed = new URL(url);
    if (parsed.hostname !== "github.com") return undefined;
    const parts = parsed.pathname.split("/").filter(Boolean);
    const blobIndex = parts.indexOf("blob");
    const owner = parts[0];
    const repo = parts[1];
    const ref = parts[blobIndex + 1];
    const path = parts.slice(blobIndex + 2).map(decodeURIComponent).join("/");
    if (!owner || !repo || blobIndex !== 2 || !ref || !path) return undefined;
    return { repo_url: `https://github.com/${owner}/${repo}`, ref, path };
  } catch {
    return undefined;
  }
}

/** @param {unknown} value @returns {value is Record<string, unknown>} */
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

const SHA256_RE = /^sha256:[0-9a-f]{64}$/;
