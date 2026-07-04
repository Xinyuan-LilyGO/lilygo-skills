# Skill Generation

Chinese version: [SKILL_GENERATION.zh-CN.md](SKILL_GENERATION.zh-CN.md).

This project publishes a small meta Skill plus a deterministic generator. The
source tree does not commit generated board, chip, peripheral, framework,
playbook, debug, app, or recipe Skill snapshots.

## Source Inputs

| Path | Role |
|------|------|
| `skills/lilygo-router/SKILL.md` | Meta entry loaded by the agent. |
| `skills/references/*.md` | Static expansion references for operating context. |
| `templates/skills/*.md` | Markdown templates used by generation. |
| `index/routes.json` | Skill registry and route triggers. |
| `data/boards.json` | Board/product source model. |
| `data/facts/**` | Source-backed board facts and completeness data. |
| `data/peripherals/**` | Peripheral/chip/feature source packs. |
| `data/playbooks/**` | Generated playbook source model. |
| `data/skills/reference/**` | Source markdown for reference practice skills. |

## Generated Output

```text
<out>/skills/<skill-id>/SKILL.md
<out>/skills/references/*.md
<out>/templates/skills/*.md
<out>/index/routes.json
```

Only directories with a `SKILL.md` are routed generated skills. The
`skills/references/` support directory is copied for expansion and does not
become a skill.

## Natural Language Trigger

Users do not need to memorize the generation command. They can say:

```text
Initialize this firmware repo for the LilyGO Skill. I use T-Display-S3 with PlatformIO.
```

The agent should map that to project-local initialization:

```bash
lilygo-skills project init --project . --board board-t-display-s3 --framework fw-platformio --json
```

That writes committed `.lilygo-skills/project.json` and an ignored
`.lilygo-skills/generated-skills/` cache.

If the user says:

```text
Regenerate the LilyGO skills for this project and check that they are complete.
```

the agent should write only the generated cache, not the source tree:

```bash
lilygo-skills generate skills --out .lilygo-skills/generated-skills --json
lilygo-skills verify --generated-root .lilygo-skills/generated-skills --json
```

Route and hook never generate files for ordinary questions. They report needed
layers, missing pieces, and the executable generate/update next step; actual
writes require an explicit install, project-init, generate, or update request.

## Template Shape, Source Content

Generated files share a predictable shape because the CLI renders them from
`templates/skills/*.md`. The useful content does not live in the template. It
comes from `data/boards.json`, `data/facts/**`, `data/peripherals/**`,
`data/playbooks/**`, `data/recipes/**`, `index/routes.json`, and official source
references.

Generated chip skills are intentionally narrow: they are created only for real
chip identifiers. Composite descriptions such as "SX1262 or SX1280", memory
capacity labels, and storage media stay in board/peripheral facts and source
query output. This keeps the chip layer precise while still exposing those
facts through `goal plan`, `goal complete`, and `source query`.

For example, `board-t-watch-ultra` is generated with a shared board template,
but its content includes T-Watch Ultra-specific facts: ESP32-S3, Arduino FQBN,
AMOLED `CO5300`, touch `CST9217`, GNSS `MIA-M10Q`, radio option
`SX1262 or SX1280`, NFC `ST25R3916`, IMU `Bosch BHI260AP`, power `AXP2101`,
RTC `PCF85063A`, haptic `DRV2605`, expander `XL9555`, SD, memory, official
LilyGoLib docs, driver headers, and demo paths.

Focused generated tree for T-Watch Ultra:

```text
skills/board-t-watch-ultra/SKILL.md
skills/chip-bhi260ap/SKILL.md
skills/chip-xl9555/SKILL.md
skills/feature-raise-to-wake/SKILL.md
skills/periph-display/SKILL.md
skills/periph-imu/SKILL.md
skills/periph-input/SKILL.md
skills/app-ota/SKILL.md
skills/app-watch-ui-lvgl/SKILL.md
skills/debug-lvgl-loop/SKILL.md
skills/fw-lvgl/SKILL.md
skills/playbook-build-flash-serial/SKILL.md
skills/playbook-lvgl-debug/SKILL.md
skills/playbook-ota-debug/SKILL.md
skills/playbook-source-discovery/SKILL.md
skills/references/*.md
templates/skills/*.md
index/routes.json
```

Use these commands to inspect the concrete generated content:

```bash
lilygo-skills generate skills --out .tmp/generated-skills --json
sed -n '1,220p' .tmp/generated-skills/skills/board-t-watch-ultra/SKILL.md
lilygo-skills goal plan --json "T-Watch Ultra Arduino LVGL touch does not move"
lilygo-skills goal plan --json "T-Watch Ultra ESP-IDF OTA rollback manifest debug"
lilygo-skills source query --board board-t-watch-ultra --topic io --json
```

The board Skill is the compact generated snapshot. The richer task-time context
is in `goal plan.context_capsule`, `source query`, and `source completeness`,
which keep default injection small while still giving the agent source-backed
facts and official demo references when implementation or debug work needs them.

## Template-Driven Renderers

Currently template-rendered:

- `templates/skills/board.md`
- `templates/skills/peripheral.md`
- `templates/skills/playbook.md`

Committed but reserved as public shape contracts:

- `templates/skills/reference.md`
- `templates/skills/framework.md`

Every template-rendered file includes:

```text
Generation Contract: templates/skills/<kind>.md
```

## Verification

```bash
lilygo-skills generate skills --out .tmp/generated-skills --json
lilygo-skills verify --generated-root .tmp/generated-skills --json
bash scripts/static-context-template-smoke.sh --dry-run
bash scripts/meta-only-source-smoke.sh
```

Generation is V3 source/context evidence. Hardware behavior still needs its own
build, simulator, flash, serial, OTA, RF/GNSS, display, or peripheral evidence.
