// Data-reading layer for the LilyGO JS context kernel.
//
// This module NEVER inlines a pin value; it only reads and shapes the committed
// data under data/**. Behavior mirrors the Rust facts/{mod,build}.rs so the JS
// core is contract-compatible with target/release/lilygo-skills.
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

/** Package root (bin/'s parent): data/** and pipeline/** live under it. */
export const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
export const FACT_PACK_INDEX_PATH = "data/facts/board-fact-packs.json";
export const DOCUMENTATION_REPO = "https://github.com/Xinyuan-LilyGO/documentation";
/** Inline discovery-hint budget (mirrors Rust ContextBudget default). */
const MAX_DISCOVERY_HINTS_INLINE = 2;

/**
 * @param {string} rel repo-relative path
 * @returns {string} absolute path anchored at the package root (never cwd)
 */
export function dataPath(rel) {
  return join(ROOT, rel);
}

/**
 * @param {string} rel
 * @returns {unknown}
 */
export function readJson(rel) {
  return JSON.parse(readFileSync(dataPath(rel), "utf8"));
}

/** @type {FactPackIndex | undefined} */
let factPackCache;
/** @returns {FactPackIndex} */
export function loadFactPacks() {
  return (factPackCache ??= /** @type {FactPackIndex} */ (readJson(FACT_PACK_INDEX_PATH)));
}

/**
 * @param {string} boardId
 * @returns {FactPack | undefined}
 */
export function getPack(boardId) {
  return loadFactPacks().packs.find((pack) => pack.board_id === boardId);
}

/** @type {BoardIndex | undefined} */
let boardCache;
/** @returns {BoardIndex} */
export function loadBoards() {
  return (boardCache ??= /** @type {BoardIndex} */ (readJson("data/boards.json")));
}

/**
 * @param {string} boardId
 * @returns {BoardRecord | undefined}
 */
export function getBoard(boardId) {
  return loadBoards().boards.find((board) => board.id === boardId);
}

/** @type {{ fact_prompt: string[]; implementation_or_debug: string[]; bus_topic_order: string[]; bus_topics: Record<string, string[]>; topic_order: string[]; topics: Record<string, string[]> } | undefined} */
let promptKeywordsCache;
export function promptKeywords() {
  return (promptKeywordsCache ??= /** @type {NonNullable<typeof promptKeywordsCache>} */ (
    readJson("data/facts/prompt-keywords.json")
  ));
}

/** @type {{ generic_required: string[]; generic_preferred: string[]; required: Record<string, string[]>; preferred: Record<string, string[]> } | undefined} */
let topicFieldsCache;
export function topicFields() {
  return (topicFieldsCache ??= /** @type {NonNullable<typeof topicFieldsCache>} */ (
    readJson("data/facts/topic-fields.json")
  ));
}

/**
 * Lowercase, collapse every run of non-alphanumerics into a single `-`, and
 * trim separators (byte-identical to Rust text_match::slug).
 * @param {string} value
 * @returns {string}
 */
export function slug(value) {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/**
 * sha256 of `text` as `sha256:<hex>` (matches Rust verify_sources::sha256_hex).
 * @param {string} text
 * @returns {string}
 */
export function sha256Hex(text) {
  return "sha256:" + createHash("sha256").update(text, "utf8").digest("hex");
}

/**
 * Authority rank by source kind (mirrors Rust source_authority_rank).
 * @param {string} kind
 * @returns {number}
 */
export function sourceAuthorityRank(kind) {
  switch (kind) {
    case "official-code": return 100;
    case "driver-header": case "arduino-pins": return 95;
    case "hardware-doc": return 90;
    case "github-repo": case "quick-start": return 85;
    case "chip-vendor": case "framework-official": return 80;
    case "documentation-repo": return 70;
    case "wiki": return 55;
    case "local-reference": return 45;
    case "community": return 20;
    default: return 10;
  }
}

/**
 * Canonicalize a topic alias to its query topic (mirrors Rust normalize_topic).
 * @param {string} topic
 * @returns {string}
 * @throws {Error} on an empty topic
 */
export function normalizeTopic(topic) {
  const normalized = slug(topic);
  if (!normalized) throw new Error("empty source topic");
  /** @type {Record<string, string>} */
  const map = {
    pin: "pinout", pins: "pinout", iic: "i2c", "serial-bus": "uart",
    socket: "connector", peripherals: "peripheral", lvgl: "display",
    screen: "display", lcd: "display", amoled: "display", gesture: "imu",
    pmu: "power", battery: "power", gps: "gnss", rfid: "nfc",
    keyboard: "input", button: "input",
  };
  return map[normalized] ?? normalized;
}

/** Topics that carry no completeness/readiness signal (mirrors Rust). */
const NON_READINESS = new Set([
  "io", "pinout", "bus", "i2c", "spi", "uart", "i2s", "gpio",
  "expander", "connector", "peripheral",
]);
/**
 * @param {string} topic
 * @returns {boolean}
 */
export function isReadinessTopic(topic) {
  return !NON_READINESS.has(topic);
}

/**
 * @param {Fact} fact
 * @param {string} substr already-lowercased needle
 */
function factHaystackHas(fact, substr) {
  return `${fact.topic} ${fact.key} ${fact.value}`.toLowerCase().includes(substr);
}

/**
 * Substring topic-needle filter over the non-pin tables (mirrors Rust
 * topic_facts; note Rust uses plain substring here, not word boundaries).
 * @param {FactPack} pack
 * @param {string[]} needles
 * @returns {Fact[]}
 */
function topicFacts(pack, needles) {
  const rows = [
    ...pack.peripheral_table, ...pack.bus_matrix,
    ...pack.expander_matrix, ...pack.connector_matrix,
  ];
  return rows.filter((fact) => {
    const value = `${fact.topic} ${fact.key}`.toLowerCase();
    return needles.some((needle) => value.includes(needle));
  });
}

/**
 * @param {string} topic
 * @returns {string[]}
 */
function topicNeedles(topic) {
  const set = new Set([topic]);
  for (const needle of promptKeywords().topics[topic] ?? []) set.add(slug(needle));
  return [...set].filter((needle) => needle !== "");
}

/**
 * @param {FactPack} pack
 * @param {string} topic
 * @returns {Fact[]}
 */
function busTopicFacts(pack, topic) {
  return topicFacts(pack, [topic]).filter((fact) => factHaystackHas(fact, topic));
}

/**
 * @param {FactPack} pack
 * @returns {Fact[]}
 */
function gpioFacts(pack) {
  const rows = [...pack.pin_matrix, ...pack.expander_matrix, ...pack.connector_matrix];
  const needles = ["gpio", "pin", "io", "xl9555", "connector"];
  return rows.filter((fact) => needles.some((needle) => factHaystackHas(fact, needle)));
}

/**
 * Select and sort the facts for a query topic (mirrors Rust facts_for_topic:
 * authority_rank desc, then key asc).
 * @param {FactPack} pack
 * @param {string} topic canonical topic (already normalized)
 * @returns {Fact[]}
 */
export function factsForTopic(pack, topic) {
  /** @type {Fact[]} */
  let facts;
  switch (topic) {
    case "io":
      facts = [
        ...pack.pin_matrix, ...pack.bus_matrix, ...pack.expander_matrix,
        ...pack.connector_matrix, ...pack.peripheral_table,
      ];
      break;
    case "pinout": facts = [...pack.pin_matrix]; break;
    case "bus": facts = [...pack.bus_matrix]; break;
    case "i2c": case "spi": case "uart": case "i2s":
      facts = busTopicFacts(pack, topic); break;
    case "gpio": facts = gpioFacts(pack); break;
    case "expander": facts = [...pack.expander_matrix]; break;
    case "connector": facts = [...pack.connector_matrix]; break;
    case "peripheral": facts = [...pack.peripheral_table]; break;
    default: facts = topicFacts(pack, topicNeedles(topic));
  }
  facts.sort((a, b) => b.authority_rank - a.authority_rank || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
  return facts;
}

/**
 * The honest fallback fact when a topic maps to nothing (mirrors Rust
 * unknown_topic_fact).
 * @param {FactPack} pack
 * @param {string} topic
 * @returns {Fact}
 */
export function unknownTopicFact(pack, topic) {
  const source = pack.source_refs[0] ?? {
    kind: "documentation-repo",
    path_or_url: DOCUMENTATION_REPO,
    hash: "sha256:unknown",
  };
  return {
    schema_version: 1,
    board_id: pack.board_id,
    topic,
    key: `${topic}.unknown`,
    value: "unknown_with_sources",
    claim: `Current source fact pack has no exact ${topic} mapping; inspect source refs before assigning pins.`,
    authority_rank: sourceAuthorityRank(source.kind),
    evidence_level: "V3-source-reference",
    stale: false,
    confidence: "unknown_with_sources",
    source,
  };
}

export { MAX_DISCOVERY_HINTS_INLINE };
