// Deterministic hook thick-capsule alignment test.
//
// The `hook <host>` command is the PUSH boundary: it inlines a board's critical
// pin/bus/driver facts into the model's context. The reference implementation
// (the now-archived Rust hook) reached 12/12 on the effect suite by pushing
// those values; the JS kernel must push the SAME values or a JS effect run would
// be silently handicapped.
//
// The reference push surface is frozen in eval/fixtures/rust-hook-golden.json —
// each task's `hook claude` additionalContext captured verbatim from the Rust
// binary before it was archived (branch rust-archive). This test extracts the
// pin/bus/driver fact VALUE SET the JS capsule pushes and asserts it agrees with
// the frozen reference value-for-value (order/wording tolerant; any missing or
// conflicting value fails). It never grades an answer — it proves the two push
// surfaces carry the same hardware facts, so the effect parity claim rests on
// data, not a rerun. Because Rust is deterministic and archived, the frozen
// reference is exactly as strict as a live diff.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const JS_BIN = join(ROOT, "bin/lilygo-skills.mjs");
const tasks = JSON.parse(readFileSync(join(ROOT, "eval/fixtures/effect-tasks.json"), "utf8")).tasks;
const golden = JSON.parse(readFileSync(join(ROOT, "eval/fixtures/rust-hook-golden.json"), "utf8"));

/**
 * The frozen Rust reference capsule for a task id. A missing entry FAILS loudly
 * (never a silent skip) — the whole point is a JS-vs-reference value diff.
 * @param {string} id
 * @returns {string}
 */
function referenceCapsule(id) {
  const capsule = golden[id];
  assert.ok(typeof capsule === "string", `frozen reference capsule missing for ${id}`);
  return capsule;
}

/**
 * Run the JS `hook claude` with a stdin prompt payload and return the pushed
 * additionalContext string.
 * @param {string} prompt
 * @returns {string}
 */
function jsHook(prompt) {
  const out = execFileSync(process.execPath, [JS_BIN, "hook", "claude"], {
    input: JSON.stringify({ prompt }),
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  const obj = JSON.parse(out);
  return String(obj.hookSpecificOutput?.additionalContext ?? "");
}

/** Collapse internal whitespace so wording spacing never masks a value match. */
const squash = (s) => s.replace(/\s+/g, " ").trim();

/**
 * Slice the `<name>=[ ... ]` bracket body out of a capsule (or "" if absent).
 * @param {string} capsule
 * @param {string} name
 * @returns {string}
 */
function bracket(capsule, name) {
  const m = capsule.match(new RegExp(`${name}=\\[([^\\]]*)\\]`));
  return m ? m[1] : "";
}

/**
 * The chip/bus/driver fact values a capsule pushes, as a normalized set. Splits
 * on the `chip=|bus=|driver=` key boundaries so commas *inside* a value (e.g.
 * "I2C (SDA=GPIO18, SCL=GPIO8)") never fracture a value.
 * @param {string} capsule
 * @returns {Set<string>}
 */
function factsValueSet(capsule) {
  const body = bracket(capsule, "facts");
  /** @type {Set<string>} */
  const set = new Set();
  const re = /(chip|bus|driver)=(.*?)(?=,(?:chip|bus|driver)=|$)/g;
  let m;
  while ((m = re.exec(body)) !== null) set.add(`${m[1]}=${squash(m[2])}`);
  return set;
}

/**
 * The pin rows a capsule pushes, as a normalized `key=value` set (order/spacing
 * tolerant). The `pins=[..]` entries are `pin.i2c.sda=SYM=GPIOnn` etc.; the
 * display-bus row carries commas inside its value, so split on the pin-key
 * boundary rather than on bare commas.
 * @param {string} capsule
 * @returns {Set<string>}
 */
function pinsValueSet(capsule) {
  const body = bracket(capsule, "pins");
  /** @type {Set<string>} */
  const set = new Set();
  const re = /([a-z][\w.]*?)=(.*?)(?=,[a-z][\w.]*?=|$)/g;
  let m;
  while ((m = re.exec(body)) !== null) set.add(`${m[1]}=${squash(m[2])}`);
  return set;
}

/**
 * Every concrete hardware token (GPIO assignments + I2C addresses) anywhere in
 * the pushed facts+pins segments — a wording-independent conflict tripwire: a
 * flipped SDA/SCL or a wrong GPIO shows up as a set difference even if the
 * surrounding prose matches.
 * @param {string} capsule
 * @returns {Set<string>}
 */
function hardwareTokenSet(capsule) {
  const scope = `${bracket(capsule, "facts")} ${bracket(capsule, "pins")}`;
  /** @type {Set<string>} */
  const set = new Set();
  for (const m of scope.matchAll(/[A-Za-z_][\w]*\s*=\s*GPIO\d+/g)) {
    set.add(m[0].replace(/\s+/g, "").toUpperCase());
  }
  for (const m of scope.matchAll(/GPIO\d+/g)) set.add(m[0].toUpperCase());
  for (const m of scope.matchAll(/0x[0-9a-fA-F]+/g)) set.add(m[0].toLowerCase());
  return set;
}

const setEq = (a, b) => a.size === b.size && [...a].every((v) => b.has(v));
const diff = (a, b) => [...a].filter((v) => !b.has(v));

// One board per unique task prompt (12 prompts across 5 boards) — exercises the
// prompt-dependent peripheral/pin selection, not just a static per-board dump.
const boards = [...new Set(tasks.map((t) => t.board))];
assert.ok(boards.length >= 5, `expected >=5 distinct boards, got ${boards.join(",")}`);

for (const task of tasks) {
  test(`hook capsule value-alignment: ${task.id} [${task.board}]`, () => {
    const rustCtx = referenceCapsule(task.id);
    const jsCtx = jsHook(task.prompt);
    assert.ok(rustCtx.length > 0, `reference capsule empty for ${task.id}`);
    assert.ok(jsCtx.length > 0, `js hook pushed no capsule for ${task.id}`);
    assert.ok(jsCtx.includes(`board-`), `js capsule missing board id for ${task.id}`);

    const rf = factsValueSet(rustCtx);
    const jf = factsValueSet(jsCtx);
    assert.ok(
      setEq(rf, jf),
      `facts value set diverged for ${task.id}\n  rust-only: ${JSON.stringify(diff(rf, jf))}\n  js-only:   ${JSON.stringify(diff(jf, rf))}`,
    );

    const rp = pinsValueSet(rustCtx);
    const jp = pinsValueSet(jsCtx);
    assert.ok(
      setEq(rp, jp),
      `pins value set diverged for ${task.id}\n  rust-only: ${JSON.stringify(diff(rp, jp))}\n  js-only:   ${JSON.stringify(diff(jp, rp))}`,
    );

    const rt = hardwareTokenSet(rustCtx);
    const jt = hardwareTokenSet(jsCtx);
    assert.ok(
      setEq(rt, jt),
      `hardware token (GPIO/address) set diverged for ${task.id}\n  rust-only: ${JSON.stringify(diff(rt, jt))}\n  js-only:   ${JSON.stringify(diff(jt, rt))}`,
    );
  });
}

// Guard the value-bearing extraction itself: at least one board must actually
// push a non-empty pin set, so a regex that silently matches nothing can never
// let a hollow "aligned" (empty==empty) pass masquerade as coverage.
test("hook alignment exercises non-empty pushed pin values", () => {
  const withPins = tasks.filter((t) => pinsValueSet(referenceCapsule(t.id)).size > 0);
  assert.ok(withPins.length >= 3, `expected >=3 tasks with pushed pins, got ${withPins.length}`);
  const withFacts = tasks.filter((t) => factsValueSet(referenceCapsule(t.id)).size > 0);
  assert.ok(withFacts.length >= 3, `expected >=3 tasks with pushed chip/bus/driver facts, got ${withFacts.length}`);
});
