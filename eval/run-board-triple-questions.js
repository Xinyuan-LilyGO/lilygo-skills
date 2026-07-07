#!/usr/bin/env node
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const boardsArg = valueAfter("--boards") || "all";

function valueAfter(flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
}

function readJson(relative) {
  return JSON.parse(fs.readFileSync(path.join(root, relative), "utf8"));
}

function allFacts(pack) {
  return [
    ...(pack.pin_matrix || []),
    ...(pack.bus_matrix || []),
    ...(pack.expander_matrix || []),
    ...(pack.connector_matrix || []),
    ...(pack.peripheral_table || [])
  ];
}

function factByKey(pack, key) {
  return allFacts(pack).find((fact) => fact.key === key);
}

function factsForTopic(pack, topic) {
  return allFacts(pack).filter((fact) => fact.topic === topic || fact.key.startsWith(`${topic}.`));
}

function hasSource(fact) {
  return Boolean(fact?.source?.path_or_url && fact?.source?.hash);
}

function passFact(pack, key) {
  const fact = factByKey(pack, key);
  return Boolean(fact && hasSource(fact));
}

function passTopic(pack, topic) {
  return factsForTopic(pack, topic).some(hasSource);
}

function passKeyPrefix(pack, prefix) {
  return allFacts(pack).some((fact) => fact.key.startsWith(prefix) && hasSource(fact));
}

function hasDisplayEvidence(pack) {
  return passTopic(pack, "display") || passKeyPrefix(pack, "peripheral.display.");
}

function primaryKind(board, pack) {
  const peripherals = new Set(board.peripherals || []);
  if (!board.supported) return "unsupported";
  if (/t-beam|lora-pager/.test(board.id)) return "radio";
  if (/t-deck|keyboard|encoder/.test(board.id)) return "input";
  if (peripherals.has("display") && hasDisplayEvidence(pack)) return "display";
  if (peripherals.has("lora") && passTopic(pack, "lora")) return "radio";
  if ((peripherals.has("gps") || peripherals.has("gnss")) && passTopic(pack, "gnss")) return "radio";
  if ((peripherals.has("keyboard") || peripherals.has("input") || peripherals.has("touch"))
    && (passTopic(pack, "input") || passTopic(pack, "touch"))) return "input";
  if (peripherals.has("storage") && passTopic(pack, "storage")) return "storage";
  if (peripherals.has("power") && passTopic(pack, "power")) return "power";
  return "generic";
}

function questionsFor(board, pack) {
  const questions = [];
  questions.push({
    id: "support-boundary",
    kind: "support",
    pass: Boolean(pack && typeof pack.supported === "boolean" && (pack.source_refs || []).some((ref) => ref.path_or_url && ref.hash))
  });
  const peripherals = new Set(board.peripherals || []);
  const kind = primaryKind(board, pack);
  if (kind === "unsupported") {
    questions.push({
      id: "unsupported-no-deep-injection",
      kind,
      pass: pack.supported === false
    });
    questions.push({
      id: "unsupported-source-boundary",
      kind,
      pass: (pack.source_refs || []).some((ref) => ref.path_or_url && ref.hash)
    });
    return questions;
  }
  if (kind === "radio") {
    questions.push({
      id: "radio-chip-and-bus",
      kind,
      pass: passFact(pack, "lora.chip") || passTopic(pack, "lora") || passTopic(pack, "gnss")
    });
    questions.push({
      id: "radio-gnss-or-antenna-caveat",
      kind,
      pass: passFact(pack, "lora.antenna") || passFact(pack, "gnss.bus_or_interface") || passTopic(pack, "gnss")
    });
    return questions;
  }
  if (kind === "input") {
    questions.push({
      id: "input-controller",
      kind,
      pass: passFact(pack, "input.chip") || passTopic(pack, "input") || passTopic(pack, "touch")
    });
    questions.push({
      id: "input-bus-or-pins",
      kind,
      pass: passFact(pack, "input.bus_or_interface") || passTopic(pack, "input") || passTopic(pack, "touch")
    });
    return questions;
  }
  if (kind === "display") {
    questions.push({
      id: "display-panel-and-bus",
      kind,
      pass: (passFact(pack, "display.panel_or_chip") || passKeyPrefix(pack, "peripheral.display."))
        && (passFact(pack, "display.bus_or_interface") || passKeyPrefix(pack, "bus.qspi.") || passKeyPrefix(pack, "bus.display."))
    });
    questions.push({
      id: "display-power-or-debug",
      kind,
      pass: passFact(pack, "display.backlight_or_power") || passTopic(pack, "display") || passKeyPrefix(pack, "peripheral.display.")
    });
    return questions;
  }
  if (kind === "storage") {
    questions.push({
      id: "storage-interface",
      kind,
      pass: passFact(pack, "storage.interface") || passTopic(pack, "storage")
    });
    questions.push({
      id: "not-display-centric",
      kind,
      pass: !peripherals.has("display")
    });
    return questions;
  }
  if (kind === "power") {
    questions.push({
      id: "power-manager",
      kind,
      pass: passFact(pack, "power.manager") || passTopic(pack, "power")
    });
    questions.push({
      id: "not-display-centric",
      kind,
      pass: !peripherals.has("display")
    });
    return questions;
  }
  questions.push({
    id: "primary-peripheral-source",
    kind,
    pass: passFact(pack, "peripheral.primary") || (pack.source_refs || []).length > 0
  });
  questions.push({
    id: "no-forced-display-question",
    kind,
    pass: true
  });
  return questions;
}

const boardIndex = readJson("data/boards.json").boards;
const factIndex = readJson("data/facts/board-fact-packs.json").packs;
const factByBoard = new Map(factIndex.map((pack) => [pack.board_id, pack]));
const selected = boardsArg === "all" ? boardIndex : boardIndex.filter((board) => boardsArg.split(",").includes(board.id));
const results = selected.map((board) => {
  const pack = factByBoard.get(board.id);
  const questions = questionsFor(board, pack);
  return {
    board_id: board.id,
    primary_kind: primaryKind(board, pack),
    questions,
    pass: questions.length === 3 && questions.every((question) => question.pass)
  };
});

const failed = results.filter((result) => !result.pass);
const displayCentricMisapplied = results.filter((result) => {
  const board = boardIndex.find((entry) => entry.id === result.board_id);
  const hasDisplay = (board.peripherals || []).includes("display");
  return !hasDisplay && result.questions.some((question) => question.kind === "display");
});

if (failed.length || displayCentricMisapplied.length) {
  console.error(JSON.stringify({
    status: "FAIL",
    failed,
    display_centric_misapplied: displayCentricMisapplied
  }, null, 2));
  process.exit(1);
}

const kinds = results.reduce((acc, result) => {
  acc[result.primary_kind] = (acc[result.primary_kind] || 0) + 1;
  return acc;
}, {});
console.log(JSON.stringify({
  status: "PASS",
  boards: results.length,
  questions: results.length * 3,
  kinds,
  display_centric_misapplied: 0
}, null, 2));
