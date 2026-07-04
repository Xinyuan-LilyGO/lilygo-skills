#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_HOME="$(mktemp -d)"
TMP_BIN="$(mktemp -d)"
OUT="$ROOT/.tmp/install-build-failure.json"

cleanup() {
  rm -rf "$TMP_HOME" "$TMP_BIN"
}
trap cleanup EXIT

mkdir -p "$ROOT/.tmp"
cat > "$TMP_BIN/cargo" <<'SH'
#!/usr/bin/env bash
echo "intentional cargo failure for install build smoke" >&2
exit 42
SH
chmod +x "$TMP_BIN/cargo"

set +e
PATH="$TMP_BIN:$PATH" node "$ROOT/install.js" --all --home "$TMP_HOME" --build > "$OUT"
STATUS=$?
set -e

if [ "$STATUS" -eq 0 ]; then
  echo "expected install.js --build to fail when cargo fails" >&2
  exit 1
fi

BUILD_STATUS="$(jq -r '.build_result.status' "$OUT")"
WRITE_COUNT="$(jq '.writes | length' "$OUT")"
VERIFIED_COUNT="$(jq '.verified_writes | length' "$OUT")"
ERROR_COUNT="$(jq '.errors | length' "$OUT")"

if [ "$BUILD_STATUS" != "FAIL" ]; then
  echo "expected build_result.status=FAIL, got $BUILD_STATUS" >&2
  exit 1
fi
if [ "$WRITE_COUNT" -ne 0 ] || [ "$VERIFIED_COUNT" -ne 0 ]; then
  echo "build failure must not install stale binaries or support files" >&2
  exit 1
fi
if [ "$ERROR_COUNT" -eq 0 ]; then
  echo "build failure should report an error" >&2
  exit 1
fi
if find "$TMP_HOME" -mindepth 1 -print -quit | grep -q .; then
  echo "build failure left files under the install home" >&2
  exit 1
fi

jq '{status, build_result, writes, verified_writes, errors}' "$OUT"
