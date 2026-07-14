# Contributing Boards

Chinese version: [CONTRIBUTING_BOARDS.zh-CN.md](CONTRIBUTING_BOARDS.zh-CN.md).

This guide is for adding or improving LilyGO board support.

## Support Boundary

The support model grows across LilyGO boards. Runtime coverage starts with
LilyGO products in the ESP32 family:

- ESP32
- ESP32-S2
- ESP32-S3
- ESP32-C3
- ESP32-P4

Other LilyGO products enter as source candidates first. Add public references,
board-family metadata, and evidence requirements; build, flash, OTA, and
hardware-debug guidance then expand through the same source-backed support
flow.

## Add Or Improve A Board

1. Identify the exact product id, aliases, MCU family, and supported frameworks.
2. Gather source refs in authority order: official repo/code/examples first,
   then official hardware docs, documentation repo, wiki fallback, and local
   reference patterns.
3. Add or refresh board source metadata.
4. Generate or update compact board skills.
5. Add source facts for IO, pinout, bus, expander, connector, peripheral, and
   quick-start topics where official sources prove them.
6. Add context/verification fixtures and negative over-injection cases under
   `eval/**`.
7. Run the verification suite.

Board and fact-pack data now travel with the skill directory and are produced
by the official-source pipeline. Regenerate and diff data with:

```bash
node pipeline/run-official-source-pipeline.js --all-boards --json
node pipeline/diff-gold-fact-packs.js
```

The pipeline is dry by default (it writes a plan under `.tmp/pipeline/`); add
`--write` only once the diff is correct to persist `data/facts/**`. Inspect the
JSON plan before committing regenerated fact packs, and do not hand-edit
generated packs.

## Fact Quality Rules

- Use `exact` only for values directly proven by a high-authority source.
- Use `derived` when the value follows from source-backed metadata.
- Use `unknown_with_sources` when relevant sources exist but do not prove the
  exact value.
- Do not guess free GPIOs, expander channels, power rails, display buses, or
  touch controllers from product names.
- Keep source refs and hashes so future updates can detect stale facts.

## Skill Quality Rules

Generated skills should stay compact:

- Trigger terms and aliases.
- What the board/chip/framework is for.
- High-value source pointers.
- Lookup commands for deeper details.
- Verification boundary.

Do not paste full datasheets, long source files, or complete fact packs into
`SKILL.md`. The AI should call `source query` or `verify sources` when it needs
more.

## Tests And Smokes

At minimum:

```bash
npx tsc --noEmit
npm test
bash scripts/ci-gate.sh
git diff --check
```

When adding a board that overlaps an existing family trigger, add an exact-board
precedence regression so `context` output does not select a misleading generic
board as the injected context.

## Documentation Checklist

Update docs when behavior changes:

- `README.md`
- `README.zh-CN.md`
- `ARCHITECTURE.md`
- `ARCHITECTURE.zh-CN.md`
- `docs/BOARD_FACTS.md`
- `docs/VERIFICATION_LEVELS.md`

For release-process changes, update the matching public docs, smoke gates, and
verification commands in the same change.
