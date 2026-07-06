# lilygo-skills

A skill runtime for AI-assisted LilyGO board development.

Install it into Codex, Claude Code, or another AI agent once. After that, the
user can describe firmware work in normal language and the agent can load the
right LilyGO board, framework, source facts, official examples, setup hints, and
safe debug steps without asking the user to manually search documentation.

The committed meta Skill is the operating entry for context injection. It keeps
board/framework/domain classification, source lookup, bounded debug planning,
permissioned command execution, completion state reporting, failure
classification, and evidence recording in one small front door. Generated
skills are supplemental context for that runtime flow, not the product boundary
by themselves.

Documentation:

| Topic | English | 中文 |
|-------|---------|------|
| Overview | [README.md](README.md) | [README.zh-CN.md](README.zh-CN.md) |
| Architecture | [ARCHITECTURE.md](ARCHITECTURE.md) | [ARCHITECTURE.zh-CN.md](ARCHITECTURE.zh-CN.md) |
| Context layers | [docs/CONTEXT_LAYER.md](docs/CONTEXT_LAYER.md) | [docs/CONTEXT_LAYER.zh-CN.md](docs/CONTEXT_LAYER.zh-CN.md) |
| Skill generation | [docs/SKILL_GENERATION.md](docs/SKILL_GENERATION.md) | [docs/SKILL_GENERATION.zh-CN.md](docs/SKILL_GENERATION.zh-CN.md) |
| Board facts | [docs/BOARD_FACTS.md](docs/BOARD_FACTS.md) | [docs/BOARD_FACTS.zh-CN.md](docs/BOARD_FACTS.zh-CN.md) |
| Source recovery | [docs/SOURCE_RECOVERY.md](docs/SOURCE_RECOVERY.md) | [docs/SOURCE_RECOVERY.zh-CN.md](docs/SOURCE_RECOVERY.zh-CN.md) |
| Verification levels | [docs/VERIFICATION_LEVELS.md](docs/VERIFICATION_LEVELS.md) | [docs/VERIFICATION_LEVELS.zh-CN.md](docs/VERIFICATION_LEVELS.zh-CN.md) |
| Board contribution | [docs/CONTRIBUTING_BOARDS.md](docs/CONTRIBUTING_BOARDS.md) | [docs/CONTRIBUTING_BOARDS.zh-CN.md](docs/CONTRIBUTING_BOARDS.zh-CN.md) |

The public repository is the runtime source: CLI, installer, router Skill,
source model, templates, references, schemas, and release gates.

The project is intended to grow across LilyGO boards. Current verified runtime
coverage starts with LilyGO products in the ESP32 family: ESP32, ESP32-S2,
ESP32-S3, ESP32-C3, and ESP32-P4. Other LilyGO products can be recorded as
future source candidates, but the runtime must report unsupported build, flash,
OTA, or hardware-debug guidance until that support is designed and verified.

## Install Into Your AI Agent

Give your agent the repo link and ask it to install the Skill:

```text
Please install this LilyGO Skill from https://github.com/Xinyuan-LilyGO/lilygo-skills
and use it for this firmware repo. If Node.js is missing, tell me first. If the
full Rust runtime needs to be built, ask before installing Rust/Cargo.
```

Recommended environment:

- Git, for cloning the repository and source references.
- Node.js, required for running `install.js` and mounting the Skill.
- Rust/Cargo, recommended for the full dynamic runtime. If it is missing, the
  Skill can still be mounted first; the agent will use setup guidance to help
  configure Rust/Cargo or install a prebuilt runtime later.

The agent should check them first:

```bash
git --version
node --version
rustup --version   # only needed before local runtime build
cargo --version    # only needed before local runtime build
```

If Node.js is missing, the agent should explain that the installer cannot run
yet. If Rust/Cargo is missing, the agent can still mount the Skill first, then
use the mounted setup guidance to help configure Rust/Cargo or use a prebuilt
runtime. The Skill installer itself does not silently install host dependencies.
Framework tools such as Arduino CLI, PlatformIO, ESP-IDF, esp-rs, board cores,
serial tools, and radio/GNSS libraries are also configured later by the agent
from `setup plan` and the current firmware task.

Manual mount after Git and Node.js are present:

```bash
git clone https://github.com/Xinyuan-LilyGO/lilygo-skills.git
cd lilygo-skills
node install.js --all --dry-run
node install.js --all
```

The installer writes an agent runtime under:

```text
~/.codex/lilygo-skills/
~/.claude/lilygo-skills/
```

and wires both hosts so injection actually happens:

- **Claude Code**: the router skill (with YAML frontmatter) is installed to
  `~/.claude/skills/lilygo-skills/SKILL.md`, and a `UserPromptSubmit` hook is
  merged idempotently into `~/.claude/settings.json`:

  ```json
  {
    "hooks": {
      "UserPromptSubmit": [
        {
          "hooks": [
            {
              "type": "command",
              "command": "\"$HOME/.claude/lilygo-skills/bin/lilygo-skills\" hook claude"
            }
          ]
        }
      ]
    }
  }
  ```

  The hook emits the `hookSpecificOutput.additionalContext` envelope on LilyGO
  prompts and stays silent (fail-open, exit 0) on everything else. If your
  `settings.json` is not valid JSON the installer reports it loudly and prints
  this manual wiring snippet instead of touching the file. Repeated installs
  never duplicate the entry.

- **Codex**: a marked `lilygo-skills` section is appended once to
  `~/.codex/AGENTS.md`, pointing the agent at the runtime root and the
  `route --json` discovery protocol. Re-installs replace the marked section in
  place.

To uninstall, do it in this order: first remove the `UserPromptSubmit` entry
from `~/.claude/settings.json` (otherwise every prompt reports a failing hook
command), then delete `~/.claude/skills/lilygo-skills/`,
`~/.claude/lilygo-skills/`, the marked section in `~/.codex/AGENTS.md`, and
`~/.codex/lilygo-skills/`.

The public source tree is meta-only: the single committed Skill is
`skills/lilygo-router/SKILL.md`, the meta router. Board, series, framework,
tool, peripheral, chip, feature, debug, and app skills are no longer committed
under `skills/`; they are generated on demand from the source model in `data/`.
Static docs under `skills/references/` and generation contracts under
`templates/skills/` are committed so users can inspect how context is selected
and how generated Skill files are shaped.

When a compiled runtime is available, `install.js` generates runtime skills into
the install root by invoking the CLI's `generate skills`, rather than copying a
committed snapshot. It installs the `lilygo-skills` binary, freshly generated
runtime skills, source/fact data, and the meta router Skill that agents load.
Source truth lives in `data/`, `index/`, and official references; full
installation regenerates skills from that source model each time it runs.
The install root also contains `skills/references/` and `templates/skills/`, so
installed agents can inspect the same contracts without reading the source
checkout.

If no compiled runtime is present and `--build` is not requested, `install.js`
still succeeds in **mount-only** mode. It wires the Codex/Claude entry points,
copies the meta router, data, templates, and references, and installs a small
setup-only launcher. That launcher does not pretend to provide full board fact
injection; it tells the agent to run `setup plan` and then build or install the
runtime before deeper firmware work.

Use `--build` when the agent should upgrade the mount into full dynamic context
in the same step. `install.js --build` runs
`cargo build --release -p lilygo-skills-cli` before installing. Without
`--build`, the installer prefers `target/release/lilygo-skills`, falls back to
`target/debug/lilygo-skills`, then falls back to mount-only mode. An agent can
also install an explicit binary:

```bash
node install.js --all --dry-run
node install.js --all
node install.js --all --dry-run --build
node install.js --all --build
node install.js --all --profile release
node install.js --all --bin /path/to/lilygo-skills
```

Normal installed usage calls `lilygo-skills` directly. `cargo run` is only for
source checkout development and tests. If an agent already has a compiled
binary, it can install that artifact with `--bin` without building the CLI
again.

The installer also does not silently install Arduino CLI, PlatformIO, ESP-IDF,
esp-rs, board cores, firmware libraries, or LoRa/GNSS dependencies.

Setup is routed through the Skill before any installer is run. For machine
readiness, use the read-only setup planner:

```bash
lilygo-skills setup plan --framework arduino --json
lilygo-skills setup plan --framework platformio --json
lilygo-skills setup plan --framework esp-idf --json
lilygo-skills setup plan --framework rust --json
```

`setup plan` reports checks and install hints with `writes=[]`; it does not
install packages, edit firmware files, open serial ports, or flash hardware.

## Use Natural Language

After installation, users should talk to the AI agent, not study the CLI first:

```text
I am using a LilyGO T-Display-S3 with PlatformIO Arduino.
Add an I2C temperature sensor and show the readings on an LVGL screen.
```

```text
This repo targets LilyGO T-Beam.
Set up LoRa + GNSS telemetry and give me the serial debug path.
```

```text
I have a LilyGO T-Deck display project.
Find the right display/input references, build a small UI, and explain how to verify it.
```

The agent uses this Skill to decide which compact context to inject, which
official examples or source files to inspect, and which setup/debug commands are
safe to run.

Common tasks can be requested directly:

| User can say | Agent should trigger |
|--------------|----------------------|
| "Initialize this repo for LilyGO. I use T-Display-S3 and PlatformIO." | `project init`, committed `.lilygo-skills/project.json`, ignored project cache |
| "Regenerate the LilyGO skills for this project and check them." | `generate skills --out .lilygo-skills/generated-skills` plus `verify --generated-root` |
| "I want to use Arduino/ESP-IDF/PlatformIO/Rust. Check my machine setup." | `setup plan --framework ...` |
| "How do I wire/use this display/LoRa/GNSS/sensor, and which demo should I read?" | `source query` and the generated board/peripheral layers |
| "Help me implement this feature and tell me what is still missing first." | `goal complete --dry-run` or `goal plan` |
| "Run the benchmark and confirm context injection did not regress." | `benchmark --generated-root ...` or the default registry benchmark |
| "Verify this to V3/V4/V5 and show the evidence." | The matching route/source/build/flash/serial/OTA/display evidence path |

These natural-language prompts map to explicit runtime paths. Ordinary Q&A does
not write files implicitly; install, project init, generation, update,
implementation, and verification work is triggered only when the user asks for
that kind of action.

For implementation, setup, demo, and debug work, the agent should normally start
with:

```bash
lilygo-skills goal complete --dry-run --json "<prompt>"
```

That single capsule reports whether the request is ready, needs a board or
framework clarification, needs source ingestion, needs generated skills, needs
setup, needs explicit permission, or can be executed through the existing safe
goal runner.

For implementation or debug requests, the Skill also routes generated
playbooks. They are short operating guides for source discovery, setup,
build/flash/serial, LVGL, OTA, BSP drivers, and radio/GNSS work. The agent sees
only compact playbook ids and summaries first, then expands a playbook with
`lilygo-skills index query playbook-lvgl-debug --json` or the matching
`playbook-*` id when the task needs the full checklist.

If the user names a framework, the agent loads that framework layer. If a
firmware/build task needs a framework and none is known from the prompt or
project context, the runtime returns `needs_clarification` with choices such as
Arduino, PlatformIO, ESP-IDF, and Rust esp-rs. A lightweight context lookup can
remain framework-unspecified instead of forcing a choice too early.

## What The Agent Does

For a prompt such as the T-Display-S3 sensor example, the Skill helps the agent:

1. Identify the exact board and framework from the prompt or project context.
2. Load only the compact board/framework/display/sensor-related layers.
3. Query source-backed facts for pins, buses, connectors, peripherals, and demo
   references when implementation detail is needed.
4. Return `needs_source_ingestion` with official references and update commands
   if the board/topic is known but not complete enough.
5. Add source-first playbook hints for the requested work pattern, then expand
   the relevant playbook only when the task needs the detailed checklist.
6. Use `goal complete` to choose the next completion state before running work.
7. Produce a setup, source, or debug plan for Arduino, PlatformIO, ESP-IDF,
   Rust esp-rs, LVGL, serial, OTA, simulator, build, or flash work.
8. Ask before actions that touch hardware, serial ports, networks, OTA, or
   simulator artifacts.

This means a beginner can start from a product name and a goal, while the agent
still avoids inventing GPIOs, buses, display chips, or unsupported workflows.

Peripherals are board facts first: pins, buses, expanders, connectors, power
rails, display panels, radios, sensors, storage, input devices, and demos must
come from the board's source-backed fact pack. Peripheral/chip layers are
reusable indexes that help route similar parts across boards; they do not
replace board-specific facts. For LoRa/GNSS prompts, for example, the runtime
can route to T-Beam, LoRa, GNSS, Arduino, and serial-debug context, but exact
chip, bus, antenna, region, and demo guidance still depends on source
completeness for that board.

## Progressive Disclosure

The runtime is intentionally layered so it does not flood the model context.

| Layer | Loaded when needed | Purpose |
|-------|--------------------|---------|
| L0 | Always | Router, hook envelope, verify, benchmark |
| L1 | Board/product prompt or project context | LilyGO board, MCU family, source pointers |
| L2 | Peripheral/chip/feature intent | Display, sensor, GNSS, LoRa, power, storage, input |
| L3 | Framework intent | Arduino, PlatformIO, ESP-IDF, Rust esp-rs, LVGL |
| L4 | Implementation/debug intent | Build, flash, serial, OTA, simulator, app recipes |
| L5 | Firmware repo context | `.lilygo-skills/project.json` defaults and clarification |
| L6 | User asks to implement/debug | Goal plan, permissions, artifacts, evidence boundary |
| L7 | Detail is required | Source facts, preferences, reference read hints |
| L8 | Facts are incomplete | Completeness status and enrichment next actions |
| L9 | Reusable implementation/debug pattern is needed | Generated playbook hints and expansion commands |
| L10 | Agent needs to finish a task | `goal complete` state, plan, permissions, and evidence summary |

Route and hook output stay small: ids, summaries, top facts, readiness status,
and lookup commands. Full fact packs, official source files, and long reference
docs are read only when the task needs them. Playbooks follow the same rule:
route and hook inject compact ids such as `playbook-lvgl-debug`, while the
agent expands the full generated playbook only after the user asks to implement,
debug, set up, flash, validate, or diagnose something.

On first use of a board, the installed runtime selects the generated layers that
already exist in the install root. Route and hook stay no-write: if a routed
generated skill is missing, they may report it and include a compact
generate/update command, but they never write skills implicitly and never fetch
network sources. Only explicit install, update, project-init, and generate
commands write generated skills, and only to an install root, a project cache,
or a test output directory. New or stale board data is refreshed through
explicit update commands such as `update boards`, `update skills`,
`update source-packs`, and `update board-facts`.

## Project Context

For a firmware repo, the agent can save public defaults:

```bash
lilygo-skills project init \
  --project /path/to/firmware \
  --board board-t-display-s3 \
  --framework fw-platformio \
  --json
```

This writes `.lilygo-skills/project.json`, which can be committed, and
materializes `.lilygo-skills/generated-skills/` as an ignored project cache.
Machine-local evidence belongs in `.lilygo-skills/local.json` or
`.lilygo-skills/evidence/` and must stay ignored.
OTA execution uses the same private layer. When OTA is requested, the agent
looks for project manifests, scripts, references, and ignored local runner
settings. It can record concrete `ota_manifest_argv` and `ota_observe_argv`
arrays in `.lilygo-skills/local.json` after deriving them from the project or
after asking only for private details that cannot be inferred. Private values
stay out of public prompt context and command output.

Routing precedence is:

```text
explicit prompt > project context > global profile > needs_clarification > no-op
```

If the board or framework is missing, the agent receives structured questions
instead of silently guessing.

## Preferences And References

Preferences tell the agent how you like LilyGO work to be done. They can set
framework order, preferred debug tools, code size limits, and safety defaults.
They are behavior policy, not source material:

```bash
lilygo-skills preference show --json
lilygo-skills preference show --project /path/to/firmware --json
```

Project preferences live in `.lilygo-skills/preferences.json` and can be
committed when they contain only public behavior choices:

Users do not have to hand-write this file first. They can say:

```text
For this firmware repo, prefer PlatformIO, use serial-mcp-server for serial debug, and keep firmware functions under 60 lines.
```

The agent should confirm that these are public behavior preferences, then write
or update `.lilygo-skills/preferences.json`. The CLI resolves, validates, and
injects the compact result when an implementation or debug prompt needs it.

```json
{
  "framework_order": ["platformio", "arduino", "esp-idf", "rust"],
  "debug_tools": ["serial-mcp-server", "espflash", "binflow"],
  "code_limits": {
    "max_function_lines": 60,
    "max_file_lines": 500,
    "max_nesting": 3
  },
  "hardware_safety": {
    "prefer_dry_run": true,
    "require_explicit_flash": true
  }
}
```

Do not put ports, Wi-Fi values, OTA hosts, credentials, raw logs, or local
evidence paths in preferences. Those belong in ignored local state such as
`.lilygo-skills/local.json`.

References tell the agent what source material to read when a task needs more
context:

```bash
lilygo-skills reference list --json
lilygo-skills reference list --project /path/to/firmware --json
```

References are usually official examples, source files, datasheets, hardware
notes, or project-local design docs. For example, a user can say:

```text
Use the LilyGoLib factory example as the display and peripheral bring-up reference for this repo.
```

After confirmation, the agent writes `.lilygo-skills/references.json`. It
should add an explanation, not just a URL: `title`, `kind`, `applies_to`,
`authority`, `summary`, `read_when`, and `inject_triggers` tell future agents
what the source is for and when it should be loaded.

```json
{
  "schema_version": 1,
  "entries": [
    {
      "id": "project-lilygo-factory-example",
      "title": "LilyGoLib factory example",
      "kind": "official-example",
      "applies_to": ["display", "peripheral", "bring-up"],
      "path_or_url": "https://github.com/Xinyuan-LilyGO/LilyGoLib/blob/master/examples/factory/factory.ino",
      "authority": "source-navigation",
      "summary": "Read as an official example before changing board display or peripheral bring-up code.",
      "read_when": "User asks to implement or debug display, sensor, radio, or board bring-up behavior.",
      "inject_triggers": ["display", "sensor", "peripheral", "bring-up", "factory"]
    }
  ]
}
```

`serial-mcp-server` is a good preference example because it is a preferred debug
tool. It can also appear in the built-in tool reference catalog, but project
references should normally point to code, official examples, board docs,
datasheets, or project design notes. Preferences do not force references to load
first. The prompt, project context, route result, and goal type are resolved
together; `goal plan` then injects only the relevant compact `preferences` and
`reference_hints`. Source-completeness and board facts still have higher
priority: if a board/topic is missing required facts, the capsule should first
surface `needs_source_ingestion` rather than pretending a reference link is
enough.

The built-in reference catalog contains only public URLs (official docs,
tool references), so a fresh public clone resolves every entry;
`reference list --json` reports each entry's source health.

OTA, LVGL, and LoRa are not committed board peripheral skills. They are
source-backed recipe packs in `data/recipes/recipes.json`. Each recipe source
pack cites official upstream docs (Espressif OTA docs, LVGL docs and examples,
RadioLib plus LilyGO LoRa examples). `goal plan` surfaces the recipe ids,
source-pack ids, and official refs, so the agent reads authoritative sources
first.

Generated playbooks are the operating-pattern layer above recipes and source
facts. They are built from `data/playbooks/playbooks.json` and generated into
the runtime like other deep skills. A playbook never creates board facts or
claims hardware success; it tells the agent what sources to read, what failure
axes to check, what evidence is required, and what not to claim from context
alone.

## Updates And Source Refresh

Users can ask naturally:

```text
Update the LilyGO Skill sources for this board and check whether display facts are complete.
```

Behind the scenes, maintainers or agents use dry-run commands first:

```bash
lilygo-skills update sources --dry-run --json
lilygo-skills update boards --dry-run --json
lilygo-skills update skills --dry-run --json
lilygo-skills update source-packs --dry-run --json
lilygo-skills update peripheral-skills --dry-run --json
lilygo-skills update fact-packs --dry-run --json
lilygo-skills update board-facts --board <board-id> --topic <topic> --dry-run --json
lilygo-skills verify --json
lilygo-skills benchmark --json --iterations 5000
```

Because skills are generated rather than committed, a generated cache can be
produced and checked directly:

```bash
lilygo-skills generate skills --out <dir> --json
lilygo-skills verify --generated-root <dir> --json
lilygo-skills benchmark --generated-root <dir> --json --iterations 5000
```

`generate skills` writes every runtime skill into a generated cache (never into
the source tree) and reports `skill_count`, `source_pack_ids`, `source_hashes`,
`warnings`, and `verification_hints`. `update skills` and
`update peripheral-skills` are compatibility wrappers around the same generated
cache path; by default they use `.lilygo-skills/generated-skills/`, and `--out`
can point at another generated root. `verify --generated-root` checks that
generated cache in both directions: registry/index consistency, every routed
skill present, no unregistered generated skill, required reference skills
present, and honest evidence-boundary language. `benchmark --generated-root`
benchmarks routing over that generated skill set.

`benchmark` is built into this project and into the installed `lilygo-skills`
binary. It is a routing and injection quality gate: route fixtures, negative
over-injection cases, registered skill coverage, goal capsules, and goal
complete state coverage must pass.
It is not a hardware performance benchmark. Normal users do not need to run it
for every prompt; agents and maintainers run it after source, skill, router, or
goal changes and before publishing an updated Skill.

Remove `--dry-run` only when the planned reads/writes are correct. Route, hook,
and goal planning never mutate source data by themselves.

## Direct Commands For Agents

The CLI is the implementation layer behind the Skill. Common agent commands:

```bash
lilygo-skills route --json "<prompt>"
lilygo-skills goal complete --dry-run --json "<prompt>"
lilygo-skills goal plan --json "<prompt>"
lilygo-skills setup plan --framework platformio --json
lilygo-skills source query --board <board-id> --topic io --json
lilygo-skills source completeness --board <board-id> --topic display --json
lilygo-skills reference list --json
lilygo-skills preference show --json
lilygo-skills index query playbook-lvgl-debug --json
lilygo-skills generate skills --out <dir> --json
```

From a source checkout, the equivalent development form is:

```bash
cargo run -p lilygo-skills-cli -- <command>
```

This README is the primary usage guide.
