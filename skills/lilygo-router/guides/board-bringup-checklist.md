# Board bring-up checklist — zero to working

Goal: take an unfamiliar LilyGO board from nothing to a running, evidence-backed
official demo. This is the spine; the peripheral guides ([[debug-display-bringup]],
[[debug-lora-gnss]], [[debug-power-battery]], [[debug-lvgl-loop]]) are the depth
for each domain.

## 1. Identify the board

Run `lilygo-skills context` ([[query-protocol]]). It infers the LilyGO product
and MCU from the project files. If it cannot, ask the user which product and
framework — never silently pick one. Record the board id, MCU, and framework.

## 2. Find the authoritative source

Read, in authority order:

1. Official LilyGO board repositories, headers (`pins_arduino.h` / variant), and
   examples.
2. The versioned wiki at `https://github.com/Xinyuan-LilyGO/documentation` and
   `https://wiki.lilygo.cc/` — navigation, not a replacement for code/header
   facts.
3. Chip-vendor datasheets and official framework docs (Espressif, LVGL, RadioLib,
   Arduino CLI, PlatformIO, esp-rs).

Project references are useful operating patterns but do not outrank official
headers, examples, or datasheets. Pull exact pins/buses with
`lilygo-skills source query --board <board-id> --topic <topic> --json`; when a
fact is missing, report it as not confirmed and cite
`update board-facts --dry-run` — do not invent it.

## 3. Run the closest official demo

Select the closest source-backed demo from the capsule's demo refs, map its
includes / driver wrappers / board-variant assumptions, then build → upload →
monitor in bounded steps ([[debug-flash-serial]]). Prove the board with the
official demo **before** writing custom code — it isolates board/toolchain
problems from your logic. Toolchain readiness first: [[toolchain-setup]].

## 4. Capture evidence at its true level

State what each step actually proved — a build exit code, an upload exit code, a
bounded serial excerpt, a simulator/page-data capture. Keep raw logs, private
ports, hosts, and credentials in ignored local evidence.

## Order of operations for a new board

power/rails up → display/backlight → official demo builds → uploads → serial
shows boot → then the target peripheral (radio, GNSS, sensor, LVGL UI). Each
stage has its own guide; classify any failure into the [[debug-flash-serial]]
buckets rather than retrying blind.

## Honesty

A source-backed demo path and a green build do not prove the board renders,
transmits, or acquires a fix. `hardware_verified=false` means not
hardware-confirmed until you observe it. See [[honesty-evidence]].
