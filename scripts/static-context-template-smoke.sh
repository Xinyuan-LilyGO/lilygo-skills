#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

fail() {
  echo "static-context-template-smoke FAIL: $1" >&2
  exit 1
}

for file in \
  skills/references/context-injection.md \
  skills/references/source-discovery.md \
  skills/references/build-flash-serial.md \
  skills/references/lvgl-context.md \
  skills/references/ota-context.md \
  skills/references/bsp-driver-context.md \
  skills/references/radio-gnss-context.md \
  skills/references/project-preferences-references.md \
  skills/references/generation-contract.md \
  templates/skills/board.md \
  templates/skills/peripheral.md \
  templates/skills/playbook.md \
  templates/skills/reference.md \
  templates/skills/framework.md
do
  [[ -f "$file" ]] || fail "missing $file"
done

INSTRUCTIONAL_WORDING_PATTERN="$(printf '%s' 'teach[[:space:]]+AI|')$(printf '\346\225\231[[:space:]]*AI|\346\225\231\344\274\232|\346\225\231\345\255\246')"
WORDING_SURFACES=(
  README.md
  README.zh-CN.md
  ARCHITECTURE.md
  ARCHITECTURE.zh-CN.md
  skills
  docs
)
EXISTING_WORDING_SURFACES=()
for surface in "${WORDING_SURFACES[@]}"; do
  [[ -e "$surface" ]] && EXISTING_WORDING_SURFACES+=("$surface")
done
BAD_WORDING="$(grep -R -n -E "$INSTRUCTIONAL_WORDING_PATTERN" "${EXISTING_WORDING_SURFACES[@]}" || true)"
[[ -z "$BAD_WORDING" ]] || { echo "$BAD_WORDING" >&2; fail "instructional-agent wording present"; }

if [[ "${1:-}" == "--wording-only" ]]; then
  echo '{"status":"PASS","wording":"context-injection"}'
  exit 0
fi

GENERATED_SKILLS="$(git ls-files 'skills/**/SKILL.md' \
  | grep -Ev '^skills/lilygo-router/SKILL.md$' || true)"
[[ -z "$GENERATED_SKILLS" ]] || { echo "$GENERATED_SKILLS" >&2; fail "generated source Skill snapshot tracked"; }

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"
OUT="$ROOT/.tmp/static-context-generated"
rm -rf "$OUT"
"$BIN" generate skills --out "$OUT" --json >.tmp/static-context-generate.json
"$BIN" verify --generated-root "$OUT" --json >.tmp/static-context-verify.json
node install.js --all --dry-run >.tmp/static-context-install-dry.json

grep -n -F "Generation Contract: templates/skills/board.md" "$OUT/skills/board-t-watch-ultra/SKILL.md" >/dev/null
grep -n -F "Generation Contract: templates/skills/peripheral.md" "$OUT/skills/periph-imu/SKILL.md" >/dev/null
grep -n -F "Generation Contract: templates/skills/playbook.md" "$OUT/skills/playbook-lvgl-debug/SKILL.md" >/dev/null
! grep -n -F "{{" "$OUT/skills/board-t-watch-ultra/SKILL.md" "$OUT/skills/periph-imu/SKILL.md" "$OUT/skills/playbook-lvgl-debug/SKILL.md" >/dev/null

node <<'NODE'
const fs = require("fs");
function read(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}: ${JSON.stringify(detail, null, 2)}`);
    process.exit(1);
  }
}
const gen = read(".tmp/static-context-generate.json");
const ver = read(".tmp/static-context-verify.json");
const install = read(".tmp/static-context-install-dry.json");
check("generate pass", gen.status === "PASS", gen);
check("verify pass", ver.status === "PASS", ver);
check("reference support copied", fs.existsSync(".tmp/static-context-generated/skills/references/generation-contract.md"), {});
check("template support copied", fs.existsSync(".tmp/static-context-generated/templates/skills/playbook.md"), {});
check("references are not skills", !fs.existsSync(".tmp/static-context-generated/skills/references/SKILL.md"), {});
const writes = install.planned_writes || [];
check("install dry-run pass", install.status === "PASS", install);
check("install plans skill references", writes.some((w) => w.includes("skills/references")), writes);
check("install plans templates", writes.some((w) => w.includes("templates/skills")), writes);
check("install plans public references", writes.some((w) => w.includes("public-skill/references")), writes);
process.stdout.write(JSON.stringify({
  status: "PASS",
  generated_skill_count: gen.skill_count,
  generated_root_verified: ver.status === "PASS",
  static_reference_files: 9,
  template_files: 5,
  install_plans_support_files: true
}, null, 2) + "\n");
NODE
