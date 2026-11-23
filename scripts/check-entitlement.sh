#!/usr/bin/env bash
# Assert the app inside the shipped DMG carries the entitlement that lets its own framework load.
#
#   scripts/check-entitlement.sh <dmg>
#
# This is only the artifact-interior assertion that belongs to this app: the staging app proving it is
# not the shipped one. The launch test itself is Release Tooling's `install-launch-check.sh`, used from
# the gate rather than reimplemented here — 0.1.8's fault lived in the packaged pair, so a check that
# runs anything but the released image proves nothing.
set -euo pipefail
cd "$(dirname "$0")/.."

DMG="${1:?usage: check-entitlement.sh <dmg>}"
[ -f "$DMG" ] || { echo "check-entitlement: no such DMG: $DMG" >&2; exit 2; }
MOUNT="$(mktemp -d "${TMPDIR:-/tmp}/kh-entitlement.XXXXXX")"
trap 'hdiutil detach "$MOUNT" >/dev/null 2>&1 || true; rm -rf "$MOUNT"' EXIT
hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT" "$DMG" >/dev/null
APP="$MOUNT/Displays Studio KH.app"
[ -d "$APP" ] || { echo "the DMG carries no Displays Studio KH.app" >&2; exit 1; }
codesign -d --entitlements :- "$APP" 2>/dev/null \
  | grep -q 'com.apple.security.cs.disable-library-validation' \
  || { echo "the shipped app lacks com.apple.security.cs.disable-library-validation" >&2; exit 1; }
echo "check-entitlement: the app inside the DMG carries com.apple.security.cs.disable-library-validation"
