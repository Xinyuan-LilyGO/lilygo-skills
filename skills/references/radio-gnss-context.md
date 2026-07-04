# Radio And GNSS Context

LoRa, GNSS, and Meshtastic-style work combine board facts, chip drivers,
antenna/region constraints, framework libraries, and runtime observation.

## Read First

- Board radio and GNSS fact packs.
- Official LilyGO examples for the specific board.
- RadioLib, Espressif, or framework docs as applicable.
- Project references for packet format or telemetry behavior.

## Boundary

Source facts can identify chips, buses, pins, examples, and setup steps. They do
not prove a radio link, a GNSS fix, antenna quality, regional compliance, or
field behavior. Those need runtime evidence from the target environment.

Keep route output compact. Expand this reference or the generated
`playbook-radio-gnss` only for implementation, setup, or debug tasks.
