#!/usr/bin/env node
// Maintainer export for verified runtime cache packs. This command never
// extracts or maps pins itself: it accepts only cache verdicts that already
// passed the runtime provenance, authority, and auto-map gates.
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { getBoardRegistry } from "../bin/board-registry.mjs";
import { provenanceGatePasses, readIngestVerdict } from "../bin/on-demand-ingest.mjs";

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const args = process.argv.slice(2);
const cacheDir = valueAfter("--cache-dir");
const write = args.includes("--write");
const jsonOut = args.includes("--json");

/** @typedef {{ board_id: string; mcu: string; mcu_evidence_url: string }} WarmupMetadata */
/** @typedef {{ schema_version: number; boards: WarmupMetadata[] }} WarmupConfig */
/** @typedef {{ board_id: string; url: string; source_kind?: string; line_range?: string; topic?: string; authority_rank?: number; auto_pins?: boolean; buses?: unknown[] }} ManifestSource */

if (!cacheDir) {
  process.stderr.write("usage: export-on-demand-cache --cache-dir <m38-cache-root> [--write] [--json]\n");
  process.exitCode = 2;
} else {
  await main(cacheDir);
}

/** @param {string} root */
async function main(root) {
  const config = /** @type {WarmupConfig} */ (await readJson("pipeline/warmup-auto-boards.json"));
  const boardData = /** @type {BoardIndex} */ (await readJson("data/boards.json"));
  const factData = /** @type {FactPackIndex} */ (await readJson("data/facts/board-fact-packs.json"));
  const sniffData = /** @type {SniffRules} */ (await readJson("data/sniff-rules.json"));
  const manifest = /** @type {{ schema_version: number; note?: string; sources: ManifestSource[] }} */ (
    await readJson("pipeline/source-manifest.json")
  );
  const registry = await getBoardRegistry({ cacheDir: root, networkEnabled: false, maxAgeMs: Number.POSITIVE_INFINITY });
  const boardById = new Map(boardData.boards.map((board) => [board.id, board]));
  const packById = new Map(factData.packs.map((pack) => [pack.board_id, pack]));
  const sniffById = new Map(sniffData.boards.map((board) => [board.board_id, board]));
  const sourceById = new Map(manifest.sources.map((source) => [source.board_id, source]));
  const exported = [];

  for (const metadata of config.boards) {
    const verdict = await readIngestVerdict(root, metadata.board_id);
    if (!verdict || verdict.status !== "verified") {
      throw new Error(`${metadata.board_id}: no verified cache verdict`);
    }
    if (!provenanceGatePasses(verdict.fact_pack, verdict.repo_url)) {
      throw new Error(`${metadata.board_id}: cached pack failed provenance recheck`);
    }
    const registryBoard = registry.boards.find((board) => board.id === metadata.board_id);
    if (!registryBoard || registryBoard.official_repo !== verdict.repo_url) {
      throw new Error(`${metadata.board_id}: cache repository does not match the official registry`);
    }

    const pack = {
      ...verdict.fact_pack,
      mcu_family: metadata.mcu,
      supported: metadata.mcu.startsWith("esp32"),
    };
    const board = boardRecord(registryBoard, verdict, metadata);
    const sniff = sniffRecord(board);
    const source = manifestSource(verdict);
    boardById.set(metadata.board_id, board);
    packById.set(metadata.board_id, pack);
    sniffById.set(metadata.board_id, sniff);
    sourceById.set(metadata.board_id, source);
    exported.push({
      board_id: metadata.board_id,
      pins: pack.pin_matrix.length,
      source: verdict.source.path,
      sha256: verdict.source.sha256,
      line_range: verdict.source.line_range,
    });
  }

  boardData.boards = upsertOrder(boardData.boards, boardById, config.boards.map((board) => board.board_id));
  factData.packs = upsertOrder(factData.packs, packById, config.boards.map((board) => board.board_id), "board_id");
  sniffData.boards = upsertOrder(sniffData.boards, sniffById, config.boards.map((board) => board.board_id), "board_id");
  manifest.sources = upsertOrder(manifest.sources, sourceById, config.boards.map((board) => board.board_id), "board_id");

  if (write) {
    await writeJson("data/boards.json", boardData);
    await writeJson("data/facts/board-fact-packs.json", factData);
    await writeJson("data/sniff-rules.json", sniffData);
    await writeJson("pipeline/source-manifest.json", manifest);
  }
  const report = { status: "PASS", mode: write ? "written" : "dry-run", exported };
  process.stdout.write(jsonOut ? `${JSON.stringify(report, null, 2)}\n` : `${report.mode}: ${exported.length} boards\n`);
}

/** @param {import("../bin/board-registry.mjs").BoardRegistryEntry} registryBoard @param {import("../bin/on-demand-ingest.mjs").VerifiedIngestVerdict} verdict @param {{ board_id: string; mcu: string; mcu_evidence_url: string }} metadata */
function boardRecord(registryBoard, verdict, metadata) {
  const aliases = unique([
    ...registryBoard.aliases,
    registryBoard.product_name,
    registryBoard.repository_name || "",
  ]);
  const pinKeys = verdict.fact_pack.pin_matrix.map((fact) => fact.key);
  /** @type {string[]} */
  const peripherals = [];
  if (pinKeys.some((key) => key.includes(".display."))) peripherals.push("display");
  if (pinKeys.some((key) => key.includes(".lora."))) peripherals.push("lora");
  if (pinKeys.some((key) => key.includes(".touch."))) peripherals.push("touch");
  if (pinKeys.some((key) => key.includes(".input.") || key.includes(".keyboard.") || key.includes(".button."))) peripherals.push("input");
  if (pinKeys.some((key) => key.includes(".power."))) peripherals.push("power");
  return {
    id: metadata.board_id,
    family_id: null,
    product: true,
    display_name: registryBoard.product_name,
    aliases,
    mcu: metadata.mcu,
    supported: metadata.mcu.startsWith("esp32"),
    frameworks: ["arduino", "platformio"],
    peripherals,
    repo_url: verdict.repo_url,
    wiki_url: "",
    source_status: "github-live-gated",
    source_urls: [
      { kind: "arduino-pins", url: verdict.source.url, status: "github-live-gated" },
      { kind: "github-repo", url: verdict.repo_url, status: "github-live" },
      { kind: "mcu-evidence", url: metadata.mcu_evidence_url, status: "official" },
    ],
    source_hashes: { "arduino-pins": verdict.source.sha256.replace(/^sha256:/, "") },
    stale: false,
    peripheral_matrix: [],
    demo_refs: [{
      framework: "source",
      target: "official-pin-source",
      source_url: verdict.repo_url,
      path: verdict.source.path,
      stale: false,
      source_status: "github-live-gated",
      evidence_level: "V3-source-reference",
    }],
    warnings: ["Pin coverage is verified; peripherals not represented by the gated pin map remain unclassified."],
  };
}

/** @param {import("../bin/on-demand-ingest.mjs").VerifiedIngestVerdict} verdict */
function manifestSource(verdict) {
  const repo = new URL(verdict.repo_url).pathname.split("/").filter(Boolean);
  const rawPath = verdict.source.path.split("/").map(encodeURIComponent).join("/");
  return {
    board_id: verdict.board_id,
    url: `https://raw.githubusercontent.com/${repo[0]}/${repo[1]}/${verdict.source.commit}/${rawPath}`,
    source_kind: "arduino-pins",
    line_range: verdict.source.line_range,
    topic: "pinout",
    authority_rank: 95,
    auto_pins: true,
    buses: [],
  };
}

/** @param {{ id: string; aliases: string[] }} board */
function sniffRecord(board) {
  return {
    board_id: board.id,
    aliases: unique([board.id, ...board.aliases])
      .map((alias) => alias.toLowerCase().replace(/[^a-z0-9]+/g, ""))
      .filter((alias) => alias.length >= 4),
  };
}

/** @param {any[]} original @param {Map<string, any>} byId @param {string[]} ids @param {string} [key] */
function upsertOrder(original, byId, ids, key = "id") {
  const wanted = new Set(ids);
  return [
    ...original.filter((entry) => !wanted.has(entry[key])),
    ...ids.map((id) => byId.get(id)),
  ];
}

/** @param {string[]} values */
function unique(values) {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}

/** @param {string} flag */
function valueAfter(flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
}

/** @param {string} relative */
async function readJson(relative) {
  return JSON.parse(await readFile(join(ROOT, relative), "utf8"));
}

/** @param {string} relative @param {unknown} value */
async function writeJson(relative, value) {
  await writeFile(join(ROOT, relative), `${JSON.stringify(value, null, 2)}\n`, "utf8");
}
