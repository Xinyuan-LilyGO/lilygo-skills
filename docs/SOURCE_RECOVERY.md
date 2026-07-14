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
- Re-proof commands such as `verify sources --board board-t-display-s3`.

The thin `context` capsule receives only the compact version. Richer detail
remains behind `source query`, `verify sources`, and the generated board skills.

## How It Is Used

Users do not need to request a CLI command first. Natural language is enough:

```text
I use T-Display-S3 with PlatformIO Arduino. Make a TFT_eSPI screen demo that
also reads an I2C sensor.
```

The agent can then inspect the generated context and, when more detail is
needed, expand through:

```bash
lilygo-skills context --json "T-Display-S3 PlatformIO Arduino TFT_eSPI I2C sensor screen"
lilygo-skills source query --board board-t-display-s3 --topic io --json
lilygo-skills verify sources --board board-t-display-s3 --topic io --json
```

This keeps exact pins and demo paths source-backed instead of relying on model
memory.

## Generated Board Skills

Generated board skills include a compact `Source-Backed Board Facts` section.
For T-Display-S3, that section includes official I2C pins, touch pins, display
facts, and demo references. The source tree still does not commit generated
board snapshots; they are materialized by the installer and the official-source
pipeline. Inspect the facts directly with:

```bash
lilygo-skills source query --board board-t-display-s3 --topic i2c --json
lilygo-skills source query --board board-t-display-s3 --topic display --json
```

## Verification

When changing board detection, fact packs, generated skills, or hook rendering,
run the gates:

```bash
npx tsc --noEmit
npm test
bash scripts/ci-gate.sh
```

The test suite covers the `context`, `hook`, `source query`, and `verify
sources` surfaces and the generated `board-t-display-s3` facts.

Source recovery is V3 source/context evidence. It can guide implementation and
debugging, but it is not a hardware-success claim until build, flash, serial,
display, OTA, or other device evidence is collected.
