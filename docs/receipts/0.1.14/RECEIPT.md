# Receipt — Displays Studio KH 0.1.14

Fix F1/F2 brightness keys (`NX_SYSDEFINED` subtype-8 `NSEvent` decoding, hardware `144`/`145` keyDown/keyUp, standard F1/F2 `122`/`120`, 30 ms cross-shape deduplication, `tapDisabledByUserInput` re-arm, display under pointer routing, Accessibility status banner and one-click Settings link).

```
app: displays-studio-kh
version: 0.1.14
commit: 6bb611d90375839d8c4dd137ce2e56bc5fec50db
artifact: dist/Displays Studio KH-0.1.14.dmg
sha256: f76e8d54deb009f3aaecc65d4e53b23f6fd928f7fd6577b88a685f1c47a6ba81
bytes: 4751314
scan: scan-dmg: clean (scan-receipt sha256 f76e8d54deb009f3, exit 0, scanner scan-dmg/2026-11-20, 89 files, excused: [])
gate config: $HOME/Developer/_orchestra/tools/gate-configs/displays-0.1.14.sh
gate exit: 0 — GATE PASS displays-studio 6bb611d90375839d8c4dd137ce2e56bc5fec50db 2026-11-25T06:20:39Z
tag: 0.1.14 → annotated 1854190c1d934e10b2b75e578868bae556a9c6df, peeled 6bb611d90375839d8c4dd137ce2e56bc5fec50db, verified 2026-11-25 via `git ls-remote --tags origin`
feed readback: per-app appcast https://theng12.github.io/displays-studio-kh/appcast.xml HTTP 200 (2,301 B, 3 items: 0.1.14, 0.1.13, 0.1.12; commit af111b2), `appcast-verify.sh` PASS against anonymous download of https://github.com/theng12/displays-studio-kh/releases/download/0.1.14/f76e8d54deb0-Displays.Studio.KH-0.1.14.dmg (HTTP 200, 4,751,314 B, sha256 f76e8d54deb009f3aaecc65d4e53b23f6fd928f7fd6577b88a685f1c47a6ba81) and `SUPublicEDKey` from `src-tauri/Info.plist`; estate feed (`kh-releases/release.json` `.apps.displays-studio`) UNMEASURED for 0.1.14 (still served 0.1.13 at published_at 2026-11-25T03:05:18.835Z, awaiting publisher pass)
page state: appcast.xml serves 0.1.14 (SIGNATURE PASS); kh-releases/release.json .apps.displays-studio still serves 0.1.13 pending publisher pass
dirty: false
retired: "2026-11-25 0.1.14 dmg from pre-sanitize gate d3c03e4", "2026-11-25 0.1.14 dmg from superseded gate 02b9df72", "2026-11-25 0.1.14 dmg from incomplete gate 130708", and "2026-11-25 0.1.12 dmg retention" → ~/Desktop/_trash/displays-studio-kh/ (each with MANIFEST.md); dist/ holds exact two DMGs (0.1.14 + 0.1.13)
not proven: estate feed row (`kh-releases/release.json` still 0.1.13 pending publisher pass), live physical F1/F2 key press on the owner's Mac (synthetic input forbidden), update-under-load
```

## Identity

| what | value |
|---|---|
| app / bundle id | Displays Studio KH / `app.displaysstudiokh.desktop` |
| version | 0.1.14 |
| artifact | `Displays Studio KH-0.1.14.dmg` |
| sha256 | `f76e8d54deb009f3aaecc65d4e53b23f6fd928f7fd6577b88a685f1c47a6ba81` |
| bytes | 4,751,314 |
| signing | KH Studio Local Signing — self-signed, not notarized |
| gated commit | `6bb611d90375839d8c4dd137ce2e56bc5fec50db` (fast-forward merged to `main`, `dirty=0` at both identity checks) |
| runner | `gate-run.sh 528de9fe53d0` — canonical `$HOME/Developer/_orchestra/tools/gate-run.sh`, pinned `$HOME/Developer/_orchestra/tools/pinned/gate-run-528de9fe53d0c0de.sh` (48,065 B) |

## What it does

- Decodes macOS `NX_SYSDEFINED` (type 14) subtype-8 brightness media keys via `NSEvent` (`data1 >> 16`: `NX_KEYTYPE_BRIGHTNESS_UP = 2`, `NX_KEYTYPE_BRIGHTNESS_DOWN = 3`) instead of reading field 9.
- Decodes hardware brightness `keyDown`/`keyUp` virtual keycodes `144` (up) and `145` (down) alongside standard F1/F2 (`122`/`120`), with 30 ms cross-shape deduplication so one physical press never steps twice.
- Re-arms the `CGEventTap` on both `kCGEventTapDisabledByTimeout` and `kCGEventTapDisabledByUserInput`.
- Targets the display under the pointer by default (while still supporting all displays or one selected display in Settings), stepping DDC/CI for external monitors and passing through / using native macOS brightness for the built-in display.
- Surfaces "Brightness keys need Accessibility" in Settings with a one-click System Settings link and logs once per launch when Accessibility trust is off.
- Credits `alin23/Lunar` and `alin23/MediaKeyTap` (MIT License) in `ABOUT.md` and commit history.

## Gate

```
GATE PASS displays-studio 6bb611d90375839d8c4dd137ce2e56bc5fec50db 2026-11-25T06:20:39Z
runner: gate-run.sh 528de9fe53d0 at ~/Developer/_orchestra/tools/gate-run.sh
artifact dist/Displays Studio KH-0.1.14.dmg 4751314 bytes sha256=f76e8d54deb009f3aaecc65d4e53b23f6fd928f7fd6577b88a685f1c47a6ba81
```

Evidence: `lead/gate-20261125-131919-71002` (copied here as `gate-summary.txt` and `GATE-PASS.txt`).

Every step recorded `ok`:
- `preflight`, `no-release`, `versions`
- `cargo-test`: 38/38 Rust tests pass (including headless `NX_SYSDEFINED`, `144`/`145` keyDown/keyUp, cross-shape dedup, and `tapDisabledByUserInput` re-arm)
- `hidden-ui`, `hold`, `release-build`, `package`
- `screenshots`: 4 screenshots refreshed and verified (`check-app-screenshots.sh 7a035b7b380c` PASS)
- `entitlement`
- `launch`: `install-launch-check-847e77db5c99d5d6.sh` — `PASS: Displays Studio KH.app 0.1.14 launched hidden and is alive after 10s (pid 72821; app.displaysstudiokh.desktop)` (with installed 0.1.13 closed during the gate and relaunched hidden afterward)
- `scan`: `studio-gate-17ff93c69376d391.sh` — 0 failing check(s), `SHIM PASS current`
- `idle-launch` + `idle-footprint`: `pid 73493 = 72 MB after 20s, -5 MB while idle (ceiling 200 MB)`
- `desktop-safety`, `commit-identity` (all 4 commits authored with `112568204+theng12@users.noreply.github.com`)
- `scan-dmg`: `clean` (`89` files, `excused: []`)

An earlier gate attempt (`gate-20261125-130708`) exited 76 (`GATE INCOMPLETE`) because installed `0.1.13` was running while `LSMultipleInstancesProhibited` is set; that void run's DMG was retired with a manifest before the clean re-gate.

## Published & remaining

- **Published (2026-11-25, on owner's "Publish 0.1.14"):**
  - Branch `displays-0.1.14-keys` fast-forward merged to `main` at `6bb611d90375839d8c4dd137ce2e56bc5fec50db`, annotated tag `0.1.14` pushed and verified on `origin`.
  - GitHub Release: `https://github.com/theng12/displays-studio-kh/releases/tag/0.1.14` (`isDraft: false`, `publishedAt: 2026-11-25T06:34:14Z`, asset `f76e8d54deb0-Displays.Studio.KH-0.1.14.dmg`, `release-asset-name-check.sh` PASS).
  - Anonymous download verified: HTTP 200, `4,751,314` B, sha256 `f76e8d54deb009f3aaecc65d4e53b23f6fd928f7fd6577b88a685f1c47a6ba81`.
  - Per-app appcast (`appcast.xml`, commit `af111b2`): added `0.1.14` and dropped `0.1.11` to keep three items (`0.1.14`, `0.1.13`, `0.1.12`); read back from `https://theng12.github.io/displays-studio-kh/appcast.xml` (HTTP 200, `2,301` B) and verified with `appcast-verify.sh` against the anonymously downloaded DMG and `SUPublicEDKey` (**PASS**).
  - Local `dist/` retention: exact two DMGs (`Displays Studio KH-0.1.14.dmg` + `Displays Studio KH-0.1.13.dmg`); `0.1.12` retired with manifest.
- **Remaining:**
  - Estate feed row (`kh-releases/release.json` `.apps.displays-studio`) still serves `0.1.13` pending the publisher pass.
  - Owner's live physical F1/F2 key press check on the display under the pointer after updating to `0.1.14`.
  - Update-under-load proof.
