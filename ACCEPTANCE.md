# Acceptance — Displays Studio KH 0.1.0

Owner rule 2026-11-13: before a page is scored, this file carries the task list with an
end-to-end test per task, a capability cross-check from live hardware, and a standard-practice
comparison. Evidence for the numbers below is `../lead/0.1.0-build/HARDWARE-RECEIPT.md`.

There is no create/editor page: the app is one panel of sliders. The rows below are that panel.

## 1. Task list — one end-to-end test per claimed behaviour

| Claimed behaviour | Test that fails if it breaks |
|---|---|
| The panel lists each external display, and only external ones | `scripts/verify-hidden.py` asserts the panel's `main h2` list contains `MSI MP273` on this Mac |
| Brightness is a slider that reads the monitor's real value | `verify-hidden.py` asserts the Brightness slider shows 31 for MSI MP273; `cargo run --example probe` reads 0x10 = 31/100 |
| Moving the slider changes the monitor, confirms it and reports the confirmed value | `displays-studio-kh --ddc-check 'MSI MP273'`: 31 → 36 → 31 read back, exit 0 |
| Contrast appears only where a read succeeds | `verify-hidden.py` sees Contrast at 50 (0x12) |
| Volume appears only where a read succeeds | `verify-hidden.py` sees Volume at 98 (0x62) |
| A setting the monitor does not report says "Not reported" and cannot be dragged | `hardware::tests::a_setting_the_monitor_did_not_report_is_never_a_number`; `hardware::row` returns `percent: None`, and `ui-shell/panel.js` draws no slider for it |
| A change the monitor does not confirm is refused, not reported as done | `hardware::write` re-reads twice, then errors; `cargo test` covers the range maths |
| Percentages map onto whatever range the monitor reports | `hardware::tests::scales_to_the_monitors_reported_range` |
| Bad input never reaches the hardware | `hardware::tests::rejects_unknown_setting_without_accessing_hardware` |
| One shortcut steps brightness on every supported display | shortcuts register (asserted by `verify-hidden.py`); **the key press itself is not verified** — it needs the owner's desktop |
| What's New opens in-app with the shipped version's notes | `verify-hidden.py` clicks the version button and requires three bullets for v0.1.0 |
| Version is shown in the panel | `verify-hidden.py` asserts `#version` reads v0.1.0 |
| Escape and ⌘W close the panel without quitting the app | `close_panel` command; **not exercised by an automated click** |
| Reopening from the app icon or Dock shows the panel again | `RunEvent::Reopen` → `show_panel`; **not exercised** (needs a Finder/Dock interaction) |
| No polling, no helper: idle is flat | `top` 26–27 MB, 0.0 % CPU; `check-idle-footprint.sh` +0 MB over 30 s |
| Every failure is shown in the panel's status line, in words | `status()` renders the Rust error text; errors are mapped in `hardware.rs`, never a code |

## 2. Capability cross-check (live hardware vs controls)

- VCP `0x10` brightness → Brightness slider. Read: 31/100. Write proven.
- VCP `0x12` contrast → Contrast slider, only when the read succeeds. Read: 50/100. **Writes not exercised.**
- VCP `0x62` volume → Volume slider, only when the read succeeds. Read: 98/100. **Writes not exercised.**
- Documented exceptions: a display that answers no VCP gets a row that says "No controls reported"
  rather than an invented slider; harnesses and adapters that do not pass DDC show the same row.

## 3. Standard-practice comparison

- **Reference products:** Lunar (free tier), MonitorControl, BetterDisplay.
- **Present:** per-display slider with the real value read back, contrast and volume only where
  reported, a global step shortcut, a menu-bar-only app with no Dock or window on launch.
- **Deliberately absent in 0.1.0** (backlog, `../lead/BACKLOG.md`): ambient/adaptive brightness,
  sunrise–sunset scheduling, per-app presets, BlackOut, input switching, gamma-overlay dimming,
  per-display naming. Each is a named row, not an omission.
- **Compared with Lunar:** Lunar syncs hardware and software brightness; here only hardware, so a
  monitor at 0 % cannot be dimmed further — stated in the README rather than half-done.

## 4. Not proven (and not claimed)

- A real tray click, a real key chord, and a Finder/Dock reopen: all three need the owner's desktop;
  the code paths exist and the shortcut registration is asserted.
- Writes to contrast and volume; any display other than the MSI MP273 on HDMI; any second display.
- A signed DMG install on another Mac, and anything about update holds — this app ships no hold route.
