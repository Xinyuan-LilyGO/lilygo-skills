#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -p lilygo-skills-cli >.tmp/runtime-degradation-build.log 2>&1
RUN_HOME="/tmp/lilygo-skills-runtime.$$.$RANDOM"
mkdir -p "$RUN_HOME"
node install.js --all --home "$RUN_HOME" >.tmp/runtime-degradation-install.json

node - "$RUN_HOME" <<'NODE'
const cp = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const repo = process.cwd();
const home = process.argv[2];
const install = JSON.parse(fs.readFileSync(".tmp/runtime-degradation-install.json", "utf8"));
const commands = [];
const checks = [];

function hostRoot(host) {
  return path.join(home, host === "codex" ? ".codex" : ".claude", "lilygo-skills");
}

function run(name, cwd, args, input = "") {
  const result = cp.spawnSync(args[0], args.slice(1), {
    cwd,
    input,
    encoding: "utf8"
  });
  const record = {
    name,
    cwd,
    cmd: args,
    exit_code: result.status,
    stdout: result.stdout,
    stderr: result.stderr
  };
  try {
    record.json = JSON.parse(result.stdout);
  } catch (_) {
    record.json = null;
  }
  commands.push({
    name,
    exit_code: record.exit_code,
    stdout_preview: result.stdout.slice(0, 600),
    stderr_preview: result.stderr.slice(0, 600)
  });
  return record;
}

function check(name, ok, detail = null) {
  checks.push({ name, status: ok ? "PASS" : "FAIL", detail });
}

function copyRuntime(src) {
  const dst = fs.mkdtempSync(path.join(os.tmpdir(), "lilygo-skills-broken."));
  fs.cpSync(src, dst, { recursive: true });
  return dst;
}

function executable(pathname) {
  return fs.existsSync(pathname) && (fs.statSync(pathname).mode & 0o111) !== 0;
}

function verifyInstalledHost(host) {
  const root = hostRoot(host);
  const bin = path.join(root, "bin", "lilygo-skills");
  check(`${host}: installed binary executable`, executable(bin), bin);

  const verify = run(`${host}: verify`, root, [bin, "verify", "--json"]);
  check(`${host}: verify PASS`, verify.exit_code === 0 && verify.json?.status === "PASS", verify.json);

  const noSkill = run(`${host}: route no skill`, root, [
    bin,
    "route",
    "--json",
    "Generic ESP32 LVGL screen is blank"
  ]);
  check(`${host}: no-skill route is empty`, noSkill.json?.decision === "no-op" && noSkill.json?.skills?.length === 0, noSkill.json);

  const route = run(`${host}: route positive`, root, [
    bin,
    "route",
    "--json",
    "LilyGO T-Watch Ultra ESP-IDF LVGL serial"
  ]);
  const routeSkills = route.json?.skills ?? [];
  const routeOk =
    routeSkills.includes("board-t-watch-ultra") &&
    !routeSkills.includes("board-t-watch") &&
    routeSkills.includes("fw-esp-idf") &&
    routeSkills.includes("fw-lvgl") &&
    routeSkills.includes("periph-display") &&
    routeSkills.includes("debug-flash-serial") &&
    route.json?.hardware_verified === false;
  check(`${host}: positive route injects`, routeOk, route.json);

  const hookNoSkill = run(
    `${host}: hook no skill`,
    root,
    [bin, "hook", host],
    "Generic ESP32 LVGL screen is blank\n"
  );
  check(`${host}: hook no skill has empty context`,
    host === "claude"
      ? hookNoSkill.json?.hookSpecificOutput?.hookEventName === "UserPromptSubmit" &&
        !("additionalContext" in (hookNoSkill.json?.hookSpecificOutput ?? {}))
      : hookNoSkill.json?.decision === "no-op" && hookNoSkill.json?.context === "",
    hookNoSkill.json);

  const hookPositive = run(
    `${host}: hook positive`,
    root,
    [bin, "hook", host],
    "LilyGO T-Watch Ultra ESP-IDF LVGL serial\n"
  );
  check(`${host}: hook positive injects context`,
    host === "claude"
      ? hookPositive.json?.hookSpecificOutput?.additionalContext?.includes("board-t-watch-ultra")
      : hookPositive.json?.context?.includes("board-t-watch-ultra"),
    hookPositive.json);
}

function verifyBrokenRuntime() {
  const sourceRoot = hostRoot("codex");
  const bin = path.join(sourceRoot, "bin", "lilygo-skills");
  const empty = fs.mkdtempSync(path.join(os.tmpdir(), "lilygo-skills-empty."));
  const emptyHook = run("empty cwd hook installed runtime", empty, [bin, "hook", "codex"], "LilyGO T-Watch Ultra serial\n");
  check(
    "empty cwd hook installed runtime injects",
    emptyHook.exit_code === 0 &&
      emptyHook.json?.fail_open === true &&
      emptyHook.json?.decision === "inject" &&
      emptyHook.json?.context?.includes("board-t-watch-ultra"),
    emptyHook.json
  );

  const invalid = fs.mkdtempSync(path.join(os.tmpdir(), "lilygo-skills-invalid."));
  fs.mkdirSync(path.join(invalid, "index"), { recursive: true });
  fs.writeFileSync(path.join(invalid, "index", "routes.json"), "{not json");
  const invalidVerify = run("invalid registry verify fails", invalid, [bin, "verify", "--json"]);
  check("invalid registry verify fails", invalidVerify.exit_code !== 0 && invalidVerify.json?.status === "FAIL", invalidVerify.json);
  const invalidHook = run("invalid registry hook fail-open", invalid, [bin, "hook", "codex"], "LilyGO T-Watch serial\n");
  check("invalid registry hook fail-open", invalidHook.exit_code === 0 && invalidHook.json?.fail_open === true && invalidHook.json?.context === "", invalidHook.json);

  const missingSkill = copyRuntime(sourceRoot);
  fs.rmSync(path.join(missingSkill, "skills", "fw-lvgl", "SKILL.md"));
  const missingVerify = run("missing skill verify fails", missingSkill, [bin, "verify", "--json"]);
  check("missing skill verify fails", missingVerify.exit_code !== 0 && /missing skill file/.test(JSON.stringify(missingVerify.json)), missingVerify.json);
  const missingRoute = run("missing skill route fails", missingSkill, [bin, "route", "--json", "LilyGO T-Watch LVGL"]);
  check("missing skill route fails", missingRoute.exit_code !== 0 && /missing skill file/.test(missingRoute.stderr), missingRoute.stderr);
  const missingHook = run("missing skill hook fail-open", missingSkill, [bin, "hook", "codex"], "LilyGO T-Watch LVGL\n");
  check("missing skill hook fail-open", missingHook.exit_code === 0 && missingHook.json?.fail_open === true && missingHook.json?.context === "", missingHook.json);

  const missingData = copyRuntime(sourceRoot);
  fs.rmSync(path.join(missingData, "data", "boards.json"));
  const dataVerify = run("missing board data verify fails", missingData, [bin, "verify", "--json"]);
  check("missing board data verify fails", dataVerify.exit_code !== 0 && dataVerify.json?.status === "FAIL", dataVerify.json);
}

for (const host of ["codex", "claude"]) {
  verifyInstalledHost(host);
}
verifyBrokenRuntime();

check("installer apply PASS", install.status === "PASS" && install.errors.length === 0, install);
check("installer verified all writes", install.writes.length === install.verified_writes.length, install);

const failed = checks.filter((entry) => entry.status !== "PASS");
const report = {
  status: failed.length === 0 ? "PASS" : "FAIL",
  temp_home: home,
  checks,
  command_count: commands.length,
  failed_checks: failed,
  install_writes: install.writes.length,
  install_verified_writes: install.verified_writes.length,
  commands
};
fs.writeFileSync(".tmp/runtime-degradation-smoke.json", JSON.stringify(report, null, 2));
process.stdout.write(JSON.stringify(report, null, 2) + "\n");
process.exit(report.status === "PASS" ? 0 : 2);
NODE
