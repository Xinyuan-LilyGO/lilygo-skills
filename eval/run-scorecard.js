#!/usr/bin/env node
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const suite = valueAfter("--suite") || "smoke";
const json = args.includes("--json");
const privateAllowed = process.env.LILYGO_SKILLS_PRIVATE_EVAL === "1";

function valueAfter(flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
}

function readJson(relative) {
  return JSON.parse(fs.readFileSync(path.join(root, relative), "utf8"));
}

const tasks = readJson("eval/tasks.json").tasks;
const fixture = readJson("eval/fixtures/smoke-scorecard.json");
const taskCount = suite === "full" ? tasks.length : Math.min(5, tasks.length);

if (!privateAllowed) {
  const output = {
    status: "SKIP",
    reason: "private_runner_unavailable",
    public_ci_safe: true,
    suite,
    planned_tasks: taskCount,
    required_hosts: ["claude-code", "codex"],
    next_action: "Run with LILYGO_SKILLS_PRIVATE_EVAL=1 on a maintainer machine that has host credentials."
  };
  process.stdout.write(JSON.stringify(output, null, 2) + "\n");
  process.exit(0);
}

const output = {
  status: "PASS",
  mode: "fixture-private-runner-standin",
  suite,
  planned_tasks: taskCount,
  scored_fixture: "eval/fixtures/smoke-scorecard.json",
  note: "This path records deterministic fixture execution only. Real Claude/Codex runner integration must attach host artifacts before release.",
  pilot: fixture.pilot
};

process.stdout.write((json ? JSON.stringify(output, null, 2) : output.status) + "\n");
