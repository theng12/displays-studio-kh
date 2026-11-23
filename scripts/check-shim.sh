#!/usr/bin/env bash
# Assert the vendored update library is the one this release requires.
#
#   bash scripts/check-shim.sh [expected-sha256]
#
# The library is a build input the scanner cannot judge: it is not inside the DMG, so a clean scan
# says nothing about which shim a build carries. Called from build-mac.sh, so every gate's preflight
# prints this line and the receipt can cite it instead of letting the scan verdict imply it.
set -uo pipefail
cd "$(dirname "$0")/.."

LIB=src-tauri/vendor/khsparkle/libkhsparkle.a
REQUIRED="818826a79d4e54885cbb5ad167f7425da8ba77436c6a5e277532f400aa1b4216"
WANT="${1:-$REQUIRED}"

[ -f "$LIB" ] || { echo "shim: MISSING $LIB"; exit 1; }
GOT=$(shasum -a 256 "$LIB" | cut -d' ' -f1)
echo "shim: $LIB $(stat -f %z "$LIB") bytes sha256 $GOT"
if [ "$GOT" != "$WANT" ]; then
  echo "shim: FAIL — required $WANT"
  exit 1
fi
echo "shim: PASS"