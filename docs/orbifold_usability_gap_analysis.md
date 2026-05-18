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
- Default audio output name-read failures now surface as explicit audio
  unavailable errors instead of silently connecting an output named `Unknown`.
- Sound-producing controls such as A4 test are disabled when no audio stream is
  connected, while `All Off` remains available as a panic/reset action because
  it also clears live MIDI held-note and sustain state.
- The A4 test path reports a visible status when audio is unavailable.
- Settings capture no longer persists an empty audio output name as a meaningful
  saved device.
- The README now describes the Operad UI and current prototype limits instead of
  the old egui/menu workflow.
- A first-run guide now covers launch, setup-required state, audio/MIDI setup,
  scale and Lumatone loading, recording, piano-roll editing, save/recovery,
  visual sizing, and basic troubleshooting for early testers.
- `docs/troubleshooting.md` now covers deeper startup-probe, logging, audio,
  MIDI, settings, project, autosave, and bug-report diagnostics for early
  testers.
- `docs/lumatone_setup.md` now explains the current scale/key-map distinction,
  factory `.ltn` presets, user key-map loading, MIDI setup, mapping capture, and
  Lumatone workflow limitations.
- `docs/known_limitations.md` now gives testers a single place to check current
  prototype limits across composition, piano-roll editing, audio/MIDI, assets,
  tuning/Lumatone, files, UI/help, release, and screenshot reporting.
- `docs/keyboard_shortcuts.md` now includes workflow examples for recording,
  adding and shaping notes, undo/redo, copy/paste at the playhead, zoom/readable
  views, and safe save/open behavior, rather than only listing shortcut chords.
- `docs/lumatone_troubleshooting.md` now covers advanced Lumatone setup checks,
  manual scale/key-map validation, channel-filter mistakes, mapping capture,
  common `.ltn` parse errors, wrong-note diagnosis, and bug-report evidence.
- Loaded Lumatone key maps are now only applied to MIDI inputs whose port names
  identify them as Lumatone devices. Regular MIDI keyboards keep chromatic
  note-to-scale mapping even when a key map is loaded, and the control panel
  reports `Key map inactive` for that state.
- The fake recent-project list has been replaced by real recent-project state
  and compact session-strip actions that only appear when useful.
- Unsupported track Add/Mute/Solo controls are visually disabled until there is a
  real multi-track engine behind them.
- The fake multi-track arrangement preview has been replaced by the current
  single-clip surface: one `Current Clip` lane, an `Empty clip` arrangement state, and
  no preview notes in the piano roll before recording.
- A basic project dirty state now exists. Recording, clip edits, tuning/keymap
  changes, transport loop changes, synth changes, undo, and redo mark the project
  dirty; save/load mark it clean; and project state helpers report `No file`,
  `Unsaved`, `Unsaved changes`, or `Saved`.
- The Operad host no longer applies DPI scaling twice. The latest screenshot
  fits the full workspace at 2x scale instead of showing a cropped, oversized
  subset of the UI.
- The top bar now labels the overdub toggle honestly, reports loop length in
  beats instead of falsely calling the value bars, and removes a decorative meter
  that collided with controls.
- The default layout now gives more horizontal space to the arrangement and
  piano roll by narrowing the left browser, clip, and control columns.
- Workspace resize handles now have wider hit targets and larger visible grips,
  making the left/clip/right/bottom splitters easier to find and grab at compact
  and 4K sizes.
- Disabled placeholder track controls (`+ Track`, mute, and solo) have been
  removed from the visible single-clip UI.
- Project save now writes through a temporary file in the project directory and
  creates a `.bak` copy before overwriting an existing project file.
- Project saves that succeed but fail to clear a stale autosave now keep the
  saved status visible while also logging and recording the autosave cleanup
  failure in diagnostics.
- Piano-roll notes can now be dragged to change start beat and pitch, with the
  drag grouped into a single undo history entry.
- Piano-roll note right edges can now be dragged to resize duration, also grouped
  into a single undo history entry.
- Piano-roll note left edges can now be dragged to change the note start while
  preserving the note end, also grouped into a single undo history entry.
- Piano-roll note edge drags now honor the active snap grid, while `Snap off`
  keeps edge resizing free.
- Snapped direct note edits that resolve to the note's current start, length,
  pitch, or velocity now remain true no-ops instead of dirtying the project or
  enabling Undo for no visible change.
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
- File actions now include an explicit `Save As` action, `Ctrl`/`Cmd+Shift+S`
  routes to the same workflow, and Save As has a regression test proving it
  writes a new project file without overwriting the original saved file.
- Top-bar Save, Save As, Open, Scale, and Keys actions now have pointer-level
  regression coverage that proves they queue pending file dialogs with visible
  opening statuses instead of blocking the UI action path.
- Invalid BPM commits and stale library/key-map selection actions now use the
  shared logged diagnostic path instead of unlogged footer-only statuses.
- Recoverable project load and autosave recovery warnings, such as missing
  referenced sample instruments or key maps, now preserve the successful
  load/recover status while also recording the warning in diagnostics.
- The default autosave recovery file is now included in `.gitignore`, matching
  the existing settings and screenshot ignores so local recovery state is less
  likely to leak into release artifacts.
- Re-loading the already-active Scala file through file-open style paths, or
  the already-active Lumatone key map through edit paths, now reports that it is
  already loaded instead of creating unsaved project state.
- No-op quantize and minimum-length note resize commands now detect the no-op
  before touching undo history, so they no longer clear available redo edits.
- The compact top transport now spaces the tempo controls without overlapping
  the loop/quantize controls, and the visible tempo buttons say `BPM -` and
  `BPM +` instead of anonymous plus/minus glyphs.
- The top-bar BPM value is now an editable numeric field: typing a tempo and
  pressing Enter commits it, while Escape cancels without dirtying the project.
- Wider top-transport layouts now label loop-length controls as `Loop -` and
  `Loop +`, so loop duration editing no longer relies on anonymous plus/minus
  buttons beside the beats readout.
- The top-bar settings-save action now expands from `Prefs` to `Save Pref` to
  `Save Settings` as horizontal space increases, avoiding unnecessary
  abbreviation on wide displays.
- The top quantize-grid control now labels itself as `Q1/16` at compact sizes
  and `Grid 1/16` when there is room, instead of presenting a bare value with no
  control context.
- The piano-roll option strip now uses the same explicit `Q1/16` grid label,
  so its Snap/grid pair is readable without relying on source-order context.
- The piano-roll option strip now also exposes adjacent previous/next grid
  buttons around the grid label, so users can move one step coarser or finer
  without repeatedly cycling through every value.
- Wide top-bar layouts now expose the same previous/next grid controls around
  `Grid 1/16`; compact layouts still keep the single cycle button to avoid
  crowding the transport.
- The compact all-notes-off action uses the visible label `Panic`, keeping the
  emergency note reset discoverable without the ambiguous standalone `Off`.
- The same action now exposes `Panic: all notes off` through accessibility and
  focus status, instead of relying on a generic fallback label.
- Saved and opened projects now update a persisted recent-project list, and the
  session strip can surface recent-project rows when history is available.
- The left session strip now shows up to three named recent-project rows. Clean
  projects can open any visible row directly; dirty projects keep those open
  actions disabled until the user saves or discards changes.
- `Open Recent` now prunes missing recent projects, persists the cleaned list,
  and opens the next available project in the same action, so moved/deleted files
  do not leave a dead one-click action in the session strip.
- The left session strip now keeps per-row `Forget` controls visible when recent
  project state exists, removing that entry from Orbifold's recent list without
  deleting the project file.
- The native window title now includes the project name and an unsaved marker,
  so saved/dirty state is visible outside the left browser.
- Failed project open/parse statuses now name the target file, and failed opens
  leave the current project and recent-project list untouched.
- Failed autosave write/open/parse/load statuses now name the autosave file, and
  a bad autosave recovery attempt leaves the current project untouched.
- Autosaves can now be dismissed from the clean left session strip, removing stale or
  unwanted recovery files without forcing a recover attempt or leaving the
  session strip stuck in recovery mode.
- Startup now sizes the window from the current monitor instead of always opening
  at the old fixed `1400x760` prototype size, so large displays get a much larger
  initial workspace.
- The asset browser now shows a selected-kind count and a real empty state when
  there are no imported files for the active tab.
- The asset browser empty state now uses a single clear status line, avoiding
  the duplicate adjacent text rendering seen in visual QA.
- Missing audio asset files are marked in the asset browser until Refresh
  removes stale entries and clears stale selection.
- Orbifold now has a wasm browser entry at `examples/orbifold_web.rs`, a static
  `web/index.html` fallback shell, a `scripts/build-web.sh dist` build path, and
  a GitHub Pages workflow. The wasm entry uses shared `AppState` and action
  dispatch. Browser project open/save uses the same `.orbifold` text format,
  and browser Scala/key-map loading uses the same `.scl` and `.ltn` parsers as
  desktop. Browser MIDI input uses Web MIDI where available and feeds the same
  MIDI handling path as desktop. Browser audio uses Web Audio and the shared
  synth engine. Browser settings reuse the same plain-text settings format in
  `localStorage`, and the browser keeps the latest project session there so
  reloads can restore the current `.orbifold` state. The browser surface now
  uses the shared Operad document builder, including piano-roll note editing,
  panel splitters, loop-end drags, viewport thumb drags, and piano-roll wheel
  navigation. Browser asset import can load WAV samples for preview and
  sample-instrument use, persists browser-imported bytes in IndexedDB with
  legacy `localStorage` migration/merge/fallback, and restores them before the
  saved browser project session so sample instruments survive reloads within
  browser storage quota. Web UI-scale actions persist the new setting and reload
  the page so the fixed Operad web runtime scale is reapplied on startup.
- Browser keyboard shortcuts for file, scale, key-map, asset, and MIDI actions
  now use the same browser API paths as pointer clicks instead of falling
  through to desktop file-dialog commands. Browser project open also keeps the
  dirty-project confirmation gate before launching the picker, and the browser
  shortcut bridge now includes the `?` shortcut help action plus native-style
  repeat behavior for arrow note edits.
- Browser shortcut modifier handling now also matches native for shifted copy,
  paste, and add-note keys, so `Ctrl`/`Cmd+Shift+C`, `Ctrl`/`Cmd+Shift+V`, and
  `Shift+N` do not trigger browser-only edit behavior.
- Browser MIDI connect now matches native stale-device behavior: if the selected
  browser MIDI input name is no longer present, it reports an error instead of
  disconnecting the current input and silently opening the first available one.
- Browser MIDI refresh now treats an empty input list as a successful no-device
  refresh, matching native setup behavior instead of reporting an error only
  because the browser returned zero inputs.
- Browser MIDI connect now explicitly opens the selected `MIDIInput` when the
  browser exposes `input.open()`, and publishes input state/connection
  diagnostics for real-device validation instead of only relying on the message
  handler assignment.
- Asynchronous Web Audio resume and cleanup failures now queue into Orbifold's
  browser runtime and surface through the same visible error-status path as
  other device failures instead of remaining console-only messages.
- Browser audio output listing now checks whether Web Audio is available before
  advertising `Browser audio`, so browsers without `AudioContext` enter the same
  setup-required no-audio state as native hosts with no output device.
- Browser audio refresh now uses the browser's asynchronous device API when
  `AudioContext.setSinkId` is available, keeps a fallback `Browser audio` output
  when sink selection is unavailable, and reports asynchronous sink-selection
  failures through the visible Orbifold error status.
- The web smoke test now stubs named browser audio outputs and `setSinkId`, so
  CI exercises the named-output routing path instead of only proving the generic
  fallback `Browser audio` path.
- The web smoke test now checks the documented browser shortcut mapping table,
  including file/edit commands, transport keys, clip-editing keys, UI zoom,
  repeat behavior, and ignored Alt-modified chords.
- The Devices panel now shows browser-specific Web Audio and Web MIDI
  diagnostics on web, including scan counts, connection state, and sink-routing
  support, so real-device manual parity checks have visible evidence in the UI.
- Browser text edit actions now share the native handler for BPM, root note,
  base frequency, scale search, and asset search, so the web runtime no longer
  silently ignores Operad `TextEdit` actions for those controls.
- Browser tab titles now share the native project/dirty-state title formatter,
  so saved projects, untitled dirty work, and clean sessions report the same
  title text on native and web.
- Browser project-session restoration now preserves parse/load errors instead
  of overwriting them with a successful restored-session status, and failed
  browser project, Scala, and Lumatone opens no longer write a replacement
  session snapshot.
- Browser startup no longer overwrites invalid or unavailable stored settings
  with fallback defaults; defaults are still written when no browser settings
  exist yet.
- Browser startup success statuses for restored assets, restored sessions, and
  successful browser file loads now append to an existing error status instead
  of hiding the earlier startup failure.
- Browser project download now marks the project clean only after the browser
  download API call succeeds; a thrown or unavailable download path leaves the
  project dirty and reports a visible error.
- Browser asset restore now attempts IndexedDB even when old localStorage asset
  records are malformed, so corrupt legacy storage no longer blocks newer
  IndexedDB-backed assets from loading.
- Browser arrangement-ruler and piano-ruler seeking now keep an active drag
  capture just like native, so long playhead drags continue after the pointer
  leaves the original ruler hit target.
- The status bar now fits long path/device/status messages to the available
  footer width instead of letting them run beyond the bar.
- When startup or screenshot mode accumulates multiple status messages, the
  footer now shows the latest actionable status first and summarizes earlier
  messages with a count, instead of reading like a clipped event log.
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
- The workspace now has visible resize handles between the asset browser, clip
  panel, main editor, control panel, and piano roll, with persisted sizes and a
  right-panel layout reset action that also restores default left-browser
  visibility.
- Workspace resize handles now preserve the pointer grab offset while dragging
  and use larger hit areas, so resizing asset, clip, control, and piano-roll
  panels does not jump on the first drag update.
- Active note and workspace resize drags now keep receiving updates when the
  pointer crosses another pointer-edit surface or empty panel space, so dragging
  does not stop just because the cursor leaves the original hit target.
- Orbifold now has a regression test for the live-style resize case where the
  document rebuilds during an active drag and the pointer moves over a visible
  button; the active drag capture still receives the update.
- A no-window host-frame regression test now drives raw pointer down/move/up
  through Operad's document-frame action collection across multiple rebuilt
  documents, so long resize drags catch stale node-id capture failures.
- Workspace splitters now have stronger visible gutters and larger grab handles,
  and the splitter rendering code lives beside the workspace sizing math instead
  of in the native host module.
- The current clip color is now shared between the clip panel, arrangement
  surface, piano-roll notes, and velocity lane, so the same clip no longer
  changes identity between editors.
- The clip side panel can now be hidden from the piano-roll option panel, and
  that view preference persists, so the center editor and piano roll can reclaim
  the clip panel's horizontal space when the clip summary is not needed.
- When both Assets and Scales are shown, the left browser now has its own
  vertical resize handle, the split height is persisted, and taller asset
  browsers can show more rows instead of being capped at a fixed count.
- Asset-browser and scale-browser visibility are now saved as view preferences,
  so hiding either left-panel browser survives restart and does not dirty the
  project.
- The arrangement ruler and overview now use the same horizontal time viewport
  as the piano roll, so piano time zoom/scroll keeps the playhead, visible beat
  labels, note overview dots, and arrangement seek coordinates in sync.
- The piano roll now draws small time and pitch viewport indicators, so scroll
  and zoom state is visible instead of only being implied by changing ruler
  labels.
- Those piano-roll viewport indicators are now draggable: the time thumb scrolls
  the visible beat window, and the pitch thumb scrolls the visible row window.
- The arrangement and piano roll now draw explicit loop-start and loop-end
  boundary lines whenever those boundaries are inside the shared visible time
  viewport.
- The loop end can now be dragged in either the arrangement ruler or the
  piano-roll ruler to shorten or extend the loop length, including dragging past
  the current right edge to grow the loop.
- The piano-roll option panel now exposes snap as a visible toggle next to the
  grid value, so users can discover `Snap off`/`Snap on` without knowing the `G`
  shortcut.
- Long scale and asset lists now draw lightweight scroll indicators and expose
  small up/down controls tied to the visible row window, so overflow is
  discoverable and reachable even before full scroll containers exist.
- Those scale and asset browser scroll controls, plus mouse-wheel scrolling in
  tests, now move the visible list window without changing selection, so browsing
  a long list does not accidentally select or load the row under the scroll
  gesture.
- The top bar now has a `New` action. Dirty projects use a two-step
  `Discard?` confirmation before unsaved changes are thrown away.
- `Open` now uses the same dirty-project confirmation pattern before another
  project can replace unsaved work.
- Closing the window with unsaved changes now warns first and requires a second
  close request before quitting.
- `Esc` now cancels pending dirty-project discard confirmations.
- Native piano-roll hit testing, drag state, cursor selection, and action-name
  parsing now live in `ui/native/piano_interaction.rs`, keeping the host module
  focused on window/document orchestration while preserving the existing direct
  edit behavior.
- Status-bar presentation now lives with the other native presenter helpers
  instead of inside the Operad host module, reducing one more slice of
  formatting logic from `ui/native.rs`.
- Project location helpers now produce either the saved project directory or a
  compact unsaved location hint without clipping in the default layout.
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
- When an autosave exists, the left session strip exposes a `Recover` action
  above the asset browser, keeping recovery separate from asset management while
  loading the recovery file as an unsaved project.
- `Recover` and `Open Recent` can now coexist in the left session strip, so a
  stale or unwanted autosave does not hide the one-click path back to the last
  saved project.
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
- The control panel now exposes previous/next key-map preset buttons beside the
  active key-map label, so bundled Lumatone maps can be browsed without opening
  an OS file dialog.
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
- `docs/file_formats.md` now documents the current settings, project, autosave,
  temporary-save, backup, and legacy compatibility formats, with README linkage
  and regression coverage for the important markers.
- Settings load/save failures now include the settings path, so fallback-to-
  defaults startup and failed `Save Settings` reports identify the file involved.
- Startup no longer auto-persists fallback defaults over an unreadable or invalid
  settings file; the settings load error remains visible, and overwriting the
  file requires an explicit later settings save.
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
- The asset browser category tabs now use a two-row layout with full labels
  (`Samples`, `Instruments`, `Presets`, and `Impulses`) at the minimum layout,
  so the browser no longer depends on unexplained abbreviations.
- Pointer-through coverage now also includes the right-side control panel:
  scale root/base controls, metronome, UI zoom, waveform, and synth gain are
  clicked through the rendered document and verified against real state.
- The internal synth waveform row now uses previous/next buttons beside the
  current waveform label instead of a single opaque `Cycle` button.
- The synth output row now includes a `Reset` action that restores default synth
  settings through the same checked settings path as other synth edits.
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
- The 4K physical-density path now has a regression test tying a 3840x2160
  surface to the expected 1920x1080 logical layout and checking text-overlap
  safety at that derived size.
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
- The bottom status bar now includes the Cargo package version, giving testers a
  visible version string to report without adding another branded title to the
  main workspace.
- The repo now includes a minimal Linux desktop-entry file at
  `packaging/linux/orbifold.desktop`, with a regression test proving it launches
  the `orbifold` binary and uses the expected desktop metadata keys.
- The Linux packaging metadata now includes a matching scalable `orbifold.svg`
  icon under the hicolor app-icon path, with regression coverage tying the
  desktop file's `Icon=orbifold` entry to the packaged asset.
- Release-facing docs now include `CHANGELOG.md` and
  `docs/release_checklist.md`, with regression coverage that keeps the changelog
  tied to the current Cargo package version and the checklist tied to the
  required format, test, lint, screenshot, license, and desktop metadata gates.
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
- In the compact control panel, the two device-control rows now keep visible
  `MIDI` and `Audio` context labels instead of rendering as identical
  Refresh/Connect rows.
- The right panel now has a Devices mode with direct MIDI input and audio output
  picker rows, so users are not limited to blind previous/next cycling when
  choosing a device.
- When the Devices panel has more MIDI inputs or audio outputs than visible
  rows, the picker headings now expose previous/next controls that move the
  selected row and reveal hidden devices without leaving the panel.
- The right-panel device entry point now relabels itself to `Setup` when audio
  or MIDI setup is missing, and opening it reports the missing setup state in
  status text instead of a generic panel-open message.
- The right-panel mode buttons now keep stable labels while active: the active
  setup/device button stays `Setup` or `Devices`, and the active settings button
  stays `Settings`, instead of highlighting a `Control` label while another
  panel title is visible.
- The Devices panel now keeps that setup-required state visible inside the panel
  with a concise `SETUP REQUIRED` summary, so the warning does not disappear
  after the status bar changes.
- Setup-required Devices mode now uses compact MIDI and audio recovery sections,
  so both Refresh/Connect paths remain visible above the piano roll at the
  minimum supported layout.
- Error and startup-failure messages now feed a bounded diagnostic history, and
  the setup-required Devices panel shows the latest diagnostic when there is
  room. This gives failures such as audio stream errors a visible place to
  persist after the status bar changes.
- The Settings panel now exposes recent diagnostics when errors exist, keeping
  the normal settings layout uncluttered while giving non-setup failures a
  durable in-app place to be rediscovered.
- The Settings diagnostics section now includes a `Clear Diagnostics` action, so
  resolved startup/device/file errors do not remain as stale warnings forever.
- Startup now opens the right panel directly in Devices/Setup mode when audio or
  MIDI setup is incomplete, so the setup path is visible before the user hunts
  through the control panel.
- The README now documents the first-run Devices/Setup path, the fact that
  screenshot mode intentionally skips device probing, and the current workspace
  splitter behavior for browser, clip, right-panel, piano-roll, and browser
  section resizing.
- The Devices panel now also shows concise diagnostics for each backend,
  distinguishing live, disconnected, selected-but-not-connected, and empty scan
  states.
- Startup audio fallback status now explicitly says when a saved output is
  unavailable and Orbifold is trying the system default, rather than only
  reporting the missing saved device.
- The last-MIDI monitor now names common controller policy directly: sustain
  pedal events show `sustain on/off`; pitch bend and non-sustain CC messages
  say `ignored`, making the current synth policy visible instead of implying
  hidden modulation support.
- MIDI input now has a runtime channel filter exposed in the control panel. The
  last-event monitor still shows filtered-out events, but synth playback,
  held-note state, mapping capture, and recording ignore channels outside the
  selected filter. The selected filter is saved and restored with app settings.
- The MIDI channel filter row now has previous/next step buttons around the
  `All`/`Ch N` filter value, so users can move both directions instead of
  repeatedly cycling through all 17 filter states.
- The transport return-to-start button now says `Home`, matching the documented
  keyboard shortcut and avoiding the earlier ambiguity of placing `Start` beside
  `Play`.
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
  visible `Home` button. That action seeks to the loop start without stopping
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
- Clicking the piano-roll note ruler now auditions the pitch under the pointer
  without creating notes or moving the viewport; dragging the same ruler still
  pans/zooms the pitch view.
- Existing clip-note frequencies are now retuned when loading a different Scala
  scale or changing the root/base tuning, so recorded piano-roll notes keep
  following the active project tuning instead of preserving stale frequencies.
- Active playback voices and held/sustained MIDI voices now receive non-retrigger
  retune commands when the scale/root/base changes, so sounding notes follow the
  new tuning without resetting phase or amplitude.
- Selecting, adding, pasting, duplicating, nudging, resizing, quantizing, or
  transposing a clip note now scrolls the piano-roll time and pitch viewport just
  enough to keep the edited note visible, so successful command edits do not look
  like no-ops when the viewport was elsewhere.
- The piano-roll option panel now includes `Fit`, which recenters the visible
  time and pitch windows around clip notes, or resets to the root/default view
  when the clip is empty.
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
- The piano-roll option panel now has a pitch-label mode toggle: 12-TET can
  switch from note names to scale-degree/cents labels without dirtying the
  project.
- Ordinary MIDI keyboard input now maps 12-TET chromatic key positions to the
  nearest degree of the active tuning, so playing C-E-G in a 31-EDO project
  produces a recognizable triad instead of adjacent low-numbered 31-EDO degrees.
- Last-MIDI and capture-event labels now prioritize musician-readable event
  descriptions such as `note C4 (60)` or `cc64 value127` instead of presenting
  every incoming event as a raw note number. Remapped microtonal MIDI notes show
  the played key and tuned destination, such as `C5->d9 +310c`.
- Screenshot writing now performs a pixel-level smoke check before saving,
  rejecting malformed, blank, or obviously corner-cropped output. The check is
  covered by synthetic full-surface, blank, and corner-only image tests.
- No-probe startup now has a regression test that verifies screenshot-mode app
  construction creates no audio stream, audio output list, MIDI connection, or
  MIDI input list while preserving visible skip statuses.
- Windowless startup probing now has an integration test that runs
  `orbifold --startup-probe` with logging disabled and checks that raw ALSA/JACK
  backend diagnostics do not escape to stdout or stderr.
- Settings save failures now flow through the same logged error-status path as
  other recoverable errors, and the Save Settings control has regression coverage
  for both successful writes and write failures.
- Project save failures now have deterministic regression coverage proving the
  app reports the failed target path, keeps the project dirty, and does not adopt
  the failed path as the current project file.
- Project save also logs backup-removal and temp-cleanup failures instead of
  swallowing them, and the backup-failure path has regression coverage proving
  an existing project file is preserved when the save cannot safely rotate the
  previous backup.
- Project save now rotates three backup generations (`.bak`, `.bak.2`, and
  `.bak.3`) instead of overwriting a single backup file on every save.
- Audio command queue failures from All Off, synth setting changes, playback
  note-on/off updates, stale device refreshes, and related MIDI/asset failure
  paths now use the shared logged error-status path instead of writing unlogged
  status strings directly. Deterministic disconnected-audio-queue tests cover
  All Off, A4 test tone, and synth parameter failures without opening a window or
  audio device.
- MIDI input connection failures now preserve the backend error in the visible
  logged status instead of reducing it to a generic failed-connect message.
- Scale, Lumatone, audio, and MIDI labels now use real names or explicit error
  statuses instead of silently falling back to `Unknown`; unreadable MIDI port
  names are logged and skipped during device listing.
- MIDI connection now re-matches the selected visible device by live port name
  before opening it, so a stale list index cannot connect the wrong input after
  device enumeration changes.
- Failed user-triggered device selection/connect actions now use the shared
  logged diagnostic path instead of writing unlogged footer statuses.
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
- Asset imports now avoid overwriting existing library files by selecting a
  unique destination filename and reporting when a conflict caused a renamed
  import.
- `docs/asset_browser.md` now documents current asset folders, supported
  extensions, refresh behavior, import copying, conflict renaming, missing-file
  expectations, and the current no-preview/no-assignment limitations.
- `docs/asset_to_sound.md` now explains that the built-in synth is the current
  sound-producing path, that imported assets are library-only, and which
  sample/instrument/preset/impulse workflows are not implemented yet.
- Selecting an asset now reports the relevant library-only status in the footer,
  such as missing sample preview/assignment or instrument loading, instead of
  leaving users to infer why the selected asset does not affect sound.
- Asset footer statuses now avoid developer-facing `not implemented` wording and
  say plainly which sound workflows are unavailable yet.
- The asset browser now has a first real sound workflow: selected WAV samples can
  be previewed and stopped through the connected audio output. Unsupported
  formats and missing audio output report visible errors instead of silently
  doing nothing.
- `docs/architecture.md` now gives contributors a high-level map of startup,
  `AppState`, audio/synth threading, MIDI handling, project/settings files,
  Operad UI modules, action flow, and test locations.
- `docs/audio_midi_threading.md` now documents the UI/main thread, CPAL audio
  callback, midir callback, shared handles, command flow, device lifecycle,
  failure policy, and rules for adding audio, MIDI, and recording work.
- `docs/operad_integration.md` now documents the native Operad host, frame
  lifecycle, document construction, ordinary controls, custom editor surfaces,
  action dispatch, focus/cursor behavior, screenshot mode, and UI test
  expectations.
- Project save now reports autosave cleanup failures in the visible saved status
  and only exposes autosave recovery when the autosave path is a real file, not
  a stale directory or other non-recoverable filesystem entry.
- Note editing, key-map loading, loop-length changes, and common
  transport-setting status updates now preserve an autosave/persistence error
  instead of replacing it with a normal success label such as `Added note`,
  `Metronome on`, or `Loop length 8 beats`.
- Scale-browser rows now identify whether a scale is bundled or show the user
  folder it came from, so the scale list no longer hides source/location context
  behind bare names.
- The scale browser now has the same kind of search/filter path as the asset
  browser, with a clear action and filtered list navigation for larger tuning
  libraries.
- The active scale card now shows compact equal-division metadata when the scale
  is an EDO/TET-style division and previews the first scale intervals in cents.
- Base frequency now has a numeric Hz input in the control panel; committing a
  typed value uses the same retune path as the stepper buttons.
- Root selection now has a text input in the control panel. It accepts note names
  such as `C4`, accidentals such as `Bb3`, or raw MIDI numbers such as `60`,
  then retunes existing and sounding notes through the shared root-change path.
- The scale browser now has a direct `Import` action wired to the non-blocking
  Scala file dialog, so importing scales is available beside Load, Refresh, and
  Remove instead of only through the top bar.
- Orbifold is pinned to the pushed Operad `codex/v8-roadmap` commit
  `a517853440f5f8f78b4de5b9f500556501285a5a` and handles the new
  `ScenePrimitive::MorphPolygon` primitive in its local piano-roll scene
  translation path.

Remaining caveat: normal interactive startup still probes the host audio stack.
Linux ALSA/JACK diagnostics are routed through Orbifold logging on the covered
startup-probe path, but broader hardware and distro coverage is still needed.

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

The latest screenshot is no longer a fake DAW mockup: it shows real left-browser
state, a single `Current Clip` lane, and an empty arrangement that says `Empty
clip`. It also now fits the full workspace at the captured 2x scale:
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
- Linux ALSA/JACK diagnostics are now routed through logging on the covered
  startup-probe path instead of raw terminal output, but broader host coverage is
  still needed.
- A basic device setup state and diagnostics panel now exist, but the workflow is
  still not a full preferences surface with durable troubleshooting history.
- Fatal graphics/window/renderer errors still prevent the UI from appearing.

Missing:

- Richer structured status for missing audio and missing MIDI beyond the current
  compact setup summary.
- A visible diagnostics panel or status dialog for startup failures.
- A distinction between "audio unavailable", "MIDI unavailable", "settings
  invalid", and "project failed to load" across the whole app, not just the
  device panel.
- More hardware coverage for quiet audio probing across Linux audio stacks.

Minimum acceptable behavior:

- If audio fails, Orbifold launches muted and says why. The basic version of this
  now exists.
- If MIDI fails, Orbifold launches and shows "No MIDI input connected".
- The A4 test button is disabled or reports a useful error when audio is not
  available. The basic version of this now exists.
- The UI offers refresh and reconnect without requiring restart. The Devices
  panel now covers the basic version of this for audio and MIDI.

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
- More complete collapsible side-panel policies beyond the current persisted
  splitters.
- Stable panel minimum sizes.
- Full scroll containers where content can exceed available space; left-browser
  lists have visible overflow indicators and small up/down controls, but still
  use selection movement rather than true scroll state.
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
- BPM now has an editable numeric field between the self-describing `BPM -` /
  `BPM +` buttons, though it is still a compact prototype input rather than a
  mature tempo-control component.
- Compact top-bar quantize still cycles through values, and the direct
  previous/next grid buttons are still not a full menu or dropdown.
- Compact device rows still use prev/next buttons, while the Devices panel now
  has direct picker rows with navigation for hidden rows.
- The top-bar preferences action now uses `Prefs` at minimum width, `Save Pref`
  at normal desktop width, and `Save Settings` when the wide layout has room;
  there is still no full settings window.

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
- Dedicated arrangement scroll and zoom state, if arrangement and piano time
  views should eventually diverge.
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
- Dedicated arrangement-specific horizontal zoom controls if the current shared
  piano/arrangement time viewport is not enough.
- Dedicated arrangement-specific horizontal scroll controls if the current
  shared piano/arrangement time viewport is not enough.
- Scroll vertically.
- Show loop region. Basic loop-start and loop-end boundary lines now render;
  shaded/editable loop-region controls are still missing.
- Set loop region. Loop end is now draggable from the arrangement and piano-roll
  rulers; loop start remains fixed at beat 1 and shaded loop-region editing is
  still missing.
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
- Project loads now resolve relative scale, key-map, and sample references from
  the project file's directory, and saving writes project-local references back
  as relative paths so moved project folders remain portable.

Missing:

- A full recent-project browser with multiple choices, missing-file handling,
  and project management actions.
- Configurable autosave interval and storage location.
- Crash-recovery polish beyond the current manual Recover action.
- Clear error messages for failed save/open.
- Confirmation for destructive operations.

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
- The UI exposes audio through compact prev/next/refresh/connect controls and a
  right-panel Devices mode with direct picker rows and concise diagnostics.
- There is no full device preferences workflow.

Missing:

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
- The right-panel Devices mode exposes direct MIDI input picker rows.

Missing:

- Hotplug refresh behavior.
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

- Remove scale.
- Rename/favorite scale.
- `.kbm` keyboard mapping support, or an explicit decision to use Lumatone maps
  instead.
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
- Proper controls for gain, attack, release, filter, delay, and drive.
- Parameter ranges and units shown clearly.
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

- The old always-visible project panel has been removed from the default
  surface.
- Recovery and recent-project actions now live in the left session strip only
  when autosave or recent state exists.
- The session strip can recover autosaves, open a visible recent-project row,
  and forget stale recent state without deleting project files.
- Missing recent project files are not opened and can be forgotten.
- This is still a lightweight session shortcut, not a full project browser.

Missing:

- Duplicate project.
- Rename project.
- Delete project files.
- Search.
- Rich project metadata and file-management actions.
- Rich missing-project recovery actions beyond forgetting stale entries.

Minimum useful behavior:

- The compact session strip is acceptable for the prototype. A full browser
  should not return until it is backed by richer recent-project data and project
  actions.

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
- A richer track/clip color system once there is more than one real track.
- A fuller status/event history surface beyond the one-line footer.
- Better empty states in secondary panels. The empty clip path now reports
  itself in the arrangement and clip controls, while the piano-roll grid stays
  clear of large placeholder text.
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

- Polished final app icon.
- Bundle/package format.
- About dialog.
- License display.
- Crash/error reporting story.
- Default directories.
- App data directory policy.
- Config directory policy.
- Project file association.
- Installer or archive packaging.
- GitHub repo setup and tags, if not already done.

Minimum useful alpha behavior:

- A tester can download/build, launch, find settings/projects, and report the app
  version.

## Documentation Gaps

Basic user docs now covered by `docs/first_run.md`:

- First run.
- Audio setup.
- MIDI setup.
- Loading scales.
- Recording.
- Editing notes.
- Saving/opening projects.
- Troubleshooting.

Deeper hardware/file troubleshooting is now covered by `docs/troubleshooting.md`:

- Startup probe and logging.
- Audio setup failures.
- MIDI setup failures.
- Settings parse/save issues.
- Project/autosave recovery issues.

Lumatone setup docs are now covered by `docs/lumatone_setup.md`:

- Scale versus key-map behavior.
- Factory `.ltn` presets.
- User key-map loading.
- MIDI/channel-filter setup.
- Mapping capture limitations.

Asset browser/import docs are now covered by `docs/asset_browser.md`:

- Asset folders and supported extensions.
- Refresh and stale-selection behavior.
- Import copy and conflict-renaming behavior.
- Current no-preview/no-assignment limitations.

Asset-to-sound workflow docs are now covered by `docs/asset_to_sound.md`:

- Current built-in synth sound path.
- Asset library-only status by category.
- Status-bar feedback when selected assets cannot affect sound yet.
- Target sample, instrument, preset, impulse, and missing-asset workflows that
  are not implemented yet.

Known limitations are now covered by `docs/known_limitations.md`:

- Single-clip and not-yet-real multi-track arrangement limits.
- Piano-roll, audio/MIDI, and asset workflow limits.
- Scale/key-map validation and Lumatone hardware programming limits.
- Project asset packaging, UI/help, release packaging, and screenshot reporting.

Keyboard shortcut docs are now covered by `docs/keyboard_shortcuts.md`:

- Full shortcut categories.
- Focus and discoverability.
- Modifier and key-repeat policy.
- Workflow examples for recording, editing, view zoom, undo/redo, copy/paste,
  and safe project save/open behavior.

Advanced Lumatone troubleshooting docs are now covered by
`docs/lumatone_troubleshooting.md`:

- Manual scale/key-map validation.
- Wrong-note and silent-input diagnosis.
- Channel-filter and mapping-capture checks.
- `.ltn` parse error interpretation.
- Bug-report evidence for Lumatone issues.

Still missing or too thin:

- No remaining user-doc items are listed in this section. Developer docs below
  are still thin.

Missing developer docs:

- No remaining developer-doc items are listed in this section.

Current documentation problem:

- The README now links tester-focused guides and a basic architecture overview,
  and developer docs now cover threading, Operad integration, UI testing,
  release flow, and command/action patterns.

## Data Safety Gaps

Missing:

- Autosave recovery.
- Backup rotation and cleanup policy.
- Confirm before destructive operations.
- Distinguish remove-from-library vs delete-from-disk.
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
3. Replace the current global scaling behavior with a responsive density policy.
4. Add visual screenshot review to the normal development checklist.
5. Keep splitting `src/ui/native.rs` before it becomes too costly to change.
6. Add saved-baseline tracking, autosave, backup, and recovery on top of the
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
9. Turn the current Devices picker panel into a fuller device preferences
   workflow.
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
