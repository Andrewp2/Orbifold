# Lumatone Troubleshooting And Manual Validation

Orbifold can load Scala scales and Lumatone key maps, but it does not yet
automatically validate that the active `.scl` and `.ltn` belong together. Use
this checklist when a Lumatone setup is silent, partially working, or tuned
wrong.

## Baseline Checks

1. Open `DEVICES`.
2. Refresh MIDI inputs and connect the Lumatone input.
3. Set the MIDI channel filter to `Ch All` unless you are intentionally testing
   one channel.
4. Load the intended Scala scale with `Scale`.
5. Load the intended key map with `Keys`, or choose a matching factory preset.
6. Play one key and confirm the last-MIDI label changes.

If the last-MIDI label does not change, debug MIDI connection first. If the
last-MIDI label changes but there is no sound, debug audio output, channel
filter, and synth state next.

## Manual Scale And Key-Map Validation

Match the active scale and key map yourself:

- `19-EDO` scale with `5. 19 EDO.ltn`
- `24-EDO` scale with `7. 24 EDO.ltn`
- `31-EDO` scale with `8. 31 EDO.ltn`
- `53-EDO` scale with `9. 53 EDO.ltn`

A successful key-map load reports:

```text
Loaded key map: <name> (<key count> keys)
```

That key count only means Orbifold parsed `Key_` and `Chan_` pairs. It does not
prove the map matches the active tuning, root note, base frequency, or physical
device layout.

Normal MIDI keyboards and Lumatone input are treated differently. A normal MIDI
keyboard maps chromatic MIDI notes to nearby degrees in the active tuning. With
a loaded Lumatone map, Orbifold uses the map to identify the physical key and
uses the incoming MIDI note value as the musical note in the active tuning. If a
Lumatone chord sounds wrong, verify that the device is sending the note numbers
expected by the loaded `.ltn` file and that the active scale has the expected
degree count.

## Mapping Capture

Use mapping capture when keys arrive on unexpected channels or notes:

1. Set `Ch All`.
2. Click `Capture`.
3. Press a small, known group of Lumatone keys.
4. Click `Stop`.
5. Check the captured count and last-MIDI label.
6. Click `Clear` before another pass.

Capture is diagnostic only. It does not save a new `.ltn`, compare the capture
against the loaded key map, or program the hardware.

## Common Symptoms

### No MIDI Activity

- Confirm the Lumatone appears in `MIDI INPUTS`.
- Click `Refresh`, select the Lumatone row, then `Connect`.
- Make sure another app is not holding the device exclusively.
- Run `cargo run -- --startup-probe` to collect startup diagnostics without
  opening the full UI.

### MIDI Activity But No Sound

- Confirm an audio output is connected in `AUDIO OUTPUTS`.
- Confirm the filter says `Ch All`, not only `Ch 1`, `Ch 2`, or another single
  channel.
- Use `A4` to test output.
- Use `P` or `All Off` if notes are stuck or the synth state is suspect.

### Only Some Keys Work

- Set the filter to `Ch All`; Lumatone layouts commonly use more than one MIDI
  channel.
- Reload the `.ltn` file and check the key count in the status bar.
- A key-map entry only becomes usable when both `Key_<n>` and `Chan_<n>` exist
  for that key.

### Chords Sound Wrong

- Confirm scale, root, and base frequency.
- Confirm the `.ltn` preset name matches the loaded scale family.
- Check whether you are playing a normal MIDI keyboard or the Lumatone. Normal
  MIDI input is remapped to nearby scale degrees; Lumatone input with a loaded
  map uses the incoming note value as the musical note.
- Try a matching factory pair such as `31-EDO` with `8. 31 EDO.ltn`.

### Key Map Fails To Load

Common parse errors include:

- `No Key/Chan pairs found in key map`
- `Invalid MIDI channel <n> in line: Chan_<k>=<n>`
- `Invalid MIDI note <n> in line: Key_<k>=<n>`

The parser reads `Key_`, `Chan_`, and `Col_` entries. Missing color entries are
allowed. Missing key/channel pairs mean those keys are not part of the loaded
map.

### Saved Project Opens Without The Expected Map

If a project references a missing `.ltn`, Orbifold loads the project, clears the
active map, and reports the missing path. Load another key map with `Keys` or
restore the missing file at its saved path.

## Bug Reports

Include these details when reporting a Lumatone issue:

- Active scale name and degree count.
- Active key-map filename and loaded key count.
- MIDI input name and current channel filter label.
- Last-MIDI label after pressing one known key.
- Whether the key was on a Lumatone or a normal MIDI keyboard.
- Output from `RUST_LOG=info cargo run -- --startup-probe` when setup fails.
- A screenshot from `cargo run -- --screenshot-size=1200x760` if the UI state is
  relevant.

For lower-level audio, MIDI, settings, and logging diagnostics, see
`docs/troubleshooting.md`.
