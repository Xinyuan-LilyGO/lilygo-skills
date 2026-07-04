#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "product-board-smoke requires --dry-run for unattended runs" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo run -q -p lilygo-skills-cli -- update sources --dry-run --json >.tmp/product-update-sources.json
cargo run -q -p lilygo-skills-cli -- update boards --dry-run --json >.tmp/product-update-boards.json
cargo run -q -p lilygo-skills-cli -- update skills --dry-run --json >.tmp/product-update-skills.json
cargo run -q -p lilygo-skills-cli -- route --json "T-Watch Ultra ESP-IDF LVGL serial demo" >.tmp/product-route.json
cargo run -q -p lilygo-skills-cli -- index query board-t-watch-ultra --json >.tmp/product-index-query.json

cargo build -q -p lilygo-skills-cli
# Meta-only source tree: materialize generated skills into a cache to inspect.
"$ROOT/target/debug/lilygo-skills" generate skills --out "$ROOT/.tmp/product-generated" --json >"$ROOT/.tmp/product-generate.json"
PROFILE_ROOT="$ROOT/.tmp/product-profile-root.$$.$RANDOM"
mkdir -p "$PROFILE_ROOT"
cp -R index "$PROFILE_ROOT/index"
cp -R .tmp/product-generated/skills "$PROFILE_ROOT/skills"
cp -R data "$PROFILE_ROOT/data"

(
  cd "$PROFILE_ROOT"
  "$ROOT/target/debug/lilygo-skills" profile set --board board-t-watch-ultra --json >"$ROOT/.tmp/product-profile-set.json"
  "$ROOT/target/debug/lilygo-skills" route --json "LVGL screen is blank" >"$ROOT/.tmp/product-profile-route.json"
  "$ROOT/target/debug/lilygo-skills" route --json "how do I prune tomatoes" >"$ROOT/.tmp/product-profile-noop.json"
  "$ROOT/target/debug/lilygo-skills" profile clear --json >"$ROOT/.tmp/product-profile-clear.json"
)

node <<'NODE'
const fs = require("fs");

function read(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

const sources = read(".tmp/product-update-sources.json");
const boards = read(".tmp/product-update-boards.json");
const skills = read(".tmp/product-update-skills.json");
const route = read(".tmp/product-route.json");
const query = read(".tmp/product-index-query.json");
const profileSet = read(".tmp/product-profile-set.json");
const profileRoute = read(".tmp/product-profile-route.json");
const profileNoop = read(".tmp/product-profile-noop.json");
const profileClear = read(".tmp/product-profile-clear.json");
const skillText = fs.readFileSync(".tmp/product-generated/skills/board-t-watch-ultra/SKILL.md", "utf8");
const ultraCandidate = boards.product_candidates.find((candidate) => candidate.id === "board-t-watch-ultra");

const dryRunOk =
  sources.status === "PASS" &&
  boards.status === "PASS" &&
  skills.status === "PASS" &&
  sources.dry_run === true &&
  boards.dry_run === true &&
  skills.dry_run === true &&
  sources.writes.length === 0 &&
  boards.writes.length === 0 &&
  skills.writes.length === 0;
const routeOk =
  route.decision === "inject" &&
  route.skills.includes("board-t-watch-ultra") &&
  !route.skills.includes("board-t-watch") &&
  route.skills.includes("fw-lvgl") &&
  route.skills.includes("fw-esp-idf");
const skillOk =
  query.id === "board-t-watch-ultra" &&
  query.product === true &&
  query.family_id === "board-t-watch" &&
  skillText.includes("Peripheral Matrix") &&
  skillText.includes("Demo References") &&
  skillText.includes("github.com/Xinyuan-LilyGO/LilyGoLib") &&
  skillText.length < 30000;
const profileOk =
  profileSet.status === "PASS" &&
  profileSet.writes.includes("data/profile.json") &&
  profileRoute.skills.includes("board-t-watch-ultra") &&
  profileRoute.skills.includes("fw-lvgl") &&
  profileNoop.decision === "no-op" &&
  profileNoop.skills.length === 0 &&
  profileClear.status === "PASS";
const candidateOk =
  ultraCandidate &&
  ultraCandidate.supported === true &&
  ultraCandidate.family_id === "board-t-watch" &&
  boards.product_candidate_count >= 1;

const ok = dryRunOk && routeOk && skillOk && profileOk && candidateOk;
process.stdout.write(JSON.stringify({
  status: ok ? "PASS" : "FAIL",
  dry_run: true,
  product_candidate_count: boards.product_candidate_count,
  generated_candidate_count: boards.generated_candidate_count,
  route_skills: route.skills,
  product_skill: query.id,
  profile_route_skills: profileRoute.skills,
  profile_noop: profileNoop.decision,
  skill_bytes: Buffer.byteLength(skillText),
  hardware_verified: false,
  highest_verification_level: "V3",
  boundaries: [
    "Demo references are source evidence only.",
    "LVGL and OTA remain framework/application layers.",
    "No build, flash, serial I/O, OTA completion, or rendered pixels are claimed."
  ],
  checks: { dryRunOk, routeOk, skillOk, profileOk, candidateOk }
}, null, 2) + "\n");
process.exit(ok ? 0 : 2);
NODE
