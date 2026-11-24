# Receipt — Displays Studio KH 0.1.13

Settings window with its own gear icon, a shortcut pair per display, one-click Accessibility.
```
app: displays-studio-kh
version: 0.1.13
commit: 764bc5c220976c35513d5d8db901b42b4b1e3ab5
artifact: dist/Displays Studio KH-0.1.13.dmg
sha256: 54d97a0b119c851df13a69cc619b39491eb820d55789c1fa9894adffe100560c
bytes: 4726035
scan: scan-dmg: clean (scan-receipt sha256 54d97a0b119c851d, exit 0, scanner scan-dmg/2026-11-20, 89 files)
gate config: lead/gate-0.1.13.conf
gate exit: 0 — GATE PASS displays-studio 764bc5c220976c35513d5d8db901b42b4b1e3ab5 2026-11-24T12:19:35Z
tag: 0.1.13 → 954e0bca03e8ae81257d3a4f010d78a561f0f0b8, on the public remote — verified 2026-11-24 with `git ls-remote --tags origin` (refs/tags/0.1.13 b918a1a4…, peeled refs/tags/0.1.13^{} 954e0bca…). Pushed by U001's session; my own push was refused.
feed readback: UNMEASURED — not published: the release and appcast steps are refused by this session's permission layer (verbatim in "Not done"); no appcast item written
page state: UNMEASURED — the feed is untouched, so the last served version is still 0.1.12
dirty: false
retired: "20261124 0.1.13 dmg from gate 152220" → ~/Desktop/_trash/displays-studio-kh/ (manifest beside it); the failed scan gate's artifact, never published
not proven: feed readback, public release, appcast item, update-under-load, install on the owner's Mac
```

## Identity

| what | value |
|---|---|
| app / bundle id | Displays Studio KH / `app.displaysstudiokh.desktop` |
| version | 0.1.13 |
| artifact | `Displays Studio KH-0.1.13.dmg` |
| sha256 | `54d97a0b119c851df13a69cc619b39491eb820d55789c1fa9894adffe100560c` |
| bytes | 4,726,035 |
| signing | KH Studio Local Signing — self-signed, not notarized |
| gated commit | `764bc5c220976c35513d5d8db901b42b4b1e3ab5` (branch main, dirty=0 at both identity checks) |
| runner | `gate-run.sh f05b010c16e5` — SHA-256 `f05b010c16e577986c796341a43b41b450e4639b82159f664c15b7b89d06bd5a`, 43,672 B |

## What it does

- Settings window (gear in the panel footer and the tray menu): the two brightness switches, the Mac's
  F1/F2 keys, which displays those keys move (all, or one selected), startup, and the version.
- A shortcut pair per display, recorded in the Settings window (`Control+Shift+KeyB` form), replacing
  the single global pair; the global pair still exists.
- The Accessibility note links straight to the pane
  (`x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`).
- §2d: the app owns its hold token — `~/Library/Application Support/Displays Studio KH/hold-token`,
  seeded by copying the shared Launcher token at 0600 on first real launch. The shared file is never
  moved or deleted.
- The panel grows to fit the displays it lists instead of scrolling (owner bug 2).
- Vendored the current khsparkle shim (`2bce760e…`) so the release scan reads one library, not an
  accepted-previous one.

## Gate

```
GATE PASS displays-studio 764bc5c220976c35513d5d8db901b42b4b1e3ab5 2026-11-24T12:19:35Z
runner: gate-run.sh f05b010c16e5 at /Users/vengtheng/Developer/_orchestra/tools/gate-run.sh
artifact dist/Displays Studio KH-0.1.13.dmg 4726035 bytes sha256=54d97a0b119c851df13a69cc619b39491eb820d55789c1fa9894adffe100560c
```

Evidence: `lead/gate-20261124-182218-51430` (copied here as `gate-summary.txt` and `GATE-PASS.txt`).
The runner was `f05b010c16e577986c796341a43b41b450e4639b82159f664c15b7b89d06bd5a` (SHA-256,
43,672 B), checked with `shasum -a 256` **before** the run and again **after** it — identical.

Every step recorded `ok`: preflight (with `shim: … sha256 2bce760e… / shim: PASS`), no-release,
versions, cargo-test (30 tests), hidden-ui (`ok:true`, bounds `[372,668,668]`), hold, package,
screenshots, entitlement, launch, scan (`SHIM PASS`; `scan-dmg: clean`), release-build, idle-launch,
desktop-safety, commit-identity, and **`idle-footprint` measured**: `pid 77014 = 76 MB after 20s,
-2 MB while idle (ceiling 200 MB)` — flat at rest, well under the ceiling. The app the idle step
started was quit normally afterwards.

**A note on the two runs.** `lead/gate-20261124-174254-3606` also passed (`954e0bc`, DMG sha
`1482c412…`), but recorded `idle-footprint :: UNMEASURED` and compiled the release only inside
`package`. U001 asked for a green carrying both, so 0.1.13 was re-gated from `764bc5c`; that earlier
DMG was moved to `lead/gate-20261124-174254-3606/artifact-superseded/` (never published) rather than
replaced silently, and the artifact of record is the newer one above.

Two earlier runs are **void / failed**, named so nobody mistakes them for evidence:
- `gate-20261124-154900-22318` — void: started inside a window where the shared runner was being
  rewritten in place (`7cbdbe31f72a`, no longer on disk). Killed on Main's instruction.
- `gate-20261124-155559-63837` — a real failure of this build: `hidden-ui` panicked with
  `state() called before manage() for displays_studio_kh::Bus`, because an edit had dropped
  `.manage(Bus::default())`. Fixed in `0f3e318`, issue row `20261124-67`.

## Screenshots (§5d)

`docs/screenshots/panel-light.png`, `panel-dark.png`, `settings-light.png`, `settings-dark.png` —
149–171 KB each, taken by `scripts/capture-screenshots.sh` from the build being released, inside the
gate. Method: the app's own webviews, built `visible(false)`, never ordered and never activated, with
invented displays in an isolated data dir; `WKWebView.takeSnapshot` at 2×, TIFF → PNG via `sips`.
`check-app-screenshots.sh`: PASS (size, naming, README references, metadata, OCR).

Read by a person: panel-light, panel-dark, settings-light, settings-dark — **read, nothing** (no names,
emails, tokens, paths, host names; invented product names only).

## Own hold token (§2d) — proved both ways

`lead/prove-own-token.sh` (saved beside this receipt's evidence as `own-token-proof.txt`), run under
the test lane against the debug build with a test port:

- **The app owns its credential:** the first real launch created
  `~/Library/Application Support/Displays Studio KH/hold-token` — 64 hex, **0600** — by **copying**
  the shared `~/Library/KHUpdater/state/hold-token`.
- **Accepted:** `POST /api/maintenance/hold` with the app's own token → **HTTP 200**, answer carries
  `"safe": true`.
- **Refused when absent:** with the app's token file absent (`KHU_HOLD_TOKEN_FILE` pointing at a
  missing path) the same POST → **HTTP 401**.
- **The shared file is never touched:** its sha256 and mtime are identical before and after both
  runs — copied, never moved, never deleted.
- Honesty note: the shared token was rotated by the Launcher in the middle of the first proof run, so
  the run-A sha comparison read "differs" on a stale baseline; the second run, with the baseline taken
  fresh, shows own == shared, and both runs already proved the own token is accepted and the shared
  file unchanged. The shim still refuses to start in the hidden test binary (`Info.plist is missing
  SUFeedURL` — expected: the bare test binary has no bundle Info.plist; the shipped app's plist names
  `~/Library/Application Support/Displays Studio KH/hold-token`).

## Not done

- Published: no. 0.1.13 is not published; the release, asset and appcast item wait on the owner's own
  approval in a session the permission layer accepts.
- **Tag state, verified:** `refs/tags/0.1.13` exists on the public remote and peels to
  `954e0bc` — read back with `git ls-remote --tags origin`, not taken on report. It points at the
  earlier passing commit; the artifact of record was built from `764bc5c`, which differs only by
  `scripts/idle-launch.sh` and the gate config (no app source). Retagging to `764bc5c` before the
  release is a decision for whoever publishes.
- **The tag stays at `954e0bc` (decision by U001, 2026-11-24).** `refs/tags/0.1.13` exists on the
  remote and points one commit behind the artifact's build commit (`764bc5c`). Moving a tag already
  on the public remote is rewriting published history, and that rule is stronger than the gain. The
  deviation is asserted here instead:

  ```
  $ git diff --stat 954e0bc..764bc5c
   docs/receipts/0.1.13/GATE-PASS.txt    |   1 +
   docs/receipts/0.1.13/RECEIPT.md       | 109 ++++++++++++++
   docs/receipts/0.1.13/gate-summary.txt |  61 ++++++++++++
   docs/screenshots/settings-light.png   | Bin 170472 -> 170470 bytes
   scripts/idle-launch.sh                |  16 +++++
   5 files changed, 187 insertions(+)
  ```

  **No app source changes** — nothing under `src-tauri/` or `ui-shell/` — so the shipped binary is
  the same code and the tag marks the release honestly. The gate config also changed, but it lives
  outside the repository (`lead/gate-0.1.13.conf`), so it cannot appear in the diff.
- **Tag `0.1.12`: the remote's `3dd61db` is authoritative.** It is what anyone else sees; the local
  `067a2e1` is a local artefact, not a second truth. Both are named; neither was moved or deleted.
- **Own hold token — the drift decision (U001, 2026-11-24):** the app **copies once and never
  refreshes** (§2d), and the **publisher reads each app's `KHHoldTokenFile` from that app's
  Info.plist** rather than a shared path. The publisher-side change belongs to Release Tooling; this
  app adds no refresh.
- **Tag 0.1.12 differs between this checkout and the remote:** locally `067a2e1` (the appcast item),
  remotely `3dd61db` (a receipt commit). Neither was moved by me; recorded so nobody reads one as the
  other.
- Update-under-load proof: needs a published 0.1.13 to install from; runs after publish.
- Tag `0.1.12`: exists locally at `067a2e1`; the public push needs the owner's own approval.
