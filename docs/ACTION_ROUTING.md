# Action Routing

Action routing keeps LilyGO context compact while making the next useful path
visible to the agent.

When a prompt is about implementation or debugging, `goal plan` and installed
hook context can expose:

- the smallest relevant official demo, such as `examples/tft/tft.ino` for a
  first T-Display-S3 screen instead of the full factory test;
- source queries for exact IO and bus facts, including `io`, `i2c`, `spi`,
  `uart`, `i2s`, and `gpio`;
- permission-aware `next_actions` that distinguish read-only source lookup
  from build, flash, serial, network, or OTA work;
- project-local custom skill hints from `.lilygo-skills/skills/index.json`;
- install health through `doctor --json`.

Pure fact lookup remains compact. If the user asks "which pins or buses are
used?", the runtime returns fact tables and source-query commands, not build,
flash, serial, OTA, or demo actions.

## Natural-Language Use

Users can ask directly:

```text
I am using LilyGO T-Display-S3 with PlatformIO Arduino.
Bring up the first TFT screen and then add an I2C sensor.
```

The agent should first inspect:

```bash
lilygo-skills goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor"
```

The capsule should rank the minimal TFT demo first, expose `source query`
commands for IO/I2C, and show permission-gated build or device steps.

## Project Custom Skills

Firmware repositories can add local operating patterns without modifying the
public LilyGO runtime:

```text
.lilygo-skills/
  skills/
    index.json
    project-lvgl-loop/
      SKILL.md
```

Each custom skill id must start with `project-`, use a relative path under
`.lilygo-skills/skills/`, and avoid private paths, credentials, raw logs,
serial ports, or local network values. Project skills are supplemental
patterns; official board facts, headers, and examples remain the authority.

## Health Check

After install, or anytime context injection looks silent, run:

```bash
lilygo-skills doctor --json
```

When checking an installed sandbox or user home:

```bash
lilygo-skills doctor --json --home "$HOME"
```

`doctor` proves the runtime data, generated skills, route sample, no-op sample,
and installed host files where present. It does not prove hardware behavior.
