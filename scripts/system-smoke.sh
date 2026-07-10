#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo test -p lilygo-skills-cli hook_envelopes >/tmp/lilygo-hook-test.log 2>&1
cargo run -p lilygo-skills-cli -- verify --json >.tmp/system-verify.json
cargo run -p lilygo-skills-cli -- update fact-packs --dry-run --json >.tmp/system-update-fact-packs.json
cargo run -p lilygo-skills-cli -- route --json "T-Display-S3 Arduino LVGL" >.tmp/system-route.json
node install.js --codex --dry-run >.tmp/install-codex.json
node install.js --claude --dry-run >.tmp/install-claude.json

node <<'NODE'
const fs = require("fs");
const files = [
  ".tmp/system-verify.json",
  ".tmp/system-update-fact-packs.json",
  ".tmp/system-route.json",
  ".tmp/install-codex.json",
  ".tmp/install-claude.json"
];
const data = Object.fromEntries(files.map((file) => [file, JSON.parse(fs.readFileSync(file, "utf8"))]));
const ok =
  data[".tmp/system-verify.json"].status === "PASS" &&
  data[".tmp/system-update-fact-packs.json"].status === "PASS" &&
  data[".tmp/system-route.json"].verification_level === "context-injection" &&
  data[".tmp/install-codex.json"].status === "PASS" &&
  data[".tmp/install-claude.json"].status === "PASS";
process.stdout.write(JSON.stringify({
  status: ok ? "PASS" : "FAIL",
  install_dry_run: ["codex", "claude"],
  route_skills: data[".tmp/system-route.json"].skills,
  registry_status: data[".tmp/system-verify.json"].status,
  fact_pack_count: data[".tmp/system-update-fact-packs.json"].fact_pack_count,
  update_targets: ["board-facts", "fact-packs"],
  highest_verification_level: "V3",
  hardware_verified: false
}, null, 2) + "\n");
process.exit(ok ? 0 : 2);
NODE
