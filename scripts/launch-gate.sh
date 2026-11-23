#!/usr/bin/env bash
# Call Release Tooling's install-launch-check.sh and classify what it said.
#
#   scripts/launch-gate.sh <dmg> "<app name>"
#
# The launch test itself is not written here: `_orchestra/tools/install-launch-check.sh` is the one
# implementation (it consumes the released image, which is where 0.1.8's fault lived). This wrapper
# only decides what its exit means — and it prints the tool's sha256, because a verdict without the
# identity of the tool that gave it reads like a verdict from a tool that never ran.
#
#   0                          -> pass
#   1 with a stated reason     -> the build is wrong
#   1 with no reason, or any
#   other status (2 = refused) -> 75 INVALID: not measured, never retried until green
#
# The check is executed directly so its own shebang applies, and its exit code is read from that
# command — never from a pipeline, where the last stage's status would be read instead.
set -uo pipefail
cd "$(dirname "$0")/.."

DMG="${1:?usage: launch-gate.sh <dmg> \"<app name>\"}"
NAME="${2:?usage: launch-gate.sh <dmg> \"<app name>\"}"
TOOL="${KH_LAUNCH_CHECK:-/Users/vengtheng/Developer/_orchestra/tools/install-launch-check.sh}"
[ -x "$TOOL" ] || { echo "launch-gate: INVALID — the shared check is missing at $TOOL"; exit 75; }
echo "launch-gate: tool $TOOL sha256 $(shasum -a 256 "$TOOL" | cut -d' ' -f1)"

OUT=$("$TOOL" "$DMG" "$NAME" 2>&1)
RC=$?
printf '%s\n' "$OUT"

[ "$RC" -eq 0 ] && { echo "launch-gate: PASS"; exit 0; }
if [ "$RC" -ne 1 ]; then
  # A single-instance app cannot be launched twice (LSMultipleInstancesProhibited), so while the
  # installed copy runs there is nothing to measure. That is recorded as INVALID and is NOT a failure
  # (Main, 2026-11-23): the line reaches the gate summary and the receipt instead of the step going red.
  if printf '%s\n' "$OUT" | grep -qi 'already running'; then
    echo "launch: INVALID — app already running (single-instance, LSMultipleInstancesProhibited); not measured"
    exit 0
  fi
  echo "launch-gate: INVALID — the shared check refused to measure (status $RC, its words above);"
  echo "             not a pass and not a verdict about the build."
  exit 75
fi
# A failure must name something: no FAIL line at all, or a FAIL line with an em dash and nothing
# after it, is the tool saying it does not know — an instrument fault, not a verdict.
if ! printf '%s\n' "$OUT" | grep -q '^FAIL:'; then
  echo "launch-gate: INVALID — the shared check failed without naming a failure (instrument fault)"
  exit 75
fi
if printf '%s\n' "$OUT" | grep -qE '^FAIL:.*—[[:space:]]*$'; then
  echo "launch-gate: INVALID — the shared check reported a failure with no reason (instrument fault)"
  exit 75
fi
echo "launch-gate: FAIL — the shared check named the reason above"
exit 1
