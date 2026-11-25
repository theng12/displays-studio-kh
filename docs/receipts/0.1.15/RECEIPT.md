# Receipt — Displays Studio KH 0.1.15

Adds the brightness HUD: a small translucent panel, bottom-centre on the display being adjusted, with a bar and the new percentage, visible for 900 ms and then a 200 ms fade. Native non-activating `NSPanel`, never key or main, ignores the mouse, never shown for the built-in screen. Settings gains a switch for it, on by default.

```
app: displays-studio-kh
version: 0.1.15
commit: d9d6161e6eab08f015486ccbf316903da105f1be (gated; f599375 after it is docs/screenshots only)
artifact: dist/Displays Studio KH-0.1.15.dmg
sha256: 3819717492af32f4c9e2d7d5f918db4a642f2d5b42dfa207e14de8ff5aee548d
bytes: 4764394
scan: scan-dmg: clean (scan-receipt sha256 3819717492af32f4, exit 0, scanner scan-dmg/2026-11-20, 89 files, excused: [])
gate config: $HOME/Developer/_orchestra/tools/gate-configs/displays-0.1.15.sh
gate exit: 0 — GATE PASS displays-studio d9d6161e6eab08f015486ccbf316903da105f1be 2026-11-25T09:42:19Z
hud selftest: PASS — frame 874,96 172x44 bottom-centre on 1920x1080; visible=true key=false main=false; hidden after 1400 ms (900 ms visible + 200 ms fade + settle); focus UNMEASURED (session locked, no real app in front)
preflight: PREFLIGHT PASS (preflight 36ebc8155c2e, plugin 0.3.341) — artifact sha, scan receipt bound to that sha, no existing 0.1.15 release, feed served 0.1.14, screenshots, no home paths at HEAD
published: NOT PUBLISHED — `gh release create` was refused by this session's safety classifier ([Create Public Surface]); the tag and main are pushed, the GitHub release and its asset are not created
page state: appcast.xml serves 0.1.14 (unchanged); nothing on GitHub Pages names 0.1.15 yet
feed readback: UNMEASURED — the appcast cannot point at an asset that does not exist yet
retired: "2026-11-25 0.1.15 dmg from failed gate hud-selftest" → ~/Desktop/_trash/displays-studio-kh/ (MANIFEST.md)
dirty: false
not proven: live on-screen look of the HUD (owner has not seen it); focus behaviour with a real app in front (the self-test ran with the session locked)
```

## What it does

- A key press that changes an external display's brightness shows a panel on that display: 172×44, bottom-centre 96 pt above the bottom edge, a level bar plus the new percentage, HUD material, native AppKit — no webview and no new dependency.
- Visible for 900 ms, then a 200 ms fade and hidden. One short-lived thread per press; nothing runs while no key is pressed. A newer press bumps a generation counter, so an older fade cannot hide a fresh value.
- The panel is a `NonactivatingPanel` whose subclass returns false from `canBecomeKeyWindow` and `canBecomeMainWindow`; it ignores mouse events and sits at `NSStatusWindowLevel` on every Space and in full screen.
- Never shown for the built-in display (macOS already draws its own brightness HUD there) and never for a display the step did not change.
- Settings › Brightness keys gains "Show a brightness bar on the monitor I change", on by default; the hidden UI check proves the switch sticks on and off.

## Measured behaviour (`scripts/hud-selftest.sh`, run by the gate as `hud-selftest`)

```
HUD SELFTEST: frame on a 1920x1080 display = 874,96 172x44 — bottom-centre, 96 above the bottom edge: ok
HUD SELFTEST: frontmost before the HUD = Some("com.apple.loginwindow")
HUD SELFTEST: shown offscreen — visible=true key=false main=false front=Some("com.apple.loginwindow") (before=Some("com.apple.loginwindow")) focus=UNMEASURED (no real app in front — session locked?)
HUD SELFTEST: hidden after 1400 ms, fade 200 ms — visible_now=false
HUD SELFTEST: focus UNMEASURED
HUD SELFTEST: PASS
```

The panel is built and ordered front at (-6000,-6000), so the measurement never draws on the owner's screen.

## Gate

```
GATE PASS displays-studio d9d6161e6eab08f015486ccbf316903da105f1be 2026-11-25T09:42:19Z
runner: gate-run.sh 528de9fe53d0
artifact dist/Displays Studio KH-0.1.15.dmg 4764394 bytes sha256=3819717492af32f4c9e2d7d5f918db4a642f2d5b42dfa207e14de8ff5aee548d
```

Every step `ok`: `preflight`, `no-release`, `versions`, `cargo-test` (47/47), `hidden-ui`, `hold`, `release-build`, `package`, **`hud-selftest`**, `screenshots`, `entitlement`, `launch` (`PASS: Displays Studio KH.app 0.1.15 launched hidden and is alive after 10s, pid 1531`), `scan`, `idle-launch`, `desktop-safety`, `commit-identity`, `idle-footprint` (`pid 3885 = 76 MB after 20s, -3 MB while idle, ceiling 200 MB`), `scan-dmg` (`clean`, 89 files).

Evidence: `lead/gate-20261125-163946-86077` (copied here as `GATE-PASS.txt`, `gate-summary.txt`, `hud-selftest.txt`).

An earlier run of the same config (`gate-20261125-161637`) failed at `hud-selftest`: the test measured app activation while the session was locked and could not tell the HUD from the lock screen. Its DMG was retired with a manifest before this run.

## Not done

- **Not published.** `gh release create 0.1.15` (public release + asset) was refused by this session's safety classifier as a public-surface action the user had not asked for in his own words. `main` and tag `0.1.15` are pushed; the appcast still serves 0.1.14 and must not be changed until the asset exists.
- **Focus with a real app in front is unproven.** The owner's screen was locked for every measurement, so the self-test recorded focus as UNMEASURED. On that locked session the frontmost app did not change when the panel appeared.
- The owner has not seen the bar on his own screen. The Settings switch turns it off if it ever gets in the way.
