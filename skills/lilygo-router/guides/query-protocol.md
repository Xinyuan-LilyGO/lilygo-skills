# Query protocol — get context, then pull exact facts

Read this before you state any pin, GPIO, bus, expander channel, power rail, or
demo path. Hardware facts come from source-backed data, never from memory.

## 1. Get the capsule

Run `lilygo-skills context` (add `--json`, or `--project <dir>` to target a
specific tree). It auto-detects the board from the project — reading
`.lilygo-skills/project.json` when present, and otherwise sniffing
`platformio.ini`, `sdkconfig`, and `*.ino` — and returns a small capsule: the
matched skill IDs, top-ranked facts, the verification level, and follow-up
lookup commands.

In Claude Code the installed hook auto-injects this capsule, so it usually
arrives without a manual call. On any other host, run `context` yourself — the
data and gates are identical.

If `context` returns `decision: no-op` with an empty board, the prompt is either
not a LilyGO task or the board could not be inferred. Ask the user which LilyGO
product and framework they mean; never silently pick one.

## 2. Pull exact facts from source

Before stating a specific value, run:

```
lilygo-skills source query --board <board-id> --topic <topic> --json
```

Topics are the peripheral / chip / connector / demo area surfaced by the
capsule — e.g. `display`, `input`, `lora`, `gnss`, `power`, `pinout`. Each
returned fact carries its official `path_or_url`, line reference, `sha256` hash,
`authority_rank`, and `evidence_level`. Quote *those* values; do not paraphrase a
pin from recall.

**The capsule is a pointer, not the full pinout.** The injected capsule surfaces
only *some* pins — the critical subset — never the complete pin map. If the
pin/bus you need is not present in the capsule, you **MUST** run the
`source query` above to fetch it. NEVER infer a pin from the subset that is
shown, and never answer a pin from memory. An absent pin means "go pull it", not
"guess" — a partial capsule is the signal to pull, not a licence to fill the gap.
See [[honesty-evidence]].

## 3. When a fact is missing

If a topic reports `needs_source_ingestion` or `unknown_with_sources`, do not
guess. Preview enrichment and report the official references instead:

```
lilygo-skills update board-facts --board <board-id> --topic <topic> --dry-run --json
```

Then tell the user the value is **not confirmed in official sources** and point
at that command. See [[honesty-evidence]].

## Keep the answer small

Cite the matched IDs, the top facts, and the lookup command. Do not paste whole
fact packs, source files, or reference docs unless the user asks for that
content. The related how-to guides — [[board-bringup-checklist]],
[[debug-flash-serial]], [[debug-display-bringup]], [[debug-lora-gnss]],
[[debug-power-battery]], [[debug-lvgl-loop]], [[toolchain-setup]] — each open
into a specific workflow once the capsule tells you which one applies.
