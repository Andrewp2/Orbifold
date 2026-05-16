use operad::platform::PixelSize;
use operad::{
    ApproxTextMeasurer, ColorRgba, CornerRadii, CursorRequest, CursorShape, EmptyResourceResolver,
    FontFamily, NativeCanvasInput, NativeKeyboardInput, NativeWgpuCanvasRenderRegistry,
    NativeWindowHooks, NativeWindowMetrics, NativeWindowOptions, PaintRect, PaintText,
    PlatformRequest, PointerButton, PointerEventKind, RawInputEvent, RawPointerEvent,
    RenderFrameRequest, RenderOptions, RenderTarget, RendererAdapter, ScenePrimitive, StrokeStyle,
    TextHorizontalAlign, TextStyle, TextVerticalAlign, UiDocument, UiPoint, UiRect, UiSize,
    UiVisual, WgpuRenderer, WheelPhase, WidgetAction, WidgetActionBinding, WidgetActionKind,
    WidgetValueEditPhase, layout, widgets,
};
#[cfg(test)]
use operad::{FocusDirection, PointerButtons, UiInputEvent, UiNodeId, WidgetActionMode};
#[cfg(test)]
use std::time::Duration;
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(test)]
use winit::dpi::PhysicalPosition;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
#[cfg(test)]
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::app::{AppState, AudioAssetKind};
use crate::project::ClipNote;
#[cfg(test)]
use crate::project::QuantizeGrid;

mod controls;
mod surfaces;
use controls::{
    add_button_at, add_button_at_with_visible_label, add_label_at, add_selectable_at,
    add_toggle_button_at,
};
pub(super) use surfaces::SurfaceRects;
use surfaces::piano_pitch_label_step;
use surfaces::{
    PIANO_INPUT_CANVAS_KEY, note_resize_edge_width, piano_note_rects, piano_velocity_hit_rects,
};
use surfaces::{add_center_editor_surfaces, surface_rects};
#[cfg(test)]
use surfaces::{piano_pitch_grid_line_step, visible_quantize_grid_step};

use super::accessibility::apply_focus_name;
#[cfg(test)]
use super::accessibility::button_accessibility_label;
#[cfg(test)]
use super::accessibility::{focused_node_name, keyboard_focus_status};
#[cfg(test)]
use super::actions::clamp_index;
use super::actions::{dispatch_action, handle_key};
use super::labels::{
    audio_connect_label, lumatone_map_label, midi_connect_label, midi_event_label, midi_note_name,
    pitch_label, selected_audio_output_name, selected_midi_input_name,
};
use super::text::{compact_label, estimated_text_width, fit_label};
use super::theme::{accent, color, muted, stroke, strong};

#[cfg(test)]
use super::accessibility::node_is_focusable_action;
#[cfg(test)]
use super::actions::{selected_device_status, shortcut_help_status};
#[cfg(test)]
use super::labels::{
    audio_output_status_label, device_connect_label, device_label_with_position,
    device_status_label, midi_input_status_label, selected_name_matches_connected,
};

const MIN_LAYOUT_WIDTH: f32 = 1200.0;
const MIN_LAYOUT_HEIGHT: f32 = 760.0;
const MIN_EFFECTIVE_UI_SCALE: f32 = 0.25;
const MIN_QUANTIZE_GRID_SPACING: f32 = 10.0;
const MIN_PITCH_GRID_SPACING: f32 = 8.0;
const MIN_POINTER_TARGET_SIZE: f32 = 24.0;
const MIN_EDITOR_TOP_HEIGHT: f32 = 320.0;
const MIN_BOTTOM_EDITOR_HEIGHT: f32 = 260.0;
const MAX_BOTTOM_EDITOR_HEIGHT: f32 = 520.0;
const PIANO_GRID_DOUBLE_CLICK_MILLIS: u64 = 500;
const PIANO_GRID_DOUBLE_CLICK_DISTANCE: f32 = 8.0;

pub(crate) fn run(
    app: AppState,
    exit_after_first_frame: bool,
    initial_window_size: Option<(f64, f64)>,
) -> Result<(), String> {
    if exit_after_first_frame {
        return write_startup_screenshot(&app, initial_window_size).map(|_| ());
    }

    let state = NativeOperadApp::new(app, false, None);
    let hooks = NativeWindowHooks::new()
        .with_initial_size(move |event_loop| {
            requested_or_monitor_window_size(initial_window_size, event_loop)
        })
        .with_title(|state: &NativeOperadApp| window_title_for_app(&state.app))
        .with_scale_factor(|state: &NativeOperadApp, metrics| {
            ui_scale_for_pixel_size(
                metrics.dpi_scale,
                metrics.physical_size.width,
                metrics.physical_size.height,
                state.app.ui_scale(),
            )
        })
        .with_close_requested(|state: &mut NativeOperadApp| state.app.request_quit())
        .with_keyboard_input(|state: &mut NativeOperadApp, input: NativeKeyboardInput| {
            state.handle_native_keyboard_input(input)
        })
        .with_canvas_input(|state: &mut NativeOperadApp, input: NativeCanvasInput| {
            state.handle_canvas_input(input)
        })
        .with_platform_requests(|state: &mut NativeOperadApp, _metrics| {
            state.cursor_platform_requests()
        })
        .with_before_render(
            |state: &mut NativeOperadApp, metrics: NativeWindowMetrics| {
                state.prepare_frame(metrics);
            },
        )
        .with_idle_redraw(|state: &NativeOperadApp| should_redraw_when_idle(&state.app));

    operad::run_app_with_canvas_renderers_and_hooks(
        NativeWindowOptions::new("Orbifold")
            .with_size(1400.0, MIN_LAYOUT_HEIGHT)
            .with_min_size(MIN_LAYOUT_WIDTH, MIN_LAYOUT_HEIGHT)
            .with_ui_scale(1.0),
        state,
        NativeOperadApp::update,
        NativeOperadApp::view,
        NativeWgpuCanvasRenderRegistry::new(),
        hooks,
    )
    .map_err(|error| error.to_string())
}

struct NativeOperadApp {
    app: AppState,
    #[cfg(test)]
    document: Option<UiDocument>,
    layout: Option<SurfaceRects>,
    cursor_pos: Option<UiPoint>,
    #[cfg(test)]
    pressed_action: Option<String>,
    note_drag: Option<NoteDrag>,
    timeline_drag: Option<TimelineDragMode>,
    piano_keyboard_drag: Option<PianoKeyboardDrag>,
    piano_grid_press: Option<PianoGridPress>,
    last_piano_grid_click: Option<PianoGridClick>,
    focused_action: Option<String>,
    cursor_shape: CursorShape,
    applied_cursor_shape: CursorShape,
    ui_scale: f32,
}

impl NativeOperadApp {
    fn new(
        app: AppState,
        _exit_after_first_frame: bool,
        _initial_window_size: Option<(f64, f64)>,
    ) -> Self {
        Self {
            app,
            #[cfg(test)]
            document: None,
            layout: None,
            cursor_pos: None,
            #[cfg(test)]
            pressed_action: None,
            note_drag: None,
            timeline_drag: None,
            piano_keyboard_drag: None,
            piano_grid_press: None,
            last_piano_grid_click: None,
            focused_action: None,
            cursor_shape: CursorShape::Default,
            applied_cursor_shape: CursorShape::Default,
            ui_scale: 1.0,
        }
    }

    fn prepare_frame(&mut self, metrics: NativeWindowMetrics) {
        self.app.poll_pending_file_dialog();
        self.ui_scale = metrics.scale_factor;
        self.layout = Some(surface_rects(
            &self.app,
            metrics.viewport.width,
            metrics.viewport.height,
        ));
        self.app.update_music_playback();
    }

    fn view(&self, viewport: UiSize) -> UiDocument {
        let mut document = build_surface_document(&self.app, viewport.width, viewport.height);
        apply_focus_name(&mut document, self.focused_action.as_deref());
        document
    }

    fn update(&mut self, action: WidgetAction) {
        let Some(action_name) = action_name_from_binding(&action.binding) else {
            return;
        };
        match action.kind {
            WidgetActionKind::Activate(_) => {
                dispatch_action(&mut self.app, &action_name, None, self.layout);
            }
            WidgetActionKind::PointerEdit(edit) => {
                self.handle_pointer_edit_action(&action_name, edit.phase, edit.position);
            }
            WidgetActionKind::Drag(drag) => {
                dispatch_action(&mut self.app, &action_name, Some(drag.current), self.layout);
            }
            _ => {}
        }
    }

    fn handle_pointer_edit_action(
        &mut self,
        action: &str,
        phase: WidgetValueEditPhase,
        point: UiPoint,
    ) {
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

        if action == "piano.grid" {
            if matches!(phase, WidgetValueEditPhase::Commit) {
                dispatch_action(&mut self.app, action, Some(point), self.layout);
            }
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

    fn handle_native_keyboard_input(&mut self, input: NativeKeyboardInput) -> bool {
        handle_key(
            &mut self.app,
            &input.logical_key,
            input.modifiers,
            input.repeat,
        )
    }

    fn handle_canvas_input(&mut self, input: NativeCanvasInput) -> bool {
        if input.key != PIANO_INPUT_CANVAS_KEY {
            return false;
        }
        match input.input {
            RawInputEvent::Pointer(pointer) => self.handle_piano_pointer_input(pointer),
            RawInputEvent::Wheel(wheel) => {
                if matches!(wheel.phase, WheelPhase::Ended) {
                    return true;
                }
                self.cursor_pos = Some(wheel.position);
                self.update_cursor_for_point(wheel.position);
                let Some(layout) = self.layout else {
                    return true;
                };
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
                let horizontal_px = if wheel.modifiers.shift {
                    if delta.x.abs() > f32::EPSILON {
                        delta.x
                    } else {
                        delta.y
                    }
                } else {
                    delta.x
                };
                let vertical_px = if wheel.modifiers.shift { 0.0 } else { delta.y };
                let delta_beats =
                    horizontal_px * layout.view_beats / layout.piano_grid.width.max(1.0);
                let delta_pitches = (-(vertical_px / layout.row_height().max(1.0))).round() as i32;
                let _ = self.app.scroll_piano_roll(delta_beats, delta_pitches);
                true
            }
            _ => false,
        }
    }

    fn handle_piano_pointer_input(&mut self, pointer: RawPointerEvent) -> bool {
        self.cursor_pos = Some(pointer.position);
        match pointer.kind {
            PointerEventKind::Move => self.handle_piano_pointer_move(pointer),
            PointerEventKind::Down(PointerButton::Primary) => {
                self.handle_piano_pointer_down(pointer)
            }
            PointerEventKind::Up(PointerButton::Primary) => self.handle_piano_pointer_up(pointer),
            PointerEventKind::Cancel => {
                self.note_drag = None;
                self.timeline_drag = None;
                self.piano_keyboard_drag = None;
                self.piano_grid_press = None;
                self.update_cursor_for_point(pointer.position);
                true
            }
            PointerEventKind::Down(_) | PointerEventKind::Up(_) => {
                self.update_cursor_for_point(pointer.position);
                false
            }
        }
    }

    fn handle_piano_pointer_down(&mut self, pointer: RawPointerEvent) -> bool {
        let Some(layout) = self.layout else {
            self.update_cursor_for_point(pointer.position);
            return false;
        };
        self.note_drag = None;
        self.timeline_drag = None;
        self.piano_keyboard_drag = None;
        self.piano_grid_press = None;

        if rect_contains_point(layout.piano_ruler, pointer.position) {
            self.timeline_drag = Some(TimelineDragMode::Piano);
            let _ = self.seek_timeline(pointer.position);
            self.update_cursor_for_point(pointer.position);
            return true;
        }

        if let Some((note_id, mode)) = piano_note_hit_at(&self.app, layout, pointer.position) {
            self.app.select_clip_note(Some(note_id));
            self.note_drag = Some(self.note_drag_for_pointer(note_id, mode, pointer.position));
            if matches!(mode, NoteDragMode::Velocity) {
                let _ = self.drag_selected_note(pointer.position);
            }
            self.update_cursor_for_point(pointer.position);
            return true;
        }

        if rect_contains_point(layout.piano_keyboard, pointer.position) {
            self.piano_keyboard_drag = Some(PianoKeyboardDrag {
                last_position: pointer.position,
                pitch_remainder_px: 0.0,
            });
            self.update_cursor_for_point(pointer.position);
            return true;
        }

        if rect_contains_point(layout.piano_grid, pointer.position) {
            self.piano_grid_press = Some(PianoGridPress {
                position: pointer.position,
            });
            self.update_cursor_for_point(pointer.position);
            return true;
        }

        self.update_cursor_for_point(pointer.position);
        false
    }

    fn handle_piano_pointer_move(&mut self, pointer: RawPointerEvent) -> bool {
        if self.note_drag.is_some() {
            let _ = self.drag_selected_note(pointer.position);
            self.update_cursor_for_point(pointer.position);
            return true;
        }
        if self.piano_keyboard_drag.is_some() {
            let _ = self.drag_piano_keyboard(pointer.position);
            self.update_cursor_for_point(pointer.position);
            return true;
        }
        if self.timeline_drag.is_some() {
            let _ = self.seek_timeline(pointer.position);
            self.update_cursor_for_point(pointer.position);
            return true;
        }
        if !pointer.buttons.contains(PointerButton::Primary) {
            self.piano_grid_press = None;
        }
        self.update_cursor_for_point(pointer.position);
        false
    }

    fn handle_piano_pointer_up(&mut self, pointer: RawPointerEvent) -> bool {
        if self.note_drag.is_some() {
            let _ = self.drag_selected_note(pointer.position);
            self.note_drag = None;
            self.update_cursor_for_point(pointer.position);
            return true;
        }

        if self.piano_keyboard_drag.is_some() {
            let _ = self.drag_piano_keyboard(pointer.position);
            self.piano_keyboard_drag = None;
            self.update_cursor_for_point(pointer.position);
            return true;
        }

        if self.timeline_drag.is_some() {
            let _ = self.seek_timeline(pointer.position);
            self.timeline_drag = None;
            self.update_cursor_for_point(pointer.position);
            return true;
        }

        let Some(press) = self.piano_grid_press.take() else {
            self.update_cursor_for_point(pointer.position);
            return false;
        };
        let Some(layout) = self.layout else {
            self.update_cursor_for_point(pointer.position);
            return false;
        };
        if !rect_contains_point(layout.piano_grid, pointer.position)
            || point_distance(press.position, pointer.position) > PIANO_GRID_DOUBLE_CLICK_DISTANCE
        {
            self.update_cursor_for_point(pointer.position);
            return true;
        }

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
        self.update_cursor_for_point(pointer.position);
        true
    }

    fn is_piano_grid_double_click(&self, point: UiPoint, timestamp_millis: u64) -> bool {
        self.last_piano_grid_click.is_some_and(|click| {
            timestamp_millis.saturating_sub(click.timestamp_millis)
                <= PIANO_GRID_DOUBLE_CLICK_MILLIS
                && point_distance(click.position, point) <= PIANO_GRID_DOUBLE_CLICK_DISTANCE
        })
    }

    fn drag_piano_keyboard(&mut self, point: UiPoint) -> bool {
        let Some(layout) = self.layout else {
            return false;
        };
        let row_height = layout.row_height().max(1.0);
        let Some((delta_x, row_delta)) = self.piano_keyboard_drag.as_mut().map(|drag| {
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

    fn cursor_platform_requests(&mut self) -> Vec<PlatformRequest> {
        if self.cursor_shape == self.applied_cursor_shape {
            return Vec::new();
        }
        self.applied_cursor_shape = self.cursor_shape;
        vec![PlatformRequest::Cursor(CursorRequest::SetShape(
            self.cursor_shape,
        ))]
    }

    fn update_cursor_for_point(&mut self, point: UiPoint) {
        if self.piano_keyboard_drag.is_some() {
            self.cursor_shape = CursorShape::ResizeNorthEastSouthWest;
            return;
        }
        self.cursor_shape = piano_cursor_shape_at(&self.app, self.layout, self.note_drag, point);
    }

    #[cfg(test)]
    fn press_pointer(&mut self, point: UiPoint) {
        let Some(action) = self
            .document
            .as_ref()
            .and_then(|document| hit_action_name(document, point))
        else {
            self.pressed_action = None;
            self.note_drag = None;
            self.timeline_drag = None;
            self.piano_keyboard_drag = None;
            return;
        };
        self.pressed_action = Some(action.clone());
        self.timeline_drag = TimelineDragMode::from_action(&action);
        self.note_drag = note_drag_from_action(&action).map(|(note_id, mode)| {
            self.app.select_clip_note(Some(note_id));
            self.note_drag_for_pointer(note_id, mode, point)
        });
        let Some(document) = self.document.as_mut() else {
            return;
        };
        let _ = document.handle_input(UiInputEvent::PointerDown(point));
        self.focused_action = focused_node_name(document);
        if matches!(
            self.note_drag.as_ref().map(|drag| drag.mode),
            Some(NoteDragMode::Velocity)
        ) {
            let _ = self.drag_selected_note(point);
        }
        if self.timeline_drag.is_some() {
            let _ = self.seek_timeline(point);
        }
    }

    fn drag_selected_note(&mut self, point: UiPoint) -> bool {
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

    fn note_drag_for_pointer(&self, note_id: u64, mode: NoteDragMode, point: UiPoint) -> NoteDrag {
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

    fn seek_timeline(&mut self, point: UiPoint) -> bool {
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

    #[cfg(test)]
    fn release_pointer(&mut self, point: UiPoint) {
        if self.timeline_drag.is_some() {
            if let Some(document) = self.document.as_mut() {
                let _ = document.handle_input(UiInputEvent::PointerUp(point));
            }
            self.timeline_drag = None;
            self.pressed_action = None;
            return;
        }
        if self.note_drag.take().is_some() {
            if let Some(document) = self.document.as_mut() {
                let _ = document.handle_input(UiInputEvent::PointerUp(point));
            }
            self.pressed_action = None;
            return;
        }
        let pressed_action = self.pressed_action.clone();
        let action = self.document.as_mut().and_then(|document| {
            let released_action = hit_action_name(document, point);
            let result = document.handle_input(UiInputEvent::PointerUp(point));
            result
                .clicked
                .and_then(|clicked| node_action_name(document, clicked))
                .or_else(|| {
                    if released_action.is_some()
                        && released_action.as_ref() == pressed_action.as_ref()
                    {
                        released_action
                    } else {
                        None
                    }
                })
        });
        self.pressed_action = None;
        if let Some(action) = action {
            let cursor = self.cursor_pos;
            dispatch_action(&mut self.app, &action, cursor, self.layout);
        }
    }

    #[cfg(test)]
    fn handle_keyboard_focus_key(
        &mut self,
        key: &Key,
        modifiers: ModifiersState,
        repeat: bool,
    ) -> bool {
        if repeat || modifiers.control_key() || modifiers.super_key() || modifiers.alt_key() {
            return false;
        }
        match key {
            Key::Named(NamedKey::Tab) => self.move_keyboard_focus(if modifiers.shift_key() {
                FocusDirection::Previous
            } else {
                FocusDirection::Next
            }),
            Key::Named(NamedKey::Enter) if !modifiers.shift_key() => self.activate_keyboard_focus(),
            _ => false,
        }
    }

    #[cfg(test)]
    fn move_keyboard_focus(&mut self, direction: FocusDirection) -> bool {
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        let result = document.handle_input(UiInputEvent::Focus(direction));
        let Some(focused) = result.focused else {
            self.focused_action = None;
            return false;
        };
        self.focused_action = Some(document.node(focused).name.clone());
        self.app.last_status = keyboard_focus_status(document, focused);
        true
    }

    #[cfg(test)]
    fn activate_keyboard_focus(&mut self) -> bool {
        let action = if let Some(document) = self.document.as_ref() {
            focused_node_name(document)
        } else {
            self.focused_action.clone()
        };
        let Some(action) = action else {
            return false;
        };
        dispatch_action(&mut self.app, &action, self.cursor_pos, self.layout);
        true
    }

    #[cfg(test)]
    fn scroll_left_browser_list(&mut self, point: UiPoint, delta_y: f32) -> bool {
        if delta_y.abs() <= f32::EPSILON {
            return false;
        }
        let Some(layout) = self.layout else {
            return false;
        };
        let sections = left_browser_rects(&self.app, layout.left);
        let direction = if delta_y < 0.0 { 1 } else { -1 };
        if rect_contains_point(sections.scales, point) {
            return self.scroll_scale_library(direction);
        }
        if rect_contains_point(sections.assets, point) {
            return self.scroll_audio_assets(direction);
        }
        false
    }

    #[cfg(test)]
    fn scroll_scale_library(&mut self, direction: isize) -> bool {
        if self.app.scale_library.is_empty() {
            return false;
        }
        let current = self
            .app
            .selected_scale_library
            .min(self.app.scale_library.len().saturating_sub(1));
        let next = clamp_index(current, self.app.scale_library.len(), direction);
        if next == current {
            return false;
        }
        self.app.selected_scale_library = next;
        self.app.last_status = format!("Selected scale: {}", self.app.scale_library[next].name);
        true
    }

    #[cfg(test)]
    fn scroll_audio_assets(&mut self, direction: isize) -> bool {
        let indices = self
            .app
            .audio_assets
            .iter()
            .enumerate()
            .filter_map(|(idx, asset)| {
                (asset.kind == self.app.selected_audio_asset_kind).then_some(idx)
            })
            .collect::<Vec<_>>();
        if indices.is_empty() {
            return false;
        }
        let Some(current_pos) = self
            .app
            .selected_audio_asset
            .and_then(|selected| indices.iter().position(|idx| *idx == selected))
        else {
            self.app.select_audio_asset(indices[0]);
            return true;
        };
        let next_pos = clamp_index(current_pos, indices.len(), direction);
        if next_pos == current_pos {
            return false;
        }
        self.app.select_audio_asset(indices[next_pos]);
        true
    }
}

fn action_name_from_binding(binding: &WidgetActionBinding) -> Option<String> {
    binding
        .action_id()
        .map(|action| action.as_str().to_string())
}

fn requested_or_monitor_window_size(
    requested: Option<(f64, f64)>,
    event_loop: &ActiveEventLoop,
) -> UiSize {
    let size = requested
        .map(|(width, height)| LogicalSize::new(width, height))
        .unwrap_or_else(|| initial_window_size(event_loop));
    UiSize::new(size.width as f32, size.height as f32)
}

fn initial_window_size(event_loop: &ActiveEventLoop) -> LogicalSize<f64> {
    event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
        .map(|monitor| initial_window_size_for_monitor(monitor.size(), monitor.scale_factor()))
        .unwrap_or_else(|| LogicalSize::new(1400.0, MIN_LAYOUT_HEIGHT as f64))
}

fn initial_window_size_for_monitor(
    monitor_size: PhysicalSize<u32>,
    scale_factor: f64,
) -> LogicalSize<f64> {
    let scale = scale_factor.max(1.0);
    let logical_width = monitor_size.width as f64 / scale;
    let logical_height = monitor_size.height as f64 / scale;
    LogicalSize::new(
        (logical_width * 0.9).clamp(1400.0, 3200.0),
        (logical_height * 0.88).clamp(MIN_LAYOUT_HEIGHT as f64, 1900.0),
    )
}

fn window_title_for_app(app: &AppState) -> String {
    match app.project_path.as_ref() {
        Some(path) if app.project_dirty => {
            format!("Orbifold - {} *", project_name_from_path(path))
        }
        Some(path) => format!("Orbifold - {}", project_name_from_path(path)),
        None if app.project_dirty => "Orbifold - Untitled *".to_string(),
        None => "Orbifold".to_string(),
    }
}

fn project_name_from_path(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Untitled")
        .to_string()
}

#[derive(Clone, Copy, Debug)]
struct NoteDrag {
    note_id: u64,
    mode: NoteDragMode,
    beat_offset: f32,
    pitch_offset: i32,
    pushed_history: bool,
}

#[derive(Clone, Copy, Debug)]
struct PianoKeyboardDrag {
    last_position: UiPoint,
    pitch_remainder_px: f32,
}

#[derive(Clone, Copy, Debug)]
struct PianoGridPress {
    position: UiPoint,
}

#[derive(Clone, Copy, Debug)]
struct PianoGridClick {
    position: UiPoint,
    timestamp_millis: u64,
}

#[derive(Clone, Copy, Debug)]
enum NoteDragMode {
    Move,
    ResizeStart,
    ResizeEnd,
    Velocity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TimelineDragMode {
    Arrangement,
    Piano,
}

impl TimelineDragMode {
    fn from_action(action: &str) -> Option<Self> {
        match action {
            "transport.seek" => Some(Self::Arrangement),
            "piano.seek" => Some(Self::Piano),
            _ => None,
        }
    }
}

#[cfg(test)]
fn hit_action_name(document: &UiDocument, point: UiPoint) -> Option<String> {
    let hit = document.hit_test(point)?;
    node_action_name(document, hit)
}

#[cfg(test)]
fn node_action_name(document: &UiDocument, id: UiNodeId) -> Option<String> {
    document
        .node(id)
        .action
        .as_ref()
        .and_then(|binding| binding.action_id())
        .map(|action| action.as_str().to_string())
}

fn rect_contains_point(rect: UiRect, point: UiPoint) -> bool {
    point.x >= rect.x && point.x <= rect.right() && point.y >= rect.y && point.y <= rect.bottom()
}

fn point_distance(a: UiPoint, b: UiPoint) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

fn piano_cursor_shape_at(
    app: &AppState,
    layout: Option<SurfaceRects>,
    active_drag: Option<NoteDrag>,
    point: UiPoint,
) -> CursorShape {
    if let Some(drag) = active_drag {
        return match drag.mode {
            NoteDragMode::Move => CursorShape::Grabbing,
            NoteDragMode::ResizeStart | NoteDragMode::ResizeEnd => CursorShape::ResizeHorizontal,
            NoteDragMode::Velocity => CursorShape::ResizeVertical,
        };
    }
    let Some(layout) = layout else {
        return CursorShape::Default;
    };
    if let Some((_, mode)) = piano_note_hit_at(app, layout, point) {
        return match mode {
            NoteDragMode::Move => CursorShape::Grab,
            NoteDragMode::ResizeStart | NoteDragMode::ResizeEnd => CursorShape::ResizeHorizontal,
            NoteDragMode::Velocity => CursorShape::ResizeVertical,
        };
    }
    if rect_contains_point(layout.piano_ruler, point) {
        return CursorShape::Pointer;
    }
    if rect_contains_point(layout.piano_keyboard, point) {
        return CursorShape::ResizeNorthEastSouthWest;
    }
    if rect_contains_point(layout.piano_grid, point) {
        return CursorShape::Crosshair;
    }
    CursorShape::Default
}

fn piano_note_hit_at(
    app: &AppState,
    layout: SurfaceRects,
    point: UiPoint,
) -> Option<(u64, NoteDragMode)> {
    let notes = app.music_project.lock().clip.notes.clone();
    for note in notes.iter().rev() {
        for velocity_rect in piano_velocity_hit_rects(note.clone(), layout) {
            if rect_contains_point(velocity_rect, point) {
                return Some((note.id, NoteDragMode::Velocity));
            }
        }
        for note_rect in piano_note_rects(note.clone(), layout) {
            if !rect_contains_point(note_rect, point) {
                continue;
            }
            if let Some(edge_w) = note_resize_edge_width(note_rect.width) {
                if point.x <= note_rect.x + edge_w {
                    return Some((note.id, NoteDragMode::ResizeStart));
                }
                if point.x >= note_rect.right() - edge_w {
                    return Some((note.id, NoteDragMode::ResizeEnd));
                }
            }
            return Some((note.id, NoteDragMode::Move));
        }
    }
    None
}

fn note_drag_from_action(action: &str) -> Option<(u64, NoteDragMode)> {
    if let Some(id) = action.strip_prefix("note.velocity.") {
        return id.parse().ok().map(|id| (id, NoteDragMode::Velocity));
    }
    if let Some(id) = action.strip_prefix("note.resize_start.") {
        return id.parse().ok().map(|id| (id, NoteDragMode::ResizeStart));
    }
    if let Some(id) = action.strip_prefix("note.resize_end.") {
        return id.parse().ok().map(|id| (id, NoteDragMode::ResizeEnd));
    }
    action
        .strip_prefix("note.select.")?
        .parse()
        .ok()
        .map(|id| (id, NoteDragMode::Move))
}

fn should_redraw_when_idle(app: &AppState) -> bool {
    app.music_project.lock().transport.playing || app.has_pending_file_dialog()
}

fn write_startup_screenshot(
    app: &AppState,
    requested_size: Option<(f64, f64)>,
) -> Result<PathBuf, String> {
    let fallback_size = PhysicalSize::new(1400, MIN_LAYOUT_HEIGHT as u32);
    let size = screenshot_physical_size(requested_size, fallback_size);
    let ui_scale = screenshot_ui_scale_for_values(1.0, requested_size, size, app.ui_scale());
    let logical_size = logical_size_for_window(size, ui_scale);
    let path = write_operad_screenshot(app, size, logical_size, ui_scale)?;
    log::info!("Wrote screenshot to {}", path.display());
    Ok(path)
}

fn write_operad_screenshot(
    app: &AppState,
    size: PhysicalSize<u32>,
    logical_size: UiSize,
    ui_scale: f32,
) -> Result<PathBuf, String> {
    let viewport = logical_size;
    let mut document = build_surface_document(app, logical_size.width, logical_size.height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(logical_size, &mut text_measurer)
        .map_err(|error| error.to_string())?;
    let options = RenderOptions {
        clear_color: color(8, 12, 18),
        scale_factor: ui_scale,
        ..Default::default()
    };
    let request = RenderFrameRequest::new(
        RenderTarget::snapshot(PixelSize::new(size.width.max(1), size.height.max(1))),
        viewport,
        document.paint_list(),
    )
    .options(options);
    let output = WgpuRenderer::new()
        .render_frame(request, &EmptyResourceResolver)
        .map_err(|error| error.to_string())?;
    let image = output
        .snapshot
        .ok_or_else(|| "snapshot render did not return image data".to_string())?;
    validate_screenshot_pixels(
        image.size.width,
        image.size.height,
        &image.pixels,
        color(8, 12, 18),
    )?;
    let path = next_screenshot_path()?;
    write_png_rgba(&path, image.size.width, image.size.height, &image.pixels)?;
    let latest = Path::new("screenshots").join("latest.png");
    write_png_rgba(&latest, image.size.width, image.size.height, &image.pixels)?;
    Ok(path)
}

fn validate_screenshot_pixels(
    width: u32,
    height: u32,
    pixels: &[u8],
    background: ColorRgba,
) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("screenshot image has zero-sized dimensions".to_string());
    }
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixel_count| pixel_count.checked_mul(4))
        .ok_or_else(|| "screenshot image dimensions overflow pixel buffer size".to_string())?;
    if pixels.len() != expected_len {
        return Err(format!(
            "screenshot pixel buffer has {} bytes; expected {expected_len}",
            pixels.len()
        ));
    }

    let mut active_count = 0_usize;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;
    for y in 0..height {
        for x in 0..width {
            let index = ((y as usize * width as usize) + x as usize) * 4;
            if screenshot_pixel_is_active(&pixels[index..index + 4], background) {
                active_count += 1;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    let pixel_count = (width as usize) * (height as usize);
    let minimum_active = (pixel_count / 200).max(1);
    if active_count < minimum_active {
        return Err(format!(
            "screenshot appears blank: only {active_count} active pixels"
        ));
    }

    let active_width = max_x - min_x + 1;
    let active_height = max_y - min_y + 1;
    let width_coverage = active_width as f32 / width as f32;
    let height_coverage = active_height as f32 / height as f32;
    if width_coverage < 0.9 || height_coverage < 0.9 {
        return Err(format!(
            "screenshot content appears cropped: active bounds cover {:.0}% x {:.0}% of image",
            width_coverage * 100.0,
            height_coverage * 100.0
        ));
    }

    Ok(())
}

fn screenshot_pixel_is_active(pixel: &[u8], background: ColorRgba) -> bool {
    let color_distance = (pixel[0] as i16 - background.r as i16).abs()
        + (pixel[1] as i16 - background.g as i16).abs()
        + (pixel[2] as i16 - background.b as i16).abs();
    pixel[3] > 0 && color_distance > 12
}

fn next_screenshot_path() -> Result<PathBuf, String> {
    let directory = Path::new("screenshots");
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    Ok(directory.join(format!("ui-{timestamp}.png")))
}

fn write_png_rgba(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    image::save_buffer_with_format(
        path,
        pixels,
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|error| error.to_string())
}

fn ui_scale_for_pixel_size(dpi_scale: f32, width: u32, height: u32, user_scale: f32) -> f32 {
    ui_scale_for_values(dpi_scale, PhysicalSize::new(width, height), user_scale)
}

fn ui_scale_for_values(dpi_scale: f32, size: PhysicalSize<u32>, user_scale: f32) -> f32 {
    let large_screen_scale = if size.width >= 3600 || size.height >= 2000 {
        2.0
    } else if size.width >= 3000 || size.height >= 1700 {
        1.6
    } else if size.width >= 2400 || size.height >= 1400 {
        1.25
    } else {
        1.0
    };
    let display_scale = dpi_scale.max(large_screen_scale).max(1.0);
    let user_scale = user_scale.clamp(0.75, 2.0);
    let requested_scale = (display_scale * user_scale).clamp(0.75, 3.0);
    requested_scale.min(max_ui_scale_for_minimum_layout(size))
}

fn max_ui_scale_for_minimum_layout(size: PhysicalSize<u32>) -> f32 {
    let width_scale = size.width.max(1) as f32 / MIN_LAYOUT_WIDTH;
    let height_scale = size.height.max(1) as f32 / MIN_LAYOUT_HEIGHT;
    width_scale.min(height_scale).max(1.0)
}

fn logical_size_for_window(size: PhysicalSize<u32>, ui_scale: f32) -> UiSize {
    let scale = effective_ui_scale(ui_scale);
    UiSize::new(
        size.width.max(1) as f32 / scale,
        size.height.max(1) as f32 / scale,
    )
}

fn effective_ui_scale(ui_scale: f32) -> f32 {
    ui_scale.max(MIN_EFFECTIVE_UI_SCALE)
}

fn screenshot_physical_size(
    requested_size: Option<(f64, f64)>,
    actual_window_size: PhysicalSize<u32>,
) -> PhysicalSize<u32> {
    requested_size
        .map(|(width, height)| {
            PhysicalSize::new(screenshot_dimension(width), screenshot_dimension(height))
        })
        .unwrap_or(actual_window_size)
}

fn screenshot_dimension(value: f64) -> u32 {
    value.round().clamp(1.0, u32::MAX as f64) as u32
}

fn screenshot_ui_scale_for_values(
    window_dpi_scale: f32,
    requested_size: Option<(f64, f64)>,
    screenshot_size: PhysicalSize<u32>,
    user_scale: f32,
) -> f32 {
    let dpi_scale = if requested_size.is_some() {
        1.0
    } else {
        window_dpi_scale
    };
    ui_scale_for_values(dpi_scale, screenshot_size, user_scale)
}

fn build_surface_document(app: &AppState, width: f32, height: f32) -> UiDocument {
    let mut document = UiDocument::new(operad::root_style(width, height));
    document.node_mut(document.root).visual = UiVisual::panel(color(8, 12, 18), None, 0.0);
    let root = document.root;
    let rects = surface_rects(app, width, height);
    widgets::scene(
        &mut document,
        root,
        "orbifold.native.surface",
        build_surface_primitives(app, width, height),
        widgets::SceneOptions::default()
            .with_layout(layout::fixed(width, height))
            .accessibility_label("Orbifold main workspace"),
    );
    add_center_editor_surfaces(&mut document, root, app, rects);
    add_operad_controls(&mut document, app, width, height);
    add_piano_panel_button_labels(&mut document, app, rects.piano_options);
    add_piano_text_overlay(&mut document, app, width, height);
    document
}

fn build_surface_primitives(app: &AppState, width: f32, height: f32) -> Vec<ScenePrimitive> {
    let mut primitives = Vec::new();
    let top_h = 62.0;
    let bottom_h = 26.0;
    let body = body_rects(width, height, top_h, bottom_h);

    push_rect(
        &mut primitives,
        UiRect::new(0.0, 0.0, width, height),
        color(8, 12, 18),
        0.0,
        None,
    );
    draw_top_bar(&mut primitives, width, top_h);
    draw_left_browser(&mut primitives, body.left, app);
    draw_track_list(&mut primitives, body.track, app);
    draw_right_tools(&mut primitives, body.right, app);
    draw_status_bar(&mut primitives, width, height - bottom_h, bottom_h, app);
    primitives
}

fn add_piano_text_overlay(document: &mut UiDocument, app: &AppState, width: f32, height: f32) {
    let rects = surface_rects(app, width, height);
    let mut primitives = Vec::new();
    add_piano_keyboard_label_text(&mut primitives, app, rects);

    widgets::scene(
        document,
        document.root,
        "orbifold.native.piano.text_overlay",
        primitives,
        widgets::SceneOptions::default()
            .with_layout(layout::absolute(0.0, 0.0, width, height))
            .accessibility_label("Piano roll labels"),
    );
}

fn add_piano_panel_button_labels(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    for button in piano_panel_button_specs(app, panel) {
        let color = if button.enabled { strong() } else { muted() };
        let text = fit_label(&button.label, button.rect.width - 8.0, 12.0);
        if text.is_empty() {
            continue;
        }
        let text_w = estimated_text_width(&text, 12.0).min(button.rect.width - 8.0);
        let label_rect = UiRect::new(
            button.rect.x + ((button.rect.width - text_w) * 0.5).max(4.0),
            button.rect.y + ((button.rect.height - 16.0) * 0.5).max(0.0),
            text_w.max(1.0),
            16.0,
        );
        widgets::label(
            document,
            document.root,
            format!("{}.overlay_label", button.name),
            text,
            TextStyle {
                font_size: 12.0,
                line_height: 16.0,
                family: FontFamily::SansSerif,
                color,
                ..Default::default()
            },
            layout::absolute(
                label_rect.x,
                label_rect.y,
                label_rect.width,
                label_rect.height,
            ),
        );
    }
}

fn add_piano_keyboard_label_text(
    primitives: &mut Vec<ScenePrimitive>,
    app: &AppState,
    rects: SurfaceRects,
) {
    let pitch_count = (rects.max_pitch - rects.min_pitch + 1).max(1);
    let row_height = rects.row_height();
    let label_step = piano_pitch_label_step(pitch_count, row_height);
    for row in 0..pitch_count {
        if row % label_step != 0 {
            continue;
        }
        let y = rects.piano_grid.y + row_height * row as f32;
        let pitch = rects.max_pitch - row;
        let black_key = matches!(pitch.rem_euclid(12), 1 | 3 | 6 | 8 | 10);
        let bg = if black_key {
            color(18, 19, 21)
        } else {
            color(35, 36, 39)
        };
        let mask = UiRect::new(
            rects.piano_keyboard.x + 4.0,
            y + 1.0,
            (rects.keyboard_width - 8.0).max(1.0),
            (row_height - 2.0).max(12.0),
        );
        push_rect(primitives, mask, bg, 0.0, None);
        push_text(
            primitives,
            UiRect::new(
                rects.piano_keyboard.x + 10.0,
                y + 1.0,
                (rects.keyboard_width - 14.0).max(1.0),
                row_height.max(12.0),
            ),
            pitch_label(app, pitch),
            10.0,
            color(184, 188, 194),
            TextHorizontalAlign::Start,
        );
    }
}

#[derive(Clone, Copy, Debug)]
struct BodyRects {
    left: UiRect,
    track: UiRect,
    center: UiRect,
    right: UiRect,
}

fn body_rects(width: f32, height: f32, top_h: f32, bottom_h: f32) -> BodyRects {
    let gap = 8.0;
    let body_top = top_h + gap;
    let body_bottom = height - bottom_h - gap;
    let full_body_h = (body_bottom - body_top).max(1.0);
    let editor_h = bottom_editor_height(full_body_h, gap);
    let top_body_h = (full_body_h - editor_h - gap).max(1.0);
    let compact = width < 1320.0;
    let mut left_w = if compact {
        (width * 0.19).clamp(200.0, 230.0)
    } else {
        width.mul_add(0.12, 72.0).clamp(220.0, 260.0)
    };
    let mut track_w = if compact {
        (width * 0.14).clamp(160.0, 190.0)
    } else {
        width.mul_add(0.06, 96.0).clamp(176.0, 210.0)
    };
    let mut right_w = if compact {
        (width * 0.21).clamp(240.0, 270.0)
    } else {
        width.mul_add(0.10, 110.0).clamp(260.0, 310.0)
    };
    let min_center_w = if width < 1100.0 { 280.0 } else { 360.0 };
    let side_available = (width - gap * 5.0 - min_center_w).max(1.0);
    let side_total = left_w + track_w + right_w;
    if side_total > side_available {
        let factor = side_available / side_total;
        left_w *= factor;
        track_w *= factor;
        right_w *= factor;
    }
    let center_w = (width - left_w - track_w - right_w - gap * 5.0).max(1.0);
    let left = UiRect::new(gap, body_top, left_w, top_body_h);
    let track = UiRect::new(left.right() + gap, body_top, track_w, top_body_h);
    let center = UiRect::new(track.right() + gap, body_top, center_w, top_body_h);
    let right = UiRect::new(center.right() + gap, body_top, right_w, full_body_h);
    BodyRects {
        left,
        track,
        center,
        right,
    }
}

fn bottom_editor_height(body_h: f32, gap: f32) -> f32 {
    let desired = (body_h * 0.44).clamp(MIN_BOTTOM_EDITOR_HEIGHT, MAX_BOTTOM_EDITOR_HEIGHT);
    let max_with_top = (body_h - MIN_EDITOR_TOP_HEIGHT - gap).max(MIN_BOTTOM_EDITOR_HEIGHT);
    desired.min(max_with_top).min((body_h - gap).max(1.0))
}

#[derive(Clone, Copy, Debug)]
struct LeftBrowserRects {
    module_bar: UiRect,
    scales: UiRect,
    assets: UiRect,
}

fn left_browser_rects(app: &AppState, rect: UiRect) -> LeftBrowserRects {
    let gap = 8.0;
    let module_bar = UiRect::new(rect.x, rect.y, rect.width, 42.0_f32.min(rect.height));
    let content_y = module_bar.bottom() + gap;
    let content_h = (rect.bottom() - content_y).max(0.0);
    let scales_h = if app.show_scale_browser {
        (content_h * 0.48).clamp(154.0, 180.0).min(content_h)
    } else {
        0.0
    };
    let scales = UiRect::new(rect.x, content_y, rect.width, scales_h);
    let asset_y = if app.show_scale_browser {
        scales.bottom() + gap
    } else {
        content_y
    };
    let asset_h = if app.show_asset_browser {
        (rect.bottom() - asset_y).max(0.0)
    } else {
        0.0
    };
    let assets = UiRect::new(rect.x, asset_y, rect.width, asset_h);
    LeftBrowserRects {
        module_bar,
        scales,
        assets,
    }
}

fn add_operad_controls(document: &mut UiDocument, app: &AppState, width: f32, height: f32) {
    let rects = surface_rects(app, width, height);
    let body = body_rects(width, height, 62.0, 26.0);
    let project = app.music_project.lock();
    let playing = project.transport.playing;
    let recording = project.transport.recording;
    let overdub = project.transport.overdub;
    let metronome_enabled = project.transport.metronome_enabled;
    let bpm = project.transport.bpm;
    let loop_beats = project.transport.loop_beats;
    let quantize_grid = project.transport.quantize_grid;
    let quantize_on_record = project.transport.quantize_on_record;
    let current_beat = project.current_position_beats(std::time::Instant::now());
    drop(project);
    let scale = app.scale_state.lock().clone();
    let synth = app.synth.settings();
    let synth_muted = app.synth.muted();
    let synth_limited = app.synth.output_limited();
    let selected_midi = selected_midi_input_name(app);
    let selected_audio = selected_audio_output_name(app);
    let audio_available = app.audio_stream.is_some();
    let selected_asset_kind = app.selected_audio_asset_kind;
    let capture = app.midi_capture.lock();
    let capture_armed = capture.is_armed();
    let capture_events = capture.events();
    let capture_count = capture_events.len();
    let capture_last = capture_events.last().map(midi_event_label);
    drop(capture);
    let capture_actions = capture_action_state(capture_armed, capture_count);
    let midi_last = app
        .midi_last
        .lock()
        .clone()
        .map(|event| midi_event_label(&event));
    let lumatone_status = lumatone_map_label(app);
    let compact_top = width < 1340.0;

    add_button_at(
        document,
        "file.new",
        if app.new_project_confirm_pending() {
            "Discard?"
        } else {
            "New"
        },
        UiRect::new(12.0, 18.0, 62.0, 30.0),
        app.new_project_confirm_pending(),
        true,
    );
    add_button_at(
        document,
        "file.open",
        if app.open_project_confirm_pending() {
            "Discard?"
        } else {
            "Open"
        },
        UiRect::new(80.0, 18.0, 62.0, 30.0),
        app.open_project_confirm_pending(),
        true,
    );
    add_button_at(
        document,
        "file.save",
        "Save",
        UiRect::new(148.0, 18.0, 48.0, 30.0),
        false,
        true,
    );
    add_button_at(
        document,
        "file.save_as",
        "Save As",
        UiRect::new(202.0, 18.0, 62.0, 30.0),
        false,
        true,
    );
    add_button_at(
        document,
        "scale.open",
        "Scale",
        UiRect::new(270.0, 18.0, 52.0, 30.0),
        false,
        true,
    );
    add_button_at(
        document,
        "keymap.open",
        "Keys",
        UiRect::new(328.0, 18.0, 46.0, 30.0),
        false,
        true,
    );
    add_button_at(
        document,
        "edit.undo",
        "Undo",
        UiRect::new(380.0, 18.0, 46.0, 30.0),
        false,
        app.can_undo_project_edit(),
    );
    add_button_at(
        document,
        "edit.redo",
        "Redo",
        UiRect::new(432.0, 18.0, 46.0, 30.0),
        false,
        app.can_redo_project_edit(),
    );
    add_button_at(
        document,
        "transport.prev",
        "Start",
        UiRect::new(486.0, 18.0, 52.0, 30.0),
        false,
        true,
    );
    add_toggle_button_at(
        document,
        "transport.play_stop",
        if playing { "Pause" } else { "Play" },
        UiRect::new(544.0, 18.0, 52.0, 30.0),
        playing,
        true,
    );
    add_button_at(
        document,
        "transport.stop",
        "Stop",
        UiRect::new(602.0, 18.0, 52.0, 30.0),
        false,
        true,
    );
    add_toggle_button_at(
        document,
        "transport.record",
        if recording { "Stop Rec" } else { "Record" },
        UiRect::new(660.0, 18.0, 68.0, 30.0),
        recording,
        true,
    );
    add_toggle_button_at(
        document,
        "transport.loop",
        if overdub { "Overdub" } else { "Replace" },
        UiRect::new(734.0, 18.0, 66.0, 30.0),
        overdub,
        true,
    );
    add_button_at(
        document,
        "transport.bpm_down",
        "-",
        UiRect::new(792.0, 18.0, 28.0, 30.0),
        false,
        true,
    );
    add_button_at(
        document,
        "transport.bpm_up",
        "+",
        UiRect::new(908.0, 18.0, 28.0, 30.0),
        false,
        true,
    );
    if !compact_top {
        add_button_at(
            document,
            "transport.loop_down",
            "-",
            UiRect::new(944.0, 18.0, 28.0, 30.0),
            false,
            true,
        );
        add_button_at(
            document,
            "transport.loop_up",
            "+",
            UiRect::new(1050.0, 18.0, 28.0, 30.0),
            false,
            true,
        );
    }
    add_button_at(
        document,
        "transport.quantize_grid",
        quantize_grid.as_str(),
        UiRect::new(if compact_top { 944.0 } else { 1088.0 }, 18.0, 58.0, 30.0),
        false,
        true,
    );
    if width >= 1500.0 {
        add_label_at(
            document,
            "readout.meter",
            "4/4",
            UiRect::new(1160.0, 18.0, 44.0, 30.0),
            true,
        );
    }
    if width >= 1600.0 {
        add_label_at(
            document,
            "readout.position",
            transport_position_label(current_beat),
            UiRect::new(1210.0, 18.0, 76.0, 30.0),
            false,
        );
    }
    add_button_at(
        document,
        "audio.all_off",
        "All Off",
        UiRect::new(width - 190.0, 18.0, 62.0, 30.0),
        false,
        true,
    );
    add_button_at(
        document,
        "settings.save",
        "Save Settings",
        UiRect::new(width - 122.0, 18.0, 70.0, 30.0),
        false,
        true,
    );
    add_button_at(
        document,
        "audio.test_a4",
        "A4",
        UiRect::new(width - 44.0, 14.0, 36.0, 36.0),
        false,
        audio_available,
    );
    add_clip_panel_labels(document, app, body.track);
    add_recording_options_control(document, body.track, quantize_on_record);

    add_label_at(
        document,
        "readout.bpm",
        format!("{bpm:.2} BPM"),
        UiRect::new(824.0, 18.0, 78.0, 30.0),
        true,
    );
    if !compact_top {
        add_label_at(
            document,
            "readout.loop",
            format!("{loop_beats:.0} beats"),
            UiRect::new(976.0, 18.0, 68.0, 30.0),
            false,
        );
    }

    let left_sections = left_browser_rects(app, rects.left);
    add_browser_module_controls(document, app, left_sections.module_bar);
    add_deferred_file_controls(document, app, left_sections.assets);

    if app.show_scale_browser && left_sections.scales.height > 0.0 {
        add_scale_browser_controls(document, app, left_sections.scales);
    }

    let asset_panel = left_sections.assets;
    if app.show_asset_browser && asset_panel.height > 0.0 {
        add_asset_browser_controls(document, app, asset_panel, selected_asset_kind);
    }

    add_label_at(
        document,
        "scale.root.label",
        format!(
            "Root {} ({})",
            midi_note_name(scale.root_midi),
            scale.root_midi
        ),
        UiRect::new(rects.right.x + 16.0, rects.right.y + 44.0, 150.0, 18.0),
        false,
    );
    add_button_at(
        document,
        "scale.root_down",
        "-",
        UiRect::new(rects.right.right() - 84.0, rects.right.y + 40.0, 30.0, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        "scale.root_up",
        "+",
        UiRect::new(rects.right.right() - 48.0, rects.right.y + 40.0, 30.0, 26.0),
        false,
        true,
    );
    add_label_at(
        document,
        "scale.base.label",
        format!("Base {:.2} Hz", scale.base_freq),
        UiRect::new(rects.right.x + 16.0, rects.right.y + 70.0, 150.0, 18.0),
        false,
    );
    add_button_at(
        document,
        "scale.base_down",
        "-",
        UiRect::new(rects.right.right() - 84.0, rects.right.y + 66.0, 30.0, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        "scale.base_up",
        "+",
        UiRect::new(rects.right.right() - 48.0, rects.right.y + 66.0, 30.0, 26.0),
        false,
        true,
    );
    add_label_at(
        document,
        "scale.description",
        format!("Scale {}", compact_label(&scale.scale.description, 28)),
        UiRect::new(
            rects.right.x + 16.0,
            rects.right.y + 96.0,
            rects.right.width - 32.0,
            18.0,
        ),
        false,
    );
    add_label_at(
        document,
        "transport.metronome.label",
        "Metronome",
        UiRect::new(rects.right.x + 16.0, rects.right.y + 122.0, 130.0, 18.0),
        false,
    );
    add_toggle_button_at(
        document,
        "transport.metronome",
        if metronome_enabled { "On" } else { "Off" },
        UiRect::new(
            rects.right.right() - 82.0,
            rects.right.y + 118.0,
            64.0,
            26.0,
        ),
        metronome_enabled,
        true,
    );
    let capture_buttons = capture_control_rects(rects.right, rects.right.y + 154.0);
    add_button_at(
        document,
        "capture.start",
        "Capture",
        capture_buttons.capture,
        false,
        capture_actions.start_enabled,
    );
    add_button_at(
        document,
        "capture.stop",
        "Stop",
        capture_buttons.stop,
        false,
        capture_actions.stop_enabled,
    );
    add_button_at(
        document,
        "capture.clear",
        "Clear",
        capture_buttons.clear,
        false,
        capture_actions.clear_enabled,
    );
    add_button_at(
        document,
        "keymap.refresh",
        "Maps",
        capture_buttons.maps,
        false,
        true,
    );
    add_label_at(
        document,
        "capture.status",
        format!(
            "{} {}{}",
            if capture_armed {
                "Capture armed"
            } else {
                "Capture idle"
            },
            capture_count,
            capture_last
                .as_ref()
                .map(|event| format!("  {event}"))
                .unwrap_or_default()
        ),
        UiRect::new(
            rects.right.x + 16.0,
            rects.right.y + 184.0,
            rects.right.width - 32.0,
            18.0,
        ),
        false,
    );

    add_label_at(
        document,
        "midi.last",
        midi_last.unwrap_or_else(|| "Last MIDI none".to_string()),
        UiRect::new(
            rects.right.x + 16.0,
            rects.right.y + 214.0,
            rects.right.width - 110.0,
            18.0,
        ),
        false,
    );
    add_toggle_button_at(
        document,
        "midi.channel_filter",
        app.midi_channel_filter_label(),
        UiRect::new(
            rects.right.right() - 82.0,
            rects.right.y + 210.0,
            64.0,
            26.0,
        ),
        app.midi_channel_filter().is_some(),
        true,
    );
    add_label_at(
        document,
        "lumatone.map",
        lumatone_status,
        UiRect::new(
            rects.right.x + 16.0,
            rects.right.y + 236.0,
            rects.right.width - 32.0,
            18.0,
        ),
        false,
    );

    add_ui_scale_control(document, app, rects.right, rects.right.y + 266.0);

    let right_control_bottom = rects.piano_roll.y - 8.0;
    let compact_device_buttons = rects.right.width < 300.0;
    let bottom_midi_buttons = device_control_rects(rects.right, rects.right.bottom() - 108.0);
    let bottom_audio_buttons = device_control_rects(rects.right, rects.right.bottom() - 50.0);
    let bottom_device_controls_fit = bottom_audio_buttons.connect.bottom() <= right_control_bottom;
    let compact_device_rows = if bottom_device_controls_fit {
        None
    } else {
        let audio_buttons = device_control_rects(rects.right, right_control_bottom - 26.0);
        let midi_buttons = device_control_rects(rects.right, audio_buttons.prev.y - 32.0);
        (midi_buttons.prev.y >= rects.right.y + 292.0).then_some((midi_buttons, audio_buttons))
    };
    let synth_control_bottom = compact_device_rows
        .map(|(midi_buttons, _)| midi_buttons.prev.y - 8.0)
        .unwrap_or(right_control_bottom);
    let synth_y = rects.right.y + 292.0;
    if synth_y + 22.0 <= synth_control_bottom {
        add_label_at(
            document,
            "synth.waveform.label",
            format!("Wave {}", synth.waveform.as_str()),
            UiRect::new(rects.right.x + 16.0, synth_y, 150.0, 18.0),
            false,
        );
        add_button_at(
            document,
            "synth.waveform",
            "Cycle",
            UiRect::new(rects.right.right() - 82.0, synth_y - 4.0, 64.0, 26.0),
            false,
            true,
        );
    }
    if synth_y + 52.0 <= synth_control_bottom {
        add_synth_mute_control(
            document,
            synth_muted,
            synth_limited,
            rects.right,
            synth_y + 30.0,
        );
    }
    for (key, label, value, y) in [
        (
            "gain",
            "Gain",
            format!("{:.0}%", synth.master_gain * 100.0),
            synth_y + 60.0,
        ),
        (
            "attack",
            "Attack",
            format!("{:.0} ms", synth.attack_ms),
            synth_y + 90.0,
        ),
        (
            "release",
            "Release",
            format!("{:.0} ms", synth.release_ms),
            synth_y + 120.0,
        ),
        (
            "filter",
            "Filter",
            format!("{:.0} Hz", synth.filter_cutoff_hz),
            synth_y + 150.0,
        ),
        (
            "delay",
            "Delay",
            format!("{:.0}%", synth.delay_mix * 100.0),
            synth_y + 180.0,
        ),
        (
            "drive",
            "Drive",
            format!("{:.1}x", synth.drive),
            synth_y + 210.0,
        ),
    ] {
        if y + 22.0 <= synth_control_bottom {
            add_synth_control(document, key, label, value, rects.right, y);
        }
    }

    add_piano_roll_option_controls(document, app, rects.piano_options);
    if bottom_device_controls_fit {
        add_label_at(
            document,
            "midi.selected",
            selected_midi,
            UiRect::new(
                rects.right.x + 12.0,
                rects.right.bottom() - 134.0,
                rects.right.width - 24.0,
                18.0,
            ),
            false,
        );
        add_button_at(
            document,
            "midi.prev",
            "<",
            bottom_midi_buttons.prev,
            false,
            app.midi_inputs.len() > 1,
        );
        add_button_at(
            document,
            "midi.next",
            ">",
            bottom_midi_buttons.next,
            false,
            app.midi_inputs.len() > 1,
        );
        add_button_at(
            document,
            "midi.refresh",
            if compact_device_buttons {
                "Refresh"
            } else {
                "Refresh MIDI"
            },
            bottom_midi_buttons.refresh,
            false,
            true,
        );
        add_button_at(
            document,
            "midi.connect",
            midi_connect_label(app, compact_device_buttons),
            bottom_midi_buttons.connect,
            false,
            !app.midi_inputs.is_empty(),
        );
        add_label_at(
            document,
            "audio.selected",
            selected_audio,
            UiRect::new(
                rects.right.x + 12.0,
                rects.right.bottom() - 76.0,
                rects.right.width - 24.0,
                18.0,
            ),
            false,
        );
        add_button_at(
            document,
            "audio.prev",
            "<",
            bottom_audio_buttons.prev,
            false,
            app.audio_outputs.len() > 1,
        );
        add_button_at(
            document,
            "audio.next",
            ">",
            bottom_audio_buttons.next,
            false,
            app.audio_outputs.len() > 1,
        );
        add_button_at(
            document,
            "audio.refresh",
            if compact_device_buttons {
                "Refresh"
            } else {
                "Refresh Audio"
            },
            bottom_audio_buttons.refresh,
            false,
            true,
        );
        add_button_at(
            document,
            "audio.connect",
            audio_connect_label(app, compact_device_buttons),
            bottom_audio_buttons.connect,
            false,
            !app.audio_outputs.is_empty(),
        );
    } else if let Some((midi_buttons, audio_buttons)) = compact_device_rows {
        add_button_at(
            document,
            "midi.prev",
            "<",
            midi_buttons.prev,
            false,
            app.midi_inputs.len() > 1,
        );
        add_button_at(
            document,
            "midi.next",
            ">",
            midi_buttons.next,
            false,
            app.midi_inputs.len() > 1,
        );
        add_button_at(
            document,
            "midi.refresh",
            if compact_device_buttons {
                "Refresh"
            } else {
                "Refresh MIDI"
            },
            midi_buttons.refresh,
            false,
            true,
        );
        add_button_at(
            document,
            "midi.connect",
            midi_connect_label(app, compact_device_buttons),
            midi_buttons.connect,
            false,
            !app.midi_inputs.is_empty(),
        );
        add_button_at(
            document,
            "audio.prev",
            "<",
            audio_buttons.prev,
            false,
            app.audio_outputs.len() > 1,
        );
        add_button_at(
            document,
            "audio.next",
            ">",
            audio_buttons.next,
            false,
            app.audio_outputs.len() > 1,
        );
        add_button_at(
            document,
            "audio.refresh",
            if compact_device_buttons {
                "Refresh"
            } else {
                "Refresh Audio"
            },
            audio_buttons.refresh,
            false,
            true,
        );
        add_button_at(
            document,
            "audio.connect",
            audio_connect_label(app, compact_device_buttons),
            audio_buttons.connect,
            false,
            !app.audio_outputs.is_empty(),
        );
    }
}

#[derive(Clone, Copy, Debug)]
struct DeviceControlRects {
    prev: UiRect,
    next: UiRect,
    refresh: UiRect,
    connect: UiRect,
}

#[derive(Clone, Copy, Debug)]
struct CaptureControlRects {
    capture: UiRect,
    stop: UiRect,
    clear: UiRect,
    maps: UiRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CaptureActionState {
    start_enabled: bool,
    stop_enabled: bool,
    clear_enabled: bool,
}

fn capture_action_state(capture_armed: bool, capture_count: usize) -> CaptureActionState {
    CaptureActionState {
        start_enabled: !capture_armed,
        stop_enabled: capture_armed,
        clear_enabled: capture_count > 0,
    }
}

fn capture_control_rects(panel: UiRect, y: f32) -> CaptureControlRects {
    let x = panel.x + 16.0;
    let width = (panel.width - 32.0).max(0.0);
    let gap = 6.0;
    let available = (width - gap * 3.0).max(0.0);
    let compact_w = (available / 4.3).floor();
    let capture_w = (compact_w * 1.3).max(0.0);
    let capture = UiRect::new(x, y, capture_w, 26.0);
    let stop = UiRect::new(capture.right() + gap, y, compact_w, 26.0);
    let clear = UiRect::new(stop.right() + gap, y, compact_w, 26.0);
    let maps = UiRect::new(clear.right() + gap, y, compact_w, 26.0);
    CaptureControlRects {
        capture,
        stop,
        clear,
        maps,
    }
}

fn device_control_rects(panel: UiRect, y: f32) -> DeviceControlRects {
    let x = panel.x + 12.0;
    let width = (panel.width - 24.0).max(0.0);
    let gap = 6.0;
    let nav_w = 30.0;
    let action_w = ((width - nav_w * 2.0 - gap * 3.0) * 0.5).max(0.0);
    let prev = UiRect::new(x, y, nav_w, 26.0);
    let next = UiRect::new(prev.right() + gap, y, nav_w, 26.0);
    let refresh = UiRect::new(next.right() + gap, y, action_w, 26.0);
    let connect = UiRect::new(refresh.right() + gap, y, action_w, 26.0);
    DeviceControlRects {
        prev,
        next,
        refresh,
        connect,
    }
}

fn add_browser_module_controls(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    let gap = 6.0;
    let button_w = ((panel.width - 20.0 - gap) / 2.0).max(44.0);
    let y = panel.y + 8.0;
    add_toggle_button_at(
        document,
        "view.assets",
        "Assets",
        UiRect::new(panel.x + 10.0, y, button_w, 26.0),
        app.show_asset_browser,
        true,
    );
    add_toggle_button_at(
        document,
        "view.scales",
        "Scales",
        UiRect::new(panel.x + 10.0 + button_w + gap, y, button_w, 26.0),
        app.show_scale_browser,
        true,
    );
}

fn add_deferred_file_controls(document: &mut UiDocument, app: &AppState, asset_panel: UiRect) {
    let mut y = asset_panel.bottom() - 34.0;
    let gap = 6.0;
    if !app.recent_project_paths().is_empty() {
        let button_w = ((asset_panel.width - 20.0 - gap) / 2.0).max(50.0);
        let open_recent_enabled =
            !app.project_dirty && app.recent_project_paths().iter().any(|path| path.exists());
        add_button_at(
            document,
            "file.open_recent",
            "Recent",
            UiRect::new(asset_panel.x + 10.0, y, button_w, 26.0),
            false,
            open_recent_enabled,
        );
        add_button_at(
            document,
            "file.forget_recent",
            "Forget",
            UiRect::new(asset_panel.x + 10.0 + button_w + gap, y, button_w, 26.0),
            false,
            true,
        );
        y -= 32.0;
    }
    if app.autosave_available {
        let recover_enabled = !app.project_dirty;
        let button_w = ((asset_panel.width - 20.0 - gap) / 2.0).max(50.0);
        add_button_at(
            document,
            "file.recover",
            "Recover",
            UiRect::new(asset_panel.x + 10.0, y, button_w, 26.0),
            recover_enabled,
            recover_enabled,
        );
        add_button_at(
            document,
            "file.dismiss_autosave",
            "Dismiss",
            UiRect::new(asset_panel.x + 10.0 + button_w + gap, y, button_w, 26.0),
            recover_enabled,
            recover_enabled,
        );
    }
}

fn add_scale_browser_controls(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    let mut y = panel.y + 64.0;
    let scale_button_y = panel.bottom() - 34.0;
    let scale_rows = ((scale_button_y - y - 4.0) / 26.0).floor().max(0.0) as usize;
    let scale_start = visible_list_start(
        app.selected_scale_library,
        app.scale_library.len(),
        scale_rows,
    );
    for (idx, item) in app
        .scale_library
        .iter()
        .enumerate()
        .skip(scale_start)
        .take(scale_rows)
    {
        let selected = idx == app.selected_scale_library;
        add_selectable_at(
            document,
            format!("scale.select.{idx}"),
            scale_library_row_label(item),
            UiRect::new(panel.x + 10.0, y, panel.width - 20.0, 24.0),
            selected,
            true,
        );
        y += 26.0;
    }
    let scale_button_gap = 6.0;
    let scale_button_w = ((panel.width - 20.0 - scale_button_gap * 2.0) / 3.0).max(50.0);
    let selected_scale_loaded = app.selected_library_scale_is_loaded();
    add_button_at(
        document,
        "scale.load_selected",
        if selected_scale_loaded {
            "Loaded"
        } else {
            "Load"
        },
        UiRect::new(panel.x + 10.0, scale_button_y, scale_button_w, 26.0),
        selected_scale_loaded,
        !app.scale_library.is_empty() && !selected_scale_loaded,
    );
    add_button_at(
        document,
        "scale.refresh",
        "Refresh",
        UiRect::new(
            panel.x + 10.0 + scale_button_w + scale_button_gap,
            scale_button_y,
            scale_button_w,
            26.0,
        ),
        false,
        true,
    );
    add_button_at(
        document,
        "scale.remove_selected",
        "Remove",
        UiRect::new(
            panel.x + 10.0 + (scale_button_w + scale_button_gap) * 2.0,
            scale_button_y,
            scale_button_w,
            26.0,
        ),
        false,
        app.can_remove_selected_library_scale(),
    );
}

fn add_asset_browser_controls(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    selected_asset_kind: AudioAssetKind,
) {
    let asset_tab_gap = 4.0;
    let asset_tab_w = ((panel.width - 20.0 - asset_tab_gap * 3.0) / 4.0).max(44.0);
    let asset_tab_y = panel.y + 54.0;
    for (idx, kind) in AudioAssetKind::all().iter().enumerate() {
        add_selectable_at(
            document,
            format!("asset.kind.{idx}"),
            asset_tab_label(*kind),
            UiRect::new(
                panel.x + 10.0 + idx as f32 * (asset_tab_w + asset_tab_gap),
                asset_tab_y,
                asset_tab_w,
                26.0,
            ),
            *kind == app.selected_audio_asset_kind,
            true,
        );
    }
    let deferred_file_rows =
        usize::from(app.autosave_available) + usize::from(!app.recent_project_paths().is_empty());
    let asset_button_y = panel.bottom() - 34.0 - deferred_file_rows as f32 * 32.0;
    let bottom_reserved = (panel.bottom() - asset_button_y) + 42.0;
    let asset_y = panel.y + 84.0;
    let asset_row_stride = MIN_POINTER_TARGET_SIZE + 2.0;
    let visible_asset_rows = ((panel.bottom() - asset_y - bottom_reserved) / asset_row_stride)
        .floor()
        .max(0.0) as usize;
    let visible_assets = app
        .audio_assets
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind == selected_asset_kind)
        .collect::<Vec<_>>();
    let selected_asset_position = app
        .selected_audio_asset
        .and_then(|selected| visible_assets.iter().position(|(idx, _)| *idx == selected))
        .unwrap_or(0);
    let asset_rows = visible_asset_rows.min(12);
    let asset_start = visible_list_start(selected_asset_position, visible_assets.len(), asset_rows);
    for (visible_idx, (idx, item)) in visible_assets
        .into_iter()
        .skip(asset_start)
        .take(asset_rows)
        .enumerate()
    {
        add_selectable_at(
            document,
            format!("asset.select.{idx}"),
            audio_asset_row_label(item),
            UiRect::new(
                panel.x + 10.0,
                asset_y + visible_idx as f32 * asset_row_stride,
                panel.width - 20.0,
                MIN_POINTER_TARGET_SIZE,
            ),
            app.selected_audio_asset == Some(idx),
            true,
        );
    }
    add_button_at(
        document,
        "asset.refresh",
        "Refresh Assets",
        UiRect::new(panel.x + 10.0, asset_button_y, 112.0, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        "asset.import",
        "Import",
        UiRect::new(panel.x + 128.0, asset_button_y, 78.0, 26.0),
        false,
        true,
    );
}

#[derive(Clone, Debug)]
struct PianoPanelButtonSpec {
    name: &'static str,
    label: String,
    rect: UiRect,
    active: bool,
    enabled: bool,
}

fn piano_panel_button_specs(app: &AppState, panel: UiRect) -> Vec<PianoPanelButtonSpec> {
    let (has_notes, quantize_grid) = {
        let project = app.music_project.lock();
        (
            !project.clip.notes.is_empty(),
            project.transport.quantize_grid,
        )
    };
    let selected_note = app.selected_clip_note().is_some();
    let gap = 4.0;
    let row_h = 24.0;
    let x = panel.x + 10.0;
    let width = (panel.width - 20.0).max(1.0);
    let half_w = ((width - gap) * 0.5).max(1.0);
    let third_w = ((width - gap * 2.0) / 3.0).max(1.0);
    let mut y = panel.y + 26.0;
    let mut buttons = Vec::new();

    buttons.push(PianoPanelButtonSpec {
        name: "piano.view.scales",
        label: "Scale".to_string(),
        rect: UiRect::new(x, y, third_w, row_h),
        active: app.show_scale_browser,
        enabled: true,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "piano.transport.quantize_grid",
        label: format!("Grid {}", quantize_grid.as_str()),
        rect: UiRect::new(x + third_w + gap, y, third_w * 2.0 + gap, row_h),
        active: false,
        enabled: true,
    });
    y += row_h + gap + 18.0 + 18.0 + 20.0 + 18.0;

    buttons.push(PianoPanelButtonSpec {
        name: "clip.add_note",
        label: "Add".to_string(),
        rect: UiRect::new(x, y, half_w, row_h),
        active: false,
        enabled: true,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.delete_note",
        label: "Delete".to_string(),
        rect: UiRect::new(x + half_w + gap, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    y += row_h + gap;

    let paste_w = 48.0_f32.min((width * 0.34).max(MIN_POINTER_TARGET_SIZE));
    buttons.push(PianoPanelButtonSpec {
        name: "clip.duplicate_note",
        label: "Duplicate".to_string(),
        rect: UiRect::new(
            x,
            y,
            (width - paste_w - gap).max(MIN_POINTER_TARGET_SIZE),
            row_h,
        ),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.paste_note",
        label: "Paste".to_string(),
        rect: UiRect::new(x + width - paste_w, y, paste_w, row_h),
        active: false,
        enabled: app.can_paste_clip_note(),
    });
    y += row_h + gap;

    let nudge_w = 28.0;
    buttons.push(PianoPanelButtonSpec {
        name: "clip.nudge_left",
        label: "<".to_string(),
        rect: UiRect::new(x, y, nudge_w, row_h),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.nudge_right",
        label: ">".to_string(),
        rect: UiRect::new(x + nudge_w + gap, y, nudge_w, row_h),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.quantize",
        label: "Quantize".to_string(),
        rect: UiRect::new(
            x + (nudge_w + gap) * 2.0,
            y,
            (width - (nudge_w + gap) * 2.0).max(MIN_POINTER_TARGET_SIZE),
            row_h,
        ),
        active: false,
        enabled: has_notes,
    });
    y += row_h + gap;

    buttons.push(PianoPanelButtonSpec {
        name: "clip.pitch_down",
        label: "Pitch -".to_string(),
        rect: UiRect::new(x, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.pitch_up",
        label: "Pitch +".to_string(),
        rect: UiRect::new(x + half_w + gap, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    y += row_h + gap;

    buttons.push(PianoPanelButtonSpec {
        name: "clip.velocity_down",
        label: "Vel -".to_string(),
        rect: UiRect::new(x, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.velocity_up",
        label: "Vel +".to_string(),
        rect: UiRect::new(x + half_w + gap, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });

    buttons
}

fn add_piano_roll_option_controls(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    let (note_count, loop_beats) = {
        let project = app.music_project.lock();
        (project.clip.notes.len(), project.transport.loop_beats)
    };
    let scale_name = {
        let scale = app.scale_state.lock();
        format!("{} notes", scale.scale.steps.len())
    };
    let gap = 4.0;
    let row_h = 24.0;
    let x = panel.x + 10.0;
    let width = (panel.width - 20.0).max(1.0);
    let mut y = panel.y + 8.0;

    add_label_at(
        document,
        "piano.panel.notes_header",
        "NOTES",
        UiRect::new(x, y, width, 14.0),
        false,
    );
    y += 18.0;
    y += row_h + gap;
    add_label_at(
        document,
        "piano.panel.scale_summary",
        scale_name,
        UiRect::new(x, y, width, 14.0),
        false,
    );
    y += 18.0;
    add_label_at(
        document,
        "piano.panel.clip_header",
        "CLIP",
        UiRect::new(x, y, width, 14.0),
        false,
    );
    y += 18.0;
    add_label_at(
        document,
        "piano.panel.clip_summary",
        format!("{loop_beats:.0} beats  {note_count} notes"),
        UiRect::new(x, y, width, 14.0),
        false,
    );
    y += 20.0;
    add_label_at(
        document,
        "piano.panel.edit_header",
        "EDIT",
        UiRect::new(x, y, width, 14.0),
        false,
    );
    for button in piano_panel_button_specs(app, panel) {
        add_button_at_with_visible_label(
            document,
            button.name,
            button.label,
            Some(""),
            button.rect,
            button.active,
            button.enabled,
        );
    }
}

fn add_clip_panel_labels(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    let clip = clip_panel_summary(app);
    let row = UiRect::new(panel.x + 10.0, panel.y + 44.0, panel.width - 20.0, 96.0);
    let text_w = (row.width - 60.0).max(44.0);
    add_label_at(
        document,
        "clip.title",
        fit_label("Current Clip", text_w, 12.0),
        UiRect::new(row.x + 52.0, row.y + 13.0, text_w, 18.0),
        true,
    );
    add_label_at(
        document,
        "clip.note_count",
        fit_label(&clip.note_count, text_w, 11.0),
        UiRect::new(row.x + 52.0, row.y + 32.0, text_w, 16.0),
        false,
    );
    add_label_at(
        document,
        "clip.loop_grid",
        fit_label(&clip.loop_and_grid, row.width - 20.0, 11.0),
        UiRect::new(row.x + 10.0, row.y + 56.0, row.width - 20.0, 16.0),
        false,
    );
    if let Some(selected) = clip.selected_note {
        add_label_at(
            document,
            "clip.selected_note",
            fit_label(&selected, row.width - 20.0, 10.0),
            UiRect::new(row.x + 10.0, row.y + 74.0, row.width - 20.0, 16.0),
            false,
        );
    }
}

fn add_recording_options_control(
    document: &mut UiDocument,
    panel: UiRect,
    quantize_on_record: bool,
) {
    let y = panel.y + 158.0;
    let button_w = 54.0;
    let button = UiRect::new(panel.right() - 10.0 - button_w, y - 4.0, button_w, 26.0);
    let label_w = (button.x - panel.x - 22.0).max(54.0);
    add_label_at(
        document,
        "record.quantize.label",
        fit_label("Rec quantize", label_w, 11.0),
        UiRect::new(panel.x + 12.0, y, label_w, 18.0),
        false,
    );
    add_toggle_button_at(
        document,
        "transport.record_quantize",
        if quantize_on_record { "On" } else { "Off" },
        button,
        quantize_on_record,
        true,
    );
}

fn add_synth_control(
    document: &mut UiDocument,
    key: &str,
    label: impl Into<String>,
    value: impl Into<String>,
    panel: UiRect,
    y: f32,
) {
    add_label_at(
        document,
        format!("synth.{key}.label"),
        format!("{} {}", label.into(), value.into()),
        UiRect::new(panel.x + 16.0, y, panel.width - 110.0, 18.0),
        false,
    );
    add_button_at(
        document,
        format!("synth.{key}_down"),
        "-",
        UiRect::new(panel.right() - 84.0, y - 4.0, 30.0, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        format!("synth.{key}_up"),
        "+",
        UiRect::new(panel.right() - 48.0, y - 4.0, 30.0, 26.0),
        false,
        true,
    );
}

fn add_synth_mute_control(
    document: &mut UiDocument,
    muted: bool,
    limited: bool,
    panel: UiRect,
    y: f32,
) {
    let label = if limited {
        "Output limit"
    } else if muted {
        "Output muted"
    } else {
        "Output live"
    };
    add_label_at(
        document,
        "synth.mute.label",
        label,
        UiRect::new(panel.x + 16.0, y, panel.width - 110.0, 18.0),
        false,
    );
    add_toggle_button_at(
        document,
        "synth.mute",
        if muted { "Muted" } else { "Mute" },
        UiRect::new(panel.right() - 82.0, y - 4.0, 64.0, 26.0),
        muted,
        true,
    );
}

fn add_ui_scale_control(document: &mut UiDocument, app: &AppState, panel: UiRect, y: f32) {
    let gap = 6.0;
    let down_w = 30.0;
    let reset_w = 52.0;
    let up_w = 30.0;
    let total_w = down_w + reset_w + up_w + gap * 2.0;
    let button_x = panel.right() - 16.0 - total_w;
    let label_w = (button_x - panel.x - 24.0).max(46.0);
    add_label_at(
        document,
        "ui.scale.label",
        format!("Zoom {:.0}%", app.ui_scale() * 100.0),
        UiRect::new(panel.x + 16.0, y, label_w, 18.0),
        false,
    );
    add_button_at(
        document,
        "ui.scale_down",
        "-",
        UiRect::new(button_x, y - 4.0, down_w, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        "ui.scale_reset",
        "Reset",
        UiRect::new(button_x + down_w + gap, y - 4.0, reset_w, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        "ui.scale_up",
        "+",
        UiRect::new(button_x + down_w + reset_w + gap * 2.0, y - 4.0, up_w, 26.0),
        false,
        true,
    );
}

fn asset_tab_label(kind: AudioAssetKind) -> &'static str {
    match kind {
        AudioAssetKind::Sample => "Samp",
        AudioAssetKind::Instrument => "Instr",
        AudioAssetKind::Preset => "Pres",
        AudioAssetKind::Impulse => "IRs",
    }
}

fn visible_list_start(selected: usize, total: usize, visible_rows: usize) -> usize {
    if total <= visible_rows || visible_rows == 0 {
        return 0;
    }
    let selected = selected.min(total.saturating_sub(1));
    selected
        .saturating_sub(visible_rows - 1)
        .min(total - visible_rows)
}

#[cfg(test)]
fn project_location_label(app: &AppState) -> String {
    let Some(path) = app.project_path.as_ref() else {
        if app.project_dirty {
            return "Unsaved changes".to_string();
        }
        if let Some(recent) = recent_project_display_name(app) {
            return format!("Recent: {recent}");
        }
        return "Save to choose file".to_string();
    };
    path.parent()
        .and_then(|parent| parent.to_str())
        .filter(|parent| !parent.is_empty())
        .unwrap_or(".")
        .to_string()
}

#[cfg(test)]
fn recent_project_display_name(app: &AppState) -> Option<String> {
    app.recent_project_paths()
        .first()
        .and_then(|path| path.file_stem().or_else(|| path.file_name()))
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(|value| compact_label(value, 28))
}

#[cfg(test)]
fn recent_project_row_label(index: usize, path: &Path) -> String {
    let name = path
        .file_stem()
        .or_else(|| path.file_name())
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("project");
    if !path.exists() {
        return format!("{} Missing {}", index + 1, compact_label(name, 16));
    }
    match file_modified_age_label(path) {
        Some(age) => format!("{} {} {}", index + 1, compact_label(name, 18), age),
        None => format!("{} {}", index + 1, compact_label(name, 24)),
    }
}

#[cfg(test)]
fn file_modified_age_label(path: &Path) -> Option<String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            log::error!("Failed to read metadata for {}: {err}", path.display());
            return None;
        }
    };
    let modified = match metadata.modified() {
        Ok(modified) => modified,
        Err(err) => {
            log::error!(
                "Failed to read modification time for {}: {err}",
                path.display()
            );
            return None;
        }
    };
    let elapsed = match SystemTime::now().duration_since(modified) {
        Ok(elapsed) => elapsed,
        Err(err) => {
            log::error!(
                "Modification time for {} is in the future: {err}; using age 0",
                path.display()
            );
            Duration::from_secs(0)
        }
    };
    Some(compact_age_label(elapsed))
}

#[cfg(test)]
fn compact_age_label(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        "now".to_string()
    } else if seconds < 60 * 60 {
        format!("{}m", seconds / 60)
    } else if seconds < 24 * 60 * 60 {
        format!("{}h", seconds / (60 * 60))
    } else {
        format!("{}d", seconds / (24 * 60 * 60))
    }
}

fn scale_library_row_label(item: &crate::app::ScaleLibraryItem) -> String {
    if !item.path.exists() {
        return format!("Missing {}", item.name);
    }
    item.name.clone()
}

fn audio_asset_row_label(item: &crate::app::AudioAssetItem) -> String {
    let name = if item.is_dir {
        format!("> {}", item.name)
    } else {
        item.name.clone()
    };
    if !item.path.exists() {
        return format!("Missing {name}");
    }
    name
}

#[cfg(test)]
fn project_file_state_label(app: &AppState) -> &'static str {
    if app.project_path.is_none() && app.project_dirty {
        "Unsaved"
    } else if app.project_path.is_none() {
        "No file"
    } else if app.project_dirty {
        "Unsaved changes"
    } else {
        "Saved"
    }
}

fn transport_position_label(position_beats: f32) -> String {
    let position = position_beats.max(0.0);
    let bar = (position / 4.0).floor() as i32 + 1;
    let beat = (position.rem_euclid(4.0).floor() as i32) + 1;
    format!("Bar {bar}.{beat}")
}

fn current_scale_label(app: &AppState) -> String {
    let scale = app.scale_state.lock().clone();
    format!(
        "Current: {}  {} notes",
        scale.scale.description,
        scale.scale.steps.len()
    )
}

fn asset_browser_summary(app: &AppState) -> String {
    let count = app
        .audio_assets
        .iter()
        .filter(|asset| asset.kind == app.selected_audio_asset_kind)
        .count();
    format!("{}  {count}", app.selected_audio_asset_kind.label())
}

fn draw_top_bar(primitives: &mut Vec<ScenePrimitive>, width: f32, height: f32) {
    push_rect(
        primitives,
        UiRect::new(0.0, 0.0, width, height),
        color(13, 20, 29),
        0.0,
        Some(stroke(color(37, 51, 68), 1.0)),
    );
}

fn draw_left_browser(primitives: &mut Vec<ScenePrimitive>, rect: UiRect, app: &AppState) {
    let LeftBrowserRects {
        module_bar,
        scales,
        assets,
    } = left_browser_rects(app, rect);
    draw_browser_module_bar(primitives, module_bar, app);
    if app.show_scale_browser && scales.height > 0.0 {
        draw_panel(primitives, scales, "SCALES & TUNINGS");
        draw_current_scale(primitives, scales, app);
    }
    if app.show_asset_browser && assets.height > 0.0 {
        draw_panel(primitives, assets, "ASSET BROWSER");
        draw_asset_browser(primitives, assets, app);
    }
}

fn draw_browser_module_bar(primitives: &mut Vec<ScenePrimitive>, rect: UiRect, _app: &AppState) {
    draw_panel(primitives, rect, "");
}

fn draw_asset_browser(primitives: &mut Vec<ScenePrimitive>, panel: UiRect, app: &AppState) {
    push_rect(
        primitives,
        UiRect::new(panel.x + 10.0, panel.y + 34.0, panel.width - 20.0, 24.0),
        color(10, 16, 23),
        3.0,
        Some(stroke(color(38, 52, 70), 1.0)),
    );
    push_text(
        primitives,
        UiRect::new(panel.x + 18.0, panel.y + 38.0, panel.width - 36.0, 16.0),
        asset_browser_summary(app),
        12.0,
        muted(),
        TextHorizontalAlign::Start,
    );
    let selected_kind = app.selected_audio_asset_kind;
    let has_visible_assets = app
        .audio_assets
        .iter()
        .any(|asset| asset.kind == selected_kind);
    if !has_visible_assets && panel.height >= 190.0 {
        let empty = UiRect::new(panel.x + 10.0, panel.y + 100.0, panel.width - 20.0, 68.0);
        push_rect(
            primitives,
            empty,
            color(10, 16, 23),
            5.0,
            Some(stroke(color(35, 48, 64), 1.0)),
        );
        push_text(
            primitives,
            UiRect::new(empty.x + 12.0, empty.y + 24.0, empty.width - 24.0, 20.0),
            format!("No {} in library", selected_kind.label()),
            13.0,
            strong(),
            TextHorizontalAlign::Start,
        );
    }
}

fn draw_current_scale(primitives: &mut Vec<ScenePrimitive>, panel: UiRect, app: &AppState) {
    let row = UiRect::new(panel.x + 10.0, panel.y + 34.0, panel.width - 20.0, 28.0);
    push_rect(
        primitives,
        row,
        color(15, 35, 38),
        5.0,
        Some(stroke(accent(), 1.0)),
    );
    let text_w = (row.width - 20.0).max(40.0);
    push_text(
        primitives,
        UiRect::new(row.x + 10.0, row.y + 5.0, text_w, 18.0),
        fit_label(&current_scale_label(app), text_w, 12.0),
        12.0,
        strong(),
        TextHorizontalAlign::Start,
    );
    if app.scale_library.is_empty() && row.bottom() + 34.0 <= panel.bottom() - 34.0 {
        push_text(
            primitives,
            UiRect::new(
                panel.x + 12.0,
                row.bottom() + 12.0,
                panel.width - 24.0,
                18.0,
            ),
            "No .scl files found",
            12.0,
            muted(),
            TextHorizontalAlign::Start,
        );
    }
}

fn draw_track_list(primitives: &mut Vec<ScenePrimitive>, rect: UiRect, app: &AppState) {
    draw_panel(primitives, rect, "CLIP");
    let clip = clip_panel_summary(app);
    let track_color = color(132, 81, 238);
    let row = UiRect::new(rect.x + 10.0, rect.y + 44.0, rect.width - 20.0, 96.0);
    push_rect(primitives, row, color(13, 20, 29), 6.0, None);
    push_rect(
        primitives,
        UiRect::new(row.x + 10.0, row.y + 10.0, 32.0, 32.0),
        track_color,
        8.0,
        None,
    );
    let activity_ratio = if clip.note_total == 0 {
        0.05
    } else {
        (clip.note_total as f32 / 16.0).clamp(0.18, 0.86)
    };
    push_rect(
        primitives,
        UiRect::new(row.x + 10.0, row.bottom() - 5.0, row.width - 20.0, 3.0),
        color(28, 40, 54),
        2.0,
        None,
    );
    push_rect(
        primitives,
        UiRect::new(
            row.x + 10.0,
            row.bottom() - 5.0,
            (row.width - 20.0) * activity_ratio,
            3.0,
        ),
        track_color,
        2.0,
        None,
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ClipPanelSummary {
    note_total: usize,
    note_count: String,
    loop_and_grid: String,
    selected_note: Option<String>,
}

fn clip_panel_summary(app: &AppState) -> ClipPanelSummary {
    let project = app.music_project.lock();
    let notes = project.clip.notes.clone();
    let loop_beats = project.transport.loop_beats;
    let quantize_grid = project.transport.quantize_grid;
    drop(project);

    let note_count = notes.len();
    let note_label = if note_count == 1 { "note" } else { "notes" };
    let selected_note = app.selected_clip_note.and_then(|selected| {
        notes
            .iter()
            .find(|note| note.id == selected)
            .map(|note| clip_panel_selected_note_label(app, note))
    });

    ClipPanelSummary {
        note_total: note_count,
        note_count: format!("{note_count} {note_label}"),
        loop_and_grid: format!("{loop_beats:.0} beats  Grid {}", quantize_grid.as_str()),
        selected_note,
    }
}

fn clip_panel_selected_note_label(app: &AppState, note: &ClipNote) -> String {
    app.scale_state
        .lock()
        .note_info(note.musical_note)
        .map(|info| {
            format!(
                "Sel d{} o{} {:.1}Hz {:+.0}c b{:.2} l{:.2} v{}",
                info.degree + 1,
                info.octave,
                info.freq,
                info.cents_from_root,
                note.start_beats,
                note.duration_beats,
                note.velocity
            )
        })
        .unwrap_or_else(|| {
            format!(
                "Sel pitch {} b{:.2} l{:.2} v{}",
                note.musical_note, note.start_beats, note.duration_beats, note.velocity
            )
        })
}

fn draw_right_tools(primitives: &mut Vec<ScenePrimitive>, rect: UiRect, app: &AppState) {
    draw_panel(primitives, rect, "CONTROL");
    push_line(
        primitives,
        UiPoint::new(rect.x + 12.0, rect.y + 258.0),
        UiPoint::new(rect.right() - 12.0, rect.y + 258.0),
        color(37, 51, 68),
        1.0,
    );
    push_line(
        primitives,
        UiPoint::new(rect.x + 12.0, rect.bottom() - 154.0),
        UiPoint::new(rect.right() - 12.0, rect.bottom() - 154.0),
        color(37, 51, 68),
        1.0,
    );
    draw_output_meter(primitives, rect, app);
}

fn draw_output_meter(primitives: &mut Vec<ScenePrimitive>, rect: UiRect, app: &AppState) {
    let synth_y = rect.y + 292.0;
    let meter = UiRect::new(rect.x + 16.0, synth_y + 52.0, rect.width - 110.0, 4.0);
    push_rect(primitives, meter, color(21, 30, 40), 2.0, None);
    let level = app.synth.output_level().clamp(0.0, 1.0);
    if level > 0.0 {
        let fill = UiRect::new(
            meter.x,
            meter.y,
            (meter.width * level).max(2.0),
            meter.height,
        );
        push_rect(
            primitives,
            fill,
            if app.synth.output_limited() {
                color(255, 105, 92)
            } else {
                accent()
            },
            2.0,
            None,
        );
    }
}

fn draw_status_bar(
    primitives: &mut Vec<ScenePrimitive>,
    width: f32,
    y: f32,
    height: f32,
    app: &AppState,
) {
    push_rect(
        primitives,
        UiRect::new(0.0, y, width, height),
        color(11, 18, 26),
        0.0,
        Some(stroke(color(37, 51, 68), 1.0)),
    );
    push_text(
        primitives,
        UiRect::new(8.0, y + 5.0, width - 16.0, 16.0),
        status_bar_label(app, width),
        12.0,
        muted(),
        TextHorizontalAlign::Start,
    );
}

fn status_bar_label(app: &AppState, width: f32) -> String {
    let label = format!(
        "Voices {}  Active {}   |   {}",
        app.synth.active_voice_count(),
        app.synth.active_notes().len(),
        app.last_status
    );
    fit_label(&label, width - 16.0, 12.0)
}

fn draw_panel(primitives: &mut Vec<ScenePrimitive>, rect: UiRect, title: &str) {
    push_rect(
        primitives,
        rect,
        color(15, 23, 32),
        8.0,
        Some(stroke(color(42, 58, 78), 1.0)),
    );
    if !title.is_empty() {
        push_text(
            primitives,
            UiRect::new(rect.x + 12.0, rect.y + 10.0, rect.width - 24.0, 18.0),
            title,
            12.0,
            muted(),
            TextHorizontalAlign::Start,
        );
    }
}

fn push_rect(
    primitives: &mut Vec<ScenePrimitive>,
    rect: UiRect,
    fill: ColorRgba,
    radius: f32,
    border: Option<StrokeStyle>,
) {
    let mut paint = PaintRect::solid(rect, fill).corner_radii(CornerRadii::uniform(radius));
    if let Some(border) = border {
        paint = paint.stroke(border);
    }
    primitives.push(ScenePrimitive::Rect(paint));
}

fn push_line(
    primitives: &mut Vec<ScenePrimitive>,
    from: UiPoint,
    to: UiPoint,
    color: ColorRgba,
    width: f32,
) {
    primitives.push(ScenePrimitive::Line {
        from,
        to,
        stroke: stroke(color, width),
    });
}

fn push_text(
    primitives: &mut Vec<ScenePrimitive>,
    rect: UiRect,
    text: impl Into<String>,
    size: f32,
    color: ColorRgba,
    align: TextHorizontalAlign,
) {
    let text = text.into();
    let fitted_text = fit_label(&text, rect.width, size);
    primitives.push(ScenePrimitive::Text(
        PaintText::new(
            fitted_text,
            rect,
            TextStyle {
                font_size: size,
                line_height: size + 4.0,
                family: FontFamily::SansSerif,
                color,
                ..Default::default()
            },
        )
        .horizontal_align(align)
        .vertical_align(TextVerticalAlign::Center)
        .multiline(false),
    ));
}

#[cfg(test)]
fn point_from_position(position: PhysicalPosition<f64>, ui_scale: f32) -> UiPoint {
    let scale = effective_ui_scale(ui_scale) as f64;
    UiPoint::new((position.x / scale) as f32, (position.y / scale) as f32)
}

#[cfg(test)]
mod tests;
