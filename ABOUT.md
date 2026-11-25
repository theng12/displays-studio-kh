# About Displays Studio KH

Displays Studio KH is a local macOS menu-bar app for external monitor brightness. It talks directly to compatible monitors over DDC/CI and shows contrast and monitor volume only when the monitor reports those settings. Shortcuts step brightness, and the panel can turn them off.

## Structure

- `src-tauri/` — Tauri shell, tray, hotkeys, and Rust DDC/CI backend.
- `ui-shell/` — local panel and release notes. No server, daemon, account, or network service.

## Attribution

- macOS brightness-key event-tap decoding (`NX_SYSDEFINED` subtype 8 `NSEvent.data1` and brightness keyDown keycodes `144`/`145`) is adapted from [alin23/Lunar](https://github.com/alin23/Lunar) and [alin23/MediaKeyTap](https://github.com/alin23/MediaKeyTap) (MIT License).
- External monitor DDC/CI transport uses `ddc-macos` (MIT License, bundled in `src-tauri/vendor/ddc-macos`).
