# Operad Integration Model

Orbifold uses Operad as its primary UI toolkit through the `widgets` and
`native-window` features. The current integration is a native desktop UI with
custom rendered editor surfaces for arrangement and piano-roll workflows.

## Dependency Shape

`Cargo.toml` depends on Operad with `default-features = false` and enables:

- `widgets` for standard buttons, labels, selectable labels, scenes, and
  canvases.
- `native-window` for the native window/event-loop host.

Because `native-window` owns the event loop, Orbifold supplies an app state,
view function, update function, canvas render registry, and native hooks instead
of running its own winit loop directly.

## Native Host

`src/ui/mod.rs` exposes `native::run`.

`src/ui/native.rs` owns the Operad host type, `NativeOperadApp`. That host wraps
`AppState` plus UI-only interaction state:

- current computed `SurfaceRects`,
- pointer/cursor state,
- note, timeline, keyboard, viewport, and workspace drag state,
- focused action name,
- current UI scale.

`operad::run_app_with_canvas_renderers_and_hooks` receives:

- `NativeWindowOptions` for title, size, minimum size, and base scale,
- `NativeOperadApp::update` for widget actions,
- `NativeOperadApp::view` for document construction,
- `NativeWgpuCanvasRenderRegistry`,
- `NativeWindowHooks` for startup size, title, scale factor, close handling,
  keyboard input, canvas input, platform requests, before-render work, and idle
  redraw.

## Frame Lifecycle

Each frame follows this shape:

1. `prepare_frame` polls pending file-dialog results, stores UI scale, and
   computes the current layout rectangles.
2. `view` calls `build_surface_document`.
3. `build_surface_document` builds a fresh `UiDocument` from current `AppState`.
4. Operad renders the document and returns widget/canvas events.
5. `update`, keyboard hooks, or canvas hooks dispatch state changes into
   `AppState` or native drag state.

Orbifold treats the Operad document as a rendered projection of state. Durable
state belongs in `AppState`, `MusicProject`, settings, or explicit native drag
fields, not in widget instances.

## Document Construction

`build_surface_document` creates a root `UiDocument`, then layers the UI:

1. A full-workspace `widgets::scene` named `orbifold.native.surface` draws the
   background, top bar, browser, clip panel, right panel, workspace splitters,
   and status bar using `ScenePrimitive`s.
2. `add_center_editor_surfaces` adds rendered arrangement and piano-roll
   surfaces.
3. `add_operad_controls` adds ordinary controls, buttons, selectable labels,
   toggles, device rows, browser rows, and editor controls.
4. Resize and editor hit targets are added as transparent Operad nodes.
5. Piano-label overlays are added after controls so dense pitch text stays
   readable.

Most layout is absolute. Shared layout math lives in `native/workspace.rs`,
`native/browser.rs`, and `native/surfaces.rs`.

## Ordinary Controls

Use helpers in `native/controls.rs` for ordinary controls:

- `add_button_at`
- `add_toggle_at`
- `add_selectable_at`
- `add_label_at`
- `add_hit_at`
- `add_pointer_edit_hit_at`

These helpers set stable action names, fitted labels, visual state, enabled
state, and Operad action bindings. Prefer them over directly constructing
widgets unless a new control pattern is genuinely needed.

Visible controls should have an action name that routes through
`ui/actions.rs`. Accessibility/focus labels for action names belong in
`ui/accessibility.rs`. The detailed workflow for adding a UI control or action
lives in `docs/add_ui_control.md`.

## Custom Editor Surfaces

The arrangement and piano roll are not ordinary button grids. They use a split
pattern:

- `native/surfaces.rs` draws dense visuals with `ScenePrimitive`s.
- Transparent canvas/hit-target nodes capture pointer, wheel, and drag gestures.
- `native/piano_interaction.rs` interprets piano-roll hit tests, drag modes,
  cursor shape, and double-click behavior.

This keeps dense editor visuals independent from ordinary control widgets while
still letting Operad route pointer input.

Canvas hit targets that are not meaningful keyboard controls should stay hidden
from focus traversal. Keyboard focus should land on visible buttons and controls,
not invisible piano-grid regions.

## Action Dispatch

Ordinary widget actions flow through `NativeOperadApp::update`:

```text
Operad WidgetAction -> action name -> ui/actions.rs -> AppState
```

Pointer-edit actions and canvas gestures may also update native drag state before
calling `AppState`. Examples include note drags, loop-end drags, piano viewport
drags, and workspace splitter drags.

Action names should be stable and descriptive, such as:

- `file.save`
- `transport.play`
- `asset.select.4`
- `note.resize_end.42`
- `layout.resize.bottom`

If an action needs aliases for older names or panel-local names, normalize it in
`canonical_action_name`.

If the action mutates project data, follow `docs/add_project_command.md` so dirty
state, undo/redo, autosave, and persistence stay correct.

## Keyboard, Focus, And Cursor

Keyboard shortcuts enter through the native keyboard hook and are handled in
`ui/actions.rs`. Tab focus and Enter activation are handled by Operad document
focus state.

Cursor shape is computed by the native host from the current pointer position
and active drag state. Resize handles, note edges, velocity bars, viewport
indicators, and piano keyboard drags should report appropriate cursor shapes.

Focus and accessibility status are derived from action names in
`ui/accessibility.rs`; do not rely on raw button text when a more precise action
description is available.

## Screenshot Mode

Screenshot mode builds the same Operad document without opening an interactive
window or probing hardware. `native/screenshot.rs` renders through
`WgpuRenderer` into a snapshot target, validates that pixels are nonblank and not
obviously corner-cropped, writes a timestamped image, and updates
`screenshots/latest.png`.

Use screenshot mode for visual QA:

```sh
cargo run -- --screenshot-size=1200x760
cargo run -- --screenshot-size=3840x2160
```

## Testing Expectations

Important Operad integration tests live in `src/ui/native/tests.rs`:

- document construction and text-overlap checks,
- action binding checks,
- disabled-control action checks,
- pointer hit-target dispatch,
- canvas coordinate dispatch,
- cursor-shape checks,
- focus traversal,
- screenshot pixel validation.

When changing a layout, custom surface, hit target, or visible label, add or
update tests near the behavior and inspect a rendered screenshot. The full UI
testing workflow lives in `docs/ui_testing_workflow.md`.

## Rules For New UI Work

- Put durable behavior in `AppState` or lower-level model modules.
- Put action routing in `ui/actions.rs`.
- Put control construction in the relevant `native/*` panel module.
- Put dense editor drawing and hit testing in `native/surfaces.rs` and
  `native/piano_interaction.rs`.
- Keep invisible hit targets out of keyboard focus.
- Prefer stable action names over matching visible text.
- Use `fit_label` or existing control helpers when text must fit variable-width
  controls.
- Add regression tests for action bindings, disabled states, cursor behavior, or
  text overlap when those are part of the change.
