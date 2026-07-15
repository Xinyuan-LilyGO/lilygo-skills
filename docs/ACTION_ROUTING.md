# Action Routing

Chinese version: [ACTION_ROUTING.zh-CN.md](ACTION_ROUTING.zh-CN.md).

Action routing keeps LilyGO context compact while making the next useful path
visible to the agent. In the JS thin core this is done by two commands over the
same board capsule:

- `context` resolves the board and returns a thin capsule (≤~1KB) that names the
  board, the verification boundary, and the `source query` command to pull exact
  facts.
- `hook` pushes the thick board capsule into the `UserPromptSubmit` context,
  inlining the top source-backed facts plus a mandatory pull-before-claim rule.

Both surfaces point the agent at exact facts instead of pasting whole sources or
generated skill bodies into the prompt.

## What The Capsule Exposes

When a board is detected, the capsule exposes:

- the resolved `board` id and how it was detected (`keyword` or project files);
- the verification boundary (`context-injection`, `hardware_verified=false`,
  `evidence_boundary=V3`);
- the `source query` command with the topics that carry facts, such as
  `pinout`, `display`, `i2c`, `spi`, `power`, `lora`, `gnss`, and `touch`;
- for the thick capsule, the top-ranked inlined facts (chip, bus, driver, and
  power pins) so common questions are answered without a second call.

Pure fact lookup stays compact. If the user asks "which pins or buses are
used?", the capsule returns the board facts and the `source query` command, not
build, flash, serial, or OTA actions. The thin core never emits mutation-oriented
actions; execution stays with the agent and the user's own toolchain.

## Pull Before Claim

The thick capsule carries a hard rule: for any concrete pin, bus, address, or
setting that is not already inlined in the capsule, the agent must first run
`source query` for the relevant topic and quote the returned official
`url + line_range + sha256`. Values that are neither inlined nor recoverable via
`source query` must be reported as unknown rather than guessed.

`verify sources` re-proves those facts against their recorded sources and reports
`OK`, `DRIFT`, or `UNREACHABLE`, so a stale capsule is caught before the agent
relies on it.

## Natural-Language Use

Users can ask directly:

```text
I am using LilyGO T-Display-S3 with PlatformIO Arduino.
Bring up the first TFT screen and then add an I2C sensor.
```

The agent should first inspect the capsule:

```bash
lilygo-skills context --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor"
```

Then pull the exact IO/I2C facts before writing pins:

```bash
lilygo-skills source query --board board-t-display-s3 --topic i2c --json
lilygo-skills source query --board board-t-display-s3 --topic pinout --json
```

For a firmware repository, pass the project directory so board detection uses the
build config and sources instead of the prompt alone:

```bash
lilygo-skills context --project . --json "bring up the display"
```

## Project Detection

`context --project <dir>` sniffs a firmware repository for board-identifying
tokens in `platformio.ini`, build config, and a bounded set of source files.
Project-file evidence is treated as more specific than prompt keywords, so an
in-repo board wins when both are present. Detection is read-only and does not
write any project-local state.

## Health Check

After install, or anytime context injection looks silent, run:

```bash
lilygo-skills doctor --json
```

`doctor` proves the runtime data model for the active install: it checks that the
runtime data files are present, that the board registry and fact packs match,
that every fact carries V3 evidence, and that the sniff matchers load. It also
returns a `sample_injection` capsule so you can confirm the injection chain end
to end. A data-integrity problem fails closed; anything the agent then runs on
hardware is verified through that task's own build/flash/serial evidence.
