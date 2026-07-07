# Context Layers

Chinese version: [CONTEXT_LAYER.zh-CN.md](CONTEXT_LAYER.zh-CN.md).

lilygo-skills does not paste every source into the prompt at once. It separates
stable runtime logic, generated Skill files, source facts, project preferences,
and references so the agent receives the smallest useful context first, then
expands only when the task needs more detail.

## Static And Dynamic Layers

| Layer | Path | Purpose |
|-------|------|---------|
| Meta Skill | `skills/lilygo-router/SKILL.md` | The only committed Skill entry. |
| Static references | `skills/references/*.md` | Readable expansion docs, not routed skills. |
| Templates | `templates/skills/*.md` | Public templates for generated runtime Skill files. |
| Source model | `data/**`, `index/**` | Board, fact, peripheral, playbook, and route data. |
| Generated skills | install/cache/project output | Materialized by install, update, or explicit generation. |
| Project ledger | `.lilygo-skills/ledger.json`, `.lilygo-skills/context-digest.json` | Prompt-safe project memory and repeated-context digests. |

The route layer uses explicit tokens rather than unsafe prefix or substring
matches. The source model creates chip skills only for real chip identifiers;
composite labels and capacity facts stay in board or peripheral facts. `goal
complete` coordinates completion state and can turn existing facts and demo refs
into readiness, but it must not describe context as hardware proof.

Templates define file shape, not board truth. Generated skills may share section
structure because they use `templates/skills/*.md`; the real content comes from
the source model, fact packs, source packs, recipes, playbooks, and official
references. This keeps the public repository reviewable and reproducible without
committing large generated snapshots.

For T-Watch Ultra, the generated `board-t-watch-ultra` content is not an empty
template. It includes ESP32-S3, Arduino FQBN, AMOLED `CO5300`, touch
`CST9217`, GNSS `MIA-M10Q`, radio option `SX1262 or SX1280`, NFC `ST25R3916`,
IMU `Bosch BHI260AP`, power `AXP2101`, RTC `PCF85063A`, haptic `DRV2605`,
expander `XL9555`, SD, PSRAM, official LilyGoLib docs, driver headers, and demo
paths.

Focused generated tree for T-Watch Ultra:

```text
skills/board-t-watch-ultra/SKILL.md
skills/chip-bhi260ap/SKILL.md
skills/chip-xl9555/SKILL.md
skills/feature-raise-to-wake/SKILL.md
skills/periph-display/SKILL.md
skills/periph-imu/SKILL.md
skills/periph-input/SKILL.md
skills/app-ota/SKILL.md
skills/app-watch-ui-lvgl/SKILL.md
skills/debug-lvgl-loop/SKILL.md
skills/fw-lvgl/SKILL.md
skills/playbook-build-flash-serial/SKILL.md
skills/playbook-lvgl-debug/SKILL.md
skills/playbook-ota-debug/SKILL.md
skills/playbook-source-discovery/SKILL.md
skills/references/*.md
templates/skills/*.md
index/routes.json
```

At runtime, the board Skill is only a compact snapshot. Richer task context
comes from `goal plan.context_capsule`, `source query`, and `source
completeness`:

```bash
lilygo-skills goal plan --json "T-Watch Ultra Arduino LVGL touch does not move"
lilygo-skills goal plan --json "T-Watch Ultra ESP-IDF OTA rollback manifest debug"
lilygo-skills source query --board board-t-watch-ultra --topic io --json
```

Those surfaces load display, touch, OTA, IMU, source refs, demo refs,
permission, and evidence boundaries for the task without putting the entire fact
pack into the default prompt.

## Default Injection And Budget

Default injection contains:

- Matched skill ids and summaries.
- Top facts required by the current task.
- Readiness such as `complete`, `needs_source_ingestion`, or `unsupported`.
- Source/query/generation next commands.
- Evidence boundaries and permission hints.

Full fact packs, reference docs, and templates are not injected by default. The
agent reads those files only for implementation, debug, setup, generation, or
verification work.

The runtime treats prompt budget as part of correctness. Pure lookup prompts
stay read-only and compact: fact tables, source refs, and `source query`
commands are enough. Implementation/debug prompts can add `goal-plan-bridge`,
selected demos, and permissioned next actions, but repeated board, framework,
demo, project-skill, and topic details collapse into short incremental hints
with expansion commands.

Incremental hook context is session-scoped. It activates only when the hook
event carries a stable session id, or when tests set `LILYGO_SKILLS_SESSION_ID`.
The cache expires by TTL and runtime version; set
`LILYGO_SKILLS_DISABLE_INCREMENTAL=1` to force full capsules. An incremental
capsule still includes critical pins, source-query expansion, and evidence
boundaries. It trims only repeated bulky lists such as facts, demos, recipes,
generated skills, and playbook summaries.

Project ledger context is repo-scoped. After `project init`, the runtime may
record public context digests and previously verified capability summaries under
`.lilygo-skills/`. A repeated prompt can then receive a small ledger capsule
instead of the same full context. The capsule still keeps critical facts,
evidence boundaries, stale markers, and expansion commands. Ledger entries
become stale when code, source signatures, runtime version, TTL, or explicit
re-verify wording says the prior evidence should not be reused.

Useful expansion commands remain stable:

```bash
lilygo-skills source query --board <board-id> --topic io --json
lilygo-skills index query <skill-or-playbook-id> --json
lilygo-skills goal plan --json "<prompt>"
lilygo-skills project ledger show --project <project-dir> --json
```

Incomplete starter board packs follow the same rule. They may expose
`unknown_with_sources` or `needs_source_ingestion` plus official references so
the agent knows where to inspect next; they do not invent pins, peripherals, or
runtime behavior to fill the capsule.

Implementation and debug prompts also receive a compact source recovery capsule:
the nearest official demo path, board-owned source headers, a few critical
facts, and expansion commands. For example, a T-Display-S3 TFT_eSPI + I2C
prompt can surface `examples/tft/tft.ino`, Setup206, `pin_config.h`,
`PIN_IIC_SDA=GPIO18`, `PIN_IIC_SCL=GPIO17`, and a `source query` command without
injecting the whole fact pack.

## Preferences And References

Preferences are public behavior choices such as framework order, serial debug
tooling, code-size limits, and safety defaults. References are public reading
hints such as official examples, source files, datasheets, hardware docs, and
project design notes.

Neither preferences nor references override official source facts. When a board
or topic lacks required facts, the runtime should return `needs_source_ingestion`
or `unknown_with_sources` instead of treating a reference as ready evidence.

## Installed Runtime

`node install.js --all` first mounts the Skill. If no compiled runtime binary is
available, it enters mount-only mode: Codex/Claude entry points are wired, the
meta router, source data, `skills/references/`, and `templates/skills/` are
copied, and a setup-only launcher is installed. Full dynamic injection is enabled
later with a release-bundle prebuilt path
(`node install.js --all --prebuilt-only`), `node install.js --all --build`, or
`--bin /path/to/lilygo-skills`.
The installed agent can still inspect the same context contracts and use setup
plans to configure Rust/Cargo, Arduino, PlatformIO, ESP-IDF, or Rust esp-rs.

`doctor --json` validates the installed runtime from the checked home. Besides
route and hook samples, it compares Codex and Claude runtime mirrors when both
exist. Matching mirrors pass, a single installed host passes, and drift is a
warning with a reinstall command.

The support model is board-family extensible. Current verified runtime coverage
starts with the LilyGO ESP32 family. Without matching V4/V5 evidence,
source/context output must not be described as completed hardware behavior.
