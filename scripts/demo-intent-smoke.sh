#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

first_run="$(cargo run -q -p lilygo-skills-cli -- goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor")"
factory="$(cargo run -q -p lilygo-skills-cli -- goal plan --json "T-Display-S3 Arduino factory full peripheral test")"

FIRST_RUN_JSON="$first_run" FACTORY_JSON="$factory" node <<'NODE'
const firstRun = JSON.parse(process.env.FIRST_RUN_JSON);
const factory = JSON.parse(process.env.FACTORY_JSON);
const firstDemo = firstRun.context_capsule.demo_refs[0]?.path;
if (firstDemo !== "examples/tft/tft.ino") {
  throw new Error(`first-run display prompt selected ${firstDemo}`);
}
if (!firstRun.context_capsule.demo_refs.some((demo) => demo.path === "examples/factory/factory.ino")) {
  throw new Error("factory demo disappeared from display prompt candidates");
}
const factoryDemo = factory.context_capsule.demo_refs[0]?.path;
if (factoryDemo !== "examples/factory/factory.ino") {
  throw new Error(`factory prompt selected ${factoryDemo}`);
}
console.log(JSON.stringify({ status: "PASS", first_demo: firstDemo, factory_demo: factoryDemo }));
NODE
