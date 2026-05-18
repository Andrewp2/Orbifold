# Web Parity Audit

Use this audit before saying the browser build has desktop parity. The goal is
not to prove every future DAW feature exists on both targets; it is to prove
that every workflow currently advertised as usable on desktop has a matching
browser path or an explicit limitation.

## Parity Definition

A web parity claim requires evidence for all of these areas:

- The generated Pages artifact loads the live wasm UI, not only the static
  fallback shell.
- The browser build uses the shared `AppState`, project model, action dispatch,
  Operad document builder, and piano-roll interaction logic.
- Browser file flows can open and save `.orbifold` projects, Scala `.scl`
  scales, Lumatone `.ltn` key maps, and supported WAV assets.
- Browser dirty-project confirmation matches native before replacing work.
- Browser settings, the latest browser project session, browser-loaded text
  resources, and browser-imported asset bytes survive a page reload.
- Browser transport, note editing, piano-roll wheel gestures, timeline seeking,
  loop boundary dragging, and workspace resizing produce the same model changes
  as native.
- Browser keyboard shortcuts route through the same action names as native, with
  browser-specific file/device behavior where required.
- Browser Web MIDI input reaches the same shared MIDI handling path as native
  MIDI input.
- Browser Web Audio creates the shared synth-backed audio stream and reports
  recoverable errors visibly.
- The canvas is correctly sized at normal and high-DPI device pixel ratios.
- The deployed GitHub Pages site serves the same artifact that passed local
  build and smoke checks.

If any item above is untested, document it as an open validation gap instead of
claiming complete parity.

## Automated Evidence

Run these from the repo root:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
./scripts/build-web.sh dist
./scripts/check-web-dist.mjs dist
python3 -m http.server 4173 --directory dist
./scripts/check-web-layout.mjs http://127.0.0.1:4173/
./scripts/check-web-smoke.mjs http://127.0.0.1:4173/
./scripts/check-web-live.mjs https://<user>.github.io/<repo>/
./scripts/check-web-layout.mjs https://<user>.github.io/<repo>/
./scripts/check-web-smoke.mjs https://<user>.github.io/<repo>/
./scripts/capture-web-visuals.mjs https://<user>.github.io/<repo>/
./scripts/check-web-manual-devices.mjs https://<user>.github.io/<repo>/
./scripts/check-web-manual-report.mjs reports/
./scripts/check-web-parity-gate.mjs https://<user>.github.io/<repo>/ --report reports/
```

The headless smoke currently covers:

- wasm startup through the first Orbifold frame readiness signal,
- WebGPU availability and high-DPI canvas backing-store scaling,
- ordinary toolbar clicks through the canvas hit-testing path,
- action dispatch, the documented browser shortcut mapping table, representative
  keyboard shortcut dispatch through the live canvas, actual shortcut-driven
  note nudge/length/pitch/velocity edits, and ignored Alt/repeat shortcut cases,
- piano-roll double-click creation, note drag, edge resize, velocity-handle
  drag, wheel pan/zoom, arrangement/piano-ruler seeking, loop-end dragging, and
  workspace splitter drags,
- shared clip edit actions for copy, paste, duplicate, delete, snap toggle, and
  quantizing an off-grid selected note,
- browser project save/download, keyboard-driven Save As download, project open
  through real browser file inputs, and visible error-preserving rejection for
  malformed browser project, Scala, key-map, and asset imports,
- dirty-project open confirmation before creating a browser file input,
- browser tab title updates for dirty and loaded project states,
- browser startup rejection for invalid saved settings/project storage without
  overwriting the bad browser storage before the user can recover it,
- Scala, Lumatone, and WAV asset imports through real browser file inputs, plus
  assigning the imported WAV as the project sample instrument,
- browser reload persistence for the saved project session, browser-loaded
  Scala/key-map resources, imported sample instrument, imported assets, and
  panel-visibility settings,
- browser UI-scale action persistence and runtime reload,
- deterministic Web MIDI refresh, explicit `MIDIInput.open()` connection when
  available, note-on, note-off, and recording through a browser MIDI stub, plus
  visible recovery states for browsers where Web MIDI is unavailable or
  permission is denied,
- Web Audio output discovery through the browser device APIs when sink
  selection is available, named-output routing through a browser audio-output
  stub, fallback `Browser audio` discovery when it is not, audio-context
  creation, default-output connection, processor attachment, sink-selection
  request/error surfacing, resume request, Orbifold audio connected state,
  callback activity, rendered frame count, nonzero A4 test-tone samples, and
  browser device diagnostics
  that report Web Audio/Web MIDI support, scan counts, connection state, and
  sink-routing state.

The `tests/web_pages.rs` integration tests assert that the web shell, build
script, Pages workflow, smoke script, runtime bridges, persistence paths, and
browser-specific action routes keep those checks in place.

The artifact check verifies that `dist/index.html`, `dist/pkg/orbifold_web.js`,
`dist/pkg/orbifold_web_bg.wasm`, favicon assets, `.nojekyll`, relative asset
references, and runtime-ready/fallback hooks are present before the Pages
workflow uploads the artifact.

The live-site check fetches the deployed Pages URL plus the wasm loader, wasm
binary, and favicon assets. It catches stale deploys, missing assets, broken
relative paths, and fallback-only uploads. The Pages workflow also runs the
headless smoke against the deployed URL after deployment, which verifies the
published wasm runtime path in CI. Manual browser/device checks are still needed
for audio output, real Web MIDI hardware, and visual inspection.

The web layout check launches headless Chrome at compact, desktop, high-DPI, and
4K viewports. It verifies that the wasm runtime replaces the fallback shell, the
canvas client and backing-store sizes fill the viewport at the expected device
pixel ratio, the document does not overflow/scroll, key editor geometry such as
the piano grid, piano roll, and right panel stays usable, and Orbifold's
estimated rendered text boxes report no overlap or invalid layout. This catches
automatable layout regressions like the UI rendering only in the top-left
quarter of the window or obvious text collisions, but it is still not a
substitute for human visual review.

The visual capture script launches headless Chrome with WebGPU enabled, waits
for the live runtime to render at compact, desktop, high-DPI, and 4K viewports,
then writes visual artifacts and a manifest under `screenshots/web/`. It writes
PNG screenshots when Chrome can capture the WebGPU surface; when headless Chrome
returns transparent screenshots, it falls back to an SVG paint snapshot exported
by the live wasm runtime for that frame. The manifest records each browser
screenshot attempt and why it was accepted or rejected. The capture fails if any
viewport records a JavaScript exception, console error/assertion, network load
failure, or browser log error. Inspect the artifacts yourself; this is visual
evidence, not a substitute for a human layout pass.
The Pages workflow uploads visual artifacts for both the local build artifact
and the deployed Pages URL so review evidence survives CI.

## Manual Evidence

Automated checks do not cover all browser parity risks. Record results for
these manual checks before treating web as parity-complete:

- Open `dist/` locally in a WebGPU-capable browser and confirm the live UI
  replaces the fallback shell.
- Open the deployed GitHub Pages URL after the Pages workflow completes and
  run `./scripts/check-web-live.mjs` against it, then confirm it serves the live
  wasm UI and current favicon/assets.
- Run `./scripts/check-web-smoke.mjs` against the deployed URL when you need
  local confirmation of the same runtime gate the Pages workflow uses.
- Run `./scripts/check-web-layout.mjs` against the deployed URL to confirm
  multi-viewport canvas coverage, high-DPI backing size, no page overflow, and
  non-collapsed editor geometry.
- Run the browser UI on a high-DPI or 4K display and inspect layout scale, text
  overlap, piano-roll labels, panel resize handles, and canvas coverage.
- Run `./scripts/capture-web-visuals.mjs` against the deployed Pages URL and
  inspect the compact, desktop, high-DPI, and 4K PNG or SVG artifacts it writes,
  or inspect the deployed visual artifact uploaded by the Pages workflow.
- Run `./scripts/check-web-manual-devices.mjs` against the deployed Pages URL
  with a real audio output and Web MIDI device attached. Keep the generated
  `reports/web-manual-devices-*.json` artifact with the release evidence. The
  report records a hash fingerprint for the deployed HTML, wasm loader, wasm
  binary, and icon files at the time of manual testing.
- Run `./scripts/check-web-manual-report.mjs reports/` and keep the validator
  output with the manual report. A report only counts when every required check,
  user confirmation, real browser click, audible Web Audio sample, real MIDI
  input, MIDI recording evidence field, and deployed artifact fingerprint passes
  validation. The validator also requires the verifier's host, Chrome, real
  click-coordinate, and timestamp metadata, and rejects reports that recorded
  browser runtime exceptions, console errors/assertions, network load failures,
  or browser log errors, so unchecked or partial JSON cannot stand in for the
  browser session.
- Run `./scripts/check-web-parity-gate.mjs` against the deployed Pages URL with
  the validated manual report. This wraps the deployed live/layout/smoke checks,
  visual capture, manual report validation, and live-vs-manual artifact
  fingerprint comparison into one final pass/fail report under `reports/`.
- Use a real browser file picker to open/save projects, scales, key maps, and
  assets, then reload and confirm the same state restores.
- Grant Web MIDI permission in a browser that supports Web MIDI, connect a real
  MIDI device, and confirm note-on/note-off updates status and playback through
  the same path as native. Treat this as the real MIDI device check; the
  deterministic smoke stub is not enough for hardware parity. Confirm the
  Devices panel reports a Web MIDI diagnostic such as permission/input/connection
  state while you test.
- Connect a real audio output, click the browser audio connect path, play A4 or
  a short clip, and confirm audible output plus visible error reporting when the
  output is unavailable. Confirm the Devices panel reports whether Web Audio can
  select sinks or is limited to the default browser sink.
- Compare native and browser keyboard shortcuts for transport, editing, file
  commands, help, and UI zoom.
- Compare native and browser piano-roll workflows: create, select, move, resize,
  velocity edit, nudge, transpose, quantize, delete, scroll, zoom, seek, and
  drag loop boundaries.

Use `docs/manual_qa_checklist.md` for the longer workflow checklist. Capture the
browser name, version, operating system, hardware connected, Pages URL, commands
run, and any skipped checks.

## Non-Parity Signals

The following are useful signals, but do not prove full web parity by
themselves:

- `./scripts/build-web.sh dist` succeeds.
- The static fallback shell displays.
- Headless Chrome smoke passes without a manual browser/device pass.
- A CDP screenshot is nonblank or an SVG paint snapshot exists. The SVG fallback
  is generated from Orbifold's paint list and can miss renderer-specific GPU
  defects.
- Web MIDI passes only with the deterministic mock, without a real device.
- Web Audio reports an attached processor and nonzero generated samples, without
  confirming audible output in a real browser session.
- `scripts/check-web-manual-devices.mjs` exists, without a passing report from a
  real browser/audio/MIDI session.
- A manual-device report exists, without a passing
  `./scripts/check-web-manual-report.mjs` validation run.
- A manual-device report validates but was generated against an older deployed
  artifact than the one currently served by Pages.
- Individual deployed or manual checks pass, without a passing
  `./scripts/check-web-parity-gate.mjs` run that ties the evidence together.
- The Pages workflow file exists, without a successful deployed Pages run.
- `scripts/check-web-live.mjs` passes without a manual browser runtime check.
- `scripts/check-web-layout.mjs` passes without visual inspection; it checks
  measurable geometry and estimated text overlap, not whether the rendered DAW
  surface is aesthetically or ergonomically correct.

Treat these as partial evidence and keep the open gap visible in the handoff.
