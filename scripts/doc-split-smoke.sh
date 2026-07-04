#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  echo "doc-split-smoke FAIL: $1" >&2
  exit 1
}

TRACKED_DOC_FILES="$(git ls-files 'doc/**' || true)"
if [[ -n "$TRACKED_DOC_FILES" ]]; then
  echo "$TRACKED_DOC_FILES" >&2
  fail "public runtime repo must not track singular doc/ files"
fi

for file in \
  data/references/source-intake/manifest.md \
  data/references/source-intake/raw/lilygo-repos.json.gz \
  data/references/source-intake/raw/wiki-products.json \
  data/references/source-intake/auxiliary-skill-references.md
do
  [[ -f "$file" ]] || fail "missing runtime source data: $file"
done

OLD_SOURCE_PATH="$(printf '%s/%s/%s' doc references source-intake)"
PUBLIC_SCAN=(
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
  AGENTS.md
  CLAUDE.md
  .github
)

OLD_PATH_MATCHES="$(git grep -n "$OLD_SOURCE_PATH" -- "${PUBLIC_SCAN[@]}" ":(exclude)scripts/doc-split-smoke.sh" || true)"
if [[ -n "$OLD_PATH_MATCHES" ]]; then
  echo "$OLD_PATH_MATCHES" >&2
  fail "old source-intake path is still referenced"
fi

PRIVATE_SOURCE_PATTERN="$(printf '%s|%s|%s|%s' 'Co''nol' 'co''nol' 'private origin' 'internal note')"
PRIVATE_MATCHES="$(git grep -n -E "$PRIVATE_SOURCE_PATTERN" -- "${PUBLIC_SCAN[@]}" ":(exclude)scripts/doc-split-smoke.sh" || true)"
if [[ -n "$PRIVATE_MATCHES" ]]; then
  echo "$PRIVATE_MATCHES" >&2
  fail "private design input leaked into public runtime surface"
fi

echo "{\"status\":\"PASS\",\"public_doc_files\":0,\"source_intake\":\"data/references/source-intake\"}"
