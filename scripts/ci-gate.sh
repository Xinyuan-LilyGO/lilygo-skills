#!/usr/bin/env bash
set -euo pipefail

# Aggregated deterministic gate: every hardware-free smoke runs here so a
# HEAD-failing smoke can never ride a green pipeline. Hardware-dependent
# smokes participate via their --dry-run planning paths.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GATES=(
  "capsule-byte-diff-smoke.sh"
  "cjk-prompt-smoke.sh"
  "doc-split-smoke.sh"
  "goal-context-smoke.sh --dry-run"
  "board-completeness-smoke.sh --dry-run"
  "board-data-expansion-smoke.sh"
  "../pipeline/run-official-source-pipeline.js --gold-only --json"
  "../pipeline/diff-gold-fact-packs.js --json"
  "../pipeline/run-official-source-pipeline.js --all-boards --json"
  "../eval/run-board-triple-questions.js --boards all --json --require-topic board-t-watch-s3:display --require-topic board-t-watch-s3:input"
  "../eval/verify-provenance.js --json"
  "pure-query-compact-smoke.sh"
  "context-budget-smoke.sh"
  "project-context-smoke.sh --dry-run"
  "playbook-quality-smoke.sh --dry-run"
  "preference-reference-smoke.sh --dry-run"
  "setup-plan-smoke.sh --dry-run"
  "source-completeness-smoke.sh --dry-run"
  "source-fact-smoke.sh --dry-run"
  "source-recovery-smoke.sh"
  "demo-intent-smoke.sh"
  "bus-facts-smoke.sh --dry-run"
  "next-actions-smoke.sh --dry-run"
  "project-custom-skills-smoke.sh"
  "doctor-smoke.sh"
  "code-size-boundary-smoke.sh"
  "rust-module-doc-smoke.sh"
  "router-skill-surface-smoke.sh"
  "source-comment-hygiene-smoke.sh"
  "install-build-failure-smoke.sh"
  "install-binary-selection-smoke.sh"
  "install-injection-smoke.sh"
  "practice-layer-free-smoke.sh"
  "system-smoke.sh"
  "scorecard-private-boundary-smoke.sh"
)

failed=()
skipped=()
for gate in "${GATES[@]}"; do
  # shellcheck disable=SC2086
  set -- $gate
  script="scripts/$1"
  shift
  echo "== ci-gate: $script $* =="
  set +e
  if [[ "$script" == scripts/../pipeline/*.js || "$script" == scripts/../eval/*.js ]]; then
    node "${script#scripts/../}" "$@"
  else
    bash "$script" "$@"
  fi
  code=$?
  set -e
  if [[ "$code" -eq 2 && "$script" =~ hardware|goal-hardware ]]; then
    skipped+=("$script")
    continue
  fi
  if [[ "$code" -ne 0 ]]; then
    failed+=("$script")
  fi
done

echo "== ci-gate: eval/run-scorecard.js --suite smoke --json =="
node eval/run-scorecard.js --suite smoke --json

echo "== ci-gate: eval/grade-scorecard.js --assert-forbidden-claims --json =="
node eval/grade-scorecard.js --assert-forbidden-claims --json

if [[ ${#failed[@]} -gt 0 ]]; then
  echo "FAIL ci-gate: ${failed[*]}" >&2
  exit 1
fi

echo "{\"status\":\"PASS\",\"gates\":${#GATES[@]},\"boundary_skips\":${#skipped[@]}}"
