# Changelog

## 0.1.0 - 2026-07-02

- Added the LilyGO Skill runtime with a Rust CLI, meta Skill entry, generated
  runtime skills, source facts, setup planning, goal planning, and benchmark
  gates.
- Kept the public source tree meta-only: generated board, framework,
  peripheral, chip, feature, playbook, and app skills are runtime artifacts
  produced from `data/**`, `skills/references/**`, and `templates/skills/**`.
- Limited runnable guidance to LilyGO products in the ESP32 family until other
  families have their own design and verification evidence.
- Added release-hardening checks for mixed ASCII/CJK prompts, T-Display-S3
  display source completeness, and build-capable installation.
