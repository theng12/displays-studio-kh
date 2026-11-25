# Receipt — Displays Studio KH 0.1.17

Moves the brightness HUD from the bottom of the screen to directly under this app's menu-bar status item (6 pt below the icon's bottom edge, horizontally centred on the icon, clamped 8 pt inside the screen edges). When the icon has no laid-out window (menu-bar overflow), it falls back to the top-right corner of the active screen — never the bottom. When the status item's screen differs from the external display being adjusted, the HUD deliberately opens on the status item's screen (where the eye looks for the app's tray icon) rather than silently choosing.

```
app: displays-studio-kh
version: 0.1.17
commit: e3f0698417dadbc6e667c9c447ebb1d69b504fde
artifact: dist/Displays Studio KH-0.1.17.dmg
sha256: a76888c406894624a48a93d987022b05ee12599349d5ddbe5516d21f598d926a
bytes: 4786657
scan: scan-dmg: clean (scan-receipt sha256 a76888c40689, exit 0, scanner scan-dmg/2026-11-20, 89 files, excused: [])
gate config: $HOME/Developer/_orchestra/tools/gate-configs/displays-0.1.17.sh
gate exit: 0 — GATE PASS displays-studio e3f0698417dadbc6e667c9c447ebb1d69b504fde 2026-11-25T15:05:49Z
runner: gate-run.sh 528de9fe53d0 at ~/Developer/_orchestra/tools/gate-run.sh
hud selftest: PASS — under-icon maths 874,1006; pre-layout fallback icon None -> 1740,1002; live icon 1057,1050 34x30 -> HUD 988,1000 172x44 (centred=true under=true); offscreen visible=true key=false main=false; second press 1000 ms into fade kept visible; hidden after fade; focus unchanged
preflight: PREFLIGHT PASS (36ebc8155c2e, plugin 0.3.341)
tag: 0.1.17 → annotated 0f3083d74d0e803f8572dff744109a84914fae47, peeled e3f0698417dadbc6e667c9c447ebb1d69b504fde, verified 2026-11-25 via `git ls-remote --tags origin`
release: https://github.com/theng12/displays-studio-kh/releases/tag/0.1.17 (asset a76888c40689-Displays.Studio.KH-0.1.17.dmg)
download: anonymous GET of the asset = HTTP 200, 4,786,657 B, sha256 a76888c406894624a48a93d987022b05ee12599349d5ddbe5516d21f598d926a — identical to the gate artifact
feed readback: per-app appcast https://theng12.github.io/displays-studio-kh/appcast.xml HTTP 200 (2,301 B, 3 items: 0.1.17, 0.1.16, 0.1.14; commit de5c850, byte-identical to HEAD), `appcast-verify.sh` PASS against anonymous download of a76888c40689-Displays.Studio.KH-0.1.17.dmg and `SUPublicEDKey` from `src-tauri/Info.plist`; estate feed (`kh-releases/release.json` `.apps.displays-studio`) UNMEASURED for 0.1.17 (still serves 0.1.16, sha256 705e03b4f5e5…, 4,768,647 B, handed to S001 for feed update)
page state: appcast.xml serves 0.1.17 (SIGNATURE PASS); kh-releases/release.json .apps.displays-studio still serves 0.1.16 pending S001 feed pass
dirty: false
retired: "2026-11-25 0.1.14 dmg retired after 0.1.17 publish" → ~/Desktop/_trash/displays-studio-kh/ (with MANIFEST.md); dist/ holds exact two DMGs (0.1.17 + 0.1.16)
not proven: estate feed row (`kh-releases/release.json` still 0.1.16 pending S001); in-app screenshot of the native HUD under the menu-bar icon (UNMEASURED — native NSPanel tested offscreen so the owner's desktop is untouched; `docs/screenshots/` covers the 4 webview panel/settings views only); 6 of the 7 anchor unit test functions in `src-tauri/src/hud.rs:507-549` lacked `#[test]` attributes in e3f0698 and did not execute in `cargo-test` (only `the_hud_sits_centred_under_the_menu_bar_icon` ran in `cargo-test`, plus under-icon/fallback/live anchor ran in `hud-selftest`); live visual check by the owner with 0.1.17 installed
```

## What changed

- **Menu-bar anchor (`src-tauri/src/hud.rs`):** `status_item_frame()` reads this app's own `NSStatusItem` button window frame via `app.tray_by_id("displays")?.with_inner_tray_icon(...)` on the main thread. `anchor(icon, screen)` places the 172×44 panel centred horizontally on the icon, 6 pt (`ICON_GAP`) below its bottom edge, and clamped at least 8 pt (`SCREEN_MARGIN`) inside the screen edges (including narrow screens).
- **Multi-display screen choice:** `hud_frame()` selects the screen containing the status item's centre via `screen_for(icon, &screens)` (falling back to the first screen). If the status item sits on a different screen from the external display being adjusted, the HUD deliberately opens on the status item's screen so it stays directly beneath the icon the user looks for.
- **Overflow / missing-frame fallback:** When `status_item_frame()` returns `None` (status item hidden in menu-bar overflow or before first layout), `anchor(None, screen)` places the HUD at the top-right corner of the active screen (`sx + sw - HUD_W - 8`, `sy + sh - 28 - 6 - HUD_H`), never at the bottom.
- **Unchanged guarantees:** Still external displays only (never built-in), non-activating `KHHudPanel` (`canBecomeKeyWindow`/`canBecomeMainWindow` false), ignores mouse events, 900 ms visible + 200 ms fade, no polling loop, Settings switch intact.

## Gate

```
GATE PASS displays-studio e3f0698417dadbc6e667c9c447ebb1d69b504fde 2026-11-25T15:05:49Z
runner: gate-run.sh 528de9fe53d0 at ~/Developer/_orchestra/tools/gate-run.sh
artifact dist/Displays Studio KH-0.1.17.dmg 4786657 bytes sha256=a76888c406894624a48a93d987022b05ee12599349d5ddbe5516d21f598d926a
```

Every step `ok`, including `cargo-test` (47/47), `hidden-ui`, `hold`, `hud-selftest`, `screenshots` (`PASS: 4 screenshots, each within size, named, referenced, and clean in text and metadata`), `entitlement`, `launch` (`PASS: Displays Studio KH.app 0.1.17 launched hidden and is alive after 10s, pid 69945`), `scan`, `idle-launch`, `desktop-safety`, `commit-identity`, `idle-footprint` (`pid 71553 = 77 MB after 20s, -3 MB while idle, ceiling 200 MB`), and `scan-dmg` (`clean`, 89 files). Evidence: `lead/gate-20261125-220408-65456`.

## Offscreen HUD self-test (`scripts/hud-selftest.sh`, gate step `hud-selftest`)

```
HUD SELFTEST: anchor under a menu-bar icon = 874,1006 172x44 — centred under the icon, 6 below it: ok
HUD SELFTEST: before layout — icon None, HUD would fall back to 1740,1002
HUD SELFTEST: frontmost before the HUD = Some("com.anthropic.claudefordesktop")
HUD SELFTEST: live anchor — icon 1057,1050 34x30, HUD 988,1000 172x44, centred=true under=true
HUD SELFTEST: shown offscreen — visible=true key=false main=false front=Some("com.anthropic.claudefordesktop") (before=Some("com.anthropic.claudefordesktop")) focus=unchanged
HUD SELFTEST: second press 1000 ms into the first fade — still visible=true
HUD SELFTEST: hidden after the last press, fade 200 ms — visible_now=false
HUD SELFTEST: focus unchanged
HUD SELFTEST: PASS
```

## Gaps and follow-up noted

- **Missing `#[test]` attributes on 6 anchor tests in `e3f0698`:** `src-tauri/src/hud.rs:507-549` contains `an_icon_near_the_left_edge_pushes_the_hud_in_from_the_edge`, `an_icon_near_the_right_edge_pushes_the_hud_in_from_the_edge`, `a_narrow_screen_keeps_the_hud_on_screen`, `a_second_screen_anchors_in_its_own_coordinates`, `a_hidden_status_item_falls_back_to_the_top_right_never_the_bottom`, and `the_icon_moves_the_hud_to_the_icon_s_own_screen` without `#[test]`, so `cargo-test` logged `never used` warnings and did not execute those 6 (the shipped `anchor()` and `screen_for()` logic itself is sound, and `a_second_screen_anchors_in_its_own_coordinates` also had an incorrect test assertion where `ix=1930` on a `1920`-origin screen clamps to `1928.0`, not `1854.0`). Follow-up needed on `main` to add `#[test]` and fix that test coordinate.
- **No HUD screenshot captured:** `docs/screenshots/` holds the 4 webview captures (`panel-dark.png`, `panel-light.png`, `settings-dark.png`, `settings-light.png`) checked by `check-app-screenshots.sh`. The native `NSPanel` HUD was verified offscreen in `hud-selftest` (live icon frame `1057,1050 34x30` → HUD `988,1000 172x44`, `centred=true under=true`) to avoid putting windows on the owner's desktop, so an on-screen visual capture of the HUD under the menu-bar icon at 375/768/1440-equivalent is UNMEASURED.
- **Estate feed & live check:** `kh-releases/release.json` still lists 0.1.16 until S001 updates the row, and the owner has not yet visually tested 0.1.17 live.
