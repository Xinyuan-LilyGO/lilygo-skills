// `source query --board <id> --topic <topic> --json`.
//
// Pulls topic-scoped, source-cited facts straight from the committed fact pack
// (values are never inlined here) and attaches an honest readiness signal.
// Mirrors Rust facts/mod.rs::source_query + facts/completeness.rs.
import {
  getPack, getBoard, factsForTopic, normalizeTopic, unknownTopicFact,
  isReadinessTopic, promptKeywords, topicFields, MAX_DISCOVERY_HINTS_INLINE, isMain,
} from "./lib.mjs";
import { ensureOnDemandPinout } from "./on-demand-ingest.mjs";

/**
 * @param {string} boardId
 * @param {string} rawTopic
 * @returns {FactQueryReport}
 */
export function sourceQuery(boardId, rawTopic) {
  const topic = normalizeTopic(rawTopic);
  const pack = getPack(boardId);
  if (!pack) throw new Error(`unknown board fact pack: ${boardId}`);
  return sourceQueryFromPack(boardId, topic, pack, getBoard(boardId));
}

/**
 * Query committed data first, then run the guarded dynamic path only on a
 * local cache miss.
 * @param {string} boardId
 * @param {string} rawTopic
 * @param {import("./on-demand-ingest.mjs").OnDemandOptions} [options]
 * @returns {Promise<FactQueryReport | HonestDegradeReport>}
 */
export async function sourceQueryWithOnDemand(boardId, rawTopic, options = {}) {
  const topic = normalizeTopic(rawTopic);
  const localPack = getPack(boardId);
  if (localPack) return sourceQueryFromPack(boardId, topic, localPack, getBoard(boardId));
  const verdict = await ensureOnDemandPinout(boardId, options);
  if (verdict.status === "verified") {
    const report = sourceQueryFromPack(verdict.board_id, topic, verdict.fact_pack, undefined);
    report.on_demand = {
      cache_status: verdict.cache_status,
      repo_url: verdict.repo_url,
      source_path: verdict.source.path,
      gates: verdict.gates,
    };
    return report;
  }
  return honestDegradeReport(boardId, topic, verdict);
}

/**
 * @param {string} boardId
 * @param {string} topic
 * @param {FactPack} pack
 * @param {BoardRecord | undefined} board
 * @returns {FactQueryReport}
 */
function sourceQueryFromPack(boardId, topic, pack, board) {
  let facts = factsForTopic(pack, topic);
  if (facts.length === 0) facts = [unknownTopicFact(pack, topic)];
  const unknowns = facts.filter((fact) => fact.confidence === "unknown_with_sources");
  /** @type {FactQueryReport} */
  const report = {
    status: pack.supported ? "PASS" : "UNSUPPORTED",
    board_id: boardId,
    topic,
    supported: pack.supported,
    fact_pack: pack,
    facts,
    unknowns,
    conflicts: pack.conflicts,
    source_refs: pack.source_refs,
    ...(board && isReadinessTopic(topic) ? { completeness: completenessSignal(boardId, topic) } : {}),
    discovery_hints: discoveryHints(boardId, topic, true),
    warnings: queryWarnings(pack),
  };
  return report;
}

/**
 * @param {string} requestedBoard
 * @param {string} topic
 * @param {import("./on-demand-ingest.mjs").DegradedIngestVerdict} verdict
 * @returns {HonestDegradeReport}
 */
function honestDegradeReport(requestedBoard, topic, verdict) {
  return {
    status: "NO_VERIFIABLE_PINOUT",
    board_id: requestedBoard,
    resolved_board_id: verdict.board_id,
    topic,
    supported: false,
    repo_url: verdict.repo_url,
    reason: verdict.reason,
    message: verdict.message,
    facts: [],
    pin_matrix: [],
    source_refs: [],
    conflicts: [],
    warnings: ["No pin value is served unless every dynamic-ingest gate passes."],
  };
}

/**
 * @param {FactPack} pack
 * @returns {string[]}
 */
function queryWarnings(pack) {
  if (!pack.supported) {
    return ["unsupported LilyGO product boundary: runnable guidance is limited to ESP32-family boards"];
  }
  return [
    "source query returns V3 source/context evidence, not a successful firmware run",
    "unknown_with_sources means the current source cache has pointers but no exact actionable value",
  ];
}

/**
 * @param {string} boardId
 * @param {string} topic
 * @param {boolean} includeUnknownHint
 * @returns {DiscoveryHint[]}
 */
function discoveryHints(boardId, topic, includeUnknownHint) {
  /** @type {DiscoveryHint[]} */
  const hints = [{
    when: "need source-backed board facts before writing firmware",
    action: "run_command",
    command: `lilygo-skills source query --board ${boardId} --topic ${topic} --json`,
    reason: "Fetch the full fact pack on demand instead of inlining every table.",
  }];
  if (includeUnknownHint) {
    hints.push({
      when: "a fact is unknown or ambiguous",
      action: "run_command",
      command: `lilygo-skills source query --board ${boardId} --topic expander --json`,
      reason: "Check the expander table and source refs before assigning XL9555 channels.",
    });
  }
  return hints.slice(0, MAX_DISCOVERY_HINTS_INLINE);
}

// --- completeness (mirrors Rust facts/completeness.rs) ---------------------

/** @type {Record<string, string[]>} */
const DEMO_TOPIC_NEEDLES = {
  display: ["display", "lvgl", "screen", "factory"],
  imu: ["imu", "bhi260", "sensor", "factory"],
  power: ["power", "battery", "factory"],
  lora: ["lora", "radio", "sx1262", "sx1268", "sx1276", "sx1278", "sx1280", "factory"],
  gnss: ["gnss", "gps", "mia-m10", "factory"],
  nfc: ["nfc", "st25r3916", "rfal", "factory"],
  input: ["input", "keyboard", "button", "touch", "factory"],
};

/**
 * @param {DemoRef} demo
 * @param {string} topic
 * @returns {boolean}
 */
function demoMatchesTopic(demo, topic) {
  const target = `${demo.target} ${demo.path}`.toLowerCase();
  const needles = DEMO_TOPIC_NEEDLES[topic] ?? [topic, "factory"];
  return needles.some((needle) => target.includes(needle));
}

/**
 * @param {Fact} fact
 * @returns {boolean}
 */
function isKnownFact(fact) {
  return fact.confidence !== "unknown_with_sources" && fact.value !== "unknown_with_sources";
}

/**
 * @param {string} topic
 * @param {Fact} fact
 * @returns {boolean}
 */
function isKnownTopicFact(topic, fact) {
  if (!isKnownFact(fact) || fact.key.startsWith("framework.")) return false;
  const haystack = `${fact.topic} ${fact.key} ${fact.value}`.toLowerCase();
  const needles = promptKeywords().topics[topic];
  return (needles?.some((needle) => haystack.includes(needle)) ?? false) || haystack.includes(topic);
}

/**
 * @param {string} topic
 * @returns {string[]}
 */
function requiredFields(topic) {
  const rules = topicFields();
  return rules.required[topic] ?? rules.generic_required.map((field) => field.replace("{topic}", topic));
}

/**
 * Present-field detection (mirrors Rust present_fields + add_* helpers).
 * @param {BoardRecord} board
 * @param {FactPack} pack
 * @param {string} topic
 * @param {Fact[]} facts completeness facts (topic facts + generated)
 * @returns {Set<string>}
 */
function presentFields(board, pack, topic, facts) {
  /** @type {Set<string>} */
  const present = new Set();
  if (pack.source_refs.length > 0) present.add("source_refs");
  const hasDemo = board.demo_refs.some((demo) => demoMatchesTopic(demo, topic));
  const factHasDemo = facts.some((f) => f.key === "framework.demo_refs" && isKnownFact(f));
  const factHasBuildHint = facts.some((f) => f.key === "framework.build_hint" && isKnownFact(f));
  if (hasDemo || factHasDemo) present.add("framework.demo_refs");
  if ((hasDemo && board.frameworks.length > 0) || factHasBuildHint) present.add("framework.build_hint");
  if (topic === "display" && board.source_urls.length > 0) present.add("debug.blank_screen_hints");
  if (topic === "display") {
    addDisplayFields(board, facts, present);
  } else if (facts.some((f) => isKnownTopicFact(topic, f))) {
    present.add(`${topic}.chip`);
    present.add(`${topic}.bus_or_interface`);
  }
  return present;
}

/**
 * @param {BoardRecord} board
 * @param {Fact[]} facts
 * @param {Set<string>} present
 */
function addDisplayFields(board, facts, present) {
  const display = board.peripheral_matrix.find((p) => p.category === "display");
  if (display) {
    if (display.chip) present.add("display.panel_or_chip");
    if (display.bus) present.add("display.bus_or_interface");
  }
  if (
    board.peripheral_matrix.some((p) => p.category === "power") ||
    board.demo_refs.some((d) => d.target.toLowerCase().includes("brightness"))
  ) {
    present.add("display.backlight_or_power");
  }
  if (board.peripheral_matrix.some((p) => p.category === "touch")) present.add("display.touch");
  /** @type {Record<string, string>} */
  const displayKeys = {
    "display.panel_or_chip": "display.panel_or_chip",
    "display.bus_or_interface": "display.bus_or_interface",
    "display.backlight_or_power": "display.backlight_or_power",
    "display.resolution": "display.resolution",
    "display.touch": "display.touch",
  };
  for (const fact of facts.filter(isKnownFact)) {
    const mapped = displayKeys[fact.key];
    if (mapped) present.add(mapped);
  }
}

/**
 * @param {BoardRecord} board
 * @param {FactPack} pack
 * @param {string} topic
 * @returns {Fact[]}
 */
function generatedCompletenessFacts(board, pack, topic) {
  const source = pack.source_refs[0];
  if (!source) return [];
  /** @type {Fact[]} */
  const facts = [];
  if (board.demo_refs.some((demo) => demoMatchesTopic(demo, topic))) {
    facts.push(mkFact(board, topic, "framework.demo_refs", "official demo refs present", source, "derived"));
    facts.push(mkFact(board, topic, "framework.build_hint", `frameworks=${board.frameworks.join(",")}`, source, "derived"));
  }
  if (topic === "display" && board.source_urls.length > 0) {
    facts.push(mkFact(board, topic, "debug.blank_screen_hints", "check power/backlight, bus init, reset, color order, LVGL tick/flush", source, "derived"));
  }
  return facts;
}

/**
 * @param {BoardRecord} board
 * @param {string} topic
 * @param {string} key
 * @param {string} value
 * @param {SourceRef} source
 * @param {string} confidence
 * @returns {Fact}
 */
function mkFact(board, topic, key, value, source, confidence) {
  return {
    schema_version: 1, board_id: board.id, topic, key, value,
    claim: "generated completeness fact", source,
    authority_rank: 0, evidence_level: "V3-source-reference", stale: false, confidence,
  };
}

/**
 * Build the readiness signal for a supported board/topic (mirrors Rust
 * signal_from_report over evaluate_completeness).
 * @param {string} boardId
 * @param {string} topic
 * @returns {CompletenessSignal}
 */
function completenessSignal(boardId, topic) {
  const board = getBoard(boardId);
  const pack = getPack(boardId);
  const source_query_command = `lilygo-skills source query --board ${boardId} --topic ${topic} --json`;
  // Unsupported boundary or missing board: no readiness proof, all required missing.
  if (!board || !pack || !pack.supported) {
    return finishSignal(boardId, topic, "unsupported", requiredFields(topic), source_query_command);
  }
  const facts = [...factsForTopic(pack, topic), ...generatedCompletenessFacts(board, pack, topic)];
  const required = requiredFields(topic);
  const present = presentFields(board, pack, topic, facts);
  const requiredMissing = required.filter((field) => !present.has(field));
  const completeness = requiredMissing.length === 0
    ? "complete"
    : pack.source_refs.length > 0 ? "needs_source_ingestion" : "partial";
  return finishSignal(boardId, topic, completeness, requiredMissing, source_query_command);
}

/**
 * Assemble a CompletenessSignal in Rust field order: update_command (only when
 * ingestion is needed) precedes required_missing, which is omitted when empty.
 * @param {string} boardId
 * @param {string} topic
 * @param {string} completeness
 * @param {string[]} requiredMissing
 * @param {string} source_query_command
 * @returns {CompletenessSignal}
 */
function finishSignal(boardId, topic, completeness, requiredMissing, source_query_command) {
  /** @type {CompletenessSignal} */
  const signal = { board_id: boardId, topic, completeness, evidence_level: "V3", source_query_command };
  if (completeness === "needs_source_ingestion") {
    signal.update_command =
      `lilygo-skills update board-facts --board ${boardId} --topic ${topic} --dry-run --json`;
  }
  if (requiredMissing.length > 0) signal.required_missing = requiredMissing;
  return signal;
}

// --- CLI -------------------------------------------------------------------

/**
 * Parse `[source] [query] --board X --topic Y [--json]` argv tail.
 * @param {string[]} argv
 * @returns {{ board?: string, topic?: string, json: boolean }}
 */
export function parseQueryArgs(argv) {
  const args = argv[0] === "source" && argv[1] === "query" ? argv.slice(2)
    : argv[0] === "query" ? argv.slice(1) : argv;
  /** @type {{ board?: string, topic?: string, json: boolean }} */
  const out = { json: false };
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--board") out.board = args[++i];
    else if (args[i] === "--topic") out.topic = args[++i];
    else if (args[i] === "--json") out.json = true;
  }
  return out;
}

/**
 * @param {string[]} argv
 * @returns {Promise<number>} exit code
 */
export async function runSourceQuery(argv) {
  const { board, topic, json } = parseQueryArgs(argv);
  if (!board || !topic) {
    process.stderr.write("usage: source query --board <id> --topic <topic> --json\n");
    return 2;
  }
  let report;
  try {
    report = await sourceQueryWithOnDemand(board, topic);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
  if (json) process.stdout.write(JSON.stringify(report, null, 2) + "\n");
  else process.stdout.write(`${report.board_id}/${report.topic}: ${report.status} (${report.facts.length} facts)\n`);
  return 0;
}

if (isMain(import.meta.url)) {
  runSourceQuery(process.argv.slice(2)).then((code) => { process.exitCode = code; });
}
