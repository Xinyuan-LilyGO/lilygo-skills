# Source Intake Manifest

## Captured Artifacts

| Artifact | Path | Source | Captured | SHA256 |
|----------|------|--------|----------|--------|
| LilyGO GitHub org metadata | `data/references/source-intake/raw/lilygo-repos.json.gz` | `gh api /orgs/Xinyuan-LilyGO/repos` | 2026-06-29 | `5ddf40559570787a7872fb79d87e6a6c3e905f21d1eacfda6ddd21875aa3f3aa` |
| LilyGO Wiki product index | `data/references/source-intake/raw/wiki-products.json` | `https://wiki.lilygo.cc/products/` | 2026-06-29 | `f24880fb9e4f84208b5a2d82fb1e434a32247a26fac450e435cec0647ef2e953` |
| Auxiliary skill/tool references | `data/references/source-intake/auxiliary-skill-references.md` | local skill search + public GitHub/official docs | 2026-06-29 | tracked markdown |

## Current Source Status

| Source Family | Local Status | Design Use | Next Verification |
|---------------|--------------|------------|-------------------|
| LilyGO GitHub org | Organization repository metadata is stored locally. | Repo names, stars, update times, and official source URLs seed board/repo matching. | `sync-boards --dry-run --json` reports repo count and candidate count. |
| LilyGO Wiki | Product index is cached locally with hashes; board records use resolved product-page URLs where available. | Maps board records to official Wiki product pages while keeping pin-level content as on-demand source lookup. | `update sources --json` refreshes `wiki-products.json`; `sync-boards --json` applies cached Wiki URLs to board data. |
| LilyGO documentation repo | `https://github.com/Xinyuan-LilyGO/documentation` is the official versioned source for `https://wiki.lilygo.cc/`; not yet mirrored locally. | High-authority next source for product pages, per-language docs, assets, and product catalog extraction. | Add a source adapter that fetches tree metadata and hashes product docs before treating repo content as cached facts. |
| Espressif and LVGL docs | Official URLs are captured in design docs, not mirrored locally. | Framework skill source authority. | Framework skills must point to versioned official URLs and avoid stale copied content. |
| Auxiliary skill/tool refs | Candidate official docs, local skills, and community MCP projects are cataloged in `auxiliary-skill-references.md`. | Optional helper skills and evidence adapters for install/debug/serial/simulator flows. | Verify command availability and output schema before raising trust above reference. |

## Local Reference Sources

| Source | Path | Intended Skill Use |
|--------|------|--------------------|

## Auxiliary Reference Sources

| Source | Path / URL | Intended Skill Use |
|--------|------------|--------------------|
| serial-debug skill | `serial-debug` plus `https://github.com/Adancurusul/serial-mcp-server` | `tool-serial-debug`, `debug-flash-serial`, serial evidence |
| embedded-debugger skill | `embedded-debugger` | `tool-embedded-debugger`, hardware evidence safety model |
| serial-mcp-server | `https://github.com/adancurusul/serial-mcp-server` | serial probe/read/write/RTS-DTR evidence adapter |
| Arduino CLI docs | `https://docs.arduino.cc/arduino-cli/` | `tool-arduino-cli`, `fw-arduino` installation/build/upload guidance |
| PlatformIO Core docs | `https://docs.platformio.org/en/latest/core/index.html` | `tool-platformio-cli`, PlatformIO environment guidance |
| Espressif Documentation MCP | `https://mcp.espressif.com/` | optional official source lookup adapter for ESP-IDF docs |
| LVGL PC simulator docs | `https://lvgl.io/docs/open/8.3/get-started/platforms/pc-simulator` | `tool-lvgl-simulator`, V4 render evidence |

## Use Rules

- Treat official LilyGO, Espressif, and LVGL sources as primary authority.
- Treat `Xinyuan-LilyGO/documentation` as the versioned documentation source for the public LilyGO Wiki; prefer it over HTML scraping once a hashed source adapter exists.
- Runtime generated skills may point to source artifacts, but should not inject raw artifacts wholesale.
- Refresh live LilyGO GitHub/Wiki facts during `sync-boards` when network and rate limits allow.
- Until raw Wiki pages are cached and hashed, do not claim that official Wiki
  product pages have been fully mirrored locally.
- Treat auxiliary MCP/community projects as reference patterns until this repo wraps and verifies their command behavior.
