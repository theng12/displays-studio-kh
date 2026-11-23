#!/usr/bin/env bash
# Call Release Tooling's install-launch-check.sh and classify what it said.
#
#   scripts/launch-gate.sh <dmg> "<app name>"
#
# The launch test itself is not written here: `_orchestra/tools/install-launch-check.sh` is the one
# implementation (it consumes the released image, which is where 0.1.8's fault lived). This wrapper
# only decides what its exit means, because its self-test has been seen to fail with no reason at all
# and an empty reason is an instrument fault, not a verdict about the build:
#
#   PASS                     -> 0   the step passes
#   FAIL with a real reason  -> 1   the build is wrong
#   FAIL with no reason      -> 75  INVALID: instrument fault, never retried until green
#   REFUSED (usage/refusal)  -> 75  INVALID: not measured at all
set -uo pipefail
cd "$(dirname "$0")/.."

DMG="${1:?usage: launch-gate.sh <dmg> \"<app name>\"}"
NAME="${2:?usage: launch-gate.sh <dmg> \"<app name>\"}"
TOOL="${KH_LAUNCH_CHECK:-/Users/vengtheng/Developer/_orchestra/tools/install-launch-check.sh}"
[ -x "$TOOL" ] || { echo "launch-gate: INVALID — the shared check is missing at $TOOL"; exit 75; }

OUT=$(/bin/bash "$TOOL" "$DMG" "$NAME" 2>&1)
RC=$?
printf '%s\n' "$OUT"

case "$RC" in
  0) echo "launch-gate: PASS"; exit 0 ;;
  2) echo "launch-gate: INVALID — the shared check refused to measure (its own reason above, e.g. a copy already running); not a pass and not a verdict about the build"; exit 75 ;;
esac

# An em dash with nothing after it is the tool reporting a failure it cannot explain.
if printf '%s\n' "$OUT" | grep -qE '^FAIL: .*—[[:space:]]*$'; then
  echo "launch-gate: INVALID — the shared check reported a failure with no reason; this is an"
  echo "             instrument fault, not a verdict about the build. Not retried until it is green"
  echo "             for a stated reason."
  exit 75
fi
echo "launch-gate: FAIL — the shared check named a reason above"
exit 1
