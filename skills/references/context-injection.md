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
| Goal plan | Read-only execution plan with permissions and evidence boundaries. |
| Generated skills | Compact runtime summaries generated from the source model. |

## Default Injection

Inline only:

- matched skill ids and summaries;
- top-ranked facts needed for the current prompt;
- readiness or `needs_source_ingestion`;
- expansion commands such as `index query`, `source query`, and `goal plan`;
- evidence boundary and required permissions.

Do not inline complete reference docs or templates unless the user asks for
that material or the implementation needs it.

## No-Write Rule

`route`, host hooks, `index query`, `source query`, `source completeness`, and
`goal plan` are read-only. They may report a generation or update command, but
they do not write files, fetch sources, open serial ports, flash devices, or run
network operations.

Writing happens only through explicit commands such as `generate skills`,
`project init`, `update board-facts`, or permissioned `goal start`.
