#!/usr/bin/env bash
set -euo pipefail

# Generated runtime skills may be materialized only into a generated
# cache/install/project output, never back into the committed source skills/
# tree or source index/routes.json.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

node <<'NODE' >.tmp/generated-cache-source-before.json
const fs = require("fs");
const crypto = require("crypto");
const cp = require("child_process");
const files = cp.execFileSync("git", ["ls-files", "skills/**/SKILL.md", "index/routes.json"], {
  encoding: "utf8"
}).trim().split(/\n/).filter(Boolean);
files.push(...cp.execFileSync("git", ["ls-files", "skills/references/**", "templates/skills/**"], {
  encoding: "utf8"
}).trim().split(/\n/).filter(Boolean));
const hashes = Object.fromEntries(files.map((file) => [
  file,
  crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex")
]));
process.stdout.write(JSON.stringify(hashes, null, 2) + "\n");
NODE

"$BIN" update skills --dry-run --json >.tmp/generated-cache-update-skills-dry.json
"$BIN" update peripheral-skills --dry-run --json >.tmp/generated-cache-update-peripheral-dry.json

OUT_SKILLS="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-update-skills.XXXXXX")"
OUT_PERIPH="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-update-periph.XXXXXX")"
PROJECT_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-project-init-cache.XXXXXX")"

"$BIN" update skills --out "$OUT_SKILLS" --json >.tmp/generated-cache-update-skills-apply.json
"$BIN" verify --generated-root "$OUT_SKILLS" --json >.tmp/generated-cache-update-skills-verify.json
"$BIN" update peripheral-skills --out "$OUT_PERIPH" --json \
  >.tmp/generated-cache-update-peripheral-apply.json
"$BIN" verify --generated-root "$OUT_PERIPH" --json \
  >.tmp/generated-cache-update-peripheral-verify.json
"$BIN" project init --project "$PROJECT_ROOT" --board board-t-display-s3 --framework fw-arduino --json \
  >.tmp/generated-cache-project-init.json
"$BIN" verify --generated-root "$PROJECT_ROOT/.lilygo-skills/generated-skills" --json \
  >.tmp/generated-cache-project-verify.json

set +e
"$BIN" generate skills --out "$ROOT" --json >.tmp/generated-cache-out-root.json 2>.tmp/generated-cache-out-root.err
OUT_ROOT_STATUS=$?
"$BIN" generate skills --out "$ROOT/skills/.." --json \
  >.tmp/generated-cache-out-normalized-root.json 2>.tmp/generated-cache-out-normalized-root.err
OUT_NORMALIZED_STATUS=$?
"$BIN" generate skills --out "$ROOT/templates" --json \
  >.tmp/generated-cache-out-templates.json 2>.tmp/generated-cache-out-templates.err
OUT_TEMPLATES_STATUS=$?
set -e
export OUT_ROOT_STATUS OUT_NORMALIZED_STATUS OUT_TEMPLATES_STATUS

node <<'NODE'
const fs = require("fs");
const crypto = require("crypto");

function read(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(JSON.stringify(detail, null, 2));
    process.exit(1);
  }
}
function sourceHashesUnchanged() {
  const before = read(".tmp/generated-cache-source-before.json");
  return Object.entries(before).every(([file, hash]) => {
    if (!fs.existsSync(file)) return false;
    const now = crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
    return now === hash;
  });
}
function noSourceWrites(report) {
  const forbidden = /^(skills\/|templates\/|index\/routes\.json$)/;
  return [...(report.planned_writes || []), ...(report.writes || [])]
    .every((item) => !forbidden.test(item));
}
function writesSupportFiles(report) {
  const writes = [...(report.planned_writes || []), ...(report.writes || [])];
  return writes.some((item) => item.endsWith("/skills/references") || item.includes("/skills/references/")) &&
    writes.some((item) => item.endsWith("/templates/skills") || item.includes("/templates/skills/"));
}

const skillsDry = read(".tmp/generated-cache-update-skills-dry.json");
const periphDry = read(".tmp/generated-cache-update-peripheral-dry.json");
const skillsApply = read(".tmp/generated-cache-update-skills-apply.json");
const periphApply = read(".tmp/generated-cache-update-peripheral-apply.json");
const skillsVerify = read(".tmp/generated-cache-update-skills-verify.json");
const periphVerify = read(".tmp/generated-cache-update-peripheral-verify.json");
const projectInit = read(".tmp/generated-cache-project-init.json");
const projectVerify = read(".tmp/generated-cache-project-verify.json");

check("update skills dry-run avoids source writes",
  skillsDry.status === "PASS" &&
  skillsDry.dry_run === true &&
  noSourceWrites(skillsDry) &&
  writesSupportFiles(skillsDry) &&
  skillsDry.planned_writes.every((write) => write.startsWith(".lilygo-skills/generated-skills/")),
  skillsDry);
check("update peripheral-skills dry-run avoids source writes",
  periphDry.status === "PASS" &&
  periphDry.dry_run === true &&
  noSourceWrites(periphDry) &&
  writesSupportFiles(periphDry),
  periphDry);
check("update skills apply verifies generated root",
  skillsApply.status === "PASS" &&
  noSourceWrites(skillsApply) &&
  skillsVerify.status === "PASS" &&
  skillsVerify.missing.length === 0 &&
  skillsVerify.extra.length === 0,
  { skillsApply, skillsVerify });
check("update peripheral-skills apply verifies generated root",
  periphApply.status === "PASS" &&
  noSourceWrites(periphApply) &&
  periphVerify.status === "PASS" &&
  periphVerify.missing.length === 0 &&
  periphVerify.extra.length === 0,
  { periphApply, periphVerify });
check("project init generated cache verifies",
  projectInit.status === "PASS" &&
  projectInit.generated_cache.verify_status === "PASS" &&
  projectVerify.status === "PASS" &&
  projectVerify.missing.length === 0 &&
  projectVerify.extra.length === 0,
  { projectInit, projectVerify });
check("source tracked skills/index unchanged", sourceHashesUnchanged(), {});
check("source-root generation rejected",
  Number(process.env.OUT_ROOT_STATUS) !== 0 &&
  Number(process.env.OUT_NORMALIZED_STATUS) !== 0 &&
  Number(process.env.OUT_TEMPLATES_STATUS) !== 0,
  {
    root: process.env.OUT_ROOT_STATUS,
    normalized: process.env.OUT_NORMALIZED_STATUS,
    templates: process.env.OUT_TEMPLATES_STATUS
  });
check("generated cache ignored",
  fs.readFileSync(".gitignore", "utf8").includes(".lilygo-skills/generated-skills/"),
  fs.readFileSync(".gitignore", "utf8"));

process.stdout.write(JSON.stringify({
  status: "PASS",
  update_skills_cache: skillsVerify.present,
  update_peripheral_cache: periphVerify.present,
  project_cache: projectVerify.present,
  source_tree_writes: false,
  source_root_generation_rejected: true,
  source_templates_generation_rejected: true,
  generated_extra_skills: skillsVerify.extra_count,
  highest_verification_level: "V3",
  hardware_verified: false
}, null, 2) + "\n");
NODE
