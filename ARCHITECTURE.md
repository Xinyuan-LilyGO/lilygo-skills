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

lilygo-skills is a meta Skill plus a thin **Node context kernel** for LilyGO
development agents. It turns a natural-language board task into a compact,
source-backed context capsule, then exposes deterministic commands to pull exact
pins and re-prove them against upstream.

The runtime separates **context readiness** from **hardware verification**. The
kernel proves that the agent received the right board facts, each traceable to an
official upstream line; whether the user's firmware actually builds, flashes,
renders pixels, or acquires an RF/GNSS fix is a separate evidence track the kernel
never claims.

## Two parts

- **The meta Skill** — `skills/lilygo-router/SKILL.md`, the single committed
  Skill. It is the operating document: the query protocol, the debug loop, and
  the honesty rules as prose, with focused how-to guides under
  `skills/lilygo-router/guides/`. It owns the behavior an agent follows.
- **The Node context kernel** — the JS thin core under `bin/`. It reads the
  committed data model (`data/**`) and answers a small, stable set of commands.
  It ships as `.mjs` and runs on the Node every supported host already has; the
  installer copies `bin/**` and `data/**` together as one self-contained runtime,
  so the data always travels with the reader.

The kernel never inlines a hardware value. Every pin, bus, and rail it returns
comes from a committed fact pack that carries an official URL, line range, and
sha256.

## Command surface

```text
lilygo-skills context [--project <dir>] --json "<prompt>"              CWD -> board -> capsule
lilygo-skills source query --board <id> --topic <topic> --json         source-cited facts for a topic
lilygo-skills verify sources --board <id> [--topic <t>] --json         live re-proof (OK / DRIFT / UNREACHABLE)
lilygo-skills doctor --json                                            data-integrity self-check
lilygo-skills hook <claude|codex>                                      push the thick board capsule
```

- **`context`** decides whether a prompt is LilyGO work and which board is
  involved (from `.lilygo-skills/project.json`, else sniffing `platformio.ini`,
  `sdkconfig`, and `*.ino`, else prompt keywords), then returns a small capsule:
  `board`, `board_source`, `skills`, `verification_level`, and the compact
  `context` string with the follow-up `source query` command. A recognized but
  out-of-scope product (a non-ESP32 LilyGO board) yields a support-boundary line;
  a non-LilyGO prompt is a no-op.
- **`source query`** returns topic-scoped facts straight from the fact pack —
  each with its source ref (URL + line range + sha256) — plus a completeness
  signal (`complete` / `partial` / `needs_source_ingestion` / `unsupported`) and,
  when a value is not yet ingested, a discovery hint rather than a guess.
- **`verify sources`** re-fetches each line-anchored fact's raw source,
  recomputes the sha256, and classifies `OK` / `DRIFT` / `UNREACHABLE`. Offline
  or rate-limited is graceful (`UNREACHABLE`, still exit 0); only a real content
  drift fails.
- **`doctor`** checks the data model is present and internally consistent (board
  registry, fact packs, V3 evidence coverage, sniff rules) and runs a sample
  injection.
- **`hook`** is the push boundary. On Claude Code the installed
  `UserPromptSubmit` hook (`node <root>/bin/hook.mjs claude`) reads the prompt
  JSON on stdin and emits
  `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"..."}}`
  (envelope or nothing; diagnostics on stderr; fail-open exit 0). Codex consumes
  the diagnostic envelope via `hook codex` from the marked `AGENTS.md` section.

## Push / pull boundary

The kernel seeds context and then points at the rest:

- **Push (`hook`).** The thick capsule inlines a board's critical pin/bus/driver
  facts — the subset that fits the byte budget — plus imperative
  pull-before-claim guidance. It is a pointer, not the full pinout.
- **Pull (`source query`).** Any concrete pin/bus/setting not already inlined
  MUST be pulled with `source query` and answered from the returned source ref.
  An absent pin means "go pull it", never "infer it from the ones shown".

This complete loop — push the critical subset, pull the rest with citation — is
what keeps answers correct and verifiable. A push-only read of an incomplete
capsule is unsafe, so `SKILL.md` and the guides make the pull rule firm.

## Data model and supply chain

Board facts are ingested from official LilyGO and vendor sources, never
hand-typed. The kernel reads only committed JSON:

- `data/boards.json` — the board/product registry.
- `data/facts/board-fact-packs.json` — per-board pin/bus/expander/connector/
  peripheral tables, each fact carrying its source ref and evidence level.
- `data/facts/prompt-keywords.json`, `data/facts/topic-fields.json` — the
  keyword and topic-field tables that drive selection.
- `data/sniff-rules.json` — project-file matchers for board auto-detection.

The ingest and verification pipeline is a set of Node scripts run at author time,
not part of the everyday CLI:

```text
official source (pipeline/source-manifest.json: raw URL + line range + macro->fact mapping)
  -> node pipeline/ingest-from-manifest.js --board <id> --write
  -> data/facts/board-fact-packs.json (values with URL + line range + sha256)
  -> node pipeline/verify-auto-mapping.js       extracted pins == official macros
  -> node pipeline/verify-source-authority.js   every fact keeps a ranked official source
  -> node eval/verify-provenance.js             every fact carries url + hash provenance
```

## Source authority

Source authority is ordered, not flattened:

1. Official code, headers, examples, manifests, and board repositories.
2. Official LilyGO hardware documentation.
3. `https://github.com/Xinyuan-LilyGO/documentation`, the versioned source behind
   LilyGO wiki content.
4. `wiki.lilygo.cc` fallback pages.
5. Project reference patterns, used as implementation/debug hints.

Reference hints tell the agent what to read; they do not override source facts.
When the kernel cannot prove an exact value it returns `unknown_with_sources` or
`needs_source_ingestion` rather than guessing.

## Board and project identity

Board identity resolves in priority order: explicit prompt text, then
project-local `.lilygo-skills/project.json`, then project-file sniffing, then a
no-op when the prompt is unrelated. Prompt facts always win. Ambiguous project
evidence (two boards in one `platformio.ini`) yields no board rather than a
guess. Project files under `.lilygo-skills/` exist so different firmware
directories can carry different defaults; private or machine-local state
(`local.json`, `evidence/`) must never be committed or injected into public
prompt context.

## Verification boundary

The kernel is verified at **V3** for source/context: routing, hook output,
source facts, completeness status, live re-proof, and provenance are covered by
gates. Hardware execution is a separate, task-scoped evidence track — build
artifacts, flash success, serial logs, OTA transport, display pixels, and
peripheral behavior move from "context available" to "verified" only when the
matching V4/V5 artifact exists. See
[docs/VERIFICATION_LEVELS.md](docs/VERIFICATION_LEVELS.md).

The honesty markers are machine-checkable, not just prose: `hardware_verified`
and `evidence_boundary` are emitted values a gate can grade.

## Quality gates

Two gate families guard a runtime change, both language-independent of the
firmware and both run in CI:

- **JS core** — `npm test` (unit + CLI contract parity + hook value-alignment
  against a frozen reference + CJK routing), `npx tsc --noEmit`, `doctor --json`,
  and live `verify sources`.
- **Data / provenance** — the official-source pipeline (gold + all boards),
  gold fact-pack diff, board triple-question coverage, provenance verification,
  the injected-capsule coverage gate (ratcheted, up-only), and scorecard grading.

`scripts/ci-gate.sh` aggregates both families plus the doc/surface hygiene smokes
and the install -> hook integration smoke, so a HEAD-failing check can never ride
a green pipeline.

## Runtime materialization

`install.js` owns the runtime root and mirrors `bin/**` and `data/**` there, so
stale files from a previous version cannot survive an install. User-owned host
files — Claude `settings.json` and Codex `AGENTS.md` — are merge-only and
marker-scoped: the installer replaces only its own hook entry and refuses to
touch invalid JSON or unbalanced markers. Host toolchains (Arduino CLI,
PlatformIO, ESP-IDF, esp-rs, serial and radio tools) stay explicit and are never
installed implicitly.

## File map

```text
bin/                          Node context kernel (dispatcher + hook + data readers)
data/boards.json              Board/product registry
data/facts/                   Board fact packs + keyword/topic tables
data/sniff-rules.json         Project-file matchers for board detection
data/references/source-intake Public source-intake cache and manifest
skills/lilygo-router/         Committed meta router SKILL.md + guides + references
pipeline/                     Node ingest + verification pipeline
eval/                         Contract/alignment tests, coverage + provenance gates, scorecards
scripts/                      Deterministic gate (ci-gate.sh) + hygiene/integration smokes
docs/                         Human architecture and contributor docs
```
