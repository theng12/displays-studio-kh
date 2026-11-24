# Receipt — Displays Studio KH 0.1.13

Settings window with its own gear icon, a shortcut pair per display, one-click Accessibility.
```
app: displays-studio-kh
version: 0.1.13
commit: 954e0bca03e8ae81257d3a4f010d78a561f0f0b8
artifact: dist/Displays Studio KH-0.1.13.dmg
sha256: 1482c412004e9b7f6a7ef74147c07657ac7464cc754141da5c51d1222cbe9a46
bytes: 4725758
scan: scan-dmg: clean (scan-receipt sha256 1482c412004e9b7f, exit 0, scanner scan-dmg/2026-11-20, 89 files)
gate config: lead/gate-0.1.13.conf
gate exit: 0 — GATE PASS displays-studio 954e0bca03e8ae81257d3a4f010d78a561f0f0b8 2026-11-24T10:43:56Z
feed readback: UNMEASURED — not published: the tag and release push was refused by this session's permission layer (verbatim in "Not done"); no appcast item written
page state: UNMEASURED — the feed is untouched, so the last served version is still 0.1.12
dirty: false
retired: "20261124 0.1.13 dmg from gate 152220" → ~/Desktop/_trash/displays-studio-kh/ (manifest beside it); the failed scan gate's artifact, never published
not proven: feed readback, public release, appcast item, update-under-load, install on the owner's Mac, idle footprint (UNMEASURED: no IDLE_PID in the gate config)
```

## Identity

| what | value |
|---|---|
| app / bundle id | Displays Studio KH / `app.displaysstudiokh.desktop` |
| version | 0.1.13 |
| artifact | `Displays Studio KH-0.1.13.dmg` |
| sha256 | `1482c412004e9b7f6a7ef74147c07657ac7464cc754141da5c51d1222cbe9a46` |
| bytes | 4,725,758 |
| signing | KH Studio Local Signing — self-signed, not notarized |
| gated commit | `954e0bca03e8ae81257d3a4f010d78a561f0f0b8` (branch main, dirty=0 at both identity checks) |
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
GATE PASS displays-studio 954e0bca03e8ae81257d3a4f010d78a561f0f0b8 2026-11-24T10:43:56Z
runner: gate-run.sh f05b010c16e5 at /Users/vengtheng/Developer/_orchestra/tools/gate-run.sh
artifact dist/Displays Studio KH-0.1.13.dmg 4725758 bytes sha256=1482c412004e9b7f6a7ef74147c07657ac7464cc754141da5c51d1222cbe9a46
```

Evidence: `lead/gate-20261124-174254-3606` (copied here as `gate-summary.txt` and `GATE-PASS.txt`).
The runner was `f05b010c16e577986c796341a43b41b450e4639b82159f664c15b7b89d06bd5a` (SHA-256,
43,672 B), checked with `shasum -a 256` **before** the run and again **after** it — identical.

Every step recorded `ok`: preflight (with `shim: … sha256 2bce760e… / shim: PASS`), no-release,
versions, cargo-test (30 tests), hidden-ui (`ok:true`, bounds `[372,668,668]`), hold, package,
screenshots, entitlement, launch, scan (`SHIM PASS`; `scan-dmg: clean`), desktop-safety,
commit-identity. `idle-footprint` is **UNMEASURED**: the config carries no `IDLE_PID`.

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

## Not done

- Published: no. 0.1.13 is not published; that waits on the owner-facing report.
- Update-under-load proof: needs a published 0.1.13 to install from; runs after publish.
- Tag `0.1.12`: exists locally at `067a2e1`; the public push needs the owner's own approval.
