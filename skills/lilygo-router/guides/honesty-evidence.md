# Honesty and evidence

The core rule of every LilyGO guide: **claims stay at the level the evidence
actually supports.** A source link, capsule, or generated command proves *what
the sources say* — not that firmware builds, flashes, renders pixels, or acquires
an RF/GNSS fix.

## Evidence levels

Each fact and step carries a level; state the true one, never a higher one:

- **V3 — source reference.** A pin/bus/demo backed by an official URL, line, and
  sha256. Proves what the source says. Read-only.
- **V4 — planning / host artifact.** A build, a partition/manifest inspection, a
  simulator/page-data render. Proves compilation or host-side state — not board
  behavior.
- **V5 — observed on the wire/board.** An upload exit code, a bounded serial
  excerpt, an observed reboot/rollback, a captured RF packet or GNSS fix. This is
  the only level that proves hardware behavior, and only for what was observed.

A build (V4) is not a flash (V5). A flash is not a working peripheral. A running
flush callback is not a visible frame. Visible NMEA is not a GNSS fix.

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
