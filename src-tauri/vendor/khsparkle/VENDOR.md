# khsparkle — vendored for the Sparkle pilot (displays-studio only)

- `Sparkle.framework` — Sparkle 2.10.1 (build 2065), from
  `_orchestra/developer-system/sparkle-shim-20261123/vendor/` (pin and checksum in its
  `SPARKLE-VENDOR.md`).
- `libkhsparkle.a` — the shared shim's universal static library (arm64 + x86_64), rebuilt with
  `expand_tilde()` for `KHHoldTokenFile`, plus the readable-log rebuild
  (2bce760e7749a0f21dfa378c53c391d039401355012e774a81b691043a3cb17f). Replaced 2026-11-24 from
  `sparkle-shim-20261123/dist` because the scan's pinned REQUIRED-SHIM.txt lists only the current one.
- `khsparkle.h` — the C surface the app calls.

Pilot-only vendoring: once a second app is on Sparkle this becomes a pinned download with a checksum
(INTEGRATION.md), so the repo stops carrying 2.9 MB of framework.
