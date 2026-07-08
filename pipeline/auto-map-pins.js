// Naming-convention auto-mapper. Given a source block's extracted #define
// macros, derives canonical pin fact keys using pin-naming-conventions.json.
// A manifest source with "auto_pins": true uses this instead of a hand-written
// pins array, so adding a board needs only {board_id, url, line_range}. Macros
// not covered by the convention table are skipped, never guessed.

const fs = require("fs");
const path = require("path");

const CONVENTIONS = path.join(__dirname, "pin-naming-conventions.json");

function loadConventions() {
  return JSON.parse(fs.readFileSync(CONVENTIONS, "utf8"));
}

function normalizeMacro(macro, conv) {
  let name = macro;
  for (const prefix of conv.strip_prefixes || []) {
    if (name.startsWith(prefix)) name = name.slice(prefix.length);
  }
  for (const suffix of conv.strip_suffixes || []) {
    if (name.endsWith(suffix)) name = name.slice(0, -suffix.length);
  }
  return name;
}

// macros: { MACRO_NAME: "18", ... } as extracted from a source block.
// Returns [{ key, macro, value_num }] for macros the convention recognizes,
// in the source's macro order.
function autoMapPins(macros) {
  const conv = loadConventions();
  const table = conv.macro_to_key || {};
  const mapped = [];
  for (const [macro, value] of Object.entries(macros)) {
    const key = table[normalizeMacro(macro, conv)];
    if (key) mapped.push({ key, macro, value_num: value });
  }
  return mapped;
}

module.exports = { autoMapPins, normalizeMacro, loadConventions };
