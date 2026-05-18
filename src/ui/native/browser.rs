use operad::{
    FontFamily, ScenePrimitive, TextHorizontalAlign, TextStyle, UiDocument, UiRect, UiVisual,
    WidgetActionBinding, layout, widgets,
};

use crate::app::{AppState, AudioAssetKind};
use crate::ui::text::fit_label;
use crate::ui::theme::{accent, color, muted, stroke, strong};

use super::controls::{
    add_button_at, add_button_at_with_visible_label, add_selectable_at, add_toggle_button_at,
};
use super::presenters::{
    asset_browser_summary, asset_tab_label, audio_asset_row_label, current_scale_intervals_label,
    current_scale_label, current_scale_metadata_label, recent_project_row_label,
    scale_library_row_label, selected_audio_asset_detail,
};
use super::{draw_panel, push_rect, push_text};

const MIN_POINTER_TARGET_SIZE: f32 = 24.0;
const BROWSER_GAP: f32 = 8.0;
const DEFERRED_FILE_ROW_HEIGHT: f32 = 32.0;
const MAX_RECENT_SESSION_ROWS: usize = 3;
const MIN_SCALE_BROWSER_HEIGHT: f32 = 154.0;
const MIN_ASSET_BROWSER_HEIGHT: f32 = 150.0;
const ASSET_DETAIL_MIN_PANEL_HEIGHT: f32 = 282.0;
const ASSET_DETAIL_HEIGHT: f32 = 42.0;
const ASSET_TAB_Y_OFFSET: f32 = 66.0;
const ASSET_TAB_HEIGHT: f32 = 26.0;
const ASSET_TAB_GAP: f32 = 6.0;
const ASSET_TAB_ROW_GAP: f32 = 4.0;
const SCALE_DETAIL_MIN_PANEL_HEIGHT: f32 = 236.0;
const SCALE_SEARCH_MIN_PANEL_HEIGHT: f32 = 292.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct LeftBrowserRects {
    pub(super) module_bar: UiRect,
    pub(super) deferred_files: UiRect,
    pub(super) scales: UiRect,
    pub(super) assets: UiRect,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BrowserListMetrics {
    pub(super) viewport: UiRect,
    pub(super) row_stride: f32,
    pub(super) row_height: f32,
    pub(super) visible_rows: usize,
    pub(super) start: usize,
    pub(super) total: usize,
}

impl BrowserListMetrics {
    pub(super) fn is_scrollable(self) -> bool {
        self.total > self.visible_rows && self.visible_rows > 0
    }

    fn row_width(self, panel: UiRect) -> f32 {
        let scrollbar_padding = if self.is_scrollable() { 32.0 } else { 0.0 };
        (panel.width - 20.0 - scrollbar_padding).max(1.0)
    }

    pub(super) fn row_rect(self, panel: UiRect, visible_idx: usize) -> UiRect {
        UiRect::new(
            panel.x + 10.0,
            self.viewport.y + visible_idx as f32 * self.row_stride,
            self.row_width(panel),
            self.row_height,
        )
    }
}

pub(super) fn left_browser_rects(app: &AppState, rect: UiRect) -> LeftBrowserRects {
    let module_bar = UiRect::new(rect.x, rect.y, rect.width, 42.0_f32.min(rect.height));
    let deferred_h = deferred_file_height(app, rect, module_bar);
    let deferred_y = module_bar.bottom() + BROWSER_GAP;
    let deferred_files = UiRect::new(rect.x, deferred_y, rect.width, deferred_h);
    let content_y = if deferred_h > 0.0 {
        deferred_files.bottom() + BROWSER_GAP
    } else {
        module_bar.bottom() + BROWSER_GAP
    };
    let content_h = (rect.bottom() - content_y).max(0.0);
    let (scales_h, asset_y, asset_h) = browser_section_heights(app, content_y, content_h);
    let scales = UiRect::new(rect.x, content_y, rect.width, scales_h);
    let assets = UiRect::new(rect.x, asset_y, rect.width, asset_h);
    LeftBrowserRects {
        module_bar,
        deferred_files,
        scales,
        assets,
    }
}

pub(super) fn left_browser_splitter_rect(app: &AppState, rect: UiRect) -> Option<UiRect> {
    if !app.show_scale_browser || !app.show_asset_browser {
        return None;
    }
    let sections = left_browser_rects(app, rect);
    if sections.scales.height <= 0.0 || sections.assets.height <= 0.0 {
        return None;
    }
    Some(UiRect::new(
        rect.x,
        sections.scales.bottom(),
        rect.width,
        (sections.assets.y - sections.scales.bottom()).max(1.0),
    ))
}

pub(super) fn left_browser_split_height_at(
    app: &AppState,
    rect: UiRect,
    point: operad::UiPoint,
) -> Option<f32> {
    if !app.show_scale_browser || !app.show_asset_browser {
        return None;
    }
    let (_, content_y, content_h) = left_browser_content_geometry(app, rect);
    let available = (content_h - BROWSER_GAP).max(0.0);
    let (min, max) = browser_scale_height_limits(available);
    Some((point.y - content_y).clamp(min, max))
}

pub(super) fn draw_left_browser(
    primitives: &mut Vec<ScenePrimitive>,
    rect: UiRect,
    app: &AppState,
) {
    let LeftBrowserRects {
        module_bar,
        deferred_files,
        scales,
        assets,
    } = left_browser_rects(app, rect);
    draw_browser_module_bar(primitives, module_bar);
    if deferred_files.height > 0.0 {
        draw_panel(primitives, deferred_files, "");
    }
    if app.show_scale_browser && scales.height > 0.0 {
        draw_panel(primitives, scales, "SCALES & TUNINGS");
        draw_current_scale(primitives, scales, app);
    }
    if app.show_asset_browser && assets.height > 0.0 {
        draw_panel(primitives, assets, "ASSET BROWSER");
        draw_asset_browser(primitives, assets, app);
    }
}

fn draw_browser_module_bar(primitives: &mut Vec<ScenePrimitive>, rect: UiRect) {
    draw_panel(primitives, rect, "");
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
    if scale_detail_visible(panel) {
        let mut detail_y = row.bottom() + 6.0;
        if let Some(metadata) = current_scale_metadata_label(app) {
            push_text(
                primitives,
                UiRect::new(panel.x + 12.0, detail_y, panel.width - 24.0, 14.0),
                fit_label(&metadata, panel.width - 24.0, 11.0),
                11.0,
                muted(),
                TextHorizontalAlign::Start,
            );
            detail_y += 16.0;
        }
        let intervals = current_scale_intervals_label(app);
        push_text(
            primitives,
            UiRect::new(panel.x + 12.0, detail_y, panel.width - 24.0, 14.0),
            fit_label(&intervals, panel.width - 24.0, 11.0),
            11.0,
            muted(),
            TextHorizontalAlign::Start,
        );
    }
    if app.scale_library.is_empty() && row.bottom() + 34.0 <= panel.bottom() - 34.0 {
        push_text(
            primitives,
            UiRect::new(
                panel.x + 12.0,
                scale_list_top(panel) + 4.0,
                panel.width - 24.0,
                18.0,
            ),
            "No .scl files found",
            12.0,
            muted(),
            TextHorizontalAlign::Start,
        );
    } else if !app.scale_library_search_query().is_empty()
        && app.filtered_scale_library_count() == 0
        && scale_search_visible(panel)
    {
        push_text(
            primitives,
            UiRect::new(
                panel.x + 12.0,
                scale_list_top(panel) + 4.0,
                panel.width - 24.0,
                18.0,
            ),
            "No scale matches",
            12.0,
            muted(),
            TextHorizontalAlign::Start,
        );
    }
    draw_browser_list_scrollbar(primitives, scale_list_metrics(app, panel));
}

fn draw_asset_browser(primitives: &mut Vec<ScenePrimitive>, panel: UiRect, app: &AppState) {
    push_text(
        primitives,
        UiRect::new(panel.x + 12.0, panel.y + 10.0, panel.width - 24.0, 18.0),
        asset_browser_summary(app),
        12.0,
        muted(),
        TextHorizontalAlign::End,
    );
    let selected_kind = app.selected_audio_asset_kind;
    let has_visible_assets = app
        .audio_assets
        .iter()
        .any(|asset| app.audio_asset_matches_browser_filter(asset, selected_kind));
    if !has_visible_assets && panel.height >= 190.0 {
        let empty = UiRect::new(
            panel.x + 10.0,
            asset_tabs_bottom(panel) + 14.0,
            panel.width - 20.0,
            68.0,
        );
        push_rect(
            primitives,
            empty,
            color(10, 16, 23),
            5.0,
            Some(stroke(color(35, 48, 64), 1.0)),
        );
        push_text(
            primitives,
            UiRect::new(empty.x + 12.0, empty.y + 25.0, empty.width - 24.0, 18.0),
            if app.audio_asset_search_query().is_empty() {
                "Refresh or import assets".to_string()
            } else {
                "No search matches".to_string()
            },
            13.0,
            strong(),
            TextHorizontalAlign::Start,
        );
        return;
    }
    draw_selected_asset_detail(primitives, panel, app, app.selected_audio_asset_kind);
    draw_browser_list_scrollbar(
        primitives,
        asset_list_metrics(app, panel, app.selected_audio_asset_kind),
    );
}

fn draw_selected_asset_detail(
    primitives: &mut Vec<ScenePrimitive>,
    panel: UiRect,
    app: &AppState,
    selected_asset_kind: AudioAssetKind,
) {
    let Some(rect) = asset_detail_rect(app, panel, selected_asset_kind) else {
        return;
    };
    let detail = selected_audio_asset_detail(app);
    push_rect(
        primitives,
        rect,
        color(10, 16, 23),
        5.0,
        Some(stroke(color(35, 48, 64), 1.0)),
    );
    push_text(
        primitives,
        UiRect::new(rect.x + 10.0, rect.y + 6.0, rect.width - 20.0, 15.0),
        fit_label(&detail.title, rect.width - 20.0, 12.0),
        12.0,
        strong(),
        TextHorizontalAlign::Start,
    );
    push_text(
        primitives,
        UiRect::new(rect.x + 10.0, rect.y + 23.0, rect.width - 20.0, 14.0),
        fit_label(&detail.status, rect.width - 20.0, 11.0),
        11.0,
        muted(),
        TextHorizontalAlign::Start,
    );
}

fn draw_browser_list_scrollbar(primitives: &mut Vec<ScenePrimitive>, metrics: BrowserListMetrics) {
    let Some((track, thumb)) = list_scrollbar_rects(metrics) else {
        return;
    };
    push_rect(primitives, track, color(16, 26, 36), 2.0, None);
    push_rect(primitives, thumb, color(67, 96, 124), 2.0, None);
}

fn browser_section_heights(app: &AppState, content_y: f32, content_h: f32) -> (f32, f32, f32) {
    match (app.show_scale_browser, app.show_asset_browser) {
        (true, true) => {
            let available = (content_h - BROWSER_GAP).max(0.0);
            let (min, max) = browser_scale_height_limits(available);
            let default_scale_h = default_browser_scale_height(available);
            let scale_h = app
                .left_browser_split_height()
                .unwrap_or(default_scale_h)
                .clamp(min, max);
            let asset_y = content_y + scale_h + BROWSER_GAP;
            let asset_h = (available - scale_h).max(0.0);
            (scale_h, asset_y, asset_h)
        }
        (true, false) => (content_h, content_y + content_h, 0.0),
        (false, true) => (0.0, content_y, content_h),
        (false, false) => (0.0, content_y, 0.0),
    }
}

fn left_browser_content_geometry(app: &AppState, rect: UiRect) -> (UiRect, f32, f32) {
    let module_bar = UiRect::new(rect.x, rect.y, rect.width, 42.0_f32.min(rect.height));
    let deferred_h = deferred_file_height(app, rect, module_bar);
    let deferred_y = module_bar.bottom() + BROWSER_GAP;
    let deferred_files = UiRect::new(rect.x, deferred_y, rect.width, deferred_h);
    let content_y = if deferred_h > 0.0 {
        deferred_files.bottom() + BROWSER_GAP
    } else {
        module_bar.bottom() + BROWSER_GAP
    };
    let content_h = (rect.bottom() - content_y).max(0.0);
    (deferred_files, content_y, content_h)
}

fn browser_scale_height_limits(available: f32) -> (f32, f32) {
    let available = available.max(0.0);
    let min = MIN_SCALE_BROWSER_HEIGHT.min(available);
    let max = (available - MIN_ASSET_BROWSER_HEIGHT)
        .max(min)
        .min(available);
    (min, max)
}

fn default_browser_scale_height(available: f32) -> f32 {
    let (min, max) = browser_scale_height_limits(available);
    (available * 0.36).clamp(min, max)
}

fn deferred_file_height(app: &AppState, rect: UiRect, module_bar: UiRect) -> f32 {
    let rows = deferred_file_row_count(app);
    if rows == 0 {
        return 0.0;
    }
    let max_rows = MAX_RECENT_SESSION_ROWS + usize::from(app.autosave_available);
    let max_height = max_rows as f32 * DEFERRED_FILE_ROW_HEIGHT + 2.0;
    let available = (rect.bottom() - module_bar.bottom() - BROWSER_GAP).clamp(0.0, max_height);
    (rows as f32 * DEFERRED_FILE_ROW_HEIGHT + 2.0).min(available)
}

fn deferred_file_row_count(app: &AppState) -> usize {
    usize::from(app.autosave_available) + recent_session_row_count(app)
}

fn recent_session_row_count(app: &AppState) -> usize {
    app.recent_project_paths()
        .len()
        .min(MAX_RECENT_SESSION_ROWS)
}

pub(super) fn scale_list_metrics(app: &AppState, panel: UiRect) -> BrowserListMetrics {
    let row_y = scale_list_top(panel);
    let button_y = panel.bottom() - 34.0;
    let row_stride = 26.0;
    let row_height = MIN_POINTER_TARGET_SIZE;
    let available_h = (button_y - row_y - 4.0).max(0.0);
    let visible_rows = (available_h / row_stride).floor().max(0.0) as usize;
    let indices = app.filtered_scale_library_indices();
    let total = indices.len();
    let start = app.filtered_scale_library_list_start(&indices, visible_rows);
    BrowserListMetrics {
        viewport: UiRect::new(
            panel.x + 10.0,
            row_y,
            panel.width - 20.0,
            visible_rows as f32 * row_stride,
        ),
        row_stride,
        row_height,
        visible_rows,
        start,
        total,
    }
}

fn scale_search_visible(panel: UiRect) -> bool {
    panel.height >= SCALE_SEARCH_MIN_PANEL_HEIGHT
}

fn scale_detail_visible(panel: UiRect) -> bool {
    panel.height >= SCALE_DETAIL_MIN_PANEL_HEIGHT
}

fn scale_list_top(panel: UiRect) -> f32 {
    let mut top = panel.y + 64.0;
    if scale_detail_visible(panel) {
        top += 34.0;
    }
    if scale_search_visible(panel) {
        top += 32.0;
    }
    top
}

pub(super) fn asset_list_metrics(
    app: &AppState,
    panel: UiRect,
    selected_asset_kind: AudioAssetKind,
) -> BrowserListMetrics {
    let button_y = panel.bottom() - 34.0;
    let row_y = asset_tabs_bottom(panel) + 8.0;
    let row_stride = MIN_POINTER_TARGET_SIZE + 2.0;
    let row_height = MIN_POINTER_TARGET_SIZE;
    let row_bottom = asset_detail_rect(app, panel, selected_asset_kind)
        .map(|rect| rect.y - 6.0)
        .unwrap_or_else(|| {
            let next_action_y = asset_use_sample_row_y(app, panel)
                .or_else(|| asset_preview_row_y(app, panel))
                .unwrap_or(button_y);
            next_action_y - 8.0
        });
    let visible_rows = ((row_bottom - row_y) / row_stride).floor().max(0.0) as usize;
    let visible_assets = app
        .audio_assets
        .iter()
        .enumerate()
        .filter(|(_, item)| app.audio_asset_matches_browser_filter(item, selected_asset_kind))
        .collect::<Vec<_>>();
    let selected_asset_position = app
        .selected_audio_asset
        .and_then(|selected| visible_assets.iter().position(|(idx, _)| *idx == selected))
        .unwrap_or(0);
    let total = visible_assets.len();
    let start = app.audio_asset_list_start(
        selected_asset_kind,
        selected_asset_position,
        total,
        visible_rows,
    );
    BrowserListMetrics {
        viewport: UiRect::new(
            panel.x + 10.0,
            row_y,
            panel.width - 20.0,
            visible_rows as f32 * row_stride,
        ),
        row_stride,
        row_height,
        visible_rows,
        start,
        total,
    }
}

pub(super) fn asset_detail_rect(
    app: &AppState,
    panel: UiRect,
    selected_asset_kind: AudioAssetKind,
) -> Option<UiRect> {
    app.selected_audio_asset_item()?;
    if panel.height < ASSET_DETAIL_MIN_PANEL_HEIGHT {
        return None;
    }
    let total = app
        .audio_assets
        .iter()
        .filter(|asset| app.audio_asset_matches_browser_filter(asset, selected_asset_kind))
        .count();
    if total > 1 {
        let row_y = asset_tabs_bottom(panel) + 8.0;
        let row_bottom = panel.bottom() - 146.0 - 6.0;
        let rows_with_detail = ((row_bottom - row_y) / (MIN_POINTER_TARGET_SIZE + 2.0))
            .floor()
            .max(0.0) as usize;
        if rows_with_detail < 3 {
            return None;
        }
    }
    Some(UiRect::new(
        panel.x + 10.0,
        panel.bottom() - 146.0,
        panel.width - 20.0,
        ASSET_DETAIL_HEIGHT,
    ))
}

fn asset_use_sample_row_y(app: &AppState, panel: UiRect) -> Option<f32> {
    let row_has_relevant_action = app
        .selected_audio_asset_item()
        .is_some_and(|asset| asset.kind == AudioAssetKind::Sample)
        || app.can_clear_sample_instrument();
    if !row_has_relevant_action {
        return None;
    }
    let asset_tab_bottom = asset_tabs_bottom(panel);
    let y = panel.bottom() - 34.0 - 64.0;
    (y >= asset_tab_bottom + 8.0).then_some(y)
}

fn asset_preview_row_y(app: &AppState, panel: UiRect) -> Option<f32> {
    if !app
        .selected_audio_asset_item()
        .is_some_and(|asset| asset.kind == AudioAssetKind::Sample)
    {
        return None;
    }
    let asset_tab_bottom = asset_tabs_bottom(panel);
    let y = panel.bottom() - 34.0 - 32.0;
    (y >= asset_tab_bottom + 8.0).then_some(y)
}

pub(super) fn list_scrollbar_rects(metrics: BrowserListMetrics) -> Option<(UiRect, UiRect)> {
    if !metrics.is_scrollable() || metrics.viewport.height <= 0.0 {
        return None;
    }
    let reserved_button_h = if metrics.viewport.height >= 76.0 {
        56.0
    } else {
        0.0
    };
    let track = UiRect::new(
        metrics.viewport.right() - 9.0,
        metrics.viewport.y + reserved_button_h * 0.5,
        3.0,
        (metrics.viewport.height - reserved_button_h).max(1.0),
    );
    let visible_ratio = metrics.visible_rows as f32 / metrics.total as f32;
    let thumb_h = (track.height * visible_ratio).clamp(18.0, track.height);
    let max_start = metrics.total.saturating_sub(metrics.visible_rows).max(1);
    let progress = metrics.start as f32 / max_start as f32;
    let thumb_y = track.y + (track.height - thumb_h) * progress.clamp(0.0, 1.0);
    let thumb = UiRect::new(track.x, thumb_y, track.width, thumb_h);
    Some((track, thumb))
}

pub(super) fn list_scroll_button_rects(metrics: BrowserListMetrics) -> Option<(UiRect, UiRect)> {
    if !metrics.is_scrollable() || metrics.viewport.height < 48.0 {
        return None;
    }
    let x = metrics.viewport.right() - 28.0;
    let up = UiRect::new(x, metrics.viewport.y, 24.0, 24.0);
    let down = UiRect::new(x, metrics.viewport.bottom() - 24.0, 24.0, 24.0);
    Some((up, down))
}

pub(super) fn add_browser_module_controls(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
) {
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

pub(super) fn add_deferred_file_controls(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    if panel.height <= 0.0 {
        return;
    }
    let mut y = panel.y + 3.0;
    let gap = 6.0;
    if app.autosave_available {
        let recover_enabled = !app.project_dirty;
        let button_w = ((panel.width - 20.0 - gap) / 2.0).max(50.0);
        add_button_at(
            document,
            "file.recover",
            "Recover",
            UiRect::new(panel.x + 10.0, y, button_w, 26.0),
            recover_enabled,
            recover_enabled,
        );
        add_button_at(
            document,
            "file.dismiss_autosave",
            "Dismiss",
            UiRect::new(panel.x + 10.0 + button_w + gap, y, button_w, 26.0),
            recover_enabled,
            recover_enabled,
        );
        y += DEFERRED_FILE_ROW_HEIGHT;
    }
    for (idx, path) in app
        .recent_project_paths()
        .iter()
        .take(recent_session_row_count(app))
        .enumerate()
    {
        if y + 26.0 > panel.bottom() {
            break;
        }
        let available = (panel.width - 20.0 - gap).max(1.0);
        let forget_w = (available * 0.34).clamp(48.0, 62.0).min(available * 0.45);
        let open_w = (available - forget_w).max(1.0);
        let open_action = if idx == 0 {
            "file.open_recent".to_string()
        } else {
            format!("file.open_recent.{idx}")
        };
        let forget_action = if idx == 0 {
            "file.forget_recent".to_string()
        } else {
            format!("file.forget_recent.{idx}")
        };
        add_button_at(
            document,
            open_action,
            recent_project_row_label(idx, path),
            UiRect::new(panel.x + 10.0, y, open_w, 26.0),
            false,
            !app.project_dirty && path.exists(),
        );
        add_button_at(
            document,
            forget_action,
            "Forget",
            UiRect::new(panel.x + 10.0 + open_w + gap, y, forget_w, 26.0),
            false,
            true,
        );
        y += DEFERRED_FILE_ROW_HEIGHT;
    }
}

pub(super) fn add_scale_browser_controls(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    if scale_search_visible(panel) {
        add_scale_search_control(document, app, panel);
    }
    let metrics = scale_list_metrics(app, panel);
    let scale_button_y = panel.bottom() - 34.0;
    let visible_scales = app.filtered_scale_library_indices();
    for (visible_idx, idx) in visible_scales
        .iter()
        .copied()
        .skip(metrics.start)
        .take(metrics.visible_rows)
        .enumerate()
    {
        let Some(item) = app.scale_library.get(idx) else {
            continue;
        };
        add_selectable_at(
            document,
            format!("scale.select.{idx}"),
            scale_library_row_label(item),
            metrics.row_rect(panel, visible_idx),
            idx == app.selected_scale_library,
            true,
        );
    }
    add_list_scroll_controls(document, "scale", metrics);
    let scale_button_gap = 6.0;
    let scale_button_available = (panel.width - 20.0 - scale_button_gap * 3.0).max(1.0);
    let load_button_w = if scale_button_available >= 176.0 {
        68.0
    } else {
        (scale_button_available * 0.38).max(1.0)
    };
    let scale_button_w = ((scale_button_available - load_button_w) / 3.0).max(1.0);
    let import_label = if scale_button_w >= 50.0 {
        "Import"
    } else {
        "Imp"
    };
    let refresh_label = if scale_button_w >= 56.0 {
        "Refresh"
    } else {
        "Ref"
    };
    let remove_label = if scale_button_w >= 54.0 {
        "Remove"
    } else {
        "Del"
    };
    let selected_scale_loaded = app.selected_library_scale_is_loaded();
    let selected_scale_visible = visible_scales.contains(&app.selected_scale_library);
    add_button_at(
        document,
        "scale.load_selected",
        if selected_scale_loaded {
            "Loaded"
        } else {
            "Load"
        },
        UiRect::new(panel.x + 10.0, scale_button_y, load_button_w, 26.0),
        selected_scale_loaded,
        selected_scale_visible && !selected_scale_loaded,
    );
    add_button_at_with_visible_label(
        document,
        "scale.import",
        "Import",
        Some(import_label),
        UiRect::new(
            panel.x + 10.0 + load_button_w + scale_button_gap,
            scale_button_y,
            scale_button_w,
            26.0,
        ),
        false,
        true,
    );
    add_button_at_with_visible_label(
        document,
        "scale.refresh",
        "Refresh",
        Some(refresh_label),
        UiRect::new(
            panel.x + 10.0 + load_button_w + scale_button_gap + scale_button_w + scale_button_gap,
            scale_button_y,
            scale_button_w,
            26.0,
        ),
        false,
        true,
    );
    add_button_at_with_visible_label(
        document,
        "scale.remove_selected",
        "Remove",
        Some(remove_label),
        UiRect::new(
            panel.x
                + 10.0
                + load_button_w
                + scale_button_gap
                + (scale_button_w + scale_button_gap) * 2.0,
            scale_button_y,
            scale_button_w,
            26.0,
        ),
        false,
        selected_scale_visible && app.can_remove_selected_library_scale(),
    );
}

fn add_scale_search_control(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    let state = widgets::TextInputState::new(app.scale_library_search_query());
    let show_clear = !app.scale_library_search_query().is_empty();
    let clear_w = 58.0;
    let gap = 6.0;
    let input_w = if show_clear {
        (panel.width - 20.0 - clear_w - gap).max(64.0)
    } else {
        panel.width - 20.0
    };
    let mut options = widgets::TextInputOptions::default()
        .with_layout(layout::absolute(
            panel.x + 10.0,
            scale_search_y(panel),
            input_w,
            24.0,
        ))
        .with_placeholder("Search scales")
        .with_edit_action(WidgetActionBinding::action("scale.search"));
    options.visual = UiVisual::panel(color(10, 16, 23), Some(stroke(color(38, 52, 70), 1.0)), 3.0);
    options.focused_visual = Some(UiVisual::panel(
        color(12, 22, 30),
        Some(stroke(color(68, 214, 224), 1.0)),
        3.0,
    ));
    options.text_style = TextStyle {
        font_size: 12.0,
        line_height: 16.0,
        family: FontFamily::SansSerif,
        color: strong(),
        ..Default::default()
    };
    options.placeholder_style = TextStyle {
        font_size: 12.0,
        line_height: 16.0,
        family: FontFamily::SansSerif,
        color: muted(),
        ..Default::default()
    };
    widgets::search_input(document, document.root, "scale.search", &state, options);
    if show_clear {
        add_button_at(
            document,
            "scale.search_clear",
            "Clear",
            UiRect::new(
                panel.x + 10.0 + input_w + gap,
                scale_search_y(panel),
                clear_w,
                24.0,
            ),
            false,
            true,
        );
    }
}

fn scale_search_y(panel: UiRect) -> f32 {
    if scale_detail_visible(panel) {
        panel.y + 100.0
    } else {
        panel.y + 66.0
    }
}

pub(super) fn add_asset_browser_controls(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    selected_asset_kind: AudioAssetKind,
) {
    add_asset_search_control(document, app, panel);
    for (idx, kind) in AudioAssetKind::all().iter().enumerate() {
        add_selectable_at(
            document,
            format!("asset.kind.{idx}"),
            asset_tab_label(*kind),
            asset_tab_rect(panel, idx),
            *kind == app.selected_audio_asset_kind,
            true,
        );
    }
    let asset_button_y = panel.bottom() - 34.0;
    let metrics = asset_list_metrics(app, panel, selected_asset_kind);
    let visible_assets = app
        .audio_assets
        .iter()
        .enumerate()
        .filter(|(_, item)| app.audio_asset_matches_browser_filter(item, selected_asset_kind))
        .collect::<Vec<_>>();
    for (visible_idx, (idx, item)) in visible_assets
        .into_iter()
        .skip(metrics.start)
        .take(metrics.visible_rows)
        .enumerate()
    {
        add_selectable_at(
            document,
            format!("asset.select.{idx}"),
            audio_asset_row_label(item),
            metrics.row_rect(panel, visible_idx),
            app.selected_audio_asset == Some(idx),
            true,
        );
    }
    add_list_scroll_controls(document, "asset", metrics);
    let button_gap = 6.0;
    let button_w = ((panel.width - 20.0 - button_gap) / 2.0).max(50.0);
    if let Some(preview_y) = asset_preview_row_y(app, panel) {
        add_button_at(
            document,
            "asset.preview",
            "Preview",
            UiRect::new(panel.x + 10.0, preview_y, button_w, 26.0),
            false,
            app.can_preview_selected_audio_asset(),
        );
        add_button_at(
            document,
            "asset.stop_preview",
            "Stop",
            UiRect::new(
                panel.x + 10.0 + button_w + button_gap,
                preview_y,
                button_w,
                26.0,
            ),
            false,
            app.can_preview_selected_audio_asset(),
        );
    }
    if let Some(use_sample_y) = asset_use_sample_row_y(app, panel) {
        add_button_at(
            document,
            "asset.use_sample",
            if app.selected_sample_instrument_is_loaded() {
                "Loaded"
            } else {
                "Use"
            },
            UiRect::new(panel.x + 10.0, use_sample_y, button_w, 26.0),
            app.selected_sample_instrument_is_loaded(),
            app.can_load_selected_sample_instrument()
                && !app.selected_sample_instrument_is_loaded(),
        );
        add_button_at(
            document,
            "asset.clear_sample",
            "Clear",
            UiRect::new(
                panel.x + 10.0 + button_w + button_gap,
                use_sample_y,
                button_w,
                26.0,
            ),
            false,
            app.can_clear_sample_instrument(),
        );
    }
    add_button_at(
        document,
        "asset.refresh",
        "Refresh",
        UiRect::new(panel.x + 10.0, asset_button_y, button_w, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        "asset.import",
        "Import",
        UiRect::new(
            panel.x + 10.0 + button_w + button_gap,
            asset_button_y,
            button_w,
            26.0,
        ),
        false,
        true,
    );
}

fn asset_tab_rect(panel: UiRect, idx: usize) -> UiRect {
    let col = idx % 2;
    let row = idx / 2;
    let tab_w = ((panel.width - 20.0 - ASSET_TAB_GAP) / 2.0).max(44.0);
    UiRect::new(
        panel.x + 10.0 + col as f32 * (tab_w + ASSET_TAB_GAP),
        panel.y + ASSET_TAB_Y_OFFSET + row as f32 * (ASSET_TAB_HEIGHT + ASSET_TAB_ROW_GAP),
        tab_w,
        ASSET_TAB_HEIGHT,
    )
}

fn asset_tabs_bottom(panel: UiRect) -> f32 {
    panel.y + ASSET_TAB_Y_OFFSET + ASSET_TAB_HEIGHT * 2.0 + ASSET_TAB_ROW_GAP
}

fn add_asset_search_control(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    let state = widgets::TextInputState::new(app.audio_asset_search_query());
    let show_clear = !app.audio_asset_search_query().is_empty();
    let clear_w = 58.0;
    let gap = 6.0;
    let input_w = if show_clear {
        (panel.width - 20.0 - clear_w - gap).max(64.0)
    } else {
        panel.width - 20.0
    };
    let mut options = widgets::TextInputOptions::default()
        .with_layout(layout::absolute(
            panel.x + 10.0,
            panel.y + 34.0,
            input_w,
            24.0,
        ))
        .with_placeholder("Search assets")
        .with_edit_action(WidgetActionBinding::action("asset.search"));
    options.visual = UiVisual::panel(color(10, 16, 23), Some(stroke(color(38, 52, 70), 1.0)), 3.0);
    options.focused_visual = Some(UiVisual::panel(
        color(12, 22, 30),
        Some(stroke(color(68, 214, 224), 1.0)),
        3.0,
    ));
    options.text_style = TextStyle {
        font_size: 12.0,
        line_height: 16.0,
        family: FontFamily::SansSerif,
        color: strong(),
        ..Default::default()
    };
    options.placeholder_style = TextStyle {
        font_size: 12.0,
        line_height: 16.0,
        family: FontFamily::SansSerif,
        color: muted(),
        ..Default::default()
    };
    options.accessibility_label = Some("Search assets".to_string());
    widgets::search_input(document, document.root, "asset.search", &state, options);

    if show_clear {
        add_button_at(
            document,
            "asset.search_clear",
            "Clear",
            UiRect::new(
                panel.x + 10.0 + input_w + gap,
                panel.y + 33.0,
                clear_w,
                26.0,
            ),
            false,
            true,
        );
    }
}

fn add_list_scroll_controls(document: &mut UiDocument, prefix: &str, metrics: BrowserListMetrics) {
    let Some((up, down)) = list_scroll_button_rects(metrics) else {
        return;
    };
    let max_start = metrics.total.saturating_sub(metrics.visible_rows);
    let up_start = metrics.start.saturating_sub(1);
    let down_start = (metrics.start + 1).min(max_start);
    add_button_at(
        document,
        format!("{prefix}.scroll_up.{up_start}"),
        "^",
        up,
        false,
        metrics.start > 0,
    );
    add_button_at(
        document,
        format!("{prefix}.scroll_down.{down_start}"),
        "v",
        down,
        false,
        metrics.start + metrics.visible_rows < metrics.total,
    );
}
