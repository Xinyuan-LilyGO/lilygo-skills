#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "lvgl-smoke requires --dry-run unless a simulator or hardware profile flow is added" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo run -p lilygo-skills-cli -- route --json "T-Display-S3 ESP-IDF LVGL screen is blank" >.tmp/lvgl-route.json
cargo run -p lilygo-skills-cli -- verify --json >.tmp/lvgl-verify.json

node <<'NODE'
const fs = require("fs");
const route = JSON.parse(fs.readFileSync(".tmp/lvgl-route.json", "utf8"));
const verify = JSON.parse(fs.readFileSync(".tmp/lvgl-verify.json", "utf8"));
const ok = verify.status === "PASS" && route.skills.includes("fw-lvgl") && route.skills.includes("debug-lvgl-loop");
process.stdout.write(JSON.stringify({
  status: ok ? "BOUNDARY" : "FAIL",
  dry_run: true,
  verification_level: "V1",
  checked: ["route", "registry", "LVGL debug loop", "display context"],
  boundaries: ["simulator boundary: no LVGL simulator/page-data artifact configured"],
  writes: []
}, null, 2) + "\n");
process.exit(ok ? 0 : 2);
NODE
