# First Run Guide

This guide is for early testers running Orbifold from the source checkout.

## Start Orbifold

Run:

```sh
cargo run
```

If audio or MIDI setup is incomplete, Orbifold opens the right panel in
`DEVICES` mode with `SETUP REQUIRED`.

## Audio Setup

1. In `AUDIO OUTPUTS`, click `Refresh`.
2. Select the output you want.
3. Click `Connect` or `Reconnect`.
4. Use `A4` in the top bar to confirm audio works.
5. Use `All Off` as a panic/reset action if notes hang.

If no output is available, Orbifold should stay open, disable sound-producing
test controls, and report the missing output in the status bar.

## MIDI Setup

1. In `MIDI INPUTS`, click `Refresh`.
2. Select the keyboard or virtual MIDI input you want.
3. Click `Connect` or `Reconnect`.
4. Play a key and watch the last MIDI event in the control panel.
5. Use `Ch All` if you need to filter input to a single MIDI channel.

If no input is available, Orbifold should stay open and report `MIDI no input`.

## Tuning And Key Maps

- Use `Scale` to open a Scala `.scl` file.
- Use the left `Scales` browser for bundled and recent scales.
- Use `Keys` to open a Lumatone `.ltn` key map.
- Use the key-map preset controls in the right panel to cycle bundled maps.
- See `docs/lumatone_setup.md` for scale/key-map matching, factory presets,
  mapping capture, and current limitations. If notes are silent, missing, or
  tuned wrong, see `docs/lumatone_troubleshooting.md`.

Missing scale or key-map files should be reported in the status bar instead of
silently falling back to hidden defaults.

## Record And Edit

1. Choose `Replace` or `Overdub` in the top transport.
2. Toggle `Metronome` if you want a click.
3. Toggle `Rec quantize` in the clip panel if you want recorded notes snapped.
4. Press `Record`, play a phrase, then press `Stop Rec` or `Stop`.
5. Edit notes in the piano roll:
   - Double-click the grid to add a note.
   - Drag note bodies to move pitch or time.
   - Drag note edges to resize.
   - Drag velocity bars in the velocity lane.
   - Use `Delete`, `Duplicate`, `Len -`, `Len +`, `Pitch -`, `Pitch +`, and `Quant`
     for selected notes.
   - Use `Fit` in the piano-roll panel if zooming or scrolling leaves the notes
     out of view.

Use `Ctrl`/`Cmd+Z` and `Ctrl`/`Cmd+Y` for undo and redo.

## Save And Recover

- Use `Save` or `Ctrl`/`Cmd+S` to save the current project.
- Use `Save As` or `Ctrl`/`Cmd+Shift+S` to choose a new project path.
- Use `Open` or `Ctrl`/`Cmd+O` to open a project.
- Unsaved destructive actions ask for a second confirmation.
- Dirty edits write `orbifold_autosave.orbifold`.
- If an autosave exists, use `Recover` to load it as unsaved work or `Dismiss`
  to remove stale recovery data.

## Visual Size

- On large displays, Orbifold scales the UI automatically.
- Use `Ctrl`/`Cmd` plus `+`, `-`, or `0` to adjust or reset UI zoom.
- Drag workspace splitters to give more space to the browser, clip panel,
  editor, right panel, or piano roll.
- Use the piano-roll `Fit` button to recover the clip note view after changing
  piano-roll zoom or scroll.

## Troubleshooting

- If startup reports a settings load error, Orbifold uses defaults but does not
  automatically overwrite the bad settings file.
- If audio or MIDI disappears, use `Refresh` and `Connect` in `DEVICES`.
- If a save fails, the project should remain dirty and the status bar should
  name the failed path.
- If the UI looks wrong, capture a screenshot with
  `cargo run -- --screenshot-size=1200x760` or
  `cargo run -- --screenshot-size=3840x2160` and inspect the result.

For deeper device, logging, settings, project, and autosave diagnostics, see
`docs/troubleshooting.md`.
