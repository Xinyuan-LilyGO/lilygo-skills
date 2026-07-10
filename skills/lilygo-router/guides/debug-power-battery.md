# Debug: power rails, charging, and fuel gauge

Goal: confirm power rails, charging, and battery/fuel-gauge state from
source-backed facts before wiring or claiming anything about power.

Pull facts first ([[query-protocol]]):
`lilygo-skills source query --board <board-id> --topic power --json`. Many LilyGO
boards route rails through a power-management chip (PMU) and read the battery
through a separate fuel-gauge chip — both are on a bus with an address and, often,
an enable pin. Get the chip IDs, bus, address, and enable/rail pins from source;
never state a rail or an ADC/gauge pin from memory.

## What to establish, in order

1. **Rail topology.** Which rails exist, which chip gates them, and which enable
   pin brings each up. Peripherals (display backlight, radio, GNSS) frequently
   depend on a rail being enabled first — a "dead" peripheral is often an
   un-powered rail. Cross-reference [[debug-display-bringup]] and
   [[debug-lora-gnss]].
2. **Charging path.** The charger config (current limit, source) lives in the PMU
   registers per the official example and datasheet — read it, do not assume.
3. **Fuel-gauge / battery read.** Whether battery voltage/percentage comes from a
   fuel-gauge chip over a bus or from an ADC pin, and the exact channel/address.

## Bring-up as a BSP peripheral

Treat power like any driver ([[board-bringup-checklist]]): read the official
example, chip datasheet, and framework driver first; expose **capability and
status before any action**; keep smoke checks bounded and non-destructive by
default. Query board facts and preserve `unknown_with_sources` when an exact
rail/pin is absent — never guess an expander channel or rail.

## Failure triage

- **power-rail-unknown** — the rail/enable pin is not in source: report it as not
  confirmed, cite `update board-facts --dry-run`, and stop; do not invent it.
- **Peripheral dead** — check the gating rail is enabled before blaming the
  peripheral driver.
- **Battery reads implausible** — confirm you are reading the right chip/channel
  and scaling per the datasheet.

## Honesty

A datasheet or source link proves the topology, not that a rail is up or the
battery reads correctly on this board — that needs a real-board measurement.
`hardware_verified=false` means not hardware-confirmed. Never invent rails,
enable pins, addresses, or channels. See [[honesty-evidence]].
