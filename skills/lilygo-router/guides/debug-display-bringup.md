# Debug: display bring-up

Goal: get a screen to light and render, one source-backed step at a time. This
is bring-up (does the panel initialize and show anything); LVGL refresh/touch
loop triage lives in [[debug-lvgl-loop]].

Pull facts first ([[query-protocol]]). Never state a display pin, bus, or
rotation from memory — run
`lilygo-skills source query --board <board-id> --topic display --json` and use
the returned values.

## What to establish from source, in order

1. **Display controller + bus.** Which controller (e.g. an ST7789-class TFT vs a
   panel driven over ESP-IDF i80), and the exact bus wiring — get these from the
   board's official example and `pins_arduino.h` / variant header via
   `source query`, not from a generic driver default.
2. **Backlight and power rails.** Many LilyGO panels need a backlight enable pin
   *and* a power rail brought up (often through a PMU / power-management chip)
   before the controller responds. Query the `power` topic too; confirm the
   enable pin and rail. If the rail is gated by a PMU, see [[debug-power-battery]].
3. **Framework display path.** Arduino/PlatformIO typically drive the panel
   through TFT_eSPI (a `User_Setup` / `Setup*.h` selection) or the board's own
   driver wrapper; ESP-IDF drives it through the i80/SPI LCD peripheral. Preserve
   the framework the project already uses — do not swap TFT_eSPI for i80 (or vice
   versa) to "simplify."

## Bring-up sequence

- Confirm the controller, bus pins, color order, and rotation from the official
  example before writing or editing display init code.
- Bring up power rail → backlight → controller init in that order. A dark screen
  with a running MCU is usually a rail/backlight problem, not the controller.
- Prove the panel with the board's official display demo (or the smallest solid-
  fill test) before layering your own UI on top. See [[board-bringup-checklist]].

## Failure triage

- **display-init-missing** — controller never initialized: re-check the init
  sequence and bus pins against source.
- **Blank but powered** — suspect backlight enable or power rail, not the
  controller. Verify both pins via `source query`.
- **Wrong colors / mirrored / offset** — color order or rotation mismatch; take
  these from the board example, not a guess.
- **Missing fact** — if a pin/rail is `unknown_with_sources`, report it as not
  confirmed and cite the `update board-facts --dry-run` command; do not invent it.

## Honesty

A source link proves what the sources say, not that pixels rendered. Do not claim
the screen works until you have simulator, page-data, or a real-board capture as
evidence — `hardware_verified=false` means not hardware-confirmed. See
[[honesty-evidence]].
