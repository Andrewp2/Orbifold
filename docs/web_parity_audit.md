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
./scripts/check-web-smoke.mjs http://127.0.0.1:4173/
./scripts/check-web-live.mjs https://<user>.github.io/<repo>/
./scripts/check-web-smoke.mjs https://<user>.github.io/<repo>/
./scripts/capture-web-visuals.mjs https://<user>.github.io/<repo>/
```

The headless smoke currently covers:

- wasm startup through the first Orbifold frame readiness signal,
- WebGPU availability and high-DPI canvas backing-store scaling,
- ordinary toolbar clicks through the canvas hit-testing path,
- action dispatch and keyboard shortcut dispatch,
- piano-roll double-click creation, note drag, edge resize, wheel pan/zoom, and
  workspace splitter drags,
- browser project save/download and project open through real browser file
  inputs,
- dirty-project open confirmation before creating a browser file input,
- browser tab title updates for dirty and loaded project states,
- Scala, Lumatone, and WAV asset imports through real browser file inputs, plus
  assigning the imported WAV as the project sample instrument,
- browser reload persistence for the saved project session, browser-loaded
  Scala/key-map resources, imported sample instrument, imported assets, and
  panel-visibility settings,
- browser UI-scale action persistence and runtime reload,
- deterministic Web MIDI refresh, connect, note-on, note-off, and recording
  through a browser MIDI stub,
- Web Audio output discovery, audio-context creation, processor attachment,
  resume request, Orbifold audio connected state, callback activity, rendered
  frame count, and nonzero A4 test-tone samples.

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

The visual capture script launches headless Chrome with WebGPU enabled, waits
for the live runtime to render at compact, desktop, high-DPI, and 4K viewports,
then writes PNGs and a manifest under `screenshots/web/`. It catches fallback
startup, canvas coverage regressions, and blank headless captures before
review. Inspect the images yourself; this is visual evidence, not a substitute
for a human layout pass.

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
- Run the browser UI on a high-DPI or 4K display and inspect layout scale, text
  overlap, piano-roll labels, panel resize handles, and canvas coverage.
- Run `./scripts/capture-web-visuals.mjs` against the deployed Pages URL and
  inspect the compact, desktop, high-DPI, and 4K PNGs it writes.
- Use a real browser file picker to open/save projects, scales, key maps, and
  assets, then reload and confirm the same state restores.
- Grant Web MIDI permission in a browser that supports Web MIDI, connect a real
  MIDI device, and confirm note-on/note-off updates status and playback through
  the same path as native. Treat this as the real MIDI device check; the
  deterministic smoke stub is not enough for hardware parity.
- Connect a real audio output, click the browser audio connect path, play A4 or
  a short clip, and confirm audible output plus visible error reporting when the
  output is unavailable.
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
- A CDP screenshot is nonblank or blank. Headless WebGPU screenshot capture can
  disagree with real UI state.
- Web MIDI passes only with the deterministic mock, without a real device.
- Web Audio reports an attached processor and nonzero generated samples, without
  confirming audible output in a real browser session.
- The Pages workflow file exists, without a successful deployed Pages run.
- `scripts/check-web-live.mjs` passes without a manual browser runtime check.

Treat these as partial evidence and keep the open gap visible in the handoff.
