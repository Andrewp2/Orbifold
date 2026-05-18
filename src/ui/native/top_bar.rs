use operad::{
    FontFamily, TextStyle, UiDocument, UiRect, UiVisual, WidgetActionBinding, layout, widgets,
};

use crate::app::AppState;
use crate::time::AppInstant;
use crate::ui::theme::{color, muted, stroke, strong};

use super::controls::{
    add_button_at, add_button_at_with_visible_label, add_compact_button_at,
    add_compact_button_at_with_visible_label, add_label_at, add_toggle_button_at,
};
use super::presenters::transport_position_label;

pub(super) fn add_top_bar_controls(document: &mut UiDocument, app: &AppState, width: f32) {
    let project = app.music_project.lock();
    let playing = project.transport.playing;
    let recording = project.transport.recording;
    let overdub = project.transport.overdub;
    let loop_beats = project.transport.loop_beats;
    let quantize_grid = project.transport.quantize_grid;
    let current_beat = project.current_position_beats(AppInstant::now());
    drop(project);

    let audio_available = app.audio_stream.is_some();
    let compact_top = width < 1500.0;
    let wide_trailing_controls = width >= 1800.0;
    let wide_quantize_controls = wide_trailing_controls;

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
        "Home",
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
        UiRect::new(734.0, 18.0, 60.0, 30.0),
        overdub,
        true,
    );
    let bpm_down_rect = UiRect::new(806.0, 18.0, 48.0, 30.0);
    let bpm_label_rect = UiRect::new(866.0, 18.0, 54.0, 30.0);
    let bpm_up_rect = UiRect::new(924.0, 18.0, 48.0, 30.0);
    add_compact_button_at(
        document,
        "transport.bpm_down",
        "BPM -",
        bpm_down_rect,
        false,
        true,
    );
    add_bpm_input_at(document, app, bpm_label_rect);
    add_compact_button_at(
        document,
        "transport.bpm_up",
        "BPM +",
        bpm_up_rect,
        false,
        true,
    );
    if !compact_top {
        add_compact_button_at(
            document,
            "transport.loop_down",
            "Loop -",
            UiRect::new(986.0, 18.0, 54.0, 30.0),
            false,
            true,
        );
        add_label_at(
            document,
            "readout.loop",
            format!("{loop_beats:.0} beats"),
            UiRect::new(1046.0, 18.0, 68.0, 30.0),
            false,
        );
        add_compact_button_at(
            document,
            "transport.loop_up",
            "Loop +",
            UiRect::new(1120.0, 18.0, 54.0, 30.0),
            false,
            true,
        );
    }
    let quantize_label = if compact_top {
        format!("Q{}", quantize_grid.as_str())
    } else {
        format!("Grid {}", quantize_grid.as_str())
    };
    let quantize_rect = UiRect::new(
        if compact_top { 984.0 } else { 1180.0 },
        18.0,
        if compact_top { 54.0 } else { 76.0 },
        30.0,
    );
    if compact_top {
        add_compact_button_at(
            document,
            "transport.quantize_grid",
            quantize_label,
            quantize_rect,
            false,
            true,
        );
    } else if wide_quantize_controls {
        add_compact_button_at(
            document,
            "transport.quantize_grid_prev",
            "<",
            UiRect::new(1180.0, 18.0, 24.0, 30.0),
            false,
            true,
        );
        add_button_at(
            document,
            "transport.quantize_grid",
            quantize_label,
            UiRect::new(1208.0, 18.0, 76.0, 30.0),
            false,
            true,
        );
        add_compact_button_at(
            document,
            "transport.quantize_grid_next",
            ">",
            UiRect::new(1296.0, 18.0, 24.0, 30.0),
            false,
            true,
        );
    } else {
        add_button_at(
            document,
            "transport.quantize_grid",
            quantize_label,
            quantize_rect,
            false,
            true,
        );
    }
    if width >= 1500.0 {
        let meter_x = if wide_quantize_controls {
            1328.0
        } else {
            1264.0
        };
        add_label_at(
            document,
            "readout.meter",
            "4/4",
            UiRect::new(meter_x, 18.0, 44.0, 30.0),
            true,
        );
    }
    if width >= 1600.0 {
        let position_x = if wide_quantize_controls {
            1384.0
        } else {
            1320.0
        };
        add_label_at(
            document,
            "readout.position",
            transport_position_label(current_beat),
            UiRect::new(position_x, 18.0, 76.0, 30.0),
            false,
        );
    }
    let (all_off_rect, settings_rect, a4_rect) = if compact_top {
        (
            UiRect::new(1042.0, 18.0, 58.0, 30.0),
            UiRect::new(1104.0, 18.0, 54.0, 30.0),
            UiRect::new(1160.0, 14.0, 36.0, 36.0),
        )
    } else if wide_trailing_controls {
        (
            UiRect::new(width - 238.0, 18.0, 62.0, 30.0),
            UiRect::new(width - 154.0, 18.0, 104.0, 30.0),
            UiRect::new(width - 44.0, 14.0, 36.0, 36.0),
        )
    } else {
        (
            UiRect::new(width - 190.0, 18.0, 62.0, 30.0),
            UiRect::new(width - 122.0, 18.0, 70.0, 30.0),
            UiRect::new(width - 44.0, 14.0, 36.0, 36.0),
        )
    };
    if compact_top {
        add_compact_button_at_with_visible_label(
            document,
            "audio.all_off",
            "All Off",
            "Panic",
            all_off_rect,
            false,
            true,
        );
    } else {
        add_button_at(
            document,
            "audio.all_off",
            "All Off",
            all_off_rect,
            false,
            true,
        );
    }
    if compact_top {
        add_compact_button_at_with_visible_label(
            document,
            "settings.save",
            "Save Settings",
            "Prefs",
            settings_rect,
            false,
            true,
        );
        add_compact_button_at(
            document,
            "audio.test_a4",
            "A4",
            a4_rect,
            false,
            audio_available,
        );
    } else {
        add_button_at_with_visible_label(
            document,
            "settings.save",
            "Save Settings",
            Some(if wide_trailing_controls {
                "Save Settings"
            } else {
                "Save Pref"
            }),
            settings_rect,
            false,
            true,
        );
        add_button_at(
            document,
            "audio.test_a4",
            "A4",
            a4_rect,
            false,
            audio_available,
        );
    }
}

fn add_bpm_input_at(document: &mut UiDocument, app: &AppState, rect: UiRect) {
    let state = widgets::TextInputState::new(app.bpm_edit_text());
    let mut options = widgets::TextInputOptions::default()
        .with_layout(layout::absolute(rect.x, rect.y, rect.width, rect.height))
        .with_placeholder("BPM")
        .with_edit_action(WidgetActionBinding::action("transport.bpm_input"));
    options.visual = UiVisual::panel(color(13, 20, 29), Some(stroke(color(48, 64, 84), 1.0)), 4.0);
    options.focused_visual = Some(UiVisual::panel(
        color(12, 22, 30),
        Some(stroke(color(68, 214, 224), 1.0)),
        4.0,
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
    options.accessibility_label = Some("BPM value".to_string());
    widgets::singleline_text_input(
        document,
        document.root,
        "transport.bpm_input",
        &state,
        options,
    );
}
