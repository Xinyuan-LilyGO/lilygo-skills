#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "goal-privacy-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp
PROJECT_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-privacy-project.XXXXXX")"
SOURCE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-goal-privacy-source.XXXXXX")"
export PROJECT_ROOT SOURCE_ROOT
trap 'rm -rf "$PROJECT_ROOT" "$SOURCE_ROOT"' EXIT

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

mkdir -p "$PROJECT_ROOT/.lilygo-skills"
mkdir -p "$PROJECT_ROOT/firmware/src"
cat >"$PROJECT_ROOT/.lilygo-skills/project.json" <<'JSON'
{"schema_version":1,"board":"board-t-watch-ultra","framework":"fw-arduino","features":[],"notes":"privacy smoke public project anchor"}
JSON
cat >"$PROJECT_ROOT/.lilygo-skills/local.json" <<'JSON'
{"wireless_name":"ExamplePrivateNetwork","wireless_key":"ExamplePrivateKey","ota_target":"SyntheticLocalTarget","serial_device":"/dev/cu.usbmodem-private"}
JSON

"$BIN" goal plan --json "T-Watch Ultra serial boot log unreadable" \
  >.tmp/goal-privacy-plan.json
"$BIN" goal plan --project "$PROJECT_ROOT" --json "T-Watch Ultra Arduino OTA over WiFi then serial monitor" \
  >.tmp/goal-privacy-private-local-plan.json
"$BIN" goal plan --json "T-Watch Ultra Arduino IMU 抬腕检测怎么做" \
  >.tmp/goal-privacy-start-plan.json
"$BIN" goal start \
  --plan .tmp/goal-privacy-start-plan.json \
  --project "$PROJECT_ROOT" \
  --source-root "$SOURCE_ROOT" \
  --port /dev/cu.lilygo-private-test \
  --json \
  >.tmp/goal-privacy-start.json
"$BIN" goal complete \
  --dry-run \
  --allow-build \
  --allow-flash \
  --allow-serial \
  --project "$PROJECT_ROOT/firmware/src" \
  --source-root "$SOURCE_ROOT" \
  --port /dev/cu.lilygo-private-complete \
  --json "T-Watch Ultra Arduino OTA over WiFi then serial monitor" \
  >.tmp/goal-privacy-complete.json
cat >.tmp/goal-privacy-ledger-record.json <<'JSON'
{
  "kind": "capability",
  "board_id": "board-t-watch-ultra",
  "framework": "fw-arduino",
  "capability": "imu.bhi260ap",
  "verification_level": "V5",
  "summary": "imu.bhi260ap previously reached V5 evidence in a redacted public report.",
  "source_signature": "sha256:source",
  "public_evidence_hash": "sha256:evidence",
  "expand_commands": ["lilygo-skills source query --board board-t-watch-ultra --topic imu --json"]
}
JSON
"$BIN" project ledger record \
  --project "$PROJECT_ROOT" \
  --input .tmp/goal-privacy-ledger-record.json \
  --json >.tmp/goal-privacy-ledger-record-out.json
cat >.tmp/goal-privacy-ledger-private-record.json <<'JSON'
{
  "kind": "capability",
  "board_id": "board-t-watch-ultra",
  "framework": "fw-arduino",
  "capability": "imu.bhi260ap",
  "verification_level": "V5",
  "summary": "private serial /dev/cu.usbmodem-private",
  "source_signature": "sha256:source",
  "public_evidence_hash": "sha256:evidence",
  "expand_commands": ["lilygo-skills source query --board board-t-watch-ultra --topic imu --json"]
}
JSON
set +e
"$BIN" project ledger record \
  --project "$PROJECT_ROOT" \
  --input .tmp/goal-privacy-ledger-private-record.json \
  --json >.tmp/goal-privacy-ledger-private-record-out.json 2>.tmp/goal-privacy-ledger-private-record-err.txt
LEDGER_PRIVATE_CODE=$?
export LEDGER_PRIVATE_CODE
set -e
node install.js --all --dry-run >.tmp/goal-privacy-install.json

TRACKED_PRIVATE="$(git ls-files | grep -E '(^|/)\.lilygo-skills/(local\.json|evidence/)' || true)"
CHECK_IGNORE_LOCAL=0
CHECK_IGNORE_EVIDENCE=0
git check-ignore -q .lilygo-skills/local.json || CHECK_IGNORE_LOCAL=$?
git check-ignore -q .lilygo-skills/evidence/sample.json || CHECK_IGNORE_EVIDENCE=$?

node <<'NODE'
const fs = require("fs");
const planText = fs.readFileSync(".tmp/goal-privacy-plan.json", "utf8");
const plan = JSON.parse(planText);
const privatePlanText = fs.readFileSync(".tmp/goal-privacy-private-local-plan.json", "utf8");
const privatePlan = JSON.parse(privatePlanText);
const startText = fs.readFileSync(".tmp/goal-privacy-start.json", "utf8");
const start = JSON.parse(startText);
const completeText = fs.readFileSync(".tmp/goal-privacy-complete.json", "utf8");
const complete = JSON.parse(completeText);
const ledgerText = fs.readFileSync(process.env.PROJECT_ROOT + "/.lilygo-skills/ledger.json", "utf8");
const ledgerPrivateErr = fs.readFileSync(".tmp/goal-privacy-ledger-private-record-err.txt", "utf8");
const installText = fs.readFileSync(".tmp/goal-privacy-install.json", "utf8");
const recipePacks = JSON.parse(fs.readFileSync("data/recipes/recipes.json", "utf8"));
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(detail);
    process.exit(1);
  }
}
check("plan has local evidence boundary", plan.privacy.local_state.includes(".lilygo-skills/evidence/"), JSON.stringify(plan.privacy));
check("plan has no concrete local port", !/\/dev\/(cu|tty)/.test(planText), planText);
check("plan has no LAN host", !/192\.168\./.test(planText), planText);
check("plan has no credential field", !/(wifi_ssid|wifi_password|access_token)/.test(planText), planText);
check("plan has no absolute private user path", !/\/(Users|home)\/[^\/"]+\//.test(planText), planText);
check("private local presence is injected", (privatePlan.context_capsule.facts || []).some((fact) => fact.key === "private.local_state" && fact.value.includes("present")), privatePlanText);
check("private local plan hides wireless name", !privatePlanText.includes("ExamplePrivateNetwork"), privatePlanText);
check("private local plan hides wireless key", !privatePlanText.includes("ExamplePrivateKey"), privatePlanText);
check("private local plan hides ota target", !privatePlanText.includes("SyntheticLocalTarget"), privatePlanText);
check("private local plan hides serial port", !privatePlanText.includes("/dev/cu.usbmodem-private"), privatePlanText);
check("start has no private project path", !startText.includes(process.env.PROJECT_ROOT), startText);
check("start has no private source path", !startText.includes(process.env.SOURCE_ROOT), startText);
check("start has no private port", !startText.includes("/dev/cu.lilygo-private-test"), startText);
check("start exposes required permissions", Array.isArray(start.required_permissions) && start.required_permissions.includes("allow-flash:port"), startText);
check("start exposes planned artifacts", Array.isArray(start.planned_artifacts) && start.planned_artifacts.length > 0, startText);
check("start command display is redacted", start.planned_commands.some((command) => command.command.includes("<redacted-source-root>") || command.command.includes("<redacted-port>")), startText);
check("complete public output is redacted", complete.privacy.public_output_redacted === true, completeText);
check("complete detected private state without values", complete.privacy.private_state_used === true, completeText);
check("complete has no private project path", !completeText.includes(process.env.PROJECT_ROOT), completeText);
check("complete has no private source path", !completeText.includes(process.env.SOURCE_ROOT), completeText);
check("complete has no private port", !completeText.includes("/dev/cu.lilygo-private-complete"), completeText);
check("complete hides wireless name", !completeText.includes("ExamplePrivateNetwork"), completeText);
check("complete hides wireless key", !completeText.includes("ExamplePrivateKey"), completeText);
check("complete hides ota target", !completeText.includes("SyntheticLocalTarget"), completeText);
check("ledger hides private path", !ledgerText.includes(process.env.PROJECT_ROOT) && !/\/dev\/(?:cu|tty)/.test(ledgerText), ledgerText);
check("ledger hides credentials", !/wifi_password|access_token|Bearer |ExamplePrivate/.test(ledgerText), ledgerText);
check("ledger rejects private record", Number(process.env.LEDGER_PRIVATE_CODE) !== 0 && ledgerPrivateErr.includes("private pattern"), ledgerPrivateErr);
check("install dry-run has no absolute user path", !/\/(Users|home)\/[A-Za-z0-9._-]+\//.test(installText), installText);
check("recipe packs have portable refs", recipePacks.source_packs.every((pack) => pack.source_refs.every((ref) => ref.startsWith("https://"))), JSON.stringify(recipePacks, null, 2));
check("recipe packs hash local refs", recipePacks.source_packs.every((pack) => pack.source_refs.every((ref) => ref.startsWith("https://") || ref.startsWith("http://") || /^sha256:[0-9a-f]{64}$/.test(pack.source_hashes?.[ref] || ""))), JSON.stringify(recipePacks, null, 2));
check("recipe official refs are https where present", recipePacks.source_packs.every((pack) => (pack.official_refs || []).every((ref) => ref.startsWith("https://"))), JSON.stringify(recipePacks, null, 2));
check("boundary remains V3", plan.context_capsule.boundary.verification_level === "V3" && plan.context_capsule.boundary.hardware_verified === false, JSON.stringify(plan.context_capsule.boundary));
NODE

if [[ -n "$TRACKED_PRIVATE" ]]; then
  echo "tracked private LilyGO local state found:" >&2
  echo "$TRACKED_PRIVATE" >&2
  exit 1
fi

node <<'NODE'
const fs = require("fs");
const { execFileSync } = require("child_process");
const roots = [
  "README.md",
  "README.zh-CN.md",
  "ARCHITECTURE.md",
  "ARCHITECTURE.zh-CN.md",
  "AGENTS.md",
  "CLAUDE.md",
  "docs",
  "skills/lilygo-router/SKILL.md",
  "skills/references",
  "templates/skills",
  "data"
];
function gitFiles(args) {
  return execFileSync("git", args, { encoding: "utf8" })
    .split(/\r?\n/)
    .filter(Boolean);
}
const tracked = gitFiles(["ls-files", "--", ...roots]);
const unignoredNew = gitFiles(["ls-files", "--others", "--exclude-standard", "--", ...roots]);
const files = Array.from(new Set([...tracked, ...unignoredNew].filter((file) => {
  return fs.existsSync(file) && /\.(json|md|ndjson|html|txt)$/.test(file);
})));
const patterns = [
  { name: "absolute_user_path", regex: /\/Users\/[A-Za-z0-9._-]+(?:\/[A-Za-z0-9._~ -]+)+/ },
  { name: "serial_port", regex: /\/dev\/(?:cu|tty)\.[A-Za-z0-9._-]+/ },
  { name: "private_ipv4", regex: /\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|169\.254\.\d{1,3}\.\d{1,3})\b/ },
  { name: "mdns_host", regex: /\b[a-z0-9][a-z0-9-]*\.local\b/i },
  { name: "mac_address", regex: /(?:^|[^0-9a-f])(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}(?:$|[^0-9a-f])/i },
  { name: "usb_vid_pid", regex: /\b(?:VID:PID|USB ID|vid[:=]|pid[:=]).{0,40}\b[0-9a-f]{4}:[0-9a-f]{4}\b/i },
  { name: "credential_assignment", regex: /\b(?:access_token|auth_token|wifi_ssid|wifi_password|password|passwd|private_key|secret)\s*[:=]/i },
  { name: "bearer_token", regex: /\bBearer\s+[A-Za-z0-9._~+/=-]{8,}/i }
];
const leaks = [];
for (const file of files) {
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
  lines.forEach((line, index) => {
    if (line.includes("synthetic private port")) return;
    for (const pattern of patterns) {
      if (pattern.regex.test(line)) {
        leaks.push(`${file}:${index + 1}:${pattern.name}:${line.slice(0, 220)}`);
      }
    }
  });
}
if (leaks.length > 0) {
  console.error("committed docs contain private target patterns:");
  console.error(leaks.join("\n"));
  process.exit(1);
}
NODE

if [[ "$CHECK_IGNORE_LOCAL" -ne 0 || "$CHECK_IGNORE_EVIDENCE" -ne 0 ]]; then
  echo ".lilygo-skills private paths are not ignored by default" >&2
  exit 1
fi

node <<'NODE'
const fs = require("fs");
const plan = JSON.parse(fs.readFileSync(".tmp/goal-privacy-plan.json", "utf8"));
process.stdout.write(JSON.stringify({
  status: "PASS",
  dry_run: true,
  goal_id: plan.goal_id,
  tracked_private_state: false,
  public_private_patterns: false,
  workflow_private_patterns: false,
  gitignore_private_paths: true,
  local_state: plan.privacy.local_state,
  highest_verification_level: plan.context_capsule.boundary.verification_level,
  hardware_verified: plan.context_capsule.boundary.hardware_verified
}, null, 2) + "\n");
NODE
