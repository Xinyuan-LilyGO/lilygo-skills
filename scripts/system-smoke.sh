#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo test -p lilygo-skills-cli hook_envelopes >/tmp/lilygo-hook-test.log 2>&1
cargo run -p lilygo-skills-cli -- verify --json >.tmp/system-verify.json
cargo run -p lilygo-skills-cli -- sync-boards --dry-run --json >.tmp/system-sync.json
cargo run -p lilygo-skills-cli -- update sources --dry-run --json >.tmp/system-update-sources.json
cargo run -p lilygo-skills-cli -- update boards --dry-run --json >.tmp/system-update-boards.json
cargo run -p lilygo-skills-cli -- update skills --dry-run --json >.tmp/system-update-skills.json
cargo run -p lilygo-skills-cli -- update source-packs --dry-run --json >.tmp/system-update-source-packs.json
cargo run -p lilygo-skills-cli -- update fact-packs --dry-run --json >.tmp/system-update-fact-packs.json
cargo run -p lilygo-skills-cli -- update peripheral-skills --dry-run --json >.tmp/system-update-peripheral-skills.json
cargo run -p lilygo-skills-cli -- update runtime --dry-run --json >.tmp/system-update-runtime.json
cargo run -p lilygo-skills-cli -- route --json "T-Display-S3 Arduino LVGL" >.tmp/system-route.json
node install.js --codex --dry-run >.tmp/install-codex.json
node install.js --claude --dry-run >.tmp/install-claude.json

node <<'NODE'
const fs = require("fs");
const files = [
  ".tmp/system-verify.json",
  ".tmp/system-sync.json",
  ".tmp/system-update-sources.json",
  ".tmp/system-update-boards.json",
  ".tmp/system-update-skills.json",
  ".tmp/system-update-source-packs.json",
  ".tmp/system-update-fact-packs.json",
  ".tmp/system-update-peripheral-skills.json",
  ".tmp/system-update-runtime.json",
  ".tmp/system-route.json",
  ".tmp/install-codex.json",
  ".tmp/install-claude.json"
];
const data = Object.fromEntries(files.map((file) => [file, JSON.parse(fs.readFileSync(file, "utf8"))]));
function noSourceWrites(report) {
  const forbidden = /^(skills\/|index\/routes\.json$)/;
  return [...(report.planned_writes || []), ...(report.writes || [])]
    .every((item) => !forbidden.test(item));
}
const ok =
  data[".tmp/system-verify.json"].status === "PASS" &&
  data[".tmp/system-sync.json"].dry_run === true &&
  data[".tmp/system-update-sources.json"].status === "PASS" &&
  data[".tmp/system-update-boards.json"].status === "PASS" &&
  data[".tmp/system-update-skills.json"].status === "PASS" &&
  noSourceWrites(data[".tmp/system-update-skills.json"]) &&
  data[".tmp/system-update-source-packs.json"].status === "PASS" &&
  data[".tmp/system-update-fact-packs.json"].status === "PASS" &&
  data[".tmp/system-update-peripheral-skills.json"].status === "PASS" &&
  noSourceWrites(data[".tmp/system-update-peripheral-skills.json"]) &&
  data[".tmp/system-update-runtime.json"].status === "PASS" &&
  data[".tmp/system-route.json"].verification_level === "context-injection" &&
  data[".tmp/install-codex.json"].status === "PASS" &&
  data[".tmp/install-claude.json"].status === "PASS";
process.stdout.write(JSON.stringify({
  status: ok ? "PASS" : "FAIL",
  install_dry_run: ["codex", "claude"],
  route_skills: data[".tmp/system-route.json"].skills,
  registry_status: data[".tmp/system-verify.json"].status,
  source_candidate_count: data[".tmp/system-sync.json"].generated_candidate_count,
  source_pack_count: data[".tmp/system-update-source-packs.json"].source_pack_count,
  fact_pack_count: data[".tmp/system-update-fact-packs.json"].fact_pack_count,
  peripheral_skill_count: data[".tmp/system-update-peripheral-skills.json"].generated_skill_count,
  generated_cache_boundary: true,
  update_targets: ["sources", "boards", "skills", "source-packs", "fact-packs", "peripheral-skills", "runtime"],
  highest_verification_level: "V3",
  hardware_verified: false
}, null, 2) + "\n");
process.exit(ok ? 0 : 2);
NODE
