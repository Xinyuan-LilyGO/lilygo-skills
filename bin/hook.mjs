// `hook <claude|codex>` — the push boundary command.
//
// Reproduces the Rust `lilygo-skills hook <host>` THICK capsule: it resolves the
// board, pulls that board's critical pin/bus/driver facts straight from the
// committed data (`data/boards.json` + `data/facts/board-fact-packs.json`), and
// pushes them inline into the model's context so a pin question is seeded with
// the real GPIO values (not just a pull pointer). This is the effect-bearing
// surface P0 used to reach 12/12; the thin `context` capsule only points.
//
// NEVER inlines a hardcoded pin: every value comes from the data layer. Mirrors
// crates/lilygo-skills-cli/src/capsule/{mod,context}.rs (render_hook_goal_summary
// + render_capsule_pins + add_relevant_peripherals) for the pin/bus/driver value
// set. The JS kernel has no router machine, so `skills=[..]` carries the detected
// board rather than the full routed skill list; the pin/bus/driver VALUES — the
// graded, effect-bearing payload — are byte-faithful to Rust.
import { detectBoard } from "./find.mjs";
import { getBoard, getPack, promptKeywords, isMain } from "./lib.mjs";

/** Topics the source-recovery CLI accepts, offered so the model picks the right one. */
const PULL_TOPICS = "pinout|display|lora|gnss|power|i2c|spi|touch";

/**
 * Fetch-before-claim guidance, immediately before the honesty markers (Rust
 * GUIDANCE_LINE). Imperative pull-hardening: the push capsule only carries the
 * facts that fit the byte budget; ANY concrete pin/bus/setting not already
 * inlined in facts[]/pins[] MUST be pulled via `source query` and cited before
 * the model may state it. This is what lets a neutral-cwd JS arm reach the
 * push-capped values (it has no repo CLAUDE.md/skill to steer the pull); it is
 * pure prose — it adds no fact value, so the JS↔Rust value-alignment holds.
 * @param {string} board
 * @returns {string}
 */
function guidanceLine(board) {
  return (
    " guidance=MANDATORY pull-before-claim:" +
    " for ANY concrete pin/bus/setting NOT already listed in facts[]/pins[] above," +
    ` you MUST FIRST run 'lilygo-skills source query --board ${board} --topic <topic> --json'` +
    ` (topics: ${PULL_TOPICS}) and quote the returned official url + line_range + sha256 in your answer;` +
    " never report a pin/bus/address from memory; if a value is neither inlined here nor recoverable via" +
    " source query, say so — do not invent pin numbers;"
  );
}

/**
 * Parse a hook stdin payload into a prompt (mirrors Rust extract_prompt): a JSON
 * object's prompt/input/text field, else the raw text.
 * @param {string} input
 * @returns {string}
 */
export function extractPrompt(input) {
  if (input.trim() === "") return "";
  try {
    const value = JSON.parse(input);
    if (value && typeof value === "object") {
      for (const key of ["prompt", "input", "text"]) {
        if (typeof value[key] === "string") return value[key];
      }
    }
    return input;
  } catch {
    return input;
  }
}

/**
 * @param {string} haystackLower already-lowercased text
 * @param {string[]} needles
 * @returns {boolean}
 */
function containsAny(haystackLower, needles) {
  return needles.some((n) => haystackLower.includes(n));
}

/**
 * Does the prompt ask for a concrete board fact (mirrors Rust is_fact_prompt)?
 * @param {string} prompt
 * @returns {boolean}
 */
function isFactPrompt(prompt) {
  return containsAny(prompt.toLowerCase(), promptKeywords().fact_prompt);
}

/**
 * Peripheral categories the prompt asks about (mirrors Rust
 * capsule/context.rs::requested_peripherals). The JS kernel carries no router,
 * so the `route.peripherals`/`route.chips` contributions are derived purely from
 * the prompt text (which, for a board question, already names the board and the
 * subsystem). Deterministic and data-free of pin values.
 * @param {string} prompt
 * @returns {Set<string>}
 */
export function requestedPeripherals(prompt) {
  const p = prompt.toLowerCase();
  /** @type {Set<string>} */
  const requested = new Set();
  if (containsAny(p, ["imu", "bhi260ap", "gesture", "抬腕"])) requested.add("imu");
  if (containsAny(p, ["nfc", "st25r3916"])) requested.add("nfc");
  if (containsAny(p, ["lvgl", "touch", "display", "screen"])) {
    requested.add("display");
    requested.add("touch");
    requested.add("power");
  }
  if (containsAny(p, ["ota", "flash", "partition", "manifest"])) {
    requested.add("memory");
    requested.add("storage");
  }
  if (containsAny(p, ["power", "pmic", "axp", "battery", "charge", "电源", "电池", "充电"])) {
    requested.add("power");
  }
  if (containsAny(p, ["haptic", "vibrat", "motor", "drv2605", "震动", "马达", "振动"])) {
    requested.add("haptic");
  }
  if (containsAny(p, ["xl9555", "gpio", "io", "pinout", "引脚", "外设"])) requested.add("input");
  return requested;
}

/**
 * Normalize a peripheral record to its requested-set key (mirrors Rust
 * normalized_peripheral).
 * @param {{ category: string, chip: string, name: string }} peripheral
 * @returns {string}
 */
export function normalizedPeripheral(peripheral) {
  const chip = (peripheral.chip || "").toLowerCase();
  const name = (peripheral.name || "").toLowerCase();
  const cat = peripheral.category;
  if (chip.includes("bhi260ap") || name.includes("imu")) return "imu";
  if (chip.includes("st25r3916") || cat === "nfc") return "nfc";
  if (cat === "radio") return "lora";
  if (cat === "gnss") return "gnss";
  if (cat === "io") return "input";
  if (cat === "touch") return "touch";
  if (cat === "display") return "display";
  if (cat === "memory") return "memory";
  if (cat === "storage") return "storage";
  if (cat === "power") return "power";
  if (cat === "haptic") return "haptic";
  return "other";
}

/**
 * chip/bus/driver facts for the requested peripherals, in peripheral_matrix
 * order (mirrors Rust add_relevant_peripherals + the chip|bus|driver filter in
 * render_hook_goal_summary). Values are read verbatim from data/boards.json.
 * @param {string} boardId
 * @param {string} prompt
 * @returns {Array<{ key: string, value: string }>}
 */
function chipBusDriverFacts(boardId, prompt) {
  const board = getBoard(boardId);
  if (!board) return [];
  const requested = requestedPeripherals(prompt);
  /** @type {Array<{ key: string, value: string }>} */
  const facts = [];
  for (const peripheral of board.peripheral_matrix || []) {
    if (!requested.has(normalizedPeripheral(peripheral))) continue;
    for (const key of /** @type {const} */ (["chip", "bus", "driver"])) {
      const value = peripheral[key];
      if (value) facts.push({ key, value });
    }
  }
  return facts;
}

const MAX_FACT_ROWS_PER_TABLE = 8; // Rust ContextBudget default.

/**
 * The io fact-table rows the pin renderer scans (mirrors fact_tables_for_goal:
 * the raw pack matrices in file order, each capped at 8 rows). Only rendered for
 * a fact/bus prompt; otherwise Rust surfaces the source-recovery critical list.
 * @param {string} boardId
 * @param {string} prompt
 * @returns {Array<{ key: string, value: string, confidence?: string }>}
 */
function factTableRows(boardId, prompt) {
  if (!isFactPrompt(prompt)) return [];
  const pack = getPack(boardId);
  if (!pack) return [];
  /** @type {Array<{ key: string, value: string, confidence?: string }>} */
  const rows = [];
  for (const table of [
    pack.pin_matrix,
    pack.bus_matrix,
    pack.expander_matrix,
    pack.connector_matrix,
    pack.peripheral_table,
  ]) {
    rows.push(...(table || []).slice(0, MAX_FACT_ROWS_PER_TABLE));
  }
  return rows;
}

/**
 * A concrete pin/bus assignment worth surfacing inline (mirrors Rust
 * is_concrete_pin_fact): a pin/bus/display/i2c key whose value pins a GPIO or
 * I2C address.
 * @param {string} key
 * @param {string} value
 * @returns {boolean}
 */
function isConcretePinFact(key, value) {
  const k = key.toLowerCase();
  const v = value.toLowerCase();
  const keyIsPinlike = ["pin.", "bus.", "i2c.", "display.bus", "display.backlight"].some((prefix) =>
    k.startsWith(prefix),
  );
  if (!keyIsPinlike) return false;
  return v.includes("gpio") || v.includes("0x");
}

/**
 * Priority-ordered semantic slot for a concrete pin fact (mirrors Rust
 * pin_slot). Lower priority wins the byte budget; only the first row per slot
 * is kept.
 * @param {string} key
 * @param {string} value
 * @returns {[number, string]}
 */
function pinSlot(key, value) {
  const hay = `${key} ${value}`.toLowerCase();
  const k = key.toLowerCase();
  if (hay.includes("sda")) return [0, "i2c.sda"];
  if (hay.includes("scl")) return [0, "i2c.scl"];
  if (k.startsWith("bus.display") || k.startsWith("display.bus")) return [1, "display.bus"];
  if (hay.includes("backlight") || hay.includes("bl=") || hay.includes("power_on"))
    return [2, "display.power"];
  return [3, "other"];
}

/**
 * Render the inline `pins=[..]` segment from the io fact tables (mirrors Rust
 * render_capsule_pins: skip unknown/non-concrete rows and rows already carried
 * verbatim by `factsStr`, keep one row per semantic slot in slot order, and cap
 * at 5 rows / 320 bytes).
 * @param {Array<{ key: string, value: string, confidence?: string }>} rows
 * @param {string} factsStr the joined chip/bus/driver `key=value` string
 * @returns {string}
 */
function renderCapsulePins(rows, factsStr) {
  const MAX_ROWS = 5;
  const MAX_SEGMENT_BYTES = 320;
  /** @type {Map<string, { key: string, value: string }>} */
  const bestPerSlot = new Map();
  for (const row of rows) {
    if (row.confidence === "unknown_with_sources" || row.value === "unknown_with_sources") continue;
    if (!isConcretePinFact(row.key, row.value)) continue;
    if (factsStr.includes(row.value)) continue;
    const [priority, name] = pinSlot(row.key, row.value);
    if (priority >= 3) continue;
    const slotKey = `${priority} ${name}`;
    if (!bestPerSlot.has(slotKey)) bestPerSlot.set(slotKey, row);
  }
  // Emit in slot-key order (priority asc, then name asc) — matches Rust BTreeMap.
  const slotKeys = [...bestPerSlot.keys()].sort();
  /** @type {string[]} */
  const rendered = [];
  let bytes = 0;
  for (const slotKey of slotKeys) {
    const row = /** @type {{ key: string, value: string }} */ (bestPerSlot.get(slotKey));
    const entry = `${row.key}=${row.value.trim()}`;
    if (bytes + entry.length > MAX_SEGMENT_BYTES) continue;
    bytes += entry.length + 1;
    rendered.push(entry);
    if (rendered.length >= MAX_ROWS) break;
  }
  return rendered.length === 0 ? "" : ` pins=[${rendered.join(",")}];`;
}

/**
 * Assemble the thick additionalContext capsule for `board` + `prompt`. Returns
 * "" when no board is detected (fail-open, no injection).
 * @param {string} prompt
 * @param {{ projectDir?: string }} [opts]
 * @returns {{ board: string | null, boardSource: string | null, context: string }}
 */
export function assembleHookCapsule(prompt, opts = {}) {
  const { board, source } = detectBoard({ prompt, projectDir: opts.projectDir });
  if (!board) return { board: null, boardSource: null, context: "" };

  const facts = chipBusDriverFacts(board, prompt);
  const factsStr = facts.map((f) => `${f.key}=${f.value}`).join(",");
  const pins = renderCapsulePins(factTableRows(board, prompt), factsStr);

  const prefix =
    `LilyGO context injection: skills=[${board}]; verification_level=context-injection; ` +
    `hardware_verified=false; expand=[lilygo-skills source query --board ${board} --topic io --json]`;
  const capsule =
    `${prefix} LilyGO goal capsule:` +
    ` facts=[${factsStr}];` +
    pins +
    guidanceLine(board) +
    ` evidence_boundary=V3/hardware_verified=false`;
  return { board, boardSource: source, context: capsule };
}

/**
 * Build the host envelope (mirrors Rust hook_envelope). claude emits the
 * UserPromptSubmit additionalContext envelope; codex emits the diagnostic form.
 * @param {string} host
 * @param {{ board: string | null, context: string }} result
 * @returns {object}
 */
function hookEnvelope(host, result) {
  if (host === "claude") {
    /** @type {{ hookEventName: string, additionalContext?: string }} */
    const inner = { hookEventName: "UserPromptSubmit" };
    if (result.context !== "") inner.additionalContext = result.context;
    return { hookSpecificOutput: inner };
  }
  return {
    host,
    decision: result.board ? "inject" : "no-op",
    skills: result.board ? [result.board] : [],
    context: result.context,
    fail_open: true,
  };
}

/**
 * Fail-open envelope (mirrors Rust print_hook_fail_open): claude never blocks;
 * codex reports the error.
 * @param {string} host
 * @param {string} error
 * @returns {object}
 */
function hookFailOpen(host, error) {
  if (host === "claude") {
    process.stderr.write(`lilygo-skills hook claude fail-open: ${error}\n`);
    return { hookSpecificOutput: { hookEventName: "UserPromptSubmit" } };
  }
  return { host, decision: "no-op", skills: [], context: "", fail_open: true, error };
}

const HOOK_USAGE =
  "Usage: lilygo-skills hook <claude|codex>\n\n" +
  "Reads a prompt JSON object ({\"prompt\":\"...\"}) from stdin.\n" +
  "claude: emits the UserPromptSubmit hookSpecificOutput envelope with the thick capsule.\n" +
  "codex: emits the diagnostic routing envelope.\n";

/**
 * `hook <host>` entrypoint: reads stdin, assembles the capsule, prints the host
 * envelope. Always exits 0 (fail-open) except on usage/unknown-host errors.
 * @param {string[]} argv argv tail after `hook`
 * @param {string} stdin
 * @returns {number} exit code
 */
export function runHook(argv, stdin) {
  const host = argv[0] ?? "codex";
  if (host === "--help" || host === "-h") {
    process.stdout.write(HOOK_USAGE);
    return 0;
  }
  if (host !== "claude" && host !== "codex") {
    process.stderr.write(`unsupported hook host: ${host}\n`);
    return 2;
  }
  let envelope;
  try {
    const prompt = extractPrompt(stdin);
    const result = assembleHookCapsule(prompt);
    envelope = hookEnvelope(host, result);
  } catch (error) {
    envelope = hookFailOpen(host, error instanceof Error ? error.message : String(error));
  }
  process.stdout.write(JSON.stringify(envelope, null, 2) + "\n");
  return 0;
}

/**
 * Read all of stdin (best-effort; empty string if no pipe).
 * @returns {Promise<string>}
 */
function readStdin() {
  return new Promise((resolve) => {
    if (process.stdin.isTTY) return resolve("");
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => (data += chunk));
    process.stdin.on("end", () => resolve(data));
    process.stdin.on("error", () => resolve(data));
  });
}

/**
 * Dispatcher-facing async wrapper: gathers stdin then runs the hook.
 * @param {string[]} argv argv tail after `hook`
 * @returns {Promise<number>}
 */
export async function runHookCommand(argv) {
  const stdin = await readStdin();
  return runHook(argv, stdin);
}

if (isMain(import.meta.url)) {
  runHookCommand(process.argv.slice(2)).then((code) => process.exit(code));
}
