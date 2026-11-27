# Receipt — Displays Studio KH 0.1.19

Fixes the automatic update block: 0.1.13–0.1.18 copied the Launcher's shared hold token once into an
app-owned file, while the agent rotates the shared token on every start. After the first rotation the
app refused the updater's token `401`, and the updater latched it after three attempts. 0.1.19 reads
`~/Library/KHUpdater/state/hold-token` fresh on every request and names the same file in
`KHHoldTokenFile` again; the copy-once code is gone.

```
app: displays-studio-kh
version: 0.1.19
commit: 4156eab46994be54809b6d37dfadb7099eb028a0
artifact: dist/Displays Studio KH-0.1.19.dmg
sha256: 774482f4f19bfbdb722fa73512b2f33b796577d0f00bb5c89003af8aa0ead946
bytes: 4780776
scan: sha256=774482f4f19bfbdb722fa73512b2f33b796577d0f00bb5c89003af8aa0ead946 clean — scan-dmg/2026-11-20, exit 0, 89 files, excused: []
gate config: $HOME/Developer/_orchestra/tools/gate-configs/displays-0.1.19.sh
gate exit: 0 — GATE PASS displays-studio 4156eab46994be54809b6d37dfadb7099eb028a0 2026-11-27T09:50:20Z
runner: gate-run.sh 8df512ad14da at $HOME/Developer/_orchestra/tools/gate-run.sh
cargo-test: PASS — 53 passed, 0 failed
update-under-load: PASS — updater credential accepted, stale own copy 401, rotation followed live without restart; held over a running refresh, safe=false at the 1 s deadline, settled safe in 4125 ms, executable replaced in the gap, relaunched without DELETE into new work
preflight: PREFLIGHT PASS (36ebc8155c2e, plugin 0.3.354)
tag: 0.1.19 → 4156eab46994be54809b6d37dfadb7099eb028a0, verified with `git ls-remote --tags origin`
release: https://github.com/theng12/displays-studio-kh/releases/tag/0.1.19 (asset 774482f4f19b-Displays.Studio.KH-0.1.19.dmg)
download: anonymous GET of the asset = HTTP 200, 4,780,776 B, sha256 774482f4f19bfbdb722fa73512b2f33b796577d0f00bb5c89003af8aa0ead946 — identical to the gate artifact
feed readback: per-app appcast https://theng12.github.io/displays-studio-kh/appcast.xml HTTP 200, 3 items (0.1.19, 0.1.18, 0.1.17); top item 0.1.19, enclosure length 4780776, signature PASS against the anonymous download with `SUPublicEDKey`; estate feed `kh-releases/release.json` still serves 0.1.18 (row update handed to S001)
served item: shortVersionString 0.1.19, sparkle:version 0.1.19, enclosure 774482f4f19b-Displays.Studio.KH-0.1.19.dmg, length 4780776 — matches the signed DMG in dist/
page state: appcast.xml serves 0.1.19 (SIGNATURE PASS); kh-releases .apps["displays-studio"] behind at 0.1.18 — handed to S001 with the exact row
dirty: false
retired: "2026-11-27 0.1.17 dmg retired after 0.1.19 publish" → $HOME/Desktop/_trash/displays-studio-kh/ with MANIFEST.md; dist/ holds exactly 0.1.19 + 0.1.18
not proven: a fleet-updater install over the running app on this Mac (the app's own Sparkle check installed 0.1.19 first — unified log `[khsparkle] hold taken; starting Sparkle`, 16:54:11 local); HUD focus preservation (`hud-selftest` recorded UNMEASURED — session locked, no app in front); in-app screenshot of the native HUD; the two-monitor tray click and login restore; estate feed row (S001)
```

## What changed

- **Credential (`src-tauri/src/hold.rs`, `src-tauri/Info.plist`)** — the hold route reads the
  Launcher's rotating `~/Library/KHUpdater/state/hold-token` fresh per request; `ensure_own_token`
  and `own_token_path` are deleted. `KHHoldTokenFile` names the shared file again, as 0.1.11 and
  0.1.12 shipped.
- **Proof (`tests/hold.sh`)** — sends the updater's exact request (both credential headers, empty
  body, `timeout_s`/`pause_s`), refuses a stale app-owned copy `401`, rotates the shared token live
  (old `401`, new `200`, no restart), holds over a running display refresh, replaces the executable
  during the hold, and relaunches into work without `DELETE`.
- Version 0.1.19 in `Cargo.toml`, `tauri.conf.json`, `whats-new.json`, `CHANGELOG.md`; screenshots
  refreshed by the gate.

## The two questions, answered with evidence

1. **What makes the route answer `401`, and what makes it `200`?** `authorize()` rejects any
   `Origin` header, requires a non-symlink regular token file ≤512 B whose trimmed value is
   non-empty, ≤256 chars and carries no whitespace, then requires `Authorization: Bearer` and/or
   `X-Hub-Token` to equal that token (both, when both are sent). With the credential right, the exact
   path `/api/maintenance/hold` and method `POST`/`DELETE` answer `200`; another method is `405`.
   Measured on the running 0.1.18 before the fix with the updater's exact request: the app's own file
   → `200 {"safe":true}`; the Launcher's shared file → `401 {"error":"unauthorized"}`. The updater
   always presents the shared file (`UpdateHold.localToken()`), which is why it could not hold this app.
2. **Why three failures on 2026-11-25T16:22:42Z, and why no retry?** The app-owned file was seeded
   2026-11-24 18:12 and never refreshed; the shared file is rewritten by
   `_ = UpdateHold.rotateToken()` at every agent start (`khu/main.swift:793`). The first three hold
   attempts for 0.1.18 ran 2026-11-25 16:22:42 → 16:53:09Z (15-minute backoff) and were recorded in
   `agent-memory.json` `holdRefusals["displays-studio"]` with `attempts: 3`.
   `Agent.swift:1263-1272` (`holdNeedsPerson = 3`) then skips the app while it runs and repeats the
   same latched line every five minutes. **The updater latched it, not the app**; the app's pause is
   in-memory and expires.

## Gate

```
GATE PASS displays-studio 4156eab46994be54809b6d37dfadb7099eb028a0 2026-11-27T09:50:20Z
runner: gate-run.sh 8df512ad14da at $HOME/Developer/_orchestra/tools/gate-run.sh
artifact dist/Displays Studio KH-0.1.19.dmg 4780776 bytes sha256=774482f4f19bfbdb722fa73512b2f33b796577d0f00bb5c89003af8aa0ead946
```

Every step `ok`, including `cargo-test` (53/53), `hidden-ui`, `update-under-load`, `hud-selftest`,
`screenshots` (4 clean), `entitlement`, `launch` (0.1.19 launched hidden, alive after 10 s, pid 66053),
`scan` (all PASS; `SHIM PASS`), `idle-footprint` (pid 66374 = 77 MB after 20 s, −3 MB while idle,
ceiling 200 MB) and `scan-dmg` (`clean`, 89 files). Evidence:
`$HOME/Developer/_orchestra/developer-utilities/displays-studio-kh/lead/gate-20261127-164859-64862`.

Two earlier runs are named so nobody mistakes them for evidence: an INCOMPLETE run with the live app
still open (`launch` measured nothing; exit 76) and a refused rerun whose tree carried the first run's
regenerated screenshots. The incomplete artifact was moved to that run's `artifact-superseded/` and
never published.

## Update-under-load proof (`tests/hold.sh`, gate step `update-under-load`)

```
PASS  the updater's credential is accepted; the app's old own-token copy is refused (401)
PASS  the agent's rotation is followed live: the old token 401s, the new one 200s, no restart
PASS  held -> paused 120s over live work; safe=false, unsettled=["Refreshing displays (action #1)"]; client round trip 1060 ms
PASS  real work settled inside the bound (4125 ms); answer safe, ready for updating, nothing cancelled
PASS  in-flight refresh completed; queued follow-up job ("Saving a preset") refused while paused and explained in panel UI
PASS  an abandoned hold expired by itself after pause_s
PASS  the route refuses a missing/wrong token, a GET, an unknown path and a browser origin
PASS  held 120s over live work, the executable was replaced in that gap, and the replaced app relaunched without DELETE -> resumed, new work running
```

## Install acceptance on this Mac

- `/Applications/Displays Studio KH.app` reads 0.1.19 and runs as pid 68768.
- The running 0.1.19 accepts the agent's rotating `~/Library/KHUpdater/state/hold-token`:
  `POST /api/maintenance/hold?timeout_s=20&pause_s=120` → `200 {"safe":true,"in_flight":0,...}`;
  `DELETE` → `200`.
- The install itself was made by the app's own Sparkle check (`[khsparkle] hold taken; starting
  Sparkle`, unified log 16:54:11 local), not by the fleet updater. That path is therefore `not
  proven` above; the fleet updater's credential is proven accepted by the same running app.

## Retired and kept

- `dist/` now holds exactly `Displays Studio KH-0.1.19.dmg` and `Displays Studio KH-0.1.18.dmg`.
- 0.1.17's DMG and receipt moved recoverably to
  `$HOME/Desktop/_trash/displays-studio-kh/2026-11-27 0.1.17 dmg retired after 0.1.19 publish/`
  (4.6 MB) with a `MANIFEST.md`; checked first with `lsof` (no open file) and against the estate feed
  (serves 0.1.18).
- Gate evidence stays in `lead/gate-20261127-164859-64862`; the files this receipt cites are copied
  into this folder.

## Closeout

- OPEN-WORK.md: the hold-credential item is ticked with the cause and the 0.1.19 proof; its other
  items remain open and are listed in `not proven` above where they concern this release.
- S001 handoff: `$HOME/Developer/_orchestra/developer-system/update-studio-kh/lead/displays-0.1.19-feed-handoff.md`
  carries the exact row (version, sha256, bytes, file, url, tagline, type, holdURL, holdSince).
- `kh-releases` is feed-only; no app-repo `page-sync.sh` copy exists.
