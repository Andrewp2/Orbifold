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
- Missing recent project files are marked in the left session strip and can be
  forgotten without restarting
- Project save/load for the current scale, Lumatone mapping, synth settings,
  transport, and clip
- Dirty project autosave to `orbifold_autosave.orbifold`, cleared after a
  successful save or return to a clean project state
- Window title reflects the open project name and unsaved state
- Resizable workspace splitters for the left browser, clip panel, right panel,
  piano roll, and the Assets/Scales browser split, with layout reset and
  persisted panel proportions
- Right-panel Settings mode for UI zoom, workspace visibility, layout reset,
  settings save, and device setup access
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
- If audio or MIDI setup is missing, the right panel opens directly in
  Devices/Setup mode with a `SETUP REQUIRED` summary instead of leaving the
  setup path hidden in the control panel.
- Visual layout is actively changing as the Operad integration matures.

See `docs/known_limitations.md` for the fuller tester-facing limitation list.

## Web Build

Orbifold has a browser build for GitHub Pages:

```sh
./scripts/build-web.sh dist
./scripts/check-web-dist.mjs dist
python3 -m http.server 4173 --directory dist
./scripts/check-web-layout.mjs http://127.0.0.1:4173/
./scripts/check-web-smoke.mjs http://127.0.0.1:4173/
./scripts/check-web-live.mjs https://<user>.github.io/<repo>/
./scripts/check-web-layout.mjs https://<user>.github.io/<repo>/
./scripts/capture-web-visuals.mjs https://<user>.github.io/<repo>/
./scripts/check-web-manual-devices.mjs https://<user>.github.io/<repo>/
./scripts/check-web-manual-report.mjs reports/
./scripts/check-web-parity-gate.mjs https://<user>.github.io/<repo>/ --report reports/
./scripts/check-web-parity-complete.mjs reports/
```

The layout check launches headless Chrome at compact, desktop, high-DPI, and 4K
viewports and verifies canvas coverage, backing-store size, no document
overflow, non-collapsed editor geometry, and Orbifold's estimated rendered text
boxes for overlap or invalid layout.

The smoke check launches headless Chrome with WebGPU enabled and fails on
browser exceptions, console errors, failed asset loads, missing WebGPU, a
runtime that does not reach Orbifold's first-frame readiness signal, or browser
actions/keyboard shortcuts/piano-roll pointer and wheel gestures that cannot
create, drag, resize, scroll, zoom, persist clip notes, and resize the main
workspace panels. It clicks ordinary toolbar buttons through the canvas hit
testing path. It also exercises browser save/open file flows plus Scala,
Lumatone key-map, and WAV asset imports through real browser file inputs, then
uses the imported browser WAV as the project sample instrument. It verifies
dirty-open confirmation before the browser picker appears and browser title
updates for dirty and loaded project states. It verifies
Web MIDI refresh/connect, note-on, note-off, and recording through a
deterministic browser MIDI stub. Web Audio coverage verifies browser output
discovery, context creation, processor attachment, resume request, Orbifold's
connected audio state, and nonzero samples from the browser A4 test-tone path.
The same smoke run reloads the browser page and verifies that the saved project
session, browser-loaded Scala/key-map resources, browser-imported sample
instrument, browser-imported asset, persisted panel-visibility settings, and the
web UI-scale reload path restore from browser storage. It also resizes Chrome to
a high-DPI viewport to catch canvas scaling regressions.
The visual capture script writes compact, desktop, high-DPI, and 4K browser
visual artifacts plus a manifest under `screenshots/web/` for manual
inspection. When headless Chrome returns transparent WebGPU screenshots, it
falls back to an SVG paint snapshot exported by the live wasm runtime and
records the browser screenshot attempts in the manifest.
The Pages workflow captures and uploads the same visual artifact set for both
the local build artifact and the deployed Pages URL.
Use `docs/web_parity_audit.md` before claiming browser parity; it separates the
automated evidence above from manual checks that require a real browser,
deployed Pages site, audio output, and Web MIDI hardware. The manual-device
script opens a real Chrome session, prompts for audible Web Audio and hardware
Web MIDI confirmation, records the deployed artifact fingerprint, and writes a
JSON report under `reports/`. Validate that report with
`./scripts/check-web-manual-report.mjs reports/` before treating the manual
device pass as release evidence.
After the manual report exists, `./scripts/check-web-parity-gate.mjs` runs the
deployed live/layout/smoke checks, captures deployed visuals, validates the
manual report, rejects stale reports whose artifact fingerprint no longer
matches the live Pages site, and writes a final gate report under `reports/`.
Use `./scripts/check-web-parity-complete.mjs reports/` as the final saved
evidence check; it requires a passing gate report, a validated manual-device
report, non-skipped visual capture, and matching manual/live artifact
fingerprints.

## Music Workflow

For a tester-oriented walkthrough of first launch, device setup, tuning, note
editing, saving, recovery, display sizing, and troubleshooting, see
`docs/first_run.md`. For deeper audio, MIDI, settings, project, and autosave
diagnostics, see `docs/troubleshooting.md`.

1. Launch the app with `cargo run`.
2. If the right panel opens to `DEVICES` with `SETUP REQUIRED`, use the MIDI
   INPUTS and AUDIO OUTPUTS sections to refresh, select, and connect devices.
3. Click `Control` after setup to return to synth, tuning, and recording
   controls.
4. Use `Ch All` in the control panel to cycle a MIDI channel filter when you
   want Orbifold to respond to only one incoming channel. The selected filter is
   saved with the rest of the app settings.
5. Use `Mute` in the control panel when you need to silence output without
   changing the saved master gain.
6. Load a Scala scale and Lumatone `.ltn` preset, or use the current saved
   defaults. See `docs/lumatone_setup.md` for the current scale/key-map
   workflow and limitations, and `docs/lumatone_troubleshooting.md` for manual
   validation and wrong-note diagnosis.
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

The workspace layout can be adjusted while working. Drag the vertical splitters
between the browser, clip panel, editor, and right panel to rebalance
horizontal space; drag the splitter above the piano roll to give the editor more
or less height. When both `Assets` and `Scales` are visible in the left browser,
their divider can also be dragged. Use the right-panel layout reset action to
return to default proportions.

The right panel can switch between Control, Devices/Setup, and Settings. Use
Settings for UI zoom, browser/clip visibility, layout reset, and saving
preferences.

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

The browser is currently a library surface with WAV sample preview and project
sample-instrument assignment. Full sampler/instrument management is still part
of the usability backlog. See `docs/asset_browser.md` for supported file types,
preview limits, import behavior, conflict renaming, and current limitations. See
`docs/asset_to_sound.md` for the current built-in-synth sound path and asset
workflow limits.

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

Build the current browser app with:

```sh
./scripts/build-web.sh dist
```

The web build compiles the shared Orbifold app state and action dispatch into
wasm through Operad's web runtime. Browser project open/save uses the same
`.orbifold` text format through browser file APIs, and Scala `.scl` plus
Lumatone `.ltn` loading use the same parsers as desktop. Browser MIDI input uses
Web MIDI in supported browsers and feeds the same MIDI handling path as desktop.
Browser audio uses Web Audio and the shared synth engine. Browser settings reuse
the same plain-text settings format in `localStorage`, and the browser keeps the
latest project session in `localStorage` so reloads can restore the current
`.orbifold` state. The browser surface uses the shared Operad document builder,
including piano-roll note editing, panel splitters, loop-end drags, viewport
thumb drags, timeline ruler seeking, and piano-roll wheel navigation. Browser
asset import can load WAV samples for preview and sample-instrument use, persists
browser-imported asset bytes in IndexedDB with a legacy `localStorage`
migration/merge/fallback, and restores them before the saved browser project
session so sample instruments survive reloads within browser storage quota.
Browser keyboard shortcuts route
file, scale, key-map, asset, and MIDI actions through the same browser APIs as
pointer clicks, and dirty project confirmation is preserved before replacing
the current browser project. Browser text edits for BPM, root, base frequency,
scale search, and asset search use the same shared command handling as native.
The browser tab title uses the same project name and dirty-state formatting as
the native window title. Web UI-scale actions persist the new setting and reload
the page so the fixed Operad web runtime scale is reapplied on startup.
`web/index.html` keeps a static DOM fallback for browsers where WebGPU cannot
start or does not report startup completion promptly. The generated site lives
under `dist/` and is deployed by `.github/workflows/pages.yml`.

Developer notes for the current plain-text settings, project, backup, and
autosave files live in `docs/file_formats.md`.

The current codebase architecture overview lives in `docs/architecture.md`.
The audio/MIDI threading model lives in `docs/audio_midi_threading.md`.
The Operad integration model lives in `docs/operad_integration.md`.
The UI testing workflow lives in `docs/ui_testing_workflow.md`.
The web parity audit lives in `docs/web_parity_audit.md`.
Guides for adding UI controls/actions and project commands live in
`docs/add_ui_control.md` and `docs/add_project_command.md`.

The current prototype limitation list lives in `docs/known_limitations.md`.

## Packaging Metadata

Linux desktop metadata lives at `packaging/linux/orbifold.desktop`. It is a
minimal launcher template for packaged installs and expects the installed binary
and icon to both be named `orbifold`. A matching scalable icon lives under
`packaging/linux/icons/hicolor/scalable/apps/`, the 64px PNG app icon lives under
`packaging/linux/icons/hicolor/64x64/apps/`, and the browser-style favicon is
`favicon.ico`.

The GitHub Pages shell lives in `web/index.html`. The wasm entry point is
`examples/orbifold_web.rs`, and `scripts/build-web.sh` copies the generated
`pkg/` output plus the favicon assets into `dist/`. The build also writes
`dist/.nojekyll`, and `scripts/check-web-dist.mjs` verifies the Pages artifact
contains the wasm loader, wasm binary, icons, relative asset references, and
runtime-ready/fallback hooks. `scripts/check-web-layout.mjs` verifies the live
canvas and editor geometry across compact, desktop, high-DPI, and 4K browser
viewports. After deployment, `scripts/check-web-live.mjs` performs the same
artifact-shape check against the published Pages URL.

Release notes and release checks live in `CHANGELOG.md` and
`docs/release_checklist.md`. The release workflow lives in
`docs/release_workflow.md`.

## Visual QA

Use screenshot mode to capture the current Operad surface:

```sh
cargo run -- --screenshot
cargo run -- --screenshot-size=1200x760
```

Screenshots are written to `screenshots/` as timestamped PNGs, and
`screenshots/latest.png` is updated for quick inspection. Screenshot mode skips
audio and MIDI hardware probing so visual QA can run on machines without working
devices, avoids saving device settings from that no-probe startup, and may
intentionally show the right panel in Devices/Setup mode because setup is
incomplete. Use `--screenshot-size=WIDTHxHEIGHT` to capture a specific output
image size, such as the minimum supported layout or a 4K monitor check.

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
./scripts/capture-web-visuals.mjs https://andrewp2.github.io/Orbifold/
```

Then inspect `screenshots/latest.png` and the latest `screenshots/web/` run.

## Error Handling

Errors should be handled according to whether the app can still do useful work.

- Graphics/device initialization failures may still be unrecoverable.
- Missing audio or MIDI hardware should leave the UI usable with a visible status.
- Recoverable runtime failures should be shown in the UI status area.
- Parsers should reject malformed input instead of substituting hidden defaults.
- Dirty project edits are written to `orbifold_autosave.orbifold` as a basic
  recovery file. If that file exists on startup, the left session strip shows
  a `Recover` action that loads it as an unsaved project and a `Dismiss` action
  that removes stale recovery files from a clean project.

## Usability Backlog

The current usability gap inventory lives at
`docs/orbifold_usability_gap_analysis.md`.
