#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d /tmp/lilygo-install-binary-selection.XXXXXX)"
CUSTOM_DIR="$(mktemp -d /tmp/lilygo-custom-bin.XXXXXX)"
trap 'rm -rf "$TMP" "$CUSTOM_DIR"' EXIT

mkdir -p "$TMP/target/release" "$TMP/target/debug"
cp "$ROOT/install.js" "$TMP/install.js"

write_bin() {
  local target="$1"
  mkdir -p "$(dirname "$target")"
  printf '#!/usr/bin/env sh\nexit 0\n' >"$target"
  chmod +x "$target"
}

touch_stamp() {
  local target="$1"
  local stamp="$2"
  touch -t "$stamp" "$target"
}

run_install() {
  node "$TMP/install.js" --codex --dry-run --home "$TMP/home" "$@"
}

check_profile() {
  local name="$1"
  local expected_profile="$2"
  local expected_source="$3"
  shift 3
  local output
  output="$(run_install "$@")"
  node - "$name" "$expected_profile" "$expected_source" "$output" <<'NODE'
const [name, expectedProfile, expectedSource, raw] = process.argv.slice(2);
const report = JSON.parse(raw);
const ok =
  report.status === "PASS" &&
  report.binary_profile === expectedProfile &&
  report.binary_source === expectedSource;
if (!ok) {
  console.error(JSON.stringify({ name, expectedProfile, expectedSource, report }, null, 2));
  process.exit(1);
}
NODE
}

check_runtime_mode() {
  local name="$1"
  local expected_mode="$2"
  shift 2
  local output
  output="$(run_install "$@")"
  node - "$name" "$expected_mode" "$output" <<'NODE'
const [name, expectedMode, raw] = process.argv.slice(2);
const report = JSON.parse(raw);
if (report.status !== "PASS" || report.runtime_mode !== expectedMode) {
  console.error(JSON.stringify({ name, expectedMode, report }, null, 2));
  process.exit(1);
}
NODE
}

RELEASE="$TMP/target/release/lilygo-skills"
DEBUG="$TMP/target/debug/lilygo-skills"
CUSTOM="$CUSTOM_DIR/lilygo-skills"

write_bin "$RELEASE"
write_bin "$DEBUG"
write_bin "$CUSTOM"

touch_stamp "$RELEASE" 202607030101
touch_stamp "$DEBUG" 202607030202
check_profile "auto selects newer debug" "debug" "target/debug/lilygo-skills"

touch_stamp "$RELEASE" 202607030303
touch_stamp "$DEBUG" 202607030202
check_profile "auto selects newer release" "release" "target/release/lilygo-skills"

touch_stamp "$RELEASE" 202607030101
touch_stamp "$DEBUG" 202607030404
check_profile "explicit release stays release" "release" "target/release/lilygo-skills" --profile release
check_profile "explicit debug stays debug" "debug" "target/debug/lilygo-skills" --profile debug
check_profile "auto build targets release" "release" "target/release/lilygo-skills" --build
check_profile "custom bin wins" "custom" "<redacted-path>/lilygo-skills" --bin "$CUSTOM"
rm -f "$RELEASE" "$DEBUG"
check_runtime_mode "no binary defaults to mount-only" "mount-only"

echo '{"status":"PASS","cases":7}'
