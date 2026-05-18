use operad::{
    CursorGrabMode, CursorRequest, CursorShape, NativeCanvasInput, NativeKeyboardInput,
    PlatformRequest, PointerButton, PointerEventKind, RawInputEvent, RawPointerEvent, UiPoint,
    UiSize, WheelPhase, WidgetDrag, WidgetValueEditPhase,
};

use crate::app::WorkspaceResizeTarget;

use super::piano_interaction::{
    LoopEndDragMode, NoteDrag, NoteDragMode, PianoGridClick, PianoGridPress, PianoKeyboardDrag,
    PianoViewportDrag, PianoViewportDragMode, TimelineDragMode, note_drag_from_action,
    piano_cursor_shape_at, piano_note_hit_at, point_distance,
};
use super::{
    NativeOperadApp, PIANO_GRID_DOUBLE_CLICK_DISTANCE, PIANO_GRID_DOUBLE_CLICK_MILLIS,
    PIANO_INPUT_CANVAS_KEY, WorkspaceResizeDrag, loop_end_resize_mode_at_point,
    piano_viewport_drag_cursor, piano_viewport_drag_mode_at_point, rect_contains_point,
    widget_drag_phase, workspace_resize_cursor, workspace_resize_target_at_point,
    workspace_resize_target_from_action,
};
use crate::ui::actions::handle_key;

impl NativeOperadApp {
    pub(super) fn handle_pointer_edit_action(
        &mut self,
        action: &str,
        phase: WidgetValueEditPhase,
        point: UiPoint,
    ) {
        log::trace!(
            target: "orbifold::ui::native",
            "pointer edit action={action} phase={phase:?} point={point:?}"
        );
        if self.handle_active_pointer_edit_action(phase, point) {
            return;
        }

        if let Some(target) = workspace_resize_target_from_action(action) {
            self.cursor_shape = workspace_resize_cursor(target);
            match phase {
                WidgetValueEditPhase::Begin => {
                    self.workspace_resize_drag =
                        Some(self.workspace_resize_drag_for_pointer(target, point));
                    let _ = self.drag_workspace_layout(point, false);
                }
                WidgetValueEditPhase::Update => {
                    if self.workspace_resize_drag.is_none() {
                        self.workspace_resize_drag =
                            Some(self.workspace_resize_drag_for_pointer(target, point));
                    }
                    let _ = self.drag_workspace_layout(point, false);
                }
                WidgetValueEditPhase::Commit => {
                    if self.workspace_resize_drag.is_none() {
                        self.workspace_resize_drag =
                            Some(self.workspace_resize_drag_for_pointer(target, point));
                    }
                    let _ = self.drag_workspace_layout(point, true);
                    self.workspace_resize_drag = None;
                    self.update_cursor_for_point(point);
                }
                WidgetValueEditPhase::Cancel => {
                    self.workspace_resize_drag = None;
                    self.update_cursor_for_point(point);
                }
                WidgetValueEditPhase::Preview => {}
            }
            return;
        }

        if let Some(mode) = LoopEndDragMode::from_action(action) {
            self.cursor_shape = CursorShape::ResizeHorizontal;
            match phase {
                WidgetValueEditPhase::Begin => {
                    self.loop_end_drag = Some(mode);
                    let _ = self.resize_loop_end(mode, point);
                }
                WidgetValueEditPhase::Update => {
                    if self.loop_end_drag.is_none() {
                        self.loop_end_drag = Some(mode);
                    }
                    let _ = self.resize_loop_end(mode, point);
                }
                WidgetValueEditPhase::Commit => {
                    if self.loop_end_drag.is_none() {
                        self.loop_end_drag = Some(mode);
                    }
                    let _ = self.resize_loop_end(mode, point);
                    self.loop_end_drag = None;
                    self.update_cursor_for_point(point);
                }
                WidgetValueEditPhase::Cancel => {
                    self.loop_end_drag = None;
                    self.update_cursor_for_point(point);
                }
                WidgetValueEditPhase::Preview => {}
            }
            return;
        }

        if let Some(mode) = PianoViewportDragMode::from_action(action) {
            self.cursor_shape = piano_viewport_drag_cursor(mode);
            match phase {
                WidgetValueEditPhase::Begin => {
                    self.piano_viewport_drag =
                        Some(self.piano_viewport_drag_for_pointer(mode, point));
                    let _ = self.drag_piano_viewport(point);
                }
                WidgetValueEditPhase::Update => {
                    if self.piano_viewport_drag.is_none() {
                        self.piano_viewport_drag =
                            Some(self.piano_viewport_drag_for_pointer(mode, point));
                    }
                    let _ = self.drag_piano_viewport(point);
                }
                WidgetValueEditPhase::Commit => {
                    if self.piano_viewport_drag.is_none() {
                        self.piano_viewport_drag =
                            Some(self.piano_viewport_drag_for_pointer(mode, point));
                    }
                    let _ = self.drag_piano_viewport(point);
                    self.piano_viewport_drag = None;
                    self.update_cursor_for_point(point);
                }
                WidgetValueEditPhase::Cancel => {
                    self.piano_viewport_drag = None;
                    self.update_cursor_for_point(point);
                }
                WidgetValueEditPhase::Preview => {}
            }
            return;
        }

        if matches!(action, "transport.seek" | "piano.seek") {
            if !matches!(
                phase,
                WidgetValueEditPhase::Cancel | WidgetValueEditPhase::Preview
            ) {
                self.timeline_drag = TimelineDragMode::from_action(action);
                let _ = self.seek_timeline(point);
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.timeline_drag = None;
            }
            return;
        }

        if action == "piano.keyboard" {
            self.handle_piano_keyboard_pointer_edit(phase, point);
            return;
        }

        if action == "piano.grid" {
            self.handle_piano_grid_pointer_edit(phase, point);
            return;
        }

        let Some((note_id, mode)) = note_drag_from_action(action) else {
            return;
        };
        match phase {
            WidgetValueEditPhase::Begin => {
                self.app.select_clip_note(Some(note_id));
                self.note_drag = Some(self.note_drag_for_pointer(note_id, mode, point));
                self.update_cursor_for_point(point);
                if matches!(mode, NoteDragMode::Velocity) {
                    let _ = self.drag_selected_note(point);
                }
            }
            WidgetValueEditPhase::Update => {
                if self.note_drag.is_none() {
                    self.app.select_clip_note(Some(note_id));
                    self.note_drag = Some(self.note_drag_for_pointer(note_id, mode, point));
                }
                self.update_cursor_for_point(point);
                let _ = self.drag_selected_note(point);
            }
            WidgetValueEditPhase::Commit => {
                if self.note_drag.is_none() {
                    self.app.select_clip_note(Some(note_id));
                    self.note_drag = Some(self.note_drag_for_pointer(note_id, mode, point));
                }
                let _ = self.drag_selected_note(point);
                self.note_drag = None;
                self.update_cursor_for_point(point);
            }
            WidgetValueEditPhase::Cancel => {
                self.note_drag = None;
                self.update_cursor_for_point(point);
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    pub(super) fn handle_native_keyboard_input(&mut self, input: NativeKeyboardInput) -> bool {
        handle_key(
            &mut self.app,
            &input.logical_key,
            input.modifiers,
            input.repeat,
        )
    }

    pub(super) fn handle_canvas_input(&mut self, input: NativeCanvasInput) -> bool {
        if input.key != PIANO_INPUT_CANVAS_KEY {
            return false;
        }
        match input.input {
            RawInputEvent::Pointer(pointer) => self.observe_piano_pointer_input(pointer),
            RawInputEvent::Wheel(wheel) => {
                if matches!(wheel.phase, WheelPhase::Ended) {
                    return true;
                }
                self.cursor_pos = Some(wheel.position);
                self.update_cursor_for_point(wheel.position);
                let Some(layout) = self.layout else {
                    return true;
                };
                if !layout.contains_piano_input(wheel.position) {
                    return true;
                }
                let delta =
                    wheel.pixel_delta(36.0, UiSize::new(input.rect.width, input.rect.height));
                if wheel.modifiers.ctrl || wheel.modifiers.meta {
                    let zoom_delta = if delta.y.abs() > f32::EPSILON {
                        delta.y
                    } else {
                        -delta.x
                    };
                    let _ = self
                        .app
                        .zoom_piano_roll(zoom_delta, layout.beat_at(wheel.position));
                    return true;
                }
                if wheel.modifiers.alt {
                    let zoom_delta = if delta.y.abs() > f32::EPSILON {
                        delta.y
                    } else {
                        -delta.x
                    };
                    let _ = self
                        .app
                        .zoom_piano_roll_pitches(zoom_delta, layout.pitch_at(wheel.position));
                    return true;
                }
                let (delta_beats, delta_pitches) =
                    layout.piano_wheel_scroll_delta(delta, wheel.modifiers.shift);
                let _ = self.app.scroll_piano_roll(delta_beats, delta_pitches);
                true
            }
            _ => false,
        }
    }

    pub(super) fn observe_piano_pointer_input(&mut self, pointer: RawPointerEvent) -> bool {
        self.cursor_pos = Some(pointer.position);
        match pointer.kind {
            PointerEventKind::Down(PointerButton::Primary) => {
                self.observe_piano_pointer_down(pointer);
                false
            }
            PointerEventKind::Move => {
                if pointer.buttons.contains(PointerButton::Primary)
                    && self.handle_active_drag_phase(WidgetValueEditPhase::Update, pointer.position)
                {
                    return true;
                }
                let hover_only = !pointer.buttons.contains(PointerButton::Primary);
                if hover_only {
                    self.piano_grid_press = None;
                }
                if hover_only
                    && self.note_drag.is_none()
                    && self.timeline_drag.is_none()
                    && self.loop_end_drag.is_none()
                    && self.piano_keyboard_drag.is_none()
                    && self.piano_viewport_drag.is_none()
                    && self.workspace_resize_drag.is_none()
                {
                    self.update_cursor_for_point(pointer.position);
                    return true;
                }
                false
            }
            PointerEventKind::Up(PointerButton::Primary) => {
                if self.handle_active_drag_phase(WidgetValueEditPhase::Commit, pointer.position) {
                    return true;
                }
                self.observe_piano_pointer_up(pointer);
                false
            }
            PointerEventKind::Cancel => {
                if self.handle_active_drag_phase(WidgetValueEditPhase::Cancel, pointer.position) {
                    return true;
                }
                self.piano_grid_press = None;
                self.loop_end_drag = None;
                self.update_cursor_for_point(pointer.position);
                false
            }
            PointerEventKind::Down(_) | PointerEventKind::Up(_) => {
                self.update_cursor_for_point(pointer.position);
                false
            }
        }
    }

    pub(super) fn observe_piano_pointer_down(&mut self, pointer: RawPointerEvent) {
        let Some(layout) = self.layout else {
            self.update_cursor_for_point(pointer.position);
            return;
        };
        self.piano_grid_press = if rect_contains_point(layout.piano_grid, pointer.position)
            && piano_note_hit_at(&self.app, layout, pointer.position).is_none()
        {
            Some(PianoGridPress {
                position: pointer.position,
            })
        } else {
            None
        };
        self.update_cursor_for_point(pointer.position);
    }

    pub(super) fn observe_piano_pointer_up(&mut self, pointer: RawPointerEvent) {
        let Some(press) = self.piano_grid_press.take() else {
            self.update_cursor_for_point(pointer.position);
            return;
        };
        let Some(layout) = self.layout else {
            self.update_cursor_for_point(pointer.position);
            return;
        };
        if rect_contains_point(layout.piano_grid, pointer.position)
            && point_distance(press.position, pointer.position) <= PIANO_GRID_DOUBLE_CLICK_DISTANCE
        {
            if self.is_piano_grid_double_click(pointer.position, pointer.timestamp_millis) {
                self.app.add_clip_note_at(
                    layout.beat_at(pointer.position),
                    layout.pitch_at(pointer.position),
                );
                self.last_piano_grid_click = None;
            } else {
                self.last_piano_grid_click = Some(PianoGridClick {
                    position: pointer.position,
                    timestamp_millis: pointer.timestamp_millis,
                });
            }
        }
        self.update_cursor_for_point(pointer.position);
    }

    pub(super) fn handle_active_pointer_edit_action(
        &mut self,
        phase: WidgetValueEditPhase,
        point: UiPoint,
    ) -> bool {
        if matches!(phase, WidgetValueEditPhase::Preview) {
            return false;
        }
        self.handle_active_drag_phase(phase, point)
    }

    pub(super) fn handle_active_drag_action(&mut self, drag: WidgetDrag) -> bool {
        let phase = widget_drag_phase(drag.phase);
        self.handle_active_drag_phase(phase, drag.current)
    }

    fn handle_active_drag_phase(&mut self, phase: WidgetValueEditPhase, point: UiPoint) -> bool {
        if let Some(active) = self.workspace_resize_drag {
            self.cursor_shape = workspace_resize_cursor(active.target);
            match phase {
                WidgetValueEditPhase::Begin
                | WidgetValueEditPhase::Update
                | WidgetValueEditPhase::Commit => {
                    let _ = self.drag_workspace_layout(
                        point,
                        matches!(phase, WidgetValueEditPhase::Commit),
                    );
                }
                WidgetValueEditPhase::Cancel => {}
                WidgetValueEditPhase::Preview => {}
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.workspace_resize_drag = None;
                self.update_cursor_for_point(point);
            }
            return true;
        }

        if let Some(active) = self.piano_viewport_drag {
            self.cursor_shape = piano_viewport_drag_cursor(active.mode);
            match phase {
                WidgetValueEditPhase::Begin
                | WidgetValueEditPhase::Update
                | WidgetValueEditPhase::Commit => {
                    let _ = self.drag_piano_viewport(point);
                }
                WidgetValueEditPhase::Cancel => {}
                WidgetValueEditPhase::Preview => {}
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.piano_viewport_drag = None;
                self.update_cursor_for_point(point);
            }
            return true;
        }

        if self.note_drag.is_some() {
            match phase {
                WidgetValueEditPhase::Begin
                | WidgetValueEditPhase::Update
                | WidgetValueEditPhase::Commit => {
                    let _ = self.drag_selected_note(point);
                }
                WidgetValueEditPhase::Cancel => {}
                WidgetValueEditPhase::Preview => {}
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.note_drag = None;
                self.update_cursor_for_point(point);
            } else {
                self.update_cursor_for_point(point);
            }
            return true;
        }

        if self.piano_keyboard_drag.is_some() {
            match phase {
                WidgetValueEditPhase::Begin
                | WidgetValueEditPhase::Update
                | WidgetValueEditPhase::Commit => {
                    let _ = self.drag_piano_keyboard(point);
                }
                WidgetValueEditPhase::Cancel => {}
                WidgetValueEditPhase::Preview => {}
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.piano_keyboard_drag = None;
                self.update_cursor_for_point(point);
            } else {
                self.update_cursor_for_point(point);
            }
            return true;
        }

        if let Some(mode) = self.loop_end_drag {
            self.cursor_shape = CursorShape::ResizeHorizontal;
            match phase {
                WidgetValueEditPhase::Begin
                | WidgetValueEditPhase::Update
                | WidgetValueEditPhase::Commit => {
                    let _ = self.resize_loop_end(mode, point);
                }
                WidgetValueEditPhase::Cancel => {}
                WidgetValueEditPhase::Preview => {}
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.loop_end_drag = None;
                self.update_cursor_for_point(point);
            }
            return true;
        }

        if self.timeline_drag.is_some() {
            match phase {
                WidgetValueEditPhase::Begin
                | WidgetValueEditPhase::Update
                | WidgetValueEditPhase::Commit => {
                    let _ = self.seek_timeline(point);
                }
                WidgetValueEditPhase::Cancel => {}
                WidgetValueEditPhase::Preview => {}
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.timeline_drag = None;
                self.update_cursor_for_point(point);
            }
            return true;
        }

        false
    }

    pub(super) fn handle_piano_keyboard_pointer_edit(
        &mut self,
        phase: WidgetValueEditPhase,
        point: UiPoint,
    ) {
        self.cursor_shape = CursorShape::ResizeNorthEastSouthWest;
        match phase {
            WidgetValueEditPhase::Begin => {
                self.piano_keyboard_drag = Some(PianoKeyboardDrag {
                    start_position: point,
                    last_position: point,
                    pitch_remainder_px: 0.0,
                    moved: false,
                });
            }
            WidgetValueEditPhase::Update => {
                if self.piano_keyboard_drag.is_none() {
                    self.piano_keyboard_drag = Some(PianoKeyboardDrag {
                        start_position: point,
                        last_position: point,
                        pitch_remainder_px: 0.0,
                        moved: false,
                    });
                }
                let _ = self.drag_piano_keyboard(point);
            }
            WidgetValueEditPhase::Commit => {
                if self.piano_keyboard_drag.is_none() {
                    self.audition_piano_keyboard_pitch(point);
                    self.update_cursor_for_point(point);
                    return;
                }
                let _ = self.drag_piano_keyboard(point);
                let drag = self.piano_keyboard_drag.take();
                if drag.is_some_and(|drag| {
                    !drag.moved
                        && point_distance(drag.start_position, point)
                            <= PIANO_GRID_DOUBLE_CLICK_DISTANCE
                }) {
                    self.audition_piano_keyboard_pitch(point);
                }
                self.update_cursor_for_point(point);
            }
            WidgetValueEditPhase::Cancel => {
                self.piano_keyboard_drag = None;
                self.update_cursor_for_point(point);
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    pub(super) fn handle_piano_grid_pointer_edit(
        &mut self,
        phase: WidgetValueEditPhase,
        point: UiPoint,
    ) {
        match phase {
            WidgetValueEditPhase::Begin | WidgetValueEditPhase::Update => {
                self.piano_grid_press = None;
                self.update_cursor_for_point(point);
            }
            WidgetValueEditPhase::Commit => {
                self.piano_grid_press = None;
                self.update_cursor_for_point(point);
            }
            WidgetValueEditPhase::Cancel => {
                self.piano_grid_press = None;
                self.update_cursor_for_point(point);
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    pub(super) fn is_piano_grid_double_click(&self, point: UiPoint, timestamp_millis: u64) -> bool {
        self.last_piano_grid_click.is_some_and(|click| {
            timestamp_millis.saturating_sub(click.timestamp_millis)
                <= PIANO_GRID_DOUBLE_CLICK_MILLIS
                && point_distance(click.position, point) <= PIANO_GRID_DOUBLE_CLICK_DISTANCE
        })
    }

    pub(super) fn drag_piano_keyboard(&mut self, point: UiPoint) -> bool {
        let Some(layout) = self.layout else {
            return false;
        };
        let row_height = layout.row_height().max(1.0);
        let Some((delta_x, row_delta)) = self.piano_keyboard_drag.as_mut().map(|drag| {
            if point_distance(drag.start_position, point) > PIANO_GRID_DOUBLE_CLICK_DISTANCE {
                drag.moved = true;
            }
            let delta_x = point.x - drag.last_position.x;
            let delta_y = point.y - drag.last_position.y;
            drag.last_position = point;
            let total_y = drag.pitch_remainder_px + delta_y;
            let row_delta = (total_y / row_height).trunc() as i32;
            drag.pitch_remainder_px = total_y - row_delta as f32 * row_height;
            (delta_x, row_delta)
        }) else {
            return false;
        };

        let mut changed = false;
        if delta_x.abs() > f32::EPSILON {
            changed |= self
                .app
                .zoom_piano_roll_pitches(delta_x * 2.0, layout.pitch_at(point));
        }

        if row_delta != 0 {
            changed |= self.app.scroll_piano_roll(0.0, -row_delta);
        }
        changed
    }

    pub(super) fn audition_piano_keyboard_pitch(&mut self, point: UiPoint) -> bool {
        let Some(layout) = self.layout else {
            return false;
        };
        self.app.audition_piano_pitch(layout.pitch_at(point));
        true
    }

    pub(super) fn piano_viewport_drag_for_pointer(
        &self,
        mode: PianoViewportDragMode,
        point: UiPoint,
    ) -> PianoViewportDrag {
        let Some(layout) = self.layout else {
            return PianoViewportDrag {
                mode,
                grab_offset_px: 0.0,
            };
        };
        let grab_offset_px = match mode {
            PianoViewportDragMode::Time => layout.piano_time_view_grab_offset(point),
            PianoViewportDragMode::Pitch => layout.piano_pitch_view_grab_offset(point),
        };
        PianoViewportDrag {
            mode,
            grab_offset_px,
        }
    }

    pub(super) fn drag_piano_viewport(&mut self, point: UiPoint) -> bool {
        let Some(drag) = self.piano_viewport_drag else {
            return false;
        };
        let Some(layout) = self.layout else {
            return false;
        };
        match drag.mode {
            PianoViewportDragMode::Time => {
                let fraction = layout.piano_time_view_fraction(point, drag.grab_offset_px);
                self.app.set_piano_time_view_fraction(fraction)
            }
            PianoViewportDragMode::Pitch => {
                let fraction = layout.piano_pitch_view_fraction(point, drag.grab_offset_px);
                self.app.set_piano_pitch_view_fraction(fraction)
            }
        }
    }

    pub(super) fn cursor_platform_requests(&mut self) -> Vec<PlatformRequest> {
        let mut requests = Vec::new();
        let should_grab_cursor = self.has_active_pointer_drag();
        if should_grab_cursor != self.cursor_grab_active {
            self.cursor_grab_active = should_grab_cursor;
            let mode = if should_grab_cursor {
                CursorGrabMode::Confined
            } else {
                CursorGrabMode::None
            };
            requests.push(PlatformRequest::Cursor(CursorRequest::SetGrab(mode)));
        }

        if self.cursor_shape != self.applied_cursor_shape {
            self.applied_cursor_shape = self.cursor_shape;
            requests.push(PlatformRequest::Cursor(CursorRequest::SetShape(
                self.cursor_shape,
            )));
        }
        requests
    }

    pub(super) fn update_cursor_for_point(&mut self, point: UiPoint) {
        if let Some(drag) = self.workspace_resize_drag {
            self.cursor_shape = workspace_resize_cursor(drag.target);
            return;
        }
        if let Some(drag) = self.piano_viewport_drag {
            self.cursor_shape = piano_viewport_drag_cursor(drag.mode);
            return;
        }
        if self.note_drag.is_some() {
            self.cursor_shape =
                piano_cursor_shape_at(&self.app, self.layout, self.note_drag, point);
            return;
        }
        if self.piano_keyboard_drag.is_some() {
            self.cursor_shape = CursorShape::ResizeNorthEastSouthWest;
            return;
        }
        if self.loop_end_drag.is_some() {
            self.cursor_shape = CursorShape::ResizeHorizontal;
            return;
        }
        if self.timeline_drag.is_some() {
            self.cursor_shape = CursorShape::Pointer;
            return;
        }
        if let Some(target) = workspace_resize_target_at_point(&self.app, self.layout, point) {
            self.cursor_shape = workspace_resize_cursor(target);
            return;
        }
        if loop_end_resize_mode_at_point(self.layout, point).is_some() {
            self.cursor_shape = CursorShape::ResizeHorizontal;
            return;
        }
        if let Some(mode) = piano_viewport_drag_mode_at_point(self.layout, point) {
            self.cursor_shape = piano_viewport_drag_cursor(mode);
            return;
        }
        self.cursor_shape = piano_cursor_shape_at(&self.app, self.layout, self.note_drag, point);
    }

    pub(super) fn drag_selected_note(&mut self, point: UiPoint) -> bool {
        let Some(drag) = self.note_drag.as_mut() else {
            return false;
        };
        let Some(layout) = self.layout else {
            return false;
        };
        let beat = layout.beat_at(point);
        let moved = match drag.mode {
            NoteDragMode::Move => self.app.drag_clip_note_to(
                drag.note_id,
                beat - drag.beat_offset,
                layout.pitch_at(point) - drag.pitch_offset,
                !drag.pushed_history,
            ),
            NoteDragMode::ResizeStart => {
                self.app
                    .resize_clip_note_start_to(drag.note_id, beat, !drag.pushed_history)
            }
            NoteDragMode::ResizeEnd => {
                self.app
                    .resize_clip_note_end_to(drag.note_id, beat, !drag.pushed_history)
            }
            NoteDragMode::Velocity => self.app.set_clip_note_velocity(
                drag.note_id,
                layout.velocity_at(point),
                !drag.pushed_history,
            ),
        };
        if moved {
            drag.pushed_history = true;
        }
        moved
    }

    pub(super) fn note_drag_for_pointer(
        &self,
        note_id: u64,
        mode: NoteDragMode,
        point: UiPoint,
    ) -> NoteDrag {
        let Some(layout) = self.layout else {
            return NoteDrag {
                note_id,
                mode,
                beat_offset: 0.0,
                pitch_offset: 0,
                pushed_history: false,
            };
        };
        let Some(note) = self.app.music_project.lock().note_by_id(note_id) else {
            return NoteDrag {
                note_id,
                mode,
                beat_offset: 0.0,
                pitch_offset: 0,
                pushed_history: false,
            };
        };
        let beat_offset = if matches!(mode, NoteDragMode::Move) {
            layout.beat_at(point) - note.start_beats
        } else {
            0.0
        };
        let pitch_offset = if matches!(mode, NoteDragMode::Move) {
            layout.pitch_at(point) - note.musical_note
        } else {
            0
        };
        NoteDrag {
            note_id,
            mode,
            beat_offset,
            pitch_offset,
            pushed_history: false,
        }
    }

    pub(super) fn seek_timeline(&mut self, point: UiPoint) -> bool {
        let Some(layout) = self.layout else {
            return false;
        };
        let Some(mode) = self.timeline_drag else {
            return false;
        };
        let beat = match mode {
            TimelineDragMode::Arrangement => layout.arrangement_beat_at(point),
            TimelineDragMode::Piano => layout.piano_ruler_beat_at(point),
        };
        self.app.seek_transport_to(beat);
        true
    }

    pub(super) fn resize_loop_end(&mut self, mode: LoopEndDragMode, point: UiPoint) -> bool {
        let Some(layout) = self.layout else {
            return false;
        };
        let action = match mode {
            LoopEndDragMode::Arrangement => "transport.loop_end",
            LoopEndDragMode::Piano => "piano.loop_end",
        };
        let Some(beat) = layout.loop_end_beat_at(action, point) else {
            return false;
        };
        self.app.set_loop_beats(beat)
    }

    pub(super) fn workspace_resize_drag_for_pointer(
        &self,
        target: WorkspaceResizeTarget,
        point: UiPoint,
    ) -> WorkspaceResizeDrag {
        let Some(layout) = self.layout else {
            return WorkspaceResizeDrag {
                target,
                grab_offset_px: 0.0,
            };
        };
        WorkspaceResizeDrag {
            target,
            grab_offset_px: layout.workspace_resize_grab_offset(&self.app, target, point),
        }
    }

    pub(super) fn drag_workspace_layout(&mut self, point: UiPoint, persist: bool) -> bool {
        let Some(drag) = self.workspace_resize_drag else {
            return false;
        };
        self.resize_workspace_layout_with_grab(drag.target, point, drag.grab_offset_px, persist)
    }

    #[cfg(test)]
    pub(super) fn resize_workspace_layout(
        &mut self,
        target: WorkspaceResizeTarget,
        point: UiPoint,
        persist: bool,
    ) -> bool {
        self.resize_workspace_layout_with_grab(target, point, 0.0, persist)
    }

    fn resize_workspace_layout_with_grab(
        &mut self,
        target: WorkspaceResizeTarget,
        point: UiPoint,
        grab_offset_px: f32,
        persist: bool,
    ) -> bool {
        let Some(layout) = self.layout else {
            return false;
        };
        let Some(value) = layout.workspace_resize_value(&self.app, target, point, grab_offset_px)
        else {
            return false;
        };
        self.app.set_workspace_layout_size(target, value, persist)
    }
}
