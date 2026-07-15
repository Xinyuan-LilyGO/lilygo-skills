# Honesty and evidence

The core rule of every LilyGO guide: **claims stay at the level the evidence
actually supports.** A source link, capsule, or generated command proves *what
the sources say* — not that firmware builds, flashes, renders pixels, or acquires
an RF/GNSS fix.

## Evidence levels

`evidence_level` is a first-class axis on every fact and step. It states *how
strongly the value is grounded* — never how well anything worked. Each fact
carries exactly one level; state the true one, never a higher one:

- **V3 — source reference.** A pin/bus/demo value read out of an official
  source. The strongest, re-verifiable form — `V3-source-reference` — is a
  **`#define`-verified value carrying the complete provenance triple: an
  official file URL + a `line_range` + a `sha256:` hash.** That triple is what
  makes it *runnable-verifiable*: `lilygo-skills verify sources --board <id>`
  re-fetches the URL, recomputes the sha256, and re-proves it live
  (OK / DRIFT / UNREACHABLE). A V3 fact proves only *what the source says* — it
  is read-only and says nothing about build, flash, or peripheral behavior.

  **A value taken from prose, a wiki sentence, a photo, memory, or any source
  that is not `#define`/code-verified with a URL+line+hash MUST NOT be labeled
  V3.** If a fact is source-backed at the repo/reference level (an official URL
  + hash but no `line_range`, e.g. a documentation-repo pointer), it is a weaker
  reference tier: honest as a pointer, but it is not the line-anchored,
  live-re-provable V3 that the `verify sources` re-proof and the
  `eval/verify-provenance.js` gate treat as fully verifiable. Do not round it up.
- **V4 — planning / host artifact.** A build, a partition/manifest inspection, a
  simulator/page-data render. Proves compilation or host-side state — not board
  behavior.
- **V5 — observed on the wire/board.** An upload exit code, a bounded serial
  excerpt, an observed reboot/rollback, a captured RF packet or GNSS fix. This is
  the only level that proves hardware behavior, and only for what was observed.

A build (V4) is not a flash (V5). A flash is not a working peripheral. A running
flush callback is not a visible frame. Visible NMEA is not a GNSS fix. And a
prose mention is not a V3 source reference.

## `hardware_verified=false`

This marker means exactly what it says: **not hardware-confirmed.** Do not imply
flash, OTA, serial, LVGL, or peripheral success from source links or a green
build alone. The capsule ships this marker (and an `evidence_boundary`) as a
machine-checkable honesty signal — keep it truthful.

## Never invent

Never invent pin numbers, buses, expander channels, or power rails. If a value is
not present in the source-backed data:

1. Say it is **not confirmed in official sources.**
2. Point at the discovery command:
   `lilygo-skills update board-facts --board <board-id> --topic <topic> --dry-run --json`
   (see [[query-protocol]]).
3. Stop — do not fill the gap from memory.

## The capsule is a subset, not the full pinout

The injected capsule surfaces only *some* pins — the critical subset — never the
complete pin map. **It is a pointer, not the pinout.** If the pin/bus you need is
not present in the capsule, you **MUST** run
`lilygo-skills source query --board <board-id> --topic <topic> --json` to fetch
it, then answer from that source-backed value. NEVER infer a pin from the subset
that happens to be shown, and never answer a pin from memory. An absent pin means
"go pull it", not "guess" — a partial capsule is precisely the trigger to pull,
never a licence to fill the gap by inference. This is the never-invent rule
applied to the capsule: a shown pin is source-backed; an unshown one is a
`source query` you have not run yet.

## Classify failures; don't retry blind

Sort a failed run into a bucket (missing tool/source, port/permission,
runtime-timeout-no-observation, OTA, build) rather than repeating it — see
[[debug-flash-serial]]. If a run produced no observable output, add explicit
boot/status markers or pick a smaller observable demo before rerunning.

## Keep private things private

Keep large or raw logs, private serial ports, Wi-Fi credentials, LAN hosts, OTA
URLs, and private runner argv in ignored local evidence (e.g.
`.lilygo-skills/local.json`) — not in the public answer.

## Scope

If the prompt is not about a LilyGO board, return no deep context. This honesty
framing is the same one asserted in `SKILL.md`; the per-domain guides all defer
here.
