#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SKILL="$ROOT/skills/lilygo-router/SKILL.md"

required=(
  "capsule auto-injection"
  "lilygo-skills source query"
  "lilygo-skills goal complete"
  "lilygo-skills goal plan"
  "lilygo-skills update board-facts"
)

for pattern in "${required[@]}"; do
  if ! grep -q "$pattern" "$SKILL"; then
    echo "missing AI-facing surface: $pattern" >&2
    exit 1
  fi
done

forbidden=(
  "lilygo-skills route"
  "lilygo-skills index"
  "lilygo-skills generate"
  "lilygo-skills verify"
  "lilygo-skills benchmark"
  "lilygo-skills setup"
  "lilygo-skills preference"
  "lilygo-skills reference"
  "lilygo-skills source completeness"
)

for pattern in "${forbidden[@]}"; do
  if grep -q "$pattern" "$SKILL"; then
    echo "maintainer command leaked into router skill surface: $pattern" >&2
    exit 1
  fi
done

echo '{"status":"PASS","surfaces":4,"maintainer_commands_hidden":true}'
