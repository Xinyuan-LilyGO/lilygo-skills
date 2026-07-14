# Verification Levels

Chinese version: [VERIFICATION_LEVELS.zh-CN.md](VERIFICATION_LEVELS.zh-CN.md).

lilygo-skills uses explicit verification levels so source context is not
mistaken for hardware success.

`V` means verification: what level of evidence proves the claim. Higher numbers
are closer to a real device. V3 means the context and source path are
trustworthy; V5 means the requested task has live hardware or live transport
evidence.

| Level | Meaning | Example evidence |
|-------|---------|------------------|
| V0 | Static file or schema exists | Skill file, registry entry, JSON parses |
| V1 | Data-integrity self-check | `lilygo-skills doctor --json` |
| V2 | Context routing | `lilygo-skills context`, context fixtures under `eval/**` |
| V3 | Source/context/verification | `source query`, `verify sources`, hook output |
| V4 | Runnable artifact without physical proof | Build output, simulator page data, OTA harness artifact |
| V5 | Physical device or live transport proof | Flash success, serial app log, OTA to device, display pixels, peripheral behavior |

## Natural Language Trigger

Users can ask for the evidence level directly:

| User can say | Agent should do |
|--------------|-----------------|
| "Verify that this prompt injects the right T-Display-S3/LVGL context." | Run `context` and `hook` checks, usually V2/V3 |
| "Confirm this board's display facts and demo references still hold." | Run `source query` and `verify sources`, and report drift or enrichment next steps, usually V3 |
| "Build this to a runnable artifact, but do not flash yet." | Use setup/build planning and build output, target V4 |
| "The board is plugged in. Flash it and watch the serial log." | Request serial/flash permission, collect flash success and serial app log, target V5 |
| "Verify that OTA really reaches the device." | Request network/OTA permission, use the project-private runner, collect transfer and device-side confirmation, target V5 |

Documentation and implementation guidance stay in the source/context levels.
Build, flash, serial, OTA, and display evidence paths begin when the user asks
for execution and grants the needed device or network access.

## Current Release Claim

The current release is verified at V3 for source/context/verification behavior.
Verified means:

- Exact product routing works for representative overlapping boards.
- `verify sources` reports `OK`, `DRIFT`, or `UNREACHABLE` per fact rather than
  staying silent.
- `source query` returns source-cited facts and unknowns without inventing
  values.
- `context` and `hook` readiness signals remain compact and no-write.
- `doctor` fails closed on data-integrity problems.
- `npm test`, `npx tsc --noEmit`, the ci-gate suite, and installer dry-runs
  pass.

V4/V5 evidence is task-scoped. When the user asks for execution and grants the
needed device or network access, the agent records the relevant build artifact,
flash result, serial log, display artifact, OTA transfer, or peripheral
measurement for that task. The JS thin core does not ship an automated
hardware-harness command; that evidence is collected by the agent while running
the authorized task.

## When To Claim Hardware Success

Use V5 when there is live evidence tied to the requested task, such as:

- Build plus flash command success for the target board.
- Serial monitor output from the expected firmware.
- OTA command result and device-side confirmation.
- LVGL simulator artifact for V4, or real display/camera/touch evidence for V5.
- Peripheral-specific logs or measurements for IMU, GNSS, LoRa, power, haptic,
  audio, or storage.

Official demo links and datasheet paths are useful source evidence, but they
are not proof that a local firmware build or attached board works.

## Common Verification Commands

```bash
lilygo-skills doctor --json
lilygo-skills verify sources --board <board-id> --json
npx tsc --noEmit
npm test
bash scripts/ci-gate.sh
```

V4/V5 evidence is task-scoped and lives outside the JS thin core. When a task
really needs execution, the agent runs the build, flash, serial, OTA, or
peripheral step directly, only after the user grants the matching device or
network access, and records the redacted artifact tied to that operation:

- Build with the project's own toolchain (PlatformIO, Arduino, ESP-IDF, Rust).
- Flash and read the serial monitor on the attached port.
- Run OTA through the project-private runner and capture device-side
  confirmation.

Without that explicit permission, verification stays at the V3 source/context
boundary and no execution is performed.
