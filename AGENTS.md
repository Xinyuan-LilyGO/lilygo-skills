# AGENTS.md

This is the public `lilygo-skills` runtime repository.

## What Lives Here

- JS thin core in `bin/*.mjs` (Node, zero-build; types via JSDoc + `tsc`)
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
lilygo-skills context --json "<prompt>"
lilygo-skills source query --board <board-id> --topic <topic> --json
lilygo-skills verify sources --board <board-id> --json
```

Use `data/references/source-intake/**` for public source-intake data. Do not add
runtime dependencies under `doc/**`; this public runtime repo should keep human
documentation under `docs/**`.

## Verification

For runtime changes, run the relevant focused test first, then the aggregate
gate before claiming completion:

```bash
npx tsc --noEmit
npm test
bash scripts/ci-gate.sh
node install.js --all --dry-run --build
```

Do not commit secrets, Wi-Fi credentials, tokens, raw local OTA logs, serial
ports, private device identifiers, generated skill caches, or `.lilygo-skills/`
local evidence.
