# Orbifold Keyboard Shortcuts

This is the current keyboard path for the Operad UI. Shortcuts are ignored when
`Alt` is held, so operating-system menu chords and international-keyboard
modifier chords do not accidentally fire app commands.

`Ctrl` and `Cmd` mean the same command modifier in this document. Use `Ctrl` on
Linux and Windows, and `Cmd` on macOS. Arrow-key note edits may repeat while the
key is held; transport, project, and command shortcuts do not repeat.

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

Clip-editing shortcuts that operate on a note require a selected note, except
for `N`, paste, and whole-clip quantize. After a note is added, duplicated,
pasted, or selected, Orbifold scrolls the piano roll just enough to keep that
edited note visible.

## View

- `+` or `=`: zoom the piano roll in horizontally around the playhead.
- `-`: zoom the piano roll out horizontally around the playhead.
- Wheel over the piano roll: scroll the visible time or pitch window.
- `Ctrl`/`Cmd+Wheel` over the piano roll: zoom time around the pointer.
- `Shift+Wheel` over the piano roll: pan the visible time window.
- `Alt+Wheel` over the piano roll: zoom pitch rows vertically.
- The piano-roll `Fit` button restores the visible range around the clip's notes,
  or resets to the root/default view when the clip is empty.
- `Ctrl`/`Cmd++` or `Ctrl`/`Cmd+=`: increase UI zoom.
- `Ctrl`/`Cmd+-`: decrease UI zoom.
- `Ctrl`/`Cmd+0`: reset UI zoom.

## Workflow Examples

### Record A Phrase

1. Press `M` if you want the metronome.
2. Press `Shift+Q` to toggle record quantize.
3. Press `R` to start recording.
4. Play the phrase.
5. Press `R` again, or press `Space`, to leave recording.
6. Press `Q` with no note selected to quantize the whole clip.

### Add And Shape One Note

1. Press `Home` to return to the loop start.
2. Press `N` to add a note at the playhead.
3. Use Arrow left/right to move it earlier or later.
4. Use Arrow up/down to transpose it.
5. Use `Shift+ArrowLeft` or `Shift+ArrowRight` to shorten or lengthen it.
6. Use `Shift+ArrowUp` or `Shift+ArrowDown` to adjust velocity.
7. Press `Q` to quantize the selected note.

### Recover From An Edit

1. Press `Ctrl`/`Cmd+Z` to undo the last project edit.
2. Press `Ctrl`/`Cmd+Y` or `Ctrl`/`Cmd+Shift+Z` to redo it.
3. Press `Esc` to clear the selected note when no discard confirmation is
   pending.

### Copy Material At The Playhead

1. Select a note in the piano roll.
2. Press `Ctrl`/`Cmd+C`.
3. Move the playhead by clicking or dragging the arrangement or piano ruler.
4. Press `Ctrl`/`Cmd+V` to paste the copied note at the playhead.
5. Press `D` when you want a duplicate based on the selected note instead.

### Make The View Readable

1. Press `+` or `=` to zoom the piano roll time view in around the playhead.
2. Press `-` to zoom the piano roll time view out.
3. Use `Ctrl`/`Cmd+Wheel` over the piano roll to zoom time around the pointer.
4. Use `Alt+Wheel` over the piano roll to zoom pitch rows vertically.
5. Click `Fit` in the piano-roll panel to recover the clip view after heavy
   zooming or panning.
6. Press `Ctrl`/`Cmd++` or `Ctrl`/`Cmd+=` to increase the whole UI scale.
7. Press `Ctrl`/`Cmd+-` to decrease the whole UI scale.
8. Press `Ctrl`/`Cmd+0` to return the UI scale to default.

### Save Or Open Without Losing Work

1. Press `Ctrl`/`Cmd+S` to save the current project.
2. Press `Ctrl`/`Cmd+Shift+S` to save to a new project path.
3. Press `Ctrl`/`Cmd+O` to open another project.
4. If there are unsaved changes, Orbifold asks for a second confirmation before
   replacing the current work.
