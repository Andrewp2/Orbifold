# Orbifold Manual QA Checklist

Use this checklist after layout, input, device, project, or rendering changes.
Automated tests catch regressions, but they do not prove the app is visually or
musically usable.

## Preflight

- Run `cargo fmt --check`.
- Run `cargo test`.
- Run `cargo clippy --all-targets -- -D warnings`.
- Start from a clean temporary project unless intentionally testing recovery.
- Note whether audio hardware, MIDI hardware, and a Lumatone are connected.

## Visual Smoke

- Run `cargo run -- --screenshot-size=1200x760`.
- Inspect `screenshots/latest.png`.
- Confirm the UI fills the whole image, not only the top-left corner.
- Confirm no obvious text overlaps in the top bar, left browser, track panel,
  right panel, arrangement, piano roll, or status bar.
- At the minimum layout, click asset rows, scale rows, compact steppers, and
  top-bar buttons to confirm they are not cramped or hard to target.
- Trigger a long status message such as opening a deeply nested project path and
  confirm the footer truncates it inside the status bar.
- Press Tab several times and confirm visible controls get a focus outline while
  arrangement/piano-roll canvas hit areas do not steal focus. Confirm the status
  bar names the focused control with a useful action label, then press Enter and
  confirm it activates.
- Press `?` and confirm the status bar shows a compact shortcut reference without
  creating unsaved changes.
- Spot-check `docs/keyboard_shortcuts.md` against any keyboard shortcuts changed
  during the pass.
- Confirm A4 is disabled when there is no audio output.
- Run `cargo run -- --screenshot-size=3840x2160`.
- Confirm the 4K screenshot is readable and not tiny.
- Confirm the right panel and piano-roll pitch labels remain legible.

## Startup

- Launch with normal `cargo run`.
- Confirm the app opens without audio hardware.
- Confirm the app opens without MIDI hardware.
- Confirm the status area reports missing audio or MIDI instead of failing
  silently.
- Confirm the native window title shows the current project name and an unsaved
  marker after edits.
- Run `cargo run -- --screenshot-size=1200x760` on a machine or environment
  without audio/MIDI hardware and confirm it writes a PNG without probing
  hardware.
- Confirm screenshot mode does not overwrite saved audio or MIDI device
  preferences.

## Project Workflow

- Add a note in the piano grid.
- Confirm the project becomes dirty.
- Use Save and confirm the dirty marker clears.
- Use Save As, choose a new project path, and confirm subsequent saves target
  the new file while the original remains on disk.
- Start a clean new project and confirm Open Recent loads the last saved or
  opened project.
- Save two projects, return to a clean new project, and confirm the recent rows
  can open the older project directly and show a compact modified-age label.
- Click Forget for the latest recent project and for an older recent row; confirm
  each disappears from Orbifold's recent list without deleting the project file.
- Make unsaved changes and confirm Open Recent is disabled or refuses to replace
  the dirty project.
- Delete or move the most recent project file and confirm Open Recent removes the
  stale entry, then opens the next available recent project if one exists.
- Confirm a missing recent row is marked as missing, cannot be opened, and can
  still be forgotten.
- Change the project, click New once, and confirm a discard prompt appears.
- Click New again and confirm the project resets.
- Repeat the discard prompt flow for Open.
- Try opening a missing or malformed project file and confirm the status names the
  failed file while the current project remains unchanged.
- Confirm Escape cancels a pending discard prompt.
- Save, quit, reopen the saved file, and confirm the note is visible/editable.

## Autosave And Recovery

- Edit a clean project and confirm an autosave file is created.
- Save the project and confirm the autosave file is removed.
- Seed an autosave, launch, and confirm the project browser shows Recover.
- Click Recover and confirm the project loads as unsaved.
- Seed an autosave, click Dismiss, and confirm the autosave file is removed while
  the current clean project remains unchanged.
- Seed a malformed autosave and confirm Recover reports the autosave path without
  replacing the current project.
- With both an autosave and a recent project available, confirm Recover and Open
  Recent are both visible when clean and the recovery row refuses dirty work.
- Try Recover with unsaved current edits and confirm it refuses to overwrite
  them.

## Transport And Editing

- Start with an empty clip and confirm the arrangement and piano roll both show
  truthful empty states.
- Play, pause, stop, and seek in both the arrangement and piano-roll rulers.
- After a clip has notes, click the arrangement clip and confirm the status
  reports the current clip note count.
- Toggle Replace/Overdub and confirm the status text changes.
- Toggle metronome and record quantize from the visible controls.
- Add, duplicate, delete, nudge, resize, pitch-shift, velocity-shift, quantize,
  and clear clip notes.
- Select a note and confirm the clip panel/status show velocity plus tuning
  context.
- Confirm undo and redo enable/disable correctly after edits.
- Confirm Escape clears the selected note when no discard prompt is active.
- Confirm keyboard shortcuts still work: Space, Home, R, M, Shift+Q, Q, G, P, N, D,
  arrows, Shift+arrows, Delete/Backspace, Ctrl/Cmd+C, Ctrl/Cmd+V, Ctrl/Cmd+S,
  Ctrl/Cmd+N, Ctrl/Cmd+O, Ctrl/Cmd+Z, Ctrl/Cmd+Y, Ctrl/Cmd+plus/minus/0.

## Devices

- With no audio output, confirm A4 reports a visible unavailable status.
- With audio output, test A4 and confirm it produces sound.
- Cycle audio devices, connect, disconnect/remove the device externally, refresh,
  and confirm the UI reports the disconnected state.
- With no MIDI input, confirm Refresh stays available and Connect is disabled.
- With MIDI input, connect it and confirm note-on/note-off updates the last MIDI
  label with a note name, MIDI number, channel, and velocity.
- Disconnect/remove the MIDI input externally, refresh, and confirm held notes
  and sustain state are cleared.
- Use All Off and confirm active notes stop.

## Tuning And Lumatone

- Refresh scales and load a bundled scale.
- Confirm the scale panel's active-scale card is labeled as the current scale,
  distinct from the selectable library rows.
- Confirm the selected scale action reads Loaded and is disabled when the
  selected library row is already the active scale.
- Remove or move a user-added scale file, select it, and confirm the row is
  marked missing; Load should remove the stale scale from the library without
  dirtying the project.
- Change the root control and confirm the right panel/status show a note name
  plus MIDI number.
- Load a malformed Scala file and confirm a visible parse error.
- Load a Lumatone map and confirm the key count/range summary is visible.
- Remove or move the active custom Lumatone map file, refresh key maps, and
  confirm the status and visible key-map label name the missing file instead of
  adding it as a preset.
- Load a malformed Lumatone map and confirm a visible parse error.
- Open a project that references a missing Lumatone map and confirm the project
  still loads with a warning.

## Assets

- Refresh assets.
- Switch asset tabs.
- Import a supported sample and confirm it appears selected.
- Import a duplicate sample and confirm it gets a unique name.
- Delete a selected asset file, refresh assets, and confirm the selection clears
  with a visible missing-asset status.
- Delete a listed asset file before refreshing and confirm its row is marked
  missing until Refresh removes it.
- Try importing an unsupported file and confirm a visible error with no copied
  file.

## Settings

- Change UI zoom and click Save Settings.
- Restart and confirm the zoom is restored.
- Change root/base frequency or synth settings and click Save Settings.
- Restart and confirm settings are restored.
- Seed malformed settings and confirm startup falls back to defaults while naming
  the settings file in the visible status.
- Confirm screenshot mode does not change saved device preferences.

## Final Checks

- Run one final `cargo test`.
- Run one final `cargo run -- --screenshot-size=1200x760`.
- Inspect the final screenshot.
- Confirm no unexpected project, autosave, or temp settings files were left in
  the repo.
