use operad::{CursorShape, UiPoint};

use crate::app::AppState;

use super::SurfaceRects;
use super::rect_contains_point;
use super::surfaces::{note_resize_edge_width, piano_note_rects, piano_velocity_hit_rects};

#[derive(Clone, Copy, Debug)]
pub(super) struct NoteDrag {
    pub(super) note_id: u64,
    pub(super) mode: NoteDragMode,
    pub(super) beat_offset: f32,
    pub(super) pitch_offset: i32,
    pub(super) pushed_history: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PianoKeyboardDrag {
    pub(super) start_position: UiPoint,
    pub(super) last_position: UiPoint,
    pub(super) pitch_remainder_px: f32,
    pub(super) moved: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PianoGridPress {
    pub(super) position: UiPoint,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PianoGridClick {
    pub(super) position: UiPoint,
    pub(super) timestamp_millis: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PianoViewportDrag {
    pub(super) mode: PianoViewportDragMode,
    pub(super) grab_offset_px: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum NoteDragMode {
    Move,
    ResizeStart,
    ResizeEnd,
    Velocity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TimelineDragMode {
    Arrangement,
    Piano,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LoopEndDragMode {
    Arrangement,
    Piano,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PianoViewportDragMode {
    Time,
    Pitch,
}

impl TimelineDragMode {
    pub(super) fn from_action(action: &str) -> Option<Self> {
        match action {
            "transport.seek" => Some(Self::Arrangement),
            "piano.seek" => Some(Self::Piano),
            _ => None,
        }
    }
}

impl LoopEndDragMode {
    pub(super) fn from_action(action: &str) -> Option<Self> {
        match action {
            "transport.loop_end" => Some(Self::Arrangement),
            "piano.loop_end" => Some(Self::Piano),
            _ => None,
        }
    }
}

impl PianoViewportDragMode {
    pub(super) fn from_action(action: &str) -> Option<Self> {
        match action {
            "piano.viewport.time" => Some(Self::Time),
            "piano.viewport.pitch" => Some(Self::Pitch),
            _ => None,
        }
    }
}

pub(super) fn point_distance(a: UiPoint, b: UiPoint) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

pub(super) fn piano_cursor_shape_at(
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

pub(super) fn piano_note_hit_at(
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

pub(super) fn note_drag_from_action(action: &str) -> Option<(u64, NoteDragMode)> {
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
