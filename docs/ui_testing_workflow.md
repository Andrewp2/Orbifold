# UI Testing Workflow

Use this workflow when changing Orbifold layout, controls, Operad integration,
custom editor surfaces, cursor behavior, focus behavior, or visible text.

Automated tests are required, but they are not enough. A UI change is only
ready after the relevant behavior is covered by tests and at least one rendered
screenshot has been inspected.

## Fast Local Loop

For code that changes UI behavior, run the narrowest useful check first:

```sh
cargo test -q <test_name_fragment>
```

Useful focused areas live in `src/ui/native/tests.rs`:

- layout geometry and workspace splitters,
- text-overlap checks,
- visible action bindings,
- disabled-control behavior,
- keyboard shortcuts and focus traversal,
- cursor-shape hit testing,
- piano-roll pointer and wheel gestures,
- screenshot pixel validation.

For lower-level behavior behind UI actions, use the unit tests in `src/app.rs`,
`src/project.rs`, `src/midi.rs`, `src/settings.rs`, `src/synth.rs`, or the module
that owns the state being changed.

## Required Pre-Merge Checks

Before considering a UI change complete, run:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

If the change affects startup diagnostics or device setup paths, also run:

```sh
cargo run -- --startup-probe
```

`--startup-probe` should exit without opening a window and should log setup
diagnostics instead of relying on an interactive UI.

## Screenshot Checks

Use screenshot mode for no-window visual checks:

```sh
cargo run -- --screenshot-size=1200x760
cargo run -- --screenshot-size=3840x2160
```

Screenshot mode uses the same Operad document and renderer path as the native
UI, but skips audio and MIDI hardware probing. The latest image is written to
`screenshots/latest.png`.

Inspect the image after every layout, text, custom-surface, or scaling change.
Check for:

- content filling the full image instead of rendering in one corner,
- top-bar controls overlapping or clipping,
- left browser, clip panel, right panel, arrangement, piano roll, and status bar
  text staying inside their panels,
- readable 4K scale,
- usable minimum-size density,
- visible splitter handles and correct resize affordances,
- note, velocity, loop-boundary, and viewport indicators appearing where their
  hit targets imply they are.

Passing screenshot rendering is only a smoke check. It confirms the render path
produced nonblank pixels; it does not confirm that the UI is visually usable.
For the web build, prefer the headless Chrome smoke for runtime and interaction
coverage, but do not rely on CDP screenshot pixels as visual proof: headless
WebGPU capture can return a flat surface even while the wasm UI is receiving
events and updating state. Use `docs/web_parity_audit.md` when a change affects
browser parity or release readiness.

## Manual Interaction Checks

Use `docs/manual_qa_checklist.md` for broader manual QA. At minimum, manually
exercise any workflow touched by the change:

- click each changed visible control,
- verify disabled controls do not dispatch,
- tab to focusable controls and activate them with Enter,
- check cursor changes before drag actions,
- drag changed splitters, notes, loop boundaries, or viewport indicators,
- confirm status-bar feedback names the action or failure,
- confirm dirty state, settings persistence, or undo history when affected.

Avoid opening a normal app window just to verify screenshot or startup behavior.
Use `--screenshot-size` and `--startup-probe` when the check does not require
live interaction.

## Test Coverage Expectations

Add or update tests in the same change when introducing a new UI behavior.
Choose coverage based on the risk:

- New visible control: action binding, enabled/disabled behavior, dispatch, and
  accessibility/focus label when relevant.
- New layout path: geometry bounds and text-overlap tests at `1200x760` and any
  affected wide layout.
- New custom editor interaction: hit test, cursor shape, pointer begin/update/end
  behavior, and model mutation.
- New keyboard shortcut: shortcut dispatch, repeat policy if applicable, and
  docs in `docs/keyboard_shortcuts.md`.
- New persistence-affecting UI: settings or project file assertions plus a
  restart/load style check when practical.
- New startup or error path: status text and `log::error!` coverage where the
  app falls back or refuses an operation.

Prefer tests that assert user-visible outcomes rather than implementation
details. For example, assert a panel stays overlap-free, a note moves to the
expected beat, or a missing device status is visible.

## Evidence To Include In A Change Summary

When handing off UI work, report:

- the files or panels changed,
- the exact focused tests run,
- whether full `cargo test` and clippy passed,
- screenshot sizes captured,
- whether `screenshots/latest.png` was visually inspected,
- any manual workflow that could not be checked and why.

If visual inspection finds a known remaining issue, record it in
`docs/orbifold_usability_gap_analysis.md` or the relevant feature doc instead of
leaving it only in chat.
