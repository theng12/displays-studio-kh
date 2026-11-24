#!/usr/bin/env bash
# §5d screenshots, taken from the build being released — never from an older one.
#
# The app captures its own webviews (§5d: menu-bar screens come from the hidden window, never the
# desktop) in an isolated data dir with invented displays, so nothing real reaches the pixels.
# Run this inside the release, then check-app-screenshots.sh proves the set.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
OUT=$ROOT/docs/screenshots

(cd src-tauri && cargo build --offline --locked --features custom-protocol,webview-check)

D=$(mktemp -d "${TMPDIR:-/tmp}/displays-shots-XXXXXX")
trap 'rm -rf "$D"' EXIT

# The test binary is not in a bundle, so point dyld at the vendored framework; a token override keeps
# the capture from touching the real hold-token file.
DYLD_FRAMEWORK_PATH="$ROOT/src-tauri/vendor/khsparkle" \
  KHU_HOLD_TOKEN_FILE="$D/token" \
  DISPLAYS_STUDIO_HOLD_PORT=${DISPLAYS_STUDIO_HOLD_PORT:-18677} \
  "$ROOT/src-tauri/target/debug/displays-studio-kh" --snapshot "$D"

mkdir -p "$OUT"
for name in panel settings; do
  for tone in light dark; do
    sips -s format png "$D/$name-$tone.tiff" --out "$OUT/$name-$tone.png" >/dev/null
  done
done
ls -l "$OUT"/panel-*.png "$OUT"/settings-*.png
