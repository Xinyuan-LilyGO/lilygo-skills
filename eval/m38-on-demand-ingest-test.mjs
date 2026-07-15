import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { ensureOnDemandPinout } from "../bin/on-demand-ingest.mjs";
import { sourceQueryWithOnDemand } from "../bin/query.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** @param {string} name @returns {Promise<any>} */
async function fixture(name) {
  return JSON.parse(await readFile(join(ROOT, "eval/fixtures", name), "utf8"));
}

/** @param {{ id: string; product: string; repo: string }} input @returns {import("../bin/board-registry.mjs").BoardRegistry} */
function registryFor(input) {
  return {
    schema_version: 1,
    last_checked: "2026-07-14T00:00:00.000Z",
    status: "PASS",
    cache_status: "live",
    board_count: 1,
    sources: {
      github: { status: "PASS", count: 1 },
      mcp: { status: "PASS", count: 1 },
    },
    boards: [{
      id: input.id,
      product_name: input.product,
      official_repo: input.repo,
      repository_name: input.repo.split("/").pop() || null,
      default_branch: "main",
      archived: false,
      pushed_at: "2026-07-14T00:00:00Z",
      shop_link: null,
      category: "other",
      tags: ["ESP32"],
      listing_sources: ["github-org", "official-mcp"],
      source_of_listing: "github-org+official-mcp",
      aliases: [input.product],
      repo_candidates: [input.repo],
    }],
  };
}

test("known AUTO board ingests on demand and serves only fully provenanced pins", async () => {
  const cacheDir = await mkdtemp(join(tmpdir(), "lilygo-m38-ingest-happy-"));
  const registry = registryFor({
    id: "board-t-nixietube",
    product: "T-NixieTube",
    repo: "https://github.com/Xinyuan-LilyGO/T-NixieTube",
  });
  let crawls = 0;
  try {
    const report = await sourceQueryWithOnDemand("T-NixieTube", "pinout", {
      cacheDir,
      registry,
      repositoryFetcher: async () => { crawls++; return fixture("m38-ingest-happy.json"); },
      now: new Date("2026-07-14T01:00:00Z"),
    });
    assert.equal(report.status, "PASS");
    assert.equal(report.facts.length, 6);
    assert.ok(report.facts.every((fact) => fact.source.path_or_url.includes("/blob/1111111111111111111111111111111111111111/")));
    assert.ok(report.facts.every((fact) => /^sha256:[0-9a-f]{64}$/.test(fact.source.hash)));
    assert.ok(report.facts.every((fact) => /^\d+-\d+$/.test(fact.source.line_range || "")));

    const second = await sourceQueryWithOnDemand("T-NixieTube", "pinout", {
      cacheDir,
      registry,
      repositoryFetcher: async () => { crawls++; throw new Error("cache miss"); },
    });
    assert.equal(second.status, "PASS");
    assert.equal(crawls, 1);
  } finally {
    await rm(cacheDir, { recursive: true, force: true });
  }
});

test("ambiguous FLAG-style repository degrades honestly and exposes no pin value", async () => {
  const cacheDir = await mkdtemp(join(tmpdir(), "lilygo-m38-ingest-degrade-"));
  const registry = registryFor({
    id: "board-ambiguous",
    product: "T-Ambiguous",
    repo: "https://github.com/Xinyuan-LilyGO/T-Ambiguous",
  });
  let crawls = 0;
  try {
    const report = await sourceQueryWithOnDemand("board-ambiguous", "pinout", {
      cacheDir,
      registry,
      repositoryFetcher: async () => { crawls++; return fixture("m38-ingest-ambiguous.json"); },
    });
    assert.equal(report.status, "NO_VERIFIABLE_PINOUT");
    assert.ok("reason" in report);
    assert.equal(report.reason, "multiple-sources");
    assert.equal(report.repo_url, "https://github.com/Xinyuan-LilyGO/T-Ambiguous");
    assert.deepEqual(report.facts, []);
    assert.deepEqual(report.pin_matrix, []);
    assert.deepEqual(report.source_refs, []);
    assert.doesNotMatch(JSON.stringify(report), /GPIO\d+/);

    const second = await sourceQueryWithOnDemand("board-ambiguous", "pinout", {
      cacheDir,
      registry,
      repositoryFetcher: async () => { crawls++; throw new Error("must use degraded cache"); },
    });
    assert.equal(second.status, "NO_VERIFIABLE_PINOUT");
    assert.equal(crawls, 1);
  } finally {
    await rm(cacheDir, { recursive: true, force: true });
  }
});

test("on-demand disabled returns a clean not-covered result without registry or crawl calls", async () => {
  const cacheDir = await mkdtemp(join(tmpdir(), "lilygo-m38-ingest-disabled-"));
  let calls = 0;
  try {
    const verdict = await ensureOnDemandPinout("board-t-nixietube", {
      cacheDir,
      enabled: false,
      registryLoader: async () => { calls++; throw new Error("must not load registry"); },
      repositoryFetcher: async () => { calls++; throw new Error("must not crawl"); },
    });
    assert.equal(verdict.status, "degraded");
    assert.equal(verdict.reason, "on-demand-disabled");
    assert.equal(calls, 0);
  } finally {
    await rm(cacheDir, { recursive: true, force: true });
  }
});
