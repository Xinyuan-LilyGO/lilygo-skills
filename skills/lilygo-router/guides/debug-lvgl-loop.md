# Debug: LVGL refresh and touch loop

Goal: diagnose an LVGL blank screen, stalled refresh, or dead touch by walking
the tick → flush → draw-buffer → input chain in order — without claiming rendered
pixels you have not proven. Panel/backlight bring-up is separate; see
[[debug-display-bringup]] first if the screen never lights at all.

Pull facts first ([[query-protocol]]). Confirm the display controller, touch
controller, bus, rotation, LVGL version, and framework display driver from the
board's official example before editing LVGL code:
`lilygo-skills source query --board <board-id> --topic display --json` and
`--topic input`.

## Walk the chain, in order

1. **Display init** — panel actually initialized (see [[debug-display-bringup]]).
2. **LVGL tick** — is `lv_tick` being fed? A stalled tick freezes everything.
3. **Flush callback** — is the flush callback called and does it signal ready?
4. **Draw buffer + heap** — buffer size vs available heap; too-small or failed
   allocation shows as blank or partial frames.
5. **Color order + rotation** — take these from the board example, not a guess.
6. **Touch read callback** — separate the **touch controller probe** (does the
   chip return data on its bus) from the **LVGL indev callback** (does LVGL read
   it). A dead pointer is usually one or the other, not both.

## Prove it before claiming it

Where available, use a **simulator or page-data** proof before asserting on-board
UI behavior: collect the LVGL object tree, invalidation, input state, and
tick/flush timing, and render the same page in a host harness / simulator. This
gives evidence at simulator level before you ever claim on-board pixels. LVGL
official docs (`https://docs.lvgl.io/`) are the first reference for core API
behavior.

## Failure triage

- **lvgl-tick-stalled** — instrument the tick source before anything else.
- **flush-not-called** — verify the flush callback is registered and invoked.
- **heap-or-buffer-too-small** — check allocation success and buffer sizing.
- **touch-controller-no-data** — probe the controller on its bus before blaming
  the indev callback.
- **runtime-timeout-no-observation** — reduce to simulator/page-data before
  claiming attached-board behavior.

## Honesty

Source and route context cannot prove pixels rendered, and a running flush
callback is not a visible frame. Do not claim touch works without callback or
device evidence. `hardware_verified=false` means not hardware-confirmed. See
[[honesty-evidence]].
