#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

source_report="$(cargo run -q -p lilygo-skills-cli -- doctor --json)"
SOURCE_JSON="$source_report" node <<'NODE'
const report = JSON.parse(process.env.SOURCE_JSON);
if (report.status !== "PASS" || report.sample_injection.status !== "PASS") {
  throw new Error("source doctor did not pass sample injection");
}
NODE

home="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-doctor-home.XXXXXX")"
trap 'rm -rf "$home"' EXIT

cargo build -q -p lilygo-skills-cli
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

bin="$home/.codex/lilygo-skills/bin/lilygo-skills"
installed_report="$("$bin" doctor --json --home "$home")"
INSTALLED_JSON="$installed_report" node <<'NODE'
const report = JSON.parse(process.env.INSTALLED_JSON);
if (report.status !== "PASS" || report.sample_injection.status !== "PASS") {
  throw new Error("installed doctor did not pass sample injection");
}
for (const id of ["codex-agents", "claude-skill", "claude-hook"]) {
  const check = report.checks.find((item) => item.id === id);
  if (!check || check.status !== "PASS") {
    throw new Error(`${id} check did not pass`);
  }
}
console.log(JSON.stringify({ status: "PASS", self_tests: 2 }));
NODE
