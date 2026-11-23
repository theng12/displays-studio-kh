# Receipt — Displays Studio KH 0.1.12

Saved presets, and the release that proves the update path on an installed app.

## Identity

| what | value |
|---|---|
| app / bundle id | Displays Studio KH / `app.displaysstudiokh.desktop` |
| version | 0.1.12 |
| artifact | `Displays Studio KH-0.1.12.dmg` |
| sha256 | `bc4fb6aada82a1642142c0668c36f2dcb6207846651d3a6cc448c1daf207da2d` |
| bytes | 4,702,003 |
| signing | KH Studio Local Signing — **self-signed, not notarized** |
| gated commit | `ded90cae8bfe0971c7b4101d9206df6be6bda329` |
| appcast item | `067a2e1` |
| this receipt | the commit that adds `docs/receipts/0.1.12/` (`git log -1 --format=%H -- docs/receipts`) |

## What it does

- Saves what the monitors report right now — brightness, contrast, volume where reported — under a
  name typed in the panel. A setting a monitor did not report is left out, so applying cannot
  invent a value.
- Applying writes each setting to the display that reported it, by id. A display that is no longer
  connected is counted as failed and named, never redirected to another monitor.
- One preset per name; delete removes it. A paused app refuses a save in the hold's own words.

## Gate

```
GATE PASS displays-studio ded90cae8bfe0971c7b4101d9206df6be6bda329 2026-11-23T16:32:19Z
```

Steps, all ok: preflight (`shim: … sha256 … / shim: PASS`) → no-release → versions → cargo-test
(25 tests) → hidden-ui → hold → package → entitlement → launch → scan → desktop-safety →
commit-identity. `idle-footprint` UNMEASURED (no `IDLE_PID` in the config).

- `scan-dmg: clean`, 89 files, `scan-receipt {"sha256": "bc4fb6aa…2da2d", "exit": 0,
  "scanner_version": "scan-dmg/2026-11-20", "at": "2026-11-23T16:32:18Z", "excused": []}`.
- studio scanner, pin `studio-gate-…`, 18,754 B, sha256 `76dbf7aa…235ae9`, byte-identical to the
  canonical: **0 failing checks**, `SHIM PASS libkhsparkle.a matches the required sha`,
  `ROWESCAPE AUDIT` (not a pass — row states enumerated by hand; each has a control or a named
  refusal).
- **launch step: `INVALID — app already running (single-instance, LSMultipleInstancesProhibited);
  not measured`**, with the tool line `install-launch-check-e1a4850e… sha256 e1a4850e0c34…`.
  It is recorded, not a pass: on this pin the launch check proves an app starts and lives and says
  **nothing about self-update**. The self-update claim rests on the behavioural check and the pilot.

## Behavioural check (the only self-update instrument for a shipped bundle)

A DMG ships the framework, not the vendored `.a`, so no bundle-level check can assert the shim.
Identity **as run** (the tool is not pinned yet):

```
tool: selfupdate-behaviour-check.sh b408a789741c at _orchestra/tools/selfupdate-behaviour-check.sh
      12,617 bytes  sha256 b408a789741c4525abbdc8d85a1b8a85c1b41773335c3e7ed82b81f93ef2f589
artifact: dist/Displays Studio KH-0.1.12.dmg sha256:bc4fb6aa…2da2d
listener: 127.0.0.1:57377
  as-shipped ($HOME/Library/KHUpdater/state/hold-token): PASS — [khsparkle] hold taken; starting Sparkle
  missing (/nonexistent/hold-token-27541):              PASS — refused: hold token file missing or unreadable
  absolute:                                             PASS — [khsparkle] hold taken; starting Sparkle
```

## The pilot: 0.1.11 installed, 0.1.12 published, the app replaced itself

Order, which is what makes the observation mean anything: gate PASS → asset published → **appcast
serving 0.1.12 (200 at 16:35:22Z)** → only then the next start of the installed app.

```
before: version 0.1.11 | pid 94535  | lstart Mon Nov 23 22:59:30 2026
quit (graceful, no kill) 16:35:31Z
start: open -g -j          23:35:31 local | pid 92740
23:35:32  [khsparkle] hold taken; starting Sparkle
23:35:32  [khsparkle] driver: <private>          (the update-found line, redacted by os_log)
23:35:32  [khsparkle] driver: download started
23:35:37  [khsparkle] driver: installing update
23:35:37  [khsparkle] hold taken; starting Sparkle   pid 93256 — a NEW process
after:  version 0.1.12 | pid 93256  | lstart Mon Nov 23 23:35:37 2026
```

- **The installed binary is byte-identical to the published one:** sha256
  `88427ccd66fe9c2c71f87ff3ed8f840e31e9759c86a7cff4876970f75f9fff56`, equal to the binary inside
  `dist/Displays Studio KH-0.1.12.dmg`; `codesign --verify --deep --strict` valid.
- Changed version **and** changed `lstart`, neither by hand: the app replaced itself.
- `driver: <private>` is os_log redacting a runtime-formatted string (row `20261123-168`); the chain
  is carried by `download started` → `installing update` and by the new pid at the same second.

## Publishing

- GitHub release `0.1.12`, not draft, not prerelease; asset
  `bc4fb6aada82-Displays.Studio.KH-0.1.12.dmg`, 4,702,003 bytes, digest equal to the sha above.
- Enclosure fetched back **anonymously** before the item was served: byte-identical to the gated DMG.
- Appcast item committed `067a2e1`; live readback `https://theng12.github.io/displays-studio-kh/appcast.xml`
  → 200 at **16:35:22Z**, first item 0.1.12.

## Three failed gate runs before the pass — all mine, none the build

1. **`hold` failed** because the new hidden check saved a preset while the app was paused; the app
   refused correctly and in plain words, and the check called it a failure (the rejection is a
   string, so it printed `undefined`). Fixed: assert the plain refusal when paused, save/list/delete
   when not. Row `20261123-178`.
2. **`scan` scanned `_orchestra`, not the app**, because the step said `"$PWD"` and it expanded when
   the config was read; it was failing on five `DIALOGS` hits inside another app. The run was stopped
   rather than recorded as this app's FAILED, and the step now says `"$WORKTREE"`.
3. **`hold` failed again on the port**: an orphaned listener held 18650 (pid 75294, ppid 1, run dir
   gone; reaped by U001), then 18651 too — four listeners 18650–18653 left by
   `selfupdate-behaviour-check.sh`'s older fixed-port listener (row `20261123-185`). `tests/hold.sh`
   now takes a kernel-assigned free port; `HOLD_TEST_PORT` still pins one. An environment collision,
   not a fault in this build.

The first stopped run's DMG was retired recoverably to
`$HOME/Desktop/_trash/displays-studio-kh/2026-11-23 0.1.12 built, gate stopped after a mis-targeted scan step/`
with a manifest; it was never published.

## Not true yet

- the four owner-reported panel/permission bugs remain **unproven on his hardware** (tray click on two
  monitors, F1/F2 with Accessibility granted, no scrolling at the real popover size);
- `preset_apply` was not driven against a monitor inside the gate — it takes the sliders' own
  `set_one` path (proven here by `--ddc-check`: `40/100 → 45/100 → 40/100 (read back)`) and its
  routing is unit-tested with fake targets;
- the paused path renders the hold's sentence to the person, but the click-and-read assertion is
  0.1.13's (`lead/BACKLOG.md`);
- the estate card feed has not been regenerated for 0.1.12.

## Sanitisation

Public repository. No key or token values, no absolute home paths: `$HOME`-relative, hashes only.

```sh
grep -n -E '/Users/|Bearer [A-Za-z0-9]{16}' docs/receipts/0.1.12/*.txt
# no output; this file is the one that names the pattern, so it is left out of its own check
```
