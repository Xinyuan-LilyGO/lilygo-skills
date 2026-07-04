# LVGL Context

LVGL work combines board facts, display/touch drivers, framework setup, memory,
and render evidence. Treat LVGL as an application/domain layer, not a board
fact by itself.

## Read First

- Board display and touch facts.
- Official board examples and display driver code.
- LVGL docs for display flush, tick, input devices, buffers, color format, and
  invalidation.
- Framework-specific setup for Arduino, PlatformIO, ESP-IDF, or Rust esp-rs.

## Diagnostic Axes

- tick source and period;
- display driver initialization order;
- draw buffer size and memory location;
- flush callback completion;
- color order, rotation, and resolution;
- touch/input callback mapping;
- heap pressure and task scheduling;
- simulator/page-data or hardware pixel evidence.

## Evidence Boundary

`goal plan` and source facts can produce LVGL context. A screenshot, simulator
artifact, page-data trace, or physical display evidence is required before
claiming pixels rendered correctly.
