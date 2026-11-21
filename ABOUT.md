# About Displays Studio KH

Displays Studio KH is a local macOS menu-bar app for external monitor brightness. It talks directly to compatible monitors over DDC/CI and shows contrast and monitor volume only when the monitor reports those settings. Shortcuts step brightness, and the panel can turn them off.

## Structure

- `src-tauri/` — Tauri shell, tray, hotkeys, and Rust DDC/CI backend.
- `ui-shell/` — local panel and release notes. No server, daemon, account, or network service.

Version 0.1.0 is a local development build. There is no published or installed release.
