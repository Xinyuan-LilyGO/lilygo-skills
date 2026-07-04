# AGENTS.md

This is the public `lilygo-skills` runtime repository.

## What Lives Here

- Rust CLI runtime in `crates/lilygo-skills-cli/`
- Installer in `install.js`
- Meta router skill in `skills/lilygo-router/SKILL.md`
- Static expansion references in `skills/references/`
- Generated-skill templates in `templates/skills/`
- Public source data in `data/**`
- Route registry in `index/routes.json`
- Hardware-free smoke gates in `scripts/**`

This repository is self-contained for install, generation, verification, and
runtime use.

## Agent Workflow

Prefer the installed or built CLI surfaces before browsing source:

```bash
node install.js --all --dry-run --build
cargo run -q -p lilygo-skills-cli -- verify --json
cargo run -q -p lilygo-skills-cli -- route --json "<prompt>"
cargo run -q -p lilygo-skills-cli -- goal complete --dry-run --json "<prompt>"
```

Use `data/references/source-intake/**` for public source-intake data. Do not add
runtime dependencies under `doc/**`; this public runtime repo should keep human
documentation under `docs/**`.

## Verification

For runtime changes, run the relevant focused test first, then the aggregate
gate before claiming completion:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/ci-gate.sh
node install.js --all --dry-run --build
```

Do not commit secrets, Wi-Fi credentials, tokens, raw local OTA logs, serial
ports, private device identifiers, generated skill caches, or `.lilygo-skills/`
local evidence.
