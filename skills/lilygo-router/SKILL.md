---
name: lilygo-skills
description: "LilyGO board development context router. Use for any prompt about LilyGO boards (T-Display, T-Watch, T-Beam, T-Deck, T-Echo, T-SIM and other LilyGO products), firmware, Arduino/PlatformIO/ESP-IDF/Rust builds, flashing, serial monitor, LVGL display/touch UI, OTA updates, LoRa/GNSS radios, IMU sensors, battery/power management, and board pinouts. Also handles Chinese prompts about 烧录, 显示, 固件, 抬腕检测."
---

# LilyGO Router

Use this as the top-level embedded-development Skill for LilyGO board prompts.
Generated board/peripheral/framework skills are context supplements; this meta
Skill owns the operating behavior: classify the task, find authoritative
sources, plan the debug loop, run only approved local commands, classify
failures, and record evidence.

- Route by board, framework, peripheral, and intent before adding details.
- Current verified runtime coverage starts with LilyGO products in the ESP32 family.
- Keep claims at `context-injection` or the reported evidence level; do not imply flash, OTA, serial, LVGL, or peripheral hardware success from source links.
- Prefer official LilyGO Wiki/GitHub, Espressif, and LVGL references when source lookup is requested.
- If the prompt is not about LilyGO or a seeded LilyGO board, return no deep context.

## Agent Operating Model

The injected context is a routing map, not the whole answer. Generated skills
provide compact board, peripheral, framework, chip, app, and recipe context.
They do not replace source reading, build/flash/serial evidence, or the debug
loop. When the user asks for a feature, implementation, debug session, demo, or
setup path, first classify:

1. LilyGO product and MCU or board family.
2. Framework/toolchain: Arduino, PlatformIO, ESP-IDF, or Rust esp-rs.
3. Peripheral or application domain: display, touch, sensor, radio, power,
   storage, OTA, LVGL, serial, simulator, or another routed feature.
4. Evidence target: source guidance, build, flash, serial capture, simulator, or
   real-board behavior.

For source-dependent work, read authoritative sources before writing precise
code. Start with official product repositories, headers, examples, and hardware
docs; then use `https://github.com/Xinyuan-LilyGO/documentation` as the
versioned wiki source; then chip-vendor datasheets and official framework docs
such as Espressif, LVGL, RadioLib, Arduino CLI, PlatformIO, and esp-rs. Project
references are useful operating patterns, but they do not outrank official
board facts, headers, examples, or datasheets.

If exact pins, buses, expander channels, libraries, demo paths, or setup steps
are not already present, run the discovery commands below or ask a narrow
clarification for missing board/framework/private details. Do not guess.

## Discovery Protocol

When a user asks to implement or debug a feature, keep the default surface to
four operations. The installed hook performs capsule auto-injection; do not
re-run maintainer commands unless the capsule asks for an expansion.

- Capsule auto-injection: let the installed hook decide no-op vs LilyGO context.
- `lilygo-skills source query --board <board-id> --topic <topic> --json`
  expands source-backed pins, buses, expanders, connectors, demos, and
  peripheral facts before writing precise firmware.
- `lilygo-skills goal complete --dry-run --json "<prompt>"` and
  `lilygo-skills goal plan --json "<prompt>"` decide completion state, missing
  inputs, setup readiness, permission needs, demos, and evidence steps for
  implementation or debug prompts.
- `lilygo-skills update board-facts --board <board-id> --topic <topic> --dry-run --json`
  previews official-source enrichment when a topic reports missing facts.

If the user provides a public reference source, write a structured project
reference with an AI-added explanation (`summary`, `read_when`, and
`inject_triggers` at minimum). Do not store naked links, private paths, local
logs, or credentials in references.

Setup planning is not automatic installation. The Skill installer does not
install Rust, Node, Arduino CLI, PlatformIO, ESP-IDF, esp-rs, board cores,
firmware libraries, or LoRa/GNSS dependencies. Use the setup readiness returned
by `goal complete` or `goal plan` as checks and install hints, then run real
installers only when the user explicitly asks.

If a fact is missing or ambiguous, report `unknown_with_sources` or ask a
structured clarification. Peripherals are board facts first: do not guess free
GPIOs, expander channels, buses, power rails, display chips, sensors, radio
chips, or hardware behavior from board names alone.
If a topic reports `needs_source_ingestion`, run or recommend the returned
dry-run enrichment command before writing source-precise implementation steps.
If a build or implementation task needs a framework and none is known, ask the
framework clarification; do not silently pick Arduino, PlatformIO, ESP-IDF, or
Rust esp-rs. First use of a board should select existing generated layers from
the installed registry, not mutate files or generate new skills from the hook.

Meta-only release: the public source tree commits only this router Skill.
Board, series, framework, tool, peripheral, chip, feature, debug, and app/recipe
skills are generated from the source model in `data/**`, not committed. They are
materialized only by explicit commands, written to an install root, project
cache, or test output directory — never by route or hook:

If a generated skill is missing at runtime, report it and follow the compact
repair command returned by the capsule; do not fetch sources or write skills
implicitly from this router text.

## Static Expansion References

This source tree also ships non-Skill reference docs under `../references/`.
Use them as expansion material when the task needs the full operating context,
not as default prompt payload:

- `../references/context-injection.md`
- `../references/source-discovery.md`
- `../references/build-flash-serial.md`
- `../references/lvgl-context.md`
- `../references/ota-context.md`
- `../references/bsp-driver-context.md`
- `../references/radio-gnss-context.md`
- `../references/project-preferences-references.md`
- `../references/generation-contract.md`

Generated Skill shapes are defined by public templates under
`../../templates/skills/`. Maintainer commands for regenerating and verifying
those files live in the repository docs, not in this AI-facing router.

## Generated Playbooks

Generated playbooks are compact operating-pattern context, not extra board
facts. Use the playbook hints already surfaced by `goal complete` or `goal plan`
when the user asks to implement, debug, set up, build, flash, monitor serial
output, diagnose LVGL, inspect OTA, write a BSP driver, or work with radio/GNSS.
Do not load playbook detail for unrelated chat or pure fact lookups unless the
user asks for a workflow.

Common playbook ids:

- `playbook-source-discovery`: read official code, examples, docs, datasheets,
  framework docs, and project references before precise implementation.
- `playbook-setup-toolchain`: plan Git/Rust/Node/framework/toolchain readiness
  without silently installing anything.
- `playbook-build-flash-serial`: bounded build, upload, monitor, and log
  classification with explicit permissions.
- `playbook-lvgl-debug`: LVGL display/touch/tick/flush/page-data diagnosis.
- `playbook-ota-debug`: partition, manifest, digest, reboot, rollback, and
  private local runner guidance.
- `playbook-bsp-driver`: board-fact-first driver capability/status/action/smoke
  pattern.
- `playbook-radio-gnss`: LoRa/GNSS source navigation and RF/GNSS evidence
  boundaries.

Playbooks never override board facts or source-completeness. If a board/topic
is incomplete, surface `needs_source_ingestion`, official refs, and an update
dry-run before giving runnable implementation details.

Keep injected context small. Inline matched skill IDs, short summaries,
top-ranked facts, overflow counts, and lookup commands; do not paste full fact
packs, source files, or reference docs unless the user explicitly asks for that
content.

For closed-loop debug, use `goal complete` first and `goal plan` when the
completion capsule says the request is ready to plan. Keep source/context
answers lightweight, then move to build, flash, serial, simulator, network, or
OTA evidence only when the task actually needs execution. If runtime
observation has no target output, add explicit firmware boot/status markers or
choose a smaller observable demo before rerunning; repeated identical failures
should route to problem-solving instead of blind retry.

OTA is a project workflow, not a generic command. When the user asks for OTA,
inspect the firmware project, manifests, build scripts, references, and ignored
local state to resolve the project OTA runner. If the project already has
private runner argv in `.lilygo-skills/local.json`, use it during execution. If
the runner is missing, derive one from real project artifacts or ask only for
the private endpoint, credential, or transport detail that cannot be inferred.
Keep Wi-Fi values, hosts, private auth values, ports, and raw OTA logs in local evidence,
rather than public context.
