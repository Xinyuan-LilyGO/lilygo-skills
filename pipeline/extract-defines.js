// Shared integer-define extractor for LilyGO official pin headers.
//
// Vendor headers declare GPIO numbers in three forms across the LilyGO repos:
//   #define NAME 21               (T-Beam, T-Deck, T-Display-S3, T-Dongle-S3, ...)
//   #define NAME (21)             (LilyGoLib-PlatformIO variant pins_arduino.h)
//   static const uint8_t NAME = 21;  (Arduino core bus pins: SDA/SCL/MOSI/... )
//
// Only non-negative decimal integers are accepted (real GPIO numbers). Hex
// (0x82D4 USB ids), negative sentinels (-1 = "not connected"), and
// macro-reference values (LORA_SCK (SCK)) are skipped, never guessed — the
// numeric value must be literally present in the official source. Macro names
// are uppercase, matching the existing convention.

// #define NAME <int> | #define NAME (<int>) with optional trailing comment.
// The (?![\w.]) tail rejects hex (the "0" of 0x82D4) and decimals.
const DEFINE_RE = /^[ \t]*#define[ \t]+([A-Z0-9_]+)[ \t]+\(?(\d+)\)?(?![\w.])/gm;
// static const <intN>_t NAME = <int>;  (RHS must be a literal, not a macro ref)
const STATIC_CONST_RE = /^[ \t]*static[ \t]+const[ \t]+u?int\d+_t[ \t]+([A-Z0-9_]+)[ \t]*=[ \t]*(\d+)[ \t]*;/gm;

// Yields { name, value, index } for every integer define in file order across
// both syntaxes (index = byte offset, so callers can honor original order).
function collectDefines(text) {
  const out = [];
  let m;
  DEFINE_RE.lastIndex = 0;
  while ((m = DEFINE_RE.exec(text)) !== null) out.push({ name: m[1], value: m[2], index: m.index });
  STATIC_CONST_RE.lastIndex = 0;
  while ((m = STATIC_CONST_RE.exec(text)) !== null) out.push({ name: m[1], value: m[2], index: m.index });
  out.sort((a, b) => a.index - b.index);
  return out;
}

// { NAME: "value" } with first definition (in file order) winning, matching the
// legacy first-def-wins behavior of the per-file extractors.
function firstDefineMap(text) {
  const map = {};
  for (const d of collectDefines(text)) if (!(d.name in map)) map[d.name] = d.value;
  return map;
}

// { NAME: Set(values) } across the whole text, for multi-variant conflict
// detection (a macro defined with more than one distinct value).
function allDefineValues(text) {
  const defs = {};
  for (const d of collectDefines(text)) (defs[d.name] = defs[d.name] || new Set()).add(d.value);
  return defs;
}

module.exports = { collectDefines, firstDefineMap, allDefineValues };
