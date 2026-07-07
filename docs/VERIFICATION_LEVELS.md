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
| V1 | Registry integrity | `lilygo-skills verify --json` |
| V2 | Route behavior | `route`, route fixtures, benchmark coverage |
| V3 | Source/context/completeness | `source query`, `source completeness`, dry-run enrichment, hook output |
| V4 | Runnable artifact without physical proof | Build output, simulator page data, OTA harness artifact |
| V5 | Physical device or live transport proof | Flash success, serial app log, OTA to device, display pixels, peripheral behavior |

## Natural Language Trigger

Users can ask for the evidence level directly:

| User can say | Agent should do |
|--------------|-----------------|
| "Verify that this prompt injects the right T-Display-S3/LVGL context." | Run route, hook, or benchmark checks, usually V2/V3 |
| "Confirm this board's display facts and demo references are complete." | Run `source query`, `source completeness`, and report enrichment next steps, usually V3 |
| "Build this to a runnable artifact, but do not flash yet." | Use setup/build planning and build output, target V4 |
| "The board is plugged in. Flash it and watch the serial log." | Request serial/flash permission, collect flash success and serial app log, target V5 |
| "Verify that OTA really reaches the device." | Request network/OTA permission, use the project-private runner, collect transfer and device-side confirmation, target V5 |

Documentation and implementation guidance stay in the source/context levels.
Build, flash, serial, OTA, and display evidence paths begin when the user asks
for execution and grants the needed device or network access.

## Current Release Claim

The current release is verified at V3 for source/context/completeness behavior.
Verified means:

- Exact product routing works for representative overlapping boards.
- `source completeness` reports complete, partial, `needs_source_ingestion`, or
  unsupported rather than staying silent.
- `update board-facts --dry-run` reports enrichment paths without mutation.
- Route/hook/goal readiness signals remain compact and no-write.
- Unsupported enrichment apply fails closed.
- Benchmarks, smokes, installer dry-runs, and installed runtime probes pass.

V4/V5 evidence is task-scoped. When requested and authorized, the goal flow
records the relevant build artifact, flash result, serial log, display
artifact, OTA transfer, or peripheral measurement for that task.

The hardware gold-standard harness is part of the verification surface, but its
default run is still V3:

```bash
bash scripts/hardware-gold-standard-live-smoke.sh --dry-run
```

Dry-run validates the permission model, redaction shape, failure-mode reporting,
and artifact hashing without touching hardware. Boundary modes such as no
device, wrong port, or flash timeout return structured boundary results; live
device evidence upgrades the task to V4/V5.

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
cargo run -p lilygo-skills-cli -- verify --json
cargo run --release -p lilygo-skills-cli -- benchmark --json --iterations 5000
bash scripts/source-completeness-smoke.sh --dry-run
bash scripts/board-completeness-smoke.sh --dry-run
bash scripts/full-evidence-smoke.sh --dry-run
```

Use goal permissions only when a task really needs execution:

```bash
cargo run -p lilygo-skills-cli -- goal start --plan .tmp/goal-plan.json --allow-build --json
cargo run -p lilygo-skills-cli -- goal start --plan .tmp/goal-plan.json --allow-flash --allow-serial --port <port> --json
cargo run -p lilygo-skills-cli -- goal start --plan .tmp/goal-plan.json --allow-network --allow-ota --json
cargo run -p lilygo-skills-cli -- goal start --plan .tmp/goal-plan.json --allow-simulator --json
```

If no execution permission is given, `goal start` should remain a dry-run or
no-write planning surface.

For live hardware harness work, the permission shape is explicit:

```bash
bash scripts/hardware-gold-standard-live-smoke.sh --dry-run
bash scripts/hardware-gold-standard-live-smoke.sh --port <port> --allow-build --allow-flash --allow-serial
```

The second form may produce V4/V5 evidence only when the requested operation
actually runs and writes redacted artifacts tied to that operation.
