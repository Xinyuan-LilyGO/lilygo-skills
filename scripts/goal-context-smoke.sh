#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "goal-context-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" goal plan --json "T-Watch Ultra Arduino IMU 抬腕检测怎么做" \
  >.tmp/goal-context-imu.json
"$BIN" goal plan --json "T-Watch Ultra Arduino LVGL touch does not move" \
  >.tmp/goal-context-lvgl.json
"$BIN" goal plan --json "T-Watch Ultra OTA manifest downloaded then rebooted" \
  >.tmp/goal-context-ota.json
"$BIN" goal plan --json "T-Watch Ultra run official NFC demo" \
  >.tmp/goal-context-nfc.json
"$BIN" goal plan --json "Arduino IMU 抬腕检测怎么做" \
  >.tmp/goal-context-missing-board.json

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
function values(plan) {
  return new Set(plan.context_capsule.facts.map((fact) => fact.value));
}
function demos(plan) {
  return new Set(plan.context_capsule.demo_refs.map((demo) => demo.path));
}
function recipes(plan) {
  return new Set(plan.recipe_ids);
}
const imu = read(".tmp/goal-context-imu.json");
const lvgl = read(".tmp/goal-context-lvgl.json");
const ota = read(".tmp/goal-context-ota.json");
const nfc = read(".tmp/goal-context-nfc.json");
const missingBoard = read(".tmp/goal-context-missing-board.json");

check("imu planned", imu.status === "PASS" && imu.decision === "planned", imu);
check("imu route", ["board-t-watch-ultra", "periph-imu", "chip-bhi260ap", "fw-arduino", "feature-raise-to-wake"].every((skill) => imu.route.skills.includes(skill)), imu.route);
check("imu facts", ["Bosch BHI260AP", "I2C 0x28", "SensorBHI260AP"].every((fact) => values(imu).has(fact)), imu.context_capsule.facts);
check("imu demo", demos(imu).has("examples/sensor/BHI260AP_6DoF/BHI260AP_6DoF.ino"), imu.context_capsule.demo_refs);
check("imu recipes", ["recipe-run-official-demo", "recipe-build-upload-monitor", "recipe-serial-debug"].every((id) => recipes(imu).has(id)), imu.recipe_ids);
check("imu boundary", imu.context_capsule.boundary.verification_level === "V3" && imu.context_capsule.boundary.hardware_verified === false, imu.context_capsule.boundary);

check("lvgl recipe", recipes(lvgl).has("recipe-lvgl-simulator"), lvgl.recipe_ids);
check("lvgl facts", ["CO5300", "CST9217"].every((fact) => values(lvgl).has(fact)), lvgl.context_capsule.facts);
check("lvgl demo", demos(lvgl).has("examples/lvgl/get_started/get_started.ino"), lvgl.context_capsule.demo_refs);

check("ota recipe", recipes(ota).has("recipe-ota-debug") && recipes(ota).has("recipe-serial-debug"), ota.recipe_ids);
check("ota facts", values(ota).has("16MB flash + 8MB PSRAM"), ota.context_capsule.facts);

check("nfc demo", values(nfc).has("ST25R3916") && demos(nfc).has("examples/peripheral/NFC_Reader/NFC_Reader.ino"), nfc);
check("missing board clarification", missingBoard.decision === "needs_clarification" && missingBoard.missing.includes("board"), missingBoard);

process.stdout.write(JSON.stringify({
  status: "PASS",
  dry_run: true,
  goal_cases: 5,
  imu_goal_id: imu.goal_id,
  imu_recipe_ids: imu.recipe_ids,
  lvgl_recipe_ids: lvgl.recipe_ids,
  ota_recipe_ids: ota.recipe_ids,
  highest_verification_level: "V3",
  hardware_verified: false
}, null, 2) + "\n");
NODE
