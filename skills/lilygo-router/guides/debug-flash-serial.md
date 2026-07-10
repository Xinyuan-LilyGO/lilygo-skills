# Debug: build → upload → monitor, in bounded steps

Goal: run a bounded build/upload/monitor loop with explicit permissions, bounded
logs, and honest failure classification. This is the backbone loop every other
hardware guide leans on.

Pull facts first ([[query-protocol]]) to know the board target/FQBN and the
serial observation method. Confirm the framework the project already uses
(Arduino / PlatformIO / ESP-IDF / Rust esp-rs) — ask if none is known, never
silently pick one. Toolchain readiness lives in [[toolchain-setup]].

## The loop

1. **Plan the commands first, keep them dry-run** unless the user has granted
   explicit build/flash/serial permission.
2. **Check the toolchain** (framework version command) before building.
3. **Build before upload**, preserving the exact framework target. A build proves
   compilation only.
4. **Upload only to an explicitly selected port** (`--port`). Never open or flash
   a port the user did not name.
5. **Monitor**: open the selected port only after permission, at a known baud,
   and capture a **bounded** excerpt. Record baud and reset policy in local
   evidence.
6. **Record each step at its true level**: build exit code, upload exit code, a
   bounded serial excerpt — nothing more.

Keep large/raw logs, private serial ports, Wi-Fi details, hosts, and credentials
in ignored local evidence, not in the public answer.

## Failure buckets

Sort a failed run into a bucket rather than retrying blind:

- **missing-tool-or-source** (`command not found`, `no such file`) — install the
  missing tool or bind a real source checkout before retrying. See
  [[toolchain-setup]].
- **port-or-permission** (`failed to connect`, `permission denied`, `no serial`) —
  select an explicit port and fix OS permissions before hardware steps.
- **runtime-timeout-no-observation** (`timeout`, `no data`, no output) — add
  explicit firmware boot/status markers or pick a smaller observable demo, then
  rerun. Do not claim behavior you could not observe.
- **ota** (manifest / partition / digest / rollback) — this is a distinct path;
  see the OTA notes below.
- **build-failure** (`compile`, `undefined reference`, `error:`) — patch the
  project and rerun the same build recipe once.

Repeated identical failures route to problem-solving, not another blind retry.

## OTA is a project workflow, not a generic command

OTA has its own failure bucket: partition table, manifest URL/size/version,
digest mismatch, transport reachability, reboot, and rollback. Read the framework
OTA docs and the project partition table first; validate the manifest fields and
digest; then resolve the project's OTA runner from its manifests, scripts, and
references (or ignored local state) — ask only for the private endpoint or
credential that cannot be inferred. Observe reboot/rollback through serial or a
project status endpoint. Keep Wi-Fi credentials, LAN hosts, OTA URLs, and private
runner argv in local evidence (e.g. `.lilygo-skills/local.json`).

## Honesty

A generated command cannot prove flash or OTA success until it runs and is
observed. A build exit code is not a flash; a flash is not a working peripheral.
`hardware_verified=false` means not hardware-confirmed. See [[honesty-evidence]].
