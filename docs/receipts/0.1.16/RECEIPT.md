# Receipt — Displays Studio KH 0.1.16

The brightness HUD: a small translucent panel, bottom-centre on the display being adjusted, with a bar and the new percentage, visible 900 ms then a 200 ms fade. Native non-activating `NSPanel`, never key or main, ignores the mouse, never shown for the built-in screen, switchable in Settings. 0.1.15 was built and gated but never released; its tag exists with no artifact, and the review fixes changed the build, so the version moved.

```
app: displays-studio-kh
version: 0.1.16
commit: 1f2f840a7f55798a48b0a082922c48502ec46dbb
artifact: dist/Displays Studio KH-0.1.16.dmg
sha256: 705e03b4f5e5ffed43c2857a13006c7b336e4ebd76534a59c87c44736d15d1e5
bytes: 4768647
scan: scan-dmg: clean (scan-receipt sha256 705e03b4f5e5, exit 0, scanner scan-dmg/2026-11-20, 89 files, excused: [])
gate config: $HOME/Developer/_orchestra/tools/gate-configs/displays-0.1.16.sh
gate exit: 0 — GATE PASS displays-studio 1f2f840a7f55798a48b0a082922c48502ec46dbb 2026-11-25T10:05:52Z
runner: gate-run.sh 528de9fe53d0 at ~/Developer/_orchestra/tools/gate-run.sh
hud selftest: PASS — frame bottom-centre; visible=true key=false main=false; second press 1000 ms into the first fade kept the panel visible; hidden after the last fade; focus unchanged
preflight: PREFLIGHT PASS (36ebc8155c2e, plugin 0.3.341)
tag: 0.1.16 → annotated 748edb09ce7a113afd4cf2094f519dc4110262c6, peeled 1f2f840a7f55798a48b0a082922c48502ec46dbb, verified 2026-11-25 via `git ls-remote --tags origin`
release: https://github.com/theng12/displays-studio-kh/releases/tag/0.1.16 (asset 705e03b4f5e5-Displays.Studio.KH-0.1.16.dmg)
download: anonymous GET of the asset = HTTP 200, 4,768,647 B, sha256 705e03b4f5e5ffed43c2857a13006c7b336e4ebd76534a59c87c44736d15d1e5 — identical to the gate artifact
feed readback: per-app appcast https://theng12.github.io/displays-studio-kh/appcast.xml HTTP 200 (2,301 B, 3 items: 0.1.16, 0.1.14, 0.1.13; commit ca8ff78, byte-identical to HEAD), `appcast-verify.sh` PASS against anonymous download of 705e03b4f5e5-Displays.Studio.KH-0.1.16.dmg and `SUPublicEDKey` from `src-tauri/Info.plist`; estate feed (`kh-releases/release.json` `.apps.displays-studio`) UNMEASURED for 0.1.16 (still serves 0.1.14 at published_at 2026-11-25T08:53:43.245Z, awaiting publisher pass)
page state: appcast.xml serves 0.1.16 (SIGNATURE PASS); kh-releases/release.json .apps.displays-studio still serves 0.1.14 pending publisher pass
dirty: false
retired: "2026-11-25 0.1.15 dmg from failed gate hud-selftest", "2026-11-25 0.1.15 dmg from gate before review fixes" → ~/Desktop/_trash/displays-studio-kh/ (each with MANIFEST.md); 0.1.13 retired at the 0.1.15 attempt; dist/ holds exact two DMGs (0.1.16 + 0.1.14)
not proven: estate feed row (`kh-releases/release.json` still 0.1.14 pending publisher pass); live on-screen look (owner has not seen it); one real key press with 0.1.16 installed
```

## What it does

- A press that changes an external display shows the bar on that display: 172×44, bottom-centre 96 pt above the bottom edge, a level bar and the new percentage, HUD material. No webview, no new dependency.
- Visible 900 ms, 200 ms fade, then hidden. One short-lived thread per press; nothing runs while no key is pressed; a newer press cancels an older fade and owns the hide.
- The panel is a `NonactivatingPanel` whose subclass returns false from `canBecomeKeyWindow` and `canBecomeMainWindow`, ignores mouse events, sits at `NSStatusWindowLevel` on every Space and in full screen.
- Never for the built-in display (macOS draws its own) and never for a display the step did not change.
- The app that had focus when the press arrived is re-activated if macOS hands the front to this app; measured 3/3 presses with focus unchanged.
- Settings › Brightness keys: "Show a brightness bar on the monitor I change", on by default.

## Gate

```
GATE PASS displays-studio 1f2f840a7f55798a48b0a082922c48502ec46dbb 2026-11-25T10:05:52Z
runner: gate-run.sh 528de9fe53d0 at ~/Developer/_orchestra/tools/gate-run.sh
artifact dist/Displays Studio KH-0.1.16.dmg 4768647 bytes sha256=705e03b4f5e5ffed43c2857a13006c7b336e4ebd76534a59c87c44736d15d1e5
```

Every step `ok`, including `cargo-test` (47/47), `hud-selftest`, `launch` (`PASS: 0.1.16 launched hidden and is alive after 10s, pid 89727`), `scan`, `idle-footprint` (`pid 90426 = 72 MB after 20s, -8 MB while idle, ceiling 200 MB`) and `scan-dmg` (`clean`, 89 files). Evidence: `lead/gate-20261125-170321-86458`.

## Offscreen HUD self-test (`scripts/hud-selftest.sh`, gate step `hud-selftest`)

```
HUD SELFTEST: frame on a 1920x1080 display = 874,96 172x44 — bottom-centre, 96 above the bottom edge: ok
HUD SELFTEST: shown offscreen — visible=true key=false main=false front=Some("com.khupdater.agent") (before=Some("com.khupdater.agent")) focus=unchanged
HUD SELFTEST: second press 1000 ms into the first fade — still visible=true
HUD SELFTEST: hidden after the last press, fade 200 ms — visible_now=false
HUD SELFTEST: focus unchanged
HUD SELFTEST: PASS
```

Proven to fail before the fix: with the generation guard removed, the same test reads `still visible=false` and `FAIL`.

## Not proven

- The owner has not seen the bar on his own screen, and no real key press has driven it on his Mac.
- The focus hand-back removes the measured focus move; a brief (`< 80 ms`) promotion of the app can still happen underneath it, which no measurement here can see.
