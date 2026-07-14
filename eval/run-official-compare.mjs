#!/usr/bin/env node
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { sourceQuery } from "../bin/query.mjs";
import { McpSseClient, toolText } from "./official-mcp.mjs";
import {
  compareSignals,
  extractOfficialSignals,
  extractOursSignals,
  mapBoardsToProducts,
  measureOfficialProvenance,
  measureOursProvenance,
  officialStructuredFactCount,
  parseToolJson,
  sha256,
} from "./official-compare-lib.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const ENDPOINT = "https://lilygo-doc-mcp-production.up.railway.app";
const FIXTURE = join(ROOT, "eval/fixtures/official-compare-2026-07-14.json");
const REPORT = join(ROOT, ".m36-report.md");

/** @param {number} numerator @param {number} denominator */
const rate = (numerator, denominator) => denominator === 0 ? null : Number((numerator / denominator).toFixed(4));

/** @param {unknown} value */
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** @param {unknown} parsed */
function requireProducts(parsed) {
  if (!Array.isArray(parsed)) throw new Error("list_products did not return an array");
  return parsed.map((item, index) => {
    if (!isRecord(item) || typeof item.product !== "string" || typeof item.title !== "string") {
      throw new Error(`list_products item ${index} lacks product/title`);
    }
    return {
      product: item.product,
      title: item.title,
      ...(typeof item.category === "string" ? { category: item.category } : {}),
      ...(Array.isArray(item.tags) && item.tags.every((tag) => typeof tag === "string") ? { tags: item.tags } : {}),
      ...(typeof item.shopLink === "string" ? { shopLink: item.shopLink } : {}),
    };
  });
}

/** @param {string} product @param {Record<string, unknown>} result */
function parseSpecs(product, result) {
  const parsed = parseToolJson(result, `get_product_specs(${product})`);
  if (!isRecord(parsed) || parsed.product !== product || !Array.isArray(parsed.pinTables) || !Array.isArray(parsed.parameters)) {
    throw new Error(`get_product_specs(${product}) returned an invalid structured payload`);
  }
  return parsed;
}

async function healthCheck() {
  const started = performance.now();
  try {
    const response = await fetch(`${ENDPOINT}/health`);
    const body = await response.text();
    return { status: response.status, ok: response.ok, elapsed_ms: Math.round(performance.now() - started), body };
  } catch (error) {
    return { status: null, ok: false, elapsed_ms: Math.round(performance.now() - started), error: error instanceof Error ? error.message : String(error) };
  }
}

async function run() {
  const factIndex = JSON.parse(await readFile(join(ROOT, "data/facts/board-fact-packs.json"), "utf8"));
  if (!Array.isArray(factIndex.packs)) throw new Error("fact-pack index lacks packs array");
  const boardIds = factIndex.packs.map((pack) => pack.board_id);
  const health = await healthCheck();
  const client = new McpSseClient({ baseUrl: ENDPOINT, timeoutMs: 30_000 });
  const runStarted = performance.now();
  let listElapsed = 0;
  /** @type {Record<string, unknown>[]} */
  let tools = [];
  /** @type {ReturnType<typeof requireProducts>} */
  let products = [];
  try {
    tools = await client.listTools();
    const listStarted = performance.now();
    const listResult = await client.callTool("list_products", {});
    listElapsed = Math.round(performance.now() - listStarted);
    products = requireProducts(parseToolJson(listResult, "list_products"));
    const mapping = mapBoardsToProducts(boardIds, products);
    const boards = [];

    for (const entry of mapping) {
      const pinReport = sourceQuery(entry.board_id, "pinout");
      const busReport = sourceQuery(entry.board_id, "bus");
      const oursFacts = [...pinReport.facts, ...busReport.facts]
        .filter((fact, index, rows) => !fact.key.endsWith(".unknown") && rows.findIndex((candidate) => candidate.key === fact.key && candidate.value === fact.value) === index);
      const oursProvenance = measureOursProvenance(oursFacts);
      if (!oursProvenance.all_have_url || !oursProvenance.all_have_sha256) {
        throw new Error(`${entry.board_id} has a pin/bus fact without URL+SHA-256 provenance`);
      }
      const oursSignals = extractOursSignals(oursFacts);
      const ours = {
        facts: oursFacts.length,
        comparable_values: oursSignals.length,
        ...oursProvenance,
      };

      if (entry.status !== "mapped" || !entry.official_product) {
        boards.push({
          board_id: entry.board_id,
          mapping: entry,
          coverage_status: "no-official-coverage",
          ours,
          official: { facts: 0, comparable_values: 0, has_provenance: false, has_per_fact_provenance: false, unmeasurable: false },
          value_agreement: { agree: 0, disagree: 0, official_missing: oursSignals.length, ours_missing: 0, rate: null },
          verdict: null,
        });
        continue;
      }

      const callStarted = performance.now();
      try {
        const result = await client.callTool("get_product_specs", { product: entry.official_product });
        const elapsedMs = Math.round(performance.now() - callStarted);
        const texts = toolText(result);
        const rawText = texts.join("\n");
        const parsed = parseSpecs(entry.official_product, result);
        const officialSignals = extractOfficialSignals(parsed);
        const provenance = measureOfficialProvenance(parsed);
        const compared = compareSignals(oursSignals, officialSignals);
        const comparable = compared.counts.agree + compared.counts.disagree;
        const verdict = compared.counts.disagree > 0 || compared.counts.ours_missing > 0 ? "lose" : "win";
        boards.push({
          board_id: entry.board_id,
          mapping: entry,
          coverage_status: "shared",
          ours,
          official: {
            facts: officialStructuredFactCount(parsed),
            comparable_values: officialSignals.length,
            has_provenance: provenance.has_provenance,
            has_per_fact_provenance: provenance.has_per_fact_provenance,
            provenance_observed: provenance.observed_metadata_links,
            unmeasurable: false,
            elapsed_ms: elapsedMs,
            response_sha256: sha256(rawText),
          },
          value_agreement: { ...compared.counts, rate: rate(compared.counts.agree, comparable) },
          comparisons: compared.comparisons,
          ours_missing: compared.ours_missing,
          verdict,
        });
        process.stderr.write(`${entry.board_id} <-> ${entry.official_product}: ${verdict} (${elapsedMs}ms)\n`);
      } catch (error) {
        const rawError = error instanceof Error ? error.message : String(error);
        boards.push({
          board_id: entry.board_id,
          mapping: entry,
          coverage_status: "shared",
          ours,
          official: {
            facts: 0,
            comparable_values: 0,
            has_provenance: false,
            has_per_fact_provenance: false,
            unmeasurable: true,
            elapsed_ms: Math.round(performance.now() - callStarted),
            raw_error: rawError,
          },
          value_agreement: { agree: 0, disagree: 0, official_missing: 0, ours_missing: 0, rate: null },
          verdict: "unmeasurable",
        });
        process.stderr.write(`${entry.board_id} <-> ${entry.official_product}: unmeasurable: ${rawError}\n`);
      }
    }

    const comparedBoards = boards.filter((board) => board.coverage_status === "shared");
    const measurableBoards = comparedBoards.filter((board) => board.verdict !== "unmeasurable");
    const totals = measurableBoards.reduce((sum, board) => ({
      agree: sum.agree + board.value_agreement.agree,
      disagree: sum.disagree + board.value_agreement.disagree,
      official_missing: sum.official_missing + board.value_agreement.official_missing,
      ours_missing: sum.ours_missing + board.value_agreement.ours_missing,
    }), { agree: 0, disagree: 0, official_missing: 0, ours_missing: 0 });
    const oursTotals = boards.reduce((sum, board) => ({
      facts: sum.facts + board.ours.facts,
      url: sum.url + board.ours.url,
      line_range: sum.line_range + board.ours.line_range,
      sha256: sum.sha256 + board.ours.sha256,
      complete: sum.complete + board.ours.url_line_range_sha256,
    }), { facts: 0, url: 0, line_range: 0, sha256: 0, complete: 0 });
    const officialFacts = measurableBoards.reduce((sum, board) => sum + board.official.facts, 0);
    const officialCitedFacts = measurableBoards.reduce((sum, board) => sum + (board.official.has_per_fact_provenance ? board.official.facts : 0), 0);
    const mappedProducts = new Set(mapping.filter((entry) => entry.status === "mapped").map((entry) => entry.official_product));
    const summary = {
      boards_compared: comparedBoards.length,
      boards_measurable: measurableBoards.length,
      verdicts: {
        win: comparedBoards.filter((board) => board.verdict === "win").length,
        lose: comparedBoards.filter((board) => board.verdict === "lose").length,
        unmeasurable: comparedBoards.filter((board) => board.verdict === "unmeasurable").length,
      },
      value_agreement: { ...totals, rate: rate(totals.agree, totals.agree + totals.disagree) },
      provenance: {
        ours_url_sha256_rate: rate(Math.min(oursTotals.url, oursTotals.sha256), oursTotals.facts),
        ours_url_line_range_sha256_rate: rate(oursTotals.complete, oursTotals.facts),
        ours_facts: oursTotals.facts,
        ours_line_anchored_facts: oursTotals.line_range,
        official_per_fact_citation_rate: rate(officialCitedFacts, officialFacts),
        official_facts: officialFacts,
      },
      coverage: {
        ours_registry_entries: boardIds.length,
        official_products: products.length,
        shared_products: mappedProducts.size,
        ours_no_official_coverage: mapping.filter((entry) => entry.status !== "mapped").length,
        official_no_ours_coverage: products.filter((product) => !mappedProducts.has(product.product)).length,
        ours_no_official_board_ids: mapping.filter((entry) => entry.status !== "mapped").map((entry) => entry.board_id),
        official_no_ours_product_ids: products.filter((product) => !mappedProducts.has(product.product)).map((product) => product.product),
      },
      reliability: {
        health,
        list_products_elapsed_ms: listElapsed,
        total_elapsed_ms: Math.round(performance.now() - runStarted),
        successful_spec_calls: measurableBoards.length,
        failed_spec_calls: comparedBoards.length - measurableBoards.length,
        rate_limit_errors: comparedBoards.filter((board) => /429|rate.?limit/i.test(board.official.raw_error ?? "")).length,
      },
    };
    const scorecard = {
      schema_version: 1,
      run_date: "2026-07-14",
      endpoint: ENDPOINT,
      transport: "MCP over SSE",
      protocol_version: "2024-11-05",
      tools_advertised: tools.map((tool) => tool.name).filter((name) => typeof name === "string"),
      methodology: {
        mapping: "normalized exact board/product names; ambiguous prefix matches remain no-official-coverage",
        ours: "lilygo-skills source query for pinout and bus; concrete GPIO/IO assignments normalized to GPIO<n>",
        official: "get_product_specs structured parameters and pinTables; no LLM",
        agreement: "same normalized signal and at least one identical normalized GPIO value across official variants",
        verdict: "lose on any disagreement or official pin signal missing from ours; win otherwise; endpoint/tool failure is unmeasurable",
        provenance: "URL+SHA-256 and URL+line_range+SHA-256 are reported separately; shop links are not fact citations",
      },
      mapping,
      boards,
      summary,
    };
    await writeFile(FIXTURE, `${JSON.stringify(scorecard, null, 2)}\n`);
    await writeFile(REPORT, reportMarkdown(scorecard));
    process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
  } finally {
    await client.close();
  }
}

/** @param {any} scorecard */
function reportMarkdown(scorecard) {
  const { summary } = scorecard;
  const pct = (value) => value === null ? "n/a" : `${(value * 100).toFixed(1)}%`;
  const wins = scorecard.boards.filter((board) => board.verdict === "win").map((board) => board.board_id);
  const losses = scorecard.boards.filter((board) => board.verdict === "lose").map((board) => board.board_id);
  const unmeasurable = scorecard.boards.filter((board) => board.verdict === "unmeasurable").map((board) => `${board.board_id}: ${board.official.raw_error}`);
  return `# M36 official assistant comparison\n\n` +
    `Run date: 2026-07-14. Endpoint: ${scorecard.endpoint}. Transport: MCP-SSE.\n\n` +
    `## Headline\n\n` +
    `- Shared boards compared: ${summary.boards_compared}; measurable: ${summary.boards_measurable}.\n` +
    `- Verdicts: ${summary.verdicts.win} win, ${summary.verdicts.lose} lose, ${summary.verdicts.unmeasurable} unmeasurable.\n` +
    `- Direct signal agreement: ${summary.value_agreement.agree}/${summary.value_agreement.agree + summary.value_agreement.disagree} (${pct(summary.value_agreement.rate)}); disagreements: ${summary.value_agreement.disagree}; official-missing values: ${summary.value_agreement.official_missing}; ours-missing official signals: ${summary.value_agreement.ours_missing}.\n` +
    `- Our URL+SHA-256 provenance rate: ${pct(summary.provenance.ours_url_sha256_rate)}. Our URL+line_range+SHA-256 rate: ${pct(summary.provenance.ours_url_line_range_sha256_rate)} (${summary.provenance.ours_line_anchored_facts}/${summary.provenance.ours_facts}).\n` +
    `- Official per-fact citation rate in get_product_specs responses: ${pct(summary.provenance.official_per_fact_citation_rate)}. Top-level shop links were recorded as product metadata, not citations.\n` +
    `- Coverage: ours has ${summary.coverage.ours_registry_entries} registry entries; official lists ${summary.coverage.official_products} products; ${summary.coverage.shared_products} map unambiguously.\n\n` +
    `## Three-state breakdown\n\n` +
    `Wins (${wins.length}): ${wins.join(", ") || "none"}.\n\n` +
    `Losses (${losses.length}): ${losses.join(", ") || "none"}.\n\n` +
    `Unmeasurable (${unmeasurable.length}): ${unmeasurable.join("; ") || "none"}.\n\n` +
    `No-official-coverage rows are coverage observations, not forced into a comparison verdict: ${summary.coverage.ours_no_official_board_ids.join(", ") || "none"}.\n\n` +
    `## Reliability\n\n` +
    `Health check: ${summary.reliability.health.ok ? "HTTP " + summary.reliability.health.status : "failed"}. ` +
    `${summary.reliability.successful_spec_calls}/${summary.boards_compared} spec calls succeeded; ${summary.reliability.failed_spec_calls} failed; ${summary.reliability.rate_limit_errors} rate-limit errors. ` +
    `list_products took ${summary.reliability.list_products_elapsed_ms} ms; the full run took ${summary.reliability.total_elapsed_ms} ms.\n\n` +
    `## Where the data says we win\n\n` +
    `Our shipped pin/bus facts all carry a source URL and SHA-256, and many carry line ranges. The official structured responses in this run did not attach citations to individual parameter or pin rows. We also retain source-backed pin coverage on boards where the official specs response returned no pin table.\n\n` +
    `## Where the official wins\n\n` +
    `The official catalog is broader (${summary.coverage.official_products} products versus ${summary.coverage.ours_registry_entries} local registry entries), and the loss rows identify official pin signals absent from ours or concrete value disagreements. See the committed fixture for every signal and candidate value.\n\n` +
    `## What is unmeasurable\n\n` +
    `${unmeasurable.length ? "The raw MCP errors above are preserved in the fixture; no official answer was inferred." : "Nothing in the shared-board run was unmeasurable."}\n\n` +
    `## Claims supported for human review\n\n` +
    `The data supports claiming the measured agreement rate, our 100% URL+SHA-256 provenance rate, the separately measured line-anchor rate, the official response's 0% per-fact citation rate in this run, and the observed catalog coverage counts. It does not support claiming that every one of our facts is line-anchored, that our data is universally correct, or that the official service never provides provenance outside these tools/responses.\n`;
}

run().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`);
  process.exitCode = 1;
});
