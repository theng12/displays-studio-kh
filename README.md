# Displays Studio KH

Control external monitor brightness from the Mac menu bar over DDC/CI. Contrast and monitor volume appear only when the display reports them. DisplayPort, HDMI, docks, and adapters vary: if a control is missing, check the monitor's DDC/CI setting and try a direct connection.

Click the menu-bar display icon to open the panel. Use **Control–Shift–]** or **Control–Shift–[** to step all supported displays by 5%. Turn shortcuts off in the panel. Closing the panel does not quit the app; use **Quit Displays Studio KH** in the tray menu. The app doesn't poll or run a helper. It does not intercept media keys.

## Local build

Use the macOS Tauri 2 toolchain. Take the shared studio app test lane before any build, probe, or hidden run. No launch is needed to build the crate.

```sh
cargo build --manifest-path src-tauri/Cargo.toml --locked --features custom-protocol
```

The Tauri configuration targets a macOS DMG. Use the Studio signing and release process before publishing; this checkout has no published build. Recreate `ui-shell/whats-new.json` with the Studio `whats-new.py` script whenever `CHANGELOG.md` changes.

## Local checks

`cargo test` in `src-tauri` checks level conversion and validates inputs without hardware. `cargo run --example probe` reads the hardware it actually finds. To check the embedded UI without taking the desktop, build with both `custom-protocol` and `webview-check`, then run `python3 -I scripts/verify-hidden.py src-tauri/target/debug/displays-studio-kh`. The current receipt fixture expects the connected MSI MP273. The check isolates app state and verifies the hidden pane, then samples idle with the pane closed.

Only the `webview-check` build accepts `--test-mode <dir>` (the same hidden isolated run as
`--webview-check <dir>`) and `--ddc-check 'MSI MP273'`. It reads the exact brightness on that monitor, changes it by one step, reads it back, and restores and verifies the original value. Run this only when authorized to change that display.

## Release

Build, sign and package here, then publish through the release host:

```sh
bash scripts/build-mac.sh
packaging/release-push.sh displays-studio "Displays Studio KH" 0.1.0 "dist/Displays Studio KH-0.1.0.dmg" CHANGELOG.md
```

The first writes `dist/Displays Studio KH-<version>.dmg` signed with the shared KH identity and a
receipt beside it; the second stages the DMG on 1301, publishes there, and reads the served feed
back. This app ships no update-hold route yet, so its feed entry carries no holdURL.
