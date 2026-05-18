# Audio And MIDI Threading Model

Orbifold has three important execution contexts: the UI/main thread, the CPAL
audio callback, and the midir MIDI callback. Keep these boundaries clear when
adding audio, MIDI, recording, or UI features.

## Contexts

### UI/Main Thread

The UI/main thread owns `AppState` and the Operad/winit event loop. It handles
buttons, keyboard shortcuts, pointer gestures, file dialogs, settings writes,
project save/load, device refresh, and document rebuilding.

UI code may lock shared project, MIDI, scale, and synth-handle state briefly, but
it must not run inside the CPAL audio callback.

### Audio Callback

`src/audio.rs` builds a CPAL output stream. The stream owns a `SynthEngine` and a
`Receiver<AudioCommand>`. For each callback buffer it:

1. Drains pending commands with `try_iter`.
2. Applies them to `SynthEngine`.
3. Renders samples with `next_sample`.
4. Writes the same sample to each output channel.
5. Updates meter atomics.

The audio callback must not block on UI state, filesystem I/O, dialogs, logging
loops, or long mutex holds. Send it commands and read back atomics instead.

### MIDI Callback

`AppState::open_midi_input` creates a midir connection and stores it in
`AppState` so the connection stays alive. The midir callback captures cloned
handles for scale state, synth, MIDI shared state, and debug logging.

The MIDI callback calls `handle_midi`, which:

1. Parses the raw MIDI message.
2. Applies Lumatone mapping or normal keyboard tuning mapping.
3. Updates last-event and short MIDI log state.
4. Applies the MIDI channel filter.
5. Updates capture, held-note, sustain, and project recording state.
6. Sends synth note commands through `SynthHandle`.

MIDI handlers should avoid UI structures and long-running work. They may take
short mutex locks around shared state.

## Shared State

Important shared handles:

- `SharedMusicProject`: clip, transport, recording, and project note state.
- `ScaleState`: root, base frequency, and current Scala scale.
- `SynthHandle`: UI/MIDI-facing command sender plus settings, mute state,
  active-note tracking, and meter atomics.
- `SharedMidiLast`: last received MIDI event for the UI.
- `SharedMidiLog`: bounded event history for diagnostics.
- `SharedMidiCapture`: mapping-capture diagnostic buffer.
- `SharedMidiHeld`: currently held MIDI notes.
- `SharedMidiSustain`: sustain pedal policy and deferred note-offs.
- `SharedMidiChannelFilter`: atomic selected channel filter.
- `SharedLumatoneMap`: optional loaded `.ltn` mapping.

Do not add `AppState` to the audio or MIDI callback. Pass only the narrow shared
handle needed for the job.

## Command Flow

Audio commands flow one way:

```text
UI/MIDI/AppState -> SynthHandle -> AudioCommand channel -> SynthEngine
```

Meter and voice state flow back through atomics:

```text
SynthEngine -> active_voice_count/output_level/output_limited atomics -> UI
```

MIDI events flow through shared diagnostic/project state:

```text
midir callback -> handle_midi -> SharedMidiLast/Log/Capture/Held/Sustain
               -> SharedMusicProject
               -> SynthHandle
```

## Device Lifecycle

Audio:

1. `build_audio_stream` chooses a CPAL output device and default stream config.
2. `SynthHandle::make_engine` creates a fresh `SynthEngine`, command receiver,
   and sender for that sample rate.
3. Startup or reconnect calls `stream.play`.
4. On success, `SynthHandle::install_sender` installs the new sender and resets
   active-note/meter state.
5. On failure, Orbifold clears or avoids installing the sender and reports a
   visible/logged error.

MIDI:

1. `open_midi_input` lists ports and picks the selected input.
2. The midir connection captures cloned shared handles.
3. The connection is stored in `AppState::midi_connection`.
4. Reconnecting replaces the stored connection, dropping the previous one.

## Failure Policy

- Recoverable audio/MIDI setup failures should log at error level and surface a
  status message.
- Audio command send failures should be reported by the caller that attempted
  the command.
- MIDI callback command failures are logged because there is no direct UI return
  path from the callback.
- Screenshot mode skips hardware probing; do not persist hardware-derived
  defaults from no-probe startup.

## Rules For New Work

For audio features:

- Keep the CPAL callback real-time-safe.
- Load files, decode samples, allocate large buffers, and parse presets outside
  the callback.
- Send compact commands or prebuilt data to `SynthEngine`.
- Use atomics or short-lived shared state for meters and diagnostics.

For MIDI features:

- Keep parsing and routing inside `midi.rs`.
- Keep the channel-filter and Lumatone-map policy visible in one place.
- Update diagnostic state before filtering when the UI should show ignored
  events.
- Avoid UI/document dependencies from the callback.

For project/recording features:

- Route recorded MIDI through `MusicProject`.
- Keep undo and dirty-state decisions on the UI/AppState side unless the
  recording model explicitly owns the change.
- Avoid holding project locks while sending audio commands when a shorter lock
  scope is practical.

For tests:

- Prefer injected audio/MIDI builders and direct `handle_midi` calls.
- Do not open real windows or hardware devices for deterministic behavior tests.
- Use `--startup-probe` tests for hardware-probe logging behavior.
