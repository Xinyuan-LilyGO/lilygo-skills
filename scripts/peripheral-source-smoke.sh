#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "peripheral-source-smoke requires --dry-run for unattended runs" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

node <<'NODE' >.tmp/peripheral-source-before.json
const fs = require("fs");
const crypto = require("crypto");
const paths = [
  "data/peripherals/source-packs.json",
  "index/routes.json",
  "skills/periph-imu/SKILL.md",
  "skills/chip-bhi260ap/SKILL.md",
  "skills/feature-raise-to-wake/SKILL.md"
];
const hashes = Object.fromEntries(paths.map((path) => {
  if (!fs.existsSync(path)) return [path, null];
  return [path, crypto.createHash("sha256").update(fs.readFileSync(path)).digest("hex")];
}));
process.stdout.write(JSON.stringify(hashes, null, 2) + "\n");
NODE

cargo run -q -p lilygo-skills-cli -- update source-packs --dry-run --json >.tmp/peripheral-update-source-packs.json
cargo run -q -p lilygo-skills-cli -- update peripheral-skills --dry-run --json >.tmp/peripheral-update-skills.json
cargo run -q -p lilygo-skills-cli -- route --json "T-Watch Ultra Arduino IMU 抬腕检测怎么做" >.tmp/peripheral-route.json
cargo run -q -p lilygo-skills-cli -- index query chip-bhi260ap --json >.tmp/peripheral-chip-query.json
cargo run -q -p lilygo-skills-cli -- benchmark --json --iterations 100 >.tmp/peripheral-benchmark.json

node <<'NODE'
const fs = require("fs");
const crypto = require("crypto");

function read(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

const before = read(".tmp/peripheral-source-before.json");
const sourcePacks = read(".tmp/peripheral-update-source-packs.json");
const peripheralSkills = read(".tmp/peripheral-update-skills.json");
const route = read(".tmp/peripheral-route.json");
const chip = read(".tmp/peripheral-chip-query.json");
const benchmark = read(".tmp/peripheral-benchmark.json");
const pack = sourcePacks.packs.find((item) => item.id === "periph-pack-t-watch-ultra-imu-bhi260ap");
const noSourceWrites = [...(peripheralSkills.planned_writes || []), ...(peripheralSkills.writes || [])]
  .every((item) => !/^(skills\/|index\/routes\.json$)/.test(item));
const hashesUnchanged = Object.entries(before).every(([path, hash]) => {
  if (!fs.existsSync(path)) return hash === null;
  const current = crypto.createHash("sha256").update(fs.readFileSync(path)).digest("hex");
  return current === hash;
});

const expectedRoute = [
  "lilygo-router",
  "board-t-watch-ultra",
  "periph-imu",
  "chip-bhi260ap",
  "fw-arduino",
  "feature-raise-to-wake"
];
const routeOk =
  route.decision === "inject" &&
  expectedRoute.every((skill) => route.skills.includes(skill)) &&
  route.verification_level === "context-injection" &&
  route.hardware_verified === false &&
  route.hardware_verification_boundary === true;
const sourceOk =
  sourcePacks.status === "PASS" &&
  sourcePacks.dry_run === true &&
  sourcePacks.writes.length === 0 &&
  pack &&
  ["chip-vendor", "lilygo-hardware", "lilygo-driver", "arduino-example", "framework-official"]
    .every((dimension) => pack.source_dimensions.includes(dimension));
const skillOk =
  peripheralSkills.status === "PASS" &&
  peripheralSkills.dry_run === true &&
  peripheralSkills.writes.length === 0 &&
  noSourceWrites &&
  peripheralSkills.skill_ids.includes("periph-imu") &&
  peripheralSkills.skill_ids.includes("chip-bhi260ap") &&
  peripheralSkills.skill_ids.includes("feature-raise-to-wake") &&
  chip.id === "chip-bhi260ap" &&
  chip.path === "skills/chip-bhi260ap/SKILL.md";
const benchmarkOk =
  benchmark.status === "PASS" &&
  benchmark.baseline_comparison.status === "PASS" &&
  benchmark.baseline_comparison.baseline_case_count === 63 &&
  benchmark.baseline_comparison.added_case_count >= 12;
const ok = hashesUnchanged && routeOk && sourceOk && skillOk && benchmarkOk;

process.stdout.write(JSON.stringify({
  status: ok ? "PASS" : "FAIL",
  dry_run: true,
  source_pack_count: sourcePacks.source_pack_count,
  generated_skill_count: peripheralSkills.generated_skill_count,
  route_skills: route.skills,
  chip_query: chip.id,
  source_dimensions: pack ? pack.source_dimensions : [],
  baseline_comparison: benchmark.baseline_comparison,
  route_quality: {
    m6_baseline: ["lilygo-router", "board-t-watch-ultra", "fw-arduino"],
    m7_expected: expectedRoute,
    improved: routeOk
  },
  evidence_boundary: {
    verification_level: route.verification_level,
    hardware_verified: route.hardware_verified,
    highest_verification_level: "V3"
  },
  dry_run_non_mutating: hashesUnchanged,
  writes: []
}, null, 2) + "\n");
process.exit(ok ? 0 : 2);
NODE
