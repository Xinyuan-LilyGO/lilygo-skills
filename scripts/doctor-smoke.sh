#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

home="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-doctor-home.XXXXXX")"
trap 'rm -rf "$home"' EXIT

cargo build -q -p lilygo-skills-cli
bin="$ROOT/target/debug/lilygo-skills"

source_report="$(HOME="$home" "$bin" doctor --json)"
SOURCE_JSON="$source_report" node <<'NODE'
const report = JSON.parse(process.env.SOURCE_JSON);
if (report.status !== "PASS" || report.sample_injection.status !== "PASS") {
  throw new Error("source doctor did not pass sample injection");
}
const active = report.checks.find((item) => item.id === "active_wiring");
if (!active || active.status !== "WARN") {
  throw new Error(`source doctor did not inspect default active wiring: ${JSON.stringify(report.checks)}`);
}
NODE

install_report="$(node install.js --all --profile debug --home "$home")"
INSTALL_JSON="$install_report" node <<'NODE'
const report = JSON.parse(process.env.INSTALL_JSON);
if (report.status !== "PASS") {
  throw new Error(`install failed: ${report.errors?.join("; ")}`);
}
if (!Array.isArray(report.self_tests) || report.self_tests.length !== 2) {
  throw new Error("install did not run both self-tests");
}
if (!report.self_tests.every((test) => test.status === "PASS")) {
  throw new Error(`install self-test failed: ${JSON.stringify(report.self_tests)}`);
}
NODE

installed_bin="$home/.codex/lilygo-skills/bin/lilygo-skills"
installed_report="$(HOME="$home" "$installed_bin" doctor --json)"
INSTALLED_JSON="$installed_report" node <<'NODE'
const report = JSON.parse(process.env.INSTALLED_JSON);
if (report.status !== "PASS" || report.sample_injection.status !== "PASS") {
  throw new Error("installed doctor did not pass sample injection");
}
for (const id of ["active_wiring", "codex-agents", "claude-skill", "claude-hook", "runtime-parity"]) {
  const check = report.checks.find((item) => item.id === id);
  if (!check || check.status !== "PASS") {
    throw new Error(`${id} check did not pass`);
  }
}
NODE

printf '{"schema_version":1,"drift":true}\n' >"$home/.claude/lilygo-skills/data/boards.json"
drift_report="$(HOME="$home" "$installed_bin" doctor --json)"
DRIFT_JSON="$drift_report" node <<'NODE'
const report = JSON.parse(process.env.DRIFT_JSON);
const parity = report.checks.find((item) => item.id === "runtime-parity");
if (!parity || parity.status !== "WARN") {
  throw new Error(`runtime parity did not warn on drift: ${JSON.stringify(report.checks)}`);
}
if (!parity.summary.includes("node install.js --all --build")) {
  throw new Error(`runtime parity did not include remediation: ${parity.summary}`);
}
console.log(JSON.stringify({ status: "PASS", self_tests: 2, runtime_parity_drift: "WARN" }));
NODE
