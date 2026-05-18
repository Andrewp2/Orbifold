#[cfg(test)]
use operad::{
    ApproxTextMeasurer, FocusDirection, KeyCode, PointerButton, PointerButtons, PointerEventKind,
    RawInputEvent, RawPointerEvent, UiInputEvent, WidgetDrag,
};
use operad::{
    ColorRgba, CornerRadii, FontFamily, PaintRect, PaintText, ScenePrimitive, StrokeStyle,
    TextHorizontalAlign, TextStyle, TextVerticalAlign, UiDocument, UiNodeId, UiPoint, UiRect,
    UiVisual, WidgetActionBinding, WidgetActionMode, layout, widgets,
};
#[cfg(feature = "native-app")]
use operad::{
    CursorShape, NativeCanvasInput, NativeKeyboardInput, NativeWgpuCanvasRenderRegistry,
    NativeWindowHooks, NativeWindowMetrics, NativeWindowOptions, UiSize, WidgetAction,
    WidgetActionKind, WidgetDragPhase, WidgetValueEditPhase,
};
#[cfg(test)]
use winit::dpi::{PhysicalPosition, PhysicalSize};
#[cfg(test)]
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::app::AppState;
#[cfg(feature = "native-app")]
use crate::app::WorkspaceResizeTarget;
#[cfg(test)]
use crate::project::QuantizeGrid;

mod browser;
mod control_panel;
mod controls;
mod devices;
mod editor_panels;
#[cfg(feature = "native-app")]
mod interactions;
mod piano_interaction;
mod presenters;
#[cfg(feature = "native-app")]
mod screenshot;
mod settings_panel;
mod surfaces;
mod top_bar;
#[cfg(feature = "native-app")]
mod windowing;
mod workspace;
#[cfg(test)]
use browser::{BrowserListMetrics, asset_list_metrics, list_scrollbar_rects, scale_list_metrics};
use browser::{
    add_asset_browser_controls, add_browser_module_controls, add_deferred_file_controls,
    add_scale_browser_controls, draw_left_browser, left_browser_rects, left_browser_splitter_rect,
};
use control_panel::add_control_panel_controls;
#[cfg(test)]
use control_panel::{CaptureActionState, capture_action_state, capture_control_rects};
use controls::add_pointer_edit_hit_at;
#[cfg(test)]
use devices::device_control_rects;
use devices::{add_device_panel_controls, add_right_panel_mode_control};
use editor_panels::{
    add_clip_panel_controls, add_piano_panel_button_labels, add_piano_roll_option_controls,
};
#[cfg(feature = "native-app")]
use piano_interaction::{
    LoopEndDragMode, NoteDrag, PianoGridClick, PianoGridPress, PianoKeyboardDrag,
    PianoViewportDrag, PianoViewportDragMode, TimelineDragMode,
};
#[cfg(test)]
use piano_interaction::{NoteDragMode, note_drag_from_action, piano_cursor_shape_at};
use presenters::*;
#[cfg(test)]
use screenshot::effective_ui_scale;
#[cfg(test)]
use screenshot::{
    logical_size_for_window, screenshot_physical_size, screenshot_ui_scale_for_values,
    ui_scale_for_values, validate_screenshot_pixels,
};
#[cfg(feature = "native-app")]
use screenshot::{ui_scale_for_pixel_size, write_startup_screenshot};
use settings_panel::add_settings_panel_controls;
pub(in crate::ui) use surfaces::SurfaceRects;
use surfaces::piano_pitch_label_step;
#[cfg(test)]
use surfaces::{
    LoopBoundary, piano_pitch_grid_line_step, piano_pitch_viewport_indicator_rects,
    piano_time_viewport_indicator_rects, visible_loop_boundary_positions,
    visible_quantize_grid_step,
};
#[cfg(feature = "native-app")]
use surfaces::{
    PIANO_INPUT_CANVAS_KEY, loop_end_boundary_hit_rect, piano_pitch_viewport_indicator_hit_rect,
    piano_time_viewport_indicator_hit_rect,
};
use surfaces::{add_center_editor_surfaces, surface_rects};
#[cfg(test)]
use surfaces::{
    note_resize_edge_width, piano_note_color, piano_note_rects, piano_velocity_hit_rects,
};
use top_bar::add_top_bar_controls;
#[cfg(test)]
use windowing::initial_window_size_for_monitor;
#[cfg(feature = "native-app")]
use windowing::{requested_or_monitor_window_size, window_title_for_app};
#[cfg(feature = "native-app")]
use workspace::BodyRects;
#[cfg(test)]
use workspace::MIN_EDITOR_TOP_HEIGHT;
#[cfg(test)]
use workspace::workspace_splitter_chrome_rects;
use workspace::{body_rects, draw_workspace_splitters, workspace_resize_rects};

#[cfg(feature = "native-app")]
use super::accessibility::apply_focus_name;
#[cfg(test)]
use super::accessibility::button_accessibility_label;
#[cfg(test)]
use super::accessibility::{focused_node_name, keyboard_focus_status};
#[cfg(feature = "native-app")]
use super::actions::dispatch_action;
#[cfg(test)]
use super::actions::handle_key;
#[cfg(feature = "native-app")]
use super::actions::handle_text_edit_action;
use super::labels::pitch_label;
#[cfg(test)]
use super::labels::{
    lumatone_map_label, midi_event_label, midi_note_name, selected_audio_output_name,
    selected_midi_input_name,
};
#[cfg(test)]
use super::text::compact_label;
#[cfg(test)]
use super::text::estimated_text_width;
use super::text::fit_label;
use super::theme::{accent, clip_color, color, muted, stroke};

#[cfg(test)]
use super::accessibility::node_is_focusable_action;
#[cfg(test)]
use super::actions::{selected_device_status, shortcut_help_status};
#[cfg(test)]
use super::labels::{
    audio_output_diagnostic_label, audio_output_status_label, device_connect_label,
    device_label_with_position, device_status_label, midi_input_diagnostic_label,
    midi_input_status_label, selected_name_matches_connected,
};

const MIN_LAYOUT_WIDTH: f32 = 1200.0;
const MIN_LAYOUT_HEIGHT: f32 = 760.0;
const MIN_EFFECTIVE_UI_SCALE: f32 = 0.25;
const MIN_QUANTIZE_GRID_SPACING: f32 = 10.0;
const MIN_PITCH_GRID_SPACING: f32 = 8.0;
const MIN_POINTER_TARGET_SIZE: f32 = 24.0;
const PIANO_GRID_DOUBLE_CLICK_MILLIS: u64 = 500;
const PIANO_GRID_DOUBLE_CLICK_DISTANCE: f32 = 8.0;

#[cfg(feature = "native-app")]
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

#[cfg(feature = "native-app")]
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
    loop_end_drag: Option<LoopEndDragMode>,
    piano_keyboard_drag: Option<PianoKeyboardDrag>,
    piano_grid_press: Option<PianoGridPress>,
    last_piano_grid_click: Option<PianoGridClick>,
    piano_viewport_drag: Option<PianoViewportDrag>,
    workspace_resize_drag: Option<WorkspaceResizeDrag>,
    focused_action: Option<String>,
    cursor_shape: CursorShape,
    applied_cursor_shape: CursorShape,
    cursor_grab_active: bool,
    ui_scale: f32,
}

#[derive(Clone, Copy, Debug)]
#[cfg(feature = "native-app")]
struct WorkspaceResizeDrag {
    target: WorkspaceResizeTarget,
    grab_offset_px: f32,
}

#[cfg(feature = "native-app")]
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
            loop_end_drag: None,
            piano_keyboard_drag: None,
            piano_grid_press: None,
            last_piano_grid_click: None,
            piano_viewport_drag: None,
            workspace_resize_drag: None,
            focused_action: None,
            cursor_shape: CursorShape::Default,
            applied_cursor_shape: CursorShape::Default,
            cursor_grab_active: false,
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

    fn has_active_pointer_drag(&self) -> bool {
        self.note_drag.is_some()
            || self.timeline_drag.is_some()
            || self.loop_end_drag.is_some()
            || self.piano_keyboard_drag.is_some()
            || self.piano_viewport_drag.is_some()
            || self.workspace_resize_drag.is_some()
    }

    fn view(&self, viewport: UiSize) -> UiDocument {
        let mut document = build_surface_document_for_interaction(
            &self.app,
            viewport.width,
            viewport.height,
            self.has_active_pointer_drag(),
        );
        apply_focus_name(&mut document, self.focused_action.as_deref());
        document
    }

    fn update(&mut self, action: WidgetAction) {
        let Some(action_name) = action_name_from_binding(&action.binding) else {
            return;
        };
        log::trace!(
            target: "orbifold::ui::native",
            "widget action={} kind={:?}",
            action_name,
            action.kind
        );
        match action.kind {
            WidgetActionKind::Activate(_) => {
                dispatch_action(&mut self.app, &action_name, None, self.layout);
            }
            WidgetActionKind::PointerEdit(edit) => {
                self.handle_pointer_edit_action(&action_name, edit.phase, edit.position);
            }
            WidgetActionKind::Drag(drag) => {
                if self.handle_active_drag_action(drag) {
                    return;
                }
                log::trace!(
                    target: "orbifold::ui::native",
                    "ignored drag-only widget action={action_name}"
                );
            }
            WidgetActionKind::TextEdit(edit) => {
                self.focused_action = Some(action_name.clone());
                handle_text_edit_action(&mut self.app, &action_name, edit);
            }
            _ => {}
        }
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
            self.loop_end_drag = None;
            self.piano_keyboard_drag = None;
            self.workspace_resize_drag = None;
            return;
        };
        self.pressed_action = Some(action.clone());
        self.timeline_drag = TimelineDragMode::from_action(&action);
        self.loop_end_drag = LoopEndDragMode::from_action(&action);
        self.workspace_resize_drag = workspace_resize_target_from_action(&action)
            .map(|target| self.workspace_resize_drag_for_pointer(target, point));
        self.piano_viewport_drag = PianoViewportDragMode::from_action(&action)
            .map(|mode| self.piano_viewport_drag_for_pointer(mode, point));
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
        if let Some(mode) = self.loop_end_drag {
            let _ = self.resize_loop_end(mode, point);
        }
        if self.piano_viewport_drag.is_some() {
            let _ = self.drag_piano_viewport(point);
        }
        if self.workspace_resize_drag.is_some() {
            let _ = self.drag_workspace_layout(point, false);
        }
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
        if let Some(mode) = self.loop_end_drag.take() {
            let _ = self.resize_loop_end(mode, point);
            if let Some(document) = self.document.as_mut() {
                let _ = document.handle_input(UiInputEvent::PointerUp(point));
            }
            self.pressed_action = None;
            return;
        }
        if self.piano_viewport_drag.take().is_some() {
            if let Some(document) = self.document.as_mut() {
                let _ = document.handle_input(UiInputEvent::PointerUp(point));
            }
            self.pressed_action = None;
            return;
        }
        if self.workspace_resize_drag.is_some() {
            let _ = self.drag_workspace_layout(point, true);
            if let Some(document) = self.document.as_mut() {
                let _ = document.handle_input(UiInputEvent::PointerUp(point));
            }
            self.workspace_resize_drag = None;
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
            let metrics = scale_list_metrics(&self.app, sections.scales);
            let Some(start) = scrolled_list_start(metrics, direction) else {
                return false;
            };
            self.app.set_scale_library_list_start(start);
            return true;
        }
        if rect_contains_point(sections.assets, point) {
            let metrics = asset_list_metrics(
                &self.app,
                sections.assets,
                self.app.selected_audio_asset_kind,
            );
            let Some(start) = scrolled_list_start(metrics, direction) else {
                return false;
            };
            self.app
                .set_audio_asset_list_start(self.app.selected_audio_asset_kind, start);
            return true;
        }
        false
    }
}

#[cfg(feature = "native-app")]
fn action_name_from_binding(binding: &WidgetActionBinding) -> Option<String> {
    binding
        .action_id()
        .map(|action| action.as_str().to_string())
}

#[cfg(feature = "native-app")]
fn workspace_resize_target_from_action(action: &str) -> Option<WorkspaceResizeTarget> {
    match action {
        "layout.resize.left" => Some(WorkspaceResizeTarget::Left),
        "layout.resize.track" => Some(WorkspaceResizeTarget::Track),
        "layout.resize.right" => Some(WorkspaceResizeTarget::Right),
        "layout.resize.bottom" => Some(WorkspaceResizeTarget::Bottom),
        "layout.resize.browser" => Some(WorkspaceResizeTarget::Browser),
        _ => None,
    }
}

#[cfg(feature = "native-app")]
fn workspace_resize_cursor(target: WorkspaceResizeTarget) -> CursorShape {
    match target {
        WorkspaceResizeTarget::Bottom | WorkspaceResizeTarget::Browser => {
            CursorShape::ResizeVertical
        }
        WorkspaceResizeTarget::Left
        | WorkspaceResizeTarget::Track
        | WorkspaceResizeTarget::Right => CursorShape::ResizeHorizontal,
    }
}

#[cfg(feature = "native-app")]
fn widget_drag_phase(phase: WidgetDragPhase) -> WidgetValueEditPhase {
    match phase {
        WidgetDragPhase::Begin => WidgetValueEditPhase::Begin,
        WidgetDragPhase::Update => WidgetValueEditPhase::Update,
        WidgetDragPhase::Commit => WidgetValueEditPhase::Commit,
        WidgetDragPhase::Cancel => WidgetValueEditPhase::Cancel,
    }
}

#[cfg(feature = "native-app")]
fn workspace_resize_target_at_point(
    app: &AppState,
    layout: Option<SurfaceRects>,
    point: UiPoint,
) -> Option<WorkspaceResizeTarget> {
    let layout = layout?;
    if let Some(rect) = left_browser_splitter_rect(app, layout.left)
        && rect_contains_point(rect, point)
    {
        return Some(WorkspaceResizeTarget::Browser);
    }
    let body = BodyRects {
        left: layout.left,
        track: layout.track,
        center: layout.center,
        right: layout.right,
    };
    let splitters = workspace_resize_rects(body, layout.piano_roll);
    let targets: Vec<(WorkspaceResizeTarget, UiRect)> = if app.show_clip_panel {
        vec![
            (WorkspaceResizeTarget::Left, splitters.left),
            (WorkspaceResizeTarget::Track, splitters.track),
            (WorkspaceResizeTarget::Right, splitters.right),
            (WorkspaceResizeTarget::Bottom, splitters.bottom),
        ]
    } else {
        vec![
            (WorkspaceResizeTarget::Left, splitters.left),
            (WorkspaceResizeTarget::Right, splitters.right),
            (WorkspaceResizeTarget::Bottom, splitters.bottom),
        ]
    };
    targets
        .into_iter()
        .find_map(|(target, rect)| rect_contains_point(rect, point).then_some(target))
}

#[cfg(test)]
fn scrolled_list_start(metrics: BrowserListMetrics, direction: isize) -> Option<usize> {
    if !metrics.is_scrollable() {
        return None;
    }
    let max_start = metrics.total.saturating_sub(metrics.visible_rows);
    let next = if direction > 0 {
        (metrics.start + 1).min(max_start)
    } else {
        metrics.start.saturating_sub(1)
    };
    (next != metrics.start).then_some(next)
}

#[cfg(feature = "native-app")]
fn loop_end_resize_mode_at_point(
    layout: Option<SurfaceRects>,
    point: UiPoint,
) -> Option<LoopEndDragMode> {
    let layout = layout?;
    if let Some(rect) = loop_end_boundary_hit_rect(layout.arrangement_ruler, layout)
        && rect_contains_point(rect, point)
    {
        return Some(LoopEndDragMode::Arrangement);
    }
    if let Some(rect) = loop_end_boundary_hit_rect(layout.piano_ruler, layout)
        && rect_contains_point(rect, point)
    {
        return Some(LoopEndDragMode::Piano);
    }
    None
}

#[cfg(feature = "native-app")]
fn piano_viewport_drag_mode_at_point(
    layout: Option<SurfaceRects>,
    point: UiPoint,
) -> Option<PianoViewportDragMode> {
    let layout = layout?;
    if rect_contains_point(piano_time_viewport_indicator_hit_rect(layout), point) {
        return Some(PianoViewportDragMode::Time);
    }
    if rect_contains_point(piano_pitch_viewport_indicator_hit_rect(layout), point) {
        return Some(PianoViewportDragMode::Pitch);
    }
    None
}

#[cfg(feature = "native-app")]
fn piano_viewport_drag_cursor(mode: PianoViewportDragMode) -> CursorShape {
    match mode {
        PianoViewportDragMode::Time => CursorShape::ResizeHorizontal,
        PianoViewportDragMode::Pitch => CursorShape::ResizeVertical,
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

#[cfg(feature = "native-app")]
fn should_redraw_when_idle(app: &AppState) -> bool {
    app.music_project.lock().transport.playing || app.has_pending_file_dialog()
}

pub(crate) fn build_surface_document(app: &AppState, width: f32, height: f32) -> UiDocument {
    build_surface_document_for_interaction(app, width, height, false)
}

pub(in crate::ui) fn build_surface_document_for_interaction(
    app: &AppState,
    width: f32,
    height: f32,
    capture_active_drag: bool,
) -> UiDocument {
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
    add_global_drag_capture_hit_target(&mut document, width, height);
    add_center_editor_surfaces(&mut document, root, app, rects);
    add_operad_controls(&mut document, app, width, height);
    add_workspace_splitter_overlay(&mut document, app, width, height);
    add_workspace_resize_hit_targets(&mut document, app, width, height);
    add_piano_panel_button_labels(&mut document, app, rects.piano_options);
    add_piano_text_overlay(&mut document, app, width, height);
    if capture_active_drag {
        route_passive_nodes_to_active_drag_capture(&mut document);
        add_active_drag_capture_hit_target(&mut document, width, height);
    }
    document
}

#[cfg(feature = "web-app")]
pub(in crate::ui) fn surface_layout(app: &AppState, width: f32, height: f32) -> SurfaceRects {
    surface_rects(app, width, height)
}

fn route_passive_nodes_to_active_drag_capture(document: &mut UiDocument) {
    for index in 0..document.node_count() {
        let node = &document.nodes()[index];
        let can_dispatch_action = node
            .accessibility
            .as_ref()
            .is_none_or(|accessibility| accessibility.enabled && !accessibility.hidden);
        if node.action.is_some() || !can_dispatch_action {
            continue;
        }
        // Operad drag capture is node-id based, while this document is rebuilt
        // after each drag update. Passive current-frame nodes need an active
        // route so stale captured ids still produce drag updates.
        let node = document.node_mut(UiNodeId(index));
        node.action = Some(WidgetActionBinding::action("active.drag_capture"));
        node.action_mode = WidgetActionMode::PointerEdit;
    }
}

fn add_global_drag_capture_hit_target(document: &mut UiDocument, width: f32, height: f32) {
    add_pointer_edit_hit_at(
        document,
        "global.drag_capture",
        UiRect::new(0.0, 0.0, width, height),
    );
}

fn add_active_drag_capture_hit_target(document: &mut UiDocument, width: f32, height: f32) {
    add_pointer_edit_hit_at(
        document,
        "active.drag_capture",
        UiRect::new(0.0, 0.0, width, height),
    );
}

fn build_surface_primitives(app: &AppState, width: f32, height: f32) -> Vec<ScenePrimitive> {
    let mut primitives = Vec::new();
    let top_h = 62.0;
    let bottom_h = 26.0;
    let body = body_rects(app, width, height, top_h, bottom_h);

    push_rect(
        &mut primitives,
        UiRect::new(0.0, 0.0, width, height),
        color(8, 12, 18),
        0.0,
        None,
    );
    draw_top_bar(&mut primitives, width, top_h);
    draw_left_browser(&mut primitives, body.left, app);
    if app.show_clip_panel {
        draw_track_list(&mut primitives, body.track, app);
    }
    draw_right_tools(&mut primitives, body.right, app);
    draw_status_bar(&mut primitives, width, height - bottom_h, bottom_h, app);
    primitives
}

fn add_workspace_splitter_overlay(
    document: &mut UiDocument,
    app: &AppState,
    width: f32,
    height: f32,
) {
    widgets::scene(
        document,
        document.root,
        "orbifold.native.workspace_splitters",
        workspace_splitter_primitives(app, width, height),
        widgets::SceneOptions::default()
            .with_layout(layout::absolute(0.0, 0.0, width, height))
            .accessibility_label("Workspace resize handles"),
    );
}

fn workspace_splitter_primitives(app: &AppState, width: f32, height: f32) -> Vec<ScenePrimitive> {
    let mut primitives = Vec::new();
    let body = body_rects(app, width, height, 62.0, 26.0);
    let rects = surface_rects(app, width, height);
    draw_workspace_splitters(&mut primitives, app, body, rects.piano_roll);
    primitives
}

fn add_workspace_resize_hit_targets(
    document: &mut UiDocument,
    app: &AppState,
    width: f32,
    height: f32,
) {
    let body = body_rects(app, width, height, 62.0, 26.0);
    let rects = surface_rects(app, width, height);
    let splitters = workspace_resize_rects(body, rects.piano_roll);
    add_pointer_edit_hit_at(document, "layout.resize.left", splitters.left);
    if app.show_clip_panel {
        add_pointer_edit_hit_at(document, "layout.resize.track", splitters.track);
    }
    add_pointer_edit_hit_at(document, "layout.resize.right", splitters.right);
    add_pointer_edit_hit_at(document, "layout.resize.bottom", splitters.bottom);
    if let Some(rect) = left_browser_splitter_rect(app, rects.left) {
        add_pointer_edit_hit_at(document, "layout.resize.browser", rect);
    }
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

fn add_operad_controls(document: &mut UiDocument, app: &AppState, width: f32, height: f32) {
    let rects = surface_rects(app, width, height);
    let body = body_rects(app, width, height, 62.0, 26.0);
    let project = app.music_project.lock();
    let quantize_on_record = project.transport.quantize_on_record;
    drop(project);
    let selected_asset_kind = app.selected_audio_asset_kind;

    add_top_bar_controls(document, app, width);
    if app.show_clip_panel {
        add_clip_panel_controls(document, app, body.track, quantize_on_record);
    }

    let left_sections = left_browser_rects(app, rects.left);
    add_browser_module_controls(document, app, left_sections.module_bar);
    add_deferred_file_controls(document, app, left_sections.deferred_files);

    if app.show_scale_browser && left_sections.scales.height > 0.0 {
        add_scale_browser_controls(document, app, left_sections.scales);
    }

    let asset_panel = left_sections.assets;
    if app.show_asset_browser && asset_panel.height > 0.0 {
        add_asset_browser_controls(document, app, asset_panel, selected_asset_kind);
    }

    add_piano_roll_option_controls(document, app, rects.piano_options);
    add_right_panel_mode_control(document, app, rects.right);
    let right_control_bottom = rects.piano_roll.y - 8.0;
    if app.show_device_panel {
        add_device_panel_controls(document, app, rects.right, right_control_bottom);
        return;
    }
    if app.show_settings_panel {
        add_settings_panel_controls(document, app, rects.right, right_control_bottom);
        return;
    }

    add_control_panel_controls(document, app, rects.right, right_control_bottom);
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

fn draw_track_list(primitives: &mut Vec<ScenePrimitive>, rect: UiRect, app: &AppState) {
    draw_panel(primitives, rect, "CLIP");
    let clip = clip_panel_summary(app);
    let track_color = clip_color();
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

fn draw_right_tools(primitives: &mut Vec<ScenePrimitive>, rect: UiRect, app: &AppState) {
    draw_panel(
        primitives,
        rect,
        if app.show_device_panel {
            "DEVICES"
        } else if app.show_settings_panel {
            "SETTINGS"
        } else {
            "CONTROL"
        },
    );
    if app.show_device_panel || app.show_settings_panel {
        return;
    }
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
