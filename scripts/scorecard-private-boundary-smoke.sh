#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

set +e
output="$(LILYGO_SKILLS_PRIVATE_EVAL=1 node eval/run-scorecard.js --suite smoke --json 2>&1)"
code=$?
set -e

if [[ "$code" -eq 0 ]]; then
  echo "expected private scorecard without artifacts to fail" >&2
  echo "$output" >&2
  exit 1
fi

if ! grep -q "private_runner_artifacts_required" <<<"$output"; then
  echo "expected private_runner_artifacts_required reason" >&2
  echo "$output" >&2
  exit 1
fi

printf '{"status":"PASS","private_without_artifacts":"FAILS_CLOSED"}\n'
