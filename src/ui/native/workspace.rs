use operad::{ScenePrimitive, UiPoint, UiRect};

use crate::app::{AppState, WorkspaceResizeTarget};
use crate::ui::theme::{accent, color, stroke};

use super::browser::left_browser_splitter_rect;
use super::{push_line, push_rect};

pub(super) const MIN_EDITOR_TOP_HEIGHT: f32 = 320.0;
pub(super) const MIN_BOTTOM_EDITOR_HEIGHT: f32 = 260.0;
pub(super) const MIN_LEFT_PANEL_WIDTH: f32 = 150.0;
pub(super) const MIN_TRACK_PANEL_WIDTH: f32 = 140.0;
pub(super) const MIN_RIGHT_PANEL_WIDTH: f32 = 220.0;
const WORKSPACE_SPLITTER_HIT_SIZE: f32 = 28.0;
const WORKSPACE_SPLITTER_GUTTER_THICKNESS: f32 = 14.0;
const WORKSPACE_SPLITTER_HANDLE_THICKNESS: f32 = 14.0;
const WORKSPACE_SPLITTER_DOT_SIZE: f32 = 4.0;
const WORKSPACE_VERTICAL_HANDLE_HEIGHT: f32 = 104.0;
const WORKSPACE_HORIZONTAL_HANDLE_WIDTH: f32 = 144.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct BodyRects {
    pub(super) left: UiRect,
    pub(super) track: UiRect,
    pub(super) center: UiRect,
    pub(super) right: UiRect,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WorkspaceResizeRects {
    pub(super) left: UiRect,
    pub(super) track: UiRect,
    pub(super) right: UiRect,
    pub(super) bottom: UiRect,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WorkspacePanelWidths {
    pub(super) left: f32,
    pub(super) track: f32,
    pub(super) right: f32,
}

pub(super) fn body_rects(
    app: &AppState,
    width: f32,
    height: f32,
    top_h: f32,
    bottom_h: f32,
) -> BodyRects {
    let gap = 8.0;
    let body_top = top_h + gap;
    let body_bottom = height - bottom_h - gap;
    let full_body_h = (body_bottom - body_top).max(1.0);
    let editor_h = bottom_editor_height(app, full_body_h, gap);
    let top_body_h = (full_body_h - editor_h - gap).max(1.0);
    let defaults = default_workspace_panel_widths(width);
    let desired = WorkspacePanelWidths {
        left: app.workspace_left_width().unwrap_or(defaults.left),
        track: if app.show_clip_panel {
            app.workspace_track_width().unwrap_or(defaults.track)
        } else {
            0.0
        },
        right: app.workspace_right_width().unwrap_or(defaults.right),
    };
    let panels = fit_workspace_panel_widths(width, desired, app.show_clip_panel);
    let gap_count = if app.show_clip_panel { 5.0 } else { 4.0 };
    let center_w = (width - panels.left - panels.track - panels.right - gap * gap_count).max(1.0);
    let left = UiRect::new(gap, body_top, panels.left, top_body_h);
    let track_x = left.right() + gap;
    let track = UiRect::new(track_x, body_top, panels.track, top_body_h);
    let center_x = if app.show_clip_panel {
        track.right() + gap
    } else {
        track_x
    };
    let center = UiRect::new(center_x, body_top, center_w, top_body_h);
    let right = UiRect::new(center.right() + gap, body_top, panels.right, full_body_h);
    BodyRects {
        left,
        track,
        center,
        right,
    }
}

pub(super) fn bottom_editor_height(app: &AppState, body_h: f32, gap: f32) -> f32 {
    let desired = (body_h * 0.44).max(MIN_BOTTOM_EDITOR_HEIGHT);
    let max_with_top = (body_h - MIN_EDITOR_TOP_HEIGHT - gap).max(MIN_BOTTOM_EDITOR_HEIGHT);
    app.workspace_bottom_height()
        .unwrap_or(desired)
        .clamp(MIN_BOTTOM_EDITOR_HEIGHT, max_with_top)
        .min((body_h - gap).max(1.0))
}

pub(super) fn workspace_resize_rects(body: BodyRects, piano_roll: UiRect) -> WorkspaceResizeRects {
    let bottom_center_y = piano_roll.y - 4.0;
    WorkspaceResizeRects {
        left: splitter_between(body.left, body.track),
        track: splitter_between(body.track, body.center),
        right: splitter_between(body.center, body.right),
        bottom: UiRect::new(
            piano_roll.x,
            bottom_center_y - WORKSPACE_SPLITTER_HIT_SIZE * 0.5,
            piano_roll.width,
            WORKSPACE_SPLITTER_HIT_SIZE,
        ),
    }
}

pub(super) fn workspace_splitter_chrome_rects(
    body: BodyRects,
    piano_roll: UiRect,
) -> WorkspaceResizeRects {
    let right_top = UiRect::new(
        body.right.x,
        body.center.y,
        body.right.width,
        body.center.height,
    );
    let bottom_center_y = piano_roll.y - 4.0;
    WorkspaceResizeRects {
        left: splitter_between(body.left, body.track),
        track: splitter_between(body.track, body.center),
        right: splitter_between(body.center, right_top),
        bottom: UiRect::new(
            piano_roll.x,
            bottom_center_y - WORKSPACE_SPLITTER_HIT_SIZE * 0.5,
            piano_roll.width,
            WORKSPACE_SPLITTER_HIT_SIZE,
        ),
    }
}

pub(super) fn draw_workspace_splitters(
    primitives: &mut Vec<ScenePrimitive>,
    app: &AppState,
    body: BodyRects,
    piano_roll: UiRect,
) {
    let splitters = workspace_splitter_chrome_rects(body, piano_roll);
    draw_vertical_workspace_splitter(primitives, splitters.left);
    if app.show_clip_panel {
        draw_vertical_workspace_splitter(primitives, splitters.track);
    }
    draw_vertical_workspace_splitter(primitives, splitters.right);
    draw_horizontal_workspace_splitter(primitives, splitters.bottom);
    if let Some(rect) = left_browser_splitter_rect(app, body.left) {
        draw_horizontal_workspace_splitter(primitives, rect);
    }
}

fn draw_vertical_workspace_splitter(primitives: &mut Vec<ScenePrimitive>, rect: UiRect) {
    let x = rect.x + rect.width * 0.5;
    let gutter_h = (rect.height - 12.0).max(0.0);
    if gutter_h > 0.0 {
        let gutter_w = WORKSPACE_SPLITTER_GUTTER_THICKNESS;
        push_rect(
            primitives,
            UiRect::new(x - gutter_w * 0.5, rect.y + 6.0, gutter_w, gutter_h),
            color(12, 20, 29),
            6.0,
            Some(stroke(color(31, 45, 61), 1.0)),
        );
    }
    push_line(
        primitives,
        UiPoint::new(x, rect.y + 8.0),
        UiPoint::new(x, rect.bottom() - 8.0),
        color(83, 112, 145),
        1.0,
    );

    let handle_h = WORKSPACE_VERTICAL_HANDLE_HEIGHT.min((rect.height - 28.0).max(0.0));
    if handle_h < 32.0 {
        return;
    }
    let handle_w = WORKSPACE_SPLITTER_HANDLE_THICKNESS;
    let handle = UiRect::new(
        x - handle_w * 0.5,
        rect.y + (rect.height - handle_h) * 0.5,
        handle_w,
        handle_h,
    );
    push_rect(
        primitives,
        handle,
        color(39, 58, 78),
        4.0,
        Some(stroke(color(95, 128, 161), 1.0)),
    );
    for offset in [-12.0_f32, 0.0, 12.0] {
        let dot = WORKSPACE_SPLITTER_DOT_SIZE;
        push_rect(
            primitives,
            UiRect::new(
                x - dot * 0.5,
                handle.y + handle.height * 0.5 + offset - dot * 0.5,
                dot,
                dot,
            ),
            accent(),
            dot * 0.5,
            None,
        );
    }
}

fn draw_horizontal_workspace_splitter(primitives: &mut Vec<ScenePrimitive>, rect: UiRect) {
    let y = rect.y + rect.height * 0.5;
    let gutter_w = (rect.width - 12.0).max(0.0);
    if gutter_w > 0.0 {
        let gutter_h = WORKSPACE_SPLITTER_GUTTER_THICKNESS;
        push_rect(
            primitives,
            UiRect::new(rect.x + 6.0, y - gutter_h * 0.5, gutter_w, gutter_h),
            color(12, 20, 29),
            6.0,
            Some(stroke(color(31, 45, 61), 1.0)),
        );
    }
    push_line(
        primitives,
        UiPoint::new(rect.x + 8.0, y),
        UiPoint::new(rect.right() - 8.0, y),
        color(83, 112, 145),
        1.0,
    );
    let handle_w = WORKSPACE_HORIZONTAL_HANDLE_WIDTH.min((rect.width - 28.0).max(0.0));
    if handle_w < 36.0 {
        return;
    }
    let handle_h = WORKSPACE_SPLITTER_HANDLE_THICKNESS;
    let handle = UiRect::new(
        rect.x + (rect.width - handle_w) * 0.5,
        y - handle_h * 0.5,
        handle_w,
        handle_h,
    );
    push_rect(
        primitives,
        handle,
        color(39, 58, 78),
        4.0,
        Some(stroke(color(95, 128, 161), 1.0)),
    );
    for offset in [-18.0_f32, 0.0, 18.0] {
        let dot = WORKSPACE_SPLITTER_DOT_SIZE;
        push_rect(
            primitives,
            UiRect::new(
                handle.x + handle.width * 0.5 + offset - dot * 0.5,
                y - dot * 0.5,
                dot,
                dot,
            ),
            accent(),
            dot * 0.5,
            None,
        );
    }
}

fn splitter_between(left: UiRect, right: UiRect) -> UiRect {
    let gap_center = (left.right() + right.x) * 0.5;
    let width = (right.x - left.right()).max(WORKSPACE_SPLITTER_HIT_SIZE);
    UiRect::new(
        gap_center - width * 0.5,
        left.y.min(right.y),
        width,
        left.height.max(right.height),
    )
}

pub(super) fn workspace_panel_width_limits(
    width: f32,
    target: WorkspaceResizeTarget,
    clip_panel_visible: bool,
) -> (f32, f32) {
    let side_available = side_panel_available_width(width, clip_panel_visible);
    let (min_width, min_others) = match target {
        WorkspaceResizeTarget::Left => (
            MIN_LEFT_PANEL_WIDTH,
            if clip_panel_visible {
                MIN_TRACK_PANEL_WIDTH
            } else {
                0.0
            } + MIN_RIGHT_PANEL_WIDTH,
        ),
        WorkspaceResizeTarget::Track => (
            MIN_TRACK_PANEL_WIDTH,
            MIN_LEFT_PANEL_WIDTH + MIN_RIGHT_PANEL_WIDTH,
        ),
        WorkspaceResizeTarget::Right => (
            MIN_RIGHT_PANEL_WIDTH,
            MIN_LEFT_PANEL_WIDTH
                + if clip_panel_visible {
                    MIN_TRACK_PANEL_WIDTH
                } else {
                    0.0
                },
        ),
        WorkspaceResizeTarget::Bottom => {
            return (MIN_BOTTOM_EDITOR_HEIGHT, f32::INFINITY);
        }
        WorkspaceResizeTarget::Browser => {
            return (0.0, f32::INFINITY);
        }
    };
    let max_width = (side_available - min_others).max(min_width);
    (min_width, max_width)
}

pub(super) fn fit_workspace_panel_widths(
    width: f32,
    desired: WorkspacePanelWidths,
    clip_panel_visible: bool,
) -> WorkspacePanelWidths {
    let mut panels = WorkspacePanelWidths {
        left: desired.left.max(MIN_LEFT_PANEL_WIDTH),
        track: if clip_panel_visible {
            desired.track.max(MIN_TRACK_PANEL_WIDTH)
        } else {
            0.0
        },
        right: desired.right.max(MIN_RIGHT_PANEL_WIDTH),
    };
    let side_available = side_panel_available_width(width, clip_panel_visible);
    let side_total = panels.left + panels.track + panels.right;
    if side_total <= side_available {
        return panels;
    }

    let left_flex = panels.left - MIN_LEFT_PANEL_WIDTH;
    let track_flex = if clip_panel_visible {
        panels.track - MIN_TRACK_PANEL_WIDTH
    } else {
        0.0
    };
    let right_flex = panels.right - MIN_RIGHT_PANEL_WIDTH;
    let flex_total = left_flex + track_flex + right_flex;
    if flex_total <= f32::EPSILON {
        return panels;
    }

    let overflow = side_total - side_available;
    panels.left -= overflow * (left_flex / flex_total);
    panels.track -= overflow * (track_flex / flex_total);
    panels.right -= overflow * (right_flex / flex_total);
    WorkspacePanelWidths {
        left: panels.left.max(MIN_LEFT_PANEL_WIDTH),
        track: if clip_panel_visible {
            panels.track.max(MIN_TRACK_PANEL_WIDTH)
        } else {
            0.0
        },
        right: panels.right.max(MIN_RIGHT_PANEL_WIDTH),
    }
}

fn default_workspace_panel_widths(width: f32) -> WorkspacePanelWidths {
    let compact = width < 1320.0;
    if compact {
        WorkspacePanelWidths {
            left: (width * 0.19).clamp(200.0, 230.0),
            track: (width * 0.14).clamp(160.0, 190.0),
            right: (width * 0.21).clamp(240.0, 270.0),
        }
    } else {
        WorkspacePanelWidths {
            left: width.mul_add(0.12, 72.0).clamp(220.0, 260.0),
            track: width.mul_add(0.06, 96.0).clamp(176.0, 210.0),
            right: width.mul_add(0.10, 110.0).clamp(260.0, 310.0),
        }
    }
}

fn side_panel_available_width(width: f32, clip_panel_visible: bool) -> f32 {
    let gap = 8.0;
    let min_center_w = if width < 1100.0 { 280.0 } else { 360.0 };
    let track_min = if clip_panel_visible {
        MIN_TRACK_PANEL_WIDTH
    } else {
        0.0
    };
    let gap_count = if clip_panel_visible { 5.0 } else { 4.0 };
    let min_side_total = MIN_LEFT_PANEL_WIDTH + track_min + MIN_RIGHT_PANEL_WIDTH;
    (width - gap * gap_count - min_center_w).max(min_side_total)
}
