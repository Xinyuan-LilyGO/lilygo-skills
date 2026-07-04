#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "goal-complete-permission-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp
PROJECT_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-complete-project.XXXXXX")"
SOURCE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-complete-source.XXXXXX")"
export PROJECT_ROOT SOURCE_ROOT
trap 'rm -rf "$PROJECT_ROOT" "$SOURCE_ROOT"' EXIT

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" goal complete --json "T-Watch Ultra Rust build firmware" \
  --project "$PROJECT_ROOT" \
  >.tmp/goal-complete-permission-default.json
"$BIN" goal complete --dry-run --allow-build --allow-flash --allow-serial \
  --port /dev/cu.lilygo-private-goal-complete \
  --source-root "$SOURCE_ROOT" \
  --project "$PROJECT_ROOT" \
  --json "T-Watch Ultra Rust build firmware" \
  >.tmp/goal-complete-permission-authorized-dry.json

node <<'NODE'
const fs = require("fs");
const defaults = JSON.parse(fs.readFileSync(".tmp/goal-complete-permission-default.json", "utf8"));
const authorizedDry = JSON.parse(fs.readFileSync(".tmp/goal-complete-permission-authorized-dry.json", "utf8"));
const defaultText = fs.readFileSync(".tmp/goal-complete-permission-default.json", "utf8");
const authorizedText = fs.readFileSync(".tmp/goal-complete-permission-authorized-dry.json", "utf8");
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(typeof detail === "string" ? detail : JSON.stringify(detail, null, 2));
    process.exit(1);
  }
}
check("default is permission-gated",
  defaults.status === "needs_permission" &&
  defaults.execution.attempted === false &&
  defaults.plan.required_permissions.includes("allow-build"),
  defaults);
check("explicit dry-run still does not execute",
  authorizedDry.status === "needs_permission" &&
  authorizedDry.execution.attempted === false &&
  authorizedDry.evidence.highest_verification_level === "V3",
  authorizedDry);
check("permission next action present",
  authorizedDry.next_actions.some((action) =>
    action.kind === "request_permission" &&
    action.command.includes("allow-build")),
  authorizedDry);
check("no evidence writes in dry-run",
  !fs.existsSync(`${process.env.PROJECT_ROOT}/.lilygo-skills/evidence`),
  process.env.PROJECT_ROOT);
for (const [name, text] of [["default", defaultText], ["authorizedDry", authorizedText]]) {
  check(`${name} hides private project root`, !text.includes(process.env.PROJECT_ROOT), text);
  check(`${name} hides private source root`, !text.includes(process.env.SOURCE_ROOT), text);
  check(`${name} hides private port`, !text.includes("/dev/cu.lilygo-private-goal-complete"), text);
}
process.stdout.write(JSON.stringify({
  status: "PASS",
  default_status: defaults.status,
  authorized_dry_status: authorizedDry.status,
  execution_attempted: authorizedDry.execution.attempted,
  evidence_written: false
}, null, 2) + "\n");
NODE
