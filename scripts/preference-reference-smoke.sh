#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "preference-reference-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

PROJECT_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-pref-ref.XXXXXX")"
mkdir -p "$PROJECT_ROOT/.lilygo-skills"

cat >"$PROJECT_ROOT/.lilygo-skills/preferences.json" <<'JSON'
{
  "schema_version": 1,
  "framework_order": ["platformio", "arduino", "esp-idf", "rust"],
  "debug_tools": ["binflow", "serial-mcp-server", "espflash"],
  "code_limits": {
    "max_function_lines": 48,
    "max_file_lines": 420,
    "max_nesting": 3
  },
  "hardware_safety": {
    "prefer_dry_run": true,
    "require_explicit_flash": true
  }
}
JSON

cat >"$PROJECT_ROOT/.lilygo-skills/references.json" <<'JSON'
{
  "schema_version": 1,
  "entries": [{
    "id": "project-watch-debug-note",
    "title": "Project watch debug note",
    "kind": "local-doc",
    "applies_to": ["watch", "debug"],
    "path_or_url": "doc/watch-debug.md",
    "authority": "operating-pattern",
    "summary": "Project-local read hint for watch debug flow.",
    "read_when": "User asks to debug the watch firmware in this project.",
    "inject_triggers": ["watch", "debug", "调试"]
  }]
}
JSON

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" preference show --json >.tmp/preference-default.json
"$BIN" preference show --project "$PROJECT_ROOT" --json >.tmp/preference-project.json
"$BIN" reference list --project "$PROJECT_ROOT" --json >.tmp/reference-project.json
"$BIN" goal plan --project "$PROJECT_ROOT" --json "T-Watch Ultra 用 binflow 传输并串口调试" \
  >.tmp/preference-reference-goal.json

PRIVATE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-pref-private.XXXXXX")"
mkdir -p "$PRIVATE_ROOT/.lilygo-skills"
cat >"$PRIVATE_ROOT/.lilygo-skills/preferences.json" <<'JSON'
{
  "schema_version": 1,
  "serial_port": "private-device"
}
JSON
if "$BIN" preference show --project "$PRIVATE_ROOT" --json >.tmp/preference-private.json 2>.tmp/preference-private.err; then
  echo "FAIL private preference was accepted" >&2
  cat .tmp/preference-private.json >&2
  exit 1
fi

PRIVATE_VALUES_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-pref-private-values.XXXXXX")"
mkdir -p "$PRIVATE_VALUES_ROOT/.lilygo-skills"
cat >"$PRIVATE_VALUES_ROOT/.lilygo-skills/preferences.json" <<'JSON'
{
  "schema_version": 1,
  "debug_tools": [
    "/dev/cu.usbmodem-private",
    "token=abc123",
    "host=192.168.1.40",
    "watch.local",
    "/private/source",
    ".lilygo-skills/evidence/raw-log.txt"
  ]
}
JSON
if "$BIN" preference show --project "$PRIVATE_VALUES_ROOT" --json >.tmp/preference-private-values.json 2>.tmp/preference-private-values.err; then
  echo "FAIL private preference values were accepted" >&2
  cat .tmp/preference-private-values.json >&2
  exit 1
fi
"$BIN" goal plan --project "$PRIVATE_VALUES_ROOT" --json "T-Watch Ultra debug prompt" \
  >.tmp/preference-private-values-goal.json

node <<'NODE'
const fs = require("fs");
function read(path) {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(JSON.stringify(detail, null, 2));
    process.exit(1);
  }
}
const defaults = read(".tmp/preference-default.json");
const project = read(".tmp/preference-project.json");
const refs = read(".tmp/reference-project.json");
const goal = read(".tmp/preference-reference-goal.json");
const privateErr = fs.readFileSync(".tmp/preference-private.err", "utf8");
const privateValueErr = fs.readFileSync(".tmp/preference-private-values.err", "utf8");
const privateValueGoalText = fs.readFileSync(".tmp/preference-private-values-goal.json", "utf8");
const publicReferenceText = fs.readFileSync(".tmp/reference-project.json", "utf8") +
  fs.readFileSync(".tmp/preference-reference-goal.json", "utf8");

check("default preferences", defaults.status === "PASS" &&
  defaults.effective.debug_tools.includes("binflow"), defaults);
check("project preference precedence", project.effective.framework_order[0] === "platformio" &&
  project.effective.code_limits.max_function_lines === 48, project);
check("private preference rejected", privateErr.includes("private preference fields"), privateErr);
check("private preference values rejected",
  privateValueErr.includes("private preference fields or values") &&
  privateValueErr.includes("serial-device") &&
  privateValueErr.includes("credential-value") &&
  !privateValueErr.includes("abc123") &&
  !privateValueErr.includes("/dev/cu.usbmodem-private"),
  privateValueErr);
check("private values not injected into goal",
  !/\/dev\/cu|token=|abc123|192\.168\.1\.40|watch\.local|\/Users\/private/.test(privateValueGoalText),
  privateValueGoalText);
check("reference catalog includes built-in and project entries",
  refs.entries.some((entry) => entry.id === "ref-serial-mcp-server") &&
  refs.entries.some((entry) => entry.id === "project-watch-debug-note"), refs.entries);
check("built-in references are portable public URLs",
  refs.entries.filter((entry) => entry.id.startsWith("ref-")).every((entry) =>
    entry.path_or_url.startsWith("https://") ||
    entry.path_or_url.startsWith("http://") ||
    entry.path_or_url.startsWith("binflow://")) &&
  !/\/(Users|home)\/[^\/"]+\//.test(publicReferenceText),
  refs.entries);
check("goal injects targeted preference/reference hints",
  goal.context_capsule.preferences.some((hint) => hint.key === "debug_tools" && hint.value.includes("binflow")) &&
  goal.context_capsule.reference_hints.some((hint) => hint.reference_id === "ref-binflow-transfer") &&
  goal.context_capsule.reference_hints.length <= goal.context_capsule.budget.max_reference_hints_inline,
  goal.context_capsule);

process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: [
    "default preference show",
    "project preference precedence",
    "preference privacy rejection",
    "preference private value rejection",
    "private values not injected into goal",
    "reference catalog list",
    "portable reference hints",
    "targeted preference/reference goal hints"
  ],
  preference_sources: project.sources,
  reference_hint_ids: goal.context_capsule.reference_hints.map((hint) => hint.reference_id)
}, null, 2) + "\n");
NODE
