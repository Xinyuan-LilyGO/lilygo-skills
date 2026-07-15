# Context Injection Contract

This runtime keeps default context small. The first answer should identify the
board, framework, domain, readiness state, and lookup commands. Full source
files, fact packs, reference docs, and templates are expansion material.

## Layers

| Layer | Use |
|-------|-----|
| Router | Decide inject, no-op, clarification, or source ingestion. |
| Board facts | Board-specific MCU, buses, pins, expanders, power rails, demos. |
| Framework | Arduino, PlatformIO, ESP-IDF, Rust esp-rs, LVGL. |
| Preferences | Public behavior choices such as tool order and code limits. |
| References | Public read hints for official examples, datasheets, project docs, and tools. |
| Capsule | Read-only `context`/`hook` capsule with evidence boundaries. |
| Generated skills | Compact runtime summaries generated from the source model. |

## Default Injection

Inline only:

- the resolved board id and matched skills;
- top-ranked facts needed for the current prompt;
- the verification boundary (`context-injection`, `hardware_verified=false`,
  `evidence_boundary=V3`);
- expansion commands such as `source query` and `verify sources`.

Do not inline complete reference docs or templates unless the user asks for
that material or the implementation needs it.

## No-Write Rule

`context`, host hooks, `source query`, and `verify sources` are read-only. They
may report a follow-up command, but they do not write files, fetch sources beyond
live re-proof, open serial ports, flash devices, or run network operations.

Writing to the data model happens only through the installer (`node install.js`)
and the official-source pipeline (`node pipeline/run-official-source-pipeline.js`).
