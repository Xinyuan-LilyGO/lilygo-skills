#!/usr/bin/env bash
set -euo pipefail

# The committed source tree must be meta-only. Only the meta router Skill and
# the skills README may be tracked under skills/. Every generated board,
# peripheral, framework, tool, app, chip, feature, debug, or series skill is a
# runtime artifact produced by `generate skills`, never committed.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

fail() {
  echo "meta-only-source-smoke FAIL: $1" >&2
  exit 1
}

# 1. No generated SKILL.md is git-tracked.
GENERATED_TRACKED="$(git ls-files 'skills/**/SKILL.md' \
  | grep -Ev '^skills/(lilygo-router/SKILL.md|README.md)$' || true)"
if [[ -n "$GENERATED_TRACKED" ]]; then
  echo "$GENERATED_TRACKED" >&2
  fail "generated skills are committed under skills/"
fi

# 2. The forbidden generated-skill pattern must match nothing tracked.
FORBIDDEN="$(git ls-files \
  | grep -E 'skills/(app|board|chip|debug|feature|fw|periph|series|tool)-.*/SKILL.md' || true)"
if [[ -n "$FORBIDDEN" ]]; then
  echo "$FORBIDDEN" >&2
  fail "forbidden generated-skill files are tracked"
fi

# 3. The meta router Skill must still be committed.
test -f skills/lilygo-router/SKILL.md || fail "meta router skills/lilygo-router/SKILL.md missing"

# 4. The source model can regenerate all runtime skills into a cache, and the
#    generated cache verifies clean (never writing into the source tree).
cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"
OUT="$ROOT/.tmp/meta-only-generated"
rm -rf "$OUT"
"$BIN" generate skills --out "$OUT" --json >.tmp/meta-only-generate.json
"$BIN" verify --generated-root "$OUT" --json >.tmp/meta-only-verify.json

node <<'NODE'
const fs = require("fs");
const gen = JSON.parse(fs.readFileSync(".tmp/meta-only-generate.json", "utf8"));
const ver = JSON.parse(fs.readFileSync(".tmp/meta-only-verify.json", "utf8"));
function check(name, ok, detail) {
  if (!ok) { console.error(`FAIL ${name}: ${JSON.stringify(detail)}`); process.exit(1); }
}
check("generate PASS", gen.status === "PASS", gen.status);
check("generate produced skills", gen.skill_count >= 60, gen.skill_count);
check("generate is not the source tree", !gen.out_root.endsWith("/skills"), gen.out_root);
check("generated root verifies", ver.status === "PASS", ver.errors);
check("no routed skill missing", Array.isArray(ver.missing) && ver.missing.length === 0, ver.missing);
check("no unregistered generated skill", Array.isArray(ver.extra) && ver.extra.length === 0, ver.extra);
check("required reference skills present", ver.reference_skills_missing.length === 0, ver.reference_skills_missing);
check("evidence boundary honest", ver.evidence_boundary_ok === true, ver.evidence_violations);
process.stdout.write(JSON.stringify({
  status: "PASS",
  meta_only: true,
  tracked_generated_skills: 0,
  generated_skill_count: gen.skill_count,
  generated_root_verified: ver.status === "PASS"
}, null, 2) + "\n");
NODE
