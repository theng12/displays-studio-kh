# Displays Studio KH

## Platform

macOS menu-bar utility: Tauri 2, native Rust DDC/CI, static webview panel.

## Purpose

Change an external monitor's hardware brightness from the Mac. Offer contrast and monitor volume when that monitor reports them. A global shortcut adjusts the brightness of supported displays without opening the panel.

## Constraints

Free, local, and usable from the keyboard. No background polling, daemon, or automatic monitor writes. Built-in display and system audio controls stay with macOS. Settings a monitor cannot report aren't shown.
