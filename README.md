# lilygo-skills

A **skill-first** context runtime for AI-assisted LilyGO board development.

Install it once into Claude Code, Codex, or any other agent. After that the user
describes firmware work in normal language and the agent loads the right board,
framework, source-backed pins, official examples, setup hints, and safe debug
steps — without hand-searching datasheets, and **without inventing pins**.

The design goal is deliberately narrow: **be correct about hardware facts and
honest about what has actually been verified.** Every pin, bus, and rail the
runtime states is traceable to an official upstream line (URL + line + sha256),
the data ships locally so it works offline, and machine-checkable honesty markers
(`hardware_verified`, `evidence_boundary`) keep source knowledge from being
mistaken for a working board.

## Architecture in one paragraph

The product is a small **meta Skill** (`skills/lilygo-router/SKILL.md`) plus a
thin **Node CLI** (the JS context kernel under `bin/`). The `SKILL.md` is the
*operating system*: it carries the query protocol, the debug loop, and the
honesty rules as prose. The CLI is the deterministic **Context Kernel**: it
decides whether a prompt is LilyGO work, which board is involved, and which
source-backed facts are safe to inject — then hands the AI a compact capsule and
the exact `source query` command to pull anything deeper. Board detection and
fact selection are data-driven (they read `data/**`, never inline a pin); "is
this a pin I may state" is enforced by the honesty rules and the source model.
The kernel ships as `.mjs` and runs on the Node that every supported host
already has, so there is no compiler or binary to build.

## Works on any agent — the hook is optional

- **Pure skill (any host).** `SKILL.md` + the `lilygo-skills` CLI is enough on
  Claude Code, Codex, or any agent. Run `context` to get the capsule and
  `source query` to pull exact pins. Nothing about the value depends on a hook.
- **Optional Claude Code hook.** A one-line `UserPromptSubmit` hook auto-injects
  the capsule so the user does not run `context` by hand. It is a *convenience*:
  on a host without it you run one extra command and lose nothing — the data and
  quality gates are identical across platforms.

## The CLI — the everyday surface

The command surface is intentionally small and stable. The command name is
`lilygo-skills` (an installed shim that execs `node <root>/bin/lilygo-skills.mjs`):

```bash
lilygo-skills context [--project <dir>] --json "<prompt>"              # Context Kernel: CWD → board → capsule (≤~1KB)
lilygo-skills source query --board <board-id> --topic <topic> --json   # pull exact source-backed pins/buses
lilygo-skills verify sources --board <board-id> [--topic <t>] --json   # live re-proof (OK / DRIFT / UNREACHABLE)
lilygo-skills doctor --json                                            # data-integrity self-check
lilygo-skills hook <claude|codex>                                      # push the thick board capsule (stdin: {"prompt":..})
```

`context` is the one-shot entry: it auto-detects the board from the project
(reading `.lilygo-skills/project.json`, else sniffing `platformio.ini`,
`sdkconfig`, and `*.ino`) and returns the matched skill id, top-ranked facts, the
verification level, and the follow-up lookup commands. `hook` is the push
boundary the Claude Code hook calls to inline a board's critical pins before the
model answers.

The operating depth — the debug loop, framework setup, LVGL/LoRa/power triage,
and the honesty rules — lives as prose in `SKILL.md` and its guides, not as
extra subcommands. Board fact ingestion and provenance verification are handled
by the Node data pipeline (`pipeline/**`, `eval/**`), run at author time, not by
the everyday CLI.

## Data-driven, thin reader

Board detection and fact selection read committed JSON under `data/**` — board
sniff rules (`data/sniff-rules.json`), prompt keywords
(`data/facts/prompt-keywords.json`), topic fields (`data/facts/topic-fields.json`),
and the source-backed fact packs (`data/facts/board-fact-packs.json`). The reader
never inlines a pin: every value it returns comes from a fact pack that carries
its official URL, line range, and sha256. Adding or deepening a board is a data
edit through the ingest pipeline, not a code change.

## Guides — thin code, thick guidance

The behavioral depth lives as prose guides under `skills/lilygo-router/guides/`,
in the spirit of a `dev-flow` (thin code + thick guidance). `SKILL.md` is the
entry point and the `context`/`source query` expand pointers route into them:

| Guide | What it drives |
|-------|----------------|
| `query-protocol.md` | get `context` → auto board → pull `source query` before stating any pin |
| `board-bringup-checklist.md` | zero-to-working: identify board → find official source → run demo → capture evidence |
| `debug-flash-serial.md` | bounded build → upload → monitor, and the failure buckets |
| `debug-display-bringup.md` | ST7789 / TFT_eSPI Setup vs ESP-IDF i80, backlight and power rails |
| `debug-lvgl-loop.md` | LVGL tick / flush / draw-buffer / touch loop triage |
| `debug-lora-gnss.md` | SX126x / RadioLib + GNSS bring-up and failure triage |
| `debug-power-battery.md` | power rails, charging, and fuel-gauge checks |
| `toolchain-setup.md` | Arduino / PlatformIO / ESP-IDF / Rust esp-rs setup (report + hints) |
| `honesty-evidence.md` | evidence levels, `hardware_verified=false`, and the never-invent rule |

## Our edge over a generic LilyGO skill

- **Source-backed pins.** Every board fact carries its official URL, line number,
  and sha256, so a stated pin is traceable to an exact upstream line — not a
  value recalled from training data.
- **Offline and local.** The data ships with the CLI and answers in milliseconds;
  there is no hosted service to depend on, and it works with no network.
- **Automatic board detection.** `context` infers the board from project files,
  so the user does not name it every time.
- **Quality gates.** A ratcheted coverage baseline, source-authority and
  auto-mapping checks, and a deterministic CI gate guard the data.
- **Machine-checkable honesty.** `hardware_verified` and the `evidence_boundary`
  are emitted markers a gate can grade, not just prose promises.

## Effect (small pilot, stated honestly)

An early effect pilot (2026-07-07, recorded in `eval/fixtures/smoke-scorecard.json`)
ran a set of obscure LilyGO board pin/wiring/debug questions two ways: the **full
system** (injected capsule + the model running `source query` to pull) versus a
**bare model** with no skill.

- **Full system: 6/6 correct, 0 hallucination**, with full source citation.
- **Bare model: 4/6 on hit-only scoring, 2/6 on human review**, with three
  *confident* errors — e.g. swapping SDA/SCL, calling the 8-bit parallel display
  an SPI bus, and prescribing the wrong TFT_eSPI Setup file.

The value the pilot demonstrates is **correctness + verifiability + zero
fabrication**, especially on boards where a bare model is unsure but answers
anyway. Two honest caveats:

- This is a **small pilot**, not a large benchmark. Treat it as directional.
- The full system's strength comes from the *complete* loop (capsule **pushes**
  the critical subset, model **pulls** the rest via `source query`). A push-only
  read of an *incomplete* capsule can be worse than a bare model, because the
  model may anchor on the partial "verified facts" and infer the rest. The fix is
  a firm rule in `SKILL.md` and the guides: **the capsule is a pointer, not the
  full pinout — an absent pin means "go pull it", never "guess".**
- The **final judge is P4 real-hardware A/B**, which needs the user's credentials
  and is not simulated or faked here.

## Board coverage

The source model ships **26 board fact packs** (`data/facts/board-fact-packs.json`),
all with `fields_missing_source=0` in the official-source pipeline. Incomplete
topics honestly return `unknown_with_sources` or `needs_source_ingestion` rather
than inventing pins. Recently deepened boards include:

- **T-Display-S3** — the 8-bit parallel (i8080) display bus is modeled at pin
  granularity (`display.d0`–`display.d7`, `wr`, `rd`, `cs`, `dc`, `reset`,
  `backlight`), so the runtime knows those pins are occupied before suggesting
  anything else.
- **T-Deck** — LoRa (`lora.cs`/`busy`/`rst`/`dio1`), keyboard interrupt, trackball
  interrupt, shared SPI bus, SD card, and display pins from official `utilities.h`.
- **T-CameraPlus-S3** — the peripheral matrix is present but honestly reports
  `unknown_with_sources` for topics such as storage that still need official
  product-source inspection.

A recognized-but-out-of-scope LilyGO product (a non-ESP32 board such as an RP2040
product) emits an explicit support-boundary line instead of an empty capsule.
Plain non-LilyGO prompts inject nothing.

## Install

Give your agent the repo and ask it to install, or do it manually once Git and
Node.js are present:

```bash
git clone https://github.com/Xinyuan-LilyGO/lilygo-skills.git
cd lilygo-skills
node install.js --all --dry-run     # preview writes
node install.js --all               # install + self-test
lilygo-skills doctor --json         # confirm the injection chain
```

**Node.js** is the only prerequisite. The installer copies the Node dispatcher
(`bin/**`) and the data model (`data/**`) together as a self-contained runtime —
the data travels with the dispatcher, so a runtime update can never leave stale
data behind. There is nothing to compile.

It also places a `lilygo-skills` shim in `~/.local/bin`. If that directory is not
already on your `PATH`, the installer appends it to your shell rc (idempotently)
and prints the file to `source` — so both `lilygo-skills` and the model's
`source query` pull resolve in a fresh shell.

The installer writes runtime roots under `~/.claude/lilygo-skills/` and
`~/.codex/lilygo-skills/`, installs the router Skill to
`~/.claude/skills/lilygo-skills/SKILL.md`, idempotently merges the optional
`UserPromptSubmit` hook (`node <root>/bin/hook.mjs claude`) into
`~/.claude/settings.json`, and appends a marked section to `~/.codex/AGENTS.md`.
If `settings.json` is not valid JSON the installer reports it and prints a manual
snippet instead of touching the file.

Host toolchains (Arduino CLI, PlatformIO, ESP-IDF, esp-rs, board cores, serial
tools, radio/GNSS libs) stay explicit — they are reported with install hints and
run only through user-approved steps, never installed implicitly.

## Quality gates

Before publishing a runtime change, run the gates:

```bash
npm test                                            # unit + CLI contract + hook value-alignment + CJK routing
npx tsc -p tsconfig.json --noEmit                   # typecheck the JS kernel (strict)
node eval/coverage-gate.js                          # injected-capsule coverage >= baseline
node pipeline/verify-auto-mapping.js                # extracted pins == official macros
node pipeline/verify-source-authority.js            # every fact keeps a ranked official source
bash scripts/ci-gate.sh                             # aggregated deterministic gate
```

`coverage-gate.js` grades the **real injected capsule** the model receives for
every eval prompt against its expected facts, against a ratcheted floor that may
only move up — so trimming scaffolding can never silently regress the facts the
model actually sees. `ci-gate.sh` aggregates the JS core gates (`npm test`,
typecheck, `doctor`, live `verify sources`), the data/pipeline/provenance
checks, the board triple-question tests, scorecard grading, and the
install → hook integration smoke.

## Add a new board

Board facts are ingested from official LilyGO sources, never hand-typed:

1. Add an official source entry to `pipeline/source-manifest.json` (board id, raw
   URL of the official header/example, line range, topic, `authority_rank`, and
   the macro→fact-key mapping). Extend `pipeline/pin-naming-conventions.json` if
   the board uses macro names the shared table does not know yet.
2. Dry-run then write the ingest:

   ```bash
   node pipeline/ingest-from-manifest.js --board <board-id> --json
   node pipeline/ingest-from-manifest.js --board <board-id> --write
   ```

3. Keep the gates green (auto-mapping, source-authority, the all-boards pipeline,
   the coverage gate, and `scripts/ci-gate.sh`). New expected facts go in
   `eval/tasks.json`; ratchet the coverage baseline **up** (never down) with
   `node eval/coverage-gate.js --update-baseline`.

## Documentation

| Topic | English | 中文 |
|-------|---------|------|
| Overview | [README.md](README.md) | [README.zh-CN.md](README.zh-CN.md) |
| Architecture | [ARCHITECTURE.md](ARCHITECTURE.md) | [ARCHITECTURE.zh-CN.md](ARCHITECTURE.zh-CN.md) |
| Context layers | [docs/CONTEXT_LAYER.md](docs/CONTEXT_LAYER.md) | [docs/CONTEXT_LAYER.zh-CN.md](docs/CONTEXT_LAYER.zh-CN.md) |
| Skill generation | [docs/SKILL_GENERATION.md](docs/SKILL_GENERATION.md) | [docs/SKILL_GENERATION.zh-CN.md](docs/SKILL_GENERATION.zh-CN.md) |
| Board facts | [docs/BOARD_FACTS.md](docs/BOARD_FACTS.md) | [docs/BOARD_FACTS.zh-CN.md](docs/BOARD_FACTS.zh-CN.md) |
| Source recovery | [docs/SOURCE_RECOVERY.md](docs/SOURCE_RECOVERY.md) | [docs/SOURCE_RECOVERY.zh-CN.md](docs/SOURCE_RECOVERY.zh-CN.md) |
| Action routing | [docs/ACTION_ROUTING.md](docs/ACTION_ROUTING.md) | [docs/ACTION_ROUTING.zh-CN.md](docs/ACTION_ROUTING.zh-CN.md) |
| Verification levels | [docs/VERIFICATION_LEVELS.md](docs/VERIFICATION_LEVELS.md) | [docs/VERIFICATION_LEVELS.zh-CN.md](docs/VERIFICATION_LEVELS.zh-CN.md) |
| Board contribution | [docs/CONTRIBUTING_BOARDS.md](docs/CONTRIBUTING_BOARDS.md) | [docs/CONTRIBUTING_BOARDS.zh-CN.md](docs/CONTRIBUTING_BOARDS.zh-CN.md) |
| Router Skill & guides | [skills/lilygo-router/SKILL.md](skills/lilygo-router/SKILL.md) + [guides/](skills/lilygo-router/guides/) | (bilingual prose in-file) |
| Design & review records | [docs/design/](docs/design/) | (per-milestone) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) | — |
| Agent runtime notes | [AGENTS.md](AGENTS.md) · [CLAUDE.md](CLAUDE.md) | — |

The public repository is the runtime source: CLI, installer, router Skill, source
model, data tables, references, schemas, and release gates. This README is the
primary usage guide.
