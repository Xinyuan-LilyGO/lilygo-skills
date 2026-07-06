#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

DRY_RUN=true
PORT=""
SIMULATE=""
ALLOW_BUILD=false
ALLOW_FLASH=false
ALLOW_SERIAL=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    --port)
      PORT="${2:-}"
      shift 2
      ;;
    --allow-build)
      ALLOW_BUILD=true
      DRY_RUN=false
      shift
      ;;
    --allow-flash)
      ALLOW_FLASH=true
      DRY_RUN=false
      shift
      ;;
    --allow-serial)
      ALLOW_SERIAL=true
      DRY_RUN=false
      shift
      ;;
    --simulate-no-device|--simulate-wrong-port|--simulate-flash-timeout)
      SIMULATE="${1#--simulate-}"
      DRY_RUN=true
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"
PROMPT="T-Watch Ultra Arduino BHI260AP IMU demo build flash monitor"
RUN_ID="$$"
PLAN_FILE=".tmp/hardware-gold-live-plan-$RUN_ID.json"
REPORT_FILE=".tmp/hardware-gold-live-report-$RUN_ID.json"

"$BIN" goal plan --json "$PROMPT" >"$PLAN_FILE"

sha_file() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    sha256sum "$1" | awk '{print $1}'
  fi
}

write_report() {
  local status="$1"
  local mode="$2"
  local detail="$3"
  local exit_code="$4"
  PLAN_FILE="$PLAN_FILE" node - "$status" "$mode" "$detail" <<'NODE' >"$REPORT_FILE"
const fs = require("fs");
const [status, mode, detail] = process.argv.slice(2);
const plan = JSON.parse(fs.readFileSync(process.env.PLAN_FILE, "utf8"));
process.stdout.write(JSON.stringify({
  status,
  mode,
  dry_run: mode !== "permissioned-live",
  goal_id: plan.goal_id,
  prompt: plan.prompt,
  hardware_verified: false,
  highest_verification_level: "V3",
  evidence: {
    public_redacted: true,
    raw_logs_public: false,
    wifi_or_token_public: false,
    private_device_ids_public: false
  },
  failure_modes: mode === "dry-run" ? ["no-device", "wrong-port", "flash-timeout"] : [mode],
  detail,
  writes: []
}, null, 2) + "\n");
NODE
  local hash
  hash="$(sha_file "$REPORT_FILE")"
  REPORT_FILE="$REPORT_FILE" node - "$hash" <<'NODE'
const fs = require("fs");
const hash = process.argv[2];
const path = process.env.REPORT_FILE;
const report = JSON.parse(fs.readFileSync(path, "utf8"));
report.artifacts = [{ kind: "public-report", path, sha256: hash }];
process.stdout.write(JSON.stringify(report, null, 2) + "\n");
NODE
  exit "$exit_code"
}

case "$SIMULATE" in
  no-device)
    write_report "BOUNDARY" "no-device" "no USB serial device was available for live validation" 2
    ;;
  wrong-port)
    write_report "BOUNDARY" "wrong-port" "the selected serial port did not identify as a usable ESP32 target" 2
    ;;
  flash-timeout)
    write_report "BOUNDARY" "flash-timeout" "flash command timed out before evidence could reach V4/V5" 2
    ;;
esac

if [[ "$DRY_RUN" == "true" ]]; then
  write_report "PASS" "dry-run" "live harness plan is valid and does not touch hardware by default" 0
fi

if [[ "$ALLOW_BUILD" != "true" || "$ALLOW_FLASH" != "true" || "$ALLOW_SERIAL" != "true" ]]; then
  write_report "BOUNDARY" "permission-missing" "live hardware path requires --allow-build --allow-flash --allow-serial" 2
fi

if [[ -z "$PORT" ]]; then
  write_report "BOUNDARY" "no-device" "no --port was supplied for permissioned live validation" 2
fi

bash scripts/goal-hardware-smoke.sh --port "$PORT"
