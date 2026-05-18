use operad::{UiDocument, UiRect};

use crate::app::AppState;
use crate::audio::AudioOutputDevice;
use crate::ui::labels::{
    audio_connect_label, audio_output_diagnostic_label, midi_connect_label,
    midi_input_diagnostic_label, selected_audio_output_name, selected_midi_input_name,
};

use super::controls::{add_button_at, add_label_at, add_selectable_at, add_toggle_button_at};
use super::presenters::visible_list_start;

#[derive(Clone, Copy, Debug)]
pub(super) struct DeviceControlRects {
    pub(super) prev: UiRect,
    pub(super) next: UiRect,
    pub(super) refresh: UiRect,
    pub(super) connect: UiRect,
}

struct DevicePickerHeading<'a> {
    name: &'a str,
    label: &'a str,
    prev_action: &'a str,
    next_action: &'a str,
    nav_enabled: bool,
}

pub(super) fn device_control_rects(panel: UiRect, y: f32) -> DeviceControlRects {
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

pub(super) fn add_right_panel_mode_control(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
) {
    add_toggle_button_at(
        document,
        "view.settings",
        "Settings",
        UiRect::new(panel.right() - 154.0, panel.y + 8.0, 66.0, 26.0),
        app.show_settings_panel,
        true,
    );

    let label = if app.device_setup_required() {
        "Setup"
    } else {
        "Devices"
    };
    add_toggle_button_at(
        document,
        "view.devices",
        label,
        UiRect::new(panel.right() - 82.0, panel.y + 8.0, 64.0, 26.0),
        app.show_device_panel,
        true,
    );
}

pub(super) fn add_device_panel_controls(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    bottom: f32,
) {
    let bottom = bottom.min(panel.bottom() - 12.0);
    let setup_messages = app.device_setup_messages();
    let picker_rows = if bottom - panel.y >= 360.0 { 3 } else { 2 };
    let mut y = panel.y + 44.0;
    if !setup_messages.is_empty() {
        add_label_at(
            document,
            "device.setup.heading",
            "SETUP REQUIRED",
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            true,
        );
        y += 20.0;
        add_label_at(
            document,
            "device.setup.summary",
            device_setup_panel_label(&setup_messages),
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            false,
        );
        y += 18.0;
        y += 4.0;
        y = add_compact_midi_setup_section(document, app, panel, y);
        y += 8.0;
        y = add_compact_audio_setup_section(document, app, panel, y);
        y += 8.0;
        add_latest_device_diagnostic(document, app, panel, y, bottom);
        return;
    }
    y = add_midi_picker_section(document, app, panel, y, picker_rows);
    y += 12.0;
    if y + 124.0 <= bottom {
        add_audio_picker_section(document, app, panel, y, picker_rows);
    }
}

fn add_compact_midi_setup_section(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    mut y: f32,
) -> f32 {
    add_device_picker_heading(
        document,
        DevicePickerHeading {
            name: "device.midi.heading",
            label: "MIDI INPUTS",
            prev_action: "midi.prev",
            next_action: "midi.next",
            nav_enabled: app.midi_inputs.len() > 1,
        },
        panel,
        y,
    );
    y += 22.0;
    if app.midi_inputs.is_empty() {
        add_label_at(
            document,
            "device.midi.empty",
            "No MIDI inputs",
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            false,
        );
        y += 22.0;
    } else {
        let start = visible_list_start(app.selected_input, app.midi_inputs.len(), 1);
        for (idx, name) in app.midi_inputs.iter().enumerate().skip(start).take(1) {
            add_selectable_at(
                document,
                format!("midi.select.{idx}"),
                name,
                UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 24.0),
                idx == app.selected_input,
                true,
            );
            y += 26.0;
        }
    }
    add_label_at(
        document,
        "device.midi.diagnostic",
        midi_input_diagnostic_label(app),
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 20.0;
    add_browser_device_diagnostic(
        document,
        "device.midi.browser_diagnostic",
        app.browser_midi_diagnostic_label(),
        panel,
        &mut y,
    );
    add_device_action_buttons(
        document,
        "midi.refresh",
        "midi.connect",
        midi_connect_label(app, true),
        panel,
        y + 2.0,
        !app.midi_inputs.is_empty(),
    );
    y + 32.0
}

fn add_compact_audio_setup_section(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    mut y: f32,
) -> f32 {
    add_device_picker_heading(
        document,
        DevicePickerHeading {
            name: "device.audio.heading",
            label: "AUDIO OUTPUTS",
            prev_action: "audio.prev",
            next_action: "audio.next",
            nav_enabled: app.audio_outputs.len() > 1,
        },
        panel,
        y,
    );
    y += 22.0;
    if app.audio_outputs.is_empty() {
        add_label_at(
            document,
            "device.audio.empty",
            "No audio outputs",
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            false,
        );
        y += 22.0;
    } else {
        let start = visible_list_start(app.selected_audio_output, app.audio_outputs.len(), 1);
        for (idx, output) in app.audio_outputs.iter().enumerate().skip(start).take(1) {
            add_selectable_at(
                document,
                format!("audio.select.{idx}"),
                audio_output_picker_label(output),
                UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 24.0),
                idx == app.selected_audio_output,
                true,
            );
            y += 26.0;
        }
    }
    add_label_at(
        document,
        "device.audio.diagnostic",
        audio_output_diagnostic_label(app),
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 20.0;
    add_browser_device_diagnostic(
        document,
        "device.audio.browser_diagnostic",
        app.browser_audio_diagnostic_label(),
        panel,
        &mut y,
    );
    add_device_action_buttons(
        document,
        "audio.refresh",
        "audio.connect",
        audio_connect_label(app, true),
        panel,
        y + 2.0,
        !app.audio_outputs.is_empty(),
    );
    y + 32.0
}

fn add_latest_device_diagnostic(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    y: f32,
    bottom: f32,
) {
    let Some(latest) = app.diagnostic_messages().last() else {
        return;
    };
    if y + 40.0 <= bottom {
        add_label_at(
            document,
            "device.diagnostics.heading",
            "DIAGNOSTICS",
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            false,
        );
        add_label_at(
            document,
            "device.diagnostics.latest",
            latest,
            UiRect::new(panel.x + 16.0, y + 20.0, panel.width - 32.0, 18.0),
            true,
        );
    } else if y + 18.0 <= bottom {
        add_label_at(
            document,
            "device.diagnostics.latest",
            format!("Last: {latest}"),
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            true,
        );
    }
}

fn device_setup_panel_label(messages: &[&str]) -> String {
    match messages {
        ["audio unavailable", "MIDI unavailable"] => "Audio/MIDI unavailable".to_string(),
        ["audio not connected", "MIDI not connected"] => "Audio/MIDI not connected".to_string(),
        _ => messages.join("; "),
    }
}

fn add_midi_picker_section(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    mut y: f32,
    rows: usize,
) -> f32 {
    add_device_picker_heading(
        document,
        DevicePickerHeading {
            name: "device.midi.heading",
            label: "MIDI INPUTS",
            prev_action: "midi.prev",
            next_action: "midi.next",
            nav_enabled: app.midi_inputs.len() > rows,
        },
        panel,
        y,
    );
    y += 22.0;
    add_label_at(
        document,
        "device.midi.status",
        selected_midi_input_name(app),
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 20.0;
    add_label_at(
        document,
        "device.midi.diagnostic",
        midi_input_diagnostic_label(app),
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 20.0;
    add_browser_device_diagnostic(
        document,
        "device.midi.browser_diagnostic",
        app.browser_midi_diagnostic_label(),
        panel,
        &mut y,
    );
    y += 4.0;
    if app.midi_inputs.is_empty() {
        add_label_at(
            document,
            "device.midi.empty",
            "No MIDI inputs",
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            false,
        );
        y += 26.0;
    } else {
        let start = visible_list_start(app.selected_input, app.midi_inputs.len(), rows);
        for (idx, name) in app.midi_inputs.iter().enumerate().skip(start).take(rows) {
            add_selectable_at(
                document,
                format!("midi.select.{idx}"),
                name,
                UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 24.0),
                idx == app.selected_input,
                true,
            );
            y += 26.0;
        }
    }
    add_device_action_buttons(
        document,
        "midi.refresh",
        "midi.connect",
        midi_connect_label(app, true),
        panel,
        y + 4.0,
        !app.midi_inputs.is_empty(),
    );
    y + 34.0
}

fn add_audio_picker_section(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    mut y: f32,
    rows: usize,
) {
    add_device_picker_heading(
        document,
        DevicePickerHeading {
            name: "device.audio.heading",
            label: "AUDIO OUTPUTS",
            prev_action: "audio.prev",
            next_action: "audio.next",
            nav_enabled: app.audio_outputs.len() > rows,
        },
        panel,
        y,
    );
    y += 22.0;
    add_label_at(
        document,
        "device.audio.status",
        selected_audio_output_name(app),
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 20.0;
    add_label_at(
        document,
        "device.audio.diagnostic",
        audio_output_diagnostic_label(app),
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 20.0;
    add_browser_device_diagnostic(
        document,
        "device.audio.browser_diagnostic",
        app.browser_audio_diagnostic_label(),
        panel,
        &mut y,
    );
    y += 4.0;
    if app.audio_outputs.is_empty() {
        add_label_at(
            document,
            "device.audio.empty",
            "No audio outputs",
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            false,
        );
        y += 26.0;
    } else {
        let start = visible_list_start(app.selected_audio_output, app.audio_outputs.len(), rows);
        for (idx, output) in app.audio_outputs.iter().enumerate().skip(start).take(rows) {
            add_selectable_at(
                document,
                format!("audio.select.{idx}"),
                audio_output_picker_label(output),
                UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 24.0),
                idx == app.selected_audio_output,
                true,
            );
            y += 26.0;
        }
    }
    add_device_action_buttons(
        document,
        "audio.refresh",
        "audio.connect",
        audio_connect_label(app, true),
        panel,
        y + 4.0,
        !app.audio_outputs.is_empty(),
    );
}

fn add_browser_device_diagnostic(
    document: &mut UiDocument,
    name: &str,
    diagnostic: Option<&str>,
    panel: UiRect,
    y: &mut f32,
) {
    let Some(diagnostic) = diagnostic else {
        return;
    };
    add_label_at(
        document,
        name,
        diagnostic,
        UiRect::new(panel.x + 16.0, *y, panel.width - 32.0, 18.0),
        false,
    );
    *y += 20.0;
}

fn add_device_picker_heading(
    document: &mut UiDocument,
    heading: DevicePickerHeading<'_>,
    panel: UiRect,
    y: f32,
) {
    let label_w = if heading.nav_enabled {
        panel.width - 116.0
    } else {
        panel.width - 32.0
    };
    add_label_at(
        document,
        heading.name,
        heading.label,
        UiRect::new(panel.x + 16.0, y, label_w, 18.0),
        false,
    );
    if heading.nav_enabled {
        add_button_at(
            document,
            heading.prev_action,
            "<",
            UiRect::new(panel.right() - 84.0, y - 4.0, 30.0, 26.0),
            false,
            true,
        );
        add_button_at(
            document,
            heading.next_action,
            ">",
            UiRect::new(panel.right() - 48.0, y - 4.0, 30.0, 26.0),
            false,
            true,
        );
    }
}

fn add_device_action_buttons(
    document: &mut UiDocument,
    refresh_action: &str,
    connect_action: &str,
    connect_label: &str,
    panel: UiRect,
    y: f32,
    connect_enabled: bool,
) {
    let gap = 6.0;
    let button_w = ((panel.width - 32.0 - gap) / 2.0).max(48.0);
    add_button_at(
        document,
        refresh_action,
        "Refresh",
        UiRect::new(panel.x + 16.0, y, button_w, 26.0),
        false,
        true,
    );
    add_button_at(
        document,
        connect_action,
        connect_label,
        UiRect::new(panel.x + 16.0 + button_w + gap, y, button_w, 26.0),
        false,
        connect_enabled,
    );
}

fn audio_output_picker_label(output: &AudioOutputDevice) -> String {
    if output.is_default && !output.name.eq_ignore_ascii_case("default") {
        format!("{} default", output.name)
    } else {
        output.name.clone()
    }
}
