#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

DRY_RUN=false
PORT=""
SOURCE_ROOT="${LILYGO_SOURCE_ROOT:-}"
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
    --source-root)
      SOURCE_ROOT="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" goal plan --json "T-Watch Ultra Arduino BHI260AP IMU demo build flash monitor" \
  >.tmp/goal-hardware-plan.json

if [[ -z "$PORT" ]]; then
  for pattern in /dev/cu.usbmodem* /dev/cu.usbserial* /dev/tty.usbmodem* /dev/tty.usbserial*; do
    for candidate in $pattern; do
      if [[ -e "$candidate" ]]; then
        PORT="$candidate"
        break 2
      fi
    done
  done
fi

if [[ "$DRY_RUN" == "true" ]]; then
  node <<'NODE'
const fs = require("fs");
const plan = JSON.parse(fs.readFileSync(".tmp/goal-hardware-plan.json", "utf8"));
process.stdout.write(JSON.stringify({
  status: "BOUNDARY",
  dry_run: true,
  goal_id: plan.goal_id,
  hardware_verified: false,
  highest_verification_level: "V3",
  boundaries: ["dry-run hardware smoke does not open serial, flash, or reset the board"],
  writes: []
}, null, 2) + "\n");
NODE
  exit 0
fi

if [[ -z "$PORT" ]]; then
  node <<'NODE'
const fs = require("fs");
const plan = JSON.parse(fs.readFileSync(".tmp/goal-hardware-plan.json", "utf8"));
process.stdout.write(JSON.stringify({
  status: "BOUNDARY",
  dry_run: false,
  goal_id: plan.goal_id,
  port_detected: false,
  hardware_verified: false,
  highest_verification_level: "V3",
  boundaries: ["no USB serial port matching ESP32-style USB modem/serial patterns was detected"],
  writes: []
}, null, 2) + "\n");
NODE
  exit 0
fi

if [[ -z "$SOURCE_ROOT" ]]; then
  for candidate in \
    "$ROOT/ref/LilyGoLib" \
    "$ROOT/../LilyGoLib"; do
    if [[ -f "$candidate/examples/sensor/BHI260AP_6DoF/BHI260AP_6DoF.ino" ]]; then
      SOURCE_ROOT="$candidate"
      break
    fi
  done
fi

PROFILE="$(mktemp "${TMPDIR:-/tmp}/lilygo-watch-ultra-profile.XXXXXX.json")"
PROJECT_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-hardware.XXXXXX")"
trap 'rm -f "$PROFILE"; rm -rf "$PROJECT_ROOT"' EXIT

printf '{"board":"board-t-watch-ultra","framework":"arduino","port":"%s","capabilities":["serial","flash"],"verification_level":"V5"}\n' "$PORT" >"$PROFILE"
"$BIN" verify-hardware --profile "$PROFILE" --json >.tmp/goal-hardware-verify.json

BOARD_INFO_EXIT=127
if command -v espflash >/dev/null 2>&1; then
  : >.tmp/goal-hardware-board-info.txt
  for before in no-reset default-reset; do
    set +e
    if command -v perl >/dev/null 2>&1; then
      perl -e 'alarm 20; exec @ARGV' \
        espflash board-info \
        --port "$PORT" \
        --chip esp32s3 \
        --before "$before" \
        --non-interactive \
        --skip-update-check \
        >.tmp/goal-hardware-board-info-attempt.txt \
        2>&1
    else
      espflash board-info \
        --port "$PORT" \
        --chip esp32s3 \
        --before "$before" \
        --non-interactive \
        --skip-update-check \
        >.tmp/goal-hardware-board-info-attempt.txt \
        2>&1
    fi
    BOARD_INFO_EXIT=$?
    set -e
    {
      printf '%s\n' "--- espflash board-info --before $before exit=$BOARD_INFO_EXIT ---"
      cat .tmp/goal-hardware-board-info-attempt.txt
    } >>.tmp/goal-hardware-board-info.txt
    if [[ "$BOARD_INFO_EXIT" -eq 0 ]]; then
      break
    fi
  done
else
  printf 'espflash not found\n' >.tmp/goal-hardware-board-info.txt
fi

GOAL_START_ARGS=(
  goal start
  --plan .tmp/goal-hardware-plan.json \
  --project "$PROJECT_ROOT" \
  --allow-build \
  --allow-flash \
  --allow-serial \
  --port "$PORT" \
  --json
)
if [[ -n "$SOURCE_ROOT" ]]; then
  GOAL_START_ARGS+=(--source-root "$SOURCE_ROOT")
fi

set +e
"$BIN" "${GOAL_START_ARGS[@]}" \
  >.tmp/goal-hardware-start.json \
  2>.tmp/goal-hardware-start.stderr
GOAL_START_EXIT=$?
set -e

BOARD_INFO_EXIT="$BOARD_INFO_EXIT" GOAL_START_EXIT="$GOAL_START_EXIT" node <<'NODE'
const fs = require("fs");
const plan = JSON.parse(fs.readFileSync(".tmp/goal-hardware-plan.json", "utf8"));
const verify = JSON.parse(fs.readFileSync(".tmp/goal-hardware-verify.json", "utf8"));
const boardInfoText = fs.readFileSync(".tmp/goal-hardware-board-info.txt", "utf8");
const goalStart = JSON.parse(fs.readFileSync(".tmp/goal-hardware-start.json", "utf8"));
const boardInfoExit = Number(process.env.BOARD_INFO_EXIT);
const goalStartExit = Number(process.env.GOAL_START_EXIT);
function matchLine(label) {
  const line = boardInfoText.split(/\r?\n/).find((entry) => entry.startsWith(label));
  return line ? line.split(":").slice(1).join(":").trim() : null;
}
const boardInfoPass = boardInfoExit === 0;
const goalStartPass = goalStartExit === 0 && goalStart.status === "PASS";
const status = verify.status === "PASS" && boardInfoPass && goalStartPass ? "PASS" : "BOUNDARY";
process.stdout.write(JSON.stringify({
  status,
  dry_run: false,
  goal_id: plan.goal_id,
  port_detected: true,
  port_redacted: true,
  verify_hardware_status: verify.status,
  espflash_board_info: {
    status: boardInfoPass ? "PASS" : "BOUNDARY",
    exit_code: boardInfoExit,
    chip_type: matchLine("Chip type"),
    crystal_frequency: matchLine("Crystal frequency"),
    flash_size: matchLine("Flash size"),
    features: matchLine("Features"),
    mac_address: boardInfoPass ? "redacted" : null
  },
  goal_start: {
    status: goalStart.status,
    exit_code: goalStartExit,
    highest_verification_level: goalStart.highest_verification_level || null,
    hardware_verified: Boolean(goalStart.hardware_verified),
    failure_class: goalStart.failure_class || null,
    ran_command_statuses: goalStart.ran_commands.map((command) => ({
      recipe_id: command.recipe_id,
      step_id: command.step_id,
      status: command.status
    })),
    writes_redacted: goalStart.writes.length
  },
  highest_verification_level: goalStartPass
    ? goalStart.highest_verification_level
    : (boardInfoPass ? "V5-board-identification" : "V3"),
  hardware_verified: Boolean(boardInfoPass && goalStart.hardware_verified),
  boundaries: goalStartPass
    ? ["attached board build, upload, and bounded serial observation completed via goal start"]
    : (boardInfoPass
      ? ["attached board was identified; goal build/upload/serial did not complete"]
      : ["attached-board info was not collected"]),
  writes: []
}, null, 2) + "\n");
process.exit(status === "PASS" ? 0 : 2);
NODE
