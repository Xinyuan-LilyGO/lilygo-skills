#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SRC_DIR="crates/lilygo-skills-cli/src"

python3 <<'PY'
import glob
import json
import os
import sys

src = "crates/lilygo-skills-cli/src"
missing = []
checked = []

for path in sorted(glob.glob(os.path.join(src, "**", "*.rs"), recursive=True)):
    if path.endswith("tests.rs") or path.endswith("_tests.rs"):
        continue
    checked.append(path)
    with open(path, encoding="utf-8") as handle:
        lines = handle.readlines()
    first_nonempty = next((line.strip() for line in lines if line.strip()), "")
    if not (first_nonempty.startswith("//!") or first_nonempty.startswith("/*!")):
        missing.append(path)

report = {
    "status": "PASS" if not missing else "FAIL",
    "checked": len(checked),
    "missing": missing,
}
print(json.dumps(report, indent=2))
if missing:
    sys.exit(1)
PY
