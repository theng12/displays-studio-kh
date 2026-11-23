# khsparkle — vendored for the Sparkle pilot (displays-studio only)

- `Sparkle.framework` — Sparkle 2.10.1 (build 2065), from
  `_orchestra/developer-system/sparkle-shim-20261123/vendor/` (pin and checksum in its
  `SPARKLE-VENDOR.md`).
- `libkhsparkle.a` — the shared shim's universal static library (arm64 + x86_64).
- `khsparkle.h` — the C surface the app calls.

Pilot-only vendoring: once a second app is on Sparkle this becomes a pinned download with a checksum
(INTEGRATION.md), so the repo stops carrying 2.9 MB of framework.
