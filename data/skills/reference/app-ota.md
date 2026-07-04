# Recipe Context: OTA

Use as source and recipe context for OTA update, firmware manifest, partition,
and rollback prompts. This is not an OTA implementation.

- Pair with framework context and `debug-flash-serial` when planning evidence.
- Check partition table, version metadata, transport, manifest format, rollback policy, and serial logs before implementation.
- Claim OTA success only with V4/V5 evidence such as a build artifact, OTA harness result, live transport result, or device-side confirmation.
