# Project Preferences And References

Project preferences and references are public project context. They help a
firmware directory carry its own defaults without changing the global runtime.

## Preferences

Preferences are behavior choices:

- framework order;
- preferred debug tools such as serial-mcp-server, espflash, or binflow;
- code size limits;
- safety defaults such as dry-run preference and explicit flash permission.

They belong in `.lilygo-skills/preferences.json` when they are public and safe
to commit. They must not contain local ports, private network values, raw logs,
or machine paths.

## References

References are read hints:

- official examples;
- source files;
- datasheets;
- hardware notes;
- project-local design docs;
- public tool docs when the task needs the tool.

The agent should write explained entries, not naked links. Useful fields are
`title`, `kind`, `applies_to`, `authority`, `summary`, `read_when`, and
`inject_triggers`.

References do not override official source facts. If a board/topic is missing
required facts, surface `needs_source_ingestion` before treating a reference as
enough for runnable implementation.
