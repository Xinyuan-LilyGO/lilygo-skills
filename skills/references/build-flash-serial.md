# Build, Flash, And Serial Context

Use this reference when the user asks to build, upload, monitor, debug boot
logs, or collect evidence from a supported LilyGO board.

## Flow

1. Resolve board and framework from prompt, project context, or clarification.
2. Run `setup plan` if the toolchain is missing or uncertain.
3. Use `goal plan` to get the read-only command skeleton and permission list.
4. Build before flashing.
5. Flash only with explicit permission and a selected serial port.
6. Observe serial output only with explicit serial permission.
7. Store raw local evidence under ignored project state.

## Evidence Boundary

- Build output proves source compiles for the selected framework.
- Upload output proves the flashing tool reported success for a target.
- Serial app logs prove only what the firmware actually prints.
- Source links and demo paths do not prove runtime behavior.

When serial output is empty, add or select a smaller observable firmware path
with boot/status markers before rerunning. Do not blindly retry the same command
without a new observation.
