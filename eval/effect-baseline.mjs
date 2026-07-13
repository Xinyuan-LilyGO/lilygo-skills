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
const tasksPath = flag("--tasks", path.join(ROOT, "eval/fixtures/effect-tasks.json"));
const outPath = flag("--out", null);
const dry = hasFlag("--dry");
const timeoutMs = Number(flag("--timeout-ms", "120000"));

if (!["with_skill", "bare"].includes(arm)) {
  console.error(`bad --arm '${arm}' (expected with_skill|bare)`);
  process.exit(2);
}

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

function runClaude(prompt) {
  const args = ["-p", prompt, "--model", model];
  const opts = { encoding: "utf8", timeout: timeoutMs, maxBuffer: 32 * 1024 * 1024 };
  if (arm === "bare") {
    const { env, cwd } = bareEnvAndCwd();
    opts.env = env;
    opts.cwd = cwd;
  } else {
    // with_skill: run in the repo (our skill/hook config in place).
    opts.cwd = ROOT;
  }
  const r = spawnSync("claude", args, opts);
  return {
    status: r.status,
    stdout: (r.stdout || "").trim(),
    stderr: (r.stderr || "").trim(),
    error: r.error ? String(r.error.message || r.error) : null,
    timedOut: r.signal === "SIGTERM" || (r.error && r.error.code === "ETIMEDOUT"),
  };
}

// ---- dry plan ----
if (dry) {
  const plan = {
    mode: "dry",
    arm,
    model,
    tasks_file: path.relative(ROOT, tasksPath),
    task_count: tasks.length,
    runner_command_template: `claude -p "<prompt>" --model ${model}`,
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

const report = {
  schema_version: 1,
  arm,
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
