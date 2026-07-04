#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "setup-plan-smoke requires --dry-run" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p .tmp

cargo build -q -p lilygo-skills-cli
BIN="$ROOT/target/debug/lilygo-skills"

"$BIN" setup --help >.tmp/setup-help.txt
for framework in arduino platformio esp-idf rust; do
  "$BIN" setup plan --framework "$framework" --json >".tmp/setup-plan-$framework.json"
done

node <<'NODE'
const fs = require("fs");
function read(framework) {
  return JSON.parse(fs.readFileSync(`.tmp/setup-plan-${framework}.json`, "utf8"));
}
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${name}`);
    console.error(JSON.stringify(detail, null, 2));
    process.exit(1);
  }
}
const required = {
  arduino: ["arduino-cli", "arduino-esp32-core", "lilygo-libraries"],
  platformio: ["platformio-core", "platformio-esp32-platform"],
  "esp-idf": ["esp-idf", "idf-tools"],
  rust: ["espup", "espflash", "cargo-espflash"]
};
const results = {};
for (const framework of Object.keys(required)) {
  const plan = read(framework);
  const ids = plan.toolchains.map((tool) => tool.id);
  results[framework] = ids;
  check(`${framework} plan is no-mutation`, plan.status === "planned" &&
    plan.dry_run === true &&
    plan.no_mutation === true &&
    Array.isArray(plan.writes) &&
    plan.writes.length === 0 &&
    plan.toolchains.every((tool) => tool.mutates === false), plan);
  check(`${framework} common requirements`, ["rustup", "cargo", "node"].every((id) => plan.host_requirements.includes(id)), plan);
  check(`${framework} framework tools`, required[framework].every((id) => ids.includes(id)), plan);
  check(`${framework} next commands`, plan.next_commands.some((command) => command.includes("node install.js --all --dry-run")), plan.next_commands);
}
const help = fs.readFileSync(".tmp/setup-help.txt", "utf8");
check("setup help", help.includes("setup plan --framework"), help);

process.stdout.write(JSON.stringify({
  status: "PASS",
  checked: [
    "setup help",
    "arduino setup plan",
    "platformio setup plan",
    "esp-idf setup plan",
    "rust setup plan",
    "no mutation"
  ],
  toolchains: results
}, null, 2) + "\n");
NODE
