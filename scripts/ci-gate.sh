#!/usr/bin/env bash
set -uo pipefail

# Aggregated deterministic gate for the JS context kernel. Every language-free
# data/pipeline/eval check runs here alongside the JS core gates (unit + CLI
# contract + hook value-alignment, typecheck, doctor, live source verify) and
# the install->hook integration smoke, so a HEAD-failing check can never ride a
# green pipeline. The former Rust-binary/Rust-source smokes are retired with the
# Rust tree (kept on the dedicated rust-archive git branch); their behavioral
# equivalence for the AI-facing surface (context / source query / verify sources
# / doctor / hook) is covered by `npm test` + `tsc` + the doctor/verify gates.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

failed=()

run_gate() {
  local label="$1"; shift
  echo "== ci-gate: $label =="
  if "$@"; then
    return 0
  fi
  failed+=("$label")
}

# --- JS core: unit + contract + typecheck + doctor + live verify ------------
# `npm test` runs the node:test suite: CLI contract parity (source query /
# context / verify / doctor exit codes + shapes + anti-fabrication value
# parity), hook thick-capsule value-alignment (JS == frozen Rust reference,
# value-for-value), and CJK routing.
run_gate "npm test" npm test --silent
run_gate "tsc --noEmit" npx tsc -p tsconfig.json --noEmit
run_gate "doctor --json" node bin/lilygo-skills.mjs doctor --json
# Live source re-proof: OK when the network confirms hashes, graceful
# UNREACHABLE (still exit 0) when offline/rate-limited; only a real DRIFT fails.
run_gate "verify sources (t-connect-pro)" \
  node bin/lilygo-skills.mjs verify sources --board board-t-connect-pro --topic pinout --json

# --- data / pipeline / provenance (language-independent) --------------------
run_gate "official source pipeline (gold)" \
  node pipeline/run-official-source-pipeline.js --gold-only --json
run_gate "diff gold fact packs" \
  node pipeline/diff-gold-fact-packs.js --json
run_gate "official source pipeline (all boards)" \
  node pipeline/run-official-source-pipeline.js --all-boards --json
run_gate "board triple questions" \
  node eval/run-board-triple-questions.js --boards all --json \
  --require-topic board-t-watch-s3:display --require-topic board-t-watch-s3:input
run_gate "verify provenance" \
  node eval/verify-provenance.js --json

# --- doc / surface / hygiene smokes (language-independent) -------------------
run_gate "doc-split" bash scripts/doc-split-smoke.sh
run_gate "router-skill-surface" bash scripts/router-skill-surface-smoke.sh
run_gate "practice-layer-free" bash scripts/practice-layer-free-smoke.sh
run_gate "source-comment-hygiene" bash scripts/source-comment-hygiene-smoke.sh

# --- install -> hook integration + scorecard boundary -----------------------
run_gate "install-injection" bash scripts/install-injection-smoke.sh
run_gate "scorecard-private-boundary" bash scripts/scorecard-private-boundary-smoke.sh

# --- scorecard smoke suite --------------------------------------------------
run_gate "run-scorecard (smoke)" node eval/run-scorecard.js --suite smoke --json
run_gate "grade-scorecard (forbidden-claims)" \
  node eval/grade-scorecard.js --assert-forbidden-claims --json

GATES=17
if [[ ${#failed[@]} -gt 0 ]]; then
  echo "FAIL ci-gate (${#failed[@]}/${GATES}): ${failed[*]}" >&2
  exit 1
fi

echo "{\"status\":\"PASS\",\"gates\":${GATES},\"baseline\":\"js\"}"
