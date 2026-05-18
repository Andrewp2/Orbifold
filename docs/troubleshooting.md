# Troubleshooting

This guide is for early testers diagnosing startup, audio, MIDI, and file
problems without editing source code.

## Before Diagnosing Hardware

Run the normal app first:

```sh
cargo run
```

If audio or MIDI setup is incomplete, Orbifold should still open and show the
right panel in `DEVICES` mode with `SETUP REQUIRED`. Use that panel before
restarting the app:

1. Click `Refresh` in `AUDIO OUTPUTS` or `MIDI INPUTS`.
2. Select the device row you want.
3. Click `Connect` or `Reconnect`.
4. For audio, use `A4` only after an output is connected.
5. For MIDI, play a note and watch the last-MIDI label in the control panel.

Screenshot mode intentionally skips audio and MIDI hardware probing. Use it for
visual QA, not device diagnosis:

```sh
cargo run -- --screenshot-size=1200x760
```

## Startup Probe

Use `--startup-probe` when the app should inspect hardware without opening a
window:

```sh
cargo run -- --startup-probe
```

Orbifold routes routine Linux ALSA/JACK backend diagnostics through logging
instead of printing raw backend spam to the terminal. The default log level is
`warn`.

For more detail, raise logging:

```sh
RUST_LOG=info cargo run -- --startup-probe
```

On Linux, ALSA and JACK backend diagnostics are available as trace targets:

```sh
RUST_LOG=orbifold::alsa=trace,orbifold::jack=trace cargo run -- --startup-probe
```

Use `RUST_LOG=off` only when checking that no raw backend diagnostics leak to
stdout or stderr.

## Audio Problems

If Orbifold reports no audio output:

- Confirm the operating system can see and use the output device.
- Open `DEVICES`, click audio `Refresh`, select an output, then click `Connect`.
- If a saved output disappeared, Orbifold should report that the saved output is
  unavailable and try the system default.
- If no stream is connected, sound-producing controls such as `A4` are disabled.
- `All Off` remains available because it clears live held-note and sustain state
  even when no audio stream is active.

If a connected output later disappears:

- Open `DEVICES`.
- Click audio `Refresh`.
- Select a replacement output and click `Connect`.
- Save settings after choosing the replacement if you want that device remembered.

When reporting an audio issue, include the visible audio status, selected device
name, connected device name if any, operating system, and whether
`cargo run -- --startup-probe` reports an Orbifold error.

## MIDI Problems

If Orbifold reports no MIDI input:

- Confirm the keyboard or virtual MIDI source is visible to the operating system.
- Open `DEVICES`, click MIDI `Refresh`, select an input, then click `Connect`.
- Play a note and check whether the last-MIDI label updates.
- Check `Ch All`; if it is set to one channel, notes on other channels are
  monitored but ignored for synth playback, capture, and recording.

If the connected MIDI device is unplugged or replaced:

- Click MIDI `Refresh`.
- Select the new input and click `Connect`.
- Use `All Off` if any held-note or sustain state seems stale.

Current MIDI policy:

- Sustain pedal CC64 is handled for live playback and recording.
- Pitch bend and non-sustain controller messages are monitored but ignored by
  the synth.
- Orbifold currently connects one selected MIDI input at a time.

When reporting a MIDI issue, include the visible MIDI status, selected input,
connected input if any, last-MIDI label, channel filter, and whether the device
is a normal keyboard, virtual port, or Lumatone.

## Settings Problems

Settings live in `orbifold_settings.txt`. If that file is missing, Orbifold may
load the legacy `microtonal_daw_settings.txt` fallback. If settings fail to
parse, Orbifold should use defaults for that run and leave the bad settings file
alone until an explicit later settings save.

Use `Save Settings` only after confirming the current device, layout, scale
library, and UI zoom preferences are the state you want to keep.

## Project And Autosave Problems

Project files use the `.orbifold` extension. Dirty edits write
`orbifold_autosave.orbifold`; if an autosave exists on startup, the left session
strip shows `Recover` and `Dismiss`.

- Use `Recover` to load autosave data as unsaved work.
- Use `Dismiss` only when you do not need the recovery file.
- Failed save/open/recover paths should name the relevant project or autosave
  path in the status bar.
- A failed save should leave the project dirty.

See `docs/file_formats.md` for the current settings, project, autosave,
temporary-save, and backup file details.

## What To Include In A Bug Report

Include:

- The Orbifold version shown in the status bar.
- The exact visible status message.
- Operating system and desktop/audio stack if relevant.
- Whether the issue happens in normal startup, `--startup-probe`, or screenshot
  mode.
- Device names as shown in `DEVICES`.
- The current project path or whether the project is unsaved.
- A screenshot when the problem is visual.

