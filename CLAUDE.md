# CLAUDE.md

This is the public `lilygo-skills` runtime repository.

Use the CLI and install surfaces as the source of truth for behavior:

```bash
node install.js --all --dry-run --build
lilygo-skills context --json "<prompt>"
lilygo-skills source query --board <board-id> --topic <topic> --json
lilygo-skills verify sources --board <board-id> --json
```

Runtime source data is under `data/**`, including
`data/references/source-intake/**`. Do not add runtime dependencies under
`doc/**`; this public runtime repo should keep human documentation under
`docs/**`.

Before claiming a runtime change is complete, run focused tests plus:

```bash
npx tsc --noEmit
npm test
bash scripts/ci-gate.sh
```
