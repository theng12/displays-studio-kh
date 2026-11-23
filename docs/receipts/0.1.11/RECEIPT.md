# Receipt — Displays Studio KH 0.1.11

Gated, published, and the update loop measured on a disposable copy. The change is one line of
behaviour in the bundled update framework: it could not read a token path written with `~`, so the
update check refused to start at all; the vendored library now expands it.

## Identity

| what | value |
|---|---|
| app / bundle id | Displays Studio KH / `app.displaysstudiokh.desktop` |
| version | 0.1.11 |
| artifact | `Displays Studio KH-0.1.11.dmg` |
| sha256 | `73278df4d2078b8739ca551804c39f3444ce07c88537a9ce224dd414aee95b78` |
| bytes | 4,661,024 |
| signing | KH Studio Local Signing — **self-signed, not notarized** |

**Commits.** The artifact was built from the gated commit and has not been rebuilt since.

| commit | what it carries |
|---|---|
| `6c78429669972f30779a8a99856b3cfb40e9ee90` | the gated tree: the fixed vendored library, version bump, notes |
| `34672b79c00fa7d6d25b51f8b143e907eb142060` | tag `0.1.11`: dead publish stubs removed, README's Release section, one `build-mac.sh` comment — **no artifact change** |
| this commit (`git log -1 --format=%H -- docs/receipts`) | `docs/receipts/0.1.11/` only |

```sh
git diff --stat HEAD^..HEAD     # docs/receipts/0.1.11/ only
git diff --stat 34672b7..HEAD   # that, plus appcast.xml (the item, 27ced2d)
git diff --stat 6c78429..HEAD   # those, plus the dead-code removal (34672b7)
```

None of those commits rebuild the artifact; the DMG still carries the gated commit's bytes.

The tag was created by the publisher at 15:18:46Z, before this receipt existed; it points at
`34672b7`, and this docs commit sits on top of it on `main`. A published tag was not rewritten.

## Gate

```
GATE PASS displays-studio 6c78429669972f30779a8a99856b3cfb40e9ee90 2026-11-23T15:12:12Z
```

Steps, in order: preflight → no-release (0.1.11 had no release yet) → versions → cargo-test →
hidden-ui → hold → package → entitlement → launch → desktop-safety → commit-identity → scan-dmg.
`step launch` recorded `INVALID — app already running (single-instance, LSMultipleInstancesProhibited);
not measured`; a dedicated measurement was taken separately (below).

Scanner: `scan-dmg/2026-11-20`, 89 files, exit 0 — see `scan-receipt.json`, full transcript in
`gate-summary.txt`.

## The build input this release depends on

`src-tauri/vendor/khsparkle/libkhsparkle.a` — 85,480 bytes,
sha256 `818826a79d4e5488…`, tracked in this repo at both the gated commit and the tag:

```sh
git ls-tree -r 6c78429 --name-only | grep libkhsparkle   # src-tauri/vendor/khsparkle/libkhsparkle.a
git show 6c78429:src-tauri/vendor/khsparkle/libkhsparkle.a | shasum -a 256
```

0.1.10 shipped `62ca951049daf27ea60dec7cf468a079ef00adf1f00f17b2fef8853318db870e`
(`git show 4c7f63c:src-tauri/vendor/khsparkle/libkhsparkle.a | shasum -a 256`). **Every row of the
table below was taken on a build carrying the new library; that is what makes row 1 authorise.**

## The fix, measured: three configurations

Run on a copy of the app taken **out of the built 0.1.11 DMG**, `KHHoldURL` pointed at a local
listener, the binary run for 14 s with its output captured. Transcript: `three-configurations.txt`.

**Pass condition for row 1: `Authorization: Bearer` on the wire and no `[khsparkle] refused:` line.**

| `KHHoldTokenFile` | on the wire | the shim's own line |
|---|---|---|
| `~/Library/KHUpdater/state/hold-token` (as shipped) | `POST /api/maintenance/hold` + `Authorization: Bearer <token>` | against a **silent** listener: `refused: hold request failed: The network connection was lost.` — the harness, not the token |
| as shipped, listener answers `200 {"safe":true}` | same POST, same header | **`hold taken; starting Sparkle`** — no refusal |
| `/nonexistent/hold-token` | **no request at all** | `refused: hold token file missing or unreadable: /nonexistent/hold-token` |
| absolute `$HOME/Library/KHUpdater/state/hold-token` | `POST …` + `Authorization: Bearer <token>` | same as the shipped row |

In 0.1.9/0.1.10 the first row's line was `refused: hold token file missing or unreadable:
~/Library/KHUpdater/state/hold-token` — the file existed, 0600, 64 bytes, and the `~` was never
expanded, so no request was sent.

**Two ways to pass for the wrong reason, both real:**
- the shim requires the hold answer's body to carry `"safe": true`; a `200` with any other body is
  `refused: hold answered 200 without safe:true`. A fake listener that says only `{"ok":true}` is a
  refused hold, not a regression;
- a disposable `HOME` proves nothing (the fix resolves `~` through `getpwuid(getuid())` and ignores
  `HOME`), and deleting the owner's token file to make the "missing" row is not allowed — point the
  key at a path that does not exist instead.

## Launch measurement

The gate's launch step could not measure (the installed copy was running). Measured separately,
installed copy quit gracefully first:

```
launch pin: .../tools/pinned/install-launch-check-e1a4850e0c342d3e.sh
            sha256 e1a4850e…61171e4e
PASS: Displays Studio KH.app 0.1.11 launched hidden and is alive after 10s (pid 78550; app.displaysstudiokh.desktop)
```

## The update loop, proven on a disposable copy

Fixture: the gated app copied out of the DMG, its version keys lowered to `0.1.10`, re-signed with the
same local identity, placed in `~/Applications` (LaunchServices starts no app copy from `$TMPDIR` —
measured with a pristine control copy). The owner's Mac was left on 0.1.9 throughout.

```
22:32:20.094 [khsparkle] hold taken; starting Sparkle     pid 85524 (the performer)
22:32:21.933 [khsparkle] driver: update found 0.1.11
22:32:21.933 [khsparkle] driver: download started
22:32:24.001 [khsparkle] driver: installing update
22:32:24.227 [khsparkle] hold taken; starting Sparkle     pid 85630 — a NEW process
```

After install: `CFBundleShortVersionString` 0.1.11; the executable's sha256 is `b1bef802e067…`, **the
binary inside the 0.1.11 DMG**, not the re-signed fixture's; `codesign --verify` valid. Transcript:
`loop-proof.txt`.

## Published

- GitHub release `0.1.11`, not a draft, not a prerelease; asset
  `73278df4d207-Displays.Studio.KH-0.1.11.dmg`, 4,661,024 bytes, digest equal to the sha above.
- Enclosure fetched back **anonymously** before the feed named it: byte-identical to the gated DMG
  (`feed-readback.txt`).
- Appcast item committed as `27ced2d` and read back from Pages: **HTTP 200, first item 0.1.11**.

**Feed and appcast, with times.** At 15:07:48Z they agreed on 0.1.10 (feed row `published_at`
15:07:48Z; appcast one item, 0.1.10) — they had disagreed earlier that evening, a card offering one
version while the updater offered another. At 15:20Z the appcast serves **0.1.11** and the Page feed
still serves **0.1.10** (`bc531508d48d-…0.1.10.dmg`, unchanged `published_at`): the card lags the
release, and the app's own updater does not read that feed.

**0.1.10's limitation, named.** 0.1.10 is published, tagged and feed-verified, and it **cannot update
itself**: it ships the pre-fix library `62ca95…`, so it refuses its own hold exactly as 0.1.9 does.
An appcast offering 0.1.10 does not mean an installed 0.1.9 can take it.

## Not true yet

- the owner's Mac runs 0.1.9, which cannot check at all; the pilot is **0.1.11 hand-installed, 0.1.12
  observing the update** — nothing here installed 0.1.11 over the installed copy;
- the four owner-reported panel/permission bugs are unproven on his hardware (tray click on two
  monitors, F1/F2 with Accessibility granted, no scrolling at the real popover size).

## Sanitisation

This directory is tracked in a public repository. No key or token values and no absolute home paths
are recorded: names carry hashes, paths are `$HOME`-relative, and the one bearer token that appeared
in a wire capture is replaced with `<token, redacted>`. Check:

```sh
grep -n -E '/Users/|Bearer [A-Za-z0-9]{16}' docs/receipts/0.1.11/*.txt docs/receipts/0.1.11/*.json
# no output; this file is the one that names the pattern, so it is left out of its own check
```
