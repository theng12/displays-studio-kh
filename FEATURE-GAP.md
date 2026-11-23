# Displays Studio KH — gap against Lunar

Audit written 2026-11-23 at U001's request, after the owner said: *"it has no settings, monitor - connection
and testing step, and it lacks a lot of features from Lunarr, the app we were replicating from."*

**Read this first.** Three of the four bugs the owner reported were fixed in **0.1.4**; his Mac runs
**0.1.1**, because it cannot update itself yet — that is the defect behind the report, not missing code.
The permission ask is the only genuinely new code in 0.1.7. Nothing below is a regression: the product is
unfinished. Evidence is a file and symbol, never an impression.

**Columns:** capability · **Lunar** (what it does) · **This app** (what exists in the tree today) ·
**Status** · **Since** (the version it landed in; `—` for nothing shipped) · **Evidence / the gap**.
Status is one of present, partial, missing, deliberately not, **uncertain**. `missing` means **absent from
the current tree**, never "the owner has not seen it" — anything fixed but not yet on his Mac is `present`
with the invisibility noted separately. `uncertain` is used where Lunar's exact behaviour was not confirmed,
never guessed.

| Capability | Lunar | This app | Status | Since | Evidence / the gap |
|---|---|---|---|---|---|
| Panel drops from the menu-bar icon, no scrolling | Yes, menu-style | Borderless panel anchored under the icon, sized to fit, hides on click-away | present | 0.1.4 | `main.rs` `WebviewWindowBuilder` `decorations(false)`, tray-click anchor; `webview-check.js` refuses vertical overflow |
| Accessibility permission asked for, state named on screen | Yes | Asks once on first real launch with the keys on, and on every manual enable; panel names the missing grant with a working button | present | 0.1.7 | `brightness_keys.rs` `request_accessibility()`, `main.rs` startup + `set_brightness_keys` |
| DDC/CI brightness, per display | Yes | Yes, slider per display | present | 0.1.0 | `hardware.rs` `BRIGHTNESS = 0x10`, `set()`; `panel.js` per-display rows |
| Contrast, per display | Yes | Yes, when the monitor reports it | present | 0.1.0 | `hardware.rs` `FEATURES` carries `0x12`; unreported controls are never drawn |
| Volume, per display | Yes | Yes, when the monitor reports it | present | 0.1.0 | `FEATURES` carries `0x62`; `hardware.rs:236` test proves "Not reported" renders as a sentence, not 0 |
| One slider moves every display | Yes | Yes | present | 0.1.1 | `hardware.rs` `set_all()`, `panel.js` `#all-displays` |
| Keyboard brightness keys reach external displays | Yes | Taps the system-defined brightness keys and steps external displays | present (unproven on hardware) | 0.1.1 | `brightness_keys.rs` `CGEventTapCreate` HID keyDown tap; no real F1/F2 press recorded yet |
| Software (gamma) dimming fallback | Yes, for displays without DDC | None | missing | — | No `CGSetDisplayTransferByFormula` anywhere; a monitor with no DDC has no brightness control |
| XDR / extended brightness | Yes | None | missing | — | No `IODisplaySetFloatParameter`/`GetFloatParameter` calls |
| Bezel and per-display identification ("which one is this?") | Yes, flashes each display | None | missing | — | `scan()` returns names only; no way to point at the physical monitor |
| DDC probe and test step | Yes, tests a display and reports the failure | Partial: every read failure is reported per control | partial | 0.1.0 | `read()` returns the failure text, but there is no standalone "test this display" step and no fallback offered when it fails |
| Keyboard keys follow the selected display | Yes | Partial: the keys step **every** external display | partial | 0.1.1 | `hardware.rs` `step()` loops over all monitors; there is no selection concept yet |
| Extra hotkeys per display | Yes | One global pair for brighter/dimmer | partial | 0.1.0 | `main.rs` `keys()`, one shortcut pair; no per-display bindings |
| Menu-bar icon options (icon style, show value) | Yes | Fixed template icon; tooltip carries the last step | partial | 0.1.0 | `main.rs` `TrayIconBuilder` `icon_as_template(true)`, `set_tooltip`; no options |
| Saved presets | Yes | None | missing | — | No preset model, no storage, no command |
| App-linked presets | Yes | None | missing | — | Same; needs presets first |
| Scheduled presets (time of day) | Yes | None | missing | — | No scheduler; a timer would also collide with "nothing runs when idle" |
| Sunrise/sunset scheduling | Yes | None | missing | — | Needs location input and a scheduler |
| Blackout | Yes | None | missing | — | No way to keep a display dark while staying "on" |
| Input-source switching (DDC 0x60) | Yes | None | missing | — | `0x60` appears only as an error-path test string (`hardware.rs:246`) |
| Adaptive / ambient brightness | Yes, from the ambient light sensor | None | missing | — | No AmbientLightSensor use |
| Per-display custom names | Yes | None | missing | — | `scan()` uses the monitor's own product name |
| Settings window | Yes, a full window | None — three switches live inside the dropdown | missing | — | No settings window exists; this is the piece the owner named first |
| Display rotation / resolution | Yes | None | uncertain | — | Lunar does some of this; the full extent was not confirmed, so it is not priced |
| CLI / URL scheme | Yes | None | uncertain | — | Exists in Lunar; whether we want a second API surface is a product decision, not a gap measured here |
| Login item | Yes | Yes, switchable | present | 0.1.1 | `login.rs`; `launch-at-login` preference |
| One instance, panel reopens | Yes | Yes | present | 0.1.2 | single-instance guard; `RunEvent::Reopen` |
| Touch Bar support | Yes (historical) | None | deliberately not | — | No current Mac ships one; no roadmap item |

**What the table says in one line:** the app already does the hard part well — per-display DDC controls, one
slider for all, the Mac's own keys, and (since 0.1.4) a real menu-bar dropdown — and lacks the shape of an
app around it: a settings window, a display setup/test step, presets, scheduling, and every fallback for a
monitor that answers nothing.

**Order that follows from it** (U001, 2026-11-23, not reordered): 0.1.7 the four owner bugs → 0.1.8 the
Sparkle baseline → 0.1.9 the self-update proof → 0.1.10+ product work: settings window, connection and
DDC test with software fallback, key sync, per-display hotkeys, presets; then scheduling, app-linked
presets, XDR and input switching.
