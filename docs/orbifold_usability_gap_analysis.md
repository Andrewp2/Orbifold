# Orbifold Usability Gap Analysis

Last reviewed: 2026-05-13

This document is a working inventory of what is still missing before Orbifold feels
usable as a music-making application. It is intentionally broad. Some items are
hard bugs, some are incomplete workflows, some are visual or interaction quality
problems, and some are product decisions that need to be made before the app can
be coherent.

The short version: Orbifold has a good technical seed. It has a Rust audio/MIDI
core, project serialization, Scala and Lumatone parsing, a synth, a clip model,
an Operad-rendered surface, click dispatch, keyboard shortcuts, screenshots, and
layout overlap tests. It is not yet a usable DAW or instrument workstation. The
main missing pieces are reliable startup, a coherent first-run workflow, a real
multi-track editing model, a non-placeholder UI, strong interaction ergonomics,
asset-to-sound plumbing, recoverable persistence, visual QA, and release
packaging.

## Progress Notes

Progress since this gap list was first written:

- Orbifold can now build an app state without a live audio stream instead of
  panicking immediately when CPAL cannot produce an output stream.
- Screenshot mode skips audio and MIDI hardware probing entirely, so visual QA
  can run without CPAL/JACK/ALSA or MIDI startup noise, and it avoids persisting
  hardware-derived settings from that no-probe startup.
- Startup audio warnings are preserved alongside later MIDI warnings instead of
  being overwritten during app initialization.
- Sound-producing controls such as A4 test are disabled when no audio stream is
  connected, while `All Off` remains available as a panic/reset action because
  it also clears live MIDI held-note and sustain state.
- The A4 test path reports a visible status when audio is unavailable.
- Settings capture no longer persists an empty audio output name as a meaningful
  saved device.
- The README now describes the Operad UI and current prototype limits instead of
  the old egui/menu workflow.
- The fake recent-project list has been replaced by a current-project card.
- Unsupported track Add/Mute/Solo controls are visually disabled until there is a
  real multi-track engine behind them.
- The fake multi-track arrangement preview has been replaced by the current
  single-clip surface: one `Current Clip` lane, an empty recorded-clip state, and
  no preview notes in the piano roll before recording.
- A basic project dirty state now exists. Recording, clip edits, tuning/keymap
  changes, transport loop changes, synth changes, undo, and redo mark the project
  dirty; save/load mark it clean; and the project panel reports `No file`,
  `Unsaved`, `Unsaved changes`, or `Saved`.
- The Operad host no longer applies DPI scaling twice. The latest screenshot
  fits the full workspace at 2x scale instead of showing a cropped, oversized
  subset of the UI.
- The top bar now labels the overdub toggle honestly, reports loop length in
  beats instead of falsely calling the value bars, and removes a decorative meter
  that collided with controls.
- The default layout now gives more horizontal space to the arrangement and
  piano roll by narrowing the project, clip, and control columns.
- Disabled placeholder track controls (`+ Track`, mute, and solo) have been
  removed from the visible single-clip UI.
- Project save now writes through a temporary file in the project directory and
  creates a `.bak` copy before overwriting an existing project file.
- Piano-roll notes can now be dragged to change start beat and pitch, with the
  drag grouped into a single undo history entry.
- Piano-roll note right edges can now be dragged to resize duration, also grouped
  into a single undo history entry.
- Piano-roll note left edges can now be dragged to change the note start while
  preserving the note end, also grouped into a single undo history entry.
- Piano-roll velocity bars can now be clicked or dragged in the velocity lane,
  with each gesture grouped into a single undo history entry.
- The arrangement and piano-roll editors now render the active quantize
  subdivisions, so a 1/16 or 1/32 grid is visible instead of implied only by the
  toolbar label.
- Transport pause now preserves the playhead position. `Stop` stops playback and
  resets to the loop start, while `Start` seeks to the loop start without
  stopping playback.
- The arrangement ruler can now be clicked or dragged to move the playhead.
- Orbifold now ships a small bundled `scales/` library, and the scale panel shows
  an explicit current-scale card plus readable Scala descriptions instead of raw
  filenames.
- When the selected library scale is already loaded, the scale action reads
  `Loaded` and is disabled instead of presenting a no-op `Load`.
- Missing user scale files are marked in the scale list while preserving the
  existing Load action that prunes stale entries without dirtying the project.
- Bundled scales are now explicitly non-removable: the `Remove` button disables
  when a bundled scale is selected, and the command layer refuses to remove one
  if invoked directly.
- Long scale and asset lists now window around the selected row so the currently
  selected item stays visible even before full scroll-container support exists.
- The mouse wheel now changes the selected scale or asset when the pointer is
  over those left-browser panels, giving long libraries a basic navigation path
  until Operad has real scroll containers.
- Focusable visible controls now have minimum-target regression coverage at the
  compact layout, and asset-browser rows were increased to match that 24 px
  floor.
- Loading the already-active Scala file is now a no-op that reports the scale is
  already loaded instead of dirtying the project again.
- Synth parameter buttons now skip dirty/save work when the edited value is
  already clamped at its limit, such as pressing gain up while gain is already
  100%.
- Bounded root, base-frequency, BPM, and loop-length +/- controls now do the
  same, reporting `unchanged` at clamp limits instead of creating unsaved project
  state.
- Shortening a clip note that is already at the minimum duration is now a no-op:
  it reports the length limit without dirtying the project or adding undo
  history.
- Clip-note toolbar actions now treat stale note selections as true no-ops:
  delete, duplicate, move, and resize clear the missing selection without
  dirtying the project or adding undo history.
- Copy, quantize, velocity, and pitch edits now share that stale-selection
  behavior, and the rendered clip-note toolbar disables note-specific controls
  when the selected note ID no longer exists in the clip.
- The project panel now has an explicit `Save As` action, `Ctrl`/`Cmd+Shift+S`
  routes to the same workflow, and Save As has a regression test proving it
  writes a new project file without overwriting the original saved file.
- Re-loading the already-active Scala file through file-open style paths, or
  the already-active Lumatone key map through edit paths, now reports that it is
  already loaded instead of creating unsaved project state.
- No-op quantize and minimum-length note resize commands now detect the no-op
  before touching undo history, so they no longer clear available redo edits.
- Saved and opened projects now update a persisted recent-project list, and the
  project card shows the most recent project when no project file is currently
  open.
- The project panel now exposes `Open Recent` when a recent project exists and
  the current project is clean; it refuses to replace dirty work until the user
  saves or discards changes.
- `Open Recent` now prunes missing recent projects, persists the cleaned list,
  and opens the next available project in the same action, so moved/deleted files
  do not leave a dead one-click action in the project panel.
- The project panel now shows clickable recent-project rows when there is room,
  so users can open an older recent project directly instead of only loading the
  newest saved/opened file.
- The project panel now has a `Forget` action for the latest recent project,
  removing it from Orbifold's recent list without deleting the project file.
- The native window title now includes the project name and an unsaved marker,
  so saved/dirty state is visible outside the left project panel.
- Failed project open/parse statuses now name the target file, and failed opens
  leave the current project and recent-project list untouched.
- Failed autosave write/open/parse/load statuses now name the autosave file, and
  a bad autosave recovery attempt leaves the current project untouched.
- Autosaves can now be dismissed from the clean project panel, removing stale or
  unwanted recovery files without forcing a recover attempt or leaving the
  project panel stuck in recovery mode.
- Startup now sizes the window from the current monitor instead of always opening
  at the old fixed `1400x760` prototype size, so large displays get a much larger
  initial workspace.
- The asset browser now shows a selected-kind count and a real empty state when
  there are no imported files for the active tab.
- The asset browser empty state now uses a single clear status line, avoiding
  the duplicate adjacent text rendering seen in visual QA.
- Missing audio asset files are marked in the asset browser until Refresh
  removes stale entries and clears stale selection.
- The status bar now fits long path/device/status messages to the available
  footer width instead of letting them run beyond the bar.
- Visible Operad buttons now participate in Tab focus traversal and focused
  buttons activate with Enter. Invisible canvas hit targets stay pointer-only so
  keyboard focus does not disappear into the arrangement, piano-roll grid, or
  note drag handles.
- Focusable controls now use unique surface node names, so a focused button
  keeps the same visual target after redraw even when another panel exposes the
  same underlying command.
- Tab focus now reports the focused control in the status bar with an Enter
  activation hint, so keyboard users get feedback without needing a separate
  shortcut reference panel.
- Compact controls now expose descriptive accessibility labels for focus/status
  feedback, so `+`, `-`, `<`, `>`, abbreviated asset tabs, and synth steppers
  announce useful actions instead of raw glyphs.
- Focusable controls now have regression coverage against empty, glyph-only, or
  abbreviation-only accessibility labels.
- Pressing `?` now shows a compact keyboard shortcut reference in the status bar
  without dirtying the project.
- `docs/keyboard_shortcuts.md` now records the current keyboard path by category
  so the shortcut set has a stable reference outside the UI.
- The documented keyboard shortcut list now has regression coverage for duplicate
  chord assignments and for keeping the markdown reference in sync.
- Screenshot mode now accepts `--screenshot-size=WIDTHxHEIGHT`, which lets visual
  QA capture the exact minimum supported layout instead of only the current
  monitor-sized window.
- The minimum-layout MIDI and audio device controls now collapse to shorter
  `Refresh` and `Connect` labels and fit inside the right panel.
- The minimum-layout capture/keymap controls now use responsive widths too, so
  `Capture`, `Stop`, `Clear`, and `Maps` keep a right-panel margin.
- Clip-note add, select, duplicate, move, resize, pitch, drag, and velocity edits
  now report degree, octave, beat, length, and velocity in the status bar instead
  of vague edit messages.
- Clip-note status messages now also include frequency and cents from the active
  root, and the clip panel's selected-note summary includes velocity.
- Empty clip `Clear` and `Quantize` are now true no-ops that do not dirty the
  project or push undo history; the empty piano-roll toolbar disables Quantize
  alongside Clear.
- Undo/redo history now carries the selected clip note along with the project
  snapshot, so redo after adding a note and undo after deleting a note keep the
  toolbar selection state coherent.
- The dirty indicator now compares the current project file snapshot to the last
  clean baseline, so undoing all the way back to the initial or saved state clears
  `Unsaved changes` instead of leaving a stale dirty flag.
- Recording now defers undo-history creation until recording actually changes
  saved clip content. Starting/stopping an empty recording pass no longer enables
  a useless Undo action, while recorded notes still undo back to the pre-recording
  snapshot.
- Undo/redo status messages now describe the restored selected note or clip note
  count instead of only reporting a generic history action.
- The clip side panel now reports real single-clip state: note count, loop
  length, quantize grid, and selected-note details when a note is selected,
  instead of showing only a decorative placeholder strip.
- UI zoom is now a persisted setting, and `Ctrl`/`Cmd` plus `+`, `-`, or `0`
  adjusts or resets the density so large monitors are not stuck with a single
  hard-coded scale.
- UI zoom also has visible right-panel controls, so users do not have to know
  the keyboard shortcut before making the interface readable.
- The project panel now has a `New` action. Dirty projects use a two-step
  `Discard?` confirmation before unsaved changes are thrown away.
- `Open` now uses the same dirty-project confirmation pattern before another
  project can replace unsaved work.
- Closing the window with unsaved changes now warns first and requires a second
  close request before quitting.
- `Esc` now cancels pending dirty-project discard confirmations.
- The project card now shows either the saved project directory or a compact
  unsaved location hint, making project location state visible without clipping
  in the default layout.
- The MIDI device label now distinguishes missing inputs from selected-but-
  disconnected inputs.
- Mapping capture controls now disable `Stop` and `Clear` unless capture state
  makes those actions meaningful.
- MIDI and audio connect buttons now say `Reconnect` when the corresponding
  device stream is already connected.
- Screenshot mode now honors the requested `--screenshot-size` dimensions for
  offscreen output even when the desktop window is capped smaller. Explicit
  screenshot sizes ignore host DPI scaling but still apply Orbifold's
  large-screen density rules, making minimum and 4K layout QA trustworthy again.
- MIDI and audio status labels now explicitly report connected, disconnected,
  or missing-device states instead of relying on ambiguous device names alone.
- Dirty project edits now write a basic `orbifold_autosave.orbifold` recovery
  file, and saving or returning to a clean project state clears it.
- When an autosave exists, the project panel now exposes a `Recover` action that
  loads it as an unsaved project without replacing another dirty project.
- `Recover` and `Open Recent` can now coexist in the project panel, so a stale or
  unwanted autosave does not hide the one-click path back to the last saved
  project.
- The recovery row now also exposes `Dismiss` in clean projects, so users can
  explicitly clear stale or malformed autosaves once they decide not to recover
  them.
- The metronome project setting now has a visible right-panel toggle and status,
  so recording with a click no longer depends on hidden project state.
- The `M` key now toggles the metronome for fast recording setup.
- MIDI/audio refresh actions now report whether they found zero, one, or many
  devices instead of silently changing the picker state.
- Scale refresh, scale removal, and asset refresh actions now report concrete
  result statuses instead of mutating library state silently.
- Loading a missing user-added scale now removes that stale scale entry and
  persists the cleaned library without dirtying the project.
- Asset refresh now preserves the selected asset by path across list reorderings
  and clears the selection with a visible status when the selected file is gone.
- Lumatone map refresh now reports how many key maps were found and which map is
  selected, while preserving load errors instead of hiding them behind a generic
  refresh result.
- Lumatone key-map load and missing-project-keymap errors now include the
  relevant `.ltn` path, and key-map refresh no longer adds a missing current map
  back into the preset list.
- If the loaded key-map file is removed while its map remains usable in memory,
  the visible key-map label marks it as missing instead of presenting it as a
  normal loaded file.
- The right-panel key-map label now names the selected map and key count instead
  of showing low-level `key0` debug details.
- Loading a Lumatone map now reports key count, MIDI-note range, and colored-key
  count in the status line.
- Quantize-on-record is no longer hidden project state: the clip panel now has a
  `Rec quantize` toggle and `Shift+Q` shortcut.
- Recording mode is now explicit: the transport reports `Replace` or `Overdub`,
  and recording status includes the mode plus quantize/free-timing behavior.
- Stopping a recording now reports how many notes were captured, including when
  the user stops via the transport `Stop` button instead of toggling `Record`.
- MIDI note-ons held before recording starts are now tracked and seeded into the
  new recording, so releasing an already-held key still produces a recorded note.
- Sustain pedal CC64 now affects recording duration: note-offs while sustain is
  down remain pending until sustain is released or recording stops.
- Live synth note-off now follows sustain pedal state too, so playback while
  recording matches the recorded sustain behavior.
- `All Off` and MIDI reconnect now clear live held-note and sustain state, so
  panic/reconnect paths do not preserve stale sustained notes.
- Connected audio outputs now carry stream details, and connection/status labels
  can report sample rate, channel count, and sample format.
- Effective UI scale is now capped by the available window size, so minimum-size
  windows preserve the tested `1200x760` layout instead of letting DPI or saved
  zoom settings squeeze the controls into overlapping text.
- Refreshing audio outputs now drops stale connected-stream state when the
  previously connected output disappears from the device list, then reports the
  missing device and the refreshed output count.
- Refreshing MIDI inputs now does the same for vanished connected inputs,
  clearing held-note and sustain state so reconnecting does not preserve stale
  live MIDI state.
- MIDI and audio status labels now prefer the actual connected device name over
  the currently selected row, avoiding false `connected` labels after the user
  cycles to another device but has not reconnected yet.
- MIDI and audio connect buttons now only say `Reconnect` when the selected
  device is the one currently connected; selecting a different device presents a
  normal `Connect` action.
- Device selection status now says when the selected MIDI/audio device is
  already connected versus when the user has selected a different device and
  needs to click `Connect` to switch.
- MIDI/audio previous-next controls are disabled unless there is more than one
  device in the corresponding list, so single-device setups no longer expose
  inert navigation.
- `All Off` now remains available without an audio stream, so users can clear
  stale MIDI held-note/sustain state even when audio is disconnected.
- The `P` key now runs `All Off`, giving the panic/reset path a keyboard
  shortcut instead of requiring a pointer click.
- Settings saves now write through a same-directory temporary file and clean up
  the temp file on failure, so frequent preference persistence is less likely to
  leave a partially written settings file.
- Asset import now has isolated temporary-directory tests for supported sample
  copy/selection, duplicate-name uniquing, and unsupported-extension rejection.
- Opening a project now resets the active scale back to 12-TET when the file has
  no Scala path, clears any previous Lumatone map when the file has no keymap,
  and reports a visible warning while still loading the project if a referenced
  Lumatone map is missing.
- The core record/edit/save/reopen path now has an end-to-end regression test
  using synthetic MIDI events, including note editing after recording and
  confirming the reopened note is still playable/editable.
- Lumatone map parsing now rejects malformed note values, missing Key/Chan
  pairs, invalid board headers, and out-of-range per-board key indexes, with
  tests for each case.
- Scala parsing now has a generated 4096-EDO regression test that exercises large
  scale counts, octave-period removal, and ratio generation beyond the bundled
  small fixtures.
- Project compatibility now has checked-in fixtures for both the current
  `orbifold_project=1` format and the legacy `microtonal_daw_project=1` marker,
  including defaults for fields that older project files did not store.
- Settings compatibility now has checked-in fixtures for current
  `orbifold_settings.txt` content and legacy `microtonal_daw_settings.txt`
  content, including defaulting behavior for settings older files omitted.
- Settings load/save failures now include the settings path, so fallback-to-
  defaults startup and failed `Save Settings` reports identify the file involved.
- The saved-project render path now has an end-to-end regression test that saves
  a clip, reopens it, builds the Operad surface, confirms the loaded clip state
  is visible, and checks text-overlap safety.
- No-device UI states now have layout/semantics tests: audio-unavailable surfaces
  show `Audio no output`, keep `All Off` and refresh available, disable unsafe
  sound tests/connects, and MIDI-unavailable surfaces show `MIDI no input` while
  keeping refresh available.
- Piano-roll pitch labels now use a tested density policy with larger minimum
  vertical spacing, so the keyboard strip is less cramped at the minimum layout
  while still showing more labels when there is room.
- Visible transport, scale/synth, and clip-edit actions now have command-level
  tests proving those controls update project/app state instead of only drawing
  labels.
- Command-level interaction coverage now also includes file save/new
  confirmations, undo/redo, library and asset selection, MIDI/audio device
  cycling, capture start/stop/clear, timeline seeking, and piano-grid note
  creation.
- Pointer-through interaction tests now build the real Operad document, click a
  visible transport button, verify a disabled button does not dispatch, and
  click a canvas hit target through Operad hit-testing.
- Pointer-through coverage now includes additional top-bar controls: record,
  loop/overdub mode, BPM up, quantize-grid cycle, and All Off are clicked
  through the rendered document and verified against project/app state.
- Pointer-through coverage now includes rendered note selection, proving an
  existing piano-roll note hit target selects the real clip note.
- Tiny piano-roll notes now keep their body selectable instead of being fully
  covered by resize handles; resize edge hit targets are only added once the
  note is wide enough to leave a usable center.
- Rendered note dragging now has a regression test that presses an existing
  piano-roll note through the real Operad document, verifies it starts a move
  drag, moves it, and checks the real clip note updates.
- Rendered velocity-lane dragging now has a regression test that presses a note
  velocity hit target, verifies velocity-drag mode, drags it, and checks the
  real clip note velocity updates.
- Rendered note resize-end dragging now has a regression test that presses a
  real resize handle, verifies resize mode, drags it, and checks clip duration.
- Rendered note resize-start dragging now has a regression test that presses the
  start handle, verifies resize-start mode, drags it, and checks clip start and
  duration update while preserving the note end.
- Rendered arrangement-ruler dragging now has a regression test that presses the
  real seek hit target, verifies timeline-drag mode, drags, releases, and checks
  transport position.
- The piano roll now has its own beat ruler above the note grid. Clicking or
  dragging that ruler moves the playhead, with pointer-through regression
  coverage separate from arrangement-ruler seeking.
- Clicking the rendered current clip in the arrangement now selects the current
  clip context, clears note selection, and reports the clip note count.
- Pointer-through interaction coverage now includes the rendered clip toolbar:
  add note, pitch up, velocity up, duplicate, and delete are clicked through
  Operad hit-testing and verified against real clip state.
- The compact clip toolbar now uses readable labels such as `Delete`,
  `Duplicate`, `Pitch-`, `Pitch+`, and `Quant` instead of `Del`, `Dup`, `P-`,
  `P+`, and `Q` at the minimum layout.
- The asset browser's compact tabs now use `Samp` and `Pres`, avoiding clipped
  `Sam...` and `Pres...` labels at the minimum layout.
- Pointer-through coverage now also includes the right-side control panel:
  scale root/base controls, metronome, UI zoom, waveform, and synth gain are
  clicked through the rendered document and verified against real state.
- Pointer-through coverage now includes the left browser's scale and asset
  selection buttons, proving rendered list entries and tabs update the real
  selection state.
- Pointer-through coverage now includes deterministic MIDI/audio device
  navigation buttons, proving rendered previous/next controls update selected
  devices without requiring hardware connection attempts.
- The visible `Recover` action now has a click-through regression test that
  loads an autosave-compatible project file, leaves it unsaved, and reports the
  recovery status.
- The top-bar `Save Settings` action now has a click-through regression test using
  an isolated settings path, proving the visible button writes real settings
  without touching the user's active settings file.
- Top-bar availability now has regression coverage: default no-audio startup
  keeps safe actions enabled, disables A4, and reflects undo/redo availability
  as project history changes.
- Visible scale refresh/load/remove, keymap refresh, and asset refresh actions
  now have command-level tests proving they update real library/app state without
  requiring OS file dialogs.
- Visible UI zoom and the remaining synth parameter buttons now have
  command-level tests covering real settings changes for zoom, attack, release,
  filter cutoff, delay mix, and drive.
- Scene text, label text, and button captions now pass through a width-aware
  fitting helper before rendering, so long runtime names and tight fixed-width
  controls truncate instead of painting into adjacent UI.
- Automatic large-monitor scaling now treats 4K-class outputs as 2x UI scale by
  default, while still preserving the minimum logical layout space on small
  windows.
- Zoom-out scaling now uses the actual sub-1.0 UI scale for logical sizing and
  pointer coordinate conversion, so zooming out expands the logical surface
  instead of rendering/input-mapping as if scale were still 1.0.
- Piano-roll and arrangement subdivision grids now thin out automatically when
  the current quantize step would produce unreadably tight vertical lines.
- Piano-roll pitch row lines now thin out when row height is too small, reducing
  the dense horizontal texture at minimum window size.
- Large-clip surface layout now has a bounded-render regression test that builds
  a 512-note clip, computes the real Operad layout, and checks UI-node and
  paint-item counts stay within sane limits.
- The native Operad host now only requests continuous idle redraws while the
  transport is playing, so a stopped project does not keep repainting the full
  interface every event-loop cycle.
- The bottom status bar no longer repeats the MIDI/audio device summary on the
  right side, since the device panel already reports those states and the
  duplicated footer readout made startup look noisy.
- Automatic startup scans for scales and audio assets now populate the UI
  quietly on success, so startup diagnostics are not cluttered by routine
  `Found ...` or `No assets found` messages. Explicit refresh buttons still
  report their results.
- Automatic startup key-map selection now behaves the same way: default or saved
  Lumatone maps load quietly on success, while explicit key-map loads and
  refreshes still report visible results or errors.
- Keyboard shortcut dispatch now ignores `Alt`-modified combinations, so
  application shortcuts do not accidentally fire when the user is using OS/menu
  or international-keyboard modifier chords.
- The transport bar now shows a fixed `4/4` meter readout when the window is
  wide enough, making the current single-meter constraint visible without adding
  fake time-signature editing controls.
- Wide transport layouts now also show a bar/beat playhead readout such as
  `Bar 1.1`, so the current position is visible numerically as well as through
  the timeline playhead.
- Connected audio status now reports sample rate, channels, sample format, and
  buffer information. Fixed-size buffers include an estimated one-buffer
  latency, such as `256f 5.3ms`; default host buffers are labeled `default buf`.
- The synth output now has a real runtime mute toggle in the control panel. It
  mutes the audio engine without rewriting or saving the master gain, and the
  minimum-layout overlap tests cover the muted state.
- The synth engine now publishes a master output level meter and a held
  limiting indicator. The control panel renders a compact meter below the output
  row and switches the output label to `Output limit` while the limiter is hot.
- MIDI and audio device labels now include the selected item position, such as
  `Audio 2/4`, so the existing prev/next controls behave more like a visible
  picker instead of an anonymous cycle button.
- Startup audio fallback status now explicitly says when a saved output is
  unavailable and Orbifold is trying the system default, rather than only
  reporting the missing saved device.
- The last-MIDI monitor now names common controller policy directly: sustain
  pedal events show `sustain on/off`, and pitch-bend messages show centered
  bend values instead of fake note names.
- MIDI input now has a runtime channel filter exposed in the control panel. The
  last-event monitor still shows filtered-out events, but synth playback,
  held-note state, mapping capture, and recording ignore channels outside the
  selected filter. The selected filter is saved and restored with app settings.
- The transport return-to-start button now says `Start` instead of `Prev`, so
  the visible label matches its current behavior rather than implying previous
  clip or marker navigation.
- The transport record button now says `Record` instead of `Rec`, using the
  available top-bar space for clearer first-run workflow labeling, and switches
  to `Stop Rec` while recording so the toggle action is explicit.
- Key repeat is now allowed for arrow-key note edits while staying blocked for
  transport toggles and command shortcuts, so holding an arrow can keep nudging
  or transposing a selected note without repeatedly starting/stopping playback.
- `Shift+Up` and `Shift+Down` now adjust selected-note velocity, giving velocity
  edits a keyboard path alongside the visible toolbar buttons.
- `N` now adds a new note at the playhead, so add, duplicate, delete, nudge,
  resize, transpose, velocity, and quantize all have keyboard paths.
- `Esc` now clears the selected clip note when there is no pending discard
  confirmation, so selection state can be dismissed without changing project
  data.
- `Ctrl`/`Cmd+C` and `Ctrl`/`Cmd+V` now copy the selected clip note and paste it
  at the playhead while preserving pitch, duration, and velocity; paste is one
  undoable project edit.
- The `Home` key now performs the same return-to-start transport action as the
  visible `Start` button. That action seeks to the loop start without stopping
  playback, while `Stop` still stops playback, and this path is covered so it
  does not dirty a clean project when only the playhead position changes.
- `G` now toggles piano-roll snap off and back to the previous non-off grid
  value, so the user can temporarily free-place notes without cycling through
  every quantize option.
- `Q` now quantizes the selected clip note when one is selected, and falls back
  to whole-clip quantize when nothing is selected; the selected-note path is
  undoable and leaves other notes untouched.
- Selecting a piano-roll note now auditions it briefly, and dragging a note to a
  new pitch auditions the new pitch without replacing the edit/selection status.
- The clip panel's selected-note summary now includes compact frequency and
  cents context, such as `440.0Hz +0c`, alongside degree, octave, beat, length,
  and velocity.
- The scale root display now shows a musician-readable pitch name alongside the
  MIDI number, such as `Root A4 (69)`, and root adjustment status messages use
  the same label.
- Piano-roll pitch labels now use note names for 12-TET (`A4`) and compact
  degree/cents labels for microtonal scales (`d1 +0c`) instead of raw MIDI
  number plus zero-indexed degree; the 31-EDO minimum-layout case has overlap
  coverage.
- Last-MIDI and capture-event labels now prioritize musician-readable event
  descriptions such as `note C4 (60)` or `cc64 value127` instead of presenting
  every incoming event as a raw note number.
- Screenshot writing now performs a pixel-level smoke check before saving,
  rejecting malformed, blank, or obviously corner-cropped output. The check is
  covered by synthetic full-surface, blank, and corner-only image tests.
- No-probe startup now has a regression test that verifies screenshot-mode app
  construction creates no audio stream, audio output list, MIDI connection, or
  MIDI input list while preserving visible skip statuses.
- Audio-unavailable startup handling now has deterministic tests with an
  injected audio builder, including the saved-device failure and fallback
  failure status path.
- MIDI-unavailable startup refresh handling now has a deterministic test with an
  injected empty input list, proving the app reports no inputs without opening a
  hardware MIDI connection.
- A manual QA checklist now exists at `docs/manual_qa_checklist.md`, covering
  screenshots, startup, project save/recovery, editing, devices, tuning,
  Lumatone maps, assets, settings, and final checks.
- Linux CI now has a GitHub Actions workflow that installs native audio/MIDI
  build dependencies, runs formatting, runs tests, and runs clippy with warnings
  denied.

Remaining caveat: normal interactive startup still probes the host audio stack
and can emit ALSA/JACK warnings on Linux before reporting a recoverable no-audio
state.

## Scope

This document is based on:

- The current Orbifold repo state after the Operad migration.
- The latest rendered screenshot at `screenshots/latest.png`.
- The current UI code in `src/ui/`.
- The app, project, MIDI, audio, settings, Scala, scale, and synth code in `src/`.
- The README, which now describes the current Operad workflow and prototype
  limits.

This document is not a commitment to build everything before the next step. It is
a map of the gap between the current prototype and something a musician can open,
trust, and use without a developer nearby.

## What "Usable" Should Mean

Orbifold is usable when a real user can do the following without reading the
source or being guided through terminal logs:

1. Launch the app from a normal desktop entry or binary.
2. See a readable, stable UI on a laptop, a normal desktop monitor, and a 4K
   display.
3. Select or confirm audio output.
4. Select or confirm MIDI input.
5. Load or choose a tuning.
6. Optionally load a Lumatone mapping.
7. Hear notes immediately when playing a keyboard.
8. Record a short phrase.
9. See that phrase as editable music.
10. Edit notes without fighting the UI.
11. Save the project.
12. Quit and reopen the project without losing work.
13. Understand what went wrong when audio, MIDI, files, or devices fail.
14. Recover from common failure states without restarting or editing config files.

For an early alpha, "usable" does not have to mean polished, extensible, or
feature-complete. It does need to mean the main loop of "connect, play, record,
edit, save, reopen" is reliable.

## Current Foundation

These pieces are valuable and should not be dismissed:

- `src/audio.rs` can enumerate and build CPAL output streams.
- `src/midi.rs` can enumerate MIDI input, handle note on/off, apply a Lumatone
  map, keep the last event, and capture mapping events.
- `src/scale.rs` maps MIDI notes to frequencies with root and base frequency.
- `src/scala.rs` parses Scala `.scl` files and has parser tests.
- `src/project.rs` has a transport, a looped clip, note recording, quantization,
  note editing, undo/redo snapshots, and project serialization.
- `src/synth.rs` has a polyphonic synth, settings, a command queue, delay,
  filtering, drive, envelopes, and soft clipping.
- `src/settings.rs` persists app-level settings.
- `src/app.rs` holds the main state and bridges audio, MIDI, scale, project,
  assets, and settings.
- `src/ui/native.rs` now renders the UI through Operad, handles pointer clicks,
  has keyboard shortcuts, writes screenshots, and includes layout tests that
  catch some text overlap cases. Shared UI label, text, accessibility, and theme
  helpers live in sibling `src/ui/` modules. Keyboard/action dispatch now lives
  in `src/ui/actions.rs`, and native UI tests are isolated under
  `src/ui/native/tests.rs`.
- There is an MIT license.
- The app has a clear subject: microtonal composition and Lumatone-friendly
  performance, not generic note entry.

The problem is not that there is nothing here. The problem is that the existing
pieces do not yet add up to a trustworthy end-user workflow.

## Current Visual State

The latest screenshot is no longer a fake DAW mockup: it shows a real current
project card, a single `Current Clip` lane, and an empty arrangement that says
`No recorded clip`. It also now fits the full workspace at the captured 2x scale:
left panels, track lane, arrangement, piano roll, and right controls are all
visible. That is a substantial improvement over the earlier cropped, oversized
render.

The layout is still not a finished editor. It is sparse, the project and clip
panels still need real workflows behind them, and the top transport is still
text-heavy. The current proportions are much more workable than the previous
oversized version, but Orbifold still needs a responsive density system, not only
a corrected DPI scale factor and narrower default columns.

The earlier minimum-size screenshots showed severe text overlap and clipped
controls. The current layout tests are useful, but they are not a substitute for
visual inspection. They estimate text boxes from the paint list; they do not
prove that the final GPU output is pleasant, proportionate, unclipped, or usable.

The target screenshot the user shared suggests a denser, darker, DAW-like UI with
clear panel hierarchy, compact controls, strong grid editors, visible track
identity, useful side panels, rich colors, and high information density. The
current UI is on the right path structurally, but it is still far from that level
of density and craft.

## Highest-Level Missing Pieces

The biggest missing pieces are:

1. A no-surprises startup path.
2. A complete device setup path.
3. A true multi-track project model.
4. Real arrangement editing.
5. Real piano-roll editing with direct manipulation.
6. Clear tuning and Lumatone workflows.
7. Asset and instrument workflows that actually affect sound.
8. Robust project save, autosave, recovery, and migration.
9. A responsive visual system instead of manual absolute layout everywhere.
10. Automated visual and interaction QA.
11. Packaging and user-facing documentation.

Everything else hangs off those.

## P0: Must Fix Before Calling It Usable

### Startup Must Not Be Fragile

Current problem:

- Interactive startup now has a basic no-audio path, but it still probes the host
  audio stack directly during startup.
- Linux hosts can still print ALSA/JACK warnings before the app reports a
  recoverable no-audio state.
- There is not yet a complete device setup state or diagnostics panel.
- Fatal graphics/window/renderer errors still prevent the UI from appearing.

Missing:

- A "device setup required" state.
- Clear structured status for missing audio and missing MIDI.
- A visible diagnostics panel or status dialog for startup failures.
- A distinction between "audio unavailable", "MIDI unavailable", "settings
  invalid", and "project failed to load".
- A quieter audio probing path that does not dump backend warnings into the
  terminal during normal startup.

Minimum acceptable behavior:

- If audio fails, Orbifold launches muted and says why. The basic version of this
  now exists.
- If MIDI fails, Orbifold launches and shows "No MIDI input connected".
- The A4 test button is disabled or reports a useful error when audio is not
  available. The basic version of this now exists.
- The UI offers refresh and reconnect without requiring restart.

### The UI Needs A Real Information Architecture

Current problem:

- The UI is a large manually positioned surface.
- The hierarchy is unstable across sizes.
- Some panels are too large for their importance.
- Some important workflows are hidden in buttons with no context.
- Some labels are stale or placeholder-like.
- Some sections advertise functionality that is shallow or missing.

Missing:

- A deliberate primary workflow layout.
- A compact top transport.
- A clear separation between global controls, project browser, track list,
  arrangement, editor, and inspectors.
- A true responsive policy for minimum, normal, wide, and 4K displays.
- A scale policy that makes 4K readable without making every control enormous.
- Collapsible or resizable side panels.
- Stable panel minimum sizes.
- Real scroll containers where content can exceed available space.
- Empty states that tell the truth.
- A design token system for spacing, text sizes, colors, borders, and row heights.

Minimum acceptable behavior:

- At minimum size, the app should remain coherent and not overlap.
- At normal desktop size, the arrangement and piano roll should both be useful.
- At 4K, the UI should gain workspace, not simply inflate everything.
- Text should not collide, clip awkwardly, or require guessing.

### Buttons Need Complete Semantics, Not Just Click Handling

Current problem:

- Operad-hosted buttons can dispatch actions.
- Some actions perform real changes.
- Some actions only update `last_status`.
- Some button labels are standing in for proper controls.
- There is no mature control set for menus, dropdowns, sliders, text inputs,
  toggles, tabs, splitters, or scrollbars.

Examples:

- The old placeholder track select/mute/solo/Add Track controls have been
  removed from the default single-clip surface rather than left as fake actions.
- BPM uses tiny plus/minus buttons and a label, not a proper numeric control.
- Quantize cycles through values instead of exposing a clear menu.
- Device selection uses prev/next buttons rather than a device picker.
- The top-bar preferences action now says `Save Settings`, but there is still no
  full settings window.

Missing:

- A standard action model with enabled, disabled, active, pending, and error
  states.
- Real checkbox/toggle semantics for binary options.
- Real menu or dropdown semantics for discrete choices.
- Real numeric steppers/sliders for continuous values.
- Hover, pressed, focus, and keyboard activation states.
- Tooltips for compact icon controls once the UI moves toward icons.
- A way to inspect what command a control maps to during debugging.
- Tests proving that high-value controls change the expected app state.

Minimum acceptable behavior:

- Every visible control either works, is visibly disabled, or is removed.
- `Save Settings`, "Open", "Save", "Scale", "Keys", audio `Mute`, transport,
  device, and clip-editing controls should do what their labels imply.
- Controls should not be placeholders in the default UI.

### Project Data Model Is Too Small For The UI

Current problem:

- `MusicProject` has a single `Clip`.
- The UI has been pulled back to a single `Current Clip` lane, so it no longer
  advertises fake multi-track operations in the default view.
- The central arrangement still implies a DAW timeline, but it currently renders
  only the single recorded clip state.
- Multi-track behavior remains a product/model decision rather than a partially
  exposed UI feature.

Missing:

- `Project` or `Song` owning tracks.
- Track IDs.
- Track names.
- Track colors.
- Track mute/solo/arm state.
- Track instrument assignment.
- Multiple clips per track.
- Clip IDs.
- Clip names.
- Clip colors.
- Clip start and length in arrangement time.
- Selected track and selected clip state.
- Arrangement scroll and zoom state.
- Durable editor state that can be saved or intentionally treated as ephemeral.

Minimum acceptable behavior:

- The arrangement should render real project data for the current milestone.
- If multi-track returns, Add Track should add a real track and Mute/Solo should
  affect playback.
- Selecting a clip should drive the piano roll.
- Saving and reopening should preserve the visible arrangement.

### Arrangement Editing Is Mostly Missing

Current problem:

- The arrangement is visually convincing enough to imply DAW behavior.
- The underlying behavior is closer to a single-loop clip editor.
- The rendered current clip can now be selected, but that selection is currently
  a single-clip context signal rather than a full arrangement clip model.
- There is no complete direct manipulation for clips.

Missing:

- Create clip.
- Durable selected-clip state if arrangement clips become first-class project
  data.
- Move clip.
- Resize clip.
- Split clip.
- Duplicate clip.
- Delete clip.
- Copy/paste clip.
- Rename clip.
- Change clip color.
- Drag from browser to arrangement.
- Snap to grid.
- Toggle snap.
- Zoom horizontally.
- Scroll horizontally.
- Scroll vertically.
- Show loop region.
- Set loop region.
- Scrub or click the ruler to move playhead. Basic ruler click/drag seeking now
  exists; richer scrub behavior is still missing.
- Multi-select clips.
- Marquee selection.
- Context menus.
- Undo/redo grouping for arrangement edits.
- Tests for arrangement edit commands.

Minimum acceptable behavior:

- A user can make two tracks, place two clips, move one, duplicate one, and save.
- The playhead and playback position must match what the arrangement shows.
- Clip editing should be visible, reversible, and persisted.

### Piano Roll Needs Direct Manipulation

Current problem:

- The project core supports note editing commands.
- The UI has note selection and command buttons.
- Clicking the piano-roll grid can add a note at the clicked time and pitch.
- There is a first direct-manipulation path: dragging a note changes its start
  beat and pitch.
- Dragging a note's right edge changes its duration.
- Dragging a note's left edge changes its start while preserving its end.
- The current single selected note can be deleted or duplicated with toolbar
  buttons and keyboard shortcuts.
- Velocity can be edited by clicking or dragging a note's velocity bar in the
  piano-roll velocity lane.
- The active quantize grid is visible in the editor as sub-beat grid lines.
- There is not yet a full direct-manipulation piano-roll experience.
- Dense microtonal pitch labeling is hard to read at some sizes.

Missing:

- Multi-select notes.
- Marquee note selection.
- Multi-note copy/paste once multi-select exists.
- Multi-note delete and duplicate operations once multi-select exists.
- Optional scale-degree display.
- Scroll and zoom.
- Consistent editing cursor and hit target behavior.

Minimum acceptable behavior:

- The user can record a phrase, click a wrong note, move it, resize it, adjust
  velocity, and undo the edit.
- Note labels are readable enough to identify pitches in the active tuning.

### Save/Open Must Be Trustworthy

Current problem:

- Project serialization exists.
- Open/save actions exist.
- The UI now has a basic dirty/saved/no-file indication.
- The dirty model compares against the last clean project snapshot, so undoing
  back to a saved state can clear the dirty marker.
- Save now uses a temporary file and preserves a `.bak` of the previous project
  when overwriting.
- Autosave/recovery exists, and Save As can write the current project under a
  new path.

Missing:

- A full recent-project browser with multiple choices, missing-file handling,
  and project management actions.
- Configurable autosave interval and storage location.
- Crash-recovery polish beyond the current manual Recover action.
- Backup rotation beyond the single `.bak` file.
- Clear error messages for failed save/open.
- Confirmation for destructive operations.
- Project format version migration tests with fixtures.
- Cross-platform path handling for moved assets.

Minimum acceptable behavior:

- Save creates a project file in a known location.
- Reopen restores the visible project.
- The user can tell whether changes are saved. The basic version of this now
  exists.
- A save failure is visible and does not silently discard work.

### User Docs Are Still Thin

Current problem:

- `README.md` now describes the Operad UI and prototype status, but it is still
  a developer-facing overview rather than a complete user guide.
- It lists some features that exist in code but still need clearer workflow
  documentation and UI affordances.

Missing:

- First-run setup docs.
- Audio troubleshooting.
- MIDI troubleshooting.
- Lumatone setup guide.
- Scale loading guide.
- Project save/load guide.
- A clear "prototype status" section.

Minimum acceptable behavior:

- A developer or early tester can follow the README and get the current app
  behavior.

## P1: Needed For A Useful Musical Alpha

### Audio Device Workflow

Current problem:

- Audio output enumeration and connection exist.
- The UI exposes audio through compact prev/next/refresh/connect controls, with
  visible selected-device position.
- There is no full device preferences workflow.

Missing:

- Device picker.
- Audio backend diagnostics.
- Recovery when a selected device disappears.
- Clear fallback when default device changes.
- Persisted device preference that does not make startup fragile.

Minimum useful alpha behavior:

- User opens settings, chooses an audio output, tests A4, sees a meter, and can
  reconnect if the device changes.

### MIDI Device Workflow

Current problem:

- MIDI enumeration and connection exist.
- The app opens a selected MIDI input and captures events.
- Long MIDI names have already caused layout pressure.

Missing:

- Device picker.
- Hotplug refresh behavior.
- Synth response policy for pitch bend and non-sustain CC messages.
- MPE or multi-channel policy.
- Multiple simultaneous MIDI inputs, or an explicit decision not to support them.

Minimum useful alpha behavior:

- User chooses a MIDI input, sees events arriving, plays notes, and can reconnect
  if the keyboard is unplugged.

### Lumatone Workflow

Current problem:

- `.ltn` parsing exists.
- A map can be loaded.
- Mapping capture exists.
- The UI surfaces some Lumatone status.

Missing:

- Clear distinction between scale and keymap.
- Current keymap name.
- Keymap preview.
- Preset browser.
- Validation of whether keymap matches active tuning.
- Mapping capture review.
- Save captured map.
- Send colors/layout back to Lumatone, if that is in scope.
- Explain channel/note mapping.
- Handle factory presets as first-class choices.
- Detect when no Lumatone is connected but a map is loaded.

Minimum useful alpha behavior:

- User can load a 31-EDO scale, load a matching Lumatone preset, play the device,
  and understand what mapping is active.

### Tuning And Scale Workflow

Current problem:

- Scala parsing and recent/library state exist.
- Root pitch and base frequency exist, and the root is now labeled with both a
  note name and MIDI number.
- The UI has scale controls, but the workflow is still rough.

Missing:

- Scale browser with source/location.
- Import scale.
- Remove scale.
- Rename/favorite scale.
- Search/filter.
- Display scale intervals.
- Display EDO metadata when relevant.
- `.kbm` keyboard mapping support, or an explicit decision to use Lumatone maps
  instead.
- Root note selector.
- Base frequency numeric input.
- Retune behavior for currently sounding notes.
- Tuning compatibility with recorded notes.
- Per-project scale vs global default distinction.

Minimum useful alpha behavior:

- User can load a scale, see what is loaded, set root/base frequency, play it,
  save it with the project, and reopen it.

### Synth And Sound Design Workflow

Current problem:

- The internal synth is more capable than the UI exposes cleanly.
- Parameters can be adjusted through plus/minus actions.
- There is no proper instrument panel.

Missing:

- Synth preset browser.
- Save preset.
- Load preset.
- Proper controls for gain, waveform, attack, release, filter, delay, and drive.
- Parameter ranges and units shown clearly.
- Reset to default.
- Per-track synth state once tracks exist.
- Meters.
- Polyphony status.
- Voice stealing policy visible or documented.
- All-notes-off/panic behavior that is prominent but not disruptive.
- Automation policy.

Minimum useful alpha behavior:

- User can choose a sound, adjust core parameters, save the project, and reopen
  with the same sound.

### Asset Browser Workflow

Current problem:

- Asset folders exist.
- The app scans `audio_assets/`.
- The browser can list assets and import files.
- The assets do not yet appear to drive a complete sound workflow.

Missing:

- Preview samples.
- Stop preview.
- Waveform thumbnail.
- File metadata.
- Drag asset to track.
- Load sample into instrument.
- Load preset into synth.
- Load impulse into effect, if effects exist.
- Missing-file handling.
- Relative vs absolute path policy.
- Asset copy/import collision policy.
- Tags or categories.
- Search.
- Rescan progress and errors.

Minimum useful alpha behavior:

- User imports a sample, previews it, and uses it in a track or instrument.

## P2: Needed For A Credible DAW-Like Experience

### Mixer And Routing

Missing:

- Track volume.
- Track pan.
- Track mute/solo that affects audio.
- Track meters.
- Master meter.
- Output gain.
- Basic sends or an explicit decision to defer sends.
- Per-track effects or an explicit decision to defer effects.
- Clipping protection.
- Rendering/export path.

Why it matters:

- A multi-track UI implies mixing. Without mixer semantics, track lanes are only a
  visual metaphor.

### Recording Workflow

Current problem:

- Recording into the single clip exists.
- Overdub exists as a transport flag.
- There is no full recording session flow.

Missing:

- Record arm per track.
- Input monitoring.
- Count-in.
- Audible metronome.
- Metronome level.
- Punch in/out, or an explicit decision to defer.
- Take handling, or an explicit decision to overwrite/overdub only.
- Latency compensation.
- Clear recording status.
- Handling notes already down when recording starts/stops.
- Handling sustain pedal during recording.

Minimum useful behavior:

- User knows exactly where recording will go and whether it will replace or
  overdub existing material.

### Transport And Timeline

Current problem:

- Basic Play, Pause, Stop, and return-to-start behavior exists.
- The arrangement ruler supports click/drag seeking.
- The piano-roll ruler also supports click/drag seeking, so note editing can
  reposition the playhead without jumping to the arrangement panel.
- Looping is still a single fixed loop length rather than a visible editable loop
  region.

Missing:

- Loop start/end.
- Tempo changes, or explicit single-tempo constraint.
- Time signature changes, or explicit single-meter constraint.
- Follow playhead toggle.
- Keyboard shortcut tooltips or in-app shortcut discoverability.

### Editing History

Current problem:

- Undo/redo snapshots exist for project edits.
- It is not clear that every user-visible edit participates correctly.

Missing:

- Undo grouping for arrangement drags and other future multi-object gestures.
- Undo labels.
- Undo coverage for arrangement edits.
- Undo coverage for track edits.
- Undo coverage for synth changes, or explicit decision not to include them.
- Undo coverage for scale/keymap changes.
- Tests for each command.
- Avoid saving transient UI state as undoable project edits unless intended.

### Project Browser

Current problem:

- The left project panel shows the current project file state.
- It now shows recent-project rows when history exists and a truthful empty
  state when it does not.
- Recent rows can be opened or forgotten individually without deleting project
  files.
- Missing recent project files are marked, cannot be opened from their row, and
  can be forgotten from the project panel.
- Existing recent rows now include a compact modified-age label such as `now`,
  `5m`, `2h`, or `3d`.
- It is still a lightweight project switcher, not a full project browser.

Missing:

- Duplicate project.
- Rename project.
- Delete project files.
- Search.
- Rich project metadata and file-management actions.
- Rich missing-project recovery actions beyond forgetting stale entries.

Minimum useful behavior:

- The current project card is acceptable for the prototype. A browser should not
  return until it is backed by real recent-project data and project actions.

### Scale Browser

Current problem:

- Scales and tunings are important enough to have a major panel.
- The current UI now has a small factory scale library, an explicit active-scale
  card, Load/Loaded/Refresh/Remove actions, and remembered user-added `.scl`
  paths.
- It still needs to become a full browser.

Missing:

- User scale library directory beyond the checked-in `scales/` folder.
- Recent scales.
- Search/filter.
- Interval preview.
- Optional `.kbm` import.
- Rich error display for malformed files beyond the status bar.

### Inspector Panels

Current problem:

- The right-side panels have moved between helper, chord hints, arpeggiator,
  control, and MIDI/audio controls across iterations.
- The user-visible purpose of the inspector area is not settled.

Missing:

- Decide whether the right side is an inspector, helper, device panel, synth
  panel, or tabbed utility area.
- If it is an inspector, it should follow selection.
- If it is a device panel, it should prioritize audio/MIDI setup.
- If it is a helper panel, suggestions must be useful and interactive.
- Chord hints need to be musically valid for the active tuning.
- Arpeggiator controls need an actual arpeggiator engine or should be absent.

Minimum useful behavior:

- The right panel should answer "what can I do with the thing I selected?"

## UI Toolkit And Operad Integration Gaps

Orbifold should not rebuild a whole UI toolkit inside `src/ui/native.rs`.
Operad should provide enough structure that Orbifold code can describe music
workflows instead of managing every rectangle by hand.

Missing from the current integration:

- Layout primitives that reduce absolute positioning.
- Scroll containers.
- Split panes.
- Tab controls.
- Menus.
- Popovers.
- Combo boxes.
- Sliders.
- Steppers.
- Text input.
- Lists with selection.
- Virtualized lists for large libraries.
- Canvas widgets for arrangement and piano roll.
- Hit testing helpers for canvas sub-elements.
- Pointer capture for drags.
- Gesture lifecycle: down, drag, up, cancel.
- Keyboard focus.
- Accessibility metadata.
- Theme tokens.
- Text measurement that can be trusted for layout.
- Visual state styling for hover, active, disabled, focus, selected, and error.

Orbifold-specific boundaries that should exist:

- Operad should own general UI layout, text, widgets, input events, and rendering.
- Orbifold should own musical domain state and command handlers.
- Orbifold should have custom canvas/editor widgets for timeline and piano roll.
- Domain commands should be typed where practical, not raw strings spread across
  the UI.
- The app should have a thin adapter layer from Operad events to Orbifold
  commands.

Current risk:

- `src/ui/native.rs` can become a large pile of layout, rendering, state
  querying, input dispatch, and command handling in one file.
- That makes every UI change risky and makes tests more awkward.

Recommended direction:

- Continue splitting `src/ui/` into modules:
  - `ui/native.rs` for Winit/WGPU lifecycle.
  - `ui/surface.rs` for top-level scene assembly.
  - `ui/theme.rs` for colors/type/spacing.
  - `ui/actions.rs` for command dispatch.
  - `ui/layout.rs` for responsive panel geometry.
  - `ui/widgets.rs` for reusable local controls if Operad lacks them.
  - `ui/arrangement.rs` for arrangement canvas rendering and input.
  - `ui/piano_roll.rs` for piano-roll rendering and input.
  - `ui/tests.rs` for visual/layout tests.

## Visual Design Gaps

The target visual direction is a dense, modern, dark music workstation. The
current UI uses the same broad ingredients but not yet the same design quality.

Missing:

- A compact, consistent top bar.
- Icon buttons for common transport/tools once tooltips exist.
- Stable row heights.
- Stable toolbar heights.
- Better contrast hierarchy.
- Clear selected states.
- Clear disabled states.
- Better panel density.
- Less oversized typography at high scale.
- Proper grid line hierarchy in editors.
- Better use of color: track identity should be vivid, panels should be quiet.
- Meter styling.
- Transport state styling.
- Clip color consistency between track list, arrangement, and piano roll.
- Better status bar content hierarchy.
- Better empty states in secondary panels. The empty clip path now reports
  itself in both the arrangement and the piano roll.
- Better window minimum size policy.

Specific screenshot issues to keep watching:

- At minimum window size, text can overlap or clip if a layout path is missed.
- At high scale, the UI must remain dense enough to preserve editing workspace.
- Huge controls reduce workspace.
- The arrangement can be clipped by panel proportions.
- The piano roll and lower editor can become too small or off-screen.
- Top-level controls need more density and prioritization.

## Accessibility And Input Gaps

Missing:

- Better keyboard focus scopes and ordering once the app has richer panels,
  menus, text inputs, and modal surfaces.
- Keyboard activation beyond simple focused buttons, especially editor-specific
  controls that need structured focus models instead of pointer-only hit areas.
- More deliberate visible focus styling once the visual system is closer to the
  target design.
- Broader screen reader semantics beyond button labels, if Operad supports them
  or plans to.
- High-contrast theme.
- Reduced-motion policy if animations are added.
- User text scale setting.
- A deliberate hit-target policy for dense editor gestures, such as piano-roll
  notes, resize handles, velocity bars, and timeline regions.
- Interactive shortcut reference/tooltips beyond the `?` status hint and
  markdown reference.
- Conflict checks for future non-keyboard command surfaces.
- International keyboard considerations for shortcuts.

Even if full accessibility is deferred, keyboard-only operation for common
actions should come early because it also improves power-user workflow.

## Performance And Threading Gaps

Current risk areas:

- UI rendering reads shared state through locks.
- Audio, MIDI, project, and UI state interact through shared locks and command
  queues.
- Large projects have not been tested.
- Large asset libraries have not been tested.
- Screenshot rendering uses the real renderer path, which is good, but visual
  baseline coverage is not yet present.

Missing:

- Frame time instrumentation.
- Audio underrun/dropout diagnostics.
- MIDI event latency measurement.
- Large project performance tests.
- Large asset folder scan tests.
- Lock-order review.
- Avoid holding locks while doing expensive rendering preparation.
- Explicit real-time audio thread safety review.
- Stress test for many notes.
- Stress test for many clips.
- Stress test for many assets.

Minimum useful behavior:

- A normal loop should play without audio glitches while the UI is idle and while
  editing.
- UI stalls should not break audio.

## Testing And QA Gaps

Existing tests are useful:

- Scala parser tests.
- Scale behavior tests.
- MIDI handling tests.
- Project recording/editing/serialization tests.
- Dirty state tests for clip edit, save, load, and project-file state labeling.
- Layout text overlap tests.
- Screenshot generation path has been manually exercised.

Still missing:

- Visual golden screenshots.
- Interaction tests for every important button.
- Tests for open/save dialogs are hard, but command-level save/open can be tested.
- Performance benchmarks.

Important warning:

- `cargo test` passing does not mean the UI is visually acceptable.
- `cargo run -- --screenshot` succeeding does not mean the screenshot looks good.
- Every UI iteration needs human or automated image inspection.

## Packaging And Release Gaps

Missing:

- App icon.
- Desktop file on Linux.
- Bundle/package format.
- Version display in UI.
- About dialog.
- License display.
- Crash/error reporting story.
- Default directories.
- App data directory policy.
- Config directory policy.
- Project file association.
- Installer or archive packaging.
- Release checklist.
- Changelog.
- GitHub repo setup and tags, if not already done.

Minimum useful alpha behavior:

- A tester can download/build, launch, find settings/projects, and report the app
  version.

## Documentation Gaps

Missing user docs:

- First run.
- Audio setup.
- MIDI setup.
- Lumatone setup.
- Loading scales.
- Recording.
- Editing notes.
- Saving/opening projects.
- Troubleshooting.
- Known limitations.

Missing developer docs:

- Architecture overview.
- Audio/MIDI threading model.
- Project file format.
- Settings file format.
- Operad integration model.
- UI testing workflow.
- Release workflow.
- How to add a new control/action.
- How to add a new project command.

Current documentation problem:

- The README is now current at a high level, but user docs, troubleshooting, and
  architecture docs are still too thin for testers.

## Data Safety Gaps

Missing:

- Autosave recovery.
- Backup rotation and cleanup policy.
- Confirm before destructive operations.
- Distinguish remove-from-library vs delete-from-disk.
- Import conflict handling.
- Project migration backups.
- Clear error when writing settings fails.
- Clear error when project save fails.
- Avoid saving bad state after failed startup.

Why this matters:

- Music software becomes unusable the first time a user loses a good idea.

## Musical Correctness Gaps

Missing:

- Tests that recorded frequencies remain correct under different scales.
- Policy for what happens if the scale changes after recording.
- Distinction between storing raw key, MIDI note, scale degree, and frequency.
- Clear note identity for Lumatone mapped notes.
- Retuning old clips.
- Exporting or displaying microtonal pitch information.
- Chord helper correctness in non-12-TET tunings.
- Arpeggiator correctness in non-12-TET tunings, if arpeggiator stays.
- Pitch labels that do not imply 12-TET when the active scale is not 12-TET.

Important design decision:

- Decide whether notes are stored as scale degrees, MIDI notes, Lumatone keys,
  absolute frequencies, or some combination. The current `ClipNote` stores
  several of these, but the long-term semantics need to be explicit.

## Specific Backlog

### Immediate P0 Backlog

1. Finish hardening no-audio startup and device diagnostics.
2. Make every top-bar button either fully work or visibly disable it.
3. Add a real settings/device panel.
4. Replace the current global scaling behavior with a responsive density policy.
5. Add visual screenshot review to the normal development checklist.
6. Keep splitting `src/ui/native.rs` before it becomes too costly to change.
7. Add saved-baseline tracking, autosave, backup, and recovery on top of the
   basic dirty indicator.

### Near-Term P1 Backlog

1. Decide whether the next milestone is the current single-loop instrument or a
   real multi-track arrangement.
2. If multi-track is the milestone, introduce real `Track` and
   `ArrangementClip` types before re-adding track controls.
3. Persist tracks and clips if they return.
4. Select arrangement clip and show it in the piano roll.
5. Add piano-roll multi-select, marquee selection, and multi-note edit commands.
6. Add arrangement clip selection and movement if arrangement clips return.
7. Build a fuller scale browser/import flow.
8. Build a keymap browser/load flow.
9. Build proper audio and MIDI device pickers.
10. Add project recovery/autosave.

### Medium-Term P2 Backlog

1. Add mixer state and UI.
2. Add track synth assignment.
3. Add synth preset loading/saving.
4. Make audio assets usable in sound generation.
5. Add visual golden tests.
6. Add large-project performance tests.
7. Add app packaging.
8. Add user manual pages.
9. Add accessibility pass.
10. Add export/render if Orbifold is meant to create shareable audio.

## Proposed Roadmap

### Phase 0: Make The Prototype Honest And Robust

Goal:

- The app launches reliably, tells the truth about what works, and does not show
  fake workflows.

Deliverables:

- No-audio mode.
- Keep the README accurate as behavior changes.
- Screenshot mode without audio dependency.
- Basic dirty/saved/no-file project state.
- Device status that is visible.
- Disabled or removed placeholder controls.
- Current UI visually checked at minimum, normal, wide, and 4K sizes.
- Responsive density rules so 4K is readable without wasting workspace.

Exit criteria:

- A user can launch the app even without audio/MIDI.
- The visible UI does not imply major workflows that do not exist.

### Phase 1: One Complete Musical Loop

Goal:

- A user can connect devices, choose a tuning, record one loop, edit it, save it,
  reopen it, and hear the same result.

Deliverables:

- Audio picker.
- MIDI picker.
- Scale loader.
- Keymap loader.
- Record/edit/save/reopen loop workflow.
- Direct piano-roll editing for basic note movement and resize.
- Clear saved/unsaved state.

Exit criteria:

- Manual test: record a phrase from MIDI, fix one wrong note, save, quit, reopen,
  play it back.

### Phase 2: Real Arrangement

Goal:

- The multi-lane DAW UI becomes real.

Deliverables:

- Tracks.
- Arrangement clips.
- Clip movement/resizing.
- Track mute/solo.
- Selected clip drives piano roll.
- Persisted arrangement.
- Arrangement undo/redo.

Exit criteria:

- Manual test: create three tracks, place multiple clips, mute one track, save,
  reopen, and verify the arrangement and playback state.

### Phase 3: Sound And Assets

Goal:

- Orbifold becomes musically useful beyond a single internal synth patch.

Deliverables:

- Track synth settings.
- Presets.
- Asset preview.
- Sample/instrument use path.
- Mixer basics.
- Meters.

Exit criteria:

- Manual test: import an asset or preset, assign it to a track, record against it,
  save, and reopen.

### Phase 4: Release Quality

Goal:

- Orbifold can be shared with testers.

Deliverables:

- Packaging.
- App icon.
- Manual.
- Troubleshooting guide.
- Visual regression tests.
- Performance sanity tests.
- Crash/recovery story.

Exit criteria:

- A tester can install/run the app and complete the Phase 1 workflow without
  developer help.

## Acceptance Tests For "Usable Alpha"

These should become a mix of automated tests and manual checklist items.

### Startup

- Launch with valid audio and MIDI.
- Launch with no MIDI.
- Launch with missing saved MIDI device.
- Launch with missing saved audio device.
- Launch with no audio output available.
- Launch with malformed settings.
- Run `cargo run -- --screenshot` on a machine without MIDI hardware.

### Visual

- Minimum supported size has no text overlap.
- 1400x760 has no text overlap.
- 1920x1080 has no text overlap.
- 2560x1440 logical size is readable and not comically oversized.
- 4K monitor scale is readable with useful workspace.
- Long audio/MIDI device names do not break layout.
- Long asset names do not break layout.
- Empty project state looks intentional.

### Audio/MIDI

- A4 test produces sound.
- All Off stops stuck notes.
- MIDI note-on produces sound.
- MIDI note-off releases sound.
- Reconnect MIDI after unplug/replug.
- Reconnect audio after device change.

### Tuning

- Load 12-TET.
- Load 31-EDO.
- Load Scala file.
- Reject malformed Scala file with visible error.
- Change root.
- Change base frequency.
- Save/reopen project with tuning.

### Lumatone

- Load factory preset.
- Load user `.ltn`.
- Reject malformed `.ltn`.
- Show active map.
- Capture mapping events.
- Clear capture.

### Recording And Editing

- Record one loop.
- Quantize loop.
- Select note.
- Delete note.
- Duplicate note.
- Move note.
- Resize note.
- Change velocity.
- Undo each edit.
- Redo each edit.
- Save and reopen edited loop.

### Arrangement

- Add track.
- Rename track.
- Mute track.
- Solo track.
- Add clip.
- Move clip.
- Resize clip.
- Duplicate clip.
- Delete clip.
- Save/reopen arrangement.

### Persistence

- Save new project.
- Save existing project.
- Save As.
- Open old project format if compatibility is still promised.
- Recover autosave after simulated crash.
- Handle missing asset path.
- Handle read-only project location.

## Product Decisions To Make

These should be decided explicitly because they shape the architecture.

### Is Orbifold A DAW, An Instrument, Or A Composition Sketchpad?

The UI currently points toward DAW. The engine is closer to a microtonal
instrument plus loop recorder. Both are valid, but the product must choose a
primary identity for the next milestone.

If DAW:

- Prioritize tracks, clips, arrangement, mixer, project browser, and editing.

If instrument:

- Prioritize tuning, Lumatone, synth, performance controls, MIDI, presets, and
  low-latency reliability.

If sketchpad:

- Prioritize fast capture, loop editing, saving, browsing ideas, and export.

### How Important Is Lumatone?

If Lumatone is central:

- The app should make keymaps, color layouts, scale/key consistency, and device
  feedback first-class.

If Lumatone is optional:

- The UI should not depend on Lumatone concepts to explain basic pitch behavior.

### Should Audio Assets Matter In 1.x?

If yes:

- Build the asset-to-sound path soon.

If no:

- Shrink the asset browser until it becomes real.

### Should Operad Own More Widgets?

If yes:

- Keep pressing Operad toward standard controls, layout, scrolling, focus, and
  accessibility.

If no:

- Orbifold needs a local UI layer on top of Operad, but that increases maintenance
  cost.

### What Is The Project File Contract?

Need decisions:

- Human-readable or binary long term.
- How migrations work.
- How assets are referenced.
- How scales/keymaps are embedded or referenced.
- Whether project files are portable between machines.

## Recommended Next Step

The next best engineering step is a layout and density pass, not another new
panel. The app is now more honest than it was, but the screenshot still proves
that the surface is not proportioned like a usable editor.

1. Replace global enlargement with responsive density rules for normal, wide,
   and 4K windows.
2. Continue disabling or removing controls that only pretend to work.
3. Decide whether the next milestone is "single-loop instrument" or "real
   multi-track arrangement".
4. If the answer is multi-track, add real track and arrangement-clip data before
   further polishing the arrangement UI.

The reason is simple: the app is not helped by looking more like a DAW if the
surface is too large to work in and the visible controls do not map to durable
state. Orbifold needs one complete, trustworthy musical workflow inside a layout
that fits before it needs more panels.

## Definition Of Done For First Usable Alpha

Orbifold can be called a usable alpha when all of these are true:

- It launches without audio or MIDI hardware.
- It has accurate documentation.
- It has a visible device setup path.
- It can load a tuning.
- It can optionally load a Lumatone map.
- It can play notes from MIDI.
- It can record a loop.
- It can edit that loop directly in the piano roll.
- It can save and reopen the project.
- It does not lose work on common errors.
- It has no obvious text overlap at supported sizes.
- It has been visually inspected, not only compiled and tested.
- Every visible primary control either works or is clearly unavailable.

Until that is true, Orbifold is a promising prototype rather than a usable
application.
