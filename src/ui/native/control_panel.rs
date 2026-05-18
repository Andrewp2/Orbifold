use operad::{
    FontFamily, TextStyle, UiDocument, UiRect, UiVisual, WidgetActionBinding, layout, widgets,
};

use crate::app::AppState;

use super::controls::{
    add_button_at, add_button_at_with_visible_label, add_label_at, add_toggle_button_at,
};
use super::devices::device_control_rects;
use crate::ui::labels::{
    audio_connect_label, lumatone_map_label, midi_connect_label, midi_event_label,
    selected_audio_output_name, selected_midi_input_name,
};
use crate::ui::text::compact_label;
use crate::ui::theme::{color, muted, stroke, strong};

pub(super) fn add_control_panel_controls(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    right_control_bottom: f32,
) {
    let project = app.music_project.lock();
    let metronome_enabled = project.transport.metronome_enabled;
    drop(project);

    let scale = app.scale_state.lock().clone();
    let synth = app.synth.settings();
    let synth_muted = app.synth.muted();
    let synth_limited = app.synth.output_limited();
    let selected_midi = selected_midi_input_name(app);
    let selected_audio = selected_audio_output_name(app);
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

    add_label_at(
        document,
        "scale.root.label",
        "Root",
        UiRect::new(panel.x + 16.0, panel.y + 44.0, 42.0, 18.0),
        false,
    );
    add_root_midi_input_at(
        document,
        app,
        UiRect::new(
            panel.x + 60.0,
            panel.y + 40.0,
            (panel.width - 156.0).max(72.0),
            26.0,
        ),
    );
    add_button_at(
        document,
        "scale.root_down",
        "-",
        UiRect::new(panel.right() - 84.0, panel.y + 40.0, 30.0, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        "scale.root_up",
        "+",
        UiRect::new(panel.right() - 48.0, panel.y + 40.0, 30.0, 26.0),
        false,
        true,
    );
    add_label_at(
        document,
        "scale.base.label",
        "Base",
        UiRect::new(panel.x + 16.0, panel.y + 70.0, 42.0, 18.0),
        false,
    );
    add_base_freq_input_at(
        document,
        app,
        UiRect::new(
            panel.x + 60.0,
            panel.y + 66.0,
            (panel.width - 156.0).max(72.0),
            26.0,
        ),
    );
    add_button_at(
        document,
        "scale.base_down",
        "-",
        UiRect::new(panel.right() - 84.0, panel.y + 66.0, 30.0, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        "scale.base_up",
        "+",
        UiRect::new(panel.right() - 48.0, panel.y + 66.0, 30.0, 26.0),
        false,
        true,
    );
    add_label_at(
        document,
        "scale.description",
        format!("Scale {}", compact_label(&scale.scale.description, 28)),
        UiRect::new(panel.x + 16.0, panel.y + 96.0, panel.width - 32.0, 18.0),
        false,
    );
    add_label_at(
        document,
        "transport.metronome.label",
        "Metronome",
        UiRect::new(panel.x + 16.0, panel.y + 122.0, 130.0, 18.0),
        false,
    );
    add_toggle_button_at(
        document,
        "transport.metronome",
        if metronome_enabled { "On" } else { "Off" },
        UiRect::new(panel.right() - 82.0, panel.y + 118.0, 64.0, 26.0),
        metronome_enabled,
        true,
    );
    let capture_buttons = capture_control_rects(panel, panel.y + 154.0);
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
        UiRect::new(panel.x + 16.0, panel.y + 184.0, panel.width - 32.0, 18.0),
        false,
    );

    let wide_channel_filter = panel.width >= 300.0;
    let last_midi_w = if wide_channel_filter {
        panel.width - 156.0
    } else {
        panel.width - 110.0
    };
    add_label_at(
        document,
        "midi.last",
        midi_last.unwrap_or_else(|| "Last MIDI none".to_string()),
        UiRect::new(panel.x + 16.0, panel.y + 214.0, last_midi_w, 18.0),
        false,
    );
    if wide_channel_filter {
        add_button_at(
            document,
            "midi.channel_filter_prev",
            "<",
            UiRect::new(panel.right() - 140.0, panel.y + 210.0, 30.0, 26.0),
            false,
            true,
        );
        add_toggle_button_at(
            document,
            "midi.channel_filter",
            app.midi_channel_filter_label(),
            UiRect::new(panel.right() - 104.0, panel.y + 210.0, 50.0, 26.0),
            app.midi_channel_filter().is_some(),
            true,
        );
        add_button_at(
            document,
            "midi.channel_filter_next",
            ">",
            UiRect::new(panel.right() - 48.0, panel.y + 210.0, 30.0, 26.0),
            false,
            true,
        );
    } else {
        add_toggle_button_at(
            document,
            "midi.channel_filter",
            app.midi_channel_filter_label(),
            UiRect::new(panel.right() - 82.0, panel.y + 210.0, 64.0, 26.0),
            app.midi_channel_filter().is_some(),
            true,
        );
    }
    add_label_at(
        document,
        "lumatone.map",
        lumatone_status,
        UiRect::new(panel.x + 16.0, panel.y + 236.0, panel.width - 114.0, 18.0),
        false,
    );
    let keymap_nav_enabled = !app.lumatone_presets.is_empty();
    add_button_at(
        document,
        "keymap.prev",
        "<",
        UiRect::new(panel.right() - 84.0, panel.y + 232.0, 30.0, 26.0),
        false,
        keymap_nav_enabled,
    );
    add_button_at(
        document,
        "keymap.next",
        ">",
        UiRect::new(panel.right() - 48.0, panel.y + 232.0, 30.0, 26.0),
        false,
        keymap_nav_enabled,
    );

    add_ui_scale_control(document, app, panel, panel.y + 266.0);

    let compact_device_buttons = panel.width < 300.0;
    let bottom_midi_buttons = device_control_rects(panel, panel.bottom() - 108.0);
    let bottom_audio_buttons = device_control_rects(panel, panel.bottom() - 50.0);
    let bottom_device_controls_fit = bottom_audio_buttons.connect.bottom() <= right_control_bottom;
    let compact_device_rows = if bottom_device_controls_fit {
        None
    } else {
        let audio_buttons = device_control_rects(panel, right_control_bottom - 26.0);
        let midi_buttons = device_control_rects(panel, audio_buttons.prev.y - 32.0);
        (midi_buttons.prev.y >= panel.y + 292.0).then_some((midi_buttons, audio_buttons))
    };
    let synth_control_bottom = compact_device_rows
        .map(|(midi_buttons, _)| midi_buttons.prev.y - 8.0)
        .unwrap_or(right_control_bottom);
    let synth_y = panel.y + 292.0;
    if synth_y + 22.0 <= synth_control_bottom {
        if let Some(assignment) = app.sample_instrument_assignment.as_ref() {
            add_label_at(
                document,
                "synth.sample_instrument.label",
                format!("Sample {}", compact_label(&assignment.name, 22)),
                UiRect::new(panel.x + 16.0, synth_y, panel.width - 110.0, 18.0),
                false,
            );
            add_button_at(
                document,
                "synth.clear_sample",
                "Clear",
                UiRect::new(panel.right() - 82.0, synth_y - 4.0, 64.0, 26.0),
                false,
                true,
            );
        } else if let Some(path) = app.missing_sample_instrument_path.as_ref() {
            add_label_at(
                document,
                "synth.sample_instrument.label",
                format!(
                    "Sample missing {}",
                    compact_label(&sample_source_name(path), 14)
                ),
                UiRect::new(panel.x + 16.0, synth_y, panel.width - 110.0, 18.0),
                false,
            );
            add_button_at(
                document,
                "synth.clear_sample",
                "Clear",
                UiRect::new(panel.right() - 82.0, synth_y - 4.0, 64.0, 26.0),
                false,
                true,
            );
        } else {
            add_label_at(
                document,
                "synth.waveform.label",
                format!("Wave {}", synth.waveform.as_str()),
                UiRect::new(panel.x + 16.0, synth_y, panel.width - 116.0, 18.0),
                false,
            );
            add_button_at(
                document,
                "synth.waveform_prev",
                "<",
                UiRect::new(panel.right() - 84.0, synth_y - 4.0, 30.0, 26.0),
                false,
                true,
            );
            add_button_at(
                document,
                "synth.waveform_next",
                ">",
                UiRect::new(panel.right() - 48.0, synth_y - 4.0, 30.0, 26.0),
                false,
                true,
            );
        }
    }
    if synth_y + 52.0 <= synth_control_bottom {
        add_synth_mute_control(document, synth_muted, synth_limited, panel, synth_y + 30.0);
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
            add_synth_control(document, key, label, value, panel, y);
        }
    }

    if bottom_device_controls_fit {
        add_label_at(
            document,
            "midi.selected",
            selected_midi,
            UiRect::new(
                panel.x + 12.0,
                panel.bottom() - 134.0,
                panel.width - 24.0,
                18.0,
            ),
            false,
        );
        add_device_button_row(
            document,
            app,
            compact_device_buttons,
            bottom_midi_buttons,
            DeviceKind::Midi,
        );
        add_label_at(
            document,
            "audio.selected",
            selected_audio,
            UiRect::new(
                panel.x + 12.0,
                panel.bottom() - 76.0,
                panel.width - 24.0,
                18.0,
            ),
            false,
        );
        add_device_button_row(
            document,
            app,
            compact_device_buttons,
            bottom_audio_buttons,
            DeviceKind::Audio,
        );
    } else if let Some((midi_buttons, audio_buttons)) = compact_device_rows {
        add_device_button_row(
            document,
            app,
            compact_device_buttons,
            midi_buttons,
            DeviceKind::Midi,
        );
        add_device_button_row(
            document,
            app,
            compact_device_buttons,
            audio_buttons,
            DeviceKind::Audio,
        );
    }
}

fn add_root_midi_input_at(document: &mut UiDocument, app: &AppState, rect: UiRect) {
    add_control_text_input_at(
        document,
        "scale.root_input",
        app.root_midi_edit_text(),
        "Root",
        "Root note",
        rect,
    );
}

fn add_base_freq_input_at(document: &mut UiDocument, app: &AppState, rect: UiRect) {
    add_control_text_input_at(
        document,
        "scale.base_input",
        app.base_freq_edit_text(),
        "Hz",
        "Base frequency Hz",
        rect,
    );
}

fn add_control_text_input_at(
    document: &mut UiDocument,
    name: &'static str,
    value: String,
    placeholder: &'static str,
    accessibility_label: &'static str,
    rect: UiRect,
) {
    let state = widgets::TextInputState::new(value);
    let mut options = widgets::TextInputOptions::default()
        .with_layout(layout::absolute(rect.x, rect.y, rect.width, rect.height))
        .with_placeholder(placeholder)
        .with_edit_action(WidgetActionBinding::action(name));
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
    options.accessibility_label = Some(accessibility_label.to_string());
    widgets::singleline_text_input(document, document.root, name, &state, options);
}

fn sample_source_name(path: &std::path::Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("sample")
        .to_string()
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CaptureControlRects {
    pub(super) capture: UiRect,
    pub(super) stop: UiRect,
    pub(super) clear: UiRect,
    pub(super) maps: UiRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CaptureActionState {
    pub(super) start_enabled: bool,
    pub(super) stop_enabled: bool,
    pub(super) clear_enabled: bool,
}

pub(super) fn capture_action_state(
    capture_armed: bool,
    capture_count: usize,
) -> CaptureActionState {
    CaptureActionState {
        start_enabled: !capture_armed,
        stop_enabled: capture_armed,
        clear_enabled: capture_count > 0,
    }
}

pub(super) fn capture_control_rects(panel: UiRect, y: f32) -> CaptureControlRects {
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
        UiRect::new(panel.x + 16.0, y, panel.width - 152.0, 18.0),
        false,
    );
    add_button_at(
        document,
        "synth.reset",
        "Reset",
        UiRect::new(panel.right() - 124.0, y - 4.0, 52.0, 26.0),
        false,
        true,
    );
    add_toggle_button_at(
        document,
        "synth.mute",
        if muted { "Muted" } else { "Mute" },
        UiRect::new(panel.right() - 66.0, y - 4.0, 48.0, 26.0),
        muted,
        true,
    );
}

fn add_ui_scale_control(document: &mut UiDocument, app: &AppState, panel: UiRect, y: f32) {
    let gap = 6.0;
    let down_w = 28.0;
    let reset_w = 44.0;
    let up_w = 28.0;
    let layout_w = 54.0;
    let total_w = down_w + reset_w + up_w + layout_w + gap * 3.0;
    let button_x = panel.right() - 16.0 - total_w;
    if button_x >= panel.x + 88.0 {
        add_label_at(
            document,
            "ui.scale.label",
            format!("Zoom {:.0}%", app.ui_scale() * 100.0),
            UiRect::new(panel.x + 16.0, y, button_x - panel.x - 24.0, 18.0),
            false,
        );
    }
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
        "100%",
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
    add_button_at_with_visible_label(
        document,
        "view.reset_layout",
        "Reset layout",
        Some("Layout"),
        UiRect::new(
            button_x + down_w + reset_w + up_w + gap * 3.0,
            y - 4.0,
            layout_w,
            26.0,
        ),
        false,
        true,
    );
}

#[derive(Clone, Copy)]
enum DeviceKind {
    Midi,
    Audio,
}

fn add_device_button_row(
    document: &mut UiDocument,
    app: &AppState,
    compact: bool,
    row: super::devices::DeviceControlRects,
    kind: DeviceKind,
) {
    let (prefix, refresh, connect, selectable_count) = match kind {
        DeviceKind::Midi => (
            "midi",
            if compact { "MIDI" } else { "Refresh MIDI" },
            midi_connect_label(app, compact),
            app.midi_inputs.len(),
        ),
        DeviceKind::Audio => (
            "audio",
            if compact { "Audio" } else { "Refresh Audio" },
            audio_connect_label(app, compact),
            app.audio_outputs.len(),
        ),
    };

    add_button_at(
        document,
        format!("{prefix}.prev"),
        "<",
        row.prev,
        false,
        selectable_count > 1,
    );
    add_button_at(
        document,
        format!("{prefix}.next"),
        ">",
        row.next,
        false,
        selectable_count > 1,
    );
    add_button_at(
        document,
        format!("{prefix}.refresh"),
        refresh,
        row.refresh,
        false,
        true,
    );
    add_button_at(
        document,
        format!("{prefix}.connect"),
        connect,
        row.connect,
        false,
        selectable_count > 0,
    );
}
