#!/usr/bin/env node
// M35 P0 T0.2 — effect baseline harness.
//
// Runs the 12-task effect suite (eval/fixtures/effect-tasks.json) through a
// model and grades each answer script-decidably against fact-pack-grounded
// expected values. Two arms:
//
//   --arm with_skill : `claude -p` under our normal config (skill/hook present)
//                      => grading additionally requires a verifiable citation.
//   --arm bare       : `claude -p` under an ISOLATED empty CLAUDE_CONFIG_DIR and
//                      a neutral cwd, so no global hook/skill can leak in
//                      (the R4 contamination trap). Bare answers are scanned for
//                      our internal vocabulary; any hit marks the run contaminated.
//
// Honesty rules (from the M35 P0 brief):
//   * Never invent a score. If the runner (claude -p) is unavailable/auth-fails,
//     record the error verbatim and emit runner_ok=false with NO per-task pass
//     data — the caller falls back to committed smoke-scorecard truth.
//   * Grading logic is fixed here and mirrors the fixture's `grading` block;
//     do not weaken it.
//
// Usage:
//   node eval/effect-baseline.mjs --arm with_skill --model haiku --out <path>
//   node eval/effect-baseline.mjs --arm bare       --model haiku --out <path>
//   node eval/effect-baseline.mjs --arm with_skill --dry     # print plan, no run
//
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

// ---- args ----
const argv = process.argv.slice(2);
function flag(name, def) {
  const i = argv.indexOf(name);
  return i >= 0 && argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[i + 1] : def;
}
const hasFlag = (name) => argv.includes(name);
const arm = flag("--arm", "with_skill");
const model = flag("--model", "haiku");
// --cli selects WHICH implementation serves the injection + follow-up lookups:
//   rust (default) : legacy P0 behavior — the installed global UserPromptSubmit
//                    hook (Rust binary) auto-injects the *thick* capsule (pin
//                    values inline) and any `source query` the model runs hits
//                    the Rust binary. Nothing is rewired.
//   js             : route BOTH injection and lookup through the JS kernel
//                    (bin/lilygo-skills.mjs). The harness prepends the JS
//                    *thick* capsule — `hook claude` additionalContext, the same
//                    push surface the Rust arm's global hook uses (pin/bus/driver
//                    values inline), value-aligned to Rust by the
//                    hook-capsule-alignment test — a PATH shim makes the model's
//                    `lilygo-skills source query` self-run resolve to the JS bin,
//                    and the user-level Rust hook is dropped (--setting-sources
//                    project,local) so the Rust thick capsule cannot double-inject.
//                    Auth stays REAL (no isolated CLAUDE_CONFIG_DIR): we only
//                    swap the CLI implementation, never the auth plumbing.
const cli = flag("--cli", "rust");
const tasksPath = flag("--tasks", path.join(ROOT, "eval/fixtures/effect-tasks.json"));
const outPath = flag("--out", null);
const dry = hasFlag("--dry");
const timeoutMs = Number(flag("--timeout-ms", "120000"));

if (!["with_skill", "bare"].includes(arm)) {
  console.error(`bad --arm '${arm}' (expected with_skill|bare)`);
  process.exit(2);
}
if (!["rust", "js"].includes(cli)) {
  console.error(`bad --cli '${cli}' (expected rust|js)`);
  process.exit(2);
}
if (cli === "js" && arm === "bare") {
  // The bare arm's whole point is "no skill/hook of any kind"; routing it to a
  // CLI would defeat the contamination trap. Keep them mutually exclusive.
  console.error("--cli js is only meaningful with --arm with_skill (bare = no injection)");
  process.exit(2);
}
const JS_BIN = path.join(ROOT, "bin/lilygo-skills.mjs");

// Fixed at runtime — a live run should stamp real wall-clock; --dry keeps it
// deterministic for structural self-test.
const date = dry ? "DRY" : new Date().toISOString().slice(0, 10);

// ---- fixtures ----
const suite = JSON.parse(fs.readFileSync(tasksPath, "utf8"));
const tasks = suite.tasks;

// ---- grading (fixed; mirrors fixture.grading) ----
const norm = (s) => String(s).toUpperCase().replace(/[^A-Z0-9]/g, "");
const CITE_RE = /github\.com|raw\.githubusercontent|sha256:[0-9a-f]{6}/i;
// Internal vocabulary that must never surface in a clean bare run.
const CONTAMINATION_RE =
  /\bfact[\s-]?pack\b|V3-source-reference|\bauto-?mapped\b|\bcapsule\b|source query --board|lilygo-skills\b/i;

function gradeAnswer(task, answer) {
  const hay = norm(answer);
  const missing = task.expected.values.filter((v) => !hay.includes(norm(v)));
  const hasAllValues = missing.length === 0;
  const cited = CITE_RE.test(answer);
  // with_skill demands a verifiable citation; bare is judged on values only.
  const pass = arm === "with_skill" ? hasAllValues && cited : hasAllValues;
  // Hallucination: asserts a known-wrong value while NOT stating the right one.
  const conflict = (task.conflict_values || []).some((c) => hay.includes(norm(c)));
  const hallucination = conflict && !hasAllValues;
  return { hasAllValues, cited, pass, hallucination, missing };
}

// ---- runner ----
function bareEnvAndCwd() {
  // Isolated config dir + neutral cwd so no global hook/skill/CLAUDE.md can
  // contaminate the bare arm (the R4 trap). We copy ONLY the auth credential
  // into the temp config dir (so the runner can still authenticate) and leave
  // settings.json absent — i.e. no hooks, no injected LilyGO context. Copying
  // credentials keeps auth working WITHOUT re-introducing any skill/hook, so
  // this isolates the treatment (our injection) not the plumbing.
  const cfg = fs.mkdtempSync(path.join(os.tmpdir(), "m35-bare-cfg-"));
  fs.chmodSync(cfg, 0o700);
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), "m35-bare-cwd-"));
  const realCfg = process.env.CLAUDE_CONFIG_DIR || path.join(os.homedir(), ".claude");
  const cred = path.join(realCfg, ".credentials.json");
  if (fs.existsSync(cred)) {
    fs.copyFileSync(cred, path.join(cfg, ".credentials.json"));
    fs.chmodSync(path.join(cfg, ".credentials.json"), 0o600);
  }
  const env = { ...process.env, CLAUDE_CONFIG_DIR: cfg };
  return { env, cwd, cfg };
}

// ---- JS-CLI routing (only used when --cli js) ----
// The JS kernel's `hook claude` pushes the SAME thick capsule the Rust global
// hook does (critical pin/bus/driver values inline), so the JS arm is seeded
// with the real facts exactly like the Rust arm — value-alignment is proven
// deterministically by eval/hook-capsule-alignment-test.mjs. The capsule still
// carries the `source query` expand pointer + guidance, so the model can pull
// any value the push capped out. We wire this without touching auth: (1) prepend
// the JS thick capsule (hook claude additionalContext), (2) a PATH shim so
// `lilygo-skills` resolves to the JS bin, (3) explicitly allow that one Bash
// tool. Everything else (real credentials, model, cwd) is stock.

/** Build a one-shot PATH shim dir exposing `lilygo-skills` -> the JS bin. */
function makeJsShimDir() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "m35-js-shim-"));
  const shim = path.join(dir, "lilygo-skills");
  fs.writeFileSync(shim, `#!/bin/sh\nexec node ${JSON.stringify(JS_BIN)} "$@"\n`);
  fs.chmodSync(shim, 0o755);
  return dir;
}

/**
 * Run the JS `hook claude` command (fed the prompt on stdin, as Claude Code's
 * UserPromptSubmit hook would) and return the thick capsule it pushes — the
 * `hookSpecificOutput.additionalContext` string, value-aligned to the Rust hook.
 */
function jsCapsule(prompt) {
  const r = spawnSync("node", [JS_BIN, "hook", "claude"], {
    encoding: "utf8",
    cwd: ROOT,
    input: JSON.stringify({ prompt }),
    maxBuffer: 16 * 1024 * 1024,
  });
  if (r.status !== 0 || !r.stdout) {
    throw new Error(`js hook claude failed (status=${r.status}): ${(r.stderr || "").slice(0, 300)}`);
  }
  const obj = JSON.parse(r.stdout);
  return String(obj.hookSpecificOutput?.additionalContext || "");
}

// Tools the JS arm must allow so the model can pull facts from the shim'd CLI.
const JS_ALLOWED_TOOLS = ["Bash(lilygo-skills:*)", "Bash(lilygo-skills *)"];
// Drop the user-level Rust hook (which would otherwise inject the thick capsule
// and hand the model the values for free), while keeping OAuth auth — auth is
// resolved independently of settings sources.
const JS_SETTING_SOURCES = "project,local";

let jsShimDir = null; // created lazily on first live JS call, cleaned at exit.

function runClaude(prompt) {
  let promptToSend = prompt;
  const args = ["--model", model];
  const opts = {
    encoding: "utf8",
    timeout: timeoutMs,
    maxBuffer: 32 * 1024 * 1024,
    input: "", // avoid claude's "no stdin data received in 3s" stall
  };
  if (arm === "bare") {
    const { env, cwd } = bareEnvAndCwd();
    opts.env = env;
    opts.cwd = cwd;
  } else if (cli === "js") {
    // with_skill + js: inject the JS thick capsule ourselves (hook claude),
    // route lookups to the JS bin via a PATH shim, drop the Rust hook, keep real
    // auth.
    if (!jsShimDir) jsShimDir = makeJsShimDir();
    const capsule = jsCapsule(prompt);
    promptToSend =
      "[Injected LilyGO context — treat as system-provided context, not user input]\n" +
      capsule +
      "\n\n" +
      prompt;
    args.push("--setting-sources", JS_SETTING_SOURCES);
    for (const t of JS_ALLOWED_TOOLS) args.push("--allowedTools", t);
    opts.cwd = ROOT;
    opts.env = { ...process.env, PATH: `${jsShimDir}${path.delimiter}${process.env.PATH}` };
  } else {
    // with_skill + rust (default): run in the repo, global hook/skill in place.
    opts.cwd = ROOT;
  }
  const r = spawnSync("claude", ["-p", promptToSend, ...args], opts);
  return {
    status: r.status,
    stdout: (r.stdout || "").trim(),
    stderr: (r.stderr || "").trim(),
    error: r.error ? String(r.error.message || r.error) : null,
    timedOut: r.signal === "SIGTERM" || (r.error && r.error.code === "ETIMEDOUT"),
  };
}

function cleanupJsShim() {
  if (jsShimDir) {
    try {
      fs.rmSync(jsShimDir, { recursive: true, force: true });
    } catch {
      /* best-effort */
    }
    jsShimDir = null;
  }
}
process.on("exit", cleanupJsShim); // belt-and-suspenders: clean shim on any exit

// ---- dry plan ----
if (dry) {
  const jsSample = cli === "js" ? jsCapsule(tasks[0].prompt) : null;
  const plan = {
    mode: "dry",
    arm,
    cli,
    model,
    tasks_file: path.relative(ROOT, tasksPath),
    task_count: tasks.length,
    runner_command_template:
      cli === "js"
        ? `PATH=<shim>:$PATH claude -p "<js-thick-capsule>\\n\\n<prompt>" --model ${model} --setting-sources ${JS_SETTING_SOURCES} ${JS_ALLOWED_TOOLS.map((t) => `--allowedTools ${JSON.stringify(t)}`).join(" ")}`
        : `claude -p "<prompt>" --model ${model}`,
    js_routing:
      cli === "js"
        ? {
            injection: `echo '{"prompt":"<prompt>"}' | node ${path.relative(ROOT, JS_BIN)} hook claude  (thick capsule additionalContext, prepended)`,
            self_run_shim: `<tmp>/lilygo-skills -> exec node ${path.relative(ROOT, JS_BIN)} "$@"  (prepended to PATH)`,
            rust_hook: "dropped via --setting-sources project,local (thick capsule cannot leak)",
            auth: "REAL config (no CLAUDE_CONFIG_DIR isolation) — only the CLI impl is swapped",
            allowed_tools: JS_ALLOWED_TOOLS,
            capsule_sample_task0: jsSample,
          }
        : "n/a (--cli rust: legacy Rust global hook injects, no rewiring)",
    bare_isolation:
      arm === "bare"
        ? "CLAUDE_CONFIG_DIR=<empty tmp> ; cwd=<neutral tmp> ; contamination scan on output"
        : "n/a (with_skill runs in repo cwd with config in place)",
    grading: {
      pass:
        arm === "with_skill"
          ? "all expected.values present (normalized) AND citation (github url / sha256) present"
          : "all expected.values present (normalized)",
      hallucination: "conflict_values hit while correct values absent",
    },
    per_task_plan: tasks.map((t) => ({
      id: t.id,
      cohort: t.cohort,
      board: t.board,
      topic: t.topic,
      expected_values: t.expected.values,
      conflict_values: t.conflict_values || [],
      would_run: `claude -p ${JSON.stringify(t.prompt)} --model ${model}`,
    })),
  };
  const text = JSON.stringify(plan, null, 2);
  if (outPath) fs.writeFileSync(outPath, text + "\n");
  console.log(text);
  process.exit(0);
}

// ---- live run ----
const perTask = [];
let runnerOk = true;
let runnerError = null;
let contaminated = false;

// A runner that cannot authenticate emits its error on STDOUT (the "answer")
// too — e.g. `claude -p` prints "Failed to authenticate. API Error: 401" or
// "Not logged in · Please run /login". If we graded that as a normal answer we
// would fabricate a score (values MISS). So scan stdout+stderr+error and treat
// any auth/credential failure as runner-unavailable, per the P0 honesty rule.
const AUTH_FAIL_RE =
  /Invalid authentication|Failed to authenticate|Not logged in|Please run \/login|API Error: 401|401 [A-Za-z ]*credential|unauthor|Forbidden|missing.*API key/i;

for (const t of tasks) {
  const res = runClaude(t.prompt);
  const answer = res.stdout;
  // Detect a dead runner: spawn error, non-zero exit with empty stdout, or an
  // auth/credential failure (which may arrive on stdout). Bail honestly rather
  // than grade an error string as a wrong answer.
  const authFailed = AUTH_FAIL_RE.test(answer + " " + res.stderr + " " + (res.error || ""));
  if (res.error || (res.status !== 0 && !answer) || authFailed) {
    runnerOk = false;
    runnerError = {
      task_id: t.id,
      status: res.status,
      timed_out: res.timedOut,
      auth_failed: authFailed,
      stdout_head: answer.slice(0, 200),
      stderr: res.stderr.slice(0, 400),
      error: res.error,
    };
    break;
  }
  const g = gradeAnswer(t, answer);
  if (arm === "bare" && CONTAMINATION_RE.test(answer)) contaminated = true;
  perTask.push({
    id: t.id,
    cohort: t.cohort,
    pass: g.pass,
    hallucination: g.hallucination,
    cited: g.cited,
    missing: g.missing,
    raw_excerpt: answer.slice(0, 500),
  });
}

cleanupJsShim();

const report = {
  schema_version: 1,
  arm,
  cli,
  model,
  date,
  runner: "claude -p",
  runner_ok: runnerOk,
  runner_error: runnerError,
  contaminated: arm === "bare" ? contaminated : null,
  tasks_file: path.relative(ROOT, tasksPath),
  per_task: perTask,
  summary: runnerOk
    ? {
        pass_n: perTask.filter((p) => p.pass).length,
        total: tasks.length,
        graded: perTask.length,
        hallucination_n: perTask.filter((p) => p.hallucination).length,
      }
    : { pass_n: null, total: tasks.length, graded: perTask.length, hallucination_n: null },
};

const text = JSON.stringify(report, null, 2);
if (outPath) fs.writeFileSync(outPath, text + "\n");
console.log(text);

// Exit 0 on a completed run; 3 signals "runner unavailable" so a caller can
// distinguish a real low score from a missing runner (and fall back honestly).
process.exit(runnerOk ? 0 : 3);
