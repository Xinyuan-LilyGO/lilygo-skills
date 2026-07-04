# Contributing Boards

Chinese version: [CONTRIBUTING_BOARDS.zh-CN.md](CONTRIBUTING_BOARDS.zh-CN.md).

This guide is for adding or improving LilyGO board support.

## Support Boundary

The support model is intended to grow across LilyGO boards. Current verified
runtime coverage starts with LilyGO products in the ESP32 family:

- ESP32
- ESP32-S2
- ESP32-S3
- ESP32-C3
- ESP32-P4

Other LilyGO products may be recorded as source candidates, but build, flash,
OTA, and hardware-debug guidance must remain unsupported until support is
designed and verified.

## Add Or Improve A Board

1. Identify the exact product id, aliases, MCU family, and supported frameworks.
2. Gather source refs in authority order: official repo/code/examples first,
   then official hardware docs, documentation repo, wiki fallback, and local
   reference patterns.
3. Add or refresh board source metadata.
4. Generate or update compact board skills.
5. Add source facts for IO, pinout, bus, expander, connector, peripheral, and
   quick-start topics where official sources prove them.
6. Add completeness gates for topics that should be quick-start ready.
7. Add route fixtures and negative over-injection cases.
8. Run the verification suite.

Useful commands:

```bash
cargo run -p lilygo-skills-cli -- sync-boards --dry-run --json
cargo run -p lilygo-skills-cli -- update boards --dry-run --json
cargo run -p lilygo-skills-cli -- update skills --dry-run --json
cargo run -p lilygo-skills-cli -- update fact-packs --dry-run --json
cargo run -p lilygo-skills-cli -- update board-facts --board <board-id> --topic <topic> --dry-run --json
cargo run -p lilygo-skills-cli -- update source-packs --dry-run --json
cargo run -p lilygo-skills-cli -- update peripheral-skills --dry-run --json
```

Run without `--dry-run` only when the planned writes are correct and inside the
supported paths. `update skills` and `update peripheral-skills` write generated
runtime skills only to `.lilygo-skills/generated-skills/` or `--out
<generated-root>`; they must not write generated `SKILL.md` files into the
source `skills/` tree.

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
`SKILL.md`. The AI should call `source query`, `source completeness`, `index
query`, or `reference list` when it needs more.

## Tests And Smokes

At minimum:

```bash
cargo test -q -p lilygo-skills-cli
cargo run -q -p lilygo-skills-cli -- verify --json
cargo run --release -q -p lilygo-skills-cli -- benchmark --json --iterations 5000
bash scripts/source-completeness-smoke.sh --dry-run
bash scripts/board-completeness-smoke.sh --dry-run
bash scripts/full-evidence-smoke.sh --dry-run
git diff --check
```

When adding a board that overlaps an existing family trigger, add an exact-board
precedence regression so route output does not include a misleading generic
board as the selected context.

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
