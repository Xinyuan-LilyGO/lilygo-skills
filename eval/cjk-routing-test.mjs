// CJK prompt routing coverage for the JS thin core.
//
// The Chinese trigger vocabulary (烧录/显示/固件/引脚/…) must route the same as
// the English surface: a board-bearing CN prompt injects that board's capsule,
// and a non-hardware CN prompt stays a no-op. This replaces the Rust-binary
// cjk-prompt smoke with a language-independent JS check so CN coverage survives
// the switch to the JS dispatcher.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const DISPATCHER = join(ROOT, "bin", "lilygo-skills.mjs");

/**
 * @param {string[]} argv
 * @param {string} [stdin]
 * @returns {unknown}
 */
function runJson(argv, stdin) {
  const out = execFileSync(process.execPath, [DISPATCHER, ...argv], {
    input: stdin,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  return JSON.parse(out);
}

test("CN board prompt injects the board capsule (context)", () => {
  const json = /** @type {{ board: string, decision: string }} */ (
    runJson(["context", "--json", "T-Display-S3 烧录固件 显示第一屏"])
  );
  assert.equal(json.board, "board-t-display-s3");
  assert.equal(json.decision, "inject");
});

test("CN pin prompt pushes a thick capsule with the board (hook)", () => {
  const json = /** @type {{ hookSpecificOutput: { additionalContext?: string } }} */ (
    runJson(["hook", "claude"], JSON.stringify({ prompt: "T-Watch-Ultra 显示屏 QSPI 引脚" }))
  );
  const context = String(json.hookSpecificOutput?.additionalContext ?? "");
  assert.ok(context.includes("board-t-watch-ultra"), "CN hook capsule missing board id");
});

test("non-hardware CN prompt stays a no-op (context)", () => {
  const json = /** @type {{ board: unknown, decision: string }} */ (
    runJson(["context", "--json", "帮我写一首关于春天的诗"])
  );
  assert.equal(json.board, null);
  assert.equal(json.decision, "no-op");
});
