#!/usr/bin/env bash
# Start the build under test, hidden, so the gate's idle-footprint step has something to measure.
# `open -g -j` never brings it to the front; the app is a menu-bar agent with no window of its own.
set -uo pipefail
cd "$(dirname "$0")/.."
BUNDLE="src-tauri/target/release/bundle/macos/Displays Studio KH.app"
[ -d "$BUNDLE" ] || { echo "idle-launch: no bundle at $BUNDLE"; exit 1; }
open -g -j "$BUNDLE"
for _ in $(seq 1 100); do
  n=$(pgrep -f "Displays Studio KH.app/Contents/MacOS/displays-studio-kh" | wc -l | tr -d ' ')
  [ "$n" = "1" ] && { echo "idle-launch: running, pid $(pgrep -f 'Displays Studio KH.app/Contents/MacOS/displays-studio-kh' | head -1)"; exit 0; }
  [ "$n" -gt 1 ] && { echo "idle-launch: $n instances running; refusing to pick one"; exit 1; }
  sleep 0.2
done
echo "idle-launch: the app did not start"
exit 1
