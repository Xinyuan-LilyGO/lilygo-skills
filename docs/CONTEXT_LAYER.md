# Context Layers

Chinese version: [CONTEXT_LAYER.zh-CN.md](CONTEXT_LAYER.zh-CN.md).

lilygo-skills does not paste every source into the prompt at once. It separates
stable runtime logic, generated Skill files, source facts, project preferences,
and references so the agent receives the smallest useful context first, then
expands only when the task needs more detail.

## Static And Dynamic Layers

| Layer | Path | Purpose |
|-------|------|---------|
| Meta Skill | `skills/lilygo-router/SKILL.md` | The only committed Skill entry. |
| Static references | `skills/references/*.md` | Readable expansion docs, not routed skills. |
| Templates | `templates/skills/*.md` | Public templates for generated runtime Skill files. |
| Source model | `data/**`, `index/routes.json` | Board, fact, peripheral, and skill-registry data. |
| Generated skills | install/pipeline output | Materialized by install and the official-source pipeline. |

Board detection uses explicit tokens rather than unsafe prefix or substring
matches. The source model creates chip skills only for real chip identifiers;
composite labels and capacity facts stay in board or peripheral facts. The
capsule can turn existing facts and demo refs into readiness signals, but it must
not describe context as hardware proof.

Templates define file shape, not board truth. Generated skills may share section
structure because they use `templates/skills/*.md`; the real content comes from
the source model, fact packs, source packs, recipes, playbooks, and official
references. This keeps the public repository reviewable and reproducible without
committing large generated snapshots.

For T-Watch Ultra, the generated `board-t-watch-ultra` content is not an empty
template. It includes ESP32-S3, Arduino FQBN, AMOLED `CO5300`, touch
`CST9217`, GNSS `MIA-M10Q`, radio option `SX1262 or SX1280`, NFC `ST25R3916`,
IMU `Bosch BHI260AP`, power `AXP2101`, RTC `PCF85063A`, haptic `DRV2605`,
expander `XL9555`, SD, PSRAM, official LilyGoLib docs, driver headers, and demo
paths.

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

At runtime, the board Skill is only a compact snapshot. Richer task context comes
from the `context`/`hook` capsule plus `source query` and `verify sources`:

```bash
lilygo-skills context --json "T-Watch Ultra Arduino LVGL touch does not move"
lilygo-skills source query --board board-t-watch-ultra --topic io --json
lilygo-skills verify sources --board board-t-watch-ultra --topic io --json
```

Those surfaces load display, touch, IMU, source refs, and evidence boundaries for
the task without putting the entire fact pack into the default prompt.

## Default Injection And Budget

Default injection contains:

- The resolved board id and matched skills.
- Top facts required by the current task.
- The verification boundary (`context-injection`, `hardware_verified=false`,
  `evidence_boundary=V3`).
- The `source query` expansion command for the board.

Full fact packs, reference docs, and templates are not injected by default. The
agent reads those files only for implementation, debug, generation, or
verification work.

The runtime treats prompt budget as part of correctness. The thin `context`
capsule stays compact and points to `source query`; the thick `hook` capsule
inlines the top-ranked facts so common questions are answered in place. Neither
surface pastes the whole fact pack, reference docs, or generated skill bodies
into the prompt.

Useful expansion commands remain stable:

```bash
lilygo-skills source query --board <board-id> --topic io --json
lilygo-skills verify sources --board <board-id> --json
lilygo-skills context --json "<prompt>"
```

Incomplete starter board packs follow the same rule. They may expose
`unknown_with_sources` plus official references so the agent knows where to
inspect next; they do not invent pins, peripherals, or runtime behavior to fill
the capsule.

The thick capsule also inlines a few critical source-recovery pointers: the
board-owned source headers and driver facts. For example, a T-Display-S3
TFT_eSPI + I2C prompt can surface Setup206, `pin_config.h`, `PIN_IIC_SDA=GPIO18`,
`PIN_IIC_SCL=GPIO17`, and a `source query` command without injecting the whole
fact pack.

## Preferences And References

Preferences are public behavior choices such as framework order, serial debug
tooling, code-size limits, and safety defaults. References are public reading
hints such as official examples, source files, datasheets, hardware docs, and
project design notes.

Neither preferences nor references override official source facts. When a board
or topic lacks required facts, the runtime should return `unknown_with_sources`
instead of treating a reference as ready evidence.

## Installed Runtime

`node install.js --all` mounts the runtime with no build step. The installer
copies the Node dispatcher (`bin/**`) and the data model (`data/**`) together as
one self-contained runtime under `~/.claude/lilygo-skills/` and
`~/.codex/lilygo-skills/`, wires the Codex/Claude entry points and the
`UserPromptSubmit` hook, and installs the router Skill. `--build` is accepted for
backward compatibility but is a no-op; the JS dispatcher needs nothing compiled.

`doctor --json` validates the installed runtime data model from the active home:
runtime data files present, board registry and fact packs matching, V3 evidence
coverage, sniff matchers loading, and a `sample_injection` capsule to confirm the
injection chain.

The support model is board-family extensible. Current verified runtime coverage
starts with the LilyGO ESP32 family. Without matching V4/V5 evidence,
source/context output must not be described as completed hardware behavior.
