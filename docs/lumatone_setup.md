# Lumatone Setup

Orbifold can load Lumatone `.ltn` key maps and use them with the active tuning.
This guide describes the current prototype behavior and the limits that still
matter for testers.

## Scale Versus Key Map

Orbifold treats tuning and keyboard mapping as separate choices:

- A Scala `.scl` scale defines pitches, intervals, root, and frequency behavior.
- A Lumatone `.ltn` key map defines which physical Lumatone key and MIDI channel
  correspond to incoming notes.

For normal MIDI keyboards, Orbifold maps chromatic key positions into the active
scale. For Lumatone input with a loaded `.ltn` map, Orbifold uses the map to
identify the physical key, and the incoming note value is treated as the musical
note in the active tuning.

Loaded key maps are only applied to selected MIDI inputs whose port name contains
`Lumatone`. If a regular MIDI keyboard is selected while a key map is loaded,
the control panel reports `Key map inactive` and the keyboard continues to play
chromatic notes through the active scale.

Orbifold does not yet validate that a selected `.scl` scale matches the selected
`.ltn` key map. If you load `31-EDO`, choose a matching 31-EDO Lumatone preset
yourself.

## Factory Presets

Factory key maps live in `lumatone_factory_presets/`. Current checked-in presets
include:

- `1. Classic Mode.ltn`
- `2. Melodic Mode.ltn`
- `3. Harmonic Mode.ltn`
- `4. Multi-Instrument.ltn`
- `5. 19 EDO.ltn`
- `6. 22 EDO.ltn`
- `7. 24 EDO.ltn`
- `8. 31 EDO.ltn`
- `9. 53 EDO.ltn`
- `10. 56 Polychromatic.ltn`

On startup, Orbifold loads factory presets and selects the saved key map if it is
available. If no saved map is available, it selects `1. Classic Mode.ltn` when
that preset exists.

Use the key-map previous/next controls in the right control panel to cycle
factory presets without opening a file dialog. Use the key-map refresh action
after adding or removing `.ltn` files from `lumatone_factory_presets/`.

## Loading A User Key Map

Use `Keys` to open a `.ltn` file. A successful load reports:

```text
Loaded key map: <name> (<key count> keys)
```

The loaded key map path is saved in settings and project files. If a saved
project references a missing key map, Orbifold loads the project, clears the
active map, and reports the missing `.ltn` path instead of silently pretending
the map is active.

## MIDI Setup

1. Open `DEVICES`.
2. In `MIDI INPUTS`, click `Refresh`.
3. Select the Lumatone MIDI input.
4. Click `Connect` or `Reconnect`.
5. Play keys and watch the last-MIDI label in the control panel.

If `Ch All` is set to one channel, notes on other channels are visible in the
last-MIDI monitor but ignored for synth playback, mapping capture, and
recording. Use `Ch All` unless you are intentionally filtering to a single
channel.

## Mapping Capture

Mapping capture is a diagnostic tool for incoming note-ons:

- `Capture` arms recording of MIDI note-on events.
- `Stop` disarms capture and reports the number of captured note-ons.
- `Clear` removes the captured events.

Capture currently helps verify what the device is sending. It does not yet save
a new `.ltn`, review captured maps in a table, or send colors/layouts back to
the Lumatone.

## Current Limitations

- No automatic validation that the active scale and key map match.
- No key-map preview surface.
- No captured-map save/export path.
- No device programming or color send-back.
- No multi-input MIDI policy beyond the selected input.
- Missing map files are reported, but there is no guided repair flow beyond
  loading another `.ltn`.

For wrong notes, missing channels, parse errors, and manual scale/key-map
validation, see `docs/lumatone_troubleshooting.md`. For lower-level MIDI and
logging diagnostics, see `docs/troubleshooting.md`.
