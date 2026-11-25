# What's New

## 0.1.14 — 2026-11-25

- The Mac's F1 and F2 brightness keys now change monitor brightness properly. Previously the app misread the brightness key signal from Apple keyboards and ignored plain F1 and F2 when standard function keys were turned on; both shapes work now.
- By default, pressing F1 or F2 changes the display under the pointer, while Settings still lets you choose every external display or one selected display.
- External monitors are adjusted through the monitor's own brightness control, and a built-in Mac screen uses macOS's own built-in brightness path instead of mixing the two.
- Without Accessibility permission, the app now clearly says "Brightness keys need Accessibility" with a button straight to the right System Settings pane, and records that once per launch.

## 0.1.13 — 2026-11-24

- Settings now has a real screen of its own: the gear in the panel, or Settings… in the menu-bar menu. The three switches that used to sit in the panel moved here, with the choices that belong beside them.
- Each external display can carry its own brighter and dimmer shortcut, recorded by pressing the keys you want. A chord belongs to one display and one direction, so two displays can never answer the same press.
- The brightness keys — the Mac's F1/F2 and the global pair — can move every external display, as before, or only the display you select here.
- If macOS has not granted Accessibility, the brightness-keys setting says so in plain words and opens the Accessibility pane in one click.

## 0.1.12 — 2026-11-23

- Saved presets. Name the settings your monitors report right now — Evening, Day, whatever fits — and put them back with one click. A preset remembers only what a monitor actually reported, writes only to the display it was saved from, and says so in plain words when a display is no longer connected instead of changing a different one.

## 0.1.11 — 2026-11-23

- Fixes the update check refusing to start. The bundled update framework looked for the token file that lets it ask for a quiet moment, and could not read a path written with `~` — so it gave up before checking, silently. It now expands that path. Nothing else changes.

## 0.1.10 — 2026-11-23

- Same app as 0.1.9. It is the first release the app can install by itself: from here on, updates arrive on their own instead of someone installing a new copy.

## 0.1.9 — 2026-11-23

- Fixes the app quitting a second after it started: the self-signed app and its bundled update framework were treated as two different publishers, so macOS refused to load it. The release now carries the entitlement that lets our own framework load, verified inside the shipped app rather than in the build folder.
- The release gate now launches the app it built and fails if it does not survive, so this class of fault cannot pass again.

## 0.1.8 — 2026-11-23

- The app can now update itself: it checks its own signed feed, takes the update quiet point before downloading, and installs without a window or a prompt. This is the Sparkle pilot's baseline build — installing it by hand is the last time an update is installed by hand.

## 0.1.7 — 2026-11-23

- The app now asks macOS for the Accessibility permission the brightness keys need, instead of waiting to be told: on the first launch with the keys on, and whenever the switch is turned on without it. If it is still not allowed, the panel says so and its button opens the right System Settings pane.
- The panel fixes first shipped in 0.1.4 reach you with this version: it drops from the menu-bar icon like a menu instead of opening as a window, it is sized so nothing needs scrolling, and the permission note sits at the top with its button.

## 0.1.6 — 2026-11-23

- Same code as 0.1.5. Republished so the feed's current and previous are both scan-clean, which lets the release page carry current + previous again.

## 0.1.5 — 2026-11-22

- Build the release binary with remapped source paths, so the package carries no local build path (the public release scan stops on one).

## 0.1.4 — 2026-11-22

- The panel drops from the menu-bar icon like a menu (no title bar, anchored under the icon) and folds away when you click elsewhere.
- Sized so everything fits: no scrolling, no cut-off controls, and the hidden check now refuses a vertically overflowing panel.
- The F1/F2 Accessibility fix is a banner at the top of the panel with its Open System Settings button, not a footnote at the bottom.

## 0.1.3 — 2026-11-22

- Move the update hold to its own loopback port (8650). 0.1.2 used 8649, which Wire Studio has served since its 0.1.0, so whichever app started second could not be held and a hold could land on the wrong app.

## 0.1.2 — 2026-11-22

- Only one copy can run: launching it again opens the panel instead of adding a second menu-bar icon.

## 0.1.1 — 2026-11-22

- Use the Mac's own F1/F2 brightness keys for external displays, once you allow Accessibility.
- Move every display together with one slider.
- Start automatically when you log in, with a switch in the panel.
- A first-run explanation says what the app does and what to do when a monitor does not answer.
- A setting the monitor does not report now says "Not reported" instead of showing 0.

## 0.1.0 — 2026-11-21

- Change the brightness of compatible external monitors from the menu bar.
- Adjust contrast and monitor volume where the monitor reports those controls.
- Make your displays brighter or dimmer with a keyboard shortcut, even when the panel is closed.
