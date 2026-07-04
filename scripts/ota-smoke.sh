#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "ota-smoke requires --dry-run unless a hardware profile flow is added" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo run -p lilygo-skills-cli -- route --json "T-Watch ESP-IDF OTA update fails after reboot" >.tmp/ota-route.json
cargo run -p lilygo-skills-cli -- verify --json >.tmp/ota-verify.json

node <<'NODE'
const fs = require("fs");
const route = JSON.parse(fs.readFileSync(".tmp/ota-route.json", "utf8"));
const verify = JSON.parse(fs.readFileSync(".tmp/ota-verify.json", "utf8"));
const ok = verify.status === "PASS" && route.skills.includes("app-ota") && route.skills.includes("debug-flash-serial");
process.stdout.write(JSON.stringify({
  status: ok ? "BOUNDARY" : "FAIL",
  dry_run: true,
  verification_level: "V1",
  checked: ["route", "registry", "partition vocabulary", "manifest vocabulary", "rollback boundary"],
  boundaries: ["hardware-verification boundary: OTA flash/update evidence was not run"],
  writes: []
}, null, 2) + "\n");
process.exit(ok ? 0 : 2);
NODE
