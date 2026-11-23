# Receipt — Displays Studio KH 0.1.12 (DRAFT: the gate has not run yet)

**Status: draft.** Artifact fields and the gate verdict are `TBD` on purpose — the gate is queued
behind the shared test lane. This file is committed now so the tree is clean at HEAD; it is not a
release record until those fields are filled from the gate that passes.

Saved presets: a name plus the settings the external displays reported when it was saved, put back
with one click. The first product change since the update pilot.

## Identity

| what | value |
|---|---|
| app / bundle id | Displays Studio KH / `app.displaysstudiokh.desktop` |
| version | 0.1.12 |
| artifact | `Displays Studio KH-0.1.12.dmg` |
| sha256 | TBD |
| bytes | TBD |
| signing | KH Studio Local Signing — **self-signed, not notarized** |
| commit | `13f5fbd614a78d8f8b8dee57de7dd0adccabb619` (the gated tree) |

## What it does

- Saves what the monitors report right now — brightness, contrast, volume where reported — under a
  name typed in the panel. A setting a monitor did not report is left out, so applying can never
  invent a value.
- Applying writes each setting to the display that reported it, by id. A display that is no longer
  connected is counted as failed and named in the status line, never redirected to another monitor.
- One preset per name: saving under an existing name replaces it where it sits. Delete removes it.

## Checks inside the gate

- `cargo test`: 25 tests, including `presets.rs` (round trip, broken file reads as none,
  replace-in-place, remove, name trimming and its bound) and `hardware.rs` (a preset writes each
  setting to its own display; a preset naming an absent display is failed, not redirected).
- Hidden UI check (isolated data dir, no window): the preset controls exist; `preset_save` captures
  and lists a name; `presets_list` sees it; `preset_delete` removes it. Panel measured at 372×668 —
  **body scrollHeight ≤ 668**, so nothing has to scroll. The check now measures the body's own
  `scrollHeight`; the document's always reads the viewport because the body scrolls inside it.
- Gate verdict: TBD.

## Not exercised in the gate

`preset_apply` against real hardware. It takes the same `set_one` path as the sliders (proven by the
`--ddc-check` read → write → read-back → restore run) and its routing is unit-tested with fake
targets; the apply itself was not driven against the monitor here, because a check that steps the
owner's screen is not a check the gate should run.
