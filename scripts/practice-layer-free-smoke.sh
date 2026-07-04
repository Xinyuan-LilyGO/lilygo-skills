#!/usr/bin/env bash
set -euo pipefail

# Asserts the public product surfaces carry no reference to the removed
# private practice layer. The pattern is assembled from fragments so this
# gate never matches its own content, and the gate file excludes itself.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PATTERN="$(printf '%s%s' 'vil' 'ya')"
SELF="scripts/$(basename "${BASH_SOURCE[0]}")"

SURFACES=(
  crates
  data
  index
  skills
  templates
  docs
  schemas
  scripts
  install.js
  README.md
  README.zh-CN.md
  ARCHITECTURE.md
  ARCHITECTURE.zh-CN.md
  CHANGELOG.md
  Cargo.toml
  data/references/source-intake
)

EXISTING=()
for surface in "${SURFACES[@]}"; do
  [[ -e "$surface" ]] && EXISTING+=("$surface")
done

MATCHES="$(git grep -I -i -l "$PATTERN" -- "${EXISTING[@]}" ":(exclude)$SELF" || true)"

if [[ -n "$MATCHES" ]]; then
  echo "FAIL practice-layer references found in product surfaces:" >&2
  echo "$MATCHES" >&2
  exit 1
fi

echo "{\"status\":\"PASS\",\"surfaces_checked\":${#EXISTING[@]},\"pattern\":\"<private-practice-layer>\"}"
