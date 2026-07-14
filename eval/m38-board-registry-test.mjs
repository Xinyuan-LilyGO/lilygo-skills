import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  getBoardRegistry,
  mergeBoardListings,
  parseGithubOrgResponse,
  parseOfficialProducts,
  productKey,
} from "../bin/board-registry.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** @param {string} name @returns {Promise<unknown>} */
async function fixture(name) {
  return JSON.parse(await readFile(join(ROOT, "eval/fixtures", name), "utf8"));
}

test("registry parses recorded GitHub/MCP listings and deduplicates conservatively", async () => {
  const github = parseGithubOrgResponse(await fixture("m38-github-org.json"));
  const mcp = parseOfficialProducts(await fixture("m38-mcp-products.json"));
  assert.equal(github.length, 4);
  assert.equal(mcp.length, 3);
  assert.equal(productKey("LilyGo-T-RGB"), "t-rgb");

  const boards = mergeBoardListings(github, mcp, []);
  const can = boards.find((board) => board.id === "board-t-2can");
  assert.equal(can?.official_repo, "https://github.com/Xinyuan-LilyGO/T-2Can");
  assert.deepEqual(can?.listing_sources, ["github-org", "official-mcp"]);

  const duplicate = boards.find((board) => board.id === "board-mini-epaper-s3");
  assert.equal(duplicate?.official_repo, "https://github.com/Xinyuan-LilyGO/LilyGO-Mini-Epaper-S3");
  assert.equal(duplicate?.repo_candidates.length, 1);
  assert.equal(duplicate?.listing_sources.includes("github-org"), true);
  assert.equal(boards.some((board) => board.repository_name === "LilyGo-TWR-Library"), false);
});

test("registry falls back to the stamped cache when both official listings are offline", async () => {
  const cacheDir = await mkdtemp(join(tmpdir(), "lilygo-m38-registry-"));
  const githubFixture = await fixture("m38-github-org.json");
  const mcpFixture = await fixture("m38-mcp-products.json");
  try {
    const live = await getBoardRegistry({
      cacheDir,
      forceRefresh: true,
      now: new Date("2026-07-14T00:00:00Z"),
      githubFetcher: async () => githubFixture,
      mcpFetcher: async () => mcpFixture,
      localBoards: [],
    });
    assert.equal(live.status, "PASS");
    assert.equal(live.cache_status, "live");
    assert.ok(live.board_count >= 3);

    const offline = await getBoardRegistry({
      cacheDir,
      forceRefresh: true,
      now: new Date("2026-07-15T00:00:00Z"),
      githubFetcher: async () => { throw new Error("github offline"); },
      mcpFetcher: async () => { throw new Error("mcp offline"); },
      localBoards: [],
    });
    assert.equal(offline.status, "OFFLINE");
    assert.equal(offline.cache_status, "stale-fallback");
    assert.equal(offline.board_count, live.board_count);
    assert.equal(offline.last_checked, live.last_checked);
    assert.match(offline.sources.github.error || "", /github offline/);
  } finally {
    await rm(cacheDir, { recursive: true, force: true });
  }
});

test("registry offline mode uses cache and never calls injected fetchers", async () => {
  const cacheDir = await mkdtemp(join(tmpdir(), "lilygo-m38-registry-disabled-"));
  let calls = 0;
  try {
    await getBoardRegistry({
      cacheDir,
      forceRefresh: true,
      githubFetcher: async () => { calls++; return fixture("m38-github-org.json"); },
      mcpFetcher: async () => { calls++; return fixture("m38-mcp-products.json"); },
      localBoards: [],
    });
    const offline = await getBoardRegistry({
      cacheDir,
      forceRefresh: true,
      networkEnabled: false,
      githubFetcher: async () => { calls++; throw new Error("must not call"); },
      mcpFetcher: async () => { calls++; throw new Error("must not call"); },
      localBoards: [],
    });
    assert.equal(calls, 2);
    assert.equal(offline.cache_status, "offline-cache");
  } finally {
    await rm(cacheDir, { recursive: true, force: true });
  }
});
