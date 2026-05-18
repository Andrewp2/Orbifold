# Adding A UI Control Or Action

Use this guide when adding a visible button, toggle, selectable row, keyboard
shortcut, or simple pointer target. Dense editor gestures such as piano-roll
note dragging still use the custom surface path described in
`docs/operad_integration.md`.

## Control Ownership

Pick the smallest UI module that owns the visible area:

- `src/ui/native/top_bar.rs` for file, transport, tuning, and compact global
  controls.
- `src/ui/native/browser.rs` for the left asset/scale browser.
- `src/ui/native/editor_panels.rs` for the clip panel and piano-roll option
  controls.
- `src/ui/native/control_panel.rs` for synth, capture, layout, and main control
  panel controls.
- `src/ui/native/devices.rs` for audio/MIDI setup controls.
- `src/ui/native/surfaces.rs` plus `src/ui/native/piano_interaction.rs` for
  custom arrangement or piano-roll hit testing.

Do not add durable product state to a UI widget. Durable state belongs in
`AppState`, `MusicProject`, settings, or another model module.

## Add The Visible Control

Use the shared helpers in `src/ui/native/controls.rs`:

- `add_button_at`
- `add_button_at_with_visible_label`
- `add_compact_button_at`
- `add_toggle_button_at`
- `add_selectable_at`
- `add_label_at`
- `add_hit_at`
- `add_pointer_edit_hit_at`

These helpers attach stable action names, fitted text, visual state, enabled
state, and accessibility labels. Prefer them over constructing Operad widgets
directly.

Action names should be stable and domain-oriented:

- Good: `transport.record`, `asset.import`, `clip.duplicate_note`
- Avoid: labels such as `button1`, panel coordinates, or text that may change
  for compact layouts.

If a control has a compact visible label, keep the full action/accessibility
label descriptive. For example, a visible `Q1/16` button can still have the
action `transport.quantize_grid`.

## Route The Action

Ordinary actions route through `src/ui/actions.rs`:

1. Add a `dispatch_action` match arm or an indexed-prefix handler such as
   `asset.select.<index>`.
2. Call an `AppState` method for durable behavior.
3. Update status text through the `AppState` method when the action succeeds,
   refuses, or fails.
4. Add aliases to `canonical_action_name` only when a panel-local action should
   share behavior with another action.

Avoid editing low-level fields directly from `dispatch_action` unless the state
is intentionally UI-only. Most behavior should live behind an `AppState` method
so tests, keyboard shortcuts, and future menus can reuse it.

## Keyboard Shortcuts

Keyboard shortcuts live in `handle_key` in `src/ui/actions.rs`.

When adding one:

- decide whether it is a command shortcut (`Ctrl`/`Cmd`) or a plain workflow
  shortcut,
- decide whether key repeat is allowed in `key_repeat_allowed`,
- avoid `Alt` shortcuts unless the platform behavior is understood,
- document it in `docs/keyboard_shortcuts.md`,
- test the shortcut and any repeat policy in `src/ui/native/tests.rs`.

## Accessibility And Focus

Visible controls should have useful focus/accessibility labels. The shared
control helpers call `button_accessibility_label` in `src/ui/accessibility.rs`.

Add or update a label when:

- the visible text is abbreviated,
- the same visible text appears on multiple controls,
- the control changes meaning based on app state,
- the control is a destructive or panic action.

Canvas hit targets that are not meaningful keyboard controls should stay hidden
from focus traversal. Use ordinary focusable controls for keyboard operation.

## Tests To Add

Add tests with the UI change. Choose the checks that match the behavior:

- action binding: the node exists and its action name matches,
- click dispatch: `click_surface_node` changes `AppState` as expected,
- disabled behavior: the node is disabled and does not dispatch,
- text fit: `assert_text_overlap_free` for affected layouts,
- focus/accessibility: focus status names the control clearly,
- shortcut dispatch: `handle_key` or focused keyboard activation changes state,
- persistence: settings or project files are updated when the action is meant to
  persist.

Common test homes:

- `src/ui/native/tests.rs` for visible UI, layout, focus, cursor, screenshot, and
  interaction tests.
- `tests/user_docs.rs` for documentation coverage.
- Lower-level module tests when the action delegates to non-UI behavior.

## Completion Checklist

Before handing off a new control:

- control appears in the correct panel,
- action name is stable,
- `dispatch_action` routes it,
- `AppState` owns durable behavior,
- accessibility label is useful,
- disabled state is deliberate,
- status text describes success or refusal,
- tests cover the expected user-visible result,
- `docs/keyboard_shortcuts.md` is updated if a shortcut was added,
- screenshot inspection is complete when layout or visible text changed.
