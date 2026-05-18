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
- Confirm compact top-bar transport controls do not overlap, the tempo value is
  editable, and the tempo buttons are visibly labeled `BPM -` and `BPM +`; the
  compact all-notes-off action is labeled `Panic`, and the quantize-grid control
  is labeled with `Q` plus the active grid value.
- At wider sizes, confirm the loop-length buttons are labeled `Loop -` and
  `Loop +` instead of anonymous plus/minus controls.
- At wide desktop sizes, confirm the settings action expands to `Save Settings`
  instead of staying abbreviated.
- At the minimum layout, click asset rows, scale rows, compact steppers, and
  top-bar buttons to confirm they are not cramped or hard to target.
- With enough assets or scales to overflow the visible rows, confirm the left
  browser shows a narrow scroll indicator, exposes up/down list controls, and
  keeps the selected row visible while those controls are used.
- Drag the vertical splitters between the asset browser, clip panel,
  arrangement, and control panel. Confirm the cursor changes to horizontal
  resize before dragging, the grab handles are visible, and each panel resizes
  without text overlap.
- Show both Assets and Scales in the left browser, then drag the divider between
  them. Confirm the cursor changes to vertical resize and extra height exposes
  more browser rows instead of leaving dead space.
- Drag the splitter above the piano roll. Confirm the cursor changes to vertical
  resize, its grab handle is visible, the piano roll can take more of the
  window, and the arrangement remains usable.
- Click the Layout reset button in the right panel and confirm the workspace
  returns to its default proportions.
- Open the right-panel Settings mode. Confirm UI zoom controls, Assets/Scales/Clip
  visibility toggles, Layout reset, Save Settings, and Open Setup/Open Devices
  are visible and do not overlap at minimum size.
- Trigger a long status message such as opening a deeply nested project path and
  confirm the footer truncates it inside the status bar.
- Trigger a startup or screenshot path with multiple accumulated status messages
  and confirm the footer leads with the latest action plus an earlier-message
  count.
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
- Run `cargo run -- --startup-probe` and confirm routine ALSA/JACK backend
  diagnostics do not print directly to the terminal.
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
- Seed an autosave, launch, and confirm the left session strip shows Recover
  above the asset browser rather than inside the asset browser controls.
- Click Recover and confirm the project loads as unsaved.
- Seed an autosave, click Dismiss, and confirm the autosave file is removed while
  the current clean project remains unchanged.
- Seed a malformed autosave and confirm Recover reports the autosave path without
  replacing the current project.
- With both an autosave and a recent project available, confirm Recover and Open
  Recent are both visible in the left session strip when clean and Recover
  refuses dirty work.
- Try Recover with unsaved current edits and confirm it refuses to overwrite
  them.

## Transport And Editing

- Start with an empty clip and confirm the arrangement says `Empty clip` while
  the piano-roll grid stays clear of large placeholder text.
- Play, pause, stop, and seek in both the arrangement and piano-roll rulers.
- Zoom or scroll the piano roll horizontally and vertically. Confirm the
  arrangement ruler, playhead, visible beat labels, pitch labels, and seek
  position follow the same visible windows.
- Use `Ctrl`/`Cmd+Wheel`, `Shift+Wheel`, and `Alt+Wheel` over the piano roll.
  Confirm time zoom, time pan, and pitch-row zoom all update visibly.
- Click the piano-roll `Fit` button after heavy zooming or scrolling. Confirm
  the grid recenters around the clip's notes, or around the root/default view
  when the clip is empty.
- Confirm the piano-roll option strip labels the grid action as `Q` plus the
  active grid value next to the Snap toggle.
- Confirm the loop-start and loop-end boundary lines are visible when those
  boundaries are inside the arrangement/piano shared time window.
- Drag the loop-end boundary in the arrangement ruler and piano-roll ruler.
  Confirm the loop shortens, extends, and can grow past the old right edge.
- After a clip has notes, click the arrangement clip and confirm the status
  reports the current clip note count.
- Toggle Replace/Overdub and confirm the status text changes.
- Toggle metronome and record quantize from the visible controls.
- Add, duplicate, delete, nudge, resize, pitch-shift, velocity-shift, quantize,
  and clear clip notes.
- Use the visible piano-roll Snap toggle. With Snap on, drag note edges and
  confirm start/end land on visible grid lines; turn Snap off and confirm edge
  resizing can land between grid lines.
- Select a note and confirm the clip panel/status show velocity plus tuning
  context.
- Confirm undo and redo enable/disable correctly after edits.
- Confirm Escape clears the selected note when no discard prompt is active.
- Confirm keyboard shortcuts still work: Space, Home, R, M, Shift+Q, Q, G, P, N, D,
  arrows, Shift+arrows, Delete/Backspace, Ctrl/Cmd+C, Ctrl/Cmd+V, Ctrl/Cmd+S,
  Ctrl/Cmd+N, Ctrl/Cmd+O, Ctrl/Cmd+Z, Ctrl/Cmd+Y, Ctrl/Cmd+plus/minus/0.

## Devices

- With no audio output, confirm A4 reports a visible unavailable status.
- With audio or MIDI setup missing, confirm the right-panel `Devices` action is
  relabeled `Setup` and opening it names the missing setup state in the status
  bar and shows a `SETUP REQUIRED` summary inside the Devices panel.
- Launch or screenshot with audio/MIDI probing skipped or unavailable and
  confirm the right panel opens directly in the setup/devices view.
- With audio output, test A4 and confirm it produces sound.
- On web, confirm the Devices panel shows a Web Audio diagnostic naming whether
  sink selection is available, how many outputs were scanned, and whether routing
  happened after connecting audio.
- Cycle audio devices, connect, disconnect/remove the device externally, refresh,
  and confirm the UI reports the disconnected state.
- Open the Devices panel from the right panel, select an audio output row
  directly, and confirm the selected output/status changes.
- Confirm the audio diagnostic row reports whether the output is live,
  disconnected, selected but not connected, or missing after refresh.
- With no MIDI input, confirm Refresh stays available and Connect is disabled.
- With MIDI input, connect it and confirm note-on/note-off updates the last MIDI
  label with a note name, MIDI number, channel, and velocity.
- On web, confirm the Devices panel shows a Web MIDI diagnostic naming browser
  permission/readiness, scanned input count, and connection state.
- Move pitch bend or a non-sustain controller and confirm the last MIDI label
  says the input is ignored rather than implying synth modulation happened.
- Open the Devices panel from the right panel, select a MIDI input row directly,
  and confirm the selected input/status changes.
- Confirm the MIDI diagnostic row reports whether the input is live,
  disconnected, selected but not connected, or missing after refresh.
- At minimum width, confirm the compact device rows are distinguishable as
  `MIDI` and `Audio` rather than two identical Refresh/Connect rows.
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
- Preview a WAV sample and confirm `Stop` cancels playback.
- Try previewing without an audio output and confirm the status names the missing
  output instead of doing nothing.
- Import a duplicate sample and confirm it gets a unique name.
- Delete a selected asset file, refresh assets, and confirm the selection clears
  with a visible missing-asset status.
- Delete a listed asset file before refreshing and confirm its row is marked
  missing until Refresh removes it.
- Try importing an unsupported file and confirm a visible error with no copied
  file.

## Settings

- Open the right-panel Settings mode, change UI zoom, and click Save Settings.
- Restart and confirm the zoom is restored.
- Resize workspace panels, including the Assets/Scales divider, click the
  right-panel Layout reset button, restart, and confirm the default panel
  proportions are restored.
- Hide/show the left-panel Assets and Scales browser tabs, restart, and confirm
  those visibility choices are restored without marking the project dirty.
- Click the right-panel Layout reset button and confirm Assets is shown, Scales
  is hidden, and splitter sizes return to defaults.
- Change root/base frequency or synth settings and click the top-bar preferences
  button.
- Restart and confirm settings are restored.
- Seed malformed settings and confirm startup falls back to defaults while naming
  the settings file in the visible status.
- Confirm screenshot mode does not change saved device preferences.

## Final Checks

- Run one final `cargo test`.
- Run one final `cargo run -- --screenshot-size=1200x760`.
- Inspect the final screenshot.
- Review `docs/web_parity_audit.md` before making any browser parity claim.
- Build the web app with `./scripts/build-web.sh dist`.
- Serve `dist/` locally and open it in a WebGPU-capable browser.
- Confirm the live wasm UI replaces the static fallback shell. If only the
  fallback shell appears, record the browser/GPU limitation and do not count it
  as live web parity evidence.
- Open the deployed GitHub Pages URL after the Pages workflow completes and
  confirm it serves the same live wasm UI and favicon/assets as the local
  `dist/` smoke.
- Run `./scripts/check-web-live.mjs` against the deployed Pages URL and confirm
  the live artifact has the expected wasm loader, wasm binary, icons, and
  relative asset references.
- Run `./scripts/check-web-layout.mjs` against the deployed Pages URL and
  confirm compact, desktop, high-DPI, and 4K browser layouts have full canvas
  coverage, no page overflow, usable editor geometry, and no reported text
  overlap/invalid text layout.
- Run `./scripts/check-web-smoke.mjs` against the deployed Pages URL or confirm
  the Pages workflow's deployed smoke step passed.
- Run `./scripts/capture-web-visuals.mjs` against the deployed Pages URL and
  inspect the compact, desktop, high-DPI, and 4K PNG or SVG artifacts under
  `screenshots/web/`, or inspect the deployed visual artifact uploaded by the
  Pages workflow.
- Run `./scripts/check-web-manual-devices.mjs` against the deployed Pages URL
  with a real audio output and Web MIDI device attached. Keep the generated
  `reports/web-manual-devices-*.json` artifact with the release evidence; it
  includes the deployed artifact fingerprint observed during the manual run.
- Run `./scripts/check-web-manual-report.mjs reports/` against the generated
  report and confirm it accepts the real browser/audio/MIDI evidence and
  artifact fingerprint.
- Run `./scripts/check-web-parity-gate.mjs https://<user>.github.io/<repo>/ --report reports/`
  and keep the generated `reports/web-parity-gate-*.json` with the release
  evidence. The gate rejects manual reports whose fingerprint no longer matches
  the live Pages artifact.
- Run `./scripts/check-web-parity-complete.mjs reports/` and confirm it accepts
  the saved manual-device report plus final parity-gate report.
- Confirm browser `Open` refuses dirty replacement on the first click/shortcut
  and opens the picker only after the second confirmed action.
- Confirm browser `Save`/`Save As` downloads `.orbifold` project text and the
  browser tab title reflects saved/dirty project state.
- Confirm browser keyboard shortcuts route through browser behavior:
  `Ctrl`/`Cmd+S`, `Ctrl`/`Cmd+O`, `?`, arrows, note edit shortcuts, and
  `Ctrl`/`Cmd` UI zoom.
- Confirm browser text edits work for BPM, root, base frequency, scale search,
  and asset search.
- Confirm browser piano-roll interactions match native for double-click note
  creation, note move, edge resize, velocity drag, wheel zoom/pan, viewport
  thumb drags, keyboard pitch-ruler drag, and loop-end drag.
- Confirm browser workspace splitters resize the left browser, clip panel,
  right panel, piano roll, and Assets/Scales split.
- Import a WAV sample in the browser, use it as the sample instrument, reload,
  and confirm it remains available within browser storage quota.
- Refresh/connect Web MIDI in a browser that supports Web MIDI and confirm note
  input reaches the same status/playback path as native.
- Connect a real browser audio output, play A4 or a short clip, and confirm
  audible Web Audio output plus visible failure reporting when output is
  unavailable.
- Confirm no unexpected project, autosave, or temp settings files were left in
  the repo.
