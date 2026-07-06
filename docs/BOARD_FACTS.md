# Board Facts

Chinese version: [BOARD_FACTS.zh-CN.md](BOARD_FACTS.zh-CN.md).

Board facts are source-backed claims that an AI can use before writing firmware
or debug steps. They are separate from route skills so the default context stays
compact.

## Where Facts Live

```text
data/facts/board-fact-packs.json
```

Each pack is keyed by `board_id` and can contain:

- `mcu_family`
- `supported`
- `pin_matrix`
- `bus_matrix`
- `expander_matrix`
- `connector_matrix`
- `peripheral_table`
- `source_refs`
- `conflicts`

Each fact carries a claim, value, topic, source kind, source URL or portable
reference, source hash, authority rank, evidence level, stale flag, and
confidence.

## Querying Facts

```bash
cargo run -p lilygo-skills-cli -- source query --board board-t-watch-ultra --topic io --json
cargo run -p lilygo-skills-cli -- source query --board board-t-watch-ultra --topic expander --json
cargo run -p lilygo-skills-cli -- source query --board board-t-watch-ultra --topic peripheral --json
lilygo-skills source query --board board-t-display-s3 --topic i2c --json
```

Valid topics include IO, pinout, bus, expander, connector, peripheral, display,
IMU, power, LoRa, GNSS, and input topics exposed by the CLI.

For T-Display-S3, the I2C topic returns official factory `pin_config.h` facts
such as `PIN_IIC_SDA=GPIO18` and `PIN_IIC_SCL=GPIO17`.

## Confidence Values

- `exact`: the value is directly source-backed.
- `derived`: the value is derived from source-backed metadata.
- `unknown_with_sources`: current sources prove that the topic exists or is
  relevant, but they do not prove the exact value.

`unknown_with_sources` is intentional. It is safer than inventing free GPIOs,
expander channels, or bus wiring.

## Completeness

Completeness is evaluated per board and topic:

```bash
cargo run -p lilygo-skills-cli -- source completeness --board board-t-display-s3 --topic display --json
```

Statuses:

- `complete`: enough facts and refs exist for the quick-start contract.
- `partial`: some facts exist, but key required facts are missing.
- `needs_source_ingestion`: supported board/topic, but the local fact pack needs
  enrichment from official sources.
- `unsupported`: outside the supported board/topic boundary.

Route, hook, and goal plan can expose compact readiness status. They must not
write fact packs.

## Enrichment

Use explicit update commands for enrichment:

```bash
cargo run -p lilygo-skills-cli -- update board-facts \
  --board board-t-display-s3 \
  --topic display \
  --dry-run \
  --json
```

Dry-run output should include:

- Source adapters.
- Planned reads.
- Planned writes.
- Parsed facts and unknowns.
- Source hashes.
- Validation status.
- Follow-up commands.

Removing `--dry-run` applies only supported, validated writes. Unsupported
boards or targets outside the current LilyGO support scope must fail closed
without mutating fact packs.

## Source Authority

Authority order:

1. Official code, headers, examples, and product repos.
2. Official LilyGO hardware docs.
3. `Xinyuan-LilyGO/documentation`.
4. `wiki.lilygo.cc` fallback pages.
5. Project reference skills.
6. Auxiliary community/tool references.

When sources conflict, higher authority wins and the conflict should remain
visible in the fact pack or source query output.

## Context Budget

Fact packs are not pasted by default. Route and hook output should include:

- Matched skill ids.
- Short summaries.
- Top-ranked facts.
- Overflow counts.
- `source query`, `source completeness`, or `update board-facts` commands.

The AI should call the lookup commands when the user asks for implementation
details, pin assignment, peripheral behavior, or debugging.

## Privacy Boundary

Board facts are public source facts. They must not contain local serial ports,
Wi-Fi values, OTA hosts, local log paths, or private machine evidence. Those
belong in ignored project-local state and should not be injected into public
context.
