# Architecture Overview

This is a high-level map of the current Orbifold codebase. It is meant to help
contributors decide where a change belongs before touching UI, audio, MIDI, or
project code.

## Startup

`src/main.rs` is the executable entry point. It initializes logging, parses
startup flags, builds `AppState`, and then hands control to `ui::run`.

Important startup modes:

- Normal startup probes audio and MIDI hardware.
- `--startup-probe` builds the app state, probes hardware, logs diagnostics, and
  exits without opening a window.
- `--screenshot` and `--screenshot-size=WIDTHxHEIGHT` skip hardware probing so
  visual checks can run without audio or MIDI devices.

Startup should report recoverable setup problems through status/logging instead
of panicking. Fatal errors are reserved for conditions where the app cannot do
useful work.

## Core State

`src/app.rs` owns `AppState`, the central mutable application state used by the
UI. It coordinates:

- settings and persistence,
- current scale and Lumatone map,
- selected audio and MIDI devices,
- the shared music project,
- synth settings and audio commands,
- asset browser state,
- piano-roll viewport state,
- status-bar messages and recoverable errors.

Most UI actions should call an `AppState` method rather than editing lower-level
fields directly. That keeps status text, persistence, dirty state, logging, and
undo behavior in one place.

Shared musical/audio state uses explicit handles:

- `SharedMusicProject` is the project/clip/transport model behind playback and
  recording.
- `ScaleState` maps musical notes to frequencies and scale-degree labels.
- `SynthHandle` is the UI/MIDI side of the synth command queue.
- MIDI last-event, log, capture, held-note, sustain, channel-filter, and
  Lumatone-map state are shared with MIDI handling through mutexes and atomics.

## Audio And Synth

`src/audio.rs` owns CPAL output discovery and stream construction. It builds a
`SynthEngine`, connects it to a command receiver, and calls `next_sample` from
the audio callback.

`src/synth.rs` separates the real-time engine from the UI-facing handle:

- `SynthHandle` validates and sends commands from app/UI/MIDI code.
- `SynthEngine` lives on the audio callback side and renders samples.
- Communication crosses the thread boundary through command queues and atomics,
  not by sharing `AppState` with the audio thread.

The audio callback must stay deterministic and non-blocking. New audio features
should avoid filesystem I/O, logging-heavy paths, and UI locks inside the
callback.

The detailed audio/MIDI threading model lives in
`docs/audio_midi_threading.md`.

## MIDI

`src/midi.rs` owns MIDI input listing, connection, message parsing, Lumatone
mapping, channel filtering, sustain policy, capture, held-note state, synth note
commands, and recording into `MusicProject`.

Normal MIDI keyboards and Lumatone input use different note mapping:

- Normal MIDI input maps chromatic key positions to nearby degrees in the active
  tuning.
- Lumatone input with a loaded `.ltn` map uses the map to identify the physical
  key and treats the incoming MIDI note value as the musical note.

MIDI handlers update shared MIDI state and the project model, then send synth
commands through `SynthHandle`. They should not depend on Operad UI structures.

## Project And Files

`src/project.rs` contains the current project model: clip notes, transport,
recording state, project snapshots, project file parsing, and project file
writing.

`src/settings.rs` contains app settings, including devices, scale/key-map paths,
UI scale, view preferences, layout sizes, synth settings, MIDI channel filter,
scale library, and recent projects.

Developer details for plain-text project, backup, autosave, and settings formats
live in `docs/file_formats.md`.

## UI Layers

`src/ui/mod.rs` is the UI entry point. The current implementation is the native
Operad path in `src/ui/native.rs` and `src/ui/native/`.

The native UI is split by responsibility:

- `native.rs`: host state, native event handling, document construction,
  document construction, and top-level Operad wiring.
- `native/interactions.rs`: text edits, keyboard forwarding, canvas input,
  cursor updates, drag routing, timeline seeks, piano-roll gestures, and
  workspace resize gestures.
- `native/windowing.rs`: initial window sizing and title formatting.
- `native/screenshot.rs`: screenshot rendering, screenshot UI scale, and pixel
  validation.
- `native/top_bar.rs`: top transport and file/tuning controls.
- `native/browser.rs`: left browser, scale list, asset list, list scrolling, and
  browser split layout.
- `native/devices.rs`: right-panel device setup and device picker controls.
- `native/control_panel.rs`: right-panel synth, MIDI, audio, and capture
  controls.
- `native/editor_panels.rs`: clip panel and piano-roll option controls.
- `native/surfaces.rs`: custom rendered arrangement and piano-roll surfaces.
- `native/piano_interaction.rs`: piano-roll hit testing, drag modes, cursor
  behavior, and pointer interpretation.
- `native/workspace.rs`: workspace panel sizing, splitters, and layout limits.
- `native/controls.rs`: shared low-level control helpers.
- `native/presenters.rs`: formatting/presentation helpers for UI rows and
  summaries.

Supporting UI modules:

- `ui/actions.rs` maps action names and keyboard shortcuts to `AppState` calls.
- `ui/accessibility.rs` maps action names to focus/accessibility labels.
- `ui/labels.rs` formats device, MIDI, tuning, and note labels.
- `ui/text.rs` and `ui/theme.rs` centralize text fitting and visual tokens.

The detailed Operad integration model lives in `docs/operad_integration.md`.
The UI testing workflow lives in `docs/ui_testing_workflow.md`.

## Action Flow

The usual UI flow is:

1. A button, keyboard shortcut, pointer hit target, or canvas gesture produces
   an action name or edit gesture.
2. `ui/actions.rs` or the native host dispatches that action.
3. `AppState` updates state, status, persistence, undo history, or shared audio
   and MIDI handles.
4. The next frame rebuilds an Operad document from `AppState`.

Prefer adding a named action and an `AppState` method for ordinary controls.
Use custom surface hit testing only for dense editor regions such as the piano
roll and arrangement.

The detailed guide for adding controls and action names lives in
`docs/add_ui_control.md`.

## Testing

Most behavior tests live next to the code they protect:

- `src/ui/native/tests.rs` covers layout, hit testing, actions, shortcuts,
  cursor behavior, visual text overlap, devices, assets, piano-roll editing, and
  screenshot smoke checks.
- Unit tests in `src/audio.rs`, `src/midi.rs`, `src/project.rs`, `src/scala.rs`,
  `src/scale.rs`, `src/settings.rs`, and `src/synth.rs` cover lower-level
  behavior.
- Integration tests under `tests/` cover docs, startup probes, release metadata,
  and desktop metadata.

For UI changes, a passing test suite is not enough. Render a screenshot and
inspect the pixels when the change affects layout, text, hit targets, or visual
state.

The detailed UI testing workflow, including focused tests, screenshot checks,
and manual interaction expectations, lives in `docs/ui_testing_workflow.md`.
The web parity audit for the wasm/Pages build lives in
`docs/web_parity_audit.md`.

Release gating and handoff expectations live in `docs/release_workflow.md`.

Project-command mutation, dirty-state, undo/redo, autosave, and serialization
expectations live in `docs/add_project_command.md`.
