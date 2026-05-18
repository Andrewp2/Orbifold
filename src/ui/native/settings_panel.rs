use operad::{UiDocument, UiRect};

use crate::app::AppState;

use super::controls::{add_button_at, add_label_at, add_toggle_button_at};

pub(super) fn add_settings_panel_controls(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    bottom: f32,
) {
    let bottom = bottom.min(panel.bottom() - 12.0);
    let mut y = panel.y + 44.0;

    add_label_at(
        document,
        "settings.display.heading",
        "DISPLAY",
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 22.0;
    add_label_at(
        document,
        "settings.ui.scale.label",
        format!("UI zoom {:.0}%", app.ui_scale() * 100.0),
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 22.0;
    add_three_button_row(
        document,
        panel,
        y,
        [
            ("settings.ui.scale_down", "-"),
            ("settings.ui.scale_reset", "100%"),
            ("settings.ui.scale_up", "+"),
        ],
    );
    y += 34.0;

    add_label_at(
        document,
        "settings.workspace.heading",
        "WORKSPACE",
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 22.0;
    add_three_toggle_row(
        document,
        panel,
        y,
        [
            ("settings.view.assets", "Assets", app.show_asset_browser),
            ("settings.view.scales", "Scales", app.show_scale_browser),
            ("settings.view.clip", "Clip", app.show_clip_panel),
        ],
    );
    y += 34.0;
    add_button_at(
        document,
        "settings.view.reset_layout",
        "Reset Layout",
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 26.0),
        false,
        true,
    );
    y += 36.0;

    if !app.diagnostic_messages().is_empty() {
        y = add_diagnostics_section(document, app, panel, y, bottom);
        y += 4.0;
    }

    let setup_messages = app.device_setup_messages();
    let setup_block_h = if setup_messages.is_empty() {
        52.0
    } else {
        72.0
    };
    if y + setup_block_h <= bottom {
        add_label_at(
            document,
            "settings.setup.heading",
            "SETUP",
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            false,
        );
        y += 22.0;
        if !setup_messages.is_empty() {
            add_label_at(
                document,
                "settings.setup.summary",
                settings_setup_summary_label(&setup_messages),
                UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
                true,
            );
            y += 20.0;
        }
        add_button_at(
            document,
            "settings.view.devices",
            if app.device_setup_required() {
                "Open Setup"
            } else {
                "Open Devices"
            },
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 26.0),
            false,
            true,
        );
        y += 36.0;
    }

    if y + 52.0 <= bottom {
        add_label_at(
            document,
            "settings.persist.heading",
            "PREFERENCES",
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            false,
        );
        y += 22.0;
        add_button_at(
            document,
            "settings.panel.save",
            "Save Settings",
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 26.0),
            false,
            true,
        );
    }
}

fn add_diagnostics_section(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    mut y: f32,
    bottom: f32,
) -> f32 {
    let diagnostics = app.diagnostic_messages();
    let available = (bottom - y).max(0.0);
    let rows = if available >= 96.0 {
        2
    } else if available >= 76.0 {
        1
    } else {
        return y;
    };
    add_label_at(
        document,
        "settings.diagnostics.heading",
        "DIAGNOSTICS",
        UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
        false,
    );
    y += 22.0;
    for (idx, diagnostic) in diagnostics.iter().rev().take(rows).enumerate() {
        add_label_at(
            document,
            format!("settings.diagnostics.{idx}"),
            diagnostic,
            UiRect::new(panel.x + 16.0, y, panel.width - 32.0, 18.0),
            true,
        );
        y += 20.0;
    }
    if y + 30.0 <= bottom {
        add_button_at(
            document,
            "settings.diagnostics.clear",
            "Clear Diagnostics",
            UiRect::new(panel.x + 16.0, y + 2.0, panel.width - 32.0, 26.0),
            false,
            true,
        );
        y += 34.0;
    }
    y
}

fn settings_setup_summary_label(messages: &[&str]) -> String {
    match messages {
        ["audio unavailable", "MIDI unavailable"] => "Audio/MIDI unavailable".to_string(),
        ["audio not connected", "MIDI not connected"] => "Audio/MIDI not connected".to_string(),
        [single] => settings_setup_message_label(single).to_string(),
        _ => messages
            .iter()
            .map(|message| settings_setup_message_label(message))
            .collect::<Vec<_>>()
            .join("; "),
    }
}

fn settings_setup_message_label(message: &str) -> &'static str {
    match message {
        "audio unavailable" => "Audio unavailable",
        "audio not connected" => "Audio not connected",
        "MIDI unavailable" => "MIDI unavailable",
        "MIDI not connected" => "MIDI not connected",
        _ => "Setup needed",
    }
}

fn add_three_button_row(
    document: &mut UiDocument,
    panel: UiRect,
    y: f32,
    buttons: [(&'static str, &'static str); 3],
) {
    let gap = 6.0;
    let width = (panel.width - 32.0 - gap * 2.0) / 3.0;
    for (idx, (name, label)) in buttons.into_iter().enumerate() {
        add_button_at(
            document,
            name,
            label,
            UiRect::new(panel.x + 16.0 + idx as f32 * (width + gap), y, width, 26.0),
            false,
            true,
        );
    }
}

fn add_three_toggle_row(
    document: &mut UiDocument,
    panel: UiRect,
    y: f32,
    buttons: [(&'static str, &'static str, bool); 3],
) {
    let gap = 6.0;
    let width = (panel.width - 32.0 - gap * 2.0) / 3.0;
    for (idx, (name, label, selected)) in buttons.into_iter().enumerate() {
        add_toggle_button_at(
            document,
            name,
            label,
            UiRect::new(panel.x + 16.0 + idx as f32 * (width + gap), y, width, 26.0),
            selected,
            true,
        );
    }
}
