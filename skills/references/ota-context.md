# OTA Context

OTA is a project workflow. Do not treat it as a generic binary or a board
peripheral. Resolve it from the firmware project, framework, manifest, partition
layout, transport, reboot behavior, rollback policy, and local evidence setup.

## Read First

- Project build scripts and firmware manifests.
- Partition table and rollback configuration.
- Framework OTA examples and docs.
- Project references that describe deployment format or transport.
- Ignored local runner settings if the project already has them.

## Execution Shape

Use `goal plan` first. Actual OTA execution requires explicit network and OTA
permission plus project-local runner details. Public context may describe the
shape of the runner, but raw local outputs and private network values must stay
in ignored evidence.

## Evidence Boundary

Source and manifest inspection is V3. Build output can reach V4 for compile
evidence. OTA transport, reboot, version observation, rollback, and serial
status need explicit runtime evidence before they are treated as verified.
