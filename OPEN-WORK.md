# Open work — displays-studio-kh

What this app still owes, newest first. Open items only: shipped work lives in `CHANGELOG.md` and
`docs/receipts/<version>/RECEIPT.md`, and the full product gap against the reference app is
`FEATURE-GAP.md`.

- [x] **The automatic update was blocked by a hold-credential refusal — fixed in 0.1.19.** Cause: 0.1.13–0.1.18 copied the Launcher's token once into the app's own file; the agent rotates the shared token on every start, so after the first rotation the app refused the presented token `401` and the updater latched it after three attempts on 2026-11-25. 0.1.19 reads `~/Library/KHUpdater/state/hold-token` fresh on every request, names the same file in `KHHoldTokenFile`, and `tests/hold.sh` proves the updater's exact request, a live rotation, a busy app taking the hold, and an executable replacement in the gap. Shipped work now lives in `CHANGELOG.md` and `docs/receipts/0.1.19/RECEIPT.md`.
- [ ] **A settings window** — the owner named this first: brightness keys, login item and the other switches currently sit inside the dropdown, and a real app needs a window for them. **Next:** move the existing switches into a settings window, leave the dropdown as the quick panel, and measure it at 375/768/1440 in light and dark.
- [ ] **A display test step, and a fallback for a monitor that answers nothing** — every read failure is reported per control today, but there is no standalone "test this display" action and no software (gamma) fallback, so a monitor without DDC has no brightness control at all. **Next:** add the test action that names the failure, then the gamma fallback behind it, each with a test.
- [ ] **The brightness keys should follow a chosen display** — the keys currently step every external display at once; there is no notion of a selected display. **Next:** introduce the selection, make `step()` write only to it, and prove the single-display path in the existing key tap tests.
- [ ] **The owner's own look at the 0.1.18 HUD, and its focus behaviour** — `hud-selftest` recorded the HUD's focus preservation as `UNMEASURED — session locked, no app in front`, and no in-app screenshot of the native HUD exists; it was measured offscreen so the desktop was never touched. **Next:** with 0.1.18 installed and in use, press F1/F2 while another app is frontmost and record whether focus stays put.
- [ ] **The two-monitor tray click and the login restore** — written on the 0.1.17 receipt as not exercised, still unproven. **Next:** on a Mac with two displays, click the tray icon with the second monitor attached, then restart and confirm the app returns.

Nothing above is a regression: the per-display DDC control, the single slider for all displays and the
menu-bar panel all ship and work. What is missing is the shape of an app around them.
