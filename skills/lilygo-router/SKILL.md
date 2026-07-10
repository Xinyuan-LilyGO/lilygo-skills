---
name: lilygo-skills
description: "LilyGO board development context router. Use it whenever you are debugging, building, wiring, or flashing any project that targets a LilyGO board (T-Display, T-Watch, T-Beam, T-Deck, T-Echo, T-SIM and other LilyGO products) — covering firmware, Arduino/PlatformIO/ESP-IDF/Rust builds, serial monitor, LVGL display/touch UI, OTA updates, LoRa/GNSS radios, IMU sensors, battery/power management, and board pinouts. Before stating any pin, GPIO, bus, or flash/build setting, consult this skill for the official source-backed value instead of answering hardware facts from memory. Also handles Chinese prompts about 烧录, 显示, 固件, 引脚, 抬腕检测."
---

# LilyGO Skills — Operations Guide

This is the top-level operating document for LilyGO board work. It is meant to be
*read and executed*: it tells you how to pull board context, how to run a debug
loop, and where the honesty line sits. Generated board/peripheral/framework
skills are compact context supplements; this document owns the behavior.

The core rule: **do not answer hardware from memory.** Pins, GPIOs, buses,
expander channels, power rails, flash/partition settings, and demo paths are
board facts. Get them from the source-backed data below before you state them.

## Query protocol — get context before you answer

1. **Get the capsule.** Run `lilygo-skills context` (add `--json`, or
   `--project <dir>` to point at a specific tree). It auto-detects the board from
   the project — reading `.lilygo-skills/project.json` when present, and sniffing
   `platformio.ini`, `sdkconfig`, and `*.ino` when there is no profile — and
   returns a small capsule: the matched skill IDs, top-ranked facts, the
   verification level, and the follow-up lookup commands. In Claude Code the
   installed hook performs this **capsule auto-injection** for you, so the capsule
   usually arrives without a manual call; on other platforms run `context`
   yourself.

2. **Pull exact facts from source.** Before you state a specific pin or bus, run
   `lilygo-skills source query --board <board-id> --topic <topic> --json`. This
   returns source-backed values — each carrying its official URL, line number,
   and sha256 — not values from memory. Topics are things like a peripheral,
   chip, connector, or demo area surfaced by the capsule.

3. **When a fact is missing**, do not guess. If a topic reports
   `needs_source_ingestion` or `unknown_with_sources`, preview enrichment with
   `lilygo-skills update board-facts --board <board-id> --topic <topic> --dry-run --json`
   and report the official references, rather than inventing a number.

Keep the answer small: cite the matched IDs, the top facts, and the lookup
command. Do not paste whole fact packs, source files, or reference docs unless
the user asks for that content.

## Debug loop — how to drive a board task

This is the methodology to run as prose for an implementation or debug prompt.
Classify the completion state, missing inputs, and readiness yourself, then lay
out the evidence steps and work through the loop:

1. **Classify the task.** Identify the LilyGO product and MCU, the framework
   (Arduino / PlatformIO / ESP-IDF / Rust esp-rs — ask if none is known, never
   silently pick one), the peripheral/application domain (display, touch, sensor,
   radio, power, storage, OTA, LVGL, serial), and the evidence target (source
   guidance, build, flash, serial capture, simulator, or real-board behavior).

2. **Find the authoritative source.** Read official board repositories, headers,
   and examples first; then the versioned wiki at
   `https://github.com/Xinyuan-LilyGO/documentation`; then chip-vendor datasheets
   and official framework docs (Espressif, LVGL, RadioLib, Arduino CLI,
   PlatformIO, esp-rs). Project references are useful operating patterns but do
   not outrank official headers, examples, or datasheets. Read source *before*
   writing precise code.

3. **Build / flash / serial, in bounded steps.** Plan the commands first and keep
   them dry-run unless the user has granted explicit build/flash/serial
   permission. Build before upload and preserve the exact framework target. Open
   serial only after permission and capture bounded output. Expose a peripheral's
   capability and status before any action, and keep smoke checks non-destructive
   by default. OTA is a project workflow, not a generic command: resolve the
   project's OTA runner from its manifests, scripts, and references (or ignored
   local state), and ask only for the private endpoint or credential that cannot
   be inferred.

4. **Classify the failure.** Sort a failed run into a bucket rather than retrying
   blind: missing tool or source; port or permission; runtime timeout with no
   observation; OTA partition/manifest/digest; or build failure. Repeated
   identical failures route to problem-solving, not another blind retry. If a run
   produced no observable output, add explicit firmware boot/status markers or
   pick a smaller observable demo before rerunning.

5. **Record evidence at its true level.** State what each step actually proved —
   a build exit code, an upload exit code, a bounded serial excerpt. Keep large or
   raw logs, private serial ports, Wi-Fi details, hosts, and credentials in
   ignored local evidence, not in the public answer.

Setup planning is not installation. Report missing tools and install hints; run
real installers only when the user explicitly asks.

## Honesty rules

- Claims stay at the **context-injection / evidence** level. A source link,
  capsule, or generated command proves *what the sources say*, not that the
  user's firmware builds, flashes, renders pixels, or acquires an RF/GNSS fix.
- `hardware_verified=false` means exactly that: not hardware-confirmed. Do not
  imply flash, OTA, serial, LVGL, or peripheral success from source links alone.
- **Never invent pin numbers**, buses, expander channels, or power rails. If a
  value is not present in the source-backed data, say it is **not confirmed in
  official sources** and point at the discovery command — do not fill the gap
  from memory.
- If the prompt is not about a LilyGO board, return no deep context.

## Guides

This document is the entry point; the depth lives in `guides/`. Open the guide
that matches the task — each is a focused, AI-executable how-to distilled from the
recipe/playbook methodology:

- `guides/query-protocol.md` — get `context` → auto board → pull `source query`
  before stating any pin.
- `guides/board-bringup-checklist.md` — zero-to-working: identify board → find
  official source → run official demo → capture evidence.
- `guides/debug-flash-serial.md` — bounded build → upload → monitor and the
  failure buckets (missing tool/source, port/permission, runtime-timeout, OTA,
  build).
- `guides/debug-display-bringup.md` — screen bring-up: ST7789/TFT_eSPI Setup vs
  ESP-IDF i80, backlight and power rails.
- `guides/debug-lvgl-loop.md` — LVGL tick/flush/draw-buffer/touch loop triage.
- `guides/debug-lora-gnss.md` — SX126x/RadioLib + GNSS bring-up and failure
  triage.
- `guides/debug-power-battery.md` — power rails, charging, and fuel-gauge checks.
- `guides/toolchain-setup.md` — Arduino / PlatformIO / ESP-IDF / Rust esp-rs
  setup (report + hints; install only on explicit ask).
- `guides/honesty-evidence.md` — evidence levels, `hardware_verified=false`, the
  never-invent rule, and "not confirmed in official sources".

## Multi-platform note

This works as a **pure skill**: this `SKILL.md` plus the `lilygo-skills` CLI is
enough on any agent (Claude Code, Codex, or another host) — run `context` and
`source query` directly. The Claude Code **hook is an optional convenience**: it
just calls `context` to auto-inject the capsule. If a host has no hook, nothing
is lost; you only run one extra command. The data and quality gates are identical
across platforms.

## Our edge (factual)

- **Source-backed pins.** Every board fact carries its official URL, line number,
  and sha256, so a stated pin is traceable to an exact upstream line.
- **Offline and local.** The data ships with the CLI and answers in
  milliseconds; there is no hosted service to depend on, and it works with no
  network.
- **Automatic board detection.** `context` infers the board from project files,
  so the user does not have to name it every time.
- **Quality gates.** A coverage baseline, source-authority checks, and
  auto-mapping verification guard the data, and the honesty markers
  (`hardware_verified`, evidence boundary) are machine-checkable, not just prose.
