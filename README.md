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
thin Rust CLI. The `SKILL.md` is the *operating system*: it carries the query
protocol, the debug loop, and the honesty rules as prose. The CLI is the
deterministic **Context Kernel**: it decides whether a prompt is LilyGO work,
which board/framework are involved, and which source-backed facts are safe to
inject — then hands the AI a compact capsule and the exact `source query` command
to pull anything deeper. "Which sub-skills / playbooks are relevant" is chosen by
data-driven triggers; "is this a pin I may state" is enforced by the honesty
rules and the source model.

## Works on any agent — the hook is optional

- **Pure skill (any host).** `SKILL.md` + the `lilygo-skills` CLI is enough on
  Claude Code, Codex, or any agent. Run `context` to get the capsule and
  `source query` to pull exact pins. Nothing about the value depends on a hook.
- **Optional Claude Code hook.** A one-line `UserPromptSubmit` hook auto-injects
  the capsule so the user does not run `context` by hand. It is a *convenience*:
  on a host without it you run one extra command and lose nothing — the data and
  quality gates are identical across platforms.

## The CLI — five everyday commands

The everyday surface is small and stable:

```bash
lilygo-skills context [--project <dir>] --json "<prompt>"   # Context Kernel: CWD → board → capsule (≤~1KB)
lilygo-skills source query --board <board-id> --topic <topic> --json   # pull exact source-backed pins/buses
lilygo-skills route --json "<prompt>"                        # relevance gate + matched skill/playbook ids
lilygo-skills index list|query <id> --json                  # list registered skills/playbooks, expand one
lilygo-skills doctor --json                                  # health-check the injection chain
```

`context` is the one-shot entry: it auto-detects the board from the project
(reading `.lilygo-skills/project.json`, else sniffing `platformio.ini`,
`sdkconfig`, and `*.ino`) and returns the matched skill ids, top-ranked facts,
the verification level, and the follow-up lookup commands.

A handful of supporting commands cover install/health, setup, and project memory:
`verify`, `setup plan --framework <arduino|platformio|esp-idf|rust>`,
`preference show`, `reference list`, `project init|show|clear`,
`source completeness`, and `update board-facts` for source enrichment. There is no
`goal`, `benchmark`, or `generate` command — those layers were folded into the
`SKILL.md` prose and the data model.

## Data-driven selection

Recipes, playbooks, reference hints, and preference hints are **JSON data**, not
hand-written prose branches. Each has a `*-triggers.json` table
(`data/recipes/recipe-triggers.json`, `data/playbooks/playbook-triggers.json`,
`data/references/reference-triggers.json`, `data/preferences/...`), and a single
selection engine (`selection.rs`) matches the prompt against those triggers to
decide which compact ids to surface. Adding or tuning a recipe/playbook/reference
is a data edit, not new Rust. The reader code stays thin.

- **Recipes** (`data/recipes/recipes.json`) — OTA, LVGL, and LoRa are
  source-backed recipe packs citing official upstream docs (Espressif OTA, LVGL,
  RadioLib + LilyGO examples), not committed peripheral skills.
- **Playbooks** (`data/playbooks/playbooks.json`) — short operating guides for
  source discovery, setup, build/flash/serial, LVGL, BSP drivers, and radio/GNSS.
  The agent sees compact ids first and expands with
  `lilygo-skills index query <playbook-id> --json`.

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

- **Node.js** is required to run `install.js` and mount the Skill.
- **Rust/Cargo** is recommended for the full dynamic runtime. Without it the
  installer still mounts in **mount-only** mode; use `--build` to compile the CLI
  in the same step, or `--prebuilt-only` to install a packaged runtime with no
  Rust:

  ```bash
  node install.js --all --prebuilt-only && lilygo-skills doctor --json
  ```

The installer writes runtime roots under `~/.claude/lilygo-skills/` and
`~/.codex/lilygo-skills/`, installs the router Skill to
`~/.claude/skills/lilygo-skills/SKILL.md`, idempotently merges the optional
`UserPromptSubmit` hook into `~/.claude/settings.json`, and appends a marked
section to `~/.codex/AGENTS.md`. If `settings.json` is not valid JSON the
installer reports it and prints a manual snippet instead of touching the file.

Host toolchains (Arduino CLI, PlatformIO, ESP-IDF, esp-rs, board cores, serial
tools, radio/GNSS libs) stay explicit — they are handled through `setup plan` and
user-approved steps, never installed implicitly.

## Quality gates

Before publishing a runtime change, run the gates:

```bash
cargo test --workspace                              # 154 tests
node eval/coverage-gate.js                          # injected-capsule coverage >= baseline
node pipeline/verify-auto-mapping.js                # extracted pins == official macros
node pipeline/verify-source-authority.js            # every fact keeps a ranked official source
bash scripts/ci-gate.sh                             # 34 deterministic gates
cargo fmt --check ; cargo clippy --workspace --all-targets -- -D warnings
```

`coverage-gate.js` grades the **real injected capsule** the model receives for
every eval prompt against its expected facts (currently **55 of 62 facts covered,
88.7%, with all 20 honesty markers present**, against a ratcheted floor that may
only move up). This means trimming scaffolding can never silently regress the
facts the model actually sees. `ci-gate.sh` runs 34 deterministic checks
(byte-for-byte capsule fixtures, board triple-question tests, scorecard grading,
install/doctor smokes, and the source pipeline).

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

The public repository is the runtime source: CLI, installer, router Skill, source
model, data tables, references, schemas, and release gates. This README is the
primary usage guide.
