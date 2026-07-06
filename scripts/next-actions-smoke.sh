#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

impl="$(cargo run -q -p lilygo-skills-cli -- goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor")"
multi="$(cargo run -q -p lilygo-skills-cli -- goal plan --json "T-Display-S3 debug an SPI sensor and UART module")"
fact="$(cargo run -q -p lilygo-skills-cli -- goal plan --json "T-Display-S3 Arduino IO口怎么用? 哪些GPIO接了外设?")"

IMPL_JSON="$impl" MULTI_JSON="$multi" FACT_JSON="$fact" node <<'NODE'
const impl = JSON.parse(process.env.IMPL_JSON);
const multi = JSON.parse(process.env.MULTI_JSON);
const fact = JSON.parse(process.env.FACT_JSON);
const actions = impl.context_capsule.next_actions || [];
for (const id of ["source-query-io", "source-query-i2c", "goal-build"]) {
  if (!actions.some((action) => action.id === id)) {
    throw new Error(`missing implementation next action ${id}`);
  }
}
if (!actions.some((action) => action.permission === "allow-build")) {
  throw new Error("implementation plan lacks permission-gated build action");
}
const multiActions = multi.context_capsule.next_actions || [];
for (const id of ["source-query-spi", "source-query-uart"]) {
  if (!multiActions.some((action) => action.id === id)) {
    throw new Error(`missing multi-bus next action ${id}`);
  }
}
const factActions = fact.context_capsule.next_actions || [];
if (factActions.length === 0 || !factActions.every((action) => action.permission === "none")) {
  throw new Error("fact lookup emitted permissioned next actions");
}
if ((fact.context_capsule.demo_refs || []).length !== 0) {
  throw new Error("fact lookup should not emit demo refs");
}
console.log(JSON.stringify({
  status: "PASS",
  implementation_actions: actions.map((action) => action.id),
  multi_bus_actions: multiActions.map((action) => action.id),
  fact_actions: factActions.map((action) => action.id)
}));
NODE
