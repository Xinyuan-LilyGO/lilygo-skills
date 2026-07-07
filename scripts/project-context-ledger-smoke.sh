#!/usr/bin/env bash
set -euo pipefail

if [[ $# -gt 0 && "${1:-}" != "--dry-run" ]]; then
  echo "unknown argument: $1" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"
PROJECT_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-project-ledger.XXXXXX")"
export PROJECT_ROOT
trap 'rm -rf "$PROJECT_ROOT"' EXIT
mkdir -p "$PROJECT_ROOT/firmware/src"

"$BIN" project init \
  --project "$PROJECT_ROOT" \
  --board board-t-watch-ultra \
  --framework fw-arduino \
  --json >.tmp/project-ledger-init.json

"$BIN" route \
  --project "$PROJECT_ROOT/firmware/src" \
  --json "T-Watch Ultra IMU debug" >.tmp/project-ledger-source-route.json

node <<'NODE'
const crypto = require("crypto");
const fs = require("fs");
const route = JSON.parse(fs.readFileSync(".tmp/project-ledger-source-route.json", "utf8"));
const readiness = route.readiness || [];
const material = readiness.length
  ? readiness.map((signal) => `${signal.board_id}:${signal.topic}:${signal.completeness}:${signal.evidence_level}`).join("|")
  : (route.skills || []).join("|");
const sourceSignature = crypto.createHash("sha256").update(JSON.stringify(material)).digest("hex");
fs.writeFileSync(".tmp/project-ledger-record.json", JSON.stringify({
  kind: "capability",
  board_id: "board-t-watch-ultra",
  framework: "fw-arduino",
  capability: "imu.bhi260ap",
  verification_level: "V5",
  summary: "imu.bhi260ap previously reached V5 build/upload/serial evidence on a redacted public report.",
  source_signature: sourceSignature,
  public_evidence_hash: "sha256:evidence",
  expand_commands: [
    "lilygo-skills source query --board board-t-watch-ultra --topic imu --json",
    "lilygo-skills goal evidence --id <goal-id> --json"
  ]
}, null, 2) + "\n");
NODE

"$BIN" project ledger record \
  --project "$PROJECT_ROOT/firmware/src" \
  --input .tmp/project-ledger-record.json \
  --json >.tmp/project-ledger-record-out.json
"$BIN" project ledger show \
  --project "$PROJECT_ROOT/firmware/src" \
  --json >.tmp/project-ledger-show.json
"$BIN" route \
  --project "$PROJECT_ROOT/firmware/src" \
  --json "T-Watch Ultra IMU debug" >.tmp/project-ledger-route.json

printf '{"prompt":"T-Watch Ultra IMU debug"}' \
  | (cd "$PROJECT_ROOT/firmware/src" && "$BIN" hook claude) \
  >.tmp/project-ledger-hook-full.json
printf '{"prompt":"T-Watch Ultra IMU debug"}' \
  | (cd "$PROJECT_ROOT/firmware/src" && "$BIN" hook claude) \
  >.tmp/project-ledger-hook-compact.json
"$BIN" route \
  --project "$PROJECT_ROOT/firmware/src" \
  --json "re-verify T-Watch Ultra IMU debug" >.tmp/project-ledger-redo-route.json

cat >.tmp/project-ledger-private-record.json <<'JSON'
{
  "kind": "capability",
  "board_id": "board-t-watch-ultra",
  "framework": "fw-arduino",
  "capability": "imu.bhi260ap",
  "verification_level": "V5",
  "summary": "bad private port /dev/cu.usbmodem-private",
  "source_signature": "sha256:source",
  "public_evidence_hash": "sha256:evidence",
  "expand_commands": ["lilygo-skills source query --board board-t-watch-ultra --topic imu --json"]
}
JSON

set +e
"$BIN" project ledger record \
  --project "$PROJECT_ROOT/firmware/src" \
  --input .tmp/project-ledger-private-record.json \
  --json >.tmp/project-ledger-private-record-out.json 2>.tmp/project-ledger-private-record-err.txt
PRIVATE_CODE=$?
export PRIVATE_CODE
set -e

node <<'NODE'
const fs = require("fs");
function read(path) {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}
function bytes(value) {
  return Buffer.byteLength(typeof value === "string" ? value : JSON.stringify(value));
}
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(typeof detail === "string" ? detail : JSON.stringify(detail, null, 2));
    process.exit(1);
  }
}
const show = read(".tmp/project-ledger-show.json");
const route = read(".tmp/project-ledger-route.json");
const redo = read(".tmp/project-ledger-redo-route.json");
const full = read(".tmp/project-ledger-hook-full.json").hookSpecificOutput?.additionalContext || "";
const compact = read(".tmp/project-ledger-hook-compact.json").hookSpecificOutput?.additionalContext || "";
const privateErr = fs.readFileSync(".tmp/project-ledger-private-record-err.txt", "utf8");

check("ledger show has one capability", show.capabilities.length === 1, show);
check("ledger show exposes freshness", Object.prototype.hasOwnProperty.call(show.capabilities[0], "verified_at") && Object.prototype.hasOwnProperty.call(show.capabilities[0], "stale"), show);
check("route carries ledger hit", route.project_ledger && route.project_ledger.mode === "hit", route.project_ledger);
check("route hit is relevant", route.project_ledger.entries.length === 1 && route.project_ledger.entries[0].capability === "imu.bhi260ap", route.project_ledger);
check("hook writes full context first", full.includes("project_ledger") && full.includes("previously_verified"), full);
check("second hook uses project ledger compact context", compact.includes("LilyGO project ledger"), compact);
check("project ledger compact keeps boundary", compact.includes("evidence_boundary=V3/hardware_verified=false"), compact);
check("project compact is much shorter", bytes(compact) * 3 < bytes(full), { full: bytes(full), compact: bytes(compact), fullText: full, compactText: compact });
check("explicit redo bypasses short circuit", redo.project_ledger && redo.project_ledger.mode === "bypass", redo.project_ledger);
check("private ledger record failed", Number(process.env.PRIVATE_CODE) !== 0 && privateErr.includes("private pattern"), privateErr);

const publicText = fs.readFileSync(process.env.PROJECT_ROOT + "/.lilygo-skills/ledger.json", "utf8") +
  fs.readFileSync(process.env.PROJECT_ROOT + "/.lilygo-skills/context-digest.json", "utf8");
check("public ledger has no private port", !/\/dev\/(?:cu|tty)/.test(publicText), publicText);
check("public ledger has no private network", !/192\.168\.|wifi_password|access_token|Bearer /.test(publicText), publicText);

process.stdout.write(JSON.stringify({
  status: "PASS",
  mode: route.project_ledger.mode,
  compact_bytes: bytes(compact),
  full_bytes: bytes(full),
  private_record_rejected: true
}, null, 2) + "\n");
NODE
