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
- `goal-plan-bridge`, a read-only next action that points the agent at
  `goal plan` before code edits or hardware actions;
- project-local custom skill hints from `.lilygo-skills/skills/index.json`;
- install health through `doctor --json`.

Pure fact lookup remains compact. If the user asks "which pins or buses are
used?", the runtime returns fact tables and source-query commands, not build,
flash, serial, OTA, or demo actions.

The classifier is intentionally asymmetric:

| Prompt shape | Routing behavior |
|--------------|------------------|
| Pure lookup: "which pins are used by the screen?", "哪些引脚被屏幕占用了?" | Read-only capsule: facts and source-query commands only |
| Implementation/debug: "bring up the display", "让屏幕先亮起来", "debug the sensor" | Goal bridge, selected demos, playbooks, and permission-labelled next actions |
| Mixed: "check the pins, then bring up the display" | Implementation/debug wins, but lookup expansion stays visible |

Short words such as "first", "minimal", "先", or "最小" do not trigger a demo
by themselves. They affect demo ranking only when paired with a display/run or
factory-test intent.

This is also the token-budget rule. The default capsule should expose the path
to more context, not paste every source or generated Skill body into the prompt.
When more detail is needed, the agent expands with `source query`, `index
query`, generated project skills, or `goal plan`.

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

For lookup-only wording, the same board prompt should stay smaller:

```bash
lilygo-skills route --json "T-Display-S3 的 I2C 引脚和外设地址有哪些?"
```

That output should keep fact/source-query context and omit demos, recipes, and
mutation-oriented actions.

For explicit factory bring-up, the larger factory example remains reachable:

```bash
lilygo-skills goal plan --json "T-Display-S3 run the full factory test"
```

The expected behavior is not "always use the smallest demo"; it is "use the
smallest demo for first visible output, and keep full factory examples for
full-board diagnostics."

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
and active Codex/Claude wiring for the checked home. Missing integrations are
warnings; malformed LilyGO hook wiring is a failure. When both host runtimes
exist, `doctor` also warns if their binary or data mirrors differ and prints the
reinstall command. It does not prove hardware behavior.
