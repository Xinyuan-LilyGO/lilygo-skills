# Toolchain setup — report and hint, install only on explicit ask

Goal: get a blank machine ready for LilyGO work by **reporting** what is missing
and how to install it. Setup planning is not installation: run real installers
only when the user explicitly asks, and never silently mutate the machine.

Pick the framework from the project or ask ([[query-protocol]]); do not assume
one. Check prerequisites, report missing tools with their official install hint,
and keep any install logs bounded.

## Host prerequisites (all frameworks)

| Tool | Check | Install hint |
|---|---|---|
| rustup | `rustup --version` | Install from https://rustup.rs/ |
| cargo | `cargo --version` | Installed by rustup; builds `lilygo-skills`. |
| node | `node --version` | Node.js LTS for `install.js` and parity checks. |
| git | `git --version` | For LilyGO / Espressif / reference source checkouts. |
| python3 | `python3 --version` | Required before PlatformIO / ESP-IDF tooling. |

## Framework toolchains

**Arduino** — `arduino-cli version`; `arduino-cli core install esp32:esp32`
(after `core update-index`); LilyGoLib deps per official LilyGO guidance;
`espflash` (via `cargo install espflash`) when flash/serial evidence is needed.

**PlatformIO** — `python3` first, then `pio --version` (install PlatformIO Core
from https://docs.platformio.org/). PlatformIO resolves the `espressif32`
platform from `platformio.ini` or `pio pkg install`.

**ESP-IDF** — `idf.py --version`; install ESP-IDF from the Espressif get-started
docs and use the official install script to provision the compiler, OpenOCD, and
Python environment.

**Rust / esp-rs** — `espup --version` then `espup install`; `espflash --version`
(and `cargo espflash` if used) via `cargo install`.

**Optional serial helper** — `serial-mcp-server` (
https://github.com/Adancurusul/serial-mcp-server) as a bounded serial-observation
loop for monitor output. Optional; note it only if the user wants a serial
debug loop.

## Sequence

1. Check host prerequisites and report what is missing — do not install yet.
2. Confirm the framework (project context or ask).
3. List the exact next commands (dry-run) and any private inputs needed later —
   e.g. a USB serial port is needed only for later flash/monitor, and Wi-Fi / OTA
   targets stay in private local config.
4. Install only after explicit user authorization; keep logs bounded.
5. Record project-local preferences (framework order, debug-tool choice) only
   when the user asks.

After setup, the natural next step is a bounded build/upload/monitor loop —
[[debug-flash-serial]] — then [[board-bringup-checklist]].

## Honesty

Do not silently install Rust, Node, firmware toolchains, or drivers. A setup plan
cannot prove a firmware build until the build command runs. See
[[honesty-evidence]].
