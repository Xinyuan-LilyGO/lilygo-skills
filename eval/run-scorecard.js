#!/usr/bin/env node
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const suite = valueAfter("--suite") || "smoke";
const json = args.includes("--json");
const privateAllowed = process.env.LILYGO_SKILLS_PRIVATE_EVAL === "1";
const artifactPath = valueAfter("--artifacts") || process.env.LILYGO_SKILLS_SCORECARD_ARTIFACTS;

function valueAfter(flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
}

function readJson(relative) {
  return JSON.parse(fs.readFileSync(path.join(root, relative), "utf8"));
}

function fail(reason, detail) {
  const output = { status: "FAIL", reason, detail };
  process.stderr.write(JSON.stringify(output, null, 2) + "\n");
  process.exit(1);
}

function readArtifact(filePath) {
  const resolved = path.resolve(filePath);
  if (!fs.existsSync(resolved)) {
    fail("private_runner_artifacts_missing", { path: resolved });
  }
  return JSON.parse(fs.readFileSync(resolved, "utf8"));
}

const tasks = readJson("eval/tasks.json").tasks;
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

if (!artifactPath) {
  fail("private_runner_artifacts_required", {
    suite,
    planned_tasks: taskCount,
    required_hosts: ["claude-code", "codex"],
    expected: "--artifacts <runner-report.json> or LILYGO_SKILLS_SCORECARD_ARTIFACTS"
  });
}

const artifact = readArtifact(artifactPath);
const hosts = new Set((artifact.records || []).map((record) => record.host));
const missingHosts = ["claude-code", "codex"].filter((host) => !hosts.has(host));
if (missingHosts.length) {
  fail("private_runner_hosts_missing", { missing_hosts: missingHosts, artifact: artifactPath });
}
const taskIds = new Set(tasks.slice(0, taskCount).map((task) => task.id));
for (const host of ["claude-code", "codex"]) {
  const hostTaskIds = new Set((artifact.records || []).filter((record) => record.host === host).map((record) => record.task_id));
  const missingTasks = [...taskIds].filter((id) => !hostTaskIds.has(id));
  if (missingTasks.length) {
    fail("private_runner_tasks_missing", { host, missing_tasks: missingTasks, artifact: artifactPath });
  }
}
if ((artifact.records || []).some((record) => record.status !== "PASS")) {
  fail("private_runner_record_failed", { artifact: artifactPath });
}

const output = {
  status: "PASS",
  mode: "private-runner-artifacts",
  suite,
  planned_tasks: taskCount,
  artifact: path.resolve(artifactPath),
  hosts: [...hosts].sort(),
  records: artifact.records.length
};

process.stdout.write((json ? JSON.stringify(output, null, 2) : output.status) + "\n");
