# Source Discovery

Use source discovery whenever a request depends on exact pins, buses, libraries,
examples, build flags, register behavior, or board wiring.

## Authority Order

1. Official LilyGO code, headers, examples, manifests, and board repositories.
2. Official LilyGO hardware docs and schematics.
3. `https://github.com/Xinyuan-LilyGO/documentation` as the versioned wiki source.
4. `wiki.lilygo.cc` fallback pages.
5. Chip-vendor datasheets and official framework docs.
6. Project references as operating guidance.
7. Community tools and examples as hints only.

## Missing Facts

If exact facts are absent, return one of these instead of guessing:

- `unknown_with_sources` when sources exist but do not prove the value;
- `needs_clarification` when user-owned board/framework/private details are missing;
- `unsupported` when the product is outside the current verified runtime support boundary.

## Common Lookups

```bash
lilygo-skills source query --board <board-id> --topic io --json
lilygo-skills source query --board <board-id> --topic display --json
lilygo-skills verify sources --board <board-id> --topic display --json
```

To regenerate the fact pack from official sources when a board/topic has refs but
no facts yet, run the pipeline:

```bash
node pipeline/run-official-source-pipeline.js --all-boards --json
```
