#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "hardware-gold-standard-smoke only supports --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"
HOME_DIR="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-gold-home.XXXXXX")"
PROJECT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-gold-project.XXXXXX")"
trap 'rm -rf "$HOME_DIR" "$PROJECT_DIR"' EXIT

HOME="$HOME_DIR" "$BIN" doctor --json >.tmp/hardware-gold-doctor.json
"$BIN" route --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor" \
  >.tmp/hardware-gold-route.json
"$BIN" goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor" \
  >.tmp/hardware-gold-plan.json
"$BIN" generate skills --out "$PROJECT_DIR/generated-skills" --json \
  >.tmp/hardware-gold-generate.json
"$BIN" verify --generated-root "$PROJECT_DIR/generated-skills" --json \
  >.tmp/hardware-gold-verify-generated.json
"$BIN" goal start --plan .tmp/hardware-gold-plan.json --project "$PROJECT_DIR" --dry-run --json \
  >.tmp/hardware-gold-start.json

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
const doctor = read(".tmp/hardware-gold-doctor.json");
const route = read(".tmp/hardware-gold-route.json");
const plan = read(".tmp/hardware-gold-plan.json");
const generated = read(".tmp/hardware-gold-generate.json");
const verifyGenerated = read(".tmp/hardware-gold-verify-generated.json");
const start = read(".tmp/hardware-gold-start.json");
check("doctor dry-run source health pass", doctor.status === "PASS", doctor);
check("route injects board", route.skills.includes("board-t-display-s3"), route);
check("plan has bridge", (plan.context_capsule.next_actions || []).some((action) => action.id === "goal-plan-bridge"), plan.context_capsule.next_actions);
check("generated skills ready", generated.status === "PASS" && generated.skill_count > 0, generated);
check("generated root verifies", verifyGenerated.status === "PASS", verifyGenerated);
check("start dry-run wrote no evidence", start.status === "PASS" && start.dry_run === true && start.writes.length === 0, start);
check("start lists permissions", (start.required_permissions || []).includes("allow-build"), start.required_permissions);
check("dry-run does not claim hardware", start.hardware_verified === false && start.highest_verification_level === "V3", start);
process.stdout.write(JSON.stringify({
  status: "PASS",
  dry_run: true,
  highest_verification_level: "V3",
  hardware_verified: false,
  planned_chain: [
    "doctor",
    "route",
    "goal plan",
    "generate skills",
    "goal start --dry-run",
    "permissioned build/flash/serial evidence after explicit approval"
  ],
  required_permissions: start.required_permissions,
  writes: []
}, null, 2) + "\n");
NODE
