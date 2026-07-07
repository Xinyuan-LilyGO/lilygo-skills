# Changelog

## Unreleased

- Added project-local context memory through `.lilygo-skills/project.json`,
  project ledger digests, stale detection, and compact repeat injection for
  firmware repositories.
- Added the official-source pipeline, byte-for-byte capsule gates, board
  triple-question evaluation, private scorecard fail-closed checks, and a
  four-operation AI-facing router surface.
- Improved context budget behavior: lookup hooks stay compact, implementation
  hooks expose next actions, and same-session repeated context collapses to an
  incremental capsule while preserving critical facts and expansion commands.
- Added source-backed T-Watch S3 display and touch facts, Chinese screen/touch
  routing, and narrow display/input source-query expansion.
- Aligned README and architecture docs around the same L0-L14 progressive
  disclosure model.
- Preserved the verification boundary: these updates are source/context,
  install, generation, and evaluation evidence unless a separate V4/V5 hardware
  artifact is attached.

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
