#!/usr/bin/env bash
set -euo pipefail

# Keep the CLI a runtime engine, not an opaque blob of hand-written skill text.
# Non-test production Rust code must stay below an effective code-line limit.
# Blank lines and comments are reported separately so the gate does not reward
# unreadable compression. Benchmark harness code has its own coverage gates and
# is reported outside the production total.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MAX_FILE=800
MAX_TOTAL=11500
while [[ $# -gt 0 ]]; do
  case "$1" in
    --max-production-file-lines) MAX_FILE="$2"; shift 2 ;;
    --max-production-lines) MAX_TOTAL="$2"; shift 2 ;;
    *) echo "unknown arg $1" >&2; exit 2 ;;
  esac
done

SRC_DIR="crates/lilygo-skills-cli/src"

MAX_FILE="$MAX_FILE" MAX_TOTAL="$MAX_TOTAL" SRC_DIR="$SRC_DIR" python3 <<'PY'
import os, re, glob, json, sys

src = os.environ["SRC_DIR"]
max_file = int(os.environ["MAX_FILE"])
max_total = int(os.environ["MAX_TOTAL"])

def production_lines(path):
    lines = open(path, encoding="utf-8").read().split("\n")
    n = len(lines)
    i = 0
    count = 0
    physical = 0
    while i < n:
        if re.search(r'#\[cfg\(test\)\]', lines[i]):
            j = i + 1
            while j < n and 'mod ' not in lines[j]:
                j += 1
            if j < n and '{' in lines[j]:
                bal = lines[j].count('{') - lines[j].count('}')
                j += 1
                while j < n and bal > 0:
                    bal += lines[j].count('{') - lines[j].count('}')
                    j += 1
                i = j
                continue
        physical += 1
        stripped = lines[i].strip()
        if stripped and not stripped.startswith("//"):
            count += 1
        i += 1
    return count, physical

files = sorted(glob.glob(os.path.join(src, "**", "*.rs"), recursive=True))
per = {}
physical_per = {}
total = 0
physical_total = 0
oversized = []
for f in files:
    # Dedicated test files are not production code.
    if f.endswith("tests.rs") or f.endswith("_tests.rs"):
        continue
    if f.endswith("benchmark.rs"):
        continue
    lines, physical = production_lines(f)
    per[f] = lines
    physical_per[f] = physical
    total += lines
    physical_total += physical
    if lines > max_file:
        oversized.append({"file": f, "lines": lines})

status = "PASS" if not oversized and total <= max_total else "FAIL"
report = {
    "status": status,
    "max_production_file_lines": max_file,
    "max_production_lines": max_total,
    "production_total": total,
    "physical_total": physical_total,
    "counting": "nonblank_noncomment_production_lines",
    "file_count": len(per),
    "oversized_files": oversized,
    "largest": sorted(({"file": f, "lines": n} for f, n in per.items()),
                      key=lambda x: -x["lines"])[:8],
    "largest_physical": sorted(({"file": f, "lines": n} for f, n in physical_per.items()),
                               key=lambda x: -x["lines"])[:8],
}
print(json.dumps(report, indent=2))
if status != "PASS":
    if oversized:
        print("production files over limit:", oversized, file=sys.stderr)
    if total > max_total:
        print(f"production total {total} exceeds {max_total}", file=sys.stderr)
    sys.exit(1)
PY
