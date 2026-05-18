# Known Limitations

Orbifold is still a prototype. This page lists the important current limits so
testers can separate expected unfinished behavior from bugs worth reporting.

## Composition Model

- Orbifold currently edits one single current clip.
- The visible DAW direction is multi-track, but the real multi-track
  arrangement model is not implemented yet.
- There is no clip launcher, scene launcher, timeline comping, track routing,
  track mute/solo, or per-track instrument assignment.
- Arrangement export, audio rendering, stem export, and project bounce are not
  implemented.

## Piano Roll And Editing

- The piano roll supports note creation, selection, move, resize, velocity
  editing, quantize, nudge, transpose, undo, and redo.
- Multi-select, marquee selection, fold-to-scale, draw mode, and full
  Ableton-style editing parity are not implemented yet.
- Zooming, scrolling, and resizable panels exist, but the interaction design is
  still being tuned.
- There is no separate automation editor beyond the current prototype clip
  surfaces.

## Audio And MIDI

- Orbifold uses one selected audio output and one selected MIDI input.
- Multi-input MIDI policies, MIDI output, external sync, clock receive/send, and
  plugin hosting are not implemented.
- The built-in synth is the primary sound engine. One selected WAV sample can be
  used as the project sample instrument, but there is no full sampler,
  instrument rack, mixer, effect chain, bus routing, or device preset workflow
  yet.
- If no audio output is available, sound-producing controls are disabled and the
  app should remain open in setup mode.

## Assets

- Assets live under `audio_assets/` and can be refreshed or imported into
  Samples, Instruments, Presets, or Impulses.
- WAV samples can be previewed from the browser or loaded as the project sample
  instrument; other sample formats are listed but not previewed yet.
- Assets are not yet assignable to clips, tracks, saved samplers, instruments,
  or effects.
- Project files save the selected sample path, but there is no asset packaging
  or relink workflow for moved samples yet. Missing sample-instrument references
  stay visible in the synth source row and can be cleared, but not repaired.
- Waveform thumbnails, tags, favorites, drag-to-track, preset loading, and
  convolution workflows are not implemented.

## Tuning And Lumatone

- Orbifold can load Scala `.scl` scales and Lumatone `.ltn` key maps.
- It warns when both the selected scale and selected key map advertise mismatched
  EDO/TET divisions in their names, but it does not validate arbitrary
  Scala/key-map compatibility beyond that name-based check.
- It does not program Lumatone hardware, send colors, send layouts, or save
  captured maps back to `.ltn` files.
- Mapping capture is diagnostic only.
- Missing scale and key-map files are reported, but there is no guided repair
  flow beyond loading another file.

## Files And Projects

- Project and settings files are plain-text formats intended for early
  development and debugging.
- Save, load, backup, and autosave exist, but there is no project asset
  packaging or relink workflow.
- The session strip shows a few recent-project rows, but this is not a full
  project browser or file-management surface.
- File association and installer-level project opening are not implemented.

## UI And Help

- The UI is rendered with Operad and is actively changing.
- Workspace panels can be resized and reset, but there is no saved named layout
  system.
- Keyboard shortcuts are documented in `docs/keyboard_shortcuts.md`, and `?`
  shows a compact status-bar hint, but there is no full in-app help browser.
- Accessibility naming and keyboard focus are improving, but the app has not
  reached a complete screen-reader or keyboard-only workflow.

## Web Build

- The web build shares the main Operad UI document and core app state, but it is
  not complete desktop parity yet.
- Complete web parity requires the manual validation in
  `docs/web_parity_audit.md`, including real browser/device checks and the
  deployed GitHub Pages site. The generated manual report must include real
  file-picker, shortcut, and piano-roll parity confirmations and also pass
  `./scripts/check-web-manual-report.mjs reports/`, and the final parity gate
  must confirm that report's artifact fingerprint still matches the live Pages
  artifact. The saved reports should then pass
  `./scripts/check-web-parity-complete.mjs reports/ --url https://<user>.github.io/<repo>/`,
  including saved visual manifest and viewport artifact validation for the
  deployed target, before anyone claims full web parity.
- WebGPU support is required for the live wasm UI. Browsers without WebGPU show
  the static fallback shell.
- Browser settings and the latest project session persist in `localStorage`.
  Browser-imported asset bytes persist in IndexedDB, with legacy `localStorage`
  migration/merge/fallback; large imports can still hit browser storage quota.
- Runtime UI scale changes on web persist and reload the page to apply the new
  scale because the current Operad web runtime does not expose live scale
  updates yet.

## Release And Testing

- There is no polished installer or signed release build.
- Linux desktop metadata exists, but release packaging is still minimal.
- Use screenshot mode when reporting visual issues:

```sh
cargo run -- --screenshot-size=1200x760
cargo run -- --screenshot-size=3840x2160
```

For first-run setup, see `docs/first_run.md`. For audio, MIDI, settings,
project, and autosave diagnostics, see `docs/troubleshooting.md`.
