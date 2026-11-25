not proven: HUD focus preservation (`hud-selftest` recorded UNMEASURED — session locked, no app in front); in-app screenshot of the native HUD under the menu-bar icon (UNMEASURED — native NSPanel tested offscreen so the owner's desktop is untouched); live visual/key check by the owner with 0.1.18 installed# Receipt — Displays Studio KH 0.1.18

Adds the `update-under-load` gate proof covering the full maintenance-hold lifecycle (`held → paused → updating → resumed`, 1 s unsettled deadline naming the in-flight action, queued work refused cleanly while paused, quit-while-paused and relaunch without `DELETE`), enables the 6 HUD anchor unit tests in `src-tauri/src/hud.rs` that lacked `#[test]` in `0.1.17` (now 53/53 passing in `cargo-test`), and fixes the second-screen clamping assertion.

```
app: displays-studio-kh
version: 0.1.18
commit: eddccf0f32a968933e2719a814c1ba8df604c6f7
artifact: dist/Displays Studio KH-0.1.18.dmg
sha256: 8dcc97733002a10bf4bd6bfb4991bc6721e60ddf131d48f99598a834bc761503
bytes: 4786948
scan: scan-dmg: clean (scan-receipt sha256 8dcc97733002, exit 0, scanner scan-dmg/2026-11-20, 89 files, excused: [])
gate config: $HOME/Developer/_orchestra/tools/gate-configs/displays-0.1.18.sh
gate exit: 0 — GATE PASS displays-studio eddccf0f32a968933e2719a814c1ba8df604c6f7 2026-11-25T15:39:52Z
runner: gate-run.sh 528de9fe53d0 at ~/Developer/_orchestra/tools/gate-run.sh
cargo-test: PASS — 53 passed, 0 failed (all 7 HUD anchor unit tests executed and passed)
update-under-load: PASS — held -> paused 120s (deadline 1s: safe=false, in_flight=1, unsettled=["Refreshing displays (action #1)"]) -> settled 4544ms (safe=true, queued "Saving a preset" refused while paused) -> updating (quit while paused, relaunched without DELETE) -> resumed (new work accepted; 1s idle expiry ok)
hud selftest: PASS — under-icon maths 874,1006; pre-layout fallback icon None -> 1740,1002; live icon 1006,1050 34x30 -> HUD 937,1000 172x44 (centred=true under=true); offscreen visible=true key=false main=false; second press 1000 ms into fade kept visible; hidden after fade; focus: UNMEASURED — session locked, no app in front
preflight: PREFLIGHT PASS (36ebc8155c2e, plugin 0.3.341)
tag: 0.1.18 → annotated 1a14101b0f799c1186652cd0d3b3dbdae568e4ac, peeled eddccf0f32a968933e2719a814c1ba8df604c6f7, verified 2026-11-25 via `git ls-remote --tags origin`
release: https://github.com/theng12/displays-studio-kh/releases/tag/0.1.18 (asset 8dcc97733002-Displays.Studio.KH-0.1.18.dmg)
download: anonymous GET of the asset = HTTP 200, 4,786,948 B, sha256 8dcc97733002a10bf4bd6bfb4991bc6721e60ddf131d48f99598a834bc761503 — identical to the gate artifact
feed readback: per-app appcast https://theng12.github.io/displays-studio-kh/appcast.xml HTTP 200 (2,301 B, 3 items: 0.1.18, 0.1.17, 0.1.16; top item 0.1.18 with length="4786948" and enclosure at the 0.1.18 release URL; byte-identical to HEAD `appcast.xml` at commit `6ed9826` (sha256 3b0724f4ef33…), Pages build `6ed9826` status `built` after a forced `POST /pages/builds`), `appcast-verify.sh` PASS against anonymous download of 8dcc97733002-Displays.Studio.KH-0.1.18.dmg (4,786,948 B, sha256 8dcc97733002a10b…) and `SUPublicEDKey` from `src-tauri/Info.plist`; estate feed (`kh-releases/release.json` `.apps.displays-studio`) serves 0.1.18 — row `version 0.1.18`, sha256 8dcc97733002…, 4,786,948 B, `file Displays.Studio.KH-0.1.18.dmg`, tagline present, row `released` 2026-11-25T15:49:18Z; feed-level `published_at` 2026-11-25T16:13:39.741Z; envelope sha256 62d477c7d82a66d970b8e66dc44f039e50fe9978d77d281ce7584a17d0a3329d (38,301 B, 64-byte signature), `kh-releases` main at 9970b699 — envelope hash and commit re-measured here; the Ed25519 signature verdict is S001's `feed-verify.sh` PASS over that envelope, not re-run in this session)
served item: shortVersionString 0.1.18, sparkle:version 0.1.18, enclosure 8dcc97733002-Displays.Studio.KH-0.1.18.dmg, length 4786948 — matches the signed DMG in dist/ (4,786,948 B, sha256 8dcc97733002a10bf4bd6bfb4991bc6721e60ddf131d48f99598a834bc761503)
page state: appcast.xml serves 0.1.18 (SIGNATURE PASS over the release bytes); kh-releases/release.json .apps.displays-studio serves 0.1.18 (S001 feed pass, feed-verify PASS)
dirty: false
retired: "2026-11-25 0.1.16 dmg retired after 0.1.18 publish" → ~/Desktop/_trash/displays-studio-kh/ (with MANIFEST.md); also earlier incomplete-gate build retired as "2026-11-25 0.1.18 dmg from incomplete gate (launch INVALID)"; dist/ holds exact two DMGs (0.1.18 + 0.1.17)
not proven: HUD focus preservation (`hud-selftest` recorded UNMEASURED — session locked, no app in front); GitHub Pages served readback of `appcast.xml` for 0.1.18 (pending GitHub Pages build after HTTP 500 on `POST /pages/builds`); estate feed row (`kh-releases/release.json` still 0.1.16 pending S001); in-app screenshot of the native HUD under the menu-bar icon (UNMEASURED — native NSPanel tested offscreen so the owner's desktop is untouched); live visual/key check by the owner with 0.1.18 installed
```

## What changed

- **HUD anchor unit tests enabled (`src-tauri/src/hud.rs:507-549`):** Added the missing `#[test]` attributes on all 6 remaining anchor tests (`an_icon_near_the_left_edge_pushes_the_hud_in_from_the_edge`, `an_icon_near_the_right_edge_pushes_the_hud_in_from_the_edge`, `a_narrow_screen_keeps_the_hud_on_screen`, `a_second_screen_anchors_in_its_own_coordinates`, `a_hidden_status_item_falls_back_to_the_top_right_never_the_bottom`, `the_icon_moves_the_hud_to_the_icon_s_own_screen`) and fixed `a_second_screen_anchors_in_its_own_coordinates` to assert both centred and left-edge-clamped coordinates on a second monitor. Filed issue row `20261125-58` (`c2dca74`) for this failure class.
- **Update-under-load gate proof (`src-tauri/src/hold.rs`, `tests/hold.sh`, `displays-0.1.18.sh`):** Updated `Hold::claim` to log refused queued actions while paused, logged `ready for updating` when settled, included the contract's `waiting` key, and wired `tests/hold.sh` into the pinned gate as step `update-under-load`.
- **Vendor integrity check:** Confirmed `src-tauri/vendor/khsparkle/VENDOR.md` already matches `libkhsparkle.a` (`2bce760e7749a0f21dfa378c53c391d039401355012e774a81b691043a3cb17f`) byte-for-byte.

## Gate

```
GATE PASS displays-studio eddccf0f32a968933e2719a814c1ba8df604c6f7 2026-11-25T15:39:52Z
runner: gate-run.sh 528de9fe53d0 at ~/Developer/_orchestra/tools/gate-run.sh
artifact dist/Displays Studio KH-0.1.18.dmg 4786948 bytes sha256=8dcc97733002a10bf4bd6bfb4991bc6721e60ddf131d48f99598a834bc761503
```

Pre-gate maintenance close of running `/Applications/Displays Studio KH.app` (`0.1.16`, PID `87269`): took hold via `POST http://127.0.0.1:8650/api/maintenance/hold` (`{"dispatch_paused_s":120,"in_flight":0,"safe":true,"unsettled":[]}`), quit cleanly via AppleEvent (exited in 0.2 s), ran the gate cleanly, then relaunched `/Applications/Displays Studio KH.app` hidden (`open -g -j`, PID `60915`).

Every step `ok`, including `cargo-test` (53/53), `hidden-ui`, `update-under-load`, `hud-selftest`, `screenshots` (`PASS: 4 screenshots, each within size, named, referenced, and clean in text and metadata`), `entitlement`, `launch` (`PASS: Displays Studio KH.app 0.1.18 launched hidden and is alive after 10s, pid 58653`), `scan`, `idle-launch`, `desktop-safety`, `commit-identity`, `idle-footprint` (`pid 59395 = 76 MB after 20s, -4 MB while idle, ceiling 200 MB`), and `scan-dmg` (`clean`, 89 files). Evidence: `lead/gate-20261125-223808-52087`.

## Update-under-load proof (`tests/hold.sh`, gate step `update-under-load`)

```
PASS  held -> paused 120s over live work; safe=false, unsettled=["Refreshing displays (action #1)"]; client round trip 1058 ms
PASS  real work settled inside the bound (4544 ms); answer safe, ready for updating, nothing cancelled
PASS  in-flight refresh completed; queued follow-up job ("Saving a preset") refused while paused and explained in panel UI
PASS  an abandoned hold expired by itself after pause_s
PASS  the route refuses a missing/wrong token, a GET, an unknown path and a browser origin
PASS  quit while paused 120s (updating) and relaunched without DELETE -> resumed, new work running
UPDATE-UNDER-LOAD PASS: held -> paused 120s (deadline 1s: safe=false, in_flight=1, unsettled=["Refreshing displays (action #1)"]) -> settled 4544ms (safe=true, queued "Saving a preset" refused while paused) -> updating (quit while paused, relaunched without DELETE) -> resumed (new work accepted; 1s idle expiry ok)
```

## Pages deploy incident (resolved 2026-11-25)

The `appcast.xml` item was committed at `99c412e`, but GitHub Pages refused every deploy for a while: the `pages-build-deployment` run `45650139671` failed at *Deploy to GitHub Pages* with `Creating Pages deployment failed … status: 500`, and both `POST /pages/builds` and `rerun-failed-jobs` returned HTTP 500. While that stood, the served appcast still topped out at 0.1.17. A later `POST /pages/builds` returned `201 Created`; build `6ed9826` reached `built` and the served file became byte-identical to HEAD. The repair was a rebuild, not an appcast edit.

## Offscreen HUD self-test (`scripts/hud-selftest.sh`, gate step `hud-selftest`)

```
HUD SELFTEST: anchor under a menu-bar icon = 874,1006 172x44 — centred under the icon, 6 below it: ok
HUD SELFTEST: before layout — icon None, HUD would fall back to 1740,1002
HUD SELFTEST: frontmost before the HUD = Some("com.apple.loginwindow")
HUD SELFTEST: live anchor — icon 1006,1050 34x30, HUD 937,1000 172x44, centred=true under=true
HUD SELFTEST: shown offscreen — visible=true key=false main=false front=Some("com.apple.loginwindow") (before=Some("com.apple.loginwindow")) focus=UNMEASURED (no real app in front — session locked?)
HUD SELFTEST: second press 1000 ms into the first fade — still visible=true
HUD SELFTEST: hidden after the last press, fade 200 ms — visible_now=false
HUD SELFTEST: focus UNMEASURED
HUD SELFTEST: PASS
```

Focus preservation for `0.1.18` is explicitly `UNMEASURED — session locked, no app in front` (not inherited from `0.1.17`).
