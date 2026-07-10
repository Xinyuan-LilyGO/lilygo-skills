#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"
PROMPT="T-Display-S3 PlatformIO Arduino TFT_eSPI I2C sensor screen"

"$BIN" context --plan --json "$PROMPT" >.tmp/source-recovery-goal.json
printf '{"prompt": "%s"}\n' "$PROMPT" | "$BIN" hook codex >.tmp/source-recovery-hook.json
"$BIN" source query --board board-t-display-s3 --topic i2c --json \
  >.tmp/source-recovery-i2c.json
"$BIN" source query --board board-t-display-s3 --topic io --json \
  >.tmp/source-recovery-io.json

node <<'NODE'
const fs = require("fs");

function read(path) {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}

function fail(name, detail) {
  console.error(`FAIL ${name}`);
  console.error(JSON.stringify(detail, null, 2));
  process.exit(1);
}

function containsText(value, needles) {
  const text = typeof value === "string" ? value : JSON.stringify(value);
  return needles.every((needle) => text.includes(needle));
}

const goal = read(".tmp/source-recovery-goal.json");
const hook = read(".tmp/source-recovery-hook.json");
const i2c = read(".tmp/source-recovery-i2c.json");
const io = read(".tmp/source-recovery-io.json");
const hookContext = hook.context || "";

if (goal.status !== "PASS" || goal.route.board !== "board-t-display-s3") {
  fail("goal route", goal);
}
if (!containsText(goal.context_capsule, [
  "implementation_start",
  "official-demo-first",
  "examples/tft/tft.ino",
  "Setup206_LilyGo_T_Display_S3.h",
  "pin_config.h",
  "critical_facts",
  "PIN_IIC_SDA=GPIO18",
  "PIN_IIC_SCL=GPIO17",
  "recovery_actions",
  "playbook-source-discovery"
])) {
  fail("goal source recovery fields", goal.context_capsule);
}
if (!containsText(hookContext, [
  "examples/tft/tft.ino",
  "Setup206_LilyGo_T_Display_S3.h",
  "pin_config.h",
  "PIN_IIC_SDA=GPIO18",
  "PIN_IIC_SCL=GPIO17",
  "index query playbook-source-discovery --json"
])) {
  fail("hook compact context", hook);
}
if (!containsText(i2c.facts, [
  "bus.i2c.primary",
  "PIN_IIC_SDA=GPIO18",
  "PIN_IIC_SCL=GPIO17"
])) {
  fail("i2c source facts", i2c);
}
if (!containsText(io.facts, [
  "pin.i2c.sda",
  "pin.i2c.scl",
  "pin.touch.int",
  "pin.sd.cmd"
])) {
  fail("io source facts", io);
}

process.stdout.write(JSON.stringify({
  status: "PASS",
  prompt: goal.prompt,
  goal_id: goal.goal_id,
  route_skills: goal.route.skills,
  demo_refs: goal.context_capsule.demo_refs.map((demo) => demo.path),
  critical_facts: goal.context_capsule.critical_facts.map((fact) => fact.value),
  recovery_actions: goal.context_capsule.recovery_actions.map((action) => action.command),
  highest_verification_level: "V3",
  hardware_verified: false
}, null, 2) + "\n");
NODE
