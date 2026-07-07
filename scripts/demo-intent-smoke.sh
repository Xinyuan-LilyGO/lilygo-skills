#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

first_run="$(cargo run -q -p lilygo-skills-cli -- goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor")"
factory="$(cargo run -q -p lilygo-skills-cli -- goal plan --json "T-Display-S3 Arduino factory full peripheral test")"
chinese_first="$(cargo run -q -p lilygo-skills-cli -- goal plan --json "T-Display-S3 Arduino 帮我让屏幕先亮起来，跑个最简单的显示例程")"
chinese_factory="$(cargo run -q -p lilygo-skills-cli -- goal plan --json "T-Display-S3 Arduino 跑完整出厂测试")"

FIRST_RUN_JSON="$first_run" FACTORY_JSON="$factory" CHINESE_FIRST_JSON="$chinese_first" CHINESE_FACTORY_JSON="$chinese_factory" node <<'NODE'
const firstRun = JSON.parse(process.env.FIRST_RUN_JSON);
const factory = JSON.parse(process.env.FACTORY_JSON);
const chineseFirst = JSON.parse(process.env.CHINESE_FIRST_JSON);
const chineseFactory = JSON.parse(process.env.CHINESE_FACTORY_JSON);
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
const chineseFirstDemo = chineseFirst.context_capsule.demo_refs[0]?.path;
if (chineseFirstDemo !== "examples/tft/tft.ino") {
  throw new Error(`Chinese first-run display prompt selected ${chineseFirstDemo}`);
}
const chineseFactoryDemo = chineseFactory.context_capsule.demo_refs[0]?.path;
if (chineseFactoryDemo !== "examples/factory/factory.ino") {
  throw new Error(`Chinese factory prompt selected ${chineseFactoryDemo}`);
}
console.log(JSON.stringify({
  status: "PASS",
  first_demo: firstDemo,
  factory_demo: factoryDemo,
  chinese_first_demo: chineseFirstDemo,
  chinese_factory_demo: chineseFactoryDemo
}));
NODE
