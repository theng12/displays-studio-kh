#!/usr/bin/env bash
# Launch what the DMG actually ships, and assert it survives dyld.
#
#   scripts/launch-check.sh <dmg> <expected version>
#
# Why this exists: 0.1.8's gate was green — clean scan, `--deep --strict` exit 0, framework in place —
# and the app died in one second on a real install, because hardened runtime's library validation read
# this self-signed app and its self-signed framework as different teams ("different Team IDs", SIGABRT in
# dyld::prepare). Deep verification checks the seal, not whether the process can load its own framework.
# So the gate launches the artifact, and a death here FAILS the run.
#
# It is a menu-bar app: for the ten seconds of the check one icon appears in the menu bar. Nothing is
# activated, no window is created, no input is sent, and the process is killed the moment it is asserted.
# It runs with its own HOME and its own hold port, so it touches none of the owner's preferences, login
# items or the live 8650 listener.
set -euo pipefail
cd "$(dirname "$0")/.."

DMG="${1:?usage: launch-check.sh <dmg> <expected version>}"
WANT="${2:?usage: launch-check.sh <dmg> <expected version>}"
[ -f "$DMG" ] || { echo "launch-check: no such DMG: $DMG" >&2; exit 2; }
fail() { printf 'launch-check: FAILED: %s\n' "$1" >&2; exit 1; }

MOUNT="$(mktemp -d "${TMPDIR:-/tmp}/kh-launch-mnt.XXXXXX")"
RUN="$(mktemp -d "${TMPDIR:-/tmp}/kh-launch-run.XXXXXX")"
PID=""
cleanup() {
  [ -n "$PID" ] && kill "$PID" 2>/dev/null || true
  hdiutil detach "$MOUNT" >/dev/null 2>&1 || true
  rm -rf "$MOUNT" "$RUN"
}
trap cleanup EXIT

hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT" "$DMG" >/dev/null || fail "could not mount $DMG"
SRC="$MOUNT/Displays Studio KH.app"
[ -d "$SRC" ] || fail "the DMG carries no Displays Studio KH.app"

# 1. The entitlement must be in the shipped app, not only in the build directory.
codesign -d --entitlements :- "$SRC" 2>/dev/null \
  | grep -q 'com.apple.security.cs.disable-library-validation' \
  || fail "the app inside the DMG lacks com.apple.security.cs.disable-library-validation"
echo "launch-check: entitlement present in the app inside the DMG"

# 2. Run the copy from outside the read-only image, with its own HOME and hold port.
cp -R "$SRC" "$RUN/app"
mkdir -p "$RUN/home"

HOME="$RUN/home" DISPLAYS_STUDIO_HOLD_PORT=18651 \
  "$RUN/app/Contents/MacOS/displays-studio-kh" >"$RUN/app.log" 2>&1 &
PID=$!
sleep 10

if ! kill -0 "$PID" 2>/dev/null; then
  echo "--- app log ---" >&2
  tail -20 "$RUN/app.log" >&2 || true
  fail "the app died within 10 s of launch (dyld or an early abort; the log above is why)"
fi

kill -0 "$PID" 2>/dev/null || fail "the app is not running after 10 s"
echo "launch-check: alive after 10 s at pid $PID"
# The version is read from the copy that is running, after it survived: the entitlement proves the
# artifact is right; this proves the right artifact is what ran.
VERSION=$(/usr/bin/python3 -I - "$RUN/app/Contents/Info.plist" <<'PYV'
import plistlib, sys
print(plistlib.load(open(sys.argv[1], "rb")).get("CFBundleShortVersionString", ""))
PYV
)
[ "$VERSION" = "$WANT" ] || fail "the running copy is $VERSION, not $WANT"
echo "launch-check: running copy is $VERSION"
grep -q 'update hold listening' "$RUN/app.log" && echo "launch-check: the hold listener started"
kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
echo "launch-check: PASS"
