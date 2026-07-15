import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { readFreshnessState, runDailyFreshness } from "../bin/freshness.mjs";

/** @returns {import("../bin/board-registry.mjs").BoardRegistry} */
function registry() {
  return {
    schema_version: 1,
    last_checked: "2026-07-14T00:00:00Z",
    status: "PASS",
    cache_status: "live",
    board_count: 10,
    sources: {
      github: { status: "PASS", count: 8 },
      mcp: { status: "PASS", count: 5 },
    },
    boards: [],
  };
}

/** @param {string} token @returns {import("../bin/freshness.mjs").FreshnessSource[]} */
function tracked(token) {
  return [{
    id: "cached:board-test:pins.h",
    kind: "cached",
    board_id: "board-test",
    repo_url: "https://github.com/Xinyuan-LilyGO/T-Test",
    source_url: "https://github.com/Xinyuan-LilyGO/T-Test/blob/1111111111111111111111111111111111111111/pins.h",
    path: "pins.h",
    commit: "1111111111111111111111111111111111111111",
    sha256: `sha256:${"a".repeat(64)}`,
    etag: "etag-a",
    change_token: token,
  }];
}

test("daily freshness throttles a second call within the interval", async () => {
  const cacheDir = await mkdtemp(join(tmpdir(), "lilygo-m38-freshness-throttle-"));
  let registryCalls = 0;
  let sourceChecks = 0;
  try {
    const first = await runDailyFreshness({
      cacheDir,
      now: new Date("2026-07-14T00:00:00Z"),
      registryRefresher: async () => { registryCalls++; return registry(); },
      sourceLister: async () => tracked("push-a"),
      sourceChecker: async () => { sourceChecks++; throw new Error("new sources seed without checking"); },
    });
    const second = await runDailyFreshness({
      cacheDir,
      now: new Date("2026-07-14T12:00:00Z"),
      registryRefresher: async () => { registryCalls++; return registry(); },
      sourceLister: async () => tracked("push-a"),
    });
    assert.equal(first.status, "PASS");
    assert.equal(second.status, "THROTTLED");
    assert.equal(registryCalls, 1);
    assert.equal(sourceChecks, 0);
  } finally {
    await rm(cacheDir, { recursive: true, force: true });
  }
});

test("changed source sha invokes exactly one gated re-ingest and records the update", async () => {
  const cacheDir = await mkdtemp(join(tmpdir(), "lilygo-m38-freshness-drift-"));
  let reingests = 0;
  try {
    await runDailyFreshness({
      cacheDir,
      now: new Date("2026-07-14T00:00:00Z"),
      registryRefresher: async () => registry(),
      sourceLister: async () => tracked("push-a"),
    });
    const report = await runDailyFreshness({
      cacheDir,
      now: new Date("2026-07-15T01:00:00Z"),
      registryRefresher: async () => ({ ...registry(), board_count: 11 }),
      sourceLister: async () => tracked("push-b"),
      sourceChecker: async () => ({
        status: "changed",
        commit: "2222222222222222222222222222222222222222",
        sha256: `sha256:${"b".repeat(64)}`,
        etag: "etag-b",
      }),
      reingestSource: async () => {
        reingests++;
        return /** @type {any} */ ({
          status: "verified",
          board_id: "board-test",
          source: {
            path: "pins.h",
            commit: "2222222222222222222222222222222222222222",
            sha256: `sha256:${"b".repeat(64)}`,
            etag: "etag-b",
            url: "https://github.com/Xinyuan-LilyGO/T-Test/blob/2222222222222222222222222222222222222222/pins.h",
          },
        });
      },
    });
    const state = await readFreshnessState(cacheDir);
    assert.equal(report.status, "PASS");
    assert.equal(report.changed_sources, 1);
    assert.equal(report.reingested_sources, 1);
    assert.equal(report.new_boards, 1);
    assert.equal(reingests, 1);
    assert.equal(state?.sources["cached:board-test:pins.h"]?.sha256, `sha256:${"b".repeat(64)}`);
    assert.deepEqual(report.overrides, ["board-test"]);
  } finally {
    await rm(cacheDir, { recursive: true, force: true });
  }
});

test("offline freshness is a no-op and does not advance last_checked", async () => {
  const cacheDir = await mkdtemp(join(tmpdir(), "lilygo-m38-freshness-offline-"));
  try {
    await runDailyFreshness({
      cacheDir,
      now: new Date("2026-07-14T00:00:00Z"),
      registryRefresher: async () => registry(),
      sourceLister: async () => tracked("push-a"),
    });
    const before = await readFreshnessState(cacheDir);
    const report = await runDailyFreshness({
      cacheDir,
      now: new Date("2026-07-15T01:00:00Z"),
      registryRefresher: async () => { throw new Error("offline"); },
    });
    const after = await readFreshnessState(cacheDir);
    assert.equal(report.status, "OFFLINE");
    assert.equal(after?.last_checked, before?.last_checked);
    assert.deepEqual(after, before);
  } finally {
    await rm(cacheDir, { recursive: true, force: true });
  }
});
