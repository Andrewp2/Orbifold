# Orbifold

A small Rust desktop prototype for playing microtonal scales from MIDI input.

## Features

- `eframe`/`egui` desktop interface
- Selectable `cpal` audio output with reconnect controls
- `midir` MIDI input selection with reconnect controls
- Persistent settings in `orbifold_settings.txt`
- Project save/load for the current scale, Lumatone mapping, synth settings, transport, and clip
- Scala `.scl` scale loading, recent/library scale list, root note, and base-frequency controls
- Lumatone `.ltn` preset mapping with preset colors and active-key display
- MIDI/scale inspector showing channel, note, mapped key, scale degree, frequency, and cents from root
- Performance recorder with looping transport, overdub, metronome, quantize-on-record, clip quantize, and clip view
- Piano roll editing with pitch lanes, beat grid, note creation, note selection, delete, duplicate, nudge, transpose, resize, velocity editing, undo, and redo
- Polyphonic synth with command-queue audio control, master gain, waveform, attack, release, drive, low-pass filter, delay, and soft clipping

## Music Workflow

1. Connect audio and MIDI from the Options menu.
2. Load a Scala scale and Lumatone `.ltn` preset, or use the current saved defaults.
3. Press `Record`, play a phrase, then press `Stop Rec`.
4. Use `Quantize Clip` or change the quantize grid to tighten timing.
5. Use the piano roll to add notes, select notes, delete, duplicate, move, transpose, resize, or change velocity.
6. Press `Play` to loop the clip and shape the sound from the Synth menu.
7. Use `File > Save Project As...` to save the full working state.

## Audio Assets

Reusable sound content belongs in `audio_assets/`:

- `audio_assets/samples/` for one-shots, loops, recordings, and imported clips
- `audio_assets/instruments/` for instrument definitions, multisample maps, and sample sets
- `audio_assets/presets/` for synth, effect, routing, and project sound presets
- `audio_assets/impulses/` for impulse responses and convolution assets

The left browser scans these folders on startup. Use `Refresh` after manually adding files, or `Import` to copy a file into the selected asset category.

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

Use `File > Take Screenshot` in the app to capture the current window. Screenshots are written to `screenshots/` as timestamped PNGs, and `screenshots/latest.png` is updated for quick inspection.

When working with Codex, the visual review flow is:

```sh
cargo run -- --screenshot
```

Then inspect `screenshots/latest.png`. For manual review, run `cargo run` and use `File > Take Screenshot`.

## Error Handling

Errors should be handled according to whether the app can still do useful work.

- Unrecoverable startup failures should be reported to stderr and crash.
- Recoverable runtime failures should be shown in the UI status area.
- Parsers should reject malformed input instead of substituting hidden defaults.
