# Receipt — Displays Studio KH 0.1.15

Adds the brightness HUD: a small translucent panel, bottom-centre on the display being adjusted, with a bar and the new percentage, visible for 900 ms and then a 200 ms fade. It is a native non-activating `NSPanel` that never becomes key or main, ignores the mouse, and never appears for the built-in screen. Settings gains a switch for it, on by default.

```
app: displays-studio-kh
version: 0.1.15
commit: PENDING
artifact: dist/Displays Studio KH-0.1.15.dmg
sha256: PENDING
bytes: PENDING
gate config: $HOME/Developer/_orchestra/tools/gate-configs/displays-0.1.15.sh
gate exit: PENDING
hud selftest: PENDING
feed readback: PENDING
```

## What it does

- A press that changes an external display's brightness shows a panel on that display: bottom-centre, 172×44, a bar plus the new percentage, HUD material, native AppKit — no webview, no extra dependency.
- Visible for 900 ms, then a 200 ms fade and hidden. One short-lived thread per press; nothing runs while no key is pressed. A newer press cancels an older fade, so a held key never hides a fresh value.
- The panel is a `NonactivatingPanel` subclass whose `canBecomeKeyWindow` and `canBecomeMainWindow` are both false, it ignores mouse events, and it sits at `NSStatusWindowLevel` on every Space.
- Never shown for the built-in display (macOS already draws its own brightness HUD there), and only for a display the step actually changed.
- Settings › Brightness keys gains "Show a brightness bar on the monitor I change", on by default.

## Measured behaviour (offscreen self-test, `DISPLAYS_STUDIO_HUD_SELFTEST=1`)

PENDING — filled from the gate's `hud-selftest` step.

## Not proven

- **Focus, on an unlocked Mac with a real app in front.** The self-test ran while the owner's session was locked, so it recorded focus as UNMEASURED rather than claiming a result. On a locked session the frontmost app did not change when the panel appeared.
- The owner has not yet seen the HUD on his own screen.
