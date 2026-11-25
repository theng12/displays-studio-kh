#!/usr/bin/env bash
# The fleet update hold (`_orchestra/youtube-stack/update-hold/CONTRACT.md`), proven headless with
# work genuinely in flight: a hidden instance on its own temp profile, a real display refresh
# slowed by the test hook, the hold taken while it runs, a bounded settle, an honest unsafe answer,
# DELETE and expiry resume, a restart that starts open, and the route's refusals.
# No window, no desktop input, its own port (the live app keeps 8650).
#
#   bash tests/hold.sh [path/to/displays-studio-kh]
#   (binary: cargo build --features custom-protocol,webview-check)
set -euo pipefail
cd "$(dirname "$0")/.."

BIN="${1:-src-tauri/target/debug/displays-studio-kh}"
[ -x "$BIN" ] || { echo "no executable at $BIN"; exit 2; }
# Not in a bundle, so @executable_path/../Frameworks finds nothing: dyld reads the vendored framework.
export DYLD_FRAMEWORK_PATH="$PWD/src-tauri/vendor/khsparkle"
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1" >&2; exit 1; }

RUN="$(mktemp -d "${TMPDIR:-/tmp}/displays-studio-hold.XXXXXX")"
mkdir -p "$RUN/home"
export HOME="$RUN/home"
# A fixed default collided with another session's harness squatting 18650–18653 (2026-11-23), so the
# test takes a port the kernel says is free. HOLD_TEST_PORT still pins one when that is wanted.
# shortcut: the port can be taken between the probe and the app's bind; the test then fails loudly.
PORT="${HOLD_TEST_PORT:-$(/usr/bin/python3 -I -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')}"
export DISPLAYS_STUDIO_HOLD_PORT="$PORT"
export DISPLAYS_STUDIO_TEST_WORK_MS="${HOLD_TEST_WORK_MS:-6000}"
TOKEN_FILE="$RUN/hold-token"
printf '%s\n' "$(/usr/bin/python3 -I -c 'import secrets; print(secrets.token_hex(32))')" > "$TOKEN_FILE"
chmod 600 "$TOKEN_FILE"
HOLD_TOKEN="$(cat "$TOKEN_FILE")"
export KHU_HOLD_TOKEN_FILE="$TOKEN_FILE"
LOG="$RUN/app.log"
APP_PID=""
cleanup() { [ -n "$APP_PID" ] && kill "$APP_PID" 2>/dev/null || true; }
trap cleanup EXIT

now_ms() { /usr/bin/python3 -I -c 'import time; print(int(time.time() * 1000))'; }
field() { /usr/bin/python3 -I -c 'import json,sys; print(json.dumps(json.load(sys.stdin).get(sys.argv[1])))' "$1"; }
post() { curl -sS --max-time 35 -X POST -H "X-Hub-Token: $HOLD_TOKEN" "http://127.0.0.1:$PORT/api/maintenance/hold?$1"; }
del() { curl -sS --max-time 5 -X DELETE -H "X-Hub-Token: $HOLD_TOKEN" "http://127.0.0.1:$PORT/api/maintenance/hold"; }
code() { curl -sS -o /dev/null -w '%{http_code}' "$@"; }
wait_for() { # <text> <seconds>
  local end=$(( $(now_ms) + ${2} * 1000 ))
  while [ "$(now_ms)" -lt "$end" ]; do grep -qF -- "$1" "$LOG" 2>/dev/null && return 0; sleep 0.25; done
  return 1
}
start() { # <config dir>
  : > "$LOG"
  "$BIN" --test-mode "$1" >>"$LOG" 2>&1 &
  APP_PID=$!
  wait_for "update hold listening on http://127.0.0.1:$PORT" 15 || { tail -30 "$LOG" >&2; fail "the hidden app did not start its update hold"; }
}
quit() {
  kill "$APP_PID" 2>/dev/null || true
  for _ in $(seq 1 40); do kill -0 "$APP_PID" 2>/dev/null || break; sleep 0.25; done
  kill -0 "$APP_PID" 2>/dev/null && kill -9 "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
  for _ in $(seq 1 40); do lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1 || break; sleep 0.25; done
}

mkdir -p "$RUN/config1"
echo "Run folder: $RUN"
start "$RUN/config1"
# The panel loads and starts a real display refresh; the test hook makes it last a few seconds
# and says so, so the hold below provably lands while that work is running.
wait_for "test work in flight" 30 || { tail -30 "$LOG" >&2; fail "the panel's display refresh never started"; }

# 1. A hold over live work: taken at once, honest unsafe answer naming it, bounded by 1 s.
START=$(now_ms); REPLY=$(post "timeout_s=1&pause_s=120"); ELAPSED=$(( $(now_ms) - START ))
[ "$(echo "$REPLY" | field safe)" = "false" ] || fail "a hold over running work answered safe=true: $REPLY"
[ "$(echo "$REPLY" | field in_flight)" -ge 1 ] || fail "the hold did not count the running work: $REPLY"
[ "$(echo "$REPLY" | field waiting)" = "0" ] || fail "the hold did not report waiting=0: $REPLY"
UNSETTLED_AT_DEADLINE="$(echo "$REPLY" | field unsettled)"
[ "$UNSETTLED_AT_DEADLINE" != '[]' ] || fail "the unsafe answer named nothing: $REPLY"
printf '%s' "$UNSETTLED_AT_DEADLINE" | grep -q "Refreshing displays" || fail "the unsafe answer did not name the in-flight refresh: $UNSETTLED_AT_DEADLINE"
wait_for "held new work for an update (paused 120 s)" 5 || fail "the app did not log the held -> paused 120s transition"
wait_for "still in flight: Refreshing displays" 5 || fail "the app log did not name the unsettled refresh at the deadline"
# Wall clock covers curl and the transport, so it is held to a generous band: not before the
# ceiling (it must not answer early) and not far past it. The app's own line — "update hold: not
# safe in N ms (ceiling 1 s)" — is the tight number, and it includes the reply assembly.
[ "$ELAPSED" -ge 900 ] && [ "$ELAPSED" -le 5000 ] || fail "the settle ignored its 1 s ceiling (took ${ELAPSED} ms)"
pass "held -> paused 120s over live work; safe=false, unsettled=${UNSETTLED_AT_DEADLINE}; client round trip ${ELAPSED} ms"

# 2. The same work settles inside the 20 s ceiling and the answer becomes safe, with nothing cancelled.
REPLY=$(del)
[ "$(echo "$REPLY" | field safe)" = "true" ] || fail "DELETE did not resume: $REPLY"
[ "$(echo "$REPLY" | field dispatch_paused_s)" = "0" ] || fail "DELETE left a pause behind: $REPLY"
START=$(now_ms); REPLY=$(post "timeout_s=20&pause_s=120"); SETTLE_MS=$(( $(now_ms) - START ))
[ "$(echo "$REPLY" | field safe)" = "true" ] || fail "the running work did not settle inside 20 s: $REPLY"
[ "$(echo "$REPLY" | field in_flight)" = "0" ] || fail "the safe answer still counts work: $REPLY"
[ "$(echo "$REPLY" | field unsettled)" = '[]' ] || fail "the safe answer names unsettled work: $REPLY"
[ "$(echo "$REPLY" | field waiting)" = "0" ] || fail "the safe answer did not report waiting=0: $REPLY"
[ "$SETTLE_MS" -ge 800 ] || fail "the settle did not wait for the running refresh (answered in ${SETTLE_MS} ms)"
wait_for "ready for updating" 5 || fail "the app did not log the updating transition after settling"
pass "real work settled inside the bound (${SETTLE_MS} ms); answer safe, ready for updating, nothing cancelled"

# 3. While the 120 s hold still stands, the panel's queued follow-up job ("Saving a preset") is
# refused by hold.claim in plain words, and the hidden panel check records that refusal cleanly.
wait_for '"ok":true' 45 || { tail -30 "$LOG" >&2; fail "the hidden panel check did not finish after the holds"; }
grep -qF 'update hold: refused "Saving a preset" while paused' "$LOG" || { tail -40 "$LOG" >&2; fail "no live follow-up job was refused while the hold was active"; }
grep -qF 'refused during the update hold' "$LOG" || { tail -40 "$LOG" >&2; fail "the panel UI did not record the pause refusal"; }
pass "in-flight refresh completed; queued follow-up job (\"Saving a preset\") refused while paused and explained in panel UI"

# 4. An abandoned hold expires by itself (an installer killed before its release sends nothing).
del >/dev/null
post "timeout_s=1&pause_s=1" >/dev/null
wait_for "the update pause expired; new work is accepted again" 8 || fail "the pause did not expire by itself"
pass "an abandoned hold expired by itself after pause_s"

# 5. The route refuses what it must: no token, a wrong token, a GET, a wrong path, a browser origin.
[ "$(code -X POST "http://127.0.0.1:$PORT/api/maintenance/hold?timeout_s=1")" = 401 ] || fail "a hold without the token was accepted"
[ "$(code -X POST -H 'X-Hub-Token: not-the-token' "http://127.0.0.1:$PORT/api/maintenance/hold?timeout_s=1")" = 401 ] || fail "a wrong token was accepted"
[ "$(code -X DELETE "http://127.0.0.1:$PORT/api/maintenance/hold")" = 401 ] || fail "an unauthenticated resume was accepted"
[ "$(code -H "X-Hub-Token: $HOLD_TOKEN" "http://127.0.0.1:$PORT/api/maintenance/hold")" = 405 ] || fail "a GET was not refused"
[ "$(code -X POST -H "X-Hub-Token: $HOLD_TOKEN" "http://127.0.0.1:$PORT/api/other")" = 404 ] || fail "an unknown path was not refused"
[ "$(code -X POST -H "X-Hub-Token: $HOLD_TOKEN" -H 'Origin: http://example.test' "http://127.0.0.1:$PORT/api/maintenance/hold?timeout_s=1")" = 401 ] || fail "a browser-origin request was accepted"
pass "the route refuses a missing/wrong token, a GET, an unknown path and a browser origin"

# 6. Updating gap + relaunch without DELETE: take a 120 s hold, quit the process while held
# (simulating replace during updating), relaunch fresh, and prove work starts immediately (resumed).
REPLY=$(post "timeout_s=1&pause_s=120")
[ "$(echo "$REPLY" | field safe)" = "true" ] || fail "could not enter updating hold before restart: $REPLY"
[ "$(echo "$REPLY" | field dispatch_paused_s)" = "120" ] || fail "updating hold did not report pause_s=120: $REPLY"
grep -E 'held new work|update hold:|the update pause expired|resumed;' "$LOG" | sed 's/^/  log: /'
quit
mkdir -p "$RUN/config2"
start "$RUN/config2"
wait_for "test work in flight" 30 || { tail -30 "$LOG" >&2; fail "the restarted app refused its own first work"; }
REPLY=$(post "timeout_s=1&pause_s=1")
[ "$(echo "$REPLY" | field in_flight)" -ge 1 ] || fail "a restart came up holding: $REPLY"
pass "quit while paused 120s (updating) and relaunched without DELETE -> resumed, new work running"
quit
echo "UPDATE-UNDER-LOAD PASS: held -> paused 120s (deadline 1s: safe=false, in_flight=1, unsettled=${UNSETTLED_AT_DEADLINE}) -> settled ${SETTLE_MS}ms (safe=true, queued \"Saving a preset\" refused while paused) -> updating (quit while paused, relaunched without DELETE) -> resumed (new work accepted; 1s idle expiry ok)"
echo "All hold checks passed in $RUN"
