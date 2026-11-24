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
REQUIRED="2bce760e7749a0f21dfa378c53c391d039401355012e774a81b691043a3cb17f"
WANT="${1:-$REQUIRED}"

[ -f "$LIB" ] || { echo "shim: MISSING $LIB"; exit 1; }
GOT=$(shasum -a 256 "$LIB" | cut -d' ' -f1)
echo "shim: $LIB $(stat -f %z "$LIB") bytes sha256 $GOT"
if [ "$GOT" != "$WANT" ]; then
  echo "shim: FAIL — required $WANT"
  exit 1
fi
echo "shim: PASS"