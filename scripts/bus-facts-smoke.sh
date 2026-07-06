#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

i2c="$(cargo run -q -p lilygo-skills-cli -- source query --board board-t-display-s3 --topic i2c --json)"
spi="$(cargo run -q -p lilygo-skills-cli -- source query --board board-t-display-s3 --topic spi --json)"
uart="$(cargo run -q -p lilygo-skills-cli -- source query --board board-t-display-s3 --topic uart --json)"
i2s="$(cargo run -q -p lilygo-skills-cli -- source query --board board-t-display-s3 --topic i2s --json)"
gpio="$(cargo run -q -p lilygo-skills-cli -- source query --board board-t-display-s3 --topic gpio --json)"

I2C_JSON="$i2c" SPI_JSON="$spi" UART_JSON="$uart" I2S_JSON="$i2s" GPIO_JSON="$gpio" node <<'NODE'
const i2c = JSON.parse(process.env.I2C_JSON);
const spi = JSON.parse(process.env.SPI_JSON);
const uart = JSON.parse(process.env.UART_JSON);
const i2s = JSON.parse(process.env.I2S_JSON);
const gpio = JSON.parse(process.env.GPIO_JSON);
if (i2c.topic !== "i2c" || !i2c.facts.some((fact) => fact.key === "i2c.primary.sda")) {
  throw new Error("I2C topic did not expose source-backed SDA fact");
}
if (!i2c.facts.some((fact) => fact.value === "GPIO17")) {
  throw new Error("I2C topic did not expose SCL GPIO17");
}
if (spi.topic !== "spi" || !spi.unknowns.some((fact) => fact.confidence === "unknown_with_sources")) {
  throw new Error("SPI topic should return source-backed unknown instead of guessing");
}
for (const [name, report] of [["uart", uart], ["i2s", i2s]]) {
  if (report.topic !== name || !report.unknowns.some((fact) => fact.confidence === "unknown_with_sources")) {
    throw new Error(`${name.toUpperCase()} topic should return source-backed unknown instead of guessing`);
  }
}
if (gpio.topic !== "gpio" || gpio.facts.length === 0) {
  throw new Error("GPIO topic returned no pin facts");
}
console.log(JSON.stringify({ status: "PASS", i2c_facts: i2c.facts.length, gpio_facts: gpio.facts.length }));
NODE
