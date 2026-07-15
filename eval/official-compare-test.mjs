import assert from "node:assert/strict";
import { once } from "node:events";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { compareSignals, extractOfficialSignals, extractOursSignals, mapBoardsToProducts } from "./official-compare-lib.mjs";
import { McpSseClient, parseSseBlock, toolText } from "./official-mcp.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** @typedef {{ coverage_status: string; verdict: "win" | "lose" | "unmeasurable"; ours: Record<string, any>; official: Record<string, any>; value_agreement: Record<string, number> }} FixtureBoard */

test("parseSseBlock handles endpoint and multiline data events", () => {
  assert.deepEqual(parseSseBlock("event: endpoint\ndata: /messages?id=1"), {
    event: "endpoint",
    data: "/messages?id=1",
  });
  assert.deepEqual(parseSseBlock("data: first\ndata: second"), {
    event: "message",
    data: "first\nsecond",
  });
  assert.equal(parseSseBlock(": keepalive"), undefined);
});

test("McpSseClient performs the MCP-SSE handshake and tool call without dependencies", async () => {
  /** @type {import("node:http").ServerResponse | undefined} */
  let stream;
  /** @type {string[]} */
  const methods = [];
  const server = createServer(async (request, response) => {
    if (request.method === "GET" && request.url === "/sse") {
      stream = response;
      response.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache" });
      response.write("event: endpoint\ndata: /messages?sessionId=test\n\n");
      return;
    }
    if (request.method !== "POST" || request.url !== "/messages?sessionId=test") {
      response.writeHead(404).end();
      return;
    }
    const chunks = [];
    for await (const chunk of request) chunks.push(Buffer.from(chunk));
    const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    methods.push(body.method);
    response.writeHead(202).end("Accepted");
    if (body.id === undefined || !stream) return;
    const result = body.method === "initialize"
      ? { protocolVersion: "2024-11-05", capabilities: { tools: {} }, serverInfo: { name: "test", version: "1" } }
      : body.method === "tools/call"
        ? { content: [{ type: "text", text: "[]" }] }
        : { tools: [] };
    stream.write(`event: message\ndata: ${JSON.stringify({ jsonrpc: "2.0", id: body.id, result })}\n\n`);
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  assert.ok(address && typeof address === "object");
  const client = new McpSseClient({ baseUrl: `http://127.0.0.1:${address.port}`, timeoutMs: 2_000 });
  try {
    const result = await client.callTool("list_products", {});
    assert.deepEqual(toolText(result), ["[]"]);
    assert.deepEqual(methods, ["initialize", "notifications/initialized", "tools/call"]);
  } finally {
    await client.close();
    await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve(undefined)));
  }
});

test("board mapping is normalized but refuses ambiguous prefixes", () => {
  const products = [
    { product: "t-display-s3", title: "T-Display-S3" },
    { product: "t-watch-2019", title: "T-Watch 2019" },
    { product: "t-watch-2021", title: "T-Watch 2021" },
  ];
  const mapping = mapBoardsToProducts(["board-t-display-s3", "board-t-watch"], products);
  assert.equal(mapping[0]?.official_product, "t-display-s3");
  assert.equal(mapping[0]?.match_method, "normalized-product-id");
  assert.equal(mapping[1]?.status, "no-official-coverage");
  assert.equal(mapping[1]?.official_product, null);
});

test("comparison distinguishes agree, disagree, official-missing, and ours-missing", () => {
  const citation = {
    kind: "official-code",
    path_or_url: "https://github.com/Xinyuan-LilyGO/example/blob/main/pins.h",
    line_range: "1-5",
    hash: `sha256:${"a".repeat(64)}`,
  };
  const ours = extractOursSignals([
    makeFact("pin.i2c.sda", "SDA=GPIO18", citation),
    makeFact("pin.i2c.scl", "SCL=GPIO17", citation),
    makeFact("pin.display.cs", "LCD_CS=GPIO6", citation),
  ]);
  const official = extractOfficialSignals({
    pinTables: [[
      { I2C: "ESP32-S3", SDA: "GPIO18", SCL: "GPIO16" },
      { Button: "ESP32-S3", BOOT: "GPIO0" },
    ]],
  });
  const compared = compareSignals(ours, official);
  assert.deepEqual(compared.counts, { agree: 1, disagree: 1, official_missing: 1, ours_missing: 1 });
  assert.equal(compared.ours_missing[0]?.signal, "button.1");
});

test("committed official scorecard preserves arithmetic, provenance, and honest states", async () => {
  const scorecard = /** @type {{ boards: FixtureBoard[]; summary: any }} */ (
    JSON.parse(await readFile(join(ROOT, "eval/fixtures/official-compare-2026-07-14.json"), "utf8"))
  );
  const shared = scorecard.boards.filter((board) => board.coverage_status === "shared");
  assert.equal(shared.length, scorecard.summary.boards_compared);
  assert.equal(scorecard.summary.coverage.shared_products, 22);
  assert.equal(scorecard.summary.coverage.official_products, 92);
  assert.equal(scorecard.summary.provenance.ours_url_sha256_rate, 1);
  assert.equal(scorecard.summary.provenance.ours_url_line_range_sha256_rate, 0.7961);
  assert.equal(scorecard.summary.provenance.official_per_fact_citation_rate, 0);

  const totals = shared.reduce((sum, board) => {
    assert.ok(["win", "lose", "unmeasurable"].includes(board.verdict));
    assert.equal(board.ours.all_have_url, true);
    assert.equal(board.ours.all_have_sha256, true);
    if (board.verdict === "unmeasurable") {
      assert.equal(typeof board.official.raw_error, "string");
      assert.ok(board.official.raw_error.length > 0);
    } else {
      assert.match(board.official.response_sha256, /^sha256:[0-9a-f]{64}$/);
      assert.equal(board.official.has_per_fact_provenance, false);
    }
    sum[board.verdict] += 1;
    /** @type {("agree" | "disagree" | "official_missing" | "ours_missing")[]} */
    const agreementKeys = ["agree", "disagree", "official_missing", "ours_missing"];
    for (const key of agreementKeys) {
      const value = board.value_agreement[key];
      assert.equal(typeof value, "number");
      sum[key] += value ?? 0;
    }
    return sum;
  }, { win: 0, lose: 0, unmeasurable: 0, agree: 0, disagree: 0, official_missing: 0, ours_missing: 0 });

  assert.deepEqual(
    { win: totals.win, lose: totals.lose, unmeasurable: totals.unmeasurable },
    scorecard.summary.verdicts,
  );
  assert.deepEqual(
    {
      agree: totals.agree,
      disagree: totals.disagree,
      official_missing: totals.official_missing,
      ours_missing: totals.ours_missing,
      rate: Number((totals.agree / (totals.agree + totals.disagree)).toFixed(4)),
    },
    scorecard.summary.value_agreement,
  );
});

/**
 * @param {string} key
 * @param {string} value
 * @param {SourceRef} source
 * @returns {Fact}
 */
function makeFact(key, value, source) {
  return {
    schema_version: 1,
    board_id: "board-test",
    topic: "pinout",
    key,
    value,
    claim: key,
    source,
    authority_rank: 100,
    evidence_level: "V3-source-reference",
    stale: false,
    confidence: "exact",
  };
}
