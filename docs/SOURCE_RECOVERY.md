# Source Recovery

Chinese version: [SOURCE_RECOVERY.zh-CN.md](SOURCE_RECOVERY.zh-CN.md).

Source recovery is the path used when a user asks the agent to implement,
debug, or adapt firmware for a LilyGO board. The Skill keeps the default prompt
small, but it exposes the exact official demo, source headers, critical facts,
and expansion commands needed before the agent writes code.

## What Gets Surfaced

For an implementation prompt such as:

```text
I am using a LilyGO T-Display-S3 with PlatformIO Arduino.
Add an I2C sensor and show the readings on the screen.
```

the runtime should surface:

- The closest official demo, for example `examples/tft/tft.ino`.
- Board-owned source headers, for example TFT_eSPI Setup206 and
  `examples/factory/pin_config.h`.
- Critical facts such as `PIN_IIC_SDA=GPIO18` and `PIN_IIC_SCL=GPIO17`.
- Recovery commands such as `source query --board board-t-display-s3 --topic io`.
- Internal playbook expansion such as `index query playbook-source-discovery`.

The hook context receives only the compact version. Richer detail remains behind
`goal plan`, `source query`, and generated board skills.

## How It Is Used

Users do not need to request a CLI command first. Natural language is enough:

```text
I use T-Display-S3 with PlatformIO Arduino. Make a TFT_eSPI screen demo that
also reads an I2C sensor.
```

The agent can then inspect the generated context and, when more detail is
needed, expand through:

```bash
lilygo-skills goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI I2C sensor screen"
lilygo-skills source query --board board-t-display-s3 --topic io --json
lilygo-skills index query playbook-source-discovery --json
```

This keeps exact pins and demo paths source-backed instead of relying on model
memory.

## Generated Board Skills

Generated board skills include a compact `Source-Backed Board Facts` section.
For T-Display-S3, that section includes official I2C pins, touch pins, display
facts, and demo references. The source tree still does not commit generated
board snapshots; they are materialized by install, cache generation, or explicit
project generation.

```bash
lilygo-skills generate skills --out .tmp/generated-skills --json
sed -n '1,220p' .tmp/generated-skills/skills/board-t-display-s3/SKILL.md
```

## Verification

Use the smoke test when changing routing, fact packs, generated skills, or hook
rendering:

```bash
bash scripts/source-recovery-smoke.sh
```

That script checks `goal plan`, `hook codex`, `source query`, generated
`board-t-display-s3/SKILL.md`, and generated-root verification.

Source recovery is V3 source/context evidence. It can guide implementation and
debugging, but it is not a hardware-success claim until build, flash, serial,
display, OTA, or other device evidence is collected.
