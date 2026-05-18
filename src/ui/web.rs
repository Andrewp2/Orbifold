use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Array, Reflect};
use operad::{
    ApproxTextMeasurer, KeyCode, KeyModifiers, UiDocument, UiInputEvent, UiPoint, UiSize,
    WidgetAction, WidgetActionKind, WidgetDrag, WidgetDragPhase, WidgetTextEdit,
    WidgetValueEditPhase,
};
use wasm_bindgen::JsValue;

use crate::app::{AppState, AudioAssetKind, WorkspaceResizeTarget};
use crate::project::ProjectFile;
use crate::settings::AppSettings;
use crate::time::AppInstant;

use super::actions::{canonical_action_name, dispatch_action, handle_text_edit_action};

const MIN_WIDTH: f32 = 1200.0;
const MIN_HEIGHT: f32 = 760.0;
const PIANO_GRID_DOUBLE_CLICK_MILLIS: u64 = 500;
const PIANO_GRID_DOUBLE_CLICK_DISTANCE: f32 = 8.0;

pub(crate) async fn app_from_browser_settings() -> AppState {
    let (mut app, should_write_initial_settings) = match browser_load_settings_text() {
        Ok(Some(text)) => match AppSettings::from_text(&text) {
            Ok(settings) => {
                let mut app = AppState::for_web_with_settings(settings);
                app.last_status = "Loaded browser settings".to_string();
                (app, true)
            }
            Err(err) => {
                let message = format!("Browser settings load error: {err}; using defaults");
                log::error!("{message}");
                let mut app = AppState::for_web();
                app.last_status = message;
                (app, false)
            }
        },
        Ok(None) => {
            let mut app = AppState::for_web();
            app.last_status = "Orbifold web app running".to_string();
            (app, true)
        }
        Err(err) => {
            let message = format!("Browser settings unavailable: {err}; using defaults");
            log::error!("{message}");
            let mut app = AppState::for_web();
            app.last_status = message;
            (app, false)
        }
    };
    if should_write_initial_settings
        && let Err(err) = browser_save_settings_text(&app.browser_settings_text())
    {
        app.set_error_status(format!("Browser settings save error: {err}"));
    }
    match browser_load_asset_storage_records().await {
        Ok(records) => {
            let mut restored = 0_usize;
            for record in records {
                if app
                    .restore_browser_audio_asset(
                        record.path,
                        &record.file_name,
                        &record.bytes,
                        record.kind,
                        false,
                    )
                    .is_some()
                {
                    restored += 1;
                }
            }
            if restored > 0 {
                app.set_status_preserving_error(format!("Restored {restored} browser assets"));
            }
        }
        Err(err) => app.set_error_status(format!("Browser asset storage load error: {err}")),
    }
    match browser_load_project_storage_text() {
        Ok(Some(text)) => {
            let (scale_resource, keymap_resource) = browser_project_resources_for_text(&text);
            if app.load_browser_project_text_with_resources(
                &text,
                "browser session",
                scale_resource,
                keymap_resource,
            ) {
                app.set_status_preserving_error("Restored browser project session");
            }
        }
        Ok(None) => {}
        Err(err) => app.set_error_status(format!("Browser project session load error: {err}")),
    }
    app
}

pub(crate) async fn run(app: AppState) -> Result<(), wasm_bindgen::JsValue> {
    let initial_ui_scale = app.ui_scale();
    let app = WebOrbifoldApp::new(app);
    operad::web::run_app_with_hooks(
        operad::web::WebRuntimeOptions::new("Orbifold")
            .with_canvas_id("orbifold-canvas")
            .with_status_id("orbifold-status")
            .with_target_name("orbifold")
            .with_background("#071017")
            .with_ui_scale(initial_ui_scale)
            .with_tick_action("runtime.tick")
            .with_tick_rate_hz(30.0),
        app,
        WebOrbifoldApp::update,
        WebOrbifoldApp::view,
        operad::web::WebRuntimeHooks::new()
            .with_title(|state: &WebOrbifoldApp| state.app.window_title())
            .with_before_render(|state: &mut WebOrbifoldApp, metrics| {
                state.prepare_browser_frame(metrics.viewport);
            }),
    )
    .await
}

struct WebOrbifoldApp {
    app: AppState,
    pending_text_files: Rc<RefCell<Vec<PendingBrowserTextFile>>>,
    pending_binary_files: Rc<RefCell<Vec<PendingBrowserBinaryFile>>>,
    pending_asset_saves: Rc<RefCell<Vec<Result<(), String>>>>,
    pending_midi: Rc<RefCell<Vec<BrowserMidiServiceResult>>>,
    viewport: UiSize,
    note_drag: Option<WebNoteDrag>,
    timeline_drag: Option<WebTimelineDragMode>,
    loop_end_drag: Option<WebLoopEndDragMode>,
    piano_keyboard_drag: Option<WebPianoKeyboardDrag>,
    piano_viewport_drag: Option<WebPianoViewportDrag>,
    workspace_resize_drag: Option<WebWorkspaceResizeDrag>,
    last_piano_grid_click: Option<WebPianoGridClick>,
    wheel_bridge_installed: bool,
    workspace_pointer_bridge_installed: bool,
    frame_count: u64,
    runtime_ready_published: bool,
}

#[derive(Clone, Copy, Debug)]
struct WebNoteDrag {
    note_id: u64,
    mode: WebNoteDragMode,
    beat_offset: f32,
    pitch_offset: i32,
    pushed_history: bool,
}

#[derive(Clone, Copy, Debug)]
enum WebNoteDragMode {
    Move,
    ResizeStart,
    ResizeEnd,
    Velocity,
}

#[derive(Clone, Copy, Debug)]
enum WebTimelineDragMode {
    Arrangement,
    Piano,
}

#[derive(Clone, Copy, Debug)]
enum WebLoopEndDragMode {
    Arrangement,
    Piano,
}

#[derive(Clone, Copy, Debug)]
struct WebPianoKeyboardDrag {
    start_position: UiPoint,
    last_position: UiPoint,
    pitch_remainder_px: f32,
    moved: bool,
}

#[derive(Clone, Copy, Debug)]
struct WebPianoViewportDrag {
    mode: WebPianoViewportDragMode,
    grab_offset_px: f32,
}

#[derive(Clone, Copy, Debug)]
enum WebPianoViewportDragMode {
    Time,
    Pitch,
}

#[derive(Clone, Copy, Debug)]
struct WebWorkspaceResizeDrag {
    target: WorkspaceResizeTarget,
    grab_offset_px: f32,
}

#[derive(Clone, Copy, Debug)]
struct WebPianoGridClick {
    position: UiPoint,
    timestamp_millis: u64,
}

#[derive(Clone, Copy, Debug)]
struct BrowserWheelEvent {
    position: UiPoint,
    delta: UiPoint,
    delta_mode: u32,
    shift: bool,
    ctrl: bool,
    alt: bool,
    meta: bool,
}

#[derive(Clone, Copy, Debug)]
struct BrowserWorkspacePointerEvent {
    phase: WidgetValueEditPhase,
    position: UiPoint,
}

struct PendingBrowserTextFile {
    kind: BrowserTextFileKind,
    result: Result<Option<BrowserTextFile>, String>,
}

#[derive(Clone, Copy)]
enum BrowserTextFileKind {
    Project,
    Scale,
    KeyMap,
}

struct BrowserTextFile {
    name: String,
    text: String,
}

struct PendingBrowserBinaryFile {
    kind: AudioAssetKind,
    result: Result<Option<BrowserBinaryFile>, String>,
}

struct BrowserBinaryFile {
    name: String,
    bytes: Vec<u8>,
}

struct BrowserAssetStorageRecord {
    path: std::path::PathBuf,
    kind: AudioAssetKind,
    file_name: String,
    bytes: Vec<u8>,
}

type BrowserProjectTextResource = Option<(std::path::PathBuf, String)>;

enum BrowserMidiServiceResult {
    Inputs(Result<Vec<String>, String>),
    Connected(Result<String, String>),
}

impl BrowserWheelEvent {
    fn pixel_delta(self, page_size: UiSize) -> UiPoint {
        match self.delta_mode {
            1 => UiPoint::new(self.delta.x * 36.0, self.delta.y * 36.0),
            2 => UiPoint::new(
                self.delta.x * page_size.width,
                self.delta.y * page_size.height,
            ),
            _ => self.delta,
        }
    }
}

impl BrowserTextFileKind {
    fn status_label(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Scale => "Scale",
            Self::KeyMap => "Key map",
        }
    }

    fn cancel_status(self) -> &'static str {
        match self {
            Self::Project => "Project open cancelled",
            Self::Scale => "Scale open cancelled",
            Self::KeyMap => "Key map open cancelled",
        }
    }
}

impl WebOrbifoldApp {
    fn new(app: AppState) -> Self {
        if let Err(err) = install_browser_keyboard_shortcuts() {
            log::error!("Browser keyboard shortcuts unavailable: {err}");
        }
        Self {
            app,
            pending_text_files: Rc::new(RefCell::new(Vec::new())),
            pending_binary_files: Rc::new(RefCell::new(Vec::new())),
            pending_asset_saves: Rc::new(RefCell::new(Vec::new())),
            pending_midi: Rc::new(RefCell::new(Vec::new())),
            viewport: UiSize::new(MIN_WIDTH, MIN_HEIGHT),
            note_drag: None,
            timeline_drag: None,
            loop_end_drag: None,
            piano_keyboard_drag: None,
            piano_viewport_drag: None,
            workspace_resize_drag: None,
            last_piano_grid_click: None,
            wheel_bridge_installed: false,
            workspace_pointer_bridge_installed: false,
            frame_count: 0,
            runtime_ready_published: false,
        }
    }

    fn update(&mut self, action: WidgetAction) {
        let Some(action_name) = action
            .binding
            .action_id()
            .map(|action| action.as_str().to_string())
        else {
            return;
        };
        if action_name == "runtime.tick" {
            self.poll_browser_services();
            self.app.update_music_playback();
            return;
        }
        match action.kind {
            WidgetActionKind::Activate(_) => {
                self.handle_browser_activate_action(&action_name);
            }
            WidgetActionKind::PointerEdit(edit) => {
                self.handle_pointer_edit_action(&action_name, edit.phase, edit.position);
                if matches!(edit.phase, WidgetValueEditPhase::Commit) {
                    self.persist_browser_project_snapshot();
                }
            }
            WidgetActionKind::Drag(drag) => {
                let persist_project = matches!(drag.phase, WidgetDragPhase::Commit);
                self.handle_drag_action(&action_name, drag);
                if persist_project {
                    self.persist_browser_project_snapshot();
                }
            }
            WidgetActionKind::TextEdit(edit) => {
                handle_text_edit_action(&mut self.app, &action_name, edit);
                self.persist_browser_settings();
                self.persist_browser_project_snapshot();
            }
            _ => {}
        }
    }

    fn prepare_frame(&mut self, viewport: UiSize) {
        self.viewport = UiSize::new(
            viewport.width.max(MIN_WIDTH),
            viewport.height.max(MIN_HEIGHT),
        );
        self.install_browser_wheel_bridge();
        self.install_browser_workspace_pointer_bridge();
    }

    fn prepare_browser_frame(&mut self, viewport: UiSize) {
        self.prepare_frame(viewport);
        self.poll_browser_services();
        self.app.poll_pending_file_dialog();
        self.app.update_music_playback();
        self.frame_count = self.frame_count.saturating_add(1);
        mark_browser_runtime_ready(self.frame_count, viewport);
        self.publish_browser_automation_geometry();
        self.publish_browser_text_audit();
        self.publish_browser_runtime_state();
        self.runtime_ready_published = true;
    }

    fn publish_browser_runtime_state(&self) {
        let midi_last = self.app.midi_last.lock().clone();
        let (note_count, transport_playing, transport_position_beats, loop_beats) = {
            let project = self.app.music_project.lock();
            (
                project.clip.notes.len() as f64,
                project.transport.playing,
                project.current_position_beats(AppInstant::now()) as f64,
                project.transport.loop_beats as f64,
            )
        };
        let scale = self.app.scale_state.lock().scale.clone();
        let scala_path = self
            .app
            .scala_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();
        let lumatone_path = self
            .app
            .lumatone_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();
        let lumatone_loaded = self.app.lumatone_map.lock().is_some();
        publish_runtime_state_js(
            &self.app.last_status,
            note_count,
            self.app.audio_assets.len() as f64,
            self.app.midi_inputs.len() as f64,
            &self.app.connected_midi_input,
            midi_last
                .as_ref()
                .map(|event| event.raw_status as f64)
                .unwrap_or(0.0),
            midi_last
                .as_ref()
                .map(|event| event.midi_note as f64)
                .unwrap_or(-1.0),
            self.app.audio_outputs.len() as f64,
            &self.app.connected_audio_output,
            self.app.audio_stream.is_some(),
            transport_playing,
            transport_position_beats,
            loop_beats,
            self.app.ui_scale() as f64,
            self.app.show_asset_browser,
            self.app.show_scale_browser,
            self.app.show_clip_panel,
            &scale.description,
            &scala_path,
            &lumatone_path,
            lumatone_loaded,
        );
    }

    fn publish_browser_text_audit(&self) {
        let mut document = self.view(self.viewport);
        let mut text_measurer = ApproxTextMeasurer;
        match document.compute_layout(self.viewport, &mut text_measurer) {
            Ok(()) => {
                let summary = super::text_audit::text_audit_summary(&document);
                publish_text_audit_js(
                    summary.text_count as f64,
                    summary.issue_count as f64,
                    summary.non_finite_count as f64,
                    summary.sample_issue.as_deref().unwrap_or(""),
                );
            }
            Err(err) => {
                let sample = format!("text audit layout failed: {err}");
                publish_text_audit_js(0.0, 0.0, 1.0, &sample);
            }
        }
    }

    fn publish_browser_automation_geometry(&self) {
        let layout = self.surface_rects();
        let grid = layout.piano_grid_rect();
        let root_midi = self.app.scale_state.lock().root_midi;
        let add_point = layout.piano_grid_point_for(4.0, root_midi);
        let drag_start = layout.piano_grid_point_for(4.5, root_midi);
        let drag_end = layout.piano_grid_point_for(7.0, root_midi);
        let loop_beats = self.app.music_project.lock().transport.loop_beats.max(1.0);
        let view_start = self.app.piano_view_start_beats(loop_beats);
        let view_beats = self.app.piano_view_visible_beats(loop_beats);
        let (min_pitch, max_pitch) = self.app.piano_pitch_range();
        let (resize_start, resize_end) = self
            .app
            .music_project
            .lock()
            .note_by_id(3)
            .and_then(|note| {
                let rect = layout.piano_note_primary_rect(&note)?;
                let edge_w = 8.0_f32.min(rect.width * 0.25);
                Some((
                    UiPoint::new(rect.right() - edge_w * 0.5, rect.y + rect.height * 0.5),
                    layout.piano_grid_point_for(
                        note.start_beats + note.duration_beats + 2.0,
                        note.musical_note,
                    ),
                ))
            })
            .unwrap_or_else(|| (UiPoint::new(0.0, 0.0), UiPoint::new(0.0, 0.0)));
        publish_automation_geometry_js(
            grid.x as f64,
            grid.y as f64,
            grid.width as f64,
            grid.height as f64,
            add_point.x as f64,
            add_point.y as f64,
            drag_start.x as f64,
            drag_start.y as f64,
            drag_end.x as f64,
            drag_end.y as f64,
            resize_start.x as f64,
            resize_start.y as f64,
            resize_end.x as f64,
            resize_end.y as f64,
            view_start as f64,
            view_beats as f64,
            min_pitch as f64,
            max_pitch as f64,
        );
        let right_resize = layout
            .workspace_resize_point_for(&self.app, WorkspaceResizeTarget::Right)
            .unwrap_or_else(|| UiPoint::new(0.0, 0.0));
        let bottom_resize = layout
            .workspace_resize_point_for(&self.app, WorkspaceResizeTarget::Bottom)
            .unwrap_or_else(|| UiPoint::new(0.0, 0.0));
        let arrangement_seek_start = layout.arrangement_ruler_point_for_fraction(0.25);
        let arrangement_seek_end = layout.arrangement_ruler_point_for_fraction(0.75);
        let piano_seek_start = layout.piano_ruler_point_for_fraction(0.25);
        let piano_seek_end = layout.piano_ruler_point_for_fraction(0.75);
        let (arrangement_loop_start, arrangement_loop_end) = layout
            .arrangement_loop_end_drag_points(12.0)
            .unwrap_or_else(|| (UiPoint::new(0.0, 0.0), UiPoint::new(0.0, 0.0)));
        let (piano_loop_start, piano_loop_end) = layout
            .piano_loop_end_drag_points(4.0)
            .unwrap_or_else(|| (UiPoint::new(0.0, 0.0), UiPoint::new(0.0, 0.0)));
        publish_layout_automation_js(
            right_resize.x as f64,
            right_resize.y as f64,
            (right_resize.x - 160.0).max(8.0) as f64,
            right_resize.y as f64,
            bottom_resize.x as f64,
            bottom_resize.y as f64,
            bottom_resize.x as f64,
            (bottom_resize.y - 120.0).max(8.0) as f64,
            layout.right_panel_width() as f64,
            layout.piano_roll_height() as f64,
            arrangement_seek_start.x as f64,
            arrangement_seek_start.y as f64,
            arrangement_seek_end.x as f64,
            arrangement_seek_end.y as f64,
            piano_seek_start.x as f64,
            piano_seek_start.y as f64,
            piano_seek_end.x as f64,
            piano_seek_end.y as f64,
            arrangement_loop_start.x as f64,
            arrangement_loop_start.y as f64,
            arrangement_loop_end.x as f64,
            arrangement_loop_end.y as f64,
            piano_loop_start.x as f64,
            piano_loop_start.y as f64,
            piano_loop_end.x as f64,
            piano_loop_end.y as f64,
        );
    }

    fn surface_rects(&self) -> super::native::SurfaceRects {
        super::native::surface_layout(&self.app, self.viewport.width, self.viewport.height)
    }

    fn has_active_pointer_drag(&self) -> bool {
        self.note_drag.is_some()
            || self.timeline_drag.is_some()
            || self.loop_end_drag.is_some()
            || self.piano_keyboard_drag.is_some()
            || self.piano_viewport_drag.is_some()
            || self.workspace_resize_drag.is_some()
    }

    fn handle_drag_action(&mut self, action: &str, drag: WidgetDrag) {
        self.handle_pointer_edit_action(action, widget_drag_phase(drag.phase), drag.current);
    }

    fn handle_pointer_edit_action(
        &mut self,
        action: &str,
        phase: WidgetValueEditPhase,
        point: UiPoint,
    ) {
        publish_pointer_action_js(
            action,
            widget_value_edit_phase_label(phase),
            point.x as f64,
            point.y as f64,
        );
        let layout = self.surface_rects();
        if action == "active.drag_capture" && self.workspace_pointer_bridge_installed {
            return;
        }
        if action == "active.drag_capture" {
            self.handle_active_pointer_drag(phase, point, layout);
            return;
        }
        if self.has_active_pointer_drag()
            && !matches!(
                phase,
                WidgetValueEditPhase::Begin | WidgetValueEditPhase::Preview
            )
        {
            self.handle_active_pointer_drag(phase, point, layout);
            return;
        }
        if let Some(target) = web_workspace_resize_target_from_action(action) {
            self.handle_workspace_resize_action(target, phase, point, layout);
            return;
        }
        if let Some(mode) = web_loop_end_drag_mode_from_action(action) {
            self.handle_loop_end_drag_action(mode, phase, point, layout);
            return;
        }
        if let Some(mode) = web_piano_viewport_drag_mode_from_action(action) {
            self.handle_piano_viewport_drag_action(mode, phase, point, layout);
            return;
        }
        if let Some(mode) = web_timeline_drag_mode_from_action(action) {
            self.handle_timeline_drag_action(mode, phase, point, layout);
            return;
        }
        if action == "piano.keyboard" {
            self.handle_piano_keyboard_action(phase, point, layout);
            return;
        }
        if action == "piano.grid" {
            self.handle_piano_grid_action(phase, point, layout);
            return;
        }
        let Some((note_id, mode)) = web_note_drag_from_action(action) else {
            return;
        };
        match phase {
            WidgetValueEditPhase::Begin => {
                self.app.select_clip_note(Some(note_id));
                self.note_drag = Some(self.note_drag_for_pointer(note_id, mode, point, layout));
                if matches!(mode, WebNoteDragMode::Velocity) {
                    self.drag_selected_note(point, layout);
                }
            }
            WidgetValueEditPhase::Update => {
                if self.note_drag.is_none() {
                    self.app.select_clip_note(Some(note_id));
                    self.note_drag = Some(self.note_drag_for_pointer(note_id, mode, point, layout));
                }
                self.drag_selected_note(point, layout);
            }
            WidgetValueEditPhase::Commit => {
                if self.note_drag.is_none() {
                    self.app.select_clip_note(Some(note_id));
                    self.note_drag = Some(self.note_drag_for_pointer(note_id, mode, point, layout));
                }
                self.drag_selected_note(point, layout);
                self.note_drag = None;
            }
            WidgetValueEditPhase::Cancel => {
                self.note_drag = None;
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    fn handle_active_pointer_drag(
        &mut self,
        phase: WidgetValueEditPhase,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        if self.note_drag.is_none() || matches!(phase, WidgetValueEditPhase::Preview) {
            if !matches!(phase, WidgetValueEditPhase::Preview) {
                self.handle_non_note_active_pointer_drag(phase, point, layout);
            }
            return;
        }
        if matches!(
            phase,
            WidgetValueEditPhase::Begin
                | WidgetValueEditPhase::Update
                | WidgetValueEditPhase::Commit
        ) {
            self.drag_selected_note(point, layout);
        }
        if matches!(
            phase,
            WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
        ) {
            self.note_drag = None;
        }
    }

    fn handle_non_note_active_pointer_drag(
        &mut self,
        phase: WidgetValueEditPhase,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        if let Some(active) = self.workspace_resize_drag {
            if matches!(
                phase,
                WidgetValueEditPhase::Begin
                    | WidgetValueEditPhase::Update
                    | WidgetValueEditPhase::Commit
            ) {
                self.drag_workspace_layout(
                    active,
                    point,
                    layout,
                    matches!(phase, WidgetValueEditPhase::Commit),
                );
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.workspace_resize_drag = None;
            }
            return;
        }
        if self.piano_viewport_drag.is_some() {
            if matches!(
                phase,
                WidgetValueEditPhase::Begin
                    | WidgetValueEditPhase::Update
                    | WidgetValueEditPhase::Commit
            ) {
                self.drag_piano_viewport(point, layout);
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.piano_viewport_drag = None;
            }
            return;
        }
        if self.piano_keyboard_drag.is_some() {
            if matches!(
                phase,
                WidgetValueEditPhase::Begin
                    | WidgetValueEditPhase::Update
                    | WidgetValueEditPhase::Commit
            ) {
                self.drag_piano_keyboard(point, layout);
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.piano_keyboard_drag = None;
            }
            return;
        }
        if let Some(mode) = self.loop_end_drag {
            if matches!(
                phase,
                WidgetValueEditPhase::Begin
                    | WidgetValueEditPhase::Update
                    | WidgetValueEditPhase::Commit
            ) {
                self.resize_loop_end(mode, point, layout);
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.loop_end_drag = None;
            }
            return;
        }
        if let Some(mode) = self.timeline_drag {
            if matches!(
                phase,
                WidgetValueEditPhase::Begin
                    | WidgetValueEditPhase::Update
                    | WidgetValueEditPhase::Commit
            ) {
                self.seek_timeline(mode, point, layout);
            }
            if matches!(
                phase,
                WidgetValueEditPhase::Commit | WidgetValueEditPhase::Cancel
            ) {
                self.timeline_drag = None;
            }
        }
    }

    fn handle_workspace_resize_action(
        &mut self,
        target: WorkspaceResizeTarget,
        phase: WidgetValueEditPhase,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        match phase {
            WidgetValueEditPhase::Begin => {
                let drag = WebWorkspaceResizeDrag {
                    target,
                    grab_offset_px: layout.workspace_resize_grab_offset(&self.app, target, point),
                };
                self.workspace_resize_drag = Some(drag);
                self.drag_workspace_layout(drag, point, layout, false);
            }
            WidgetValueEditPhase::Update => {
                if self.workspace_resize_drag.is_none() {
                    self.workspace_resize_drag = Some(WebWorkspaceResizeDrag {
                        target,
                        grab_offset_px: layout
                            .workspace_resize_grab_offset(&self.app, target, point),
                    });
                }
                if let Some(drag) = self.workspace_resize_drag {
                    self.drag_workspace_layout(drag, point, layout, false);
                }
            }
            WidgetValueEditPhase::Commit => {
                if self.workspace_resize_drag.is_none() {
                    self.workspace_resize_drag = Some(WebWorkspaceResizeDrag {
                        target,
                        grab_offset_px: layout
                            .workspace_resize_grab_offset(&self.app, target, point),
                    });
                }
                if let Some(drag) = self.workspace_resize_drag {
                    self.drag_workspace_layout(drag, point, layout, true);
                }
                self.workspace_resize_drag = None;
            }
            WidgetValueEditPhase::Cancel => {
                self.workspace_resize_drag = None;
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    fn drag_workspace_layout(
        &mut self,
        drag: WebWorkspaceResizeDrag,
        point: UiPoint,
        layout: super::native::SurfaceRects,
        persist: bool,
    ) {
        let Some(value) =
            layout.workspace_resize_value(&self.app, drag.target, point, drag.grab_offset_px)
        else {
            return;
        };
        let _ = self
            .app
            .set_workspace_layout_size(drag.target, value, persist);
    }

    fn handle_timeline_drag_action(
        &mut self,
        mode: WebTimelineDragMode,
        phase: WidgetValueEditPhase,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        match phase {
            WidgetValueEditPhase::Begin => {
                self.timeline_drag = Some(mode);
                self.seek_timeline(mode, point, layout);
            }
            WidgetValueEditPhase::Update => {
                if self.timeline_drag.is_none() {
                    self.timeline_drag = Some(mode);
                }
                self.seek_timeline(mode, point, layout);
            }
            WidgetValueEditPhase::Commit => {
                if self.timeline_drag.is_none() {
                    self.timeline_drag = Some(mode);
                }
                self.seek_timeline(mode, point, layout);
                self.timeline_drag = None;
            }
            WidgetValueEditPhase::Cancel => {
                self.timeline_drag = None;
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    fn seek_timeline(
        &mut self,
        mode: WebTimelineDragMode,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        let beat = match mode {
            WebTimelineDragMode::Arrangement => layout.arrangement_beat_at(point),
            WebTimelineDragMode::Piano => layout.piano_ruler_beat_at(point),
        };
        self.app.seek_transport_to(beat);
    }

    fn handle_loop_end_drag_action(
        &mut self,
        mode: WebLoopEndDragMode,
        phase: WidgetValueEditPhase,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        match phase {
            WidgetValueEditPhase::Begin => {
                self.loop_end_drag = Some(mode);
                self.resize_loop_end(mode, point, layout);
            }
            WidgetValueEditPhase::Update => {
                if self.loop_end_drag.is_none() {
                    self.loop_end_drag = Some(mode);
                }
                self.resize_loop_end(mode, point, layout);
            }
            WidgetValueEditPhase::Commit => {
                if self.loop_end_drag.is_none() {
                    self.loop_end_drag = Some(mode);
                }
                self.resize_loop_end(mode, point, layout);
                self.loop_end_drag = None;
            }
            WidgetValueEditPhase::Cancel => {
                self.loop_end_drag = None;
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    fn resize_loop_end(
        &mut self,
        mode: WebLoopEndDragMode,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        let action = match mode {
            WebLoopEndDragMode::Arrangement => "transport.loop_end",
            WebLoopEndDragMode::Piano => "piano.loop_end",
        };
        if let Some(beat) = layout.loop_end_beat_at(action, point) {
            let _ = self.app.set_loop_beats(beat);
        }
    }

    fn handle_piano_viewport_drag_action(
        &mut self,
        mode: WebPianoViewportDragMode,
        phase: WidgetValueEditPhase,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        match phase {
            WidgetValueEditPhase::Begin => {
                self.piano_viewport_drag =
                    Some(self.piano_viewport_drag_for_pointer(mode, point, layout));
                self.drag_piano_viewport(point, layout);
            }
            WidgetValueEditPhase::Update => {
                if self.piano_viewport_drag.is_none() {
                    self.piano_viewport_drag =
                        Some(self.piano_viewport_drag_for_pointer(mode, point, layout));
                }
                self.drag_piano_viewport(point, layout);
            }
            WidgetValueEditPhase::Commit => {
                if self.piano_viewport_drag.is_none() {
                    self.piano_viewport_drag =
                        Some(self.piano_viewport_drag_for_pointer(mode, point, layout));
                }
                self.drag_piano_viewport(point, layout);
                self.piano_viewport_drag = None;
            }
            WidgetValueEditPhase::Cancel => {
                self.piano_viewport_drag = None;
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    fn piano_viewport_drag_for_pointer(
        &self,
        mode: WebPianoViewportDragMode,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) -> WebPianoViewportDrag {
        let grab_offset_px = match mode {
            WebPianoViewportDragMode::Time => layout.piano_time_view_grab_offset(point),
            WebPianoViewportDragMode::Pitch => layout.piano_pitch_view_grab_offset(point),
        };
        WebPianoViewportDrag {
            mode,
            grab_offset_px,
        }
    }

    fn drag_piano_viewport(&mut self, point: UiPoint, layout: super::native::SurfaceRects) {
        let Some(drag) = self.piano_viewport_drag else {
            return;
        };
        match drag.mode {
            WebPianoViewportDragMode::Time => {
                let fraction = layout.piano_time_view_fraction(point, drag.grab_offset_px);
                let _ = self.app.set_piano_time_view_fraction(fraction);
            }
            WebPianoViewportDragMode::Pitch => {
                let fraction = layout.piano_pitch_view_fraction(point, drag.grab_offset_px);
                let _ = self.app.set_piano_pitch_view_fraction(fraction);
            }
        }
    }

    fn handle_piano_keyboard_action(
        &mut self,
        phase: WidgetValueEditPhase,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        match phase {
            WidgetValueEditPhase::Begin => {
                self.piano_keyboard_drag = Some(WebPianoKeyboardDrag {
                    start_position: point,
                    last_position: point,
                    pitch_remainder_px: 0.0,
                    moved: false,
                });
            }
            WidgetValueEditPhase::Update => {
                if self.piano_keyboard_drag.is_none() {
                    self.piano_keyboard_drag = Some(WebPianoKeyboardDrag {
                        start_position: point,
                        last_position: point,
                        pitch_remainder_px: 0.0,
                        moved: false,
                    });
                }
                self.drag_piano_keyboard(point, layout);
            }
            WidgetValueEditPhase::Commit => {
                if self.piano_keyboard_drag.is_some() {
                    self.drag_piano_keyboard(point, layout);
                    let drag = self.piano_keyboard_drag.take();
                    if drag.is_some_and(|drag| {
                        !drag.moved
                            && point_distance(drag.start_position, point)
                                <= PIANO_GRID_DOUBLE_CLICK_DISTANCE
                    }) {
                        self.app.audition_piano_pitch(layout.pitch_at(point));
                    }
                } else {
                    self.app.audition_piano_pitch(layout.pitch_at(point));
                }
            }
            WidgetValueEditPhase::Cancel => {
                self.piano_keyboard_drag = None;
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    fn handle_piano_grid_action(
        &mut self,
        phase: WidgetValueEditPhase,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) {
        match phase {
            WidgetValueEditPhase::Commit => {
                let timestamp_millis = js_sys::Date::now().max(0.0) as u64;
                if self.is_piano_grid_double_click(point, timestamp_millis) {
                    self.app
                        .add_clip_note_at(layout.beat_at(point), layout.pitch_at(point));
                    self.last_piano_grid_click = None;
                } else {
                    self.last_piano_grid_click = Some(WebPianoGridClick {
                        position: point,
                        timestamp_millis,
                    });
                }
            }
            WidgetValueEditPhase::Cancel => {
                self.last_piano_grid_click = None;
            }
            WidgetValueEditPhase::Begin
            | WidgetValueEditPhase::Update
            | WidgetValueEditPhase::Preview => {}
        }
    }

    fn is_piano_grid_double_click(&self, point: UiPoint, timestamp_millis: u64) -> bool {
        self.last_piano_grid_click.is_some_and(|click| {
            timestamp_millis.saturating_sub(click.timestamp_millis)
                <= PIANO_GRID_DOUBLE_CLICK_MILLIS
                && point_distance(click.position, point) <= PIANO_GRID_DOUBLE_CLICK_DISTANCE
        })
    }

    fn drag_piano_keyboard(&mut self, point: UiPoint, layout: super::native::SurfaceRects) {
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
            return;
        };

        if delta_x.abs() > f32::EPSILON {
            let _ = self
                .app
                .zoom_piano_roll_pitches(delta_x * 2.0, layout.pitch_at(point));
        }
        if row_delta != 0 {
            let _ = self.app.scroll_piano_roll(0.0, -row_delta);
        }
    }

    fn install_browser_wheel_bridge(&mut self) {
        if self.wheel_bridge_installed {
            return;
        }
        self.wheel_bridge_installed = true;
        match install_browser_wheel_bridge_js("orbifold-canvas") {
            Ok(()) => {}
            Err(err) => {
                log::error!(
                    "Browser piano wheel bridge unavailable: {}",
                    js_error_message(err)
                );
            }
        }
    }

    fn install_browser_workspace_pointer_bridge(&mut self) {
        if self.workspace_pointer_bridge_installed {
            return;
        }
        self.workspace_pointer_bridge_installed = true;
        match install_browser_workspace_pointer_bridge_js("orbifold-canvas") {
            Ok(()) => {}
            Err(err) => {
                log::error!(
                    "Browser workspace pointer bridge unavailable: {}",
                    js_error_message(err)
                );
            }
        }
    }

    fn handle_browser_wheel_event(&mut self, event: BrowserWheelEvent) {
        let layout = self.surface_rects();
        if !layout.contains_piano_input(event.position) {
            return;
        }
        let delta = event.pixel_delta(self.viewport);
        if event.ctrl || event.meta {
            let zoom_delta = if delta.y.abs() > f32::EPSILON {
                delta.y
            } else {
                -delta.x
            };
            let _ = self
                .app
                .zoom_piano_roll(zoom_delta, layout.beat_at(event.position));
            return;
        }
        if event.alt {
            let zoom_delta = if delta.y.abs() > f32::EPSILON {
                delta.y
            } else {
                -delta.x
            };
            let _ = self
                .app
                .zoom_piano_roll_pitches(zoom_delta, layout.pitch_at(event.position));
            return;
        }
        let (delta_beats, delta_pitches) = layout.piano_wheel_scroll_delta(delta, event.shift);
        let _ = self.app.scroll_piano_roll(delta_beats, delta_pitches);
    }

    fn handle_browser_workspace_pointer_event(&mut self, event: BrowserWorkspacePointerEvent) {
        let layout = self.surface_rects();
        match event.phase {
            WidgetValueEditPhase::Begin => {
                if self.has_active_pointer_drag() {
                    return;
                }
                if let Some(target) =
                    layout.workspace_resize_target_at_point(&self.app, event.position)
                {
                    self.handle_workspace_resize_action(
                        target,
                        event.phase,
                        event.position,
                        layout,
                    );
                } else if let Some(action) = layout.loop_end_drag_action_at_point(event.position)
                    && let Some(mode) = web_loop_end_drag_mode_from_action(action)
                {
                    self.handle_loop_end_drag_action(mode, event.phase, event.position, layout);
                } else if let Some(action) = layout.timeline_drag_action_at_point(event.position)
                    && let Some(mode) = web_timeline_drag_mode_from_action(action)
                {
                    self.handle_timeline_drag_action(mode, event.phase, event.position, layout);
                }
            }
            WidgetValueEditPhase::Update | WidgetValueEditPhase::Commit => {
                if self.has_active_pointer_drag() {
                    self.handle_active_pointer_drag(event.phase, event.position, layout);
                    if matches!(event.phase, WidgetValueEditPhase::Commit) {
                        self.persist_browser_project_snapshot();
                    }
                }
            }
            WidgetValueEditPhase::Cancel => {
                self.workspace_resize_drag = None;
                self.timeline_drag = None;
                self.loop_end_drag = None;
                self.piano_viewport_drag = None;
                self.piano_keyboard_drag = None;
                self.note_drag = None;
            }
            WidgetValueEditPhase::Preview => {}
        }
    }

    fn note_drag_for_pointer(
        &self,
        note_id: u64,
        mode: WebNoteDragMode,
        point: UiPoint,
        layout: super::native::SurfaceRects,
    ) -> WebNoteDrag {
        let Some(note) = self.app.music_project.lock().note_by_id(note_id) else {
            return WebNoteDrag {
                note_id,
                mode,
                beat_offset: 0.0,
                pitch_offset: 0,
                pushed_history: false,
            };
        };
        let beat_offset = if matches!(mode, WebNoteDragMode::Move) {
            layout.beat_at(point) - note.start_beats
        } else {
            0.0
        };
        let pitch_offset = if matches!(mode, WebNoteDragMode::Move) {
            layout.pitch_at(point) - note.musical_note
        } else {
            0
        };
        WebNoteDrag {
            note_id,
            mode,
            beat_offset,
            pitch_offset,
            pushed_history: false,
        }
    }

    fn drag_selected_note(&mut self, point: UiPoint, layout: super::native::SurfaceRects) {
        let Some(drag) = self.note_drag else {
            return;
        };
        let beat = layout.beat_at(point);
        let moved = match drag.mode {
            WebNoteDragMode::Move => self.app.drag_clip_note_to(
                drag.note_id,
                beat - drag.beat_offset,
                layout.pitch_at(point) - drag.pitch_offset,
                !drag.pushed_history,
            ),
            WebNoteDragMode::ResizeStart => {
                self.app
                    .resize_clip_note_start_to(drag.note_id, beat, !drag.pushed_history)
            }
            WebNoteDragMode::ResizeEnd => {
                self.app
                    .resize_clip_note_end_to(drag.note_id, beat, !drag.pushed_history)
            }
            WebNoteDragMode::Velocity => self.app.set_clip_note_velocity(
                drag.note_id,
                layout.velocity_at(point),
                !drag.pushed_history,
            ),
        };
        if moved && let Some(active) = self.note_drag.as_mut() {
            active.pushed_history = true;
        }
    }

    fn poll_browser_services(&mut self) {
        let mut should_persist_settings = false;
        let mut should_persist_project = false;
        let pending = self
            .pending_text_files
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>();
        for pending_file in pending {
            match pending_file.result {
                Ok(Some(file)) => match pending_file.kind {
                    BrowserTextFileKind::Project => {
                        let (scale_resource, keymap_resource) =
                            browser_project_resources_for_text(&file.text);
                        should_persist_project = self.app.load_browser_project_text_with_resources(
                            &file.text,
                            &file.name,
                            scale_resource,
                            keymap_resource,
                        );
                    }
                    BrowserTextFileKind::Scale => {
                        should_persist_project =
                            self.app.load_browser_scale_text(&file.text, &file.name);
                        if should_persist_project
                            && let Err(err) =
                                browser_save_text_resource("scale", &file.name, &file.text)
                        {
                            self.app.set_error_status(format!(
                                "Browser Scala storage save error: {err}"
                            ));
                        }
                    }
                    BrowserTextFileKind::KeyMap => {
                        should_persist_project =
                            self.app.load_browser_lumatone_text(&file.text, &file.name);
                        if should_persist_project
                            && let Err(err) =
                                browser_save_text_resource("keymap", &file.name, &file.text)
                        {
                            self.app.set_error_status(format!(
                                "Browser key map storage save error: {err}"
                            ));
                        }
                    }
                },
                Ok(None) => {
                    self.app.last_status = pending_file.kind.cancel_status().to_string();
                }
                Err(err) => self.app.set_error_status(format!(
                    "{} open error: {err}",
                    pending_file.kind.status_label()
                )),
            }
            should_persist_settings = true;
        }
        let pending_binary = self
            .pending_binary_files
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>();
        for pending_file in pending_binary {
            match pending_file.result {
                Ok(Some(file)) => {
                    if let Some(path) = self.app.import_browser_audio_asset(
                        &file.name,
                        &file.bytes,
                        pending_file.kind,
                    ) {
                        self.begin_browser_asset_storage_save(
                            path,
                            pending_file.kind,
                            file.name,
                            file.bytes,
                        );
                    }
                }
                Ok(None) => {
                    self.app.last_status = "Asset import cancelled".to_string();
                }
                Err(err) => self
                    .app
                    .set_error_status(format!("Asset import error: {err}")),
            }
        }
        let pending_asset_saves = self
            .pending_asset_saves
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>();
        for result in pending_asset_saves {
            if let Err(err) = result {
                self.app
                    .set_error_status(format!("Browser asset storage save error: {err}"));
            }
        }
        for err in crate::audio::drain_browser_audio_errors() {
            self.app.set_error_status(err);
        }
        let pending_midi = self.pending_midi.borrow_mut().drain(..).collect::<Vec<_>>();
        for result in pending_midi {
            match result {
                BrowserMidiServiceResult::Inputs(Ok(inputs)) => {
                    self.app.apply_browser_midi_inputs(inputs);
                }
                BrowserMidiServiceResult::Inputs(Err(err)) => self
                    .app
                    .set_error_status(format!("MIDI refresh error: {err}")),
                BrowserMidiServiceResult::Connected(Ok(name)) => {
                    self.app.connect_browser_midi_input(name);
                    should_persist_settings = true;
                }
                BrowserMidiServiceResult::Connected(Err(err)) => self
                    .app
                    .set_error_status(format!("MIDI connection error: {err}")),
            }
        }
        for message in browser_drain_midi_messages() {
            self.app.handle_browser_midi_message(&message);
        }
        for event in browser_drain_wheel_events() {
            self.handle_browser_wheel_event(event);
        }
        for event in browser_drain_workspace_pointer_events() {
            self.handle_browser_workspace_pointer_event(event);
        }
        for (action, edit) in browser_drain_text_edits() {
            handle_text_edit_action(&mut self.app, &action, edit);
            should_persist_settings = true;
            should_persist_project = true;
        }
        for action in browser_drain_keyboard_actions() {
            self.handle_browser_activate_action(&action);
        }
        if should_persist_settings {
            self.persist_browser_settings();
        }
        if should_persist_project {
            self.persist_browser_project_snapshot();
        }
    }

    fn begin_browser_text_open(
        &mut self,
        kind: BrowserTextFileKind,
        accept: &'static str,
        status: &'static str,
    ) {
        self.app.last_status = status.to_string();
        let pending = self.pending_text_files.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = browser_open_text_file(accept).await;
            pending
                .borrow_mut()
                .push(PendingBrowserTextFile { kind, result });
        });
    }

    fn begin_browser_project_open(&mut self) {
        if !self.app.request_open_project() {
            return;
        }
        self.begin_browser_text_open(
            BrowserTextFileKind::Project,
            ".orbifold,.mtdaw,text/plain",
            "Opening browser project...",
        );
    }

    fn handle_browser_activate_action(&mut self, action_name: &str) {
        let action = canonical_action_name(action_name);
        match action {
            "file.open" => self.begin_browser_project_open(),
            "file.save" | "file.save_as" => self.download_browser_project(),
            "scale.open" | "scale.import" => self.begin_browser_text_open(
                BrowserTextFileKind::Scale,
                ".scl,text/plain",
                "Opening browser Scala file...",
            ),
            "keymap.open" => self.begin_browser_text_open(
                BrowserTextFileKind::KeyMap,
                ".ltn,text/plain",
                "Opening browser key map...",
            ),
            "asset.import" => self.begin_browser_asset_import(),
            "midi.refresh" => self.begin_browser_midi_refresh(),
            "midi.connect" => self.begin_browser_midi_connect(),
            _ => dispatch_action(&mut self.app, action, None, None),
        }
        self.publish_browser_action_result(action);
        self.persist_browser_settings();
        self.persist_browser_project_snapshot();
        self.reload_after_ui_scale_action(action);
    }

    fn publish_browser_action_result(&self, action: &str) {
        let note_count = self.app.music_project.lock().clip.notes.len();
        publish_action_result_js(action, note_count as f64, &self.app.last_status);
    }

    fn download_browser_project(&mut self) {
        let (file_name, text) = self.app.browser_project_download_payload();
        match browser_download_text_file(&file_name, &text) {
            Ok(()) => self.app.mark_browser_project_downloaded(&file_name),
            Err(err) => self
                .app
                .set_error_status(format!("Browser project download error: {err}")),
        }
    }

    fn begin_browser_asset_import(&mut self) {
        let kind = self.app.selected_audio_asset_kind;
        let accept = browser_asset_accept(kind);
        self.app.last_status = format!("Opening browser {} import...", kind.singular_label());
        let pending = self.pending_binary_files.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = browser_open_binary_file(&accept).await;
            pending
                .borrow_mut()
                .push(PendingBrowserBinaryFile { kind, result });
        });
    }

    fn begin_browser_asset_storage_save(
        &mut self,
        path: std::path::PathBuf,
        kind: AudioAssetKind,
        file_name: String,
        bytes: Vec<u8>,
    ) {
        let pending = self.pending_asset_saves.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = browser_save_asset_storage_record(&path, kind, &file_name, &bytes).await;
            pending.borrow_mut().push(result);
        });
    }

    fn begin_browser_midi_refresh(&mut self) {
        self.app.last_status = "Refreshing browser MIDI inputs...".to_string();
        let pending = self.pending_midi.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = browser_request_midi_inputs().await;
            pending
                .borrow_mut()
                .push(BrowserMidiServiceResult::Inputs(result));
        });
    }

    fn begin_browser_midi_connect(&mut self) {
        let selected = self
            .app
            .midi_inputs
            .get(self.app.selected_input)
            .cloned()
            .unwrap_or_default();
        self.app.last_status = if selected.is_empty() {
            "Connecting first browser MIDI input...".to_string()
        } else {
            format!("Connecting browser MIDI input: {selected}")
        };
        let pending = self.pending_midi.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = browser_connect_midi_input(&selected).await;
            pending
                .borrow_mut()
                .push(BrowserMidiServiceResult::Connected(result));
        });
    }

    fn persist_browser_settings(&mut self) {
        let text = self.app.browser_settings_text();
        if let Err(err) = browser_save_settings_text(&text) {
            self.app
                .set_error_status(format!("Browser settings save error: {err}"));
        }
    }

    fn persist_browser_project_snapshot(&mut self) {
        let text = self.app.browser_project_storage_text();
        if let Err(err) = browser_save_project_storage_text(&text) {
            self.app
                .set_error_status(format!("Browser project session save error: {err}"));
        }
    }

    fn reload_after_ui_scale_action(&self, action: &str) {
        if web_ui_scale_action(action) {
            browser_reload_window();
        }
    }

    fn view(&self, viewport: UiSize) -> UiDocument {
        let width = viewport.width.max(MIN_WIDTH);
        let height = viewport.height.max(MIN_HEIGHT);
        super::native::build_surface_document_for_interaction(
            &self.app,
            width,
            height,
            self.has_active_pointer_drag(),
        )
    }
}

fn widget_drag_phase(phase: WidgetDragPhase) -> WidgetValueEditPhase {
    match phase {
        WidgetDragPhase::Begin => WidgetValueEditPhase::Begin,
        WidgetDragPhase::Update => WidgetValueEditPhase::Update,
        WidgetDragPhase::Commit => WidgetValueEditPhase::Commit,
        WidgetDragPhase::Cancel => WidgetValueEditPhase::Cancel,
    }
}

fn widget_value_edit_phase_label(phase: WidgetValueEditPhase) -> &'static str {
    match phase {
        WidgetValueEditPhase::Begin => "begin",
        WidgetValueEditPhase::Update => "update",
        WidgetValueEditPhase::Commit => "commit",
        WidgetValueEditPhase::Cancel => "cancel",
        WidgetValueEditPhase::Preview => "preview",
    }
}

fn web_ui_scale_action(action: &str) -> bool {
    matches!(
        action,
        "ui.scale_down"
            | "ui.scale_reset"
            | "ui.scale_up"
            | "settings.ui.scale_down"
            | "settings.ui.scale_reset"
            | "settings.ui.scale_up"
    )
}

fn browser_reload_window() {
    reload_browser_window_js();
}

fn web_workspace_resize_target_from_action(action: &str) -> Option<WorkspaceResizeTarget> {
    match action {
        "layout.resize.left" => Some(WorkspaceResizeTarget::Left),
        "layout.resize.track" => Some(WorkspaceResizeTarget::Track),
        "layout.resize.right" => Some(WorkspaceResizeTarget::Right),
        "layout.resize.bottom" => Some(WorkspaceResizeTarget::Bottom),
        "layout.resize.browser" => Some(WorkspaceResizeTarget::Browser),
        _ => None,
    }
}

fn web_loop_end_drag_mode_from_action(action: &str) -> Option<WebLoopEndDragMode> {
    match action {
        "transport.loop_end" => Some(WebLoopEndDragMode::Arrangement),
        "piano.loop_end" => Some(WebLoopEndDragMode::Piano),
        _ => None,
    }
}

fn web_timeline_drag_mode_from_action(action: &str) -> Option<WebTimelineDragMode> {
    match action {
        "transport.seek" => Some(WebTimelineDragMode::Arrangement),
        "piano.seek" => Some(WebTimelineDragMode::Piano),
        _ => None,
    }
}

fn web_piano_viewport_drag_mode_from_action(action: &str) -> Option<WebPianoViewportDragMode> {
    match action {
        "piano.viewport.time" => Some(WebPianoViewportDragMode::Time),
        "piano.viewport.pitch" => Some(WebPianoViewportDragMode::Pitch),
        _ => None,
    }
}

fn point_distance(a: UiPoint, b: UiPoint) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

fn web_note_drag_from_action(action: &str) -> Option<(u64, WebNoteDragMode)> {
    if let Some(note_id) = action.strip_prefix("note.select.") {
        return note_id
            .parse::<u64>()
            .ok()
            .map(|note_id| (note_id, WebNoteDragMode::Move));
    }
    if let Some(note_id) = action.strip_prefix("note.resize_start.") {
        return note_id
            .parse::<u64>()
            .ok()
            .map(|note_id| (note_id, WebNoteDragMode::ResizeStart));
    }
    if let Some(note_id) = action.strip_prefix("note.resize_end.") {
        return note_id
            .parse::<u64>()
            .ok()
            .map(|note_id| (note_id, WebNoteDragMode::ResizeEnd));
    }
    if let Some(note_id) = action.strip_prefix("note.velocity.") {
        return note_id
            .parse::<u64>()
            .ok()
            .map(|note_id| (note_id, WebNoteDragMode::Velocity));
    }
    None
}

fn browser_load_settings_text() -> Result<Option<String>, String> {
    let value = load_browser_settings_text_js().map_err(js_error_message)?;
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    Ok(value.as_string().filter(|text| !text.trim().is_empty()))
}

fn browser_save_settings_text(text: &str) -> Result<(), String> {
    save_browser_settings_text_js(text).map_err(js_error_message)
}

fn browser_project_resources_for_text(
    text: &str,
) -> (BrowserProjectTextResource, BrowserProjectTextResource) {
    let Ok(project) = ProjectFile::from_text(text) else {
        return (None, None);
    };
    let scale = project.scala_path.and_then(|path| {
        browser_load_text_resource("scale", &path.to_string_lossy()).map(|text| (path, text))
    });
    let keymap = project.lumatone_path.and_then(|path| {
        browser_load_text_resource("keymap", &path.to_string_lossy()).map(|text| (path, text))
    });
    (scale, keymap)
}

fn browser_save_text_resource(kind: &str, file_name: &str, text: &str) -> Result<(), String> {
    save_browser_text_resource_js(kind, file_name, text).map_err(js_error_message)
}

fn browser_load_text_resource(kind: &str, file_name: &str) -> Option<String> {
    load_browser_text_resource_js(kind, file_name).as_string()
}

fn browser_load_project_storage_text() -> Result<Option<String>, String> {
    let value = load_browser_project_storage_text_js().map_err(js_error_message)?;
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    Ok(value.as_string().filter(|text| !text.trim().is_empty()))
}

fn browser_save_project_storage_text(text: &str) -> Result<(), String> {
    save_browser_project_storage_text_js(text).map_err(js_error_message)
}

fn install_browser_keyboard_shortcuts() -> Result<(), String> {
    install_browser_keyboard_shortcuts_js().map_err(js_error_message)
}

async fn browser_load_asset_storage_records() -> Result<Vec<BrowserAssetStorageRecord>, String> {
    let records = Array::from(
        &load_browser_asset_storage_records_js()
            .await
            .map_err(js_error_message)?,
    );
    let mut out = Vec::new();
    for index in 0..records.length() {
        let entry = Array::from(&records.get(index));
        if entry.length() != 4 {
            log::error!("Browser asset storage skipped invalid record at index {index}");
            continue;
        }
        let Some(path) = entry.get(0).as_string().filter(|path| !path.is_empty()) else {
            log::error!("Browser asset storage skipped record with missing path at index {index}");
            continue;
        };
        let kind_index = entry.get(1).as_f64().unwrap_or(f64::NAN);
        if !kind_index.is_finite() || kind_index < 0.0 {
            log::error!("Browser asset storage skipped record with invalid kind at index {index}");
            continue;
        }
        let Some(kind) = AudioAssetKind::from_index(kind_index as usize) else {
            log::error!("Browser asset storage skipped record with invalid kind at index {index}");
            continue;
        };
        let Some(file_name) = entry.get(2).as_string().filter(|name| !name.is_empty()) else {
            log::error!(
                "Browser asset storage skipped record with missing file name at index {index}"
            );
            continue;
        };
        let bytes = js_sys::Uint8Array::new(&entry.get(3)).to_vec();
        if bytes.is_empty() {
            log::error!("Browser asset storage skipped empty asset at index {index}: {path}");
            continue;
        }
        out.push(BrowserAssetStorageRecord {
            path: std::path::PathBuf::from(path),
            kind,
            file_name,
            bytes,
        });
    }
    Ok(out)
}

async fn browser_save_asset_storage_record(
    path: &std::path::Path,
    kind: AudioAssetKind,
    file_name: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let bytes = js_sys::Uint8Array::from(bytes);
    save_browser_asset_storage_record_js(&path.to_string_lossy(), kind.index(), file_name, &bytes)
        .await
        .map_err(js_error_message)
}

fn browser_drain_wheel_events() -> Vec<BrowserWheelEvent> {
    let events = Array::from(&drain_wheel_events_js());
    let mut out = Vec::new();
    for index in 0..events.length() {
        let entry = Array::from(&events.get(index));
        if entry.length() != 9 {
            continue;
        }
        let Some(x) = entry.get(0).as_f64() else {
            continue;
        };
        let Some(y) = entry.get(1).as_f64() else {
            continue;
        };
        let Some(dx) = entry.get(2).as_f64() else {
            continue;
        };
        let Some(dy) = entry.get(3).as_f64() else {
            continue;
        };
        let delta_mode = entry.get(4).as_f64().unwrap_or(0.0).round().max(0.0) as u32;
        out.push(BrowserWheelEvent {
            position: UiPoint::new(x as f32, y as f32),
            delta: UiPoint::new(dx as f32, dy as f32),
            delta_mode,
            shift: entry.get(5).as_bool().unwrap_or(false),
            ctrl: entry.get(6).as_bool().unwrap_or(false),
            alt: entry.get(7).as_bool().unwrap_or(false),
            meta: entry.get(8).as_bool().unwrap_or(false),
        });
    }
    out
}

fn browser_drain_workspace_pointer_events() -> Vec<BrowserWorkspacePointerEvent> {
    let events = Array::from(&drain_workspace_pointer_events_js());
    let mut out = Vec::new();
    for index in 0..events.length() {
        let entry = Array::from(&events.get(index));
        if entry.length() != 3 {
            continue;
        }
        let Some(phase) = entry
            .get(0)
            .as_string()
            .and_then(|phase| browser_workspace_pointer_phase(&phase))
        else {
            continue;
        };
        let Some(x) = entry.get(1).as_f64() else {
            continue;
        };
        let Some(y) = entry.get(2).as_f64() else {
            continue;
        };
        out.push(BrowserWorkspacePointerEvent {
            phase,
            position: UiPoint::new(x as f32, y as f32),
        });
    }
    out
}

fn browser_workspace_pointer_phase(phase: &str) -> Option<WidgetValueEditPhase> {
    match phase {
        "begin" => Some(WidgetValueEditPhase::Begin),
        "update" => Some(WidgetValueEditPhase::Update),
        "commit" => Some(WidgetValueEditPhase::Commit),
        "cancel" => Some(WidgetValueEditPhase::Cancel),
        _ => None,
    }
}

async fn browser_open_text_file(accept: &str) -> Result<Option<BrowserTextFile>, String> {
    let value = open_text_file_js(accept).await.map_err(js_error_message)?;
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let array = Array::from(&value);
    if array.length() != 2 {
        return Err("Browser file picker returned an invalid result".to_string());
    }
    let name = array
        .get(0)
        .as_string()
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "Browser file picker returned no file name".to_string())?;
    let text = array
        .get(1)
        .as_string()
        .ok_or_else(|| "Browser file picker returned no text".to_string())?;
    Ok(Some(BrowserTextFile { name, text }))
}

async fn browser_open_binary_file(accept: &str) -> Result<Option<BrowserBinaryFile>, String> {
    let value = open_binary_file_js(accept)
        .await
        .map_err(js_error_message)?;
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let array = Array::from(&value);
    if array.length() != 2 {
        return Err("Browser file picker returned an invalid result".to_string());
    }
    let name = array
        .get(0)
        .as_string()
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "Browser file picker returned no file name".to_string())?;
    let bytes = js_sys::Uint8Array::new(&array.get(1)).to_vec();
    Ok(Some(BrowserBinaryFile { name, bytes }))
}

fn browser_download_text_file(file_name: &str, text: &str) -> Result<(), String> {
    download_text_file_js(file_name, text).map_err(js_error_message)
}

fn browser_asset_accept(kind: AudioAssetKind) -> String {
    kind.extensions()
        .iter()
        .map(|extension| format!(".{extension}"))
        .collect::<Vec<_>>()
        .join(",")
}

async fn browser_request_midi_inputs() -> Result<Vec<String>, String> {
    let value = request_midi_inputs_js().await.map_err(js_error_message)?;
    Ok(js_string_array_lossy(value))
}

async fn browser_connect_midi_input(selected: &str) -> Result<String, String> {
    connect_midi_input_js(selected)
        .await
        .map_err(js_error_message)?
        .as_string()
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "Browser MIDI connection returned no input name".to_string())
}

fn browser_drain_midi_messages() -> Vec<Vec<u8>> {
    let messages = Array::from(&drain_midi_messages_js());
    let mut out = Vec::new();
    for index in 0..messages.length() {
        let bytes = Array::from(&messages.get(index));
        let mut message = Vec::new();
        for byte_index in 0..bytes.length() {
            if let Some(byte) = bytes.get(byte_index).as_f64() {
                message.push((byte as i32).clamp(0, 255) as u8);
            }
        }
        if !message.is_empty() {
            out.push(message);
        }
    }
    out
}

fn browser_drain_keyboard_actions() -> Vec<String> {
    js_string_array_lossy(drain_keyboard_actions_js())
}

fn browser_drain_text_edits() -> Vec<(String, WidgetTextEdit)> {
    let edits = Array::from(&drain_text_edit_actions_js());
    let mut out = Vec::new();
    for index in 0..edits.length() {
        let item = edits.get(index);
        let Some(action) = js_object_string(&item, "action") else {
            continue;
        };
        let Some(event_kind) = js_object_string(&item, "event") else {
            continue;
        };
        let Some(event) = browser_text_edit_event(&event_kind, &item) else {
            continue;
        };
        let mut edit = WidgetTextEdit::new(event);
        edit.phase = WidgetValueEditPhase::Commit;
        out.push((action, edit));
    }
    out
}

fn browser_text_edit_event(kind: &str, item: &JsValue) -> Option<UiInputEvent> {
    match kind {
        "text" => Some(UiInputEvent::TextInput(
            js_object_string(item, "value").unwrap_or_default(),
        )),
        "key" => browser_text_edit_key(item).map(|key| UiInputEvent::Key {
            key,
            modifiers: KeyModifiers::NONE,
        }),
        _ => None,
    }
}

fn browser_text_edit_key(item: &JsValue) -> Option<KeyCode> {
    match js_object_string(item, "key")?.as_str() {
        "Backspace" => Some(KeyCode::Backspace),
        "Delete" => Some(KeyCode::Delete),
        "Enter" => Some(KeyCode::Enter),
        "Escape" => Some(KeyCode::Escape),
        key if key.chars().count() == 1 => key.chars().next().map(KeyCode::Character),
        _ => None,
    }
}

fn js_object_string(item: &JsValue, key: &str) -> Option<String> {
    Reflect::get(item, &JsValue::from_str(key))
        .ok()
        .and_then(|value| value.as_string())
        .filter(|value| !value.is_empty())
}

fn js_string_array(value: JsValue, context: &str) -> Result<Vec<String>, String> {
    let array = Array::from(&value);
    let mut strings = Vec::new();
    for index in 0..array.length() {
        if let Some(value) = array.get(index).as_string()
            && !value.trim().is_empty()
        {
            strings.push(value);
        }
    }
    if strings.is_empty() {
        return Err(format!("{context} was empty"));
    }
    Ok(strings)
}

fn js_string_array_lossy(value: JsValue) -> Vec<String> {
    let array = Array::from(&value);
    let mut strings = Vec::new();
    for index in 0..array.length() {
        if let Some(value) = array.get(index).as_string()
            && !value.trim().is_empty()
        {
            strings.push(value);
        }
    }
    strings
}

fn js_error_message(value: JsValue) -> String {
    value
        .as_string()
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| format!("{value:?}"))
}

fn mark_browser_runtime_ready(frame_count: u64, viewport: UiSize) {
    mark_runtime_ready_js(
        frame_count as f64,
        viewport.width as f64,
        viewport.height as f64,
    );
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
const ORBIFOLD_SETTINGS_KEY = "orbifold.settings.v1";
const ORBIFOLD_PROJECT_STORAGE_KEY = "orbifold.project.v1";
const ORBIFOLD_TEXT_RESOURCE_STORAGE_KEY = "orbifold.browser_text_resources.v1";
const ORBIFOLD_ASSET_STORAGE_KEY = "orbifold.browser_assets.v1";
const ORBIFOLD_ASSET_DB_NAME = "orbifold-browser-assets";
const ORBIFOLD_ASSET_DB_VERSION = 1;
const ORBIFOLD_ASSET_STORE_NAME = "assets";
let orbifoldKeyboardShortcutsInstalled = false;
let orbifoldWheelBridgeInstalled = false;
let orbifoldWheelEvents = [];
let orbifoldWorkspacePointerBridgeInstalled = false;
let orbifoldWorkspacePointerActive = false;
let orbifoldWorkspacePointerEvents = [];

function orbifoldActionQueue() {
  if (!Array.isArray(window.__orbifoldActionQueue)) {
    window.__orbifoldActionQueue = [];
  }
  return window.__orbifoldActionQueue;
}

function orbifoldTextEditQueue() {
  if (!Array.isArray(window.__orbifoldTextEditQueue)) {
    window.__orbifoldTextEditQueue = [];
  }
  return window.__orbifoldTextEditQueue;
}

function queueOrbifoldAction(action) {
  if (typeof action !== "string" || action.trim() === "") {
    return false;
  }
  const queue = orbifoldActionQueue();
  queue.push(action);
  document.body.dataset.orbifoldQueuedActionCount = String(queue.length);
  return true;
}

function queueOrbifoldTextEdit(action, event, value) {
  if (typeof action !== "string" || action.trim() === "") {
    return false;
  }
  if (event !== "text" && event !== "key") {
    return false;
  }
  const queue = orbifoldTextEditQueue();
  const item = { action, event };
  if (event === "key") {
    item.key = String(value || "");
  } else {
    item.value = String(value || "");
  }
  queue.push(item);
  document.body.dataset.orbifoldQueuedTextEditCount = String(queue.length);
  return true;
}

export function mark_runtime_ready_js(frameCount, viewportWidth, viewportHeight) {
  if (typeof window.orbifoldRuntimeReady === "function") {
    window.orbifoldRuntimeReady({
      frameCount,
      viewportWidth,
      viewportHeight,
    });
  }
}

export function publish_action_result_js(action, noteCount, status) {
  document.body.dataset.orbifoldLastAction = String(action || "");
  document.body.dataset.orbifoldProjectNoteCount = String(noteCount || 0);
  document.body.dataset.orbifoldLastStatus = String(status || "");
}

export function publish_runtime_state_js(
  status,
  noteCount,
  assetCount,
  midiInputCount,
  connectedMidiInput,
  lastMidiStatus,
  lastMidiNote,
  audioOutputCount,
  connectedAudioOutput,
  audioStreamConnected,
  transportPlaying,
  transportPositionBeats,
  loopBeats,
  uiScale,
  showAssetBrowser,
  showScaleBrowser,
  showClipPanel,
  scaleDescription,
  scalaPath,
  lumatonePath,
  lumatoneLoaded
) {
  document.body.dataset.orbifoldLastStatus = String(status || "");
  document.body.dataset.orbifoldProjectNoteCount = String(noteCount || 0);
  document.body.dataset.orbifoldAudioAssetCount = String(assetCount || 0);
  document.body.dataset.orbifoldMidiInputCount = String(midiInputCount || 0);
  document.body.dataset.orbifoldConnectedMidiInput = String(connectedMidiInput || "");
  document.body.dataset.orbifoldLastMidiStatus = String(lastMidiStatus || 0);
  document.body.dataset.orbifoldLastMidiNote = String(lastMidiNote ?? -1);
  document.body.dataset.orbifoldAudioOutputCount = String(audioOutputCount || 0);
  document.body.dataset.orbifoldConnectedAudioOutput = String(connectedAudioOutput || "");
  document.body.dataset.orbifoldAudioStreamConnected = audioStreamConnected ? "1" : "0";
  document.body.dataset.orbifoldTransportPlaying = transportPlaying ? "1" : "0";
  document.body.dataset.orbifoldTransportPositionBeats = String(transportPositionBeats || 0);
  document.body.dataset.orbifoldLoopBeats = String(loopBeats || 0);
  document.body.dataset.orbifoldUiScale = String(uiScale || 1);
  document.body.dataset.orbifoldShowAssetBrowser = showAssetBrowser ? "1" : "0";
  document.body.dataset.orbifoldShowScaleBrowser = showScaleBrowser ? "1" : "0";
  document.body.dataset.orbifoldShowClipPanel = showClipPanel ? "1" : "0";
  document.body.dataset.orbifoldScaleDescription = String(scaleDescription || "");
  document.body.dataset.orbifoldScalaPath = String(scalaPath || "");
  document.body.dataset.orbifoldLumatonePath = String(lumatonePath || "");
  document.body.dataset.orbifoldLumatoneLoaded = lumatoneLoaded ? "1" : "0";
}

export function publish_automation_geometry_js(
  gridX,
  gridY,
  gridWidth,
  gridHeight,
  addX,
  addY,
  dragStartX,
  dragStartY,
  dragEndX,
  dragEndY,
  resizeStartX,
  resizeStartY,
  resizeEndX,
  resizeEndY,
  pianoViewStart,
  pianoViewBeats,
  pianoMinPitch,
  pianoMaxPitch
) {
  document.body.dataset.orbifoldPianoGridX = String(gridX || 0);
  document.body.dataset.orbifoldPianoGridY = String(gridY || 0);
  document.body.dataset.orbifoldPianoGridWidth = String(gridWidth || 0);
  document.body.dataset.orbifoldPianoGridHeight = String(gridHeight || 0);
  document.body.dataset.orbifoldPianoAddX = String(addX || 0);
  document.body.dataset.orbifoldPianoAddY = String(addY || 0);
  document.body.dataset.orbifoldPianoDragStartX = String(dragStartX || 0);
  document.body.dataset.orbifoldPianoDragStartY = String(dragStartY || 0);
  document.body.dataset.orbifoldPianoDragEndX = String(dragEndX || 0);
  document.body.dataset.orbifoldPianoDragEndY = String(dragEndY || 0);
  document.body.dataset.orbifoldPianoResizeStartX = String(resizeStartX || 0);
  document.body.dataset.orbifoldPianoResizeStartY = String(resizeStartY || 0);
  document.body.dataset.orbifoldPianoResizeEndX = String(resizeEndX || 0);
  document.body.dataset.orbifoldPianoResizeEndY = String(resizeEndY || 0);
  document.body.dataset.orbifoldPianoViewStart = String(pianoViewStart || 0);
  document.body.dataset.orbifoldPianoViewBeats = String(pianoViewBeats || 0);
  document.body.dataset.orbifoldPianoMinPitch = String(pianoMinPitch || 0);
  document.body.dataset.orbifoldPianoMaxPitch = String(pianoMaxPitch || 0);
}

export function publish_text_audit_js(textCount, issueCount, nonFiniteCount, sampleIssue) {
  document.body.dataset.orbifoldTextAuditReady = "1";
  document.body.dataset.orbifoldTextAuditCount = String(textCount || 0);
  document.body.dataset.orbifoldTextAuditIssueCount = String(issueCount || 0);
  document.body.dataset.orbifoldTextAuditNonFiniteCount = String(nonFiniteCount || 0);
  document.body.dataset.orbifoldTextAuditSampleIssue = String(sampleIssue || "");
}

export function publish_pointer_action_js(action, phase, x, y) {
  document.body.dataset.orbifoldLastPointerAction = String(action || "");
  document.body.dataset.orbifoldLastPointerPhase = String(phase || "");
  document.body.dataset.orbifoldLastPointerX = String(x || 0);
  document.body.dataset.orbifoldLastPointerY = String(y || 0);
}

export function publish_layout_automation_js(
  rightResizeX,
  rightResizeY,
  rightResizeEndX,
  rightResizeEndY,
  bottomResizeX,
  bottomResizeY,
  bottomResizeEndX,
  bottomResizeEndY,
  rightPanelWidth,
  pianoRollHeight,
  arrangementSeekStartX,
  arrangementSeekStartY,
  arrangementSeekEndX,
  arrangementSeekEndY,
  pianoSeekStartX,
  pianoSeekStartY,
  pianoSeekEndX,
  pianoSeekEndY,
  arrangementLoopEndStartX,
  arrangementLoopEndStartY,
  arrangementLoopEndTargetX,
  arrangementLoopEndTargetY,
  pianoLoopEndStartX,
  pianoLoopEndStartY,
  pianoLoopEndTargetX,
  pianoLoopEndTargetY
) {
  document.body.dataset.orbifoldRightResizeX = String(rightResizeX || 0);
  document.body.dataset.orbifoldRightResizeY = String(rightResizeY || 0);
  document.body.dataset.orbifoldRightResizeEndX = String(rightResizeEndX || 0);
  document.body.dataset.orbifoldRightResizeEndY = String(rightResizeEndY || 0);
  document.body.dataset.orbifoldBottomResizeX = String(bottomResizeX || 0);
  document.body.dataset.orbifoldBottomResizeY = String(bottomResizeY || 0);
  document.body.dataset.orbifoldBottomResizeEndX = String(bottomResizeEndX || 0);
  document.body.dataset.orbifoldBottomResizeEndY = String(bottomResizeEndY || 0);
  document.body.dataset.orbifoldRightPanelWidth = String(rightPanelWidth || 0);
  document.body.dataset.orbifoldPianoRollHeight = String(pianoRollHeight || 0);
  document.body.dataset.orbifoldArrangementSeekStartX = String(arrangementSeekStartX || 0);
  document.body.dataset.orbifoldArrangementSeekStartY = String(arrangementSeekStartY || 0);
  document.body.dataset.orbifoldArrangementSeekEndX = String(arrangementSeekEndX || 0);
  document.body.dataset.orbifoldArrangementSeekEndY = String(arrangementSeekEndY || 0);
  document.body.dataset.orbifoldPianoSeekStartX = String(pianoSeekStartX || 0);
  document.body.dataset.orbifoldPianoSeekStartY = String(pianoSeekStartY || 0);
  document.body.dataset.orbifoldPianoSeekEndX = String(pianoSeekEndX || 0);
  document.body.dataset.orbifoldPianoSeekEndY = String(pianoSeekEndY || 0);
  document.body.dataset.orbifoldArrangementLoopEndStartX = String(arrangementLoopEndStartX || 0);
  document.body.dataset.orbifoldArrangementLoopEndStartY = String(arrangementLoopEndStartY || 0);
  document.body.dataset.orbifoldArrangementLoopEndTargetX = String(arrangementLoopEndTargetX || 0);
  document.body.dataset.orbifoldArrangementLoopEndTargetY = String(arrangementLoopEndTargetY || 0);
  document.body.dataset.orbifoldPianoLoopEndStartX = String(pianoLoopEndStartX || 0);
  document.body.dataset.orbifoldPianoLoopEndStartY = String(pianoLoopEndStartY || 0);
  document.body.dataset.orbifoldPianoLoopEndTargetX = String(pianoLoopEndTargetX || 0);
  document.body.dataset.orbifoldPianoLoopEndTargetY = String(pianoLoopEndTargetY || 0);
}

export function install_browser_keyboard_shortcuts_js() {
  if (orbifoldKeyboardShortcutsInstalled) {
    return;
  }
  window.orbifoldDispatchAction = queueOrbifoldAction;
  window.orbifoldDispatchTextInput = (action, text) => queueOrbifoldTextEdit(action, "text", text);
  window.orbifoldDispatchTextKey = (action, key) => queueOrbifoldTextEdit(action, "key", key);
  document.body.dataset.orbifoldKeyboardShortcuts = "installed";
  window.addEventListener("keydown", (event) => {
    const action = orbifoldShortcutAction(event);
    if (!action) {
      return;
    }
    event.preventDefault();
    queueOrbifoldAction(action);
  });
  orbifoldKeyboardShortcutsInstalled = true;
}

export function drain_keyboard_actions_js() {
  const queue = orbifoldActionQueue();
  const actions = queue.splice(0, queue.length);
  document.body.dataset.orbifoldQueuedActionCount = "0";
  return actions;
}

export function drain_text_edit_actions_js() {
  const queue = orbifoldTextEditQueue();
  const edits = queue.splice(0, queue.length);
  document.body.dataset.orbifoldQueuedTextEditCount = "0";
  return edits;
}

export function install_browser_wheel_bridge_js(canvasId) {
  if (orbifoldWheelBridgeInstalled) {
    return;
  }
  const canvas = document.getElementById(canvasId);
  if (!canvas) {
    throw `canvas not found: ${canvasId}`;
  }
  canvas.addEventListener("wheel", (event) => {
    const rect = canvas.getBoundingClientRect();
    event.preventDefault();
    orbifoldWheelEvents.push([
      event.clientX - rect.left,
      event.clientY - rect.top,
      -event.deltaX,
      -event.deltaY,
      event.deltaMode || 0,
      !!event.shiftKey,
      !!event.ctrlKey,
      !!event.altKey,
      !!event.metaKey,
    ]);
  }, { passive: false });
  orbifoldWheelBridgeInstalled = true;
}

export function drain_wheel_events_js() {
  const events = orbifoldWheelEvents;
  orbifoldWheelEvents = [];
  return events;
}

export function install_browser_workspace_pointer_bridge_js(canvasId) {
  if (orbifoldWorkspacePointerBridgeInstalled) {
    return;
  }
  const canvas = document.getElementById(canvasId);
  if (!canvas) {
    throw `canvas not found: ${canvasId}`;
  }
  const pushPointerEvent = (phase, event) => {
    const rect = canvas.getBoundingClientRect();
    orbifoldWorkspacePointerEvents.push([
      phase,
      event.clientX - rect.left,
      event.clientY - rect.top,
    ]);
    document.body.dataset.orbifoldLastWorkspacePointerPhase = phase;
  };
  canvas.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) {
      return;
    }
    orbifoldWorkspacePointerActive = true;
    pushPointerEvent("begin", event);
  }, { capture: true });
  window.addEventListener("pointermove", (event) => {
    if (!orbifoldWorkspacePointerActive) {
      return;
    }
    pushPointerEvent("update", event);
  }, { capture: true });
  window.addEventListener("pointerup", (event) => {
    if (!orbifoldWorkspacePointerActive) {
      return;
    }
    pushPointerEvent("commit", event);
    orbifoldWorkspacePointerActive = false;
  }, { capture: true });
  window.addEventListener("pointercancel", (event) => {
    if (!orbifoldWorkspacePointerActive) {
      return;
    }
    pushPointerEvent("cancel", event);
    orbifoldWorkspacePointerActive = false;
  }, { capture: true });
  orbifoldWorkspacePointerBridgeInstalled = true;
}

export function drain_workspace_pointer_events_js() {
  const events = orbifoldWorkspacePointerEvents;
  orbifoldWorkspacePointerEvents = [];
  return events;
}

export function reload_browser_window_js() {
  window.setTimeout(() => window.location.reload(), 25);
}

function orbifoldShortcutAction(event) {
  if (event.repeat && !orbifoldShortcutAllowsRepeat(event)) {
    return null;
  }
  const key = event.key || "";
  const lower = key.toLowerCase();
  const command = event.ctrlKey || event.metaKey;
  const plain = !command && !event.altKey;
  if (command && !event.altKey) {
    if (lower === "s") {
      return event.shiftKey ? "file.save_as" : "file.save";
    }
    if (lower === "n") {
      return "file.new";
    }
    if (lower === "o") {
      return "file.open";
    }
    if (lower === "c" && !event.shiftKey) {
      return "clip.copy_note";
    }
    if (lower === "v" && !event.shiftKey) {
      return "clip.paste_note";
    }
    if (lower === "z") {
      return event.shiftKey ? "edit.redo" : "edit.undo";
    }
    if (lower === "y") {
      return "edit.redo";
    }
    if (key === "+" || key === "=") {
      return "ui.scale_up";
    }
    if (key === "-") {
      return "ui.scale_down";
    }
    if (key === "0") {
      return "ui.scale_reset";
    }
    return null;
  }
  if (!plain) {
    return null;
  }
  if (key === " ") {
    return "transport.play_stop";
  }
  if (key === "Home") {
    return "transport.prev";
  }
  if (key === "Escape") {
    return "edit.escape";
  }
  if (key === "?" || (event.shiftKey && key === "/")) {
    return "help.shortcuts";
  }
  if (key === "Delete" || key === "Backspace") {
    return "clip.delete_note";
  }
  if (key === "ArrowLeft") {
    return event.shiftKey ? "clip.shorter" : "clip.nudge_left";
  }
  if (key === "ArrowRight") {
    return event.shiftKey ? "clip.longer" : "clip.nudge_right";
  }
  if (key === "ArrowDown") {
    return event.shiftKey ? "clip.velocity_down" : "clip.pitch_down";
  }
  if (key === "ArrowUp") {
    return event.shiftKey ? "clip.velocity_up" : "clip.pitch_up";
  }
  if (key === "+" || key === "=") {
    return "piano.zoom_in";
  }
  if (key === "-") {
    return "piano.zoom_out";
  }
  if (lower === "r") {
    return "transport.record";
  }
  if (lower === "m") {
    return "transport.metronome";
  }
  if (lower === "q") {
    return event.shiftKey ? "transport.record_quantize" : "clip.quantize";
  }
  if (lower === "g") {
    return "transport.snap";
  }
  if (lower === "p") {
    return "audio.all_off";
  }
  if (lower === "d") {
    return "clip.duplicate_note";
  }
  if (lower === "n" && !event.shiftKey) {
    return "clip.add_note";
  }
  return null;
}

function orbifoldShortcutAllowsRepeat(event) {
  const command = event.ctrlKey || event.metaKey;
  return !command
    && !event.altKey
    && ["ArrowLeft", "ArrowRight", "ArrowDown", "ArrowUp"].includes(event.key || "");
}

export function load_browser_settings_text_js() {
  try {
    if (!window.localStorage) {
      return null;
    }
    return window.localStorage.getItem(ORBIFOLD_SETTINGS_KEY);
  } catch (error) {
    throw error && error.message ? error.message : String(error);
  }
}

export function save_browser_settings_text_js(text) {
  try {
    if (!window.localStorage) {
      throw "localStorage is not available";
    }
    window.localStorage.setItem(ORBIFOLD_SETTINGS_KEY, text || "");
  } catch (error) {
    throw error && error.message ? error.message : String(error);
  }
}

export function load_browser_project_storage_text_js() {
  try {
    if (!window.localStorage) {
      return null;
    }
    return window.localStorage.getItem(ORBIFOLD_PROJECT_STORAGE_KEY);
  } catch (error) {
    throw error && error.message ? error.message : String(error);
  }
}

export function save_browser_project_storage_text_js(text) {
  try {
    if (!window.localStorage) {
      throw "localStorage is not available";
    }
    window.localStorage.setItem(ORBIFOLD_PROJECT_STORAGE_KEY, text || "");
  } catch (error) {
    throw error && error.message ? error.message : String(error);
  }
}

function browserTextResourceKey(kind, fileName) {
  return `${String(kind || "")}:${String(fileName || "")}`;
}

function browserTextResourceRecords() {
  if (!window.localStorage) {
    return {};
  }
  const text = window.localStorage.getItem(ORBIFOLD_TEXT_RESOURCE_STORAGE_KEY);
  if (!text) {
    return {};
  }
  const parsed = JSON.parse(text);
  return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
}

export function save_browser_text_resource_js(kind, fileName, text) {
  try {
    if (!window.localStorage) {
      throw "localStorage is not available";
    }
    const records = browserTextResourceRecords();
    records[browserTextResourceKey(kind, fileName)] = String(text || "");
    window.localStorage.setItem(ORBIFOLD_TEXT_RESOURCE_STORAGE_KEY, JSON.stringify(records));
  } catch (error) {
    throw error && error.message ? error.message : String(error);
  }
}

export function load_browser_text_resource_js(kind, fileName) {
  try {
    if (!window.localStorage) {
      return null;
    }
    const records = browserTextResourceRecords();
    const text = records[browserTextResourceKey(kind, fileName)];
    return typeof text === "string" && text.length > 0 ? text : null;
  } catch (error) {
    return null;
  }
}

function browserLegacyAssetStorageRecords() {
  if (!window.localStorage) {
    return [];
  }
  const text = window.localStorage.getItem(ORBIFOLD_ASSET_STORAGE_KEY);
  if (!text) {
    return [];
  }
  const parsed = JSON.parse(text);
  return Array.isArray(parsed) ? parsed : [];
}

function legacyAssetStorageRecords() {
  return browserLegacyAssetStorageRecords()
    .filter((record) => record && record.path && record.fileName && record.bytes)
    .map((record) => [
      String(record.path),
      Number(record.kind || 0),
      String(record.fileName),
      base64ToBytes(record.bytes),
    ]);
}

function safeLegacyAssetStorageRecords() {
  try {
    return { records: legacyAssetStorageRecords(), error: null };
  } catch (error) {
    return {
      records: [],
      error: error && error.message ? error.message : String(error),
    };
  }
}

function bytesToBase64(bytes) {
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes || []);
  let binary = "";
  const chunkSize = 0x8000;
  for (let index = 0; index < view.length; index += chunkSize) {
    binary += String.fromCharCode.apply(null, view.subarray(index, index + chunkSize));
  }
  return btoa(binary);
}

function base64ToBytes(text) {
  const binary = atob(text || "");
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function browserAssetStorageOpenRequest() {
  if (!window.indexedDB) {
    throw "IndexedDB is not available";
  }
  return window.indexedDB.open(ORBIFOLD_ASSET_DB_NAME, ORBIFOLD_ASSET_DB_VERSION);
}

function indexedDbRequest(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || "IndexedDB request failed");
  });
}

function indexedDbTransactionComplete(transaction) {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error || "IndexedDB transaction failed");
    transaction.onabort = () => reject(transaction.error || "IndexedDB transaction aborted");
  });
}

async function openBrowserAssetDb() {
  const request = browserAssetStorageOpenRequest();
  request.onupgradeneeded = () => {
    const db = request.result;
    if (!db.objectStoreNames.contains(ORBIFOLD_ASSET_STORE_NAME)) {
      db.createObjectStore(ORBIFOLD_ASSET_STORE_NAME, { keyPath: "path" });
    }
  };
  return await indexedDbRequest(request);
}

function assetRecordBytes(value) {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (Array.isArray(value)) {
    return new Uint8Array(value);
  }
  if (value && value.buffer instanceof ArrayBuffer) {
    return new Uint8Array(value.buffer);
  }
  return new Uint8Array();
}

async function loadBrowserAssetStorageRecordsFromIndexedDb() {
  const db = await openBrowserAssetDb();
  try {
    const transaction = db.transaction(ORBIFOLD_ASSET_STORE_NAME, "readonly");
    const records = await indexedDbRequest(transaction.objectStore(ORBIFOLD_ASSET_STORE_NAME).getAll());
    await indexedDbTransactionComplete(transaction);
    return records
      .filter((record) => record && record.path && record.fileName && record.bytes)
      .map((record) => [
        String(record.path),
        Number(record.kind || 0),
        String(record.fileName),
        assetRecordBytes(record.bytes),
      ]);
  } finally {
    db.close();
  }
}

async function saveBrowserAssetStorageRecordToIndexedDb(path, kind, fileName, bytes) {
  const db = await openBrowserAssetDb();
  try {
    const transaction = db.transaction(ORBIFOLD_ASSET_STORE_NAME, "readwrite");
    transaction.objectStore(ORBIFOLD_ASSET_STORE_NAME).put({
      path: String(path || ""),
      kind: Number(kind || 0),
      fileName: String(fileName || ""),
      bytes: new Uint8Array(bytes || []),
    });
    await indexedDbTransactionComplete(transaction);
  } finally {
    db.close();
  }
}

function saveLegacyBrowserAssetStorageRecord(path, kind, fileName, bytes) {
  if (!window.localStorage) {
    throw "localStorage is not available";
  }
  const pathText = String(path || "");
  const records = browserLegacyAssetStorageRecords()
    .filter((record) => record && record.path !== pathText);
  records.push({
    path: pathText,
    kind: Number(kind || 0),
    fileName: String(fileName || ""),
    bytes: bytesToBase64(bytes),
  });
  window.localStorage.setItem(ORBIFOLD_ASSET_STORAGE_KEY, JSON.stringify(records));
}

function browserAssetStoragePathSet(records) {
  return new Set(records.map((record) => String(record[0] || "")));
}

async function migrateMissingLegacyAssetRecordsToIndexedDb(legacyRecords, indexedRecords) {
  const indexedPaths = browserAssetStoragePathSet(indexedRecords);
  for (const record of legacyRecords) {
    if (!indexedPaths.has(String(record[0] || ""))) {
      await saveBrowserAssetStorageRecordToIndexedDb(record[0], record[1], record[2], record[3]);
    }
  }
}

function mergeBrowserAssetStorageRecords(legacyRecords, indexedRecords) {
  const merged = new Map();
  for (const record of legacyRecords) {
    merged.set(String(record[0] || ""), record);
  }
  for (const record of indexedRecords) {
    merged.set(String(record[0] || ""), record);
  }
  return Array.from(merged.values());
}

export async function load_browser_asset_storage_records_js() {
  try {
    const legacy = safeLegacyAssetStorageRecords();
    const legacyRecords = legacy.records;
    if (!window.indexedDB) {
      if (legacy.error) {
        throw `Legacy browser asset storage error: ${legacy.error}`;
      }
      return legacyRecords;
    }
    try {
      const indexedRecords = await loadBrowserAssetStorageRecordsFromIndexedDb();
      if (indexedRecords.length > 0) {
        await migrateMissingLegacyAssetRecordsToIndexedDb(legacyRecords, indexedRecords);
        return mergeBrowserAssetStorageRecords(legacyRecords, indexedRecords);
      }
      await migrateMissingLegacyAssetRecordsToIndexedDb(legacyRecords, indexedRecords);
      if (legacy.error) {
        throw `Legacy browser asset storage error: ${legacy.error}`;
      }
      return legacyRecords;
    } catch (error) {
      if (legacyRecords.length > 0) {
        return legacyRecords;
      }
      if (legacy.error) {
        throw `IndexedDB asset load error: ${error && error.message ? error.message : String(error)}; legacy storage error: ${legacy.error}`;
      }
      throw error;
    }
  } catch (error) {
    throw error && error.message ? error.message : String(error);
  }
}

export async function save_browser_asset_storage_record_js(path, kind, fileName, bytes) {
  try {
    if (window.indexedDB) {
      try {
        await saveBrowserAssetStorageRecordToIndexedDb(path, kind, fileName, bytes);
        return;
      } catch (indexedError) {
        try {
          saveLegacyBrowserAssetStorageRecord(path, kind, fileName, bytes);
        } catch (legacyError) {
          throw `IndexedDB asset save error: ${indexedError && indexedError.message ? indexedError.message : String(indexedError)}; localStorage fallback error: ${legacyError && legacyError.message ? legacyError.message : String(legacyError)}`;
        }
        return;
      }
    }
    saveLegacyBrowserAssetStorageRecord(path, kind, fileName, bytes);
  } catch (error) {
    throw error && error.message ? error.message : String(error);
  }
}

export async function open_text_file_js(accept) {
  return await new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept || "text/plain";
    input.style.position = "fixed";
    input.style.left = "-10000px";
    input.style.top = "0";
    document.body.appendChild(input);

    let completed = false;
    const cleanup = () => {
      input.remove();
      window.removeEventListener("focus", onFocus);
    };
    const finish = (value, isError = false) => {
      if (completed) {
        return;
      }
      completed = true;
      cleanup();
      if (isError) {
        reject(value);
      } else {
        resolve(value);
      }
    };
    const onFocus = () => {
      window.setTimeout(() => {
        if (!completed && (!input.files || input.files.length === 0)) {
          finish(null);
        }
      }, 250);
    };

    input.addEventListener("change", async () => {
      const file = input.files && input.files[0];
      if (!file) {
        finish(null);
        return;
      }
      try {
        finish([file.name, await file.text()]);
      } catch (error) {
        finish(error && error.message ? error.message : String(error), true);
      }
    }, { once: true });
    window.addEventListener("focus", onFocus);
    input.click();
  });
}

export async function open_binary_file_js(accept) {
  return await new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept || "";
    input.style.position = "fixed";
    input.style.left = "-10000px";
    input.style.top = "0";
    document.body.appendChild(input);

    let completed = false;
    const cleanup = () => {
      input.remove();
      window.removeEventListener("focus", onFocus);
    };
    const finish = (value, isError = false) => {
      if (completed) {
        return;
      }
      completed = true;
      cleanup();
      if (isError) {
        reject(value);
      } else {
        resolve(value);
      }
    };
    const onFocus = () => {
      window.setTimeout(() => {
        if (!completed && (!input.files || input.files.length === 0)) {
          finish(null);
        }
      }, 250);
    };

    input.addEventListener("change", async () => {
      const file = input.files && input.files[0];
      if (!file) {
        finish(null);
        return;
      }
      try {
        finish([file.name, new Uint8Array(await file.arrayBuffer())]);
      } catch (error) {
        finish(error && error.message ? error.message : String(error), true);
      }
    }, { once: true });
    window.addEventListener("focus", onFocus);
    input.click();
  });
}

export function download_text_file_js(fileName, text) {
  document.body.dataset.orbifoldLastDownloadFileName = String(fileName || "project.orbifold");
  document.body.dataset.orbifoldLastDownloadSize = String((text || "").length);
  window.__orbifoldLastDownloadText = String(text || "");
  const blob = new Blob([text], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName || "project.orbifold";
  anchor.style.display = "none";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}

let orbifoldMidiAccess = null;
let orbifoldMidiInput = null;
let orbifoldMidiMessages = [];

async function ensureMidiAccess() {
  if (!navigator.requestMIDIAccess) {
    throw "Web MIDI is not available in this browser";
  }
  if (!orbifoldMidiAccess) {
    orbifoldMidiAccess = await navigator.requestMIDIAccess({ sysex: false });
  }
  return orbifoldMidiAccess;
}

function midiInputs(access) {
  return Array.from(access.inputs.values());
}

function midiInputName(input) {
  return input.name || input.id || "MIDI input";
}

export async function request_midi_inputs_js() {
  const access = await ensureMidiAccess();
  return midiInputs(access).map(midiInputName);
}

export async function connect_midi_input_js(selectedName) {
  const access = await ensureMidiAccess();
  const inputs = midiInputs(access);
  const selected = selectedName
    ? inputs.find((input) => midiInputName(input) === selectedName)
    : null;
  if (selectedName && !selected) {
    throw `Browser MIDI input not found: ${selectedName}`;
  }
  const input = selected || inputs[0];
  if (!input) {
    throw "No browser MIDI inputs found";
  }
  if (orbifoldMidiInput) {
    orbifoldMidiInput.onmidimessage = null;
    orbifoldMidiInput = null;
  }
  orbifoldMidiMessages = [];
  input.onmidimessage = (event) => {
    orbifoldMidiMessages.push(Array.from(event.data || []));
  };
  orbifoldMidiInput = input;
  return midiInputName(input);
}

export function drain_midi_messages_js() {
  const messages = orbifoldMidiMessages;
  orbifoldMidiMessages = [];
  return messages;
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = mark_runtime_ready_js)]
    fn mark_runtime_ready_js(frame_count: f64, viewport_width: f64, viewport_height: f64);

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = publish_action_result_js)]
    fn publish_action_result_js(action: &str, note_count: f64, status: &str);

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = publish_runtime_state_js)]
    fn publish_runtime_state_js(
        status: &str,
        note_count: f64,
        asset_count: f64,
        midi_input_count: f64,
        connected_midi_input: &str,
        last_midi_status: f64,
        last_midi_note: f64,
        audio_output_count: f64,
        connected_audio_output: &str,
        audio_stream_connected: bool,
        transport_playing: bool,
        transport_position_beats: f64,
        loop_beats: f64,
        ui_scale: f64,
        show_asset_browser: bool,
        show_scale_browser: bool,
        show_clip_panel: bool,
        scale_description: &str,
        scala_path: &str,
        lumatone_path: &str,
        lumatone_loaded: bool,
    );

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = publish_automation_geometry_js)]
    fn publish_automation_geometry_js(
        grid_x: f64,
        grid_y: f64,
        grid_width: f64,
        grid_height: f64,
        add_x: f64,
        add_y: f64,
        drag_start_x: f64,
        drag_start_y: f64,
        drag_end_x: f64,
        drag_end_y: f64,
        resize_start_x: f64,
        resize_start_y: f64,
        resize_end_x: f64,
        resize_end_y: f64,
        piano_view_start: f64,
        piano_view_beats: f64,
        piano_min_pitch: f64,
        piano_max_pitch: f64,
    );

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = publish_text_audit_js)]
    fn publish_text_audit_js(
        text_count: f64,
        issue_count: f64,
        non_finite_count: f64,
        sample_issue: &str,
    );

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = publish_pointer_action_js)]
    fn publish_pointer_action_js(action: &str, phase: &str, x: f64, y: f64);

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = publish_layout_automation_js)]
    fn publish_layout_automation_js(
        right_resize_x: f64,
        right_resize_y: f64,
        right_resize_end_x: f64,
        right_resize_end_y: f64,
        bottom_resize_x: f64,
        bottom_resize_y: f64,
        bottom_resize_end_x: f64,
        bottom_resize_end_y: f64,
        right_panel_width: f64,
        piano_roll_height: f64,
        arrangement_seek_start_x: f64,
        arrangement_seek_start_y: f64,
        arrangement_seek_end_x: f64,
        arrangement_seek_end_y: f64,
        piano_seek_start_x: f64,
        piano_seek_start_y: f64,
        piano_seek_end_x: f64,
        piano_seek_end_y: f64,
        arrangement_loop_end_start_x: f64,
        arrangement_loop_end_start_y: f64,
        arrangement_loop_end_target_x: f64,
        arrangement_loop_end_target_y: f64,
        piano_loop_end_start_x: f64,
        piano_loop_end_start_y: f64,
        piano_loop_end_target_x: f64,
        piano_loop_end_target_y: f64,
    );

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = load_browser_settings_text_js)]
    fn load_browser_settings_text_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = save_browser_settings_text_js)]
    fn save_browser_settings_text_js(text: &str) -> Result<(), JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = load_browser_project_storage_text_js)]
    fn load_browser_project_storage_text_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = save_browser_project_storage_text_js)]
    fn save_browser_project_storage_text_js(text: &str) -> Result<(), JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = save_browser_text_resource_js)]
    fn save_browser_text_resource_js(
        kind: &str,
        file_name: &str,
        text: &str,
    ) -> Result<(), JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_browser_text_resource_js)]
    fn load_browser_text_resource_js(kind: &str, file_name: &str) -> JsValue;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = load_browser_asset_storage_records_js)]
    async fn load_browser_asset_storage_records_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = save_browser_asset_storage_record_js)]
    async fn save_browser_asset_storage_record_js(
        path: &str,
        kind: usize,
        file_name: &str,
        bytes: &js_sys::Uint8Array,
    ) -> Result<(), JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = install_browser_keyboard_shortcuts_js)]
    fn install_browser_keyboard_shortcuts_js() -> Result<(), JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = drain_keyboard_actions_js)]
    fn drain_keyboard_actions_js() -> JsValue;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = drain_text_edit_actions_js)]
    fn drain_text_edit_actions_js() -> JsValue;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = install_browser_wheel_bridge_js)]
    fn install_browser_wheel_bridge_js(canvas_id: &str) -> Result<(), JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = drain_wheel_events_js)]
    fn drain_wheel_events_js() -> JsValue;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = install_browser_workspace_pointer_bridge_js)]
    fn install_browser_workspace_pointer_bridge_js(canvas_id: &str) -> Result<(), JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = drain_workspace_pointer_events_js)]
    fn drain_workspace_pointer_events_js() -> JsValue;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = reload_browser_window_js)]
    fn reload_browser_window_js();

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = open_text_file_js)]
    async fn open_text_file_js(accept: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = open_binary_file_js)]
    async fn open_binary_file_js(accept: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = download_text_file_js)]
    fn download_text_file_js(file_name: &str, text: &str) -> Result<(), JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = request_midi_inputs_js)]
    async fn request_midi_inputs_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = connect_midi_input_js)]
    async fn connect_midi_input_js(selected_name: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = drain_midi_messages_js)]
    fn drain_midi_messages_js() -> JsValue;
}
