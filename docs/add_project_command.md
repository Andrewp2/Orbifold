# Adding A Project Command

Use this guide when adding a command that changes the current musical project,
clip, transport, recorded notes, save/load state, or any state that should
participate in dirty tracking, undo/redo, autosave, or project files.

## Command Boundary

Project commands should usually have two layers:

1. `MusicProject` in `src/project.rs` owns pure project mutation such as adding,
   deleting, moving, resizing, quantizing, or serializing clip notes.
2. `AppState` in `src/app.rs` owns user-facing command behavior: validation,
   tuning lookup, selection, undo history, dirty state, autosave, status text,
   and audio audition.

UI code and `ui/actions.rs` should call the `AppState` command. They should not
lock `SharedMusicProject` and mutate it directly.

## Mutation Pattern

For a command that changes project data:

1. Validate preconditions before pushing history.
2. Capture undo history with `push_project_history` or
   `push_project_history_attempt`.
3. Mutate `MusicProject` through a method that returns whether anything changed.
4. If the mutation did not happen, call `discard_project_history_attempt` when
   an attempted history entry was added.
5. Update selection and piano-roll visibility when the edited note should remain
   in view.
6. Call `mark_project_dirty` after a real change.
7. Set a status message for success, refusal, or error.
8. Audition notes only when that is useful and safe.

Use `push_project_history_attempt` when the command may discover that no mutation
is possible after it has enough context to try. This keeps no-op commands from
polluting undo history.

## Dirty State And Autosave

`mark_project_dirty` refreshes dirty state, writes autosave when persistence is
enabled, and clears pending discard confirmations. Do not set `project_dirty`
directly for ordinary edit commands.

Commands that establish a clean baseline, such as successful save, load, or new
project creation, should update `clean_project_file` through the existing save,
load, or clean-snapshot paths instead of bypassing them.

If a command changes settings rather than the project, use the settings
persistence path and avoid marking the project dirty.

## Undo And Redo

Undo/redo snapshots include:

- `MusicProject::snapshot`,
- selected clip note,
- transport/edit state that belongs to the project snapshot.

When adding project state that should undo with clip edits, include it in
`ProjectSnapshot` in `src/project.rs` and in project file serialization when it
must be saved.

A command should not create an undo entry for:

- failed validation,
- no-op edits,
- selection-only changes,
- settings-only changes,
- device setup changes.

## Tuning And Musical Correctness

Commands that create or retune notes should use `ScaleState::note_info` through
`AppState` before mutating the project. That keeps `ClipNote` frequency,
musical-note identity, and pitch labels consistent with the active tuning.

Be explicit about how the command treats:

- raw MIDI note,
- musical note,
- Lumatone key index,
- mapped-from-Lumatone state,
- frequency,
- quantize grid and snap behavior.

Do not assume 12-TET semantics in a command that should work with Scala scales.

## File Format Changes

If the command adds project state that must persist:

- update `ProjectSnapshot`, `ProjectFile`, parser, and writer in
  `src/project.rs`,
- keep compatibility with old `orbifold_project=1` files when possible,
- update `docs/file_formats.md`,
- add parse/write round-trip tests,
- decide how missing or malformed fields report errors.

If the command only changes runtime view state, decide whether it belongs in
`AppSettings` instead of the project file.

## Tests To Add

Add tests near the behavior:

- `src/project.rs` for pure model mutation, serialization, quantize behavior,
  note identity, and no-op returns.
- `src/app.rs` for dirty state, undo/redo, autosave, status messages, tuning
  lookup, selection, and refusal paths.
- `src/ui/native/tests.rs` when a visible control, keyboard shortcut, pointer
  gesture, or cursor path triggers the command.
- `tests/user_docs.rs` if docs need to mention the new workflow.

At minimum, test success and one refusal/no-op path. For edit commands, also
test undo/redo and dirty state.

## Completion Checklist

Before handing off a project command:

- `MusicProject` owns the pure mutation,
- `AppState` owns user-facing command behavior,
- command routes through `ui/actions.rs` if visible,
- no direct project mutation happens in UI code,
- dirty state and autosave are correct,
- undo/redo are correct,
- status text names the result,
- save/load behavior is correct if state persists,
- tests cover success, no-op/refusal, and undo/redo where applicable,
- docs are updated when the workflow is user-visible.
