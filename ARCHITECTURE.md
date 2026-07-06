# Architecture

Chinese version: [ARCHITECTURE.zh-CN.md](ARCHITECTURE.zh-CN.md). Related
docs: [Context layers](docs/CONTEXT_LAYER.md) /
[中文](docs/CONTEXT_LAYER.zh-CN.md), [Skill generation](docs/SKILL_GENERATION.md)
/ [中文](docs/SKILL_GENERATION.zh-CN.md), [Board facts](docs/BOARD_FACTS.md) /
[中文](docs/BOARD_FACTS.zh-CN.md), [Source recovery](docs/SOURCE_RECOVERY.md) /
[中文](docs/SOURCE_RECOVERY.zh-CN.md), [Action routing](docs/ACTION_ROUTING.md) /
[中文](docs/ACTION_ROUTING.zh-CN.md), and
[Verification levels](docs/VERIFICATION_LEVELS.md) /
[中文](docs/VERIFICATION_LEVELS.zh-CN.md).

lilygo-skills is a Rust CLI plus installed Skill runtime for LilyGO development
agents. It turns a natural-language board task into a compact context package,
then exposes deterministic commands for source lookup, runtime skill generation,
setup planning, and permissioned evidence collection.

The runtime separates context readiness from hardware verification. Route,
source, generation, install, and benchmark checks prove that the agent received
the right context and can reproduce the runtime; build, simulator, flash,
serial, OTA, display, RF, and peripheral behavior are verified only by their own
V4/V5 evidence artifacts.

The architecture is board-family extensible. Current verified runtime coverage
starts with LilyGO products in the ESP32 family: ESP32, ESP32-S2, ESP32-S3,
ESP32-C3, and ESP32-P4.

## Runtime Surfaces

```text
User prompt
  -> route/project/profile resolver
  -> matched skill ids and compact summaries
  -> optional goal complete / goal plan
  -> optional source query / source completeness / enrichment dry-run
  -> permissioned goal start evidence, only when explicitly allowed
```

Main surfaces:

- `route --json <prompt>`: choose board, framework, peripheral, feature, app,
  and tool skills.
- `hook codex|claude`: installed-runtime context envelope for AI clients.
- `project init/show/clear`: per-firmware board/framework defaults.
- `goal complete/plan/start/status/evidence/cancel`: completion state, debug
  and implementation capsules, plus safe evidence execution.
- `source query`: source-backed IO, pinout, bus, expander, connector, and
  peripheral facts.
- `source completeness`: topic readiness gates such as display/LVGL quick-start.
- `update board-facts`: explicit enrichment surface for one board/topic.
- `setup plan`: read-only toolchain setup planning for blank machines.
- `verify` and `benchmark`: integrity and route quality gates.
- `doctor --json`: installed runtime and injection-chain health check.

- `generate skills --out <dir>`: generate every runtime skill into a generated
  cache; reports skill count, source-pack ids, source hashes, warnings, and
  verification hints.

Install and setup are intentionally separate. `install.js --build` can compile
the Rust CLI from the current checkout, then installs the Skill runtime from
that binary and the source model. Rather than copying a committed skill
snapshot, it generates the runtime skills into the install root by invoking the
CLI's `generate skills`. It does not install host toolchains or firmware
dependencies. `setup plan` reports readiness checks and install hints with no
mutation; an agent may later run tool installers only as an explicit
user-approved step outside the setup-plan command.

## Layer Model

| Layer | Purpose | Default injection |
|-------|---------|-------------------|
| L0 | Router, hook, verify, benchmark | Decision, matched ids, reasons |
| L1 | Board/product/MCU-series skills | Board summary, source pointers |
| L2 | Peripheral/chip/feature skills | Relevant chip or feature context |
| L3 | Framework skills | Arduino, ESP-IDF, Rust esp-rs, PlatformIO, LVGL |
| L4 | Recipe/evidence context | OTA, watch UI, flash, serial, simulator |
| L5 | Project-local context | `.lilygo-skills/project.json` defaults and clarification |
| L6 | Goal planner | Recipes, permissions, artifacts, evidence boundary |
| L7 | Source facts and preferences | Compact fact tables, lookup commands, read hints |
| L8 | Source completeness | Topic status and enrichment next actions |
| L9 | Embedded playbooks | Source-first operating patterns and evidence checklists |
| L10 | Completion coordinator | Route, generated-root, source, setup, permissions, and evidence state |
| L11 | Action routing | Intent-ranked demos, generic bus lookup, project custom skills, and doctor |
| L12 | Experience patch | Context budgets, goal bridge, active doctor wiring, starter board refs |
| L13 | Intent/session evidence loop | Lookup/action split, session incremental context, runtime parity, hardware harness |

The committed router Skill is the embedded-development control plane. It tells
the agent how to classify a LilyGO task, read official sources, plan a bounded
debug loop, request permissions, run local commands only when approved, classify
failures, and record V3/V4/V5 evidence. Generated skills are the data plane:
short board, peripheral, framework, chip, app, and recipe context that helps the
meta Skill choose the right source lookup or command.

The important design rule is progressive disclosure. Route and hook output stay
small. Full source files, fact packs, and reference documents stay behind
commands that the AI can call when the user actually needs them. Embedded
playbooks follow the same rule: route and hook inject ids and short hints;
`index query playbook-* --json` expands the full generated playbook only for
implementation, setup, debug, or evidence work.

Source recovery is a cross-layer output rather than a separate command family.
For implementation and debug prompts, `goal plan`, hook context, `source query`,
and generated board skills converge on the same official-demo-first context:
demo path, board-owned headers, critical facts, and recovery commands.

Action routing is the next cross-layer output. It does not expand default
context; it adds compact `next_actions` to implementation/debug capsules:
minimal official demo selection, IO/bus source-query commands, project-local
custom skill hints, and permission-marked build/flash/serial/network/OTA paths.
Pure fact lookup remains compact and read-only.

The experience patch makes compactness explicit. Lookup capsules must avoid
demos, recipes, and mutation-oriented actions. Implementation/debug capsules
add a `goal-plan-bridge` and selected expansion commands so the agent sees the
next path without receiving every fact pack. Repeated board/topic content is
deduped into incremental hints, and `doctor --json` checks active installed
Codex/Claude wiring by default.

The intent/session evidence loop tightens that behavior. Pure lookup prompts in
English and Chinese stay read-only. Mixed prompts prefer implementation/debug
when the user asks the agent to act, while still keeping source-query expansion
visible. Hook context can use a session-scoped cache to emit a compact
incremental capsule on repeated same-board/topic prompts; critical pins,
evidence boundaries, and expansion commands are never dropped. `doctor --json`
also compares Codex and Claude runtime mirrors when both exist and reports
drift as a warning with a reinstall command. The live hardware harness is a
repeatable evidence path: it defaults to dry-run or boundary results until the
user grants the required build, flash, serial, network, or OTA permissions.

The public source tree is meta-only. The only committed Skill is
`skills/lilygo-router/SKILL.md`, the meta router. Board, series, framework,
tool, peripheral, chip, feature, debug, and app skills are not committed; they
are generated on demand from the source model. Source truth lives in `data/`,
`index/`, and official references; the runtime skills are a generated artifact,
never a hand-edited one. See the meta-only release boundary section below for
how generation and installation relate.

## Architectural Boundaries

These boundaries keep the runtime useful without making generated context noisy
or over-confident:

- **Completion coordinator**: `goal complete` composes route, project, generated
  root, source facts, setup, permissions, goal execution, and evidence into one
  state machine. It may summarize source-backed facts and demo refs that do not
  have a formal completeness topic, but it must not turn context into a hardware
  success claim.
- **Route token model**: routes use explicit tokens and data-backed triggers.
  Concrete chip part numbers such as `sx1262` are valid triggers; unsafe prefix
  or substring matching is not, because it reopens false-positive bugs such as
  `pio` inside `GPIO`.
- **Generated chip taxonomy**: chip skills represent real chip identifiers.
  Composite labels, memory capacities, storage media, and option strings remain
  source facts under board/peripheral context instead of becoming chip routes.
- **Runtime materialization**: `install.js` owns the runtime root and mirrors
  generated/source data there. User-owned host files such as Claude
  `settings.json` and Codex `AGENTS.md` are merge-only and marker-scoped.
- **Gate inventory**: every deterministic smoke that protects a release
  boundary must be visible in `scripts/ci-gate.sh`, even when another smoke also
  runs it transitively.
- **Context budget**: route, hook, and goal capsules are bounded. When detail is
  omitted, the output must keep a stable expansion command such as `source
  query`, `index query`, generated skill reads, or `goal plan`.
- **Intent-shaped actions**: lookup wording suppresses demos, recipes, goal
  bridges, build, flash, serial, network, and OTA actions. Implementation and
  debug wording may expose those paths only as compact, permission-labelled
  next actions.
- **Session cache safety**: incremental hook context requires a stable session
  id, TTL, runtime-version invalidation, and a kill switch. It is a token
  reduction path, not a source of board truth.
- **Split-host parity**: Codex and Claude installed runtimes are allowed to be
  installed independently, but `doctor` must make drift visible when both
  mirrors exist.

## Meta-Only Release Boundary And Generation Pipeline

The public source tree ships meta-only. The single committed Skill is
`skills/lilygo-router/SKILL.md`, the meta router that agents load. Every other
runtime skill (board, series, framework, tool, peripheral, chip, feature,
debug, app) is a generated artifact produced from the source model, not a
committed file.

The generation pipeline composes the runtime skill set from source inputs:

```text
source packs (data/boards.json, data/peripherals/**)
  + fact packs (data/facts/**)
  + route rules (index/routes.json, data/router/derived-context.json)
  + recipe packs (data/recipes/recipes.json)
  + playbook packs (data/playbooks/playbooks.json)
  + reference practice skills (data/skills/reference/**)
  + static references (skills/references/**)
  + generation templates (templates/skills/**)
  -> generate skills --out <dir>
  -> generated cache (install root, project cache, or test output dir)
  -> installer materializes the runtime root by mirror-copying owned data
```

`generate skills --out <dir> --json` writes the generated cache and reports
`skill_count`, `source_pack_ids`, `source_hashes`, `warnings`, and
`verification_hints`. It never writes into the source tree. `install.js`
generates into `~/.codex/lilygo-skills/` and `~/.claude/lilygo-skills/` at
install time by invoking that command; `--all --dry-run` reports a
`generate_plans` plan plus planned writes without producing skills.

The generated output can be validated directly, decoupled from install:

- `verify --generated-root <dir> --json` checks registry/index consistency,
  that every routed skill is present, that required reference skills are
  present, and that evidence-boundary language stays honest.
- `benchmark --generated-root <dir> --json --iterations <n>` benchmarks routing
  over that generated skill set.

Generated roots also carry support files:

- `skills/references/**`: static expansion docs for context injection, source
  discovery, build/flash/serial, LVGL, OTA, BSP, radio/GNSS, preferences, and
  generation.
- `templates/skills/**`: the public markdown templates used by the CLI to
  render generated board, peripheral/chip/feature, and playbook Skill files.

Route and hook stay no-write. If a routed generated skill is missing, they may
report it and include a compact generate/update command, but they never write
skills implicitly and never fetch network sources. Only explicit install,
update, project-init, and generate commands write generated skills, and only to
an install root, a project cache, or a test output directory.

The installer treats runtime-owned directories (`data/`, generated skills,
static references, templates, and source-intake product data) as mirrors of the
current checkout. This prevents stale files from previous versions from
surviving inside installed runtimes. Host integration files remain user-owned
and are updated only through bounded merge logic.

Domain catalogs live in `data/` where practical: route triggers
(`data/router/derived-context.json`), recipes (`data/recipes/recipes.json`),
reference entries (`data/references/built-in.json`), fact and topic keyword
rules (`data/facts/*.json`), reference practice skills
(`data/skills/reference/*.md`), and CLI help text (`data/help/*.txt`). The Rust
CLI keeps parsing, routing, generation, install, privacy, and goal policy.

On the Claude Code host, injection rides the `UserPromptSubmit` hook: the
installed binary reads the prompt JSON on stdin and emits
`{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"..."}}`
on stdout (envelope or nothing; diagnostics on stderr; fail-open exit 0). The
Codex host consumes the legacy diagnostic envelope via `hook codex` from the
marked `AGENTS.md` section.

OTA, LVGL, and LoRa are source-backed recipe packs in
`data/recipes/recipes.json`, not committed board peripheral skills. Each recipe
source pack cites official upstream docs (Espressif OTA docs, LVGL docs and
examples, RadioLib plus LilyGO LoRa examples). The built-in reference catalog
contains only public URLs, so a fresh public clone resolves every entry;
`reference list --json` reports each entry's source health.

## Embedded Playbooks

Playbooks are generated runtime skills backed by `data/playbooks/playbooks.json`.
They are operating patterns, not board facts. A playbook can tell the agent to
read official examples, compare headers, check setup readiness, run a bounded
build/flash/serial loop, diagnose LVGL, inspect OTA manifests, wrap a BSP
driver, or classify RF/GNSS evidence. It cannot create missing pin, bus, chip,
power-rail, demo, or framework facts.

Each playbook source entry carries:

- source refs with official or project authority;
- required board facts;
- diagnostic axes;
- ordered steps;
- failure classes;
- evidence targets;
- anti-claims that state what context alone cannot prove;
- resource hints and benchmark prompts.

Runtime selection is compact. `route` and installed hooks may include
`playbook-*` ids when the prompt asks for implementation, setup, debug, flash,
serial, LVGL, OTA, BSP, radio, GNSS, or source discovery. `goal plan` includes
small `playbook_hints` with evidence targets and expansion commands. Full
playbook bodies are read through `index query <playbook-id> --json` or from the
generated runtime Skill file.

Playbook priority is lower than source facts and source-completeness status. If
a board/topic is missing facts, the correct route is `needs_source_ingestion`
with official refs and an update dry-run, not a generic playbook answer.

### Release QA And Verification Boundary

Release QA covers the source model, generation pipeline, install/runtime parity,
route and benchmark coverage, privacy checks, and evidence-level enforcement.
Those checks are enough to publish the Skill runtime because they prove the
context layer is reproducible.

Hardware workflows have a separate evidence track. OTA transport, LVGL pixels,
LoRa RF, flash success, serial application logs, simulator output, and physical
peripheral behavior move from "context available" to "verified" only when the
corresponding V4/V5 artifact exists.

## Source Authority

Source authority is ordered, not flattened:

1. Official code, headers, examples, manifests, and board repositories.
2. Official LilyGO hardware documentation.
3. `https://github.com/Xinyuan-LilyGO/documentation`, the versioned source
   behind LilyGO wiki content.
4. `wiki.lilygo.cc` fallback pages.
5. Project reference skills, used as implementation/debug patterns.
6. Community or auxiliary tooling references, used only as hints.

Reference hints tell the AI what to read. They do not override source facts.
When the runtime cannot prove an exact value, it returns `unknown_with_sources`
or `needs_source_ingestion` rather than guessing.

## Board And Project Identity

Board identity can come from:

1. Explicit prompt text.
2. Project-local `.lilygo-skills/project.json`.
3. Global active profile.
4. Structured clarification.
5. No-op when the prompt is unrelated.

Prompt facts always win. Project context exists so different firmware
directories can carry different defaults without changing the global skill
registry.

Framework identity follows the same rule. Explicit framework text or project
context selects Arduino, PlatformIO, ESP-IDF, or Rust esp-rs. If an
implementation/build prompt requires a framework and none is known, the route
decision becomes `needs_clarification`. Context-only lookups may keep the
framework unspecified until the task actually needs a toolchain.

Committed project file:

```text
.lilygo-skills/project.json
```

Private or future machine-local evidence:

```text
.lilygo-skills/local.json
.lilygo-skills/evidence/
```

Private state must not be committed or injected into public prompt context.
Project OTA execution is private local state too. The agent resolves the OTA
runner from project manifests, scripts, references, and ignored local settings.
When concrete private commands are needed, they are stored as
`ota_manifest_argv` and `ota_observe_argv` arrays in `.lilygo-skills/local.json`.
The goal runner redacts private OTA output from public JSON.

## Preferences And References

Preferences are public behavior policy, not hardware facts and not private
machine state. Resolution order is built-in defaults plus project-local
`.lilygo-skills/preferences.json` when present. The resolved shape contains:

- `framework_order`: preferred framework order for ambiguous setup and planning.
- `debug_tools`: public tool preferences such as `serial-mcp-server`,
  `espflash`, or `binflow`.
- `code_limits`: code size and nesting limits for generated firmware changes.
- `hardware_safety`: dry-run and explicit-flash defaults.

Write path:

```text
user preference request
  -> agent confirms public scope
  -> agent writes .lilygo-skills/preferences.json
  -> CLI validates and resolves preferences
  -> goal capsule injects compact preferences when relevant
```

Preference values are validated before injection. Private-shaped values such as
serial ports, local paths, LAN hosts, credentials, OTA hosts, raw logs, and
evidence paths are rejected or kept out of public context. Preferences guide
tool/style/safety behavior only; source facts remain authoritative.

References are read hints for source material: official examples, source files,
hardware notes, datasheets, project design docs, or operating patterns. A
reference entry can be missing locally; that is source-health context, not
permission to invent facts. References are loaded only when the route, goal, or
prompt needs them, and they never outrank official code, headers, examples, or
source-backed board facts.

Project references use `.lilygo-skills/references.json` with `schema_version`
and `entries`. A user can ask the agent to add a public reference such as an
official LilyGO example, a datasheet, or a project design note. The agent
confirms the source is public, writes a project reference entry with an AI-added
explanation, and the CLI merges it with the built-in catalog, dedupes by id,
validates privacy and authority, then exposes compact `reference_hints` for
implementation/debug prompts. A project reference entry should explain the
source through `title`, `kind`, `applies_to`, `authority`, `summary`,
`read_when`, and `inject_triggers`; naked URLs are not enough for future agents.
Tool choices such as "use serial-mcp-server for serial debugging" are
preferences first; they become references only when the task needs the tool's
documentation or operating pattern.

Injection is deliberately bounded. Route and hook output identify the selected
skills, readiness, and clarification state. `goal plan` is the surface that
adds compact `preferences` and `reference_hints`; preference hints are capped,
reference hints are capped, and full reference bodies stay in files or URLs for
the agent to read only when needed. Preferences do not force references to load
first; both are selected from prompt, project context, route, and goal type.
Fact-only prompts do not load preference or reference hints unless they affect
the requested action. Source-completeness status outranks reference hints: a
missing board/topic fact should produce `needs_source_ingestion` before any
reference is treated as implementation-ready context.

## Board Facts And Completeness

Board facts live in `data/facts/board-fact-packs.json`. Peripherals are part of
the board fact model first; reusable peripheral/chip layers help routing, but
they do not replace board-specific source facts. A pack can include:

- MCU family and supported framework facts.
- Pin, bus, expander, connector, power, display, radio, sensor, storage, input,
  and peripheral tables.
- Source refs with authority rank and hashes.
- Conflicts and `unknown_with_sources` entries.

Completeness is topic-specific. A board can be complete for IMU and incomplete
for display. `source completeness` evaluates a board/topic and returns:

- `complete`
- `partial`
- `needs_source_ingestion`
- `unsupported`

For incomplete supported topics, route/hook/goal may surface compact readiness
and update commands, but only `update board-facts` is allowed to write enriched
fact-pack data.

First use of a board selects already installed generated layers. Route and hook
do not fetch network sources, generate new skills, or mutate fact packs. Updates
and generation are explicit maintenance actions through the update flow below.

## Goal Planning And Evidence

`route` answers “what context should be loaded?”. `goal complete` answers
“which completion state blocks or permits this work?”. `goal plan` answers “how
should the AI proceed?” once the request is ready to plan. Setup is one of those
routed plans: a blank machine or missing framework toolchain should use
`setup plan` before the agent runs any installer.

`goal complete` is a bounded coordinator over existing layers. It can return
`no_op`, `needs_clarification`, `needs_generation`,
`needs_source_ingestion`, `needs_setup`, `needs_permission`, `planned`,
`complete`, `blocked`, or `failed`. It does not add a second command runner.
When permissions are explicit and the readiness gates pass, it delegates to
`goal start`; otherwise it returns next actions and writes nothing.

A goal plan can include:

- Main board and framework.
- Source-backed facts and official demo refs.
- Recipe steps such as build, upload, monitor, LVGL simulator, OTA, or serial.
- Required permissions.
- Planned artifacts.
- Evidence boundary.
- Discovery hints when facts are missing.

Goal execution should not constrain the agent's research path. It provides a
known-good execution skeleton only when the board profile has source-backed or
locally verified facts, such as an Arduino FQBN and required library roots. If
that profile is missing, commands stay as lookup placeholders and the agent is
expected to read official board, framework, project, and user reference sources
before filling in the runnable command.

`goal complete` and `goal start` are no-write by default. Real actions require
explicit flags such as `--allow-build`, `--allow-flash --port <port>`,
`--allow-serial --port <port>`, `--allow-network --allow-ota`, or
`--allow-simulator`.

## Verification Boundary

The current runtime is verified at V3 for source/context/completeness. That
means routing, hook output, source facts, completeness status, enrichment
dry-runs, benchmarks, and installed runtime parity have been tested.

It does not mean every board has been physically flashed or that every demo has
run on hardware. Build, simulator, flash, serial log, OTA, display pixels, and
peripheral behavior require V4/V5 evidence. See
[docs/VERIFICATION_LEVELS.md](docs/VERIFICATION_LEVELS.md).

## Benchmark Gate

Benchmarking is part of the Rust CLI, not a separate external project. The
source lives under `crates/lilygo-skills-cli/`, and the installed
`lilygo-skills` binary exposes the same `benchmark --json` command.

The benchmark checks:

- Every registered skill has at least one covered trigger path.
- Positive route fixtures still inject expected skills.
- Negative fixtures prevent short trigger and unrelated-skill over-injection.
- Goal capsules still include the expected compact context.
- Goal complete covers `no_op`, `needs_clarification`,
  `needs_source_ingestion`, and `needs_permission`.
- Baseline comparisons catch regressions in case count and skill coverage.

`scripts/full-evidence-smoke.sh --dry-run` runs a short benchmark as part of
the evidence pack. Release or publish checks run the longer form:

```bash
lilygo-skills benchmark --json --iterations 5000
```

This is still V3 context quality evidence. It does not prove build, flash,
serial, OTA transport, display pixels, or physical peripheral behavior.

## File Map

```text
crates/lilygo-skills-cli/     Rust CLI implementation
data/boards.json              Generated board/product source model
data/peripherals/             Peripheral/chip/feature source packs
data/facts/                   Board fact packs
data/recipes/                 Goal recipe source packs
data/playbooks/               Generated playbook source model
data/references/built-in.json Built-in reference catalog (public URLs)
data/references/source-intake Public source-intake cache and manifest
data/skills/reference/        Reference practice skills used in generation
index/routes.json             Skill registry and triggers
skills/lilygo-router/SKILL.md Committed meta router (only committed Skill)
generated runtime skills      Produced by `generate skills` into the install root
scripts/*smoke.sh             CLI verification smokes
docs/                         Human architecture and contributor docs
```

## Update Flow

```text
update sources
  -> update boards
  -> update skills (generated cache)
  -> update source-packs
  -> update peripheral-skills (generated cache)
  -> update fact-packs
  -> update board-facts --board <id> --topic <topic>
  -> verify
  -> benchmark
```

Dry-run mode must report planned reads and writes without mutation. Apply mode
must stay inside supported paths and preserve the current LilyGO support
boundary. Generated skill updates write only to `.lilygo-skills/generated-skills/`
or an explicit `--out <generated-root>`; they must never write generated
`SKILL.md` files into the committed source `skills/` tree.
