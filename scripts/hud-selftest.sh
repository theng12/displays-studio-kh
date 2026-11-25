#!/usr/bin/env bash
# Measure the brightness HUD on the built app, offscreen, without touching the desktop.
#
#   scripts/hud-selftest.sh
#
# The app builds its HUD at (-6000,-6000), orders it front, and reports what it measured: the frame
# maths, that it is visible, that it is not key or main, that the app's activation did not change,
# and that it hid itself after the visible period plus the fade. Any FAIL, a missing PASS line or a
# non-zero exit is a failed step — silence is never a pass.
set -uo pipefail
cd "$(dirname "$0")/.."

BIN="src-tauri/target/release/bundle/macos/Displays Studio KH.app/Contents/MacOS/displays-studio-kh"
[ -x "$BIN" ] || { echo "hud-selftest: no built app at $BIN — run scripts/build-mac.sh first"; exit 1; }

OUT=$(DISPLAYS_STUDIO_HUD_SELFTEST=1 "$BIN" 2>&1)
RC=$?
printf '%s\n' "$OUT"

if [ "$RC" -eq 0 ] && printf '%s\n' "$OUT" | grep -q '^HUD SELFTEST: PASS$'; then
  echo "hud-selftest: PASS"
  exit 0
fi
echo "hud-selftest: FAIL (status $RC)"
exit 1
