# Auxiliary Skill And Tool References

## Purpose

These references are useful for LilyGO development workflows, but they are not
board-fact authorities. They can become auxiliary skills, evidence adapters, or
source pointers for installation/debugging guidance.

## Classification

| Class | Use | Injection Rule |
|-------|-----|----------------|
| Official toolchain docs | Framework setup, compile, upload, monitor, and source authority. | Can be referenced from framework skills. |
| Local installed skills | Known operating rules for Codex/Claude workflows. | Can seed auxiliary skill design and safety rules. |
| Community MCP projects | Patterns for AI-accessible embedded tooling. | Reference only until reviewed and wrapped by our verifier. |
| Evidence adapters | Tools that can produce serial, flash, simulator, or hardware evidence. | Only injected for debug/verify prompts or V4/V5 evidence flows. |

## Candidate References

| Candidate | Type | Source | LilyGO Use | Trust Level |
|-----------|------|--------|------------|-------------|
| Arduino CLI | Official CLI/docs | https://docs.arduino.cc/arduino-cli/ | `fw-arduino` install/build/upload guidance and possible Arduino evidence checks. | Primary |
| Arduino CLI GitHub | Official source | https://github.com/arduino/arduino-cli | Source pointer for CLI behavior and releases. | Primary |
| Arduino MCP servers | Community MCP | https://github.com/Volt23/mcp-arduino-server, https://github.com/niradler/arduino-mcp, https://github.com/oliver0804/arduino-cli-mcp | Reference patterns for wrapping `arduino-cli`; not a default dependency. | Reference |
| PlatformIO Core | Official CLI/docs | https://docs.platformio.org/en/latest/core/index.html | PlatformIO build/upload/monitor guidance when a LilyGO example uses `platformio.ini`. | Primary |
| PlatformIO MCP | Community MCP/CLI adapter | https://github.com/jl-codes/platformio-mcp | Reference pattern for agent-first PlatformIO workflows and task orchestration. | Reference |
| Espressif Documentation MCP | Official MCP service | https://mcp.espressif.com/ | Source lookup adapter for ESP-IDF, Arduino-ESP32, chip docs, and migration guidance. | Primary |
| ESP-IDF Tools MCP | Official/ESP-IDF workflow | https://developer.espressif.com/blog/2026/04/esp-idf-tools-mcp-server/ | Candidate evidence adapter for `idf.py` build/flash/clean workflows in ESP-IDF projects. | Primary |
| ESP-IDF community MCP | Community MCP | https://github.com/horw/esp-mcp | Reference for ESP-IDF build/flash issue workflows; keep separate from official docs. | Reference |
| serial-debug skill | Installed skill + public project | `serial-debug`; `https://github.com/Adancurusul/serial-mcp-server` | Seed `debug-flash-serial` and serial evidence behavior. | Local reference |
| serial-mcp-server | MCP + CLI | https://github.com/adancurusul/serial-mcp-server | Serial port discovery, probe, read/write, RTS/DTR evidence adapter. | Reference/evidence |
| embedded-debugger skill | Installed skill | `embedded-debugger` | Reference for CLI-first probe/debug safety model, mostly non-ESP32 but useful for evidence semantics. | Local reference |
| embedded-debugger-mcp | MCP + CLI | https://github.com/adancurusul/embedded-debugger-mcp | Reference for probe-rs style hardware evidence and skill packaging. | Reference |
| LVGL PC simulator | Official docs | https://lvgl.io/docs/open/8.3/get-started/platforms/pc-simulator | V4 LVGL simulator evidence path. | Primary |
| LVGL ESP32 MCP simulator | Community MCP | https://github.com/jaklys/Lvgl-mcp-esp32 | Reference for headless LVGL screenshot/widget-tree feedback without hardware. | Reference/evidence |
| LVGL remote display | Community tool | https://github.com/CubeCoders/LVGLRemoteServer | Possible future V4/V5 visual feedback adapter. | Reference |

## Design Rules

- Auxiliary skills are opt-in through route intent: debug, install, monitor,
  flash, source lookup, simulator, or evidence.
- Do not inject community MCP guidance for ordinary board fact prompts.
- Official docs remain the source authority for Arduino, ESP-IDF, PlatformIO,
  LVGL, and chip/framework facts.
- Community MCP projects can influence adapter design only after our CLI
  verifier checks command availability, output schema, and safety boundaries.
- Serial, flash, RTS/DTR, probe, OTA, and simulator actions must report actual
  command evidence before raising the verification level above context injection.

## Proposed Auxiliary Skill IDs

| Skill ID | Trigger Scope | Notes |
|----------|---------------|-------|
| `tool-arduino-cli` | Arduino install, compile, upload, board manager, library manager. | Primary source is official Arduino CLI docs. |
| `tool-platformio-cli` | `platformio.ini`, `pio run`, upload, monitor, board envs. | Primary source is PlatformIO docs. |
| `tool-espressif-doc-mcp` | ESP-IDF docs search, chip docs, official source lookup. | Wraps official Espressif MCP as optional source adapter. |
| `tool-serial-debug` | serial port, boot log, probe, RTS/DTR, monitor, read/write. | Based on local `serial-debug` and serial-mcp-server patterns. |
| `tool-embedded-debugger` | probe, flash/program, RTT, memory inspection. | Reference only for ESP32 first pass unless supported probes are configured. |
| `tool-lvgl-simulator` | LVGL screenshot, widget tree, page-data, simulator smoke. | Evidence adapter for V4 simulator checks. |

## Search Evidence

Discovery used local skill search plus web/GitHub searches for Arduino MCP,
PlatformIO MCP, ESP-IDF MCP, LVGL MCP/simulator, and serial/embedded debugger
MCP projects on 2026-06-29.
