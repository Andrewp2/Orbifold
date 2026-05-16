use operad::{
    CanvasContent, InputBehavior, ScenePrimitive, TextHorizontalAlign, UiDocument, UiNodeId,
    UiPoint, UiRect, UiVisual, layout, widgets,
};

use crate::app::AppState;
use crate::project::{ClipNote, QuantizeGrid};
use crate::ui::theme::{accent, color, fade, muted, stroke, strong};

use super::controls::{add_hit_at, add_pointer_edit_hit_at, add_pointer_edit_hit_at_to};
use super::{
    MIN_PITCH_GRID_SPACING, MIN_QUANTIZE_GRID_SPACING, body_rects, draw_panel, push_line,
    push_rect, push_text,
};

const MIN_VISIBLE_NOTE_WIDTH: f32 = 24.0;
pub(super) const PIANO_INPUT_CANVAS_KEY: &str = "orbifold.piano.input";

#[derive(Clone, Copy, Debug)]
pub(in crate::ui) struct SurfaceRects {
    pub(super) left: UiRect,
    pub(super) right: UiRect,
    pub(super) arrangement: UiRect,
    pub(super) arrangement_ruler: UiRect,
    pub(super) piano_roll: UiRect,
    pub(super) piano_options: UiRect,
    pub(super) piano_keyboard: UiRect,
    pub(super) piano_ruler: UiRect,
    pub(super) piano_grid: UiRect,
    pub(super) velocity_lane: UiRect,
    pub(super) keyboard_width: f32,
    pub(super) min_pitch: i32,
    pub(super) max_pitch: i32,
    pub(super) loop_beats: f32,
    pub(super) view_start_beats: f32,
    pub(super) view_beats: f32,
}

impl SurfaceRects {
    pub(in crate::ui) fn row_height(self) -> f32 {
        let pitch_count = (self.max_pitch - self.min_pitch + 1).max(1) as f32;
        self.piano_grid.height / pitch_count
    }

    pub(in crate::ui) fn beat_at(self, point: UiPoint) -> f32 {
        ((point.x - self.piano_grid.x) / self.piano_grid.width).clamp(0.0, 1.0)
            * self.view_beats.max(1.0)
            + self.view_start_beats
    }

    pub(in crate::ui) fn arrangement_beat_at(self, point: UiPoint) -> f32 {
        ((point.x - self.arrangement_ruler.x) / self.arrangement_ruler.width).clamp(0.0, 1.0)
            * self.loop_beats.max(1.0)
    }

    pub(in crate::ui) fn piano_ruler_beat_at(self, point: UiPoint) -> f32 {
        ((point.x - self.piano_ruler.x) / self.piano_ruler.width).clamp(0.0, 1.0)
            * self.view_beats.max(1.0)
            + self.view_start_beats
    }

    pub(in crate::ui) fn pitch_at(self, point: UiPoint) -> i32 {
        let row = ((point.y - self.piano_grid.y) / self.row_height()).floor() as i32;
        (self.max_pitch - row).clamp(self.min_pitch, self.max_pitch)
    }

    pub(in crate::ui) fn velocity_at(self, point: UiPoint) -> u8 {
        let normalized = (1.0
            - (point.y - self.velocity_lane.y) / self.velocity_lane.height.max(1.0))
        .clamp(0.0, 1.0);
        ((normalized * 126.0) + 1.0).round().clamp(1.0, 127.0) as u8
    }
}

pub(super) fn surface_rects(app: &AppState, width: f32, height: f32) -> SurfaceRects {
    let top_h = 62.0;
    let bottom_h = 26.0;
    let body = body_rects(width, height, top_h, bottom_h);
    let left = body.left;
    let center = body.center;
    let right = body.right;
    let arrangement_h = center.height;
    let arrangement = UiRect::new(center.x, center.y, center.width, arrangement_h);
    let arrangement_ruler = UiRect::new(
        arrangement.x + 12.0,
        arrangement.y + 44.0,
        arrangement.width - 24.0,
        30.0,
    );
    let gap = 8.0;
    let workspace_bottom = height - bottom_h - gap;
    let piano_roll_y = center.bottom() + gap;
    let piano_roll = UiRect::new(
        body.left.x,
        piano_roll_y,
        (width - gap - body.left.x).max(1.0),
        (workspace_bottom - piano_roll_y).max(1.0),
    );
    let inner_x = piano_roll.x + 10.0;
    let inner_right = piano_roll.right() - 10.0;
    let inner_width = (piano_roll.width - 20.0).max(1.0);
    let options_width = body
        .track
        .width
        .clamp(150.0, 210.0)
        .min((inner_width * 0.25).max(120.0));
    let piano_options = UiRect::new(
        inner_x,
        piano_roll.y + 10.0,
        options_width,
        (piano_roll.height - 20.0).max(1.0),
    );
    let keyboard_x = piano_options.right() + 8.0;
    let remaining_after_options = (inner_right - keyboard_x).max(1.0);
    let keyboard_width = 72.0_f32.min((remaining_after_options * 0.18).max(52.0));
    let grid_x = keyboard_x + keyboard_width;
    let grid_width = (inner_right - grid_x).max(1.0);
    let shell_h = (piano_roll.height - 64.0).max(1.0);
    let piano_ruler_h = 20.0_f32.min((shell_h * 0.16).max(14.0));
    let velocity_h = (shell_h * 0.22)
        .clamp(44.0, 58.0)
        .min((shell_h - 90.0).max(24.0));
    let velocity_gap = 6.0;
    let grid_h = (shell_h - piano_ruler_h - velocity_h - velocity_gap).max(1.0);
    let piano_keyboard = UiRect::new(keyboard_x, piano_roll.y + 54.0, keyboard_width, shell_h);
    let piano_ruler = UiRect::new(grid_x, piano_roll.y + 54.0, grid_width, piano_ruler_h);
    let piano_grid = UiRect::new(
        piano_ruler.x,
        piano_ruler.bottom(),
        piano_ruler.width,
        grid_h,
    );
    let velocity_lane = UiRect::new(
        piano_grid.x,
        piano_grid.bottom() + velocity_gap,
        piano_grid.width,
        velocity_h,
    );
    let project = app.music_project.lock();
    let loop_beats = project.transport.loop_beats;
    drop(project);
    let view_start_beats = app.piano_view_start_beats(loop_beats);
    let view_beats = app.piano_view_visible_beats(loop_beats);
    let (min_pitch, max_pitch) = app.piano_pitch_range();
    SurfaceRects {
        left,
        right,
        arrangement,
        arrangement_ruler,
        piano_roll,
        piano_options,
        piano_keyboard,
        piano_ruler,
        piano_grid,
        velocity_lane,
        keyboard_width,
        min_pitch,
        max_pitch,
        loop_beats,
        view_start_beats,
        view_beats,
    }
}

#[derive(Debug, Clone)]
struct ArrangementSurfaceModel {
    notes: Vec<ClipNote>,
    loop_beats: f32,
    quantize_grid: QuantizeGrid,
    current_beat: f32,
}

impl ArrangementSurfaceModel {
    fn from_app(app: &AppState) -> Self {
        let project = app.music_project.lock();
        let model = Self {
            notes: project.clip.notes.clone(),
            loop_beats: project.transport.loop_beats.max(1.0),
            quantize_grid: project.transport.quantize_grid,
            current_beat: project.current_position_beats(std::time::Instant::now()),
        };
        drop(project);
        model
    }
}

#[derive(Debug, Clone)]
struct PianoRollSurfaceModel {
    notes: Vec<ClipNote>,
    quantize_grid: QuantizeGrid,
    current_beat: f32,
    selected_note: Option<u64>,
}

impl PianoRollSurfaceModel {
    fn from_app(app: &AppState, _rects: SurfaceRects) -> Self {
        let project = app.music_project.lock();
        let notes = project.clip.notes.clone();
        let quantize_grid = project.transport.quantize_grid;
        let current_beat = project.current_position_beats(std::time::Instant::now());
        drop(project);
        let selected_note = app.selected_clip_note;
        Self {
            notes,
            quantize_grid,
            current_beat,
            selected_note,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CenterEditorSurfaceNodes {
    pub(super) arrangement: UiNodeId,
    pub(super) piano_roll: UiNodeId,
}

pub(super) fn add_center_editor_surfaces(
    document: &mut UiDocument,
    parent: UiNodeId,
    app: &AppState,
    rects: SurfaceRects,
) -> CenterEditorSurfaceNodes {
    let arrangement = add_arrangement_surface(document, parent, app, rects);
    let piano_roll = add_piano_roll_surface(document, parent, app, rects);
    let piano_input = add_piano_input_canvas(document, parent, rects.piano_roll);
    add_center_editor_hit_targets(document, app, rects, piano_input);
    CenterEditorSurfaceNodes {
        arrangement,
        piano_roll,
    }
}

fn add_arrangement_surface(
    document: &mut UiDocument,
    parent: UiNodeId,
    app: &AppState,
    rects: SurfaceRects,
) -> UiNodeId {
    let model = ArrangementSurfaceModel::from_app(app);
    let mut primitives = Vec::new();
    draw_arrangement(&mut primitives, rects.arrangement, &model);
    widgets::scene(
        document,
        parent,
        "orbifold.native.arrangement",
        local_scene_primitives(primitives, rects.arrangement),
        widgets::SceneOptions::default()
            .with_layout(layout::absolute(
                rects.arrangement.x,
                rects.arrangement.y,
                rects.arrangement.width,
                rects.arrangement.height,
            ))
            .accessibility_label("Arrangement timeline"),
    )
}

fn add_piano_roll_surface(
    document: &mut UiDocument,
    parent: UiNodeId,
    app: &AppState,
    rects: SurfaceRects,
) -> UiNodeId {
    let model = PianoRollSurfaceModel::from_app(app, rects);
    let mut primitives = Vec::new();
    draw_piano_roll(&mut primitives, rects.piano_roll, rects, &model);
    widgets::scene(
        document,
        parent,
        "orbifold.native.piano_roll",
        local_scene_primitives(primitives, rects.piano_roll),
        widgets::SceneOptions::default()
            .with_layout(layout::absolute(
                rects.piano_roll.x,
                rects.piano_roll.y,
                rects.piano_roll.width,
                rects.piano_roll.height,
            ))
            .accessibility_label("Piano roll editor"),
    )
}

fn add_piano_input_canvas(document: &mut UiDocument, parent: UiNodeId, rect: UiRect) -> UiNodeId {
    let options = widgets::CanvasOptions {
        layout: layout::absolute(rect.x, rect.y, rect.width, rect.height),
        visual: UiVisual::TRANSPARENT,
        input: InputBehavior {
            pointer: true,
            focusable: false,
            keyboard: false,
        },
        accessibility_label: Some("Piano roll input surface".to_string()),
        ..Default::default()
    };
    let id = widgets::canvas(
        document,
        parent,
        "orbifold.native.piano_input",
        CanvasContent::new(PIANO_INPUT_CANVAS_KEY)
            .pointer_capture(true)
            .wheel_capture(true),
        options,
    );
    let node = document.node_mut(id);
    node.input.focusable = false;
    node.input.keyboard = false;
    if let Some(accessibility) = node.accessibility.as_mut() {
        accessibility.focusable = false;
        accessibility.hidden = true;
    }
    id
}

fn add_center_editor_hit_targets(
    document: &mut UiDocument,
    app: &AppState,
    rects: SurfaceRects,
    piano_input: UiNodeId,
) {
    let notes = app.music_project.lock().clip.notes.clone();
    add_pointer_edit_hit_at(document, "transport.seek", rects.arrangement_ruler);
    if !notes.is_empty() {
        add_hit_at(
            document,
            "clip.select_current",
            arrangement_clip_rect(rects),
        );
    }
    add_pointer_edit_hit_at_to(
        document,
        piano_input,
        "piano.seek",
        local_rect(rects.piano_roll, rects.piano_ruler),
    );
    add_pointer_edit_hit_at_to(
        document,
        piano_input,
        "piano.grid",
        local_rect(rects.piano_roll, rects.piano_grid),
    );
    for note in &notes {
        for note_rect in piano_note_rects(note.clone(), rects) {
            add_pointer_edit_hit_at_to(
                document,
                piano_input,
                format!("note.select.{}", note.id),
                local_rect(rects.piano_roll, note_rect),
            );
            if let Some(edge_w) = note_resize_edge_width(note_rect.width) {
                add_pointer_edit_hit_at_to(
                    document,
                    piano_input,
                    format!("note.resize_start.{}", note.id),
                    local_rect(
                        rects.piano_roll,
                        UiRect::new(note_rect.x, note_rect.y, edge_w, note_rect.height),
                    ),
                );
                add_pointer_edit_hit_at_to(
                    document,
                    piano_input,
                    format!("note.resize_end.{}", note.id),
                    local_rect(
                        rects.piano_roll,
                        UiRect::new(
                            note_rect.right() - edge_w,
                            note_rect.y,
                            edge_w,
                            note_rect.height,
                        ),
                    ),
                );
            }
        }
        for velocity_rect in piano_velocity_hit_rects(note.clone(), rects) {
            add_pointer_edit_hit_at_to(
                document,
                piano_input,
                format!("note.velocity.{}", note.id),
                local_rect(rects.piano_roll, velocity_rect),
            );
        }
    }
}

fn local_rect(parent: UiRect, rect: UiRect) -> UiRect {
    UiRect::new(
        rect.x - parent.x,
        rect.y - parent.y,
        rect.width,
        rect.height,
    )
}

pub(super) fn piano_note_rects(note: ClipNote, rects: SurfaceRects) -> Vec<UiRect> {
    if note.musical_note < rects.min_pitch || note.musical_note > rects.max_pitch {
        return Vec::new();
    }
    let row_height = rects.row_height();
    let y = rects.piano_grid.y + (rects.max_pitch - note.musical_note) as f32 * row_height + 2.0;
    let height = (row_height - 4.0).max(4.0);
    visible_note_beat_segments(&note, rects)
        .into_iter()
        .map(|(start, end)| {
            let x = rects.piano_grid.x
                + rects.piano_grid.width * (start - rects.view_start_beats) / rects.view_beats;
            let width = rects.piano_grid.width * (end - start).max(0.0) / rects.view_beats;
            UiRect::new(x, y, width.max(MIN_VISIBLE_NOTE_WIDTH), height)
        })
        .collect()
}

fn visible_note_beat_segments(note: &ClipNote, rects: SurfaceRects) -> Vec<(f32, f32)> {
    let loop_beats = rects.loop_beats.max(1.0);
    let view_start = rects.view_start_beats.clamp(0.0, loop_beats);
    let view_end = (view_start + rects.view_beats.max(1.0)).min(loop_beats);
    if view_end <= view_start {
        return Vec::new();
    }
    if note.duration_beats >= loop_beats {
        return vec![(view_start, view_end)];
    }

    let start = note.start_beats.rem_euclid(loop_beats);
    let duration = note.duration_beats.clamp(0.0, loop_beats);
    let end = start + duration;
    let segments = if end <= loop_beats {
        vec![(start, end)]
    } else {
        vec![(start, loop_beats), (0.0, end - loop_beats)]
    };

    let mut visible = Vec::new();
    for (segment_start, segment_end) in segments {
        let start = segment_start.max(view_start);
        let end = segment_end.min(view_end);
        if end > start {
            visible.push((start, end));
        }
    }
    visible
}

pub(super) fn piano_velocity_hit_rects(note: ClipNote, rects: SurfaceRects) -> Vec<UiRect> {
    visible_note_beat_segments(&note, rects)
        .into_iter()
        .map(|(start, end)| {
            let x = rects.piano_grid.x
                + rects.piano_grid.width * (start - rects.view_start_beats) / rects.view_beats;
            let width = rects.piano_grid.width * (end - start).max(0.0) / rects.view_beats;
            UiRect::new(
                x,
                rects.velocity_lane.y,
                width.max(MIN_VISIBLE_NOTE_WIDTH),
                rects.velocity_lane.height,
            )
        })
        .collect()
}

pub(super) fn arrangement_clip_rect(rects: SurfaceRects) -> UiRect {
    let grid = UiRect::new(
        rects.arrangement.x + 12.0,
        rects.arrangement_ruler.bottom(),
        rects.arrangement.width - 24.0,
        rects.arrangement.height - 120.0,
    );
    let clip_h = (grid.height - 16.0).clamp(28.0, 58.0);
    UiRect::new(grid.x, grid.y + 8.0, grid.width, clip_h)
}

pub(super) fn note_resize_edge_width(note_width: f32) -> Option<f32> {
    (note_width >= 18.0).then(|| 8.0_f32.min(note_width * 0.25))
}

fn draw_arrangement(
    primitives: &mut Vec<ScenePrimitive>,
    rect: UiRect,
    model: &ArrangementSurfaceModel,
) {
    draw_panel(primitives, rect, "");
    push_text(
        primitives,
        UiRect::new(rect.x + 14.0, rect.y + 12.0, 160.0, 24.0),
        "Current Clip",
        16.0,
        strong(),
        TextHorizontalAlign::Start,
    );
    let ruler = UiRect::new(rect.x + 12.0, rect.y + 44.0, rect.width - 24.0, 30.0);
    let grid = UiRect::new(ruler.x, ruler.bottom(), ruler.width, rect.height - 120.0);
    push_rect(primitives, ruler, color(10, 16, 23), 0.0, None);
    push_rect(primitives, grid, color(9, 15, 22), 0.0, None);

    draw_quantize_grid_lines(primitives, grid, model.loop_beats, model.quantize_grid);
    let beat_lines = model.loop_beats.ceil() as i32;
    for beat in 0..=beat_lines {
        let x = grid.x + grid.width * beat as f32 / model.loop_beats;
        let line_color = if beat % 4 == 0 {
            color(62, 76, 96)
        } else {
            color(34, 44, 57)
        };
        push_line(
            primitives,
            UiPoint::new(x, ruler.y),
            UiPoint::new(x, grid.bottom()),
            line_color,
            if beat % 4 == 0 { 1.4 } else { 1.0 },
        );
        if beat % 4 == 0 && x + 42.0 <= grid.right() {
            push_text(
                primitives,
                UiRect::new(x + 6.0, ruler.y + 7.0, 42.0, 14.0),
                format!("{}", beat + 1),
                11.0,
                muted(),
                TextHorizontalAlign::Start,
            );
        }
    }
    let row_h = grid.height;
    for row in 0..=1 {
        let y = grid.y + row_h * row as f32;
        push_line(
            primitives,
            UiPoint::new(grid.x, y),
            UiPoint::new(grid.right(), y),
            color(29, 40, 52),
            1.0,
        );
    }
    let track_color = color(132, 81, 238);
    let row_y = grid.y;
    if model.notes.is_empty() {
        push_text(
            primitives,
            UiRect::new(grid.x + 14.0, row_y + 14.0, grid.width - 28.0, 18.0),
            "No recorded clip",
            12.0,
            muted(),
            TextHorizontalAlign::Start,
        );
    } else {
        let clip_h = (row_h - 16.0).clamp(28.0, 58.0);
        let clip = UiRect::new(grid.x, row_y + 8.0, grid.width, clip_h);
        push_rect(
            primitives,
            clip,
            fade(track_color, 0.72),
            4.0,
            Some(stroke(track_color, 1.0)),
        );
        for note in &model.notes {
            let x = clip.x + clip.width * note.start_beats / model.loop_beats;
            let y = clip.y + clip.height * (1.0 - note.velocity as f32 / 127.0);
            push_rect(
                primitives,
                UiRect::new(x, y, 3.0, 3.0),
                fade(strong(), 0.65),
                1.0,
                None,
            );
        }
    }
    let lane = UiRect::new(
        grid.x,
        grid.bottom(),
        grid.width,
        rect.bottom() - grid.bottom() - 10.0,
    );
    push_rect(primitives, lane, color(9, 15, 22), 0.0, None);
    let playhead_x = grid.x + grid.width * model.current_beat / model.loop_beats;
    push_line(
        primitives,
        UiPoint::new(playhead_x, ruler.y),
        UiPoint::new(playhead_x, lane.bottom()),
        accent(),
        2.0,
    );
}

fn draw_piano_roll(
    primitives: &mut Vec<ScenePrimitive>,
    rect: UiRect,
    rects: SurfaceRects,
    model: &PianoRollSurfaceModel,
) {
    draw_panel(primitives, rect, "");
    push_rect(
        primitives,
        rects.piano_options,
        color(21, 22, 24),
        3.0,
        Some(stroke(color(42, 45, 48), 1.0)),
    );
    push_line(
        primitives,
        UiPoint::new(rects.piano_options.x + 8.0, rects.piano_options.y + 74.0),
        UiPoint::new(
            rects.piano_options.right() - 8.0,
            rects.piano_options.y + 74.0,
        ),
        color(48, 51, 55),
        1.0,
    );
    push_line(
        primitives,
        UiPoint::new(rects.piano_options.x + 8.0, rects.piano_options.y + 132.0),
        UiPoint::new(
            rects.piano_options.right() - 8.0,
            rects.piano_options.y + 132.0,
        ),
        color(48, 51, 55),
        1.0,
    );
    let shell = UiRect::new(
        rects.piano_keyboard.x,
        rect.y + 54.0,
        rects.piano_keyboard.width + rects.piano_grid.width,
        rect.height - 64.0,
    );
    push_rect(
        primitives,
        shell,
        color(16, 17, 19),
        0.0,
        Some(stroke(color(48, 52, 58), 1.0)),
    );
    let ruler = rects.piano_ruler;
    let grid = rects.piano_grid;
    push_rect(primitives, ruler, color(31, 33, 36), 0.0, None);
    push_rect(
        primitives,
        rects.piano_keyboard,
        color(26, 27, 30),
        0.0,
        None,
    );
    let pitch_count = (rects.max_pitch - rects.min_pitch + 1).max(1);
    let row_height = rects.row_height();
    let line_step = piano_pitch_grid_line_step(row_height);
    for row in 0..pitch_count {
        let y = grid.y + row_height * row as f32;
        let pitch = rects.max_pitch - row;
        let black_key = matches!(pitch.rem_euclid(12), 1 | 3 | 6 | 8 | 10);
        let lane_color = if black_key {
            color(13, 14, 16)
        } else if row % 2 == 0 {
            color(18, 19, 21)
        } else {
            color(20, 21, 23)
        };
        push_rect(
            primitives,
            UiRect::new(grid.x, y, grid.width, row_height.max(1.0)),
            lane_color,
            0.0,
            None,
        );
        push_rect(
            primitives,
            UiRect::new(
                rects.piano_keyboard.x,
                y,
                rects.piano_keyboard.width,
                row_height.max(1.0),
            ),
            if black_key {
                color(18, 19, 21)
            } else {
                color(35, 36, 39)
            },
            0.0,
            None,
        );
    }
    for row in 0..=pitch_count {
        let y = grid.y + row_height * row as f32;
        if row == 0 || row == pitch_count || row % line_step == 0 {
            push_line(
                primitives,
                UiPoint::new(shell.x, y),
                UiPoint::new(grid.right(), y),
                color(37, 40, 45),
                1.0,
            );
        }
    }
    let loop_beats = rects.loop_beats.max(1.0);
    let view_start = rects.view_start_beats;
    let view_end = (view_start + rects.view_beats).min(loop_beats);
    for beat in view_start.floor() as i32..=view_end.ceil() as i32 {
        if beat < 0 || beat as f32 > loop_beats {
            continue;
        }
        let x = grid.x + grid.width * (beat as f32 - view_start) / rects.view_beats;
        if x < grid.x - 0.5 || x > grid.right() + 0.5 {
            continue;
        }
        push_line(
            primitives,
            UiPoint::new(x, ruler.y),
            UiPoint::new(x, grid.bottom()),
            if beat % 4 == 0 {
                color(78, 82, 90)
            } else {
                color(42, 45, 50)
            },
            1.0,
        );
        if beat % 4 == 0 && x + 34.0 <= grid.right() {
            push_text(
                primitives,
                UiRect::new(x + 4.0, ruler.y + 2.0, 34.0, 14.0),
                format!("{}", beat + 1),
                10.0,
                color(185, 188, 194),
                TextHorizontalAlign::Start,
            );
        }
    }
    draw_velocity_lane(
        primitives,
        rects,
        &model.notes,
        model.selected_note,
        model.quantize_grid,
    );
    draw_quantize_grid_lines_in_view(
        primitives,
        grid,
        view_start,
        rects.view_beats,
        loop_beats,
        model.quantize_grid,
    );
    for note in &model.notes {
        let note_color = piano_note_color(note, model.selected_note, false);
        for note_rect in piano_note_rects(note.clone(), rects) {
            let selected = Some(note.id) == model.selected_note;
            push_rect(
                primitives,
                note_rect,
                note_color,
                2.0,
                Some(stroke(
                    if selected {
                        color(255, 182, 92)
                    } else {
                        fade(color(230, 236, 244), 0.32)
                    },
                    if selected { 2.0 } else { 1.0 },
                )),
            );
            if selected && note_resize_edge_width(note_rect.width).is_some() {
                let handle_color = fade(strong(), 0.72);
                push_line(
                    primitives,
                    UiPoint::new(note_rect.x + 4.0, note_rect.y + 3.0),
                    UiPoint::new(note_rect.x + 4.0, note_rect.bottom() - 3.0),
                    handle_color,
                    1.5,
                );
                push_line(
                    primitives,
                    UiPoint::new(note_rect.right() - 4.0, note_rect.y + 3.0),
                    UiPoint::new(note_rect.right() - 4.0, note_rect.bottom() - 3.0),
                    handle_color,
                    1.5,
                );
            }
        }
    }
    if model.current_beat >= view_start && model.current_beat <= view_end {
        let playhead_x = grid.x + grid.width * (model.current_beat - view_start) / rects.view_beats;
        push_line(
            primitives,
            UiPoint::new(playhead_x, ruler.y),
            UiPoint::new(playhead_x, rects.velocity_lane.bottom()),
            accent(),
            2.0,
        );
    }
}

fn draw_quantize_grid_lines(
    primitives: &mut Vec<ScenePrimitive>,
    rect: UiRect,
    loop_beats: f32,
    quantize_grid: QuantizeGrid,
) {
    let Some(step) = visible_quantize_grid_step(loop_beats, rect.width, quantize_grid) else {
        return;
    };
    let loop_beats = loop_beats.max(1.0);
    let subdivision_count = (loop_beats / step).ceil() as i32;
    if subdivision_count > 512 {
        return;
    }
    for index in 1..subdivision_count {
        let beat = index as f32 * step;
        if beat >= loop_beats {
            break;
        }
        if (beat - beat.round()).abs() < 0.001 {
            continue;
        }
        let x = rect.x + rect.width * beat / loop_beats;
        push_line(
            primitives,
            UiPoint::new(x, rect.y),
            UiPoint::new(x, rect.bottom()),
            color(32, 34, 38),
            1.0,
        );
    }
}

fn draw_quantize_grid_lines_in_view(
    primitives: &mut Vec<ScenePrimitive>,
    rect: UiRect,
    view_start: f32,
    view_beats: f32,
    loop_beats: f32,
    quantize_grid: QuantizeGrid,
) {
    let Some(step) = visible_quantize_grid_step(view_beats, rect.width, quantize_grid) else {
        return;
    };
    let loop_beats = loop_beats.max(1.0);
    let view_beats = view_beats.max(1.0);
    let view_end = (view_start + view_beats).min(loop_beats);
    let first = (view_start / step).floor() as i32;
    let last = (view_end / step).ceil() as i32;
    if last - first > 512 {
        return;
    }
    for index in first..=last {
        let beat = index as f32 * step;
        if beat <= view_start || beat >= view_end {
            continue;
        }
        if (beat - beat.round()).abs() < 0.001 {
            continue;
        }
        let x = rect.x + rect.width * (beat - view_start) / view_beats;
        push_line(
            primitives,
            UiPoint::new(x, rect.y),
            UiPoint::new(x, rect.bottom()),
            color(17, 27, 38),
            1.0,
        );
    }
}

pub(super) fn visible_quantize_grid_step(
    loop_beats: f32,
    rect_width: f32,
    quantize_grid: QuantizeGrid,
) -> Option<f32> {
    let mut step = quantize_grid.step_beats()?;
    if step >= 1.0 {
        return None;
    }

    let loop_beats = loop_beats.max(1.0);
    let rect_width = rect_width.max(1.0);
    while rect_width * step / loop_beats < MIN_QUANTIZE_GRID_SPACING {
        step *= 2.0;
        if step >= 1.0 {
            return None;
        }
    }
    Some(step)
}

fn draw_velocity_lane(
    primitives: &mut Vec<ScenePrimitive>,
    rects: SurfaceRects,
    notes: &[ClipNote],
    selected_note: Option<u64>,
    quantize_grid: QuantizeGrid,
) {
    let lane = rects.velocity_lane;
    push_rect(primitives, lane, color(17, 18, 20), 0.0, None);
    push_rect(
        primitives,
        UiRect::new(
            rects.piano_keyboard.x,
            lane.y,
            rects.keyboard_width,
            lane.height,
        ),
        color(28, 29, 32),
        0.0,
        None,
    );
    push_text(
        primitives,
        UiRect::new(
            rects.piano_keyboard.x + 10.0,
            lane.y + 4.0,
            rects.keyboard_width - 14.0,
            16.0,
        ),
        "Velocity",
        10.0,
        color(176, 180, 186),
        TextHorizontalAlign::Start,
    );
    for fraction in [0.0_f32, 0.5, 1.0] {
        let y = lane.bottom() - lane.height * fraction;
        push_line(
            primitives,
            UiPoint::new(rects.piano_keyboard.x, y),
            UiPoint::new(lane.right(), y),
            if (fraction - 0.5).abs() < f32::EPSILON {
                color(48, 51, 57)
            } else {
                color(34, 36, 40)
            },
            1.0,
        );
    }
    let loop_beats = rects.loop_beats.max(1.0);
    let view_start = rects.view_start_beats;
    let view_end = (view_start + rects.view_beats).min(loop_beats);
    for beat in view_start.floor() as i32..=view_end.ceil() as i32 {
        if beat < 0 || beat as f32 > loop_beats {
            continue;
        }
        let x = lane.x + lane.width * (beat as f32 - view_start) / rects.view_beats;
        if x < lane.x - 0.5 || x > lane.right() + 0.5 {
            continue;
        }
        push_line(
            primitives,
            UiPoint::new(x, lane.y),
            UiPoint::new(x, lane.bottom()),
            if beat % 4 == 0 {
                color(74, 78, 86)
            } else {
                color(39, 42, 47)
            },
            1.0,
        );
    }
    draw_quantize_grid_lines_in_view(
        primitives,
        lane,
        view_start,
        rects.view_beats,
        loop_beats,
        quantize_grid,
    );
    for note in notes {
        let fill_color = if Some(note.id) == selected_note {
            accent()
        } else {
            piano_note_color(note, selected_note, false)
        };
        for hit in piano_velocity_hit_rects(note.clone(), rects) {
            let bar_h = (lane.height * (note.velocity as f32 / 127.0))
                .max(2.0)
                .min(lane.height);
            push_rect(
                primitives,
                UiRect::new(
                    hit.x + 1.0,
                    lane.bottom() - bar_h,
                    (hit.width - 2.0).max(2.0),
                    bar_h,
                ),
                fill_color,
                2.0,
                Some(stroke(fade(strong(), 0.25), 1.0)),
            );
        }
    }
}

pub(super) fn piano_pitch_label_step(pitch_count: i32, row_height: f32) -> i32 {
    const TARGET_LABELS: i32 = 12;
    const MIN_LABEL_SPACING_PX: f32 = 22.0;
    (pitch_count / TARGET_LABELS)
        .max((MIN_LABEL_SPACING_PX / row_height.max(1.0)).ceil() as i32)
        .max(1)
}

pub(super) fn piano_pitch_grid_line_step(row_height: f32) -> i32 {
    (MIN_PITCH_GRID_SPACING / row_height.max(1.0))
        .ceil()
        .max(1.0) as i32
}

fn piano_note_color(note: &ClipNote, selected: Option<u64>, preview: bool) -> operad::ColorRgba {
    if Some(note.id) == selected {
        return color(255, 182, 92);
    }
    if preview {
        return fade(color(117, 170, 205), 0.72);
    }
    let velocity = note.velocity as f32 / 127.0;
    let alpha = 0.62 + velocity * 0.28;
    fade(color(97, 150, 185), alpha)
}

fn local_scene_primitives(primitives: Vec<ScenePrimitive>, rect: UiRect) -> Vec<ScenePrimitive> {
    let offset = UiPoint::new(-rect.x, -rect.y);
    primitives
        .into_iter()
        .map(|primitive| translate_scene_primitive(primitive, offset))
        .collect()
}

fn translate_scene_primitive(primitive: ScenePrimitive, offset: UiPoint) -> ScenePrimitive {
    match primitive {
        ScenePrimitive::Line { from, to, stroke } => ScenePrimitive::Line {
            from: translate_point(from, offset),
            to: translate_point(to, offset),
            stroke,
        },
        ScenePrimitive::Circle {
            center,
            radius,
            fill,
            stroke,
        } => ScenePrimitive::Circle {
            center: translate_point(center, offset),
            radius,
            fill,
            stroke,
        },
        ScenePrimitive::Polygon {
            points,
            fill,
            stroke,
        } => ScenePrimitive::Polygon {
            points: points
                .into_iter()
                .map(|point| translate_point(point, offset))
                .collect(),
            fill,
            stroke,
        },
        ScenePrimitive::Image { key, rect, tint } => ScenePrimitive::Image {
            key,
            rect: translate_rect(rect, offset),
            tint,
        },
        ScenePrimitive::Rect(rect) => ScenePrimitive::Rect(rect.translated(offset)),
        ScenePrimitive::Text(text) => ScenePrimitive::Text(text.translated(offset)),
        ScenePrimitive::Path(path) => ScenePrimitive::Path(path.translated(offset)),
        ScenePrimitive::ImagePlacement(image) => {
            ScenePrimitive::ImagePlacement(image.translated(offset))
        }
    }
}

fn translate_point(point: UiPoint, offset: UiPoint) -> UiPoint {
    UiPoint::new(point.x + offset.x, point.y + offset.y)
}

fn translate_rect(rect: UiRect, offset: UiPoint) -> UiRect {
    UiRect::new(
        rect.x + offset.x,
        rect.y + offset.y,
        rect.width,
        rect.height,
    )
}
