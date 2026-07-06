#!/usr/bin/env bash
set -euo pipefail

# Aggregated deterministic gate: every hardware-free smoke runs here so a
# HEAD-failing smoke can never ride a green pipeline. Hardware-dependent
# smokes participate via their --dry-run planning paths.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GATES=(
  "cjk-prompt-smoke.sh --dry-run"
  "doc-split-smoke.sh"
  "full-evidence-smoke.sh --dry-run"
  "generated-cache-boundary-smoke.sh"
  "goal-privacy-smoke.sh --dry-run"
  "goal-safety-smoke.sh --dry-run"
  "goal-complete-smoke.sh --dry-run"
  "goal-complete-permission-smoke.sh --dry-run"
  "goal-context-smoke.sh --dry-run"
  "goal-hardware-smoke.sh --dry-run"
  "board-completeness-smoke.sh --dry-run"
  "product-board-smoke.sh --dry-run"
  "project-context-smoke.sh --dry-run"
  "playbook-quality-smoke.sh --dry-run"
  "preference-reference-smoke.sh --dry-run"
  "setup-plan-smoke.sh --dry-run"
  "source-completeness-smoke.sh --dry-run"
  "source-fact-smoke.sh --dry-run"
  "source-recovery-smoke.sh"
  "code-size-boundary-smoke.sh"
  "meta-only-source-smoke.sh"
  "rust-module-doc-smoke.sh"
  "source-comment-hygiene-smoke.sh"
  "static-context-template-smoke.sh"
  "install-build-failure-smoke.sh"
  "install-binary-selection-smoke.sh"
  "install-injection-smoke.sh"
  "practice-layer-free-smoke.sh"
  "system-smoke.sh"
)

failed=()
for gate in "${GATES[@]}"; do
  # shellcheck disable=SC2086
  set -- $gate
  script="scripts/$1"
  shift
  echo "== ci-gate: $script $* =="
  if ! bash "$script" "$@"; then
    failed+=("$script")
  fi
done

if [[ ${#failed[@]} -gt 0 ]]; then
  echo "FAIL ci-gate: ${failed[*]}" >&2
  exit 1
fi

echo "{\"status\":\"PASS\",\"gates\":${#GATES[@]}}"
