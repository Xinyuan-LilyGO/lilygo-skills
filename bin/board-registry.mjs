// Dynamic LilyGO board/product registry.
//
// The board universe comes from two vendor-controlled listings: the public
// Xinyuan-LilyGO GitHub organization and the official documentation MCP. This
// module intentionally proves only that a product exists. Pin authority stays
// in the curated manifest or the separately gated on-demand ingest path.
import { execFile as execFileCallback } from "node:child_process";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { promisify } from "node:util";

import { McpSseClient, toolText } from "../eval/official-mcp.mjs";
import { isMain, loadBoards, slug } from "./lib.mjs";

const execFile = promisify(execFileCallback);
const REGISTRY_SCHEMA_VERSION = 1;
const DEFAULT_MAX_AGE_MS = 24 * 60 * 60 * 1_000;
const DEFAULT_TIMEOUT_MS = 30_000;
const MAX_GH_OUTPUT_BYTES = 16 * 1024 * 1024;

/** @typedef {"github-org" | "official-mcp"} RegistryListingSource */
/**
 * @typedef {object} GithubBoardRepo
 * @property {string} name
 * @property {string} full_name
 * @property {string} html_url
 * @property {string} description
 * @property {boolean} archived
 * @property {boolean} fork
 * @property {string} default_branch
 * @property {string} pushed_at
 */
/**
 * @typedef {object} OfficialProduct
 * @property {string} product
 * @property {string} title
 * @property {string} category
 * @property {string} shop_link
 * @property {string[]} tags
 */
/**
 * @typedef {object} BoardRegistryEntry
 * @property {string} id
 * @property {string} product_name
 * @property {string | null} official_repo
 * @property {string | null} repository_name
 * @property {string | null} default_branch
 * @property {boolean | null} archived
 * @property {string | null} pushed_at
 * @property {string | null} shop_link
 * @property {string | null} category
 * @property {string[]} tags
 * @property {RegistryListingSource[]} listing_sources
 * @property {string} source_of_listing
 * @property {string[]} aliases
 * @property {string[]} repo_candidates
 * @property {{ url: string; name: string; default_branch: string; archived: boolean; pushed_at: string }[]=} repo_metadata
 */
/**
 * @typedef {object} BoardRegistry
 * @property {number} schema_version
 * @property {string} last_checked
 * @property {"PASS" | "PARTIAL" | "OFFLINE"} status
 * @property {"live" | "fresh-cache" | "stale-fallback" | "offline-cache" | "empty-offline"} cache_status
 * @property {number} board_count
 * @property {{ github: RegistrySourceStatus; mcp: RegistrySourceStatus }} sources
 * @property {BoardRegistryEntry[]} boards
 */
/**
 * @typedef {object} RegistrySourceStatus
 * @property {"PASS" | "ERROR" | "CACHED" | "DISABLED"} status
 * @property {number} count
 * @property {string=} error
 */
/**
 * @typedef {object} BoardRegistryOptions
 * @property {string=} cacheDir
 * @property {boolean=} forceRefresh
 * @property {boolean=} networkEnabled
 * @property {number=} maxAgeMs
 * @property {number=} timeoutMs
 * @property {Date=} now
 * @property {() => Promise<unknown>=} githubFetcher
 * @property {() => Promise<unknown>=} mcpFetcher
 * @property {BoardRecord[]=} localBoards
 */

/**
 * Cache root used by dynamic runtime writers. Tests and maintainers can inject
 * a separate root without touching the committed repository.
 * @param {NodeJS.ProcessEnv} [env]
 * @returns {string}
 */
export function m38CacheRoot(env = process.env) {
  if (env.LILYGO_SKILLS_CACHE_DIR) return join(env.LILYGO_SKILLS_CACHE_DIR, "m38");
  if (env.XDG_CACHE_HOME) return join(env.XDG_CACHE_HOME, "lilygo-skills", "m38");
  return join(homedir(), ".cache", "lilygo-skills", "m38");
}

/**
 * Product identity key used only for exact normalization/dedup. It never uses
 * prefix/fuzzy matching, because a fuzzy board-to-repository match is unsafe.
 * @param {string} value
 * @returns {string}
 */
export function productKey(value) {
  return slug(value)
    .replace(/^(?:board-)?(?:xinyuan-)?(?:lilygo|ttgo)-/, "")
    .replace(/^board-/, "");
}

/**
 * Conservative board-repository heuristic. It is deliberately based
 * on naming/description classes rather than an allow-list, so new board repos
 * can appear automatically while libraries, firmware ports, docs, and tools
 * stay out of the board universe.
 * @param {GithubBoardRepo} repo
 * @returns {boolean}
 */
export function isBoardRepository(repo) {
  const name = repo.name.toLowerCase();
  const description = repo.description.toLowerCase();
  const negativeName = /(?:^|[-_.])(library|lib|driver|deps|example|examples|dashboard|firmware|micropython|lvgl|case|document|docs|documentation|wiki|skills?|assistant|mcp|website|launchpad|hal|sdk|modules|script|private)(?:$|[-_.])/;
  if (negativeName.test(name)) return false;
  if (/^(?:lilygolib|lilygo-device-driver|lilygo_device_driver|document-list|document_list|ch\d+.*driver|twatch-example)/.test(name)) return false;
  if (/(?:baidu[-_]rec|see[-_]camera|weather-display|cayenne|game|helium-mapper|3d-model|3d_model)/.test(name)) return false;
  if (
    /\b(?:library for|project dependent libraries|sample repository|operating system|firmware hub|flashing tool|web flash tool|website and documentation|android application|python cli)\b/.test(description) ||
    /^(?:ui written for|3d model and case)/.test(description)
  ) return false;

  return (
    /^(?:t[-_0-9]|t\d|ttgo[-_]|lilygo[-_]|lilygo_t|lilypi$|esp32_s2$)/.test(name) ||
    /^(?:wrist-e-paper|3\.71-inch-)/.test(name)
  );
}

/**
 * Parse the `gh api --paginate --slurp` response into validated board rows.
 * A flat array is accepted as well so recorded fixtures stay small.
 * @param {unknown} raw
 * @returns {GithubBoardRepo[]}
 */
export function parseGithubOrgResponse(raw) {
  const outer = Array.isArray(raw) ? raw : [];
  const rows = outer.every(Array.isArray) ? outer.flat() : outer;
  return rows.flatMap((value) => {
    if (!isRecord(value)) return [];
    const name = stringField(value, "name");
    const htmlUrl = stringField(value, "html_url");
    const defaultBranch = stringField(value, "default_branch");
    if (!name || !htmlUrl || !defaultBranch) return [];
    /** @type {GithubBoardRepo} */
    const repo = {
      name,
      full_name: stringField(value, "full_name") || `Xinyuan-LilyGO/${name}`,
      html_url: htmlUrl.replace(/\/$/, ""),
      description: stringField(value, "description"),
      archived: value.archived === true,
      fork: value.fork === true,
      default_branch: defaultBranch,
      pushed_at: stringField(value, "pushed_at"),
    };
    return isBoardRepository(repo) ? [repo] : [];
  });
}

/**
 * Parse either a direct product array or an MCP `tools/call` result whose text
 * content contains the JSON array returned by `list_products`.
 * @param {unknown} raw
 * @returns {OfficialProduct[]}
 */
export function parseOfficialProducts(raw) {
  /** @type {unknown[]} */
  let rows = [];
  if (Array.isArray(raw)) {
    rows = raw;
  } else if (isRecord(raw)) {
    for (const part of toolText(raw)) {
      try {
        const parsed = JSON.parse(part);
        if (Array.isArray(parsed)) rows.push(...parsed);
      } catch {
        // A malformed MCP text block is ignored; it cannot authorize a board.
      }
    }
  }
  return rows.flatMap((value) => {
    if (!isRecord(value)) return [];
    const product = stringField(value, "product");
    const title = stringField(value, "title");
    if (!product || !title) return [];
    return [{
      product,
      title,
      category: stringField(value, "category"),
      shop_link: stringField(value, "shopLink"),
      tags: Array.isArray(value.tags) ? value.tags.filter((tag) => typeof tag === "string") : [],
    }];
  });
}

/**
 * Merge and deduplicate the two official listings. Two GitHub repositories
 * with the same exact product key remain one catalog row but deliberately have
 * `official_repo: null`; callers must not guess between them.
 * @param {GithubBoardRepo[]} githubRepos
 * @param {OfficialProduct[]} products
 * @param {BoardRecord[]} [localBoards]
 * @returns {BoardRegistryEntry[]}
 */
export function mergeBoardListings(githubRepos, products, localBoards = []) {
  /** @type {Map<string, { repos: GithubBoardRepo[]; products: OfficialProduct[] }>} */
  const groups = new Map();
  for (const repo of githubRepos) {
    const key = productKey(repo.name);
    if (!key) continue;
    const group = groups.get(key) ?? { repos: [], products: [] };
    group.repos.push(repo);
    groups.set(key, group);
  }
  for (const product of products) {
    const key = productKey(product.product || product.title);
    if (!key) continue;
    const group = groups.get(key) ?? { repos: [], products: [] };
    group.products.push(product);
    groups.set(key, group);
  }

  return [...groups.entries()].map(([key, group]) => {
    const repos = uniqueBy(group.repos, (repo) => repo.html_url.toLowerCase());
    const officialRepo = repos.length === 1 ? repos[0] : undefined;
    const product = group.products[0];
    const listingSources = /** @type {RegistryListingSource[]} */ ([]);
    if (repos.length > 0) listingSources.push("github-org");
    if (group.products.length > 0) listingSources.push("official-mcp");
    const productName = product?.title || officialRepo?.name || repos[0]?.name || key;
    const localId = matchLocalBoardId(localBoards, officialRepo?.html_url, [
      key,
      productName,
      product?.product || "",
      ...(repos.map((repo) => repo.name)),
    ]);
    const aliases = uniqueStrings([
      productName,
      product?.product || "",
      ...(repos.map((repo) => repo.name)),
    ]);
    return {
      id: localId || `board-${key}`,
      product_name: productName,
      official_repo: officialRepo?.html_url ?? null,
      repository_name: officialRepo?.name ?? null,
      default_branch: officialRepo?.default_branch ?? null,
      archived: officialRepo?.archived ?? null,
      pushed_at: officialRepo?.pushed_at || null,
      shop_link: product?.shop_link || null,
      category: product?.category || null,
      tags: uniqueStrings(group.products.flatMap((row) => row.tags)),
      listing_sources: listingSources,
      source_of_listing: listingSources.join("+"),
      aliases,
      repo_candidates: repos.map((repo) => repo.html_url).sort(),
      repo_metadata: repos.map((repo) => ({
        url: repo.html_url,
        name: repo.name,
        default_branch: repo.default_branch,
        archived: repo.archived,
        pushed_at: repo.pushed_at,
      })).sort((a, b) => a.url.localeCompare(b.url)),
    };
  }).sort((a, b) => a.id.localeCompare(b.id) || a.product_name.localeCompare(b.product_name));
}

/** @returns {Promise<unknown>} */
export async function fetchGithubOrgRepos() {
  const result = await execFile(
    "gh",
    ["api", "--paginate", "--slurp", "orgs/Xinyuan-LilyGO/repos"],
    { encoding: "utf8", timeout: DEFAULT_TIMEOUT_MS, maxBuffer: MAX_GH_OUTPUT_BYTES },
  );
  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    throw new Error(`parse GitHub organization response failed: ${error instanceof Error ? error.message : String(error)}`);
  }
}

/** @param {number} [timeoutMs] @returns {Promise<unknown>} */
export async function fetchOfficialMcpProducts(timeoutMs = DEFAULT_TIMEOUT_MS) {
  const client = new McpSseClient({ baseUrl: process.env.LILYGO_OFFICIAL_MCP_URL, timeoutMs });
  try {
    return await client.callTool("list_products", {});
  } finally {
    await client.close();
  }
}

/**
 * Load a fresh registry when due, with a valid cache as the offline fallback.
 * Fetchers are injectable so unit tests never touch the network.
 * @param {BoardRegistryOptions} [options]
 * @returns {Promise<BoardRegistry>}
 */
export async function getBoardRegistry(options = {}) {
  const now = options.now ?? new Date();
  const cacheDir = options.cacheDir ?? m38CacheRoot();
  const cacheFile = join(cacheDir, "board-registry.json");
  const maxAgeMs = options.maxAgeMs ?? DEFAULT_MAX_AGE_MS;
  const networkEnabled = options.networkEnabled ?? process.env.LILYGO_SKILLS_REGISTRY_NETWORK !== "0";
  const cached = await readRegistryCache(cacheFile);
  if (!options.forceRefresh && cached && cacheAgeMs(cached, now) < maxAgeMs) {
    return { ...cached, cache_status: "fresh-cache" };
  }
  if (!networkEnabled) {
    if (cached) return { ...cached, status: "OFFLINE", cache_status: "offline-cache" };
    return emptyRegistry(now, "empty-offline");
  }

  const githubFetcher = options.githubFetcher ?? fetchGithubOrgRepos;
  const mcpFetcher = options.mcpFetcher ?? (() => fetchOfficialMcpProducts(options.timeoutMs));
  const [githubResult, mcpResult] = await Promise.allSettled([githubFetcher(), mcpFetcher()]);
  if (githubResult.status === "rejected" && mcpResult.status === "rejected" && cached) {
    return {
      ...cached,
      status: "OFFLINE",
      cache_status: "stale-fallback",
      sources: {
        github: { status: "ERROR", count: 0, error: errorMessage(githubResult.reason) },
        mcp: { status: "ERROR", count: 0, error: errorMessage(mcpResult.reason) },
      },
    };
  }

  const githubRows = githubResult.status === "fulfilled" ? parseGithubOrgResponse(githubResult.value) : [];
  const mcpRows = mcpResult.status === "fulfilled" ? parseOfficialProducts(mcpResult.value) : [];
  const localBoards = options.localBoards ?? loadBoards().boards;
  const boards = mergeBoardListings(githubRows, mcpRows, localBoards);
  /** @type {BoardRegistry} */
  const registry = {
    schema_version: REGISTRY_SCHEMA_VERSION,
    last_checked: now.toISOString(),
    status: githubResult.status === "fulfilled" && mcpResult.status === "fulfilled" ? "PASS" : "PARTIAL",
    cache_status: "live",
    board_count: boards.length,
    sources: {
      github: githubResult.status === "fulfilled"
        ? { status: "PASS", count: githubRows.length }
        : { status: "ERROR", count: 0, error: errorMessage(githubResult.reason) },
      mcp: mcpResult.status === "fulfilled"
        ? { status: "PASS", count: mcpRows.length }
        : { status: "ERROR", count: 0, error: errorMessage(mcpResult.reason) },
    },
    boards,
  };
  if (boards.length > 0) await writeJsonAtomic(cacheFile, registry);
  return registry;
}

/**
 * @param {string[]} argv
 * @returns {Promise<number>}
 */
export async function runBoardRegistry(argv) {
  const args = argv[0] === "board" && argv[1] === "list" ? argv.slice(2) : argv;
  if (!args.includes("--json")) {
    process.stderr.write("usage: board list --json [--refresh] [--offline]\n");
    return 2;
  }
  try {
    const registry = await getBoardRegistry({
      forceRefresh: args.includes("--refresh"),
      networkEnabled: !args.includes("--offline"),
    });
    process.stdout.write(`${JSON.stringify(registry, null, 2)}\n`);
    return 0;
  } catch (error) {
    process.stderr.write(`${errorMessage(error)}\n`);
    return 1;
  }
}

/** @param {unknown} value @returns {value is Record<string, unknown>} */
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** @param {Record<string, unknown>} value @param {string} key @returns {string} */
function stringField(value, key) {
  const field = value[key];
  return typeof field === "string" ? field : "";
}

/**
 * @template T
 * @param {T[]} values
 * @param {(value: T) => string} keyOf
 * @returns {T[]}
 */
function uniqueBy(values, keyOf) {
  const seen = new Set();
  return values.filter((value) => {
    const key = keyOf(value);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

/** @param {string[]} values @returns {string[]} */
function uniqueStrings(values) {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}

/**
 * @param {BoardRecord[]} boards
 * @param {string | undefined} repoUrl
 * @param {string[]} names
 * @returns {string | undefined}
 */
function matchLocalBoardId(boards, repoUrl, names) {
  if (repoUrl) {
    const exact = boards.filter((board) => board.repo_url?.replace(/\/$/, "").toLowerCase() === repoUrl.toLowerCase());
    if (exact.length === 1) return exact[0]?.id;
  }
  const keys = new Set(names.map(productKey).filter(Boolean));
  const matches = boards.filter((board) => [board.id, board.display_name || "", ...board.aliases]
    .map(productKey)
    .some((key) => keys.has(key)));
  return matches.length === 1 ? matches[0]?.id : undefined;
}

/** @param {BoardRegistry} registry @param {Date} now @returns {number} */
function cacheAgeMs(registry, now) {
  const checked = Date.parse(registry.last_checked);
  return Number.isFinite(checked) ? Math.max(0, now.getTime() - checked) : Number.POSITIVE_INFINITY;
}

/** @param {string} file @returns {Promise<BoardRegistry | undefined>} */
async function readRegistryCache(file) {
  try {
    const parsed = JSON.parse(await readFile(file, "utf8"));
    if (!isRecord(parsed) || parsed.schema_version !== REGISTRY_SCHEMA_VERSION || !Array.isArray(parsed.boards)) return undefined;
    if (typeof parsed.last_checked !== "string" || typeof parsed.board_count !== "number") return undefined;
    return /** @type {BoardRegistry} */ (parsed);
  } catch {
    return undefined;
  }
}

/** @param {string} file @param {unknown} value @returns {Promise<void>} */
async function writeJsonAtomic(file, value) {
  await mkdir(dirname(file), { recursive: true });
  const temporary = `${file}.${process.pid}.${Date.now()}.tmp`;
  await writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  await rename(temporary, file);
}

/**
 * @param {Date} now
 * @param {BoardRegistry["cache_status"]} cacheStatus
 * @returns {BoardRegistry}
 */
function emptyRegistry(now, cacheStatus) {
  return {
    schema_version: REGISTRY_SCHEMA_VERSION,
    last_checked: now.toISOString(),
    status: "OFFLINE",
    cache_status: cacheStatus,
    board_count: 0,
    sources: {
      github: { status: "DISABLED", count: 0 },
      mcp: { status: "DISABLED", count: 0 },
    },
    boards: [],
  };
}

/** @param {unknown} error @returns {string} */
function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

if (isMain(import.meta.url)) {
  runBoardRegistry(process.argv.slice(2)).then((code) => { process.exitCode = code; });
}
