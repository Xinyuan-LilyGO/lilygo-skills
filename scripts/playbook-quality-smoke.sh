#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "playbook-quality-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"
OUT=".tmp/m13-playbook-generated"
rm -rf "$OUT"

"$BIN" route --json "T-Watch Ultra LVGL blank screen touch debug" \
  >.tmp/playbook-route-lvgl.json
"$BIN" route --json "what is the weather today" \
  >.tmp/playbook-route-weather.json
"$BIN" context --plan --json "T-Watch Ultra LVGL blank screen touch debug" \
  >.tmp/playbook-goal-lvgl.json
"$BIN" context --plan --json "T-Watch Ultra ESP-IDF OTA rollback manifest debug" \
  >.tmp/playbook-goal-ota.json
"$BIN" context --plan --json "T-Watch Ultra add display driver BSP status action smoke" \
  >.tmp/playbook-goal-bsp.json
"$BIN" index query playbook-lvgl-debug --json \
  >.tmp/playbook-query-lvgl.json
"$BIN" index query playbook-ota-debug --json \
  >.tmp/playbook-query-ota.json
"$BIN" generate skills --out "$OUT" --json \
  >.tmp/playbook-generate.json
"$BIN" verify --generated-root "$OUT" --json \
  >.tmp/playbook-verify-generated.json

node <<'NODE'
const fs = require("fs");
function read(path) {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}
function text(path) {
  return fs.readFileSync(path, "utf8");
}
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(JSON.stringify(detail, null, 2));
    process.exit(1);
  }
}
const routeLvgl = read(".tmp/playbook-route-lvgl.json");
const routeWeather = read(".tmp/playbook-route-weather.json");
const goalLvgl = read(".tmp/playbook-goal-lvgl.json");
const goalOta = read(".tmp/playbook-goal-ota.json");
const goalBsp = read(".tmp/playbook-goal-bsp.json");
const queryLvgl = read(".tmp/playbook-query-lvgl.json");
const queryOta = read(".tmp/playbook-query-ota.json");
const generated = read(".tmp/playbook-generate.json");
const verified = read(".tmp/playbook-verify-generated.json");
const generatedText =
  text(".tmp/playbook-generate.json") +
  text(".tmp/playbook-verify-generated.json") +
  text(".tmp/playbook-goal-lvgl.json") +
  text(".tmp/playbook-goal-ota.json");

function hintIds(goal) {
  return goal.context_capsule.playbook_hints.map((hint) => hint.playbook_id);
}
function evidenceText(goal) {
  return goal.context_capsule.playbook_hints
    .flatMap((hint) => hint.evidence_targets)
    .join(" ")
    .toLowerCase();
}
function antiClaimText(goal) {
  return goal.context_capsule.playbook_hints
    .flatMap((hint) => hint.anti_claims)
    .join(" ")
    .toLowerCase();
}

check("LVGL route injects compact playbook ids",
  routeLvgl.decision === "inject" &&
  routeLvgl.skills.includes("playbook-source-discovery") &&
  routeLvgl.skills.includes("playbook-lvgl-debug") &&
  !routeLvgl.skills.includes("playbook-ota-debug"),
  routeLvgl);
check("non-embedded prompt no-op",
  routeWeather.decision === "no-op" &&
  routeWeather.skills.every((skill) => !skill.startsWith("playbook-")),
  routeWeather);
check("LVGL goal has useful evidence and anti-claim content",
  hintIds(goalLvgl).includes("playbook-lvgl-debug") &&
  evidenceText(goalLvgl).includes("simulator") &&
  antiClaimText(goalLvgl).includes("pixels"),
  goalLvgl.context_capsule.playbook_hints);
check("OTA goal pairs OTA and serial evidence",
  hintIds(goalOta).includes("playbook-ota-debug") &&
  hintIds(goalOta).includes("playbook-build-flash-serial") &&
  evidenceText(goalOta).includes("rollback") &&
  antiClaimText(goalOta).includes("credentials"),
  goalOta.context_capsule.playbook_hints);
check("BSP goal keeps board facts plus BSP playbook",
  hintIds(goalBsp).includes("playbook-bsp-driver") &&
  goalBsp.context_capsule.facts.some((fact) => fact.value === "CO5300") &&
  evidenceText(goalBsp).includes("smoke report"),
  goalBsp.context_capsule);
check("index query returns structured playbook body",
  queryLvgl.id === "playbook-lvgl-debug" &&
  queryLvgl.diagnostic_axes.length > 0 &&
  queryLvgl.evidence_targets.length > 0 &&
  queryOta.anti_claims.some((claim) => claim.includes("planning evidence")),
  {queryLvgl, queryOta});
check("generated playbook skills materialized",
  generated.status === "PASS" &&
  generated.playbook_skills === 7 &&
  verified.status === "PASS" &&
  fs.existsSync(".tmp/m13-playbook-generated/skills/playbook-lvgl-debug/SKILL.md") &&
  fs.existsSync(".tmp/m13-playbook-generated/skills/playbook-ota-debug/SKILL.md"),
  {generated, verified});
check("public playbook output has no private machine data",
  !/\/Users\/[^" ]+|\/dev\/cu|192\.168\.|token=|password|secret/.test(generatedText),
  generatedText);

process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: [
    "route playbook selection",
    "no-over-injection",
    "goal playbook usefulness",
    "index query structured playbook",
    "generated-root playbook materialization",
    "privacy boundary"
  ],
  route_skills: routeLvgl.skills,
  lvgl_playbooks: hintIds(goalLvgl),
  ota_playbooks: hintIds(goalOta),
  generated_playbooks: generated.playbook_skills
}, null, 2) + "\n");
NODE
