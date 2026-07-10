# Debug: LoRa radio and GNSS bring-up

Goal: navigate LoRa (SX126x-class + RadioLib) and GNSS source, config, and
evidence — without ever claiming an RF link or a position fix that has not been
observed.

Pull facts first ([[query-protocol]]). Query the `lora` and `gnss` topics
separately: `lilygo-skills source query --board <board-id> --topic lora --json`
and `--topic gnss`. **Do not merge LoRa and GNSS into one generic fact** — they
are different chips on different buses.

## Map the source (read-only) before any transmit

1. **Radio chip + bus + driver entry.** Identify the LoRa radio chip, its SPI
   wiring, and the RadioLib driver entry points from the board's official LoRa
   example — not from a generic RadioLib sketch. Get pins from `source query`.
2. **GNSS module + UART.** Identify the GNSS module, its UART wiring, and the
   NMEA/config path from the official GNSS example. Keep this separate from the
   radio facts.
3. **Region / frequency / antenna.** Validate frequency, bandwidth, spreading
   factor, and region against the chip datasheet and board example **before**
   transmitting. Antenna presence and region legality are user/project facts —
   do not assert regulatory correctness on your own.

Authoritative order: official LilyGO board example first, then RadioLib as a
library reference (`https://github.com/jgromes/RadioLib`), then Meshtastic as
*application* guidance only when the user asks for it. RadioLib and Meshtastic
are not board-fact sources.

## Observe (only with explicit port permission)

Capture TX/RX or GNSS output over serial on an explicitly selected `--port`
(see [[debug-flash-serial]]). For GNSS, separate **UART/NMEA visibility** (you
see sentences) from **an actual fix** (valid position) — the first does not prove
the second.

## Failure triage

- **radio-chip-unknown / gnss-chip-unknown** — bind the official board example
  and confirm the chip via `source query`; report as unknown if missing.
- **region-frequency-missing** — confirm config against datasheet before any
  transmit.
- **rf-no-packet** — re-check config, wiring, and antenna assumptions in source
  before claiming anything about the link.
- **gnss-no-fix** — visible NMEA with no fix is normal indoors / cold-start; do
  not report a fix you did not observe.

## Honesty

Source refs cannot prove an RF link or a GNSS fix. Never claim TX/RX success or a
position fix without bounded serial evidence, and never invent a pin, bus,
frequency, or region value. See [[honesty-evidence]].
