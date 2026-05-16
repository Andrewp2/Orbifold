# Orbifold Keyboard Shortcuts

This is the current keyboard path for the Operad UI. Shortcuts are ignored when
`Alt` is held, so operating-system menu chords and international-keyboard
modifier chords do not accidentally fire app commands.

## Discoverability And Focus

- `?` or `Shift+/`: show a compact shortcut reference in the status bar.
- `Tab`: move focus to the next visible control.
- `Shift+Tab`: move focus to the previous visible control.
- `Enter`: activate the focused control.
- `Esc`: cancel a pending discard confirmation, or clear the selected note when
  no confirmation is pending.

## Project And Edit

- `Ctrl`/`Cmd+N`: start a new project. Unsaved changes require a second
  confirmation.
- `Ctrl`/`Cmd+O`: open a project. Unsaved changes require a second
  confirmation.
- `Ctrl`/`Cmd+S`: save.
- `Ctrl`/`Cmd+Shift+S`: save as a new project path.
- `Ctrl`/`Cmd+Z`: undo.
- `Ctrl`/`Cmd+Y` or `Ctrl`/`Cmd+Shift+Z`: redo.

## Transport And Recording

- `Space`: toggle playback.
- `Home`: return the playhead to the loop start.
- `R`: toggle recording.
- `M`: toggle the metronome.
- `Shift+Q`: toggle record quantize.
- `G`: toggle piano-roll snap off and back to the previous grid value.
- `P`: run All Off as a panic/reset action.

## Clip Editing

- `N`: add a note at the playhead.
- `D`: duplicate the selected note.
- `Q`: quantize the selected note, or the whole clip when no note is selected.
- `Delete` or `Backspace`: delete the selected note.
- Arrow left/right: move the selected note earlier or later.
- Arrow up/down: transpose the selected note.
- `Shift+ArrowLeft` / `Shift+ArrowRight`: resize the selected note.
- `Shift+ArrowUp` / `Shift+ArrowDown`: adjust selected-note velocity.
- `Ctrl`/`Cmd+C`: copy the selected note.
- `Ctrl`/`Cmd+V`: paste the copied note at the playhead.

## View

- `+` or `=`: zoom the piano roll in horizontally around the playhead.
- `-`: zoom the piano roll out horizontally around the playhead.
- `Ctrl`/`Cmd++` or `Ctrl`/`Cmd+=`: increase UI zoom.
- `Ctrl`/`Cmd+-`: decrease UI zoom.
- `Ctrl`/`Cmd+0`: reset UI zoom.
