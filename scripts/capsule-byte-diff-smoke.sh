#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

UPDATE=0
ALLOW_WPD_UPDATE=0
for arg in "$@"; do
  case "$arg" in
    --update)
      UPDATE=1
      ;;
    --allow-versioned-wp-d-update)
      ALLOW_WPD_UPDATE=1
      ;;
    --dry-run)
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 1
      ;;
  esac
done

FIXTURE_DIR="$ROOT/test/fixtures/capsules/m25.v1"
RUN_DIR="$ROOT/.tmp/capsule-byte-diff/run-$$"
CURRENT_DIR="$RUN_DIR/current"
CACHE_DIR="$RUN_DIR/cache"
mkdir -p "$CURRENT_DIR" "$CACHE_DIR" "$FIXTURE_DIR"
rm -rf "$CURRENT_DIR" "$CACHE_DIR"
mkdir -p "$CURRENT_DIR" "$CACHE_DIR"

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

run_json() {
  local name="$1"
  shift
  "$@" >"$CURRENT_DIR/$name.json"
}

run_hook() {
  local name="$1"
  local payload="$2"
  printf '%s' "$payload" | "$BIN" hook claude >"$CURRENT_DIR/$name.json"
}

run_hook_with_cache() {
  local name="$1"
  local payload="$2"
  printf '%s' "$payload" | env LILYGO_SKILLS_CACHE_DIR="$CACHE_DIR" "$BIN" hook claude >"$CURRENT_DIR/$name.json"
}

run_json "route-display-lookup-zh" "$BIN" route --json "T-Display-S3 的 I2C 引脚和屏幕占用了哪些 GPIO?"
run_json "goal-display-first-run" "$BIN" goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor"
run_json "goal-factory-demo" "$BIN" goal plan --json "T-Display-S3 Arduino factory full peripheral test"
run_json "goal-flash-serial" "$BIN" goal complete --dry-run --json "T-Display-S3 build flash and capture serial log"
run_json "source-t-watch-imu" "$BIN" source query --board board-t-watch-ultra --topic imu --json
run_hook "hook-display-impl" '{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor"}'
run_hook "hook-lookup-zh" '{"prompt":"T-Display-S3 的 I2C 引脚和屏幕占用了哪些 GPIO?"}'
run_hook_with_cache "hook-session-full" '{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor","session_id":"m25-byte-diff-session"}'
run_hook_with_cache "hook-session-incremental" '{"prompt":"T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor","session_id":"m25-byte-diff-session"}'

node - "$FIXTURE_DIR" "$CURRENT_DIR" "$UPDATE" "$ALLOW_WPD_UPDATE" <<'NODE'
const fs = require("fs");
const path = require("path");
const fixtureDir = process.argv[2];
const currentDir = process.argv[3];
const update = process.argv[4] === "1";
const allowWpdUpdate = process.argv[5] === "1";

function listJson(dir) {
  return fs.readdirSync(dir).filter((name) => name.endsWith(".json")).sort();
}

function read(file) {
  return fs.readFileSync(file, "utf8");
}

const current = listJson(currentDir);
const changed = [];
const missing = [];
for (const file of current) {
  const fixture = path.join(fixtureDir, file);
  const actual = path.join(currentDir, file);
  if (!fs.existsSync(fixture)) {
    missing.push(file);
    continue;
  }
  if (read(fixture) !== read(actual)) {
    changed.push(file);
  }
}

if (update || allowWpdUpdate) {
  for (const file of current) {
    fs.copyFileSync(path.join(currentDir, file), path.join(fixtureDir, file));
  }
  fs.writeFileSync(
    path.join(fixtureDir, "manifest.json"),
    JSON.stringify({
      schema_version: 1,
      fixture_version: "m25.v1",
      updated_by: update ? "explicit-update" : "wp-d-versioned-update",
      files: current,
      notes: [
        "Fixtures are byte-for-byte capsule baselines. Runtime or capsule-facing changes must not update them silently."
      ]
    }, null, 2) + "\n"
  );
  console.log(JSON.stringify({
    status: "PASS",
    mode: "updated",
    fixture_version: "m25.v1",
    checked_prompts: current.length,
    changed,
    missing
  }));
  process.exit(0);
}

if (missing.length || changed.length) {
  console.error(JSON.stringify({
    status: "FAIL",
    fixture_version: "m25.v1",
    missing,
    changed,
    remediation: "Run scripts/capsule-byte-diff-smoke.sh --update only for intentional, reviewed fixture changes."
  }, null, 2));
  process.exit(1);
}

console.log(JSON.stringify({
  status: "PASS",
  fixture_version: "m25.v1",
  checked_prompts: current.length,
  changed: [],
  missing: []
}));
NODE
