#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-project-skill.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

cargo run -q -p lilygo-skills-cli -- project init \
  --project "$tmp" \
  --board board-t-display-s3 \
  --framework fw-arduino \
  --json >/dev/null

mkdir -p "$tmp/.lilygo-skills/skills/project-lvgl-loop"
cat >"$tmp/.lilygo-skills/skills/index.json" <<'JSON'
{
  "schema_version": 1,
  "skills": [
    {
      "id": "project-lvgl-loop",
      "kind": "debug",
      "path": ".lilygo-skills/skills/project-lvgl-loop/SKILL.md",
      "summary": "Project LVGL loop checklist.",
      "triggers": ["lvgl", "touch"],
      "authority": "project-pattern",
      "read_when": "Use after official board facts are loaded."
    }
  ]
}
JSON
cat >"$tmp/.lilygo-skills/skills/project-lvgl-loop/SKILL.md" <<'MD'
---
name: project-lvgl-loop
description: Project LVGL bring-up checklist.
---

# Project LVGL Loop

Use this after official LilyGO board facts and display source refs are loaded.
MD

route="$(cargo run -q -p lilygo-skills-cli -- route --project "$tmp" --json "LVGL touch debug")"
plan="$(cargo run -q -p lilygo-skills-cli -- context --plan --project "$tmp" --json "LVGL touch debug")"

ROUTE_JSON="$route" PLAN_JSON="$plan" node <<'NODE'
const route = JSON.parse(process.env.ROUTE_JSON);
const plan = JSON.parse(process.env.PLAN_JSON);
if (!route.skills.includes("project-lvgl-loop")) {
  throw new Error("route did not include project-lvgl-loop");
}
const hints = plan.context_capsule.internal_skill_hints || [];
if (!hints.some((hint) => hint.skill_id === "project-lvgl-loop")) {
  throw new Error("context --plan did not expose compact project skill hint");
}
NODE

cat >"$tmp/.lilygo-skills/skills/project-lvgl-loop/SKILL.md" <<'MD'
---
name: project-lvgl-loop
description: private leak
---

Do not publish /Users/adan/private.log.
MD

if cargo run -q -p lilygo-skills-cli -- route --project "$tmp" --json "LVGL touch debug" >/tmp/lilygo-project-skill-private.out 2>&1; then
  echo "project skill privacy validation did not fail" >&2
  exit 1
fi

echo '{"status":"PASS","project_skill":"project-lvgl-loop"}'
