const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const GENERATED_PATH = path.join(ROOT, ".tmp/pipeline/board-fact-packs.generated.json");
const FACT_PACK_PATH = path.join(ROOT, "data/facts/board-fact-packs.json");
const BOARD_PATH = path.join(ROOT, "data/boards.json");
const DOCUMENTATION_REPO = "https://github.com/Xinyuan-LilyGO/documentation";
const TFT_ESPI_CORE_PITFALL =
  "https://github.com/Bodmer/TFT_eSPI/issues/3329";
const GOLD_BOARDS = [
  "board-t-display-s3",
  "board-t-watch-ultra",
  "board-t-beam",
  "board-t-deck",
  "board-t-watch-2021"
];

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value, null, 2) + "\n");
}

function sourceHash(text) {
  return `sha256:${crypto.createHash("sha256").update(text).digest("hex")}`;
}

function sourceFromUrl(kind, url, providedHash) {
  return {
    kind,
    path_or_url: url || DOCUMENTATION_REPO,
    hash: providedHash || sourceHash(url || DOCUMENTATION_REPO)
  };
}

function sourceRefsForBoard(board, pack) {
  const refs = new Map();
  for (const ref of pack?.source_refs || []) {
    if (ref?.path_or_url) refs.set(ref.path_or_url, ref);
  }
  const addRef = (url, ref) => {
    if (url && !refs.has(url)) refs.set(url, ref);
  };
  for (const source of board.source_urls || []) {
    const hash = board.source_hashes?.[source.kind] || board.source_hashes?.repo || board.source_hashes?.wiki;
    addRef(source.url, sourceFromUrl(source.kind, source.url, hash ? `sha256:${hash.replace(/^sha256:/, "")}` : undefined));
  }
  if (board.repo_url) {
    addRef(board.repo_url, sourceFromUrl("github-repo", board.repo_url, board.source_hashes?.repo ? `sha256:${board.source_hashes.repo.replace(/^sha256:/, "")}` : undefined));
  }
  if (board.wiki_url) {
    addRef(board.wiki_url, sourceFromUrl("wiki", board.wiki_url, board.source_hashes?.wiki ? `sha256:${board.source_hashes.wiki.replace(/^sha256:/, "")}` : undefined));
  }
  addRef(DOCUMENTATION_REPO, sourceFromUrl("documentation-repo", DOCUMENTATION_REPO, "sha256:5ff0374d566490a10db90e347e3ad995860a7de19a9409e7630b97162de2057e"));
  return Array.from(refs.values());
}

function firstSource(board, pack) {
  return sourceRefsForBoard(board, pack)[0] || sourceFromUrl("documentation-repo", DOCUMENTATION_REPO);
}

function fact(board, pack, topic, key, value, claim, confidence = "unknown_with_sources") {
  const source = firstSource(board, pack);
  return factWithSource(board, topic, key, value, claim, source, confidence);
}

function factWithSource(board, topic, key, value, claim, source, confidence = "unknown_with_sources") {
  return {
    schema_version: 1,
    board_id: board.id,
    topic,
    key,
    value,
    claim,
    source,
    authority_rank: source.kind === "github-repo" ? 85 : source.kind === "documentation-repo" ? 70 : 75,
    evidence_level: "V3-source-reference",
    stale: false,
    confidence
  };
}

function hasKey(pack, key) {
  return [
    ...(pack.pin_matrix || []),
    ...(pack.bus_matrix || []),
    ...(pack.expander_matrix || []),
    ...(pack.connector_matrix || []),
    ...(pack.peripheral_table || [])
  ].some((entry) => entry.key === key);
}

function pushUnique(list, entry) {
  if (!list.some((item) => item.key === entry.key)) {
    list.push(entry);
  }
}

function upsertByKey(list, entry) {
  const index = list.findIndex((item) => item.key === entry.key);
  if (index >= 0) {
    list[index] = entry;
    return;
  }
  list.push(entry);
}

function ensureBasePack(board, pack) {
  const next = pack ? JSON.parse(JSON.stringify(pack)) : {
    schema_version: 1,
    board_id: board.id,
    mcu_family: board.mcu || "unknown",
    supported: Boolean(board.supported && /^esp32/.test(board.mcu || "")),
    pin_matrix: [],
    bus_matrix: [],
    expander_matrix: [],
    connector_matrix: [],
    peripheral_table: [],
    source_refs: [],
    conflicts: []
  };
  next.schema_version = 1;
  next.board_id = board.id;
  next.mcu_family = board.mcu || next.mcu_family || "unknown";
  next.supported = Boolean(board.supported && /^esp32/.test(board.mcu || ""));
  for (const field of ["pin_matrix", "bus_matrix", "expander_matrix", "connector_matrix", "peripheral_table", "source_refs", "conflicts"]) {
    if (!Array.isArray(next[field])) next[field] = [];
  }
  next.source_refs = sourceRefsForBoard(board, next);
  if (!hasKey(next, "mcu.family")) {
    pushUnique(next.pin_matrix, fact(board, next, "pinout", "mcu.family", board.mcu || "unknown", "MCU family from official board index source", board.mcu ? "exact" : "unknown_with_sources"));
  }
  return next;
}

function enrichDisplay(board, pack) {
  pushUnique(pack.peripheral_table, fact(board, pack, "display", "display.panel_or_chip", "unknown_with_sources", "Display panel or chip requires official product source inspection before selecting a driver."));
  pushUnique(pack.peripheral_table, fact(board, pack, "display", "display.bus_or_interface", "unknown_with_sources", "Display bus requires official headers, setup files, or product docs before assigning pins."));
  pushUnique(pack.peripheral_table, fact(board, pack, "display", "display.backlight_or_power", "unknown_with_sources", "Backlight or display power GPIO requires official source inspection before writing firmware."));
  upsertByKey(
    pack.peripheral_table,
    factWithSource(
      board,
      "display",
      "known-pitfall.arduino-esp32-tft-espi",
      "arduino-esp32 core >2.0.14 has a reported TFT_eSPI ESP32-S3 regression; verify against official setup and library issue before upgrading.",
      "Known display pitfall from upstream TFT_eSPI issue tracker; keep as warning until board-specific sources prove a fix.",
      sourceFromUrl("github-issue", TFT_ESPI_CORE_PITFALL),
      "derived"
    )
  );
}

function enrichRadio(board, pack) {
  pushUnique(pack.peripheral_table, fact(board, pack, "lora", "lora.chip", "unknown_with_sources", "LoRa chip must be confirmed from official board source before selecting RadioLib parameters."));
  pushUnique(pack.peripheral_table, fact(board, pack, "lora", "lora.bus_or_interface", "unknown_with_sources", "LoRa bus and pins must be confirmed from official headers or product docs."));
  pushUnique(pack.peripheral_table, fact(board, pack, "lora", "lora.antenna", "unknown_with_sources", "Radio antenna and regional caveats require product documentation before RF assumptions."));
}

function enrichGnss(board, pack) {
  pushUnique(pack.peripheral_table, fact(board, pack, "gnss", "gnss.chip", "unknown_with_sources", "GNSS receiver must be confirmed from official board source before selecting a driver."));
  pushUnique(pack.peripheral_table, fact(board, pack, "gnss", "gnss.bus_or_interface", "unknown_with_sources", "GNSS UART or bus assignment must be confirmed from official headers or docs."));
}

function enrichInput(board, pack) {
  pushUnique(pack.peripheral_table, fact(board, pack, "input", "input.chip", "unknown_with_sources", "Primary input controller must be confirmed from official product source."));
  pushUnique(pack.peripheral_table, fact(board, pack, "input", "input.bus_or_interface", "unknown_with_sources", "Keyboard, encoder, trackball, touch, or button bus/pins require official source inspection."));
}

function enrichStorage(board, pack) {
  pushUnique(pack.peripheral_table, fact(board, pack, "storage", "storage.interface", "unknown_with_sources", "Storage interface requires official product source inspection before assigning SD/SPI/MMC pins."));
}

function enrichPower(board, pack) {
  pushUnique(pack.peripheral_table, fact(board, pack, "power", "power.manager", "unknown_with_sources", "Power manager or battery measurement facts require official source inspection."));
}

function enrichGenericPeripheral(board, pack, topic) {
  pushUnique(pack.peripheral_table, fact(board, pack, topic, `${topic}.chip`, "unknown_with_sources", `${topic} chip or controller requires official source inspection.`));
  pushUnique(pack.peripheral_table, fact(board, pack, topic, `${topic}.bus_or_interface`, "unknown_with_sources", `${topic} bus or pin mapping requires official source inspection.`));
}

function enrichPack(board, pack, options = {}) {
  const next = ensureBasePack(board, pack);
  if (GOLD_BOARDS.includes(board.id)) {
    return next;
  }
  const peripherals = new Set(board.peripherals || []);
  if (peripherals.has("display")) enrichDisplay(board, next);
  if (peripherals.has("lora")) enrichRadio(board, next);
  if (peripherals.has("gps") || peripherals.has("gnss")) enrichGnss(board, next);
  if (peripherals.has("input") || peripherals.has("keyboard") || peripherals.has("touch")) enrichInput(board, next);
  if (peripherals.has("storage")) enrichStorage(board, next);
  if (peripherals.has("power")) enrichPower(board, next);
  for (const topic of peripherals) {
    if (!["display", "lora", "gps", "gnss", "input", "keyboard", "touch", "storage", "power"].includes(topic)) {
      enrichGenericPeripheral(board, next, topic);
    }
  }
  if (!peripherals.size && next.supported) {
    pushUnique(next.peripheral_table, fact(board, next, "peripheral", "peripheral.primary", "unknown_with_sources", "Primary peripheral set requires official product source inspection."));
  }
  return next;
}

function generatePacks({ goldOnly = false } = {}) {
  const boards = readJson(BOARD_PATH).boards;
  const current = readJson(FACT_PACK_PATH).packs;
  const byId = new Map(current.map((pack) => [pack.board_id, pack]));
  const selectedBoards = goldOnly ? boards.filter((board) => GOLD_BOARDS.includes(board.id)) : boards;
  const packs = selectedBoards.map((board) => enrichPack(board, byId.get(board.id), { goldOnly }));
  return {
    schema_version: 1,
    packs
  };
}

function allAcceptedFieldsHaveSource(pack) {
  const tables = ["pin_matrix", "bus_matrix", "expander_matrix", "connector_matrix", "peripheral_table"];
  const missing = [];
  for (const table of tables) {
    for (const entry of pack[table] || []) {
      if (!entry.source?.path_or_url || !entry.source?.hash) {
        missing.push(`${pack.board_id}:${table}:${entry.key}`);
      }
    }
  }
  return missing;
}

function knownPitfallsHaveIssueSource(pack) {
  return (pack.peripheral_table || [])
    .filter((entry) => entry.key?.startsWith("known-pitfall."))
    .filter((entry) => entry.source?.kind !== "github-issue" || !/\/issues\/\d+/.test(entry.source?.path_or_url || ""))
    .map((entry) => `${pack.board_id}:peripheral_table:${entry.key}`);
}

module.exports = {
  ROOT,
  GENERATED_PATH,
  FACT_PACK_PATH,
  GOLD_BOARDS,
  readJson,
  writeJson,
  generatePacks,
  allAcceptedFieldsHaveSource,
  knownPitfallsHaveIssueSource
};
