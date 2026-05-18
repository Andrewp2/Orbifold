use operad::{FontFamily, TextStyle, UiDocument, UiRect, layout, widgets};

use crate::app::AppState;
use crate::ui::text::{estimated_text_width, fit_label};
use crate::ui::theme::{muted, strong};

use super::MIN_POINTER_TARGET_SIZE;
use super::controls::{add_button_at_with_visible_label, add_label_at, add_toggle_button_at};
use super::presenters::clip_panel_summary;

#[derive(Clone, Debug)]
struct PianoPanelButtonSpec {
    name: &'static str,
    label: String,
    rect: UiRect,
    active: bool,
    enabled: bool,
}

pub(super) fn add_piano_roll_option_controls(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
) {
    let x = panel.x + 10.0;
    let width = (panel.width - 20.0).max(1.0);

    add_label_at(
        document,
        "piano.panel.notes_header",
        "NOTES",
        UiRect::new(x, panel.y + 8.0, width, 14.0),
        false,
    );
    for button in piano_panel_button_specs(app, panel) {
        add_button_at_with_visible_label(
            document,
            button.name,
            button.label,
            Some(""),
            button.rect,
            button.active,
            button.enabled,
        );
    }
}

pub(super) fn add_piano_panel_button_labels(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
) {
    for button in piano_panel_button_specs(app, panel) {
        let color = if button.enabled { strong() } else { muted() };
        let text = fit_label(&button.label, button.rect.width - 8.0, 12.0);
        if text.is_empty() {
            continue;
        }
        let text_w = estimated_text_width(&text, 12.0).min(button.rect.width - 8.0);
        let label_rect = UiRect::new(
            button.rect.x + ((button.rect.width - text_w) * 0.5).max(4.0),
            button.rect.y + ((button.rect.height - 16.0) * 0.5).max(0.0),
            text_w.max(1.0),
            16.0,
        );
        widgets::label(
            document,
            document.root,
            format!("{}.overlay_label", button.name),
            text,
            TextStyle {
                font_size: 12.0,
                line_height: 16.0,
                family: FontFamily::SansSerif,
                color,
                ..Default::default()
            },
            layout::absolute(
                label_rect.x,
                label_rect.y,
                label_rect.width,
                label_rect.height,
            ),
        );
    }
}

pub(super) fn add_clip_panel_controls(
    document: &mut UiDocument,
    app: &AppState,
    panel: UiRect,
    quantize_on_record: bool,
) {
    add_clip_panel_labels(document, app, panel);
    add_recording_options_control(document, panel, quantize_on_record);
}

fn piano_panel_button_specs(app: &AppState, panel: UiRect) -> Vec<PianoPanelButtonSpec> {
    let (has_notes, quantize_grid) = {
        let project = app.music_project.lock();
        (
            !project.clip.notes.is_empty(),
            project.transport.quantize_grid,
        )
    };
    let selected_note = app.selected_clip_note().is_some();
    let gap = 2.0;
    let section_gap = 6.0;
    let row_h = 24.0;
    let x = panel.x + 10.0;
    let width = (panel.width - 20.0).max(1.0);
    let half_w = ((width - gap) * 0.5).max(1.0);
    let third_w = ((width - gap * 2.0) / 3.0).max(1.0);
    let mut y = panel.y + 26.0;
    let mut buttons = vec![
        PianoPanelButtonSpec {
            name: "piano.view.scales",
            label: "Scale".to_string(),
            rect: UiRect::new(x, y, third_w, row_h),
            active: app.show_scale_browser,
            enabled: true,
        },
        PianoPanelButtonSpec {
            name: "piano.view.clip",
            label: "Clip".to_string(),
            rect: UiRect::new(x + third_w + gap, y, third_w, row_h),
            active: app.show_clip_panel,
            enabled: true,
        },
        PianoPanelButtonSpec {
            name: "piano.pitch_labels",
            label: if app.piano_pitch_labels_show_degrees() {
                "Deg".to_string()
            } else {
                "Note".to_string()
            },
            rect: UiRect::new(x + (third_w + gap) * 2.0, y, third_w, row_h),
            active: app.piano_pitch_labels_show_degrees(),
            enabled: true,
        },
    ];
    y += row_h + gap;

    let grid_arrow_w = MIN_POINTER_TARGET_SIZE;
    let snap_w =
        ((width - gap * 3.0 - grid_arrow_w * 2.0) * 0.36).clamp(MIN_POINTER_TARGET_SIZE, 60.0);
    let grid_w = (width - snap_w - grid_arrow_w * 2.0 - gap * 3.0).max(MIN_POINTER_TARGET_SIZE);
    buttons.extend([
        PianoPanelButtonSpec {
            name: "piano.transport.snap",
            label: "Snap".to_string(),
            rect: UiRect::new(x, y, snap_w, row_h),
            active: quantize_grid.step_beats().is_some(),
            enabled: true,
        },
        PianoPanelButtonSpec {
            name: "piano.transport.quantize_grid_prev",
            label: "<".to_string(),
            rect: UiRect::new(x + snap_w + gap, y, grid_arrow_w, row_h),
            active: false,
            enabled: true,
        },
        PianoPanelButtonSpec {
            name: "piano.transport.quantize_grid",
            label: format!("Q{}", quantize_grid.as_str()),
            rect: UiRect::new(x + snap_w + gap + grid_arrow_w + gap, y, grid_w, row_h),
            active: false,
            enabled: true,
        },
        PianoPanelButtonSpec {
            name: "piano.transport.quantize_grid_next",
            label: ">".to_string(),
            rect: UiRect::new(
                x + snap_w + gap + grid_arrow_w + gap + grid_w + gap,
                y,
                grid_arrow_w,
                row_h,
            ),
            active: false,
            enabled: true,
        },
    ]);
    y += row_h + gap;

    buttons.extend([
        PianoPanelButtonSpec {
            name: "piano.zoom_out",
            label: "Time -".to_string(),
            rect: UiRect::new(x, y, third_w, row_h),
            active: false,
            enabled: true,
        },
        PianoPanelButtonSpec {
            name: "piano.fit_view",
            label: "Fit".to_string(),
            rect: UiRect::new(x + third_w + gap, y, third_w, row_h),
            active: false,
            enabled: true,
        },
        PianoPanelButtonSpec {
            name: "piano.zoom_in",
            label: "Time +".to_string(),
            rect: UiRect::new(x + (third_w + gap) * 2.0, y, third_w, row_h),
            active: false,
            enabled: true,
        },
    ]);
    y += row_h + gap;

    buttons.push(PianoPanelButtonSpec {
        name: "piano.pitch_zoom_out",
        label: "Rows -".to_string(),
        rect: UiRect::new(x, y, half_w, row_h),
        active: false,
        enabled: true,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "piano.pitch_zoom_in",
        label: "Rows +".to_string(),
        rect: UiRect::new(x + half_w + gap, y, half_w, row_h),
        active: false,
        enabled: true,
    });
    y += row_h + gap + section_gap;

    buttons.push(PianoPanelButtonSpec {
        name: "clip.add_note",
        label: "Add".to_string(),
        rect: UiRect::new(x, y, half_w, row_h),
        active: false,
        enabled: true,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.delete_note",
        label: "Delete".to_string(),
        rect: UiRect::new(x + half_w + gap, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    y += row_h + gap;

    let paste_w = 48.0_f32.min((width * 0.34).max(MIN_POINTER_TARGET_SIZE));
    buttons.push(PianoPanelButtonSpec {
        name: "clip.duplicate_note",
        label: "Duplicate".to_string(),
        rect: UiRect::new(
            x,
            y,
            (width - paste_w - gap).max(MIN_POINTER_TARGET_SIZE),
            row_h,
        ),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.paste_note",
        label: "Paste".to_string(),
        rect: UiRect::new(x + width - paste_w, y, paste_w, row_h),
        active: false,
        enabled: app.can_paste_clip_note(),
    });
    y += row_h + gap;

    let nudge_w = 28.0;
    buttons.push(PianoPanelButtonSpec {
        name: "clip.nudge_left",
        label: "<".to_string(),
        rect: UiRect::new(x, y, nudge_w, row_h),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.nudge_right",
        label: ">".to_string(),
        rect: UiRect::new(x + nudge_w + gap, y, nudge_w, row_h),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.quantize",
        label: "Quantize".to_string(),
        rect: UiRect::new(
            x + (nudge_w + gap) * 2.0,
            y,
            (width - (nudge_w + gap) * 2.0).max(MIN_POINTER_TARGET_SIZE),
            row_h,
        ),
        active: false,
        enabled: has_notes,
    });
    y += row_h + gap;

    buttons.push(PianoPanelButtonSpec {
        name: "clip.pitch_down",
        label: "Pitch -".to_string(),
        rect: UiRect::new(x, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.pitch_up",
        label: "Pitch +".to_string(),
        rect: UiRect::new(x + half_w + gap, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    y += row_h + gap;

    buttons.push(PianoPanelButtonSpec {
        name: "clip.shorter",
        label: "Len -".to_string(),
        rect: UiRect::new(x, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.longer",
        label: "Len +".to_string(),
        rect: UiRect::new(x + half_w + gap, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    y += row_h + gap;

    buttons.push(PianoPanelButtonSpec {
        name: "clip.velocity_down",
        label: "Vel -".to_string(),
        rect: UiRect::new(x, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });
    buttons.push(PianoPanelButtonSpec {
        name: "clip.velocity_up",
        label: "Vel +".to_string(),
        rect: UiRect::new(x + half_w + gap, y, half_w, row_h),
        active: false,
        enabled: selected_note,
    });

    buttons
}

fn add_clip_panel_labels(document: &mut UiDocument, app: &AppState, panel: UiRect) {
    let clip = clip_panel_summary(app);
    let row = UiRect::new(panel.x + 10.0, panel.y + 44.0, panel.width - 20.0, 96.0);
    let text_w = (row.width - 60.0).max(44.0);
    add_label_at(
        document,
        "clip.title",
        fit_label("Current Clip", text_w, 12.0),
        UiRect::new(row.x + 52.0, row.y + 13.0, text_w, 18.0),
        true,
    );
    add_label_at(
        document,
        "clip.note_count",
        fit_label(&clip.note_count, text_w, 11.0),
        UiRect::new(row.x + 52.0, row.y + 32.0, text_w, 16.0),
        false,
    );
    add_label_at(
        document,
        "clip.loop_grid",
        fit_label(&clip.loop_and_grid, row.width - 20.0, 11.0),
        UiRect::new(row.x + 10.0, row.y + 56.0, row.width - 20.0, 16.0),
        false,
    );
    if let Some(selected) = clip.selected_note {
        add_label_at(
            document,
            "clip.selected_note",
            fit_label(&selected, row.width - 20.0, 10.0),
            UiRect::new(row.x + 10.0, row.y + 74.0, row.width - 20.0, 16.0),
            false,
        );
    }
}

fn add_recording_options_control(
    document: &mut UiDocument,
    panel: UiRect,
    quantize_on_record: bool,
) {
    let y = panel.y + 158.0;
    let button_w = 54.0;
    let button = UiRect::new(panel.right() - 10.0 - button_w, y - 4.0, button_w, 26.0);
    let label_w = (button.x - panel.x - 22.0).max(54.0);
    add_label_at(
        document,
        "record.quantize.label",
        fit_label("Rec quantize", label_w, 11.0),
        UiRect::new(panel.x + 12.0, y, label_w, 18.0),
        false,
    );
    add_toggle_button_at(
        document,
        "transport.record_quantize",
        if quantize_on_record { "On" } else { "Off" },
        button,
        quantize_on_record,
        true,
    );
}
