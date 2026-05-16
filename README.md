# Orbifold

Orbifold is a Rust desktop prototype for microtonal MIDI performance,
Lumatone-oriented tuning workflows, and loop-based composition.

The current UI is rendered with Operad. Orbifold is still a prototype: the
single-clip recording and piano-roll workflow exists, while the DAW-like
multi-track arrangement is still being wired to real project state.

## Features

- Operad desktop interface
- Selectable `cpal` audio output with reconnect controls
- `midir` MIDI input selection with reconnect controls
- Persistent settings in `orbifold_settings.txt`
- Saved/opened project paths are remembered as recent projects
- Recent project rows can reopen or forget saved/opened projects without
  deleting the project file
- Missing recent project files are marked in the project panel and can be
  forgotten without restarting
- Project save/load for the current scale, Lumatone mapping, synth settings,
  transport, and clip
- Dirty project autosave to `orbifold_autosave.orbifold`, cleared after a
  successful save or return to a clean project state
- Window title reflects the open project name and unsaved state
- Scala `.scl` scale loading, recent/library scale list, root note, and base-frequency controls
- Lumatone `.ltn` preset mapping with preset colors and active-key display
- MIDI/scale inspector showing channel, note, mapped key, scale degree, frequency, and cents from root
- Performance recorder with looping transport, overdub, metronome, quantize-on-record, clip quantize, and clip view
- Piano roll editing with pitch lanes, beat grid, note creation, note selection, delete, duplicate, nudge, transpose, resize, velocity editing, undo, and redo
- Polyphonic synth with command-queue audio control, output mute/metering, master
  gain, waveform, attack, release, drive, low-pass filter, delay, and soft
  clipping

## Current Status

Orbifold is not yet a finished DAW. The current usability focus is to make the
app reliable, honest about incomplete workflows, and safe to run on machines
without a full audio/MIDI setup.

Known current limitations:

- The visible multi-track arrangement is still ahead of the project model.
- Some controls are present as workflow scaffolding while deeper behavior is
  still being implemented.
- The app can launch without an audio output. Sound-producing controls such as
  A4 are disabled until an output is connected, while All Off remains available
  as a panic/reset action.
- Visual layout is actively changing as the Operad integration matures.

## Music Workflow

1. Launch the app with `cargo run`.
2. Use the audio controls to select, refresh, and connect an output if one was
   not connected at startup.
3. Use the MIDI controls to select, refresh, and connect an input.
4. Use `Ch All` in the control panel to cycle a MIDI channel filter when you
   want Orbifold to respond to only one incoming channel. The selected filter is
   saved with the rest of the app settings.
5. Use `Mute` in the control panel when you need to silence output without
   changing the saved master gain.
6. Load a Scala scale and Lumatone `.ltn` preset, or use the current saved
   defaults.
7. Toggle `Metronome` in the right control panel if you want a click while
   recording.
8. Toggle `Rec quantize` in the clip panel to choose whether new recordings
   snap to the active grid.
9. Use the transport mode button to choose `Replace` or `Overdub`, then press
   `Record`, play a phrase, and press `Stop Rec` or `Stop` to finish recording.
10. Use the quantize and piano-roll controls to tighten timing and edit notes.
11. Press `Play` to loop the clip.
12. Use `Save`, `Save As`, `Open`, `Open Recent`, or a visible recent-project
    row for project files.

Keyboard shortcuts include the core editing and transport paths below. See
`docs/keyboard_shortcuts.md` for the fuller reference.

- `?` shows a compact shortcut reference in the status bar.
- `Tab` and `Shift+Tab` move focus through visible controls; `Enter` activates
  the focused control.
- `Space` toggles playback.
- `R` toggles recording.
- `M` toggles the metronome.
- `Q` quantizes the selected note, or the whole clip when no note is selected.
- `Shift+Q` toggles record quantize.
- `G` toggles piano-roll snap off and back to the previous grid value.
- `P` runs All Off as a panic/reset action.
- `N` adds a note at the playhead.
- `D` duplicates the selected note.
- `Home` returns the playhead to the loop start.
- Arrow keys move or transpose the selected note.
- `Shift` plus left/right resizes the selected note.
- `Shift` plus up/down adjusts selected-note velocity.
- `Delete` or `Backspace` deletes the selected note.
- `Ctrl`/`Cmd+C` copies the selected note.
- `Ctrl`/`Cmd+V` pastes the copied note at the playhead.
- `Ctrl`/`Cmd+N` starts a new project; unsaved changes require a second
  confirmation.
- `Ctrl`/`Cmd+S` saves.
- `Ctrl`/`Cmd+Shift+S` saves as a new project path.
- `Ctrl`/`Cmd+O` opens; unsaved changes require a second confirmation.
- `Ctrl`/`Cmd+Z` and `Ctrl`/`Cmd+Y` undo and redo.
- `Ctrl`/`Cmd` plus `+`, `-`, or `0` adjusts or resets UI zoom.
- `Esc` cancels a pending discard confirmation, or clears the selected note
  when no confirmation is pending.

The right control panel also has visible zoom controls for the same UI scale
setting.

Closing the window with unsaved project changes requires a second close request,
so an accidental window close does not immediately discard work.

## Audio Assets

Reusable sound content belongs in `audio_assets/`:

- `audio_assets/samples/` for one-shots, loops, recordings, and imported clips
- `audio_assets/instruments/` for instrument definitions, multisample maps, and sample sets
- `audio_assets/presets/` for synth, effect, routing, and project sound presets
- `audio_assets/impulses/` for impulse responses and convolution assets

The left browser scans these folders on startup. Use `Refresh` after manually adding files, or `Import` to copy a file into the selected asset category.

The browser is currently a library surface. Full sample/instrument assignment is
still part of the usability backlog.

## Development

```sh
cargo fmt
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

Run the app with:

```sh
cargo run
```

## Visual QA

Use screenshot mode to capture the current Operad surface:

```sh
cargo run -- --screenshot
cargo run -- --screenshot-size=1200x760
```

Screenshots are written to `screenshots/` as timestamped PNGs, and
`screenshots/latest.png` is updated for quick inspection. Screenshot mode skips
audio and MIDI hardware probing so visual QA can run on machines without working
devices, and it avoids saving device settings from that no-probe startup. Use
`--screenshot-size=WIDTHxHEIGHT` to capture a specific output image size, such
as the minimum supported layout or a 4K monitor check.

Visual review matters. Passing tests or producing a PNG does not prove the UI is
usable; inspect the image after layout changes. For broader checks, use
`docs/manual_qa_checklist.md`.

Useful development checks:

```sh
cargo fmt --check
cargo check
cargo test
cargo run -- --screenshot
cargo run -- --screenshot-size=3840x2160
```

Then inspect `screenshots/latest.png`.

## Error Handling

Errors should be handled according to whether the app can still do useful work.

- Graphics/device initialization failures may still be unrecoverable.
- Missing audio or MIDI hardware should leave the UI usable with a visible status.
- Recoverable runtime failures should be shown in the UI status area.
- Parsers should reject malformed input instead of substituting hidden defaults.
- Dirty project edits are written to `orbifold_autosave.orbifold` as a basic
  recovery file. If that file exists on startup, the project panel shows a
  `Recover` action that loads it as an unsaved project and a `Dismiss` action
  that removes stale recovery files from a clean project.

## Usability Backlog

The current usability gap inventory lives at
`docs/orbifold_usability_gap_analysis.md`.
