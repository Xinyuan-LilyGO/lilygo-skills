# CLAUDE.md

This is the public `lilygo-skills` runtime repository.

Use the CLI and install surfaces as the source of truth for behavior:

```bash
node install.js --all --dry-run --build
cargo run -q -p lilygo-skills-cli -- verify --json
cargo run -q -p lilygo-skills-cli -- route --json "<prompt>"
cargo run -q -p lilygo-skills-cli -- goal complete --dry-run --json "<prompt>"
```

Runtime source data is under `data/**`, including
`data/references/source-intake/**`. Do not add runtime dependencies under
`doc/**`; this public runtime repo should keep human documentation under
`docs/**`.

Before claiming a runtime change is complete, run focused tests plus:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/ci-gate.sh
```
