use eframe::egui;
use operad::{ScenePrimitive, UiPoint};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::app::{AppState, AudioAssetKind};
use crate::midi::{LumatoneMap, MidiEvent};
use crate::operad_bridge;
use crate::operad_ui;
use crate::project::{ClipNote, QuantizeGrid};
use crate::scale::ScaleState;
use crate::synth::Waveform;

const LUMATONE_BOARDS: usize = 5;
const LUMATONE_KEYS_PER_BOARD: usize = 56;
const LUMATONE_ROW_STARTS: [usize; 11] = [0, 2, 7, 13, 19, 25, 31, 37, 43, 49, 54];
const LUMATONE_ROW_COUNTS: [usize; 11] = [2, 5, 6, 6, 6, 6, 6, 6, 6, 5, 2];
const LUMATONE_BOARD_COLUMNS: f32 = 6.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LumatoneGridPosition {
    board: usize,
    row: usize,
    col: usize,
}

#[derive(Clone, Copy, Debug)]
struct PianoRollLayout {
    time_rect: egui::Rect,
    keyboard_rect: egui::Rect,
    min_pitch: i32,
    max_pitch: i32,
    loop_beats: f32,
    row_height: f32,
}

impl PianoRollLayout {
    fn y_for_pitch(self, pitch: i32) -> f32 {
        self.time_rect.top() + (self.max_pitch - pitch) as f32 * self.row_height
    }

    fn pitch_at(self, y: f32) -> i32 {
        let row = ((y - self.time_rect.top()) / self.row_height).floor() as i32;
        (self.max_pitch - row).clamp(self.min_pitch, self.max_pitch)
    }

    fn beat_at(self, x: f32) -> f32 {
        let norm = ((x - self.time_rect.left()) / self.time_rect.width()).clamp(0.0, 1.0);
        norm * self.loop_beats.max(1.0)
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_music_playback();
        handle_screenshot_events(ctx, self);
        handle_keyboard_shortcuts(ctx, self);
        draw_menu_bar(ctx, self);
        draw_toolbar(ctx, self);
        draw_status_bar(ctx, self);

        if self.show_scale_library {
            egui::SidePanel::left("scale_library_panel")
                .resizable(true)
                .default_width(260.0)
                .width_range(220.0..=420.0)
                .show(ctx, |ui| draw_scale_library_panel(ui, self));
        }

        if self.show_inspector {
            egui::SidePanel::right("inspector_panel")
                .resizable(true)
                .default_width(300.0)
                .width_range(240.0..=460.0)
                .show(ctx, |ui| draw_inspector_panel(ui, self));
        }

        draw_clip_panel(ctx, self);
        egui::CentralPanel::default().show(ctx, |ui| draw_workspace(ui, self));
        if self.screenshot_on_start && !self.screenshot_requested {
            request_screenshot(ctx, self, true);
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

fn draw_menu_bar(ctx: &egui::Context, app: &mut AppState) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open Project...").clicked() {
                    open_project_file(app);
                    ui.close();
                }
                if ui.button("Save Project").clicked() {
                    if !app.save_project() {
                        save_project_as(app);
                    }
                    ui.close();
                }
                if ui.button("Save Project As...").clicked() {
                    save_project_as(app);
                    ui.close();
                }
                ui.separator();
                if ui.button("Open Scala File...").clicked() {
                    open_scala_file(app, true);
                    ui.close();
                }
                if ui.button("Take Screenshot").clicked() {
                    request_screenshot(ctx, app, false);
                    ui.close();
                }
                if ui.button("Save Settings").clicked() {
                    app.persist_settings_with_status();
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(app.can_undo_project_edit(), egui::Button::new("Undo"))
                    .clicked()
                {
                    app.undo_project_edit();
                    ui.close();
                }
                if ui
                    .add_enabled(app.can_redo_project_edit(), egui::Button::new("Redo"))
                    .clicked()
                {
                    app.redo_project_edit();
                    ui.close();
                }
                ui.separator();
                let has_selection = app.selected_clip_note.is_some();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Delete Note"))
                    .clicked()
                {
                    app.delete_selected_clip_note();
                    ui.close();
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("Duplicate Note"))
                    .clicked()
                {
                    app.duplicate_selected_clip_note();
                    ui.close();
                }
            });

            ui.menu_button("Options", |ui| {
                ui.menu_button("Audio Output", |ui| draw_audio_menu(ui, app));
                ui.menu_button("MIDI Input", |ui| draw_midi_menu(ui, app));
                ui.menu_button("Scale Tuning", |ui| draw_tuning_menu(ui, app));
                ui.menu_button("Synth", |ui| draw_synth_menu(ui, app));
                ui.menu_button("Key Map", |ui| draw_lumatone_menu(ui, app));
                ui.separator();
                let mut midi_debug = app.midi_debug.load(std::sync::atomic::Ordering::Relaxed);
                if ui
                    .checkbox(&mut midi_debug, "Log MIDI to console")
                    .changed()
                {
                    app.midi_debug
                        .store(midi_debug, std::sync::atomic::Ordering::Relaxed);
                    app.persist_current_settings();
                }
            });

            ui.menu_button("View", |ui| {
                ui.checkbox(&mut app.show_scale_library, "Browser");
                ui.checkbox(&mut app.show_key_labels, "Key Labels");
            });
        });
    });
}

fn draw_toolbar(ctx: &egui::Context, app: &mut AppState) {
    egui::TopBottomPanel::top("toolbar")
        .exact_height(40.0)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), ui.available_height()),
                egui::Sense::click(),
            );
            let mut document = operad_ui::document(rect.width(), rect.height());
            let row = operad_ui::panel_row(&mut document, "toolbar.row");
            operad_ui::button(
                &mut document,
                row,
                "toolbar.test_a4",
                "Test A4",
                74.0,
                false,
                true,
            );
            operad_ui::spacer(&mut document, row, "toolbar.space.1", 6.0);
            operad_ui::button(
                &mut document,
                row,
                "toolbar.all_notes_off",
                "All Notes Off",
                112.0,
                false,
                true,
            );
            operad_ui::spacer(&mut document, row, "toolbar.space.2", 10.0);
            operad_ui::divider(&mut document, row, "toolbar.divider.1");
            operad_ui::spacer(&mut document, row, "toolbar.space.3", 10.0);
            operad_ui::label(
                &mut document,
                row,
                "toolbar.audio",
                format!("Audio: {}", app.connected_audio_output),
                260.0,
                false,
            );
            operad_ui::spacer(&mut document, row, "toolbar.space.4", 10.0);
            operad_ui::divider(&mut document, row, "toolbar.divider.2");
            operad_ui::spacer(&mut document, row, "toolbar.space.5", 10.0);
            operad_ui::mono_label(
                &mut document,
                row,
                "toolbar.voices",
                format!("Voices: {}", app.synth.active_voice_count()),
                96.0,
            );
            if operad_bridge::compute_and_paint_document(ui, rect, &mut document)
                && let Some(clicked) = operad_bridge::clicked_node_name(&response, rect, &document)
            {
                match clicked.as_str() {
                    "toolbar.test_a4" => app.test_tone(),
                    "toolbar.all_notes_off" => app.all_notes_off(),
                    _ => {}
                }
            }
        });
}

fn draw_status_bar(ctx: &egui::Context, app: &AppState) {
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(28.0)
        .show(ctx, |ui| {
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), ui.available_height()),
                egui::Sense::hover(),
            );
            let mut document = operad_ui::document(rect.width(), rect.height());
            let row = operad_ui::panel_row(&mut document, "status.row");
            operad_ui::label(
                &mut document,
                row,
                "status.message",
                format!("Status: {}", app.last_status),
                rect.width().max(1.0) - 16.0,
                false,
            );
            operad_bridge::compute_and_paint_document(ui, rect, &mut document);
        });
}

fn handle_keyboard_shortcuts(ctx: &egui::Context, app: &mut AppState) {
    if ctx.wants_keyboard_input() {
        return;
    }

    let (
        save_project,
        open_project,
        undo,
        redo,
        play_stop,
        record,
        quantize,
        delete_note,
        duplicate_note,
        nudge_left,
        nudge_right,
        pitch_down,
        pitch_up,
        resize_shorter,
        resize_longer,
    ) = ctx.input(|input| {
        let modifiers = input.modifiers;
        let command = modifiers.command;
        let no_modifiers =
            !modifiers.command && !modifiers.ctrl && !modifiers.alt && !modifiers.shift;
        (
            command && input.key_pressed(egui::Key::S),
            command && input.key_pressed(egui::Key::O),
            command && !modifiers.shift && input.key_pressed(egui::Key::Z),
            command
                && (input.key_pressed(egui::Key::Y)
                    || (modifiers.shift && input.key_pressed(egui::Key::Z))),
            no_modifiers && input.key_pressed(egui::Key::Space),
            no_modifiers && input.key_pressed(egui::Key::R),
            no_modifiers && input.key_pressed(egui::Key::Q),
            no_modifiers
                && (input.key_pressed(egui::Key::Delete)
                    || input.key_pressed(egui::Key::Backspace)),
            no_modifiers && input.key_pressed(egui::Key::D),
            no_modifiers && input.key_pressed(egui::Key::ArrowLeft),
            no_modifiers && input.key_pressed(egui::Key::ArrowRight),
            no_modifiers && input.key_pressed(egui::Key::ArrowDown),
            no_modifiers && input.key_pressed(egui::Key::ArrowUp),
            modifiers.shift && input.key_pressed(egui::Key::ArrowLeft),
            modifiers.shift && input.key_pressed(egui::Key::ArrowRight),
        )
    });

    if save_project {
        if !app.save_project() {
            save_project_as(app);
        }
    } else if open_project {
        open_project_file(app);
    } else if undo {
        app.undo_project_edit();
    } else if redo {
        app.redo_project_edit();
    } else if play_stop {
        app.toggle_transport();
    } else if record {
        app.toggle_recording();
    } else if quantize {
        app.quantize_clip();
    } else if delete_note {
        app.delete_selected_clip_note();
    } else if duplicate_note {
        app.duplicate_selected_clip_note();
    } else if nudge_left {
        app.nudge_selected_clip_note(-1.0);
    } else if nudge_right {
        app.nudge_selected_clip_note(1.0);
    } else if pitch_down {
        app.transpose_selected_clip_note(-1);
    } else if pitch_up {
        app.transpose_selected_clip_note(1);
    } else if resize_shorter {
        app.resize_selected_clip_note(-1.0);
    } else if resize_longer {
        app.resize_selected_clip_note(1.0);
    }
}

fn draw_audio_menu(ui: &mut egui::Ui, app: &mut AppState) {
    ui.label(format!("Connected: {}", app.connected_audio_output));
    ui.separator();
    if ui.button("Refresh Outputs").clicked() {
        app.refresh_audio_outputs();
    }
    if ui.button("Connect Selected Output").clicked() {
        app.connect_audio_output();
    }
    ui.separator();
    for (idx, device) in app.audio_outputs.clone().iter().enumerate() {
        let label = if device.is_default {
            format!("{} (default)", device.name)
        } else {
            device.name.clone()
        };
        if ui
            .selectable_value(&mut app.selected_audio_output, idx, label)
            .clicked()
        {
            app.connect_audio_output();
        }
    }
}

fn draw_midi_menu(ui: &mut egui::Ui, app: &mut AppState) {
    if ui.button("Refresh Inputs").clicked() {
        app.refresh_midi_inputs(None);
    }
    if ui.button("Connect Selected Input").clicked() {
        app.open_midi_input();
    }
    ui.separator();
    for (idx, name) in app.midi_inputs.clone().iter().enumerate() {
        if ui
            .selectable_value(&mut app.selected_input, idx, name)
            .clicked()
        {
            app.open_midi_input();
        }
    }
    if app.midi_inputs.is_empty() {
        ui.label("No MIDI inputs found");
    }
}

fn draw_tuning_menu(ui: &mut egui::Ui, app: &mut AppState) {
    let mut root_midi;
    let mut base_freq;
    {
        let state = app.scale_state.lock();
        root_midi = state.root_midi;
        base_freq = state.base_freq;
    }

    let mut changed = false;
    egui::Grid::new("tuning_options_grid")
        .num_columns(2)
        .spacing(egui::vec2(12.0, 8.0))
        .show(ui, |ui| {
            ui.label("Root MIDI");
            changed |= ui
                .add(egui::DragValue::new(&mut root_midi).range(0..=511))
                .changed();
            ui.end_row();

            ui.label("Base Hz");
            changed |= ui
                .add(egui::DragValue::new(&mut base_freq).range(20.0..=20000.0))
                .changed();
            ui.end_row();
        });

    if changed {
        {
            let mut state = app.scale_state.lock();
            state.root_midi = root_midi.max(0);
            state.base_freq = base_freq.max(1.0);
        }
        app.persist_current_settings();
    }
}

fn draw_synth_menu(ui: &mut egui::Ui, app: &mut AppState) {
    let mut settings = app.synth.settings();
    let original = settings;

    ui.label("Waveform");
    for waveform in Waveform::all() {
        ui.radio_value(&mut settings.waveform, waveform, waveform.as_str());
    }
    ui.separator();
    ui.add(egui::Slider::new(&mut settings.master_gain, 0.0..=1.0).text("Master gain"));
    ui.add(egui::Slider::new(&mut settings.attack_ms, 0.0..=5000.0).text("Attack ms"));
    ui.add(egui::Slider::new(&mut settings.release_ms, 0.0..=5000.0).text("Release ms"));
    ui.separator();
    ui.add(egui::Slider::new(&mut settings.drive, 0.0..=6.0).text("Drive"));
    ui.add(
        egui::Slider::new(&mut settings.filter_cutoff_hz, 20.0..=20000.0)
            .logarithmic(true)
            .text("Filter Hz"),
    );
    ui.add(egui::Slider::new(&mut settings.delay_mix, 0.0..=1.0).text("Delay mix"));
    ui.add(egui::Slider::new(&mut settings.delay_feedback, 0.0..=0.95).text("Delay feedback"));
    ui.add(egui::Slider::new(&mut settings.delay_time_ms, 1.0..=1200.0).text("Delay ms"));

    settings.master_gain = settings.master_gain.clamp(0.0, 1.0);
    settings.attack_ms = settings.attack_ms.max(0.0);
    settings.release_ms = settings.release_ms.max(0.0);
    settings.drive = settings.drive.clamp(0.0, 6.0);
    settings.filter_cutoff_hz = settings.filter_cutoff_hz.clamp(20.0, 20000.0);
    settings.delay_mix = settings.delay_mix.clamp(0.0, 1.0);
    settings.delay_feedback = settings.delay_feedback.clamp(0.0, 0.95);
    settings.delay_time_ms = settings.delay_time_ms.clamp(1.0, 1200.0);
    if settings != original {
        app.set_synth_settings(settings);
    }
}

fn draw_lumatone_menu(ui: &mut egui::Ui, app: &mut AppState) {
    if let Some(path) = &app.lumatone_path {
        ui.label(format!("Loaded: {}", path.display()));
        ui.separator();
    }
    if ui.button("Open Key Map...").clicked() {
        open_lumatone_file(app);
        ui.close();
    }
    if ui.button("Reload Presets").clicked() {
        app.reload_lumatone_presets();
    }
    ui.separator();
    for (idx, preset) in app.lumatone_presets.clone().iter().enumerate() {
        if ui
            .selectable_value(&mut app.selected_lumatone, idx, &preset.name)
            .clicked()
        {
            app.select_lumatone(idx);
        }
    }
}

fn draw_scale_library_panel(ui: &mut egui::Ui, app: &mut AppState) {
    draw_operad_panel_title(ui, "scale.library.title", "Scales");
    if let Some(action) = draw_operad_scale_library_actions(ui) {
        match action.as_str() {
            "scale.library.open" => open_scala_file(app, true),
            "scale.library.load" => app.load_selected_library_scale(),
            "scale.library.remove" => app.remove_selected_library_scale(),
            _ => {}
        }
    }

    let library = app.scale_library.clone();
    egui::ScrollArea::vertical()
        .id_salt("scale_browser_scroll")
        .max_height(180.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (idx, item) in library.iter().enumerate() {
                let selected = app.selected_scale_library == idx;
                if draw_operad_select_row(
                    ui,
                    &format!("scale.library.item.{idx}"),
                    &item.name,
                    selected,
                ) {
                    app.selected_scale_library = idx;
                }
            }
            if library.is_empty() {
                draw_operad_panel_title(ui, "scale.library.empty", "No scales");
            }
        });

    draw_audio_assets_browser(ui, app);
}

fn draw_audio_assets_browser(ui: &mut egui::Ui, app: &mut AppState) {
    if let Some(kind) = draw_operad_asset_kind_row(ui, app.selected_audio_asset_kind) {
        app.selected_audio_asset_kind = kind;
    }
    if let Some(action) = draw_operad_audio_asset_actions(ui) {
        match action.as_str() {
            "asset.import" => import_audio_asset(app),
            "asset.refresh" => app.refresh_audio_assets(),
            _ => {}
        }
    }

    let kind = app.selected_audio_asset_kind;
    let assets: Vec<_> = app
        .audio_assets
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind == kind)
        .map(|(idx, item)| (idx, item.clone()))
        .collect();

    egui::ScrollArea::vertical()
        .id_salt("audio_asset_browser_scroll")
        .show(ui, |ui| {
            for (idx, item) in &assets {
                let label = if item.is_dir {
                    format!("{} set", item.name.trim_end_matches('/'))
                } else {
                    item.name.clone()
                };
                if draw_operad_select_row(
                    ui,
                    &format!("asset.item.{idx}"),
                    &label,
                    app.selected_audio_asset == Some(*idx),
                ) {
                    app.select_audio_asset(*idx);
                }
            }
            if assets.is_empty() {
                draw_operad_panel_title(
                    ui,
                    "asset.empty",
                    format!("Drop files in audio_assets/{}", kind.label().to_lowercase()),
                );
            }
        });

    if let Some(asset) = app.selected_audio_asset_item()
        && asset.kind == kind
    {
        draw_operad_panel_title(ui, "asset.path", asset.path.display().to_string());
    }
}

fn draw_operad_panel_title(ui: &mut egui::Ui, name: &str, text: impl Into<String>) {
    let (rect, _response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
    let mut document = operad_ui::document(rect.width(), rect.height());
    let row = operad_ui::panel_row(&mut document, name);
    operad_ui::label(
        &mut document,
        row,
        &format!("{name}.label"),
        text,
        rect.width().max(1.0) - 16.0,
        true,
    );
    operad_bridge::compute_and_paint_document(ui, rect, &mut document);
}

fn draw_operad_scale_library_actions(ui: &mut egui::Ui) -> Option<String> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    let mut document = operad_ui::document(rect.width(), rect.height());
    let row = operad_ui::panel_row(&mut document, "scale.library.actions");
    operad_ui::button(
        &mut document,
        row,
        "scale.library.open",
        "Open",
        64.0,
        false,
        true,
    );
    operad_ui::spacer(&mut document, row, "scale.library.actions.space.1", 6.0);
    operad_ui::button(
        &mut document,
        row,
        "scale.library.load",
        "Load",
        64.0,
        false,
        true,
    );
    operad_ui::spacer(&mut document, row, "scale.library.actions.space.2", 6.0);
    operad_ui::button(
        &mut document,
        row,
        "scale.library.remove",
        "Remove",
        76.0,
        false,
        true,
    );
    if operad_bridge::compute_and_paint_document(ui, rect, &mut document) {
        operad_bridge::clicked_node_name(&response, rect, &document)
    } else {
        None
    }
}

fn draw_operad_audio_asset_actions(ui: &mut egui::Ui) -> Option<String> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    let mut document = operad_ui::document(rect.width(), rect.height());
    let row = operad_ui::panel_row(&mut document, "asset.actions");
    operad_ui::button(
        &mut document,
        row,
        "asset.import",
        "Import",
        76.0,
        false,
        true,
    );
    operad_ui::spacer(&mut document, row, "asset.actions.space.1", 6.0);
    operad_ui::button(
        &mut document,
        row,
        "asset.refresh",
        "Refresh",
        82.0,
        false,
        true,
    );
    if operad_bridge::compute_and_paint_document(ui, rect, &mut document) {
        operad_bridge::clicked_node_name(&response, rect, &document)
    } else {
        None
    }
}

fn draw_operad_asset_kind_row(
    ui: &mut egui::Ui,
    selected_kind: AudioAssetKind,
) -> Option<AudioAssetKind> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    let mut document = operad_ui::document(rect.width(), rect.height());
    let row = operad_ui::panel_row(&mut document, "asset.kind.row");
    for (idx, kind) in AudioAssetKind::all().iter().copied().enumerate() {
        operad_ui::button(
            &mut document,
            row,
            &format!("asset.kind.{kind:?}"),
            kind.label(),
            82.0,
            kind == selected_kind,
            true,
        );
        if idx + 1 < AudioAssetKind::all().len() {
            operad_ui::spacer(&mut document, row, &format!("asset.kind.space.{idx}"), 5.0);
        }
    }
    if !operad_bridge::compute_and_paint_document(ui, rect, &mut document) {
        return None;
    }
    let clicked = operad_bridge::clicked_node_name(&response, rect, &document)?;
    AudioAssetKind::all()
        .into_iter()
        .find(|kind| clicked == format!("asset.kind.{kind:?}"))
}

fn draw_operad_select_row(ui: &mut egui::Ui, name: &str, label: &str, selected: bool) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
    let mut document = operad_ui::document(rect.width(), rect.height());
    let row = operad_ui::panel_row(&mut document, &format!("{name}.row"));
    operad_ui::button(
        &mut document,
        row,
        name,
        label,
        (rect.width() - 12.0).max(1.0),
        selected,
        true,
    );
    operad_bridge::compute_and_paint_document(ui, rect, &mut document)
        && operad_bridge::clicked_node_name(&response, rect, &document).as_deref() == Some(name)
}

fn draw_inspector_panel(ui: &mut egui::Ui, app: &mut AppState) {
    if let Some(event) = app.midi_last.lock().clone() {
        draw_event(ui, "Last MIDI", &event);
    } else {
        ui.label("Last MIDI: none");
    }

    ui.separator();
    draw_mapping_capture_panel(ui, app);

    ui.separator();
    ui.label("Recent MIDI");
    let log = app.midi_log.lock().clone();
    egui::Grid::new("midi_log_grid")
        .striped(true)
        .num_columns(4)
        .spacing(egui::vec2(10.0, 6.0))
        .show(ui, |ui| {
            ui.label("Raw");
            ui.label("Mapped");
            ui.label("Pitch");
            ui.label("Freq");
            ui.end_row();
            for event in log.iter().rev().take(12) {
                ui.label(format!("{}:{}", event.channel + 1, event.midi_note));
                ui.label(mapping_label(event));
                ui.label(format!("{}", event.musical_note));
                ui.label(
                    event
                        .freq
                        .map(format_freq)
                        .unwrap_or_else(|| "-".to_string()),
                );
                ui.end_row();
            }
        });
}

fn draw_mapping_capture_panel(ui: &mut egui::Ui, app: &mut AppState) {
    ui.label("Mapping Capture");
    let (armed, count, events) = {
        let capture = app.midi_capture.lock();
        (capture.is_armed(), capture.len(), capture.events())
    };
    ui.horizontal(|ui| {
        let record_label = if armed { "Stop" } else { "Record" };
        if ui.button(record_label).clicked() {
            if armed {
                app.stop_mapping_capture();
            } else {
                app.start_mapping_capture();
            }
        }
        if ui.button("Clear").clicked() {
            app.clear_mapping_capture();
        }
        if ui
            .add_enabled(!events.is_empty(), egui::Button::new("Copy All"))
            .clicked()
        {
            ui.output_mut(|output| {
                output
                    .commands
                    .push(egui::OutputCommand::CopyText(capture_events_to_tsv(
                        &events,
                    )));
            });
            app.last_status = format!("Copied {} mapping capture rows", events.len());
        }
    });

    let state = if armed { "recording" } else { "idle" };
    ui.label(format!("{count} note-ons captured ({state})"));

    if events.is_empty() {
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(220.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("mapping_capture_grid")
                .striped(true)
                .num_columns(5)
                .spacing(egui::vec2(8.0, 6.0))
                .show(ui, |ui| {
                    ui.label("#");
                    ui.label("Raw");
                    ui.label("Map");
                    ui.label("Visual");
                    ui.label("Pitch");
                    ui.end_row();

                    for (idx, event) in events.iter().enumerate() {
                        ui.label(format!("{}", idx + 1));
                        ui.label(format!("{}:{}", event.channel + 1, event.midi_note));
                        ui.label(capture_mapping_label(event));
                        ui.label(visual_position_label(event));
                        ui.label(format!("{}", event.musical_note));
                        ui.end_row();
                    }
                });
        });
}

fn draw_clip_panel(ctx: &egui::Context, app: &mut AppState) {
    egui::TopBottomPanel::bottom("clip_panel")
        .resizable(true)
        .default_height(300.0)
        .height_range(220.0..=520.0)
        .show(ctx, |ui| {
            ui.add_space(2.0);
            draw_clip_view(ui, app);
        });
}

fn draw_workspace(ui: &mut egui::Ui, app: &mut AppState) {
    let scale = app.scale_state.lock().clone();
    let map = app.lumatone_map.lock().clone();
    let mut active_notes: HashSet<u32> = app.synth.active_notes().into_iter().collect();
    active_notes.extend(app.playback_active_keys.iter().copied());
    let capture_events = app.midi_capture.lock().events();
    let captured_keys = captured_key_orders(&capture_events);

    ui.vertical(|ui| {
        draw_scale_header(ui, app, &scale);
        draw_lumatone_status_row(ui, app, map.as_ref());
        let grid_max_height = (ui.available_height() - 88.0).clamp(140.0, 380.0);
        egui::ScrollArea::both()
            .max_height(grid_max_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                draw_lumatone_grid(
                    ui,
                    &active_notes,
                    &captured_keys,
                    map,
                    &scale,
                    app.show_key_labels,
                );
            });
        ui.separator();
        draw_transport_panel(ui, app);
    });
}

fn draw_transport_panel(ui: &mut egui::Ui, app: &mut AppState) {
    let snapshot = app.music_project.lock().snapshot();
    let mut bpm = snapshot.transport.bpm;
    let mut loop_beats = snapshot.transport.loop_beats;
    let mut overdub = snapshot.transport.overdub;
    let mut quantize_on_record = snapshot.transport.quantize_on_record;
    let mut metronome_enabled = snapshot.transport.metronome_enabled;
    let mut quantize_grid = snapshot.transport.quantize_grid;
    let (playing, recording, beat) = {
        let project = app.music_project.lock();
        (
            project.transport.playing,
            project.transport.recording,
            project.current_position_beats(std::time::Instant::now()),
        )
    };

    if let Some(action) = draw_operad_transport_actions(
        ui,
        playing,
        recording,
        beat,
        bpm,
        loop_beats,
        overdub,
        quantize_on_record,
        metronome_enabled,
        quantize_grid,
    ) {
        match action.as_str() {
            "transport.play" => {
                if playing {
                    app.stop_transport();
                } else {
                    app.play_transport();
                }
            }
            "transport.record" => {
                if recording {
                    app.stop_recording();
                } else {
                    app.start_recording();
                }
            }
            "transport.clear_clip" => app.clear_clip(),
            "transport.quantize_clip" => app.quantize_clip(),
            "transport.overdub" => overdub = !overdub,
            "transport.quantize_record" => quantize_on_record = !quantize_on_record,
            "transport.metronome" => metronome_enabled = !metronome_enabled,
            "transport.quantize_grid" => quantize_grid = next_quantize_grid(quantize_grid),
            _ => {}
        }
    }

    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::DragValue::new(&mut bpm)
                .range(20.0..=320.0)
                .prefix("BPM "),
        );
        ui.add(
            egui::DragValue::new(&mut loop_beats)
                .range(1.0..=128.0)
                .prefix("Loop "),
        );
    });

    let mut project = app.music_project.lock();
    project.transport.bpm = bpm.clamp(20.0, 320.0);
    project.transport.loop_beats = loop_beats.clamp(1.0, 128.0);
    project.transport.overdub = overdub;
    project.transport.quantize_on_record = quantize_on_record;
    project.transport.metronome_enabled = metronome_enabled;
    project.transport.quantize_grid = quantize_grid;
}

#[allow(clippy::too_many_arguments)]
fn draw_operad_transport_actions(
    ui: &mut egui::Ui,
    playing: bool,
    recording: bool,
    beat: f32,
    bpm: f32,
    loop_beats: f32,
    overdub: bool,
    quantize_on_record: bool,
    metronome_enabled: bool,
    quantize_grid: QuantizeGrid,
) -> Option<String> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click());
    let mut document = operad_ui::document(rect.width(), rect.height());
    let row = operad_ui::panel_row(&mut document, "transport.row");
    operad_ui::button(
        &mut document,
        row,
        "transport.play",
        if playing { "Stop" } else { "Play" },
        64.0,
        playing,
        true,
    );
    operad_ui::spacer(&mut document, row, "transport.space.1", 6.0);
    operad_ui::record_button(
        &mut document,
        row,
        "transport.record",
        if recording { "Stop Rec" } else { "Record" },
        recording,
    );
    operad_ui::spacer(&mut document, row, "transport.space.2", 6.0);
    operad_ui::button(
        &mut document,
        row,
        "transport.clear_clip",
        "Clear Clip",
        82.0,
        false,
        true,
    );
    operad_ui::spacer(&mut document, row, "transport.space.3", 6.0);
    operad_ui::button(
        &mut document,
        row,
        "transport.quantize_clip",
        "Quantize",
        78.0,
        false,
        true,
    );
    operad_ui::spacer(&mut document, row, "transport.space.4", 10.0);
    operad_ui::divider(&mut document, row, "transport.divider.1");
    operad_ui::spacer(&mut document, row, "transport.space.5", 10.0);
    operad_ui::mono_label(
        &mut document,
        row,
        "transport.beat",
        format!("Beat {:.2}", beat + 1.0),
        86.0,
    );
    operad_ui::mono_label(
        &mut document,
        row,
        "transport.bpm",
        format!("{bpm:.1} BPM"),
        92.0,
    );
    operad_ui::mono_label(
        &mut document,
        row,
        "transport.loop",
        format!("{loop_beats:.0} beat loop"),
        100.0,
    );
    operad_ui::button(
        &mut document,
        row,
        "transport.quantize_grid",
        format!("Q {}", quantize_grid.as_str()),
        64.0,
        quantize_grid != QuantizeGrid::Off,
        true,
    );
    operad_ui::spacer(&mut document, row, "transport.space.6", 8.0);
    operad_ui::checkbox(
        &mut document,
        row,
        "transport.overdub",
        "Overdub",
        overdub,
        96.0,
    );
    operad_ui::spacer(&mut document, row, "transport.space.7", 6.0);
    operad_ui::checkbox(
        &mut document,
        row,
        "transport.quantize_record",
        "Quantize Rec",
        quantize_on_record,
        116.0,
    );
    operad_ui::spacer(&mut document, row, "transport.space.8", 6.0);
    operad_ui::checkbox(
        &mut document,
        row,
        "transport.metronome",
        "Metronome",
        metronome_enabled,
        104.0,
    );

    if operad_bridge::compute_and_paint_document(ui, rect, &mut document) {
        operad_bridge::clicked_node_name(&response, rect, &document)
    } else {
        None
    }
}

fn next_quantize_grid(current: QuantizeGrid) -> QuantizeGrid {
    let all = QuantizeGrid::all();
    let current_index = all.iter().position(|grid| *grid == current).unwrap_or(0);
    all[(current_index + 1) % all.len()]
}

fn draw_clip_view(ui: &mut egui::Ui, app: &mut AppState) {
    let scale = app.scale_state.lock().clone();
    let project = app.music_project.lock();
    let notes = project.clip.notes.clone();
    let loop_beats = project.transport.loop_beats;
    let beat = project.current_position_beats(std::time::Instant::now());
    let recording = project.transport.recording;
    let grid_step = project.transport.quantize_grid.step_beats().unwrap_or(1.0);
    let selected_id = app.selected_clip_note;
    drop(project);
    let selected_note = app.selected_clip_note();
    let (min_pitch, max_pitch) = piano_roll_pitch_range(&notes, selected_note.as_ref(), &scale);

    if let Some(action) = draw_operad_clip_actions(ui, notes.len(), selected_note.as_ref()) {
        match action.as_str() {
            "clip.add_note" => app.add_clip_note_at(beat, scale.root_midi),
            "clip.delete_note" => app.delete_selected_clip_note(),
            "clip.duplicate_note" => app.duplicate_selected_clip_note(),
            "clip.nudge_left" => app.nudge_selected_clip_note(-1.0),
            "clip.nudge_right" => app.nudge_selected_clip_note(1.0),
            "clip.pitch_down" => app.transpose_selected_clip_note(-1),
            "clip.pitch_up" => app.transpose_selected_clip_note(1),
            "clip.shorter" => app.resize_selected_clip_note(-1.0),
            "clip.longer" => app.resize_selected_clip_note(1.0),
            _ => {}
        }
    }
    if let Some(note) = &selected_note {
        ui.horizontal_wrapped(|ui| {
            let mut velocity = i32::from(note.velocity);
            if ui
                .add(
                    egui::DragValue::new(&mut velocity)
                        .range(1..=127)
                        .prefix("Vel "),
                )
                .changed()
            {
                app.set_selected_clip_note_velocity(velocity.clamp(1, 127) as u8);
            }
        });
    }
    let pitch_count = (max_pitch - min_pitch + 1).max(1) as f32;
    let desired_height = (pitch_count * 16.0).clamp(300.0, 520.0);
    let desired_size = egui::vec2(ui.available_width(), desired_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let painter = ui.painter_at(rect);
    let keyboard_width = 72.0_f32.min(rect.width() * 0.22);
    let keyboard_rect = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.left() + keyboard_width, rect.bottom()),
    );
    let time_rect = egui::Rect::from_min_max(
        egui::pos2(keyboard_rect.right(), rect.top()),
        rect.right_bottom(),
    );
    let row_height = time_rect.height() / pitch_count;
    let layout = PianoRollLayout {
        time_rect,
        keyboard_rect,
        min_pitch,
        max_pitch,
        loop_beats,
        row_height,
    };

    draw_piano_roll_scene(
        ui,
        rect,
        layout,
        &scale,
        &notes,
        selected_id,
        beat,
        loop_beats,
        recording,
        grid_step,
    );
    draw_piano_roll_pitch_lane_labels(&painter, layout, &scale);
    draw_piano_roll_beat_labels(&painter, layout, grid_step);

    if notes.is_empty() {
        painter.text(
            layout.time_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Empty clip",
            egui::FontId::proportional(14.0),
            egui::Color32::from_gray(130),
        );
        handle_piano_roll_click(&response, layout, &notes, app);
        return;
    }

    for note in &notes {
        draw_clip_note_label(&painter, layout, note, selected_id == Some(note.id));
    }
    handle_piano_roll_click(&response, layout, &notes, app);
}

fn draw_operad_clip_actions(
    ui: &mut egui::Ui,
    note_count: usize,
    selected_note: Option<&ClipNote>,
) -> Option<String> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click());
    let mut document = operad_ui::document(rect.width(), rect.height());
    let row = operad_ui::panel_row(&mut document, "clip.actions.row");
    operad_ui::mono_label(
        &mut document,
        row,
        "clip.actions.count",
        format!("{note_count} notes"),
        74.0,
    );
    operad_ui::spacer(&mut document, row, "clip.actions.space.1", 6.0);
    operad_ui::button(
        &mut document,
        row,
        "clip.add_note",
        "Add Note",
        76.0,
        false,
        true,
    );
    operad_ui::spacer(&mut document, row, "clip.actions.space.2", 8.0);
    operad_ui::divider(&mut document, row, "clip.actions.divider");
    operad_ui::spacer(&mut document, row, "clip.actions.space.3", 8.0);

    if let Some(note) = selected_note {
        operad_ui::mono_label(
            &mut document,
            row,
            "clip.actions.selected",
            format!(
                "Note {} start {:.2} len {:.2}",
                note.musical_note, note.start_beats, note.duration_beats
            ),
            236.0,
        );
        operad_ui::spacer(&mut document, row, "clip.actions.space.4", 8.0);
        operad_ui::button(
            &mut document,
            row,
            "clip.delete_note",
            "Delete",
            62.0,
            false,
            true,
        );
        operad_ui::spacer(&mut document, row, "clip.actions.space.5", 5.0);
        operad_ui::button(
            &mut document,
            row,
            "clip.duplicate_note",
            "Duplicate",
            82.0,
            false,
            true,
        );
        operad_ui::spacer(&mut document, row, "clip.actions.space.6", 5.0);
        operad_ui::button(
            &mut document,
            row,
            "clip.nudge_left",
            "<",
            30.0,
            false,
            true,
        );
        operad_ui::button(
            &mut document,
            row,
            "clip.nudge_right",
            ">",
            30.0,
            false,
            true,
        );
        operad_ui::spacer(&mut document, row, "clip.actions.space.7", 5.0);
        operad_ui::button(
            &mut document,
            row,
            "clip.pitch_down",
            "Pitch -",
            66.0,
            false,
            true,
        );
        operad_ui::button(
            &mut document,
            row,
            "clip.pitch_up",
            "Pitch +",
            66.0,
            false,
            true,
        );
        operad_ui::spacer(&mut document, row, "clip.actions.space.8", 5.0);
        operad_ui::button(
            &mut document,
            row,
            "clip.shorter",
            "Shorter",
            68.0,
            false,
            true,
        );
        operad_ui::button(
            &mut document,
            row,
            "clip.longer",
            "Longer",
            64.0,
            false,
            true,
        );
    } else {
        operad_ui::label(
            &mut document,
            row,
            "clip.actions.empty",
            "No note selected",
            160.0,
            false,
        );
    }

    if operad_bridge::compute_and_paint_document(ui, rect, &mut document) {
        operad_bridge::clicked_node_name(&response, rect, &document)
    } else {
        None
    }
}

fn handle_piano_roll_click(
    response: &egui::Response,
    layout: PianoRollLayout,
    notes: &[ClipNote],
    app: &mut AppState,
) {
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let hit = notes.iter().rev().find_map(|note| {
            let rects = clip_note_rects(layout, note);
            rects
                .iter()
                .any(|note_rect| note_rect.contains(pos))
                .then_some(note.id)
        });
        if let Some(note_id) = hit {
            app.select_clip_note(Some(note_id));
        } else if response.double_clicked() && layout.time_rect.contains(pos) {
            app.add_clip_note_at(layout.beat_at(pos.x), layout.pitch_at(pos.y));
        } else {
            app.select_clip_note(None);
        }
    }
}

fn draw_clip_note_label(
    painter: &egui::Painter,
    layout: PianoRollLayout,
    note: &ClipNote,
    selected: bool,
) {
    for note_rect in clip_note_rects(layout, note) {
        painter.text(
            note_rect.left_center() + egui::vec2(5.0, 0.0),
            egui::Align2::LEFT_CENTER,
            note.musical_note.to_string(),
            egui::FontId::monospace(11.0),
            if selected {
                egui::Color32::BLACK
            } else {
                egui::Color32::WHITE
            },
        );
    }
}

fn clip_note_rects(layout: PianoRollLayout, note: &ClipNote) -> Vec<egui::Rect> {
    let x = layout.time_rect.left()
        + note.start_beats / layout.loop_beats.max(1.0) * layout.time_rect.width();
    let width =
        (note.duration_beats / layout.loop_beats.max(1.0) * layout.time_rect.width()).max(4.0);
    let y = layout.y_for_pitch(note.musical_note) + 2.0;
    let height = (layout.row_height - 4.0).max(8.0);
    let mut rects = Vec::with_capacity(2);
    let first_width = width.min(layout.time_rect.right() - x).max(0.0);
    if first_width > 0.0 {
        rects.push(egui::Rect::from_min_size(
            egui::pos2(x, y),
            egui::vec2(first_width, height),
        ));
    }
    if x + width > layout.time_rect.right() {
        let wrapped_width = (x + width - layout.time_rect.right()).min(layout.time_rect.width());
        if wrapped_width > 0.0 {
            rects.push(egui::Rect::from_min_size(
                egui::pos2(layout.time_rect.left(), y),
                egui::vec2(wrapped_width, height),
            ));
        }
    }
    rects
}

fn draw_piano_roll_scene(
    ui: &egui::Ui,
    rect: egui::Rect,
    layout: PianoRollLayout,
    scale: &ScaleState,
    notes: &[ClipNote],
    selected_id: Option<u64>,
    beat: f32,
    loop_beats: f32,
    recording: bool,
    grid_step: f32,
) {
    let mut primitives = Vec::new();
    push_rect_scene(
        &mut primitives,
        rect.min,
        rect,
        egui::Color32::from_gray(18),
        Some(egui::Stroke::new(1.0, egui::Color32::from_gray(45))),
    );
    push_rect_scene(
        &mut primitives,
        rect.min,
        layout.time_rect,
        egui::Color32::from_gray(14),
        None,
    );
    push_rect_scene(
        &mut primitives,
        rect.min,
        layout.keyboard_rect,
        egui::Color32::from_gray(24),
        None,
    );
    push_piano_roll_pitch_lane_scene(&mut primitives, rect.min, layout, scale);
    push_piano_roll_beat_grid_scene(&mut primitives, rect.min, layout, grid_step);
    for note in notes {
        push_clip_note_scene(
            &mut primitives,
            rect.min,
            layout,
            note,
            selected_id == Some(note.id),
        );
    }
    push_clip_playhead_scene(
        &mut primitives,
        rect.min,
        layout.time_rect,
        beat,
        loop_beats,
        recording,
    );

    operad_bridge::paint_scene(ui, rect, "piano_roll.scene", primitives);
}

fn push_piano_roll_pitch_lane_scene(
    primitives: &mut Vec<ScenePrimitive>,
    origin: egui::Pos2,
    layout: PianoRollLayout,
    scale: &ScaleState,
) {
    for pitch in layout.min_pitch..=layout.max_pitch {
        let y = layout.y_for_pitch(pitch);
        let row_rect = egui::Rect::from_min_max(
            egui::pos2(layout.time_rect.left(), y),
            egui::pos2(layout.time_rect.right(), y + layout.row_height),
        );
        let keyboard_row = egui::Rect::from_min_max(
            egui::pos2(layout.keyboard_rect.left(), y),
            egui::pos2(layout.keyboard_rect.right(), y + layout.row_height),
        );
        let degree = scale.note_info(pitch).map(|info| info.degree).unwrap_or(0);
        let root_lane = degree == 0;
        let lane = if root_lane {
            egui::Color32::from_rgb(28, 34, 38)
        } else if pitch % 2 == 0 {
            egui::Color32::from_gray(17)
        } else {
            egui::Color32::from_gray(13)
        };
        let key_fill = if root_lane {
            egui::Color32::from_rgb(55, 72, 77)
        } else if is_piano_black_key(pitch) {
            egui::Color32::from_gray(34)
        } else {
            egui::Color32::from_gray(68)
        };
        push_rect_scene(primitives, origin, row_rect, lane, None);
        push_rect_scene(
            primitives,
            origin,
            keyboard_row.shrink2(egui::vec2(0.0, 1.0)),
            key_fill,
            None,
        );
        push_line_scene(
            primitives,
            origin,
            egui::pos2(layout.time_rect.left(), y),
            egui::pos2(layout.time_rect.right(), y),
            egui::Stroke::new(1.0, egui::Color32::from_gray(35)),
        );
    }
    push_line_scene(
        primitives,
        origin,
        layout.keyboard_rect.right_top(),
        layout.keyboard_rect.right_bottom(),
        egui::Stroke::new(1.0, egui::Color32::from_gray(55)),
    );
}

fn push_piano_roll_beat_grid_scene(
    primitives: &mut Vec<ScenePrimitive>,
    origin: egui::Pos2,
    layout: PianoRollLayout,
    grid_step: f32,
) {
    let step = grid_step.clamp(0.125, 4.0);
    let mut beat = 0.0_f32;
    while beat <= layout.loop_beats + 0.001 {
        let x =
            layout.time_rect.left() + beat / layout.loop_beats.max(1.0) * layout.time_rect.width();
        let whole = (beat.round() - beat).abs() < 0.001;
        let bar = whole && (beat as i32).rem_euclid(4) == 0;
        let stroke = if bar {
            egui::Stroke::new(1.4, egui::Color32::from_gray(78))
        } else if whole {
            egui::Stroke::new(1.0, egui::Color32::from_gray(58))
        } else {
            egui::Stroke::new(1.0, egui::Color32::from_gray(31))
        };
        push_line_scene(
            primitives,
            origin,
            egui::pos2(x, layout.time_rect.top()),
            egui::pos2(x, layout.time_rect.bottom()),
            stroke,
        );
        beat += step;
    }
}

fn push_clip_note_scene(
    primitives: &mut Vec<ScenePrimitive>,
    origin: egui::Pos2,
    layout: PianoRollLayout,
    note: &ClipNote,
    selected: bool,
) {
    let fill = if selected {
        egui::Color32::from_rgb(255, 190, 72)
    } else {
        egui::Color32::from_rgb(90, 172, 197)
    };
    let stroke = if selected {
        egui::Stroke::new(2.0, egui::Color32::WHITE)
    } else {
        egui::Stroke::new(1.0, egui::Color32::BLACK)
    };
    for note_rect in clip_note_rects(layout, note) {
        push_rect_scene(primitives, origin, note_rect, fill, Some(stroke));
    }
}

fn push_clip_playhead_scene(
    primitives: &mut Vec<ScenePrimitive>,
    origin: egui::Pos2,
    rect: egui::Rect,
    beat: f32,
    loop_beats: f32,
    recording: bool,
) {
    let x = rect.left() + beat / loop_beats.max(1.0) * rect.width();
    let color = if recording {
        egui::Color32::from_rgb(255, 76, 76)
    } else {
        egui::Color32::WHITE
    };
    push_line_scene(
        primitives,
        origin,
        egui::pos2(x, rect.top()),
        egui::pos2(x, rect.bottom()),
        egui::Stroke::new(2.0, color),
    );
    primitives.push(ScenePrimitive::Polygon {
        points: vec![
            operad_bridge::local_point(origin, egui::pos2(x - 5.0, rect.top())),
            operad_bridge::local_point(origin, egui::pos2(x + 5.0, rect.top())),
            operad_bridge::local_point(origin, egui::pos2(x, rect.top() + 8.0)),
        ],
        fill: operad_bridge::color_from_egui(color),
        stroke: None,
    });
}

fn push_rect_scene(
    primitives: &mut Vec<ScenePrimitive>,
    origin: egui::Pos2,
    rect: egui::Rect,
    fill: egui::Color32,
    stroke: Option<egui::Stroke>,
) {
    primitives.push(operad_bridge::rect_primitive(origin, rect, fill, stroke));
}

fn push_line_scene(
    primitives: &mut Vec<ScenePrimitive>,
    origin: egui::Pos2,
    from: egui::Pos2,
    to: egui::Pos2,
    stroke: egui::Stroke,
) {
    primitives.push(operad_bridge::line_primitive(origin, from, to, stroke));
}

fn draw_piano_roll_pitch_lane_labels(
    painter: &egui::Painter,
    layout: PianoRollLayout,
    scale: &ScaleState,
) {
    for pitch in layout.min_pitch..=layout.max_pitch {
        let y = layout.y_for_pitch(pitch);
        let keyboard_row = egui::Rect::from_min_max(
            egui::pos2(layout.keyboard_rect.left(), y),
            egui::pos2(layout.keyboard_rect.right(), y + layout.row_height),
        );
        let degree = scale.note_info(pitch).map(|info| info.degree).unwrap_or(0);
        painter.text(
            keyboard_row.center(),
            egui::Align2::CENTER_CENTER,
            format!("{} D{}", pitch, degree + 1),
            egui::FontId::monospace(10.0),
            egui::Color32::from_gray(225),
        );
    }
}

fn draw_piano_roll_beat_labels(painter: &egui::Painter, layout: PianoRollLayout, grid_step: f32) {
    let step = grid_step.clamp(0.125, 4.0);
    let mut beat = 0.0_f32;
    while beat <= layout.loop_beats + 0.001 {
        let x =
            layout.time_rect.left() + beat / layout.loop_beats.max(1.0) * layout.time_rect.width();
        let whole = (beat.round() - beat).abs() < 0.001;
        if whole && beat < layout.loop_beats {
            painter.text(
                egui::pos2(x + 4.0, layout.time_rect.top() + 4.0),
                egui::Align2::LEFT_TOP,
                format!("{}", beat as i32 + 1),
                egui::FontId::monospace(10.0),
                egui::Color32::from_gray(120),
            );
        }
        beat += step;
    }
}

fn piano_roll_pitch_range(
    notes: &[ClipNote],
    selected_note: Option<&ClipNote>,
    scale: &ScaleState,
) -> (i32, i32) {
    let mut min_pitch = scale.root_midi - 12;
    let mut max_pitch = scale.root_midi + 12;
    for pitch in notes
        .iter()
        .map(|note| note.musical_note)
        .chain(selected_note.map(|note| note.musical_note))
    {
        min_pitch = min_pitch.min(pitch - 3);
        max_pitch = max_pitch.max(pitch + 3);
    }
    if max_pitch - min_pitch < 24 {
        let center = (max_pitch + min_pitch) / 2;
        min_pitch = center - 12;
        max_pitch = center + 12;
    }
    (min_pitch.max(-128), max_pitch.min(256))
}

fn is_piano_black_key(pitch: i32) -> bool {
    matches!(pitch.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

fn draw_scale_header(ui: &mut egui::Ui, app: &AppState, scale: &ScaleState) {
    let (rect, _response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::hover());
    let mut document = operad_ui::document(rect.width(), rect.height());
    let row = operad_ui::panel_row(&mut document, "scale.header.row");
    let scale_name = scale.scale.description.trim();
    if !scale_name.is_empty() && scale_name != "12-TET" {
        operad_ui::label(
            &mut document,
            row,
            "scale.header.name",
            scale_name,
            220.0,
            true,
        );
        operad_ui::spacer(&mut document, row, "scale.header.space.1", 8.0);
        operad_ui::divider(&mut document, row, "scale.header.divider.1");
        operad_ui::spacer(&mut document, row, "scale.header.space.2", 8.0);
    }
    operad_ui::mono_label(
        &mut document,
        row,
        "scale.header.steps",
        format!("{} steps", scale.scale.steps.len()),
        72.0,
    );
    operad_ui::spacer(&mut document, row, "scale.header.space.3", 8.0);
    operad_ui::divider(&mut document, row, "scale.header.divider.2");
    operad_ui::spacer(&mut document, row, "scale.header.space.4", 8.0);
    operad_ui::mono_label(
        &mut document,
        row,
        "scale.header.root",
        format!("Root MIDI {}", scale.root_midi),
        104.0,
    );
    operad_ui::spacer(&mut document, row, "scale.header.space.5", 8.0);
    operad_ui::divider(&mut document, row, "scale.header.divider.3");
    operad_ui::spacer(&mut document, row, "scale.header.space.6", 8.0);
    operad_ui::mono_label(
        &mut document,
        row,
        "scale.header.base_freq",
        format!("{:.2} Hz", scale.base_freq),
        96.0,
    );
    if let Some(path) = &app.scala_path {
        operad_ui::spacer(&mut document, row, "scale.header.space.7", 8.0);
        operad_ui::divider(&mut document, row, "scale.header.divider.4");
        operad_ui::spacer(&mut document, row, "scale.header.space.8", 8.0);
        operad_ui::label(
            &mut document,
            row,
            "scale.header.path",
            path.display().to_string(),
            (rect.width() - 560.0).max(120.0),
            false,
        );
    }
    operad_bridge::compute_and_paint_document(ui, rect, &mut document);
}

fn draw_lumatone_status_row(ui: &mut egui::Ui, app: &AppState, map: Option<&Arc<LumatoneMap>>) {
    let (rect, _response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
    let mut document = operad_ui::document(rect.width(), rect.height());
    let row = operad_ui::panel_row(&mut document, "lumatone.status.row");
    if let Some(map) = map {
        operad_ui::mono_label(
            &mut document,
            row,
            "lumatone.status.keys",
            format!("{} keys", map.len()),
            78.0,
        );
    } else {
        operad_ui::label(
            &mut document,
            row,
            "lumatone.status.keys",
            "No key map loaded",
            130.0,
            false,
        );
    }
    if let Some(path) = &app.lumatone_path {
        operad_ui::spacer(&mut document, row, "lumatone.status.space.1", 8.0);
        operad_ui::divider(&mut document, row, "lumatone.status.divider.1");
        operad_ui::spacer(&mut document, row, "lumatone.status.space.2", 8.0);
        operad_ui::label(
            &mut document,
            row,
            "lumatone.status.preset",
            format!(
                "Preset: {}",
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Unknown")
            ),
            (rect.width() - 120.0).max(160.0),
            false,
        );
    }
    operad_bridge::compute_and_paint_document(ui, rect, &mut document);
}

fn draw_event(ui: &mut egui::Ui, label: &str, event: &MidiEvent) {
    let age_ms = event.at.elapsed().as_millis();
    ui.label(format!(
        "{label}: 0x{:02X} raw 0x{:02X}, ch {}, note {}, vel {}, {} ms ago",
        event.status,
        event.raw_status,
        event.channel + 1,
        event.midi_note,
        event.velocity,
        age_ms
    ));
    ui.label(format!("Mapped key: {}", event.key_index));
    if let Some(position) = lumatone_position_for_key_index(event.key_index) {
        ui.label(format!(
            "Visual key: board {}, row {}, column {}",
            position.board + 1,
            position.row + 1,
            position.col + 1
        ));
    }
    ui.label(format!("Pitch note: {}", event.musical_note));
    ui.label(format!(
        "Mapping: {}",
        if event.mapped_from_lumatone {
            "key map"
        } else {
            "raw MIDI fallback"
        }
    ));
    if let Some(freq) = event.freq {
        ui.label(format!("Frequency: {}", format_freq(freq)));
    }
    if let Some(degree) = event.scale_degree {
        let octave = event.scale_octave.unwrap_or(0);
        ui.label(format!("Scale degree: {degree}, octave: {octave}"));
    }
    if let Some(cents) = event.cents_from_root {
        ui.label(format!("Cents from root: {cents:.2}"));
    }
}

fn draw_lumatone_grid(
    ui: &mut egui::Ui,
    active_notes: &HashSet<u32>,
    captured_keys: &HashMap<u32, usize>,
    map: Option<Arc<LumatoneMap>>,
    scale: &ScaleState,
    show_labels: bool,
) {
    let size = key_size_for_available_width(ui.available_width());
    let width = (3.0_f32).sqrt() * size;
    let height = 2.0 * size;
    let row_spacing = 1.5 * size;
    let col_spacing = width;
    let row_skew = col_spacing / 2.0;
    let board_stride = LUMATONE_BOARD_COLUMNS * col_spacing;
    let padding = size;
    let max_center_x = (0..LUMATONE_BOARDS)
        .flat_map(|board| {
            LUMATONE_ROW_COUNTS
                .iter()
                .enumerate()
                .map(move |(row, count)| {
                    board as f32 * board_stride
                        + row as f32 * row_skew
                        + (*count as f32 - 1.0) * col_spacing
                })
        })
        .fold(0.0_f32, f32::max);
    let max_center_y = (LUMATONE_ROW_COUNTS.len() as f32 - 1.0) * row_spacing;

    let total_width = max_center_x + width + padding * 2.0;
    let total_height = max_center_y + height + padding * 2.0;
    let (rect, _response) =
        ui.allocate_exact_size(egui::vec2(total_width, total_height), egui::Sense::hover());
    let mut key_primitives = Vec::with_capacity(LUMATONE_BOARDS * LUMATONE_KEYS_PER_BOARD);

    for board in 0..LUMATONE_BOARDS {
        let board_origin = egui::pos2(
            padding + width / 2.0 + board as f32 * board_stride,
            padding + size,
        );
        for (row, count) in LUMATONE_ROW_COUNTS.iter().copied().enumerate() {
            for col in 0..count {
                let local = LUMATONE_ROW_STARTS[row] + col;
                let key_index = (board * LUMATONE_KEYS_PER_BOARD + local) as u32;
                let x = board_origin.x + row as f32 * row_skew + col as f32 * col_spacing;
                let y = board_origin.y + row as f32 * row_spacing;
                let center = egui::pos2(x, y);
                let key = map.as_deref().and_then(|map| map.key(key_index));
                let captured_order = captured_keys.get(&key_index).copied();
                let fill = key
                    .and_then(|key| key.color)
                    .map(|[red, green, blue]| egui::Color32::from_rgb(red, green, blue))
                    .unwrap_or_else(|| egui::Color32::from_gray(62));
                let active = active_notes.contains(&key_index);
                let stroke = if active {
                    egui::Stroke::new(3.0, egui::Color32::WHITE)
                } else if captured_order.is_some() {
                    egui::Stroke::new(3.0, egui::Color32::from_rgb(255, 208, 64))
                } else {
                    egui::Stroke::new(1.0, egui::Color32::from_gray(25))
                };
                let points = (0..6)
                    .map(|idx| {
                        let angle = (30.0 + idx as f32 * 60.0).to_radians();
                        UiPoint::new(center.x + size * angle.cos(), center.y + size * angle.sin())
                    })
                    .collect();
                key_primitives.push(ScenePrimitive::Polygon {
                    points,
                    fill: operad_bridge::color_from_egui(fill),
                    stroke: Some(operad_bridge::stroke_from_egui(stroke)),
                });
                if captured_order.is_some() {
                    key_primitives.push(operad_bridge::circle_primitive(
                        egui::Pos2::ZERO,
                        center,
                        (size * 0.48).clamp(4.0, 8.0),
                        egui::Color32::from_rgb(255, 208, 64),
                        Some(egui::Stroke::new(1.0, egui::Color32::BLACK)),
                    ));
                }
            }
        }
    }

    operad_bridge::paint_scene(ui, rect, "lumatone.keys", key_primitives);

    let painter = ui.painter_at(rect);

    for board in 0..LUMATONE_BOARDS {
        let board_origin = egui::pos2(
            rect.min.x + padding + width / 2.0 + board as f32 * board_stride,
            rect.min.y + padding + size,
        );
        for (row, count) in LUMATONE_ROW_COUNTS.iter().copied().enumerate() {
            for col in 0..count {
                let local = LUMATONE_ROW_STARTS[row] + col;
                let key_index = (board * LUMATONE_KEYS_PER_BOARD + local) as u32;
                let x = board_origin.x + row as f32 * row_skew + col as f32 * col_spacing;
                let y = board_origin.y + row as f32 * row_spacing;
                let center = egui::pos2(x, y);
                let key = map.as_deref().and_then(|map| map.key(key_index));
                let captured_order = captured_keys.get(&key_index).copied();
                let fill = key
                    .and_then(|key| key.color)
                    .map(|[red, green, blue]| egui::Color32::from_rgb(red, green, blue))
                    .unwrap_or_else(|| egui::Color32::from_gray(62));
                if let Some(order) = captured_order {
                    draw_capture_marker(&painter, center, order, size);
                } else if show_labels {
                    draw_key_label(&painter, center, fill, key, scale, key_index, size);
                }
            }
        }
    }
}

fn draw_capture_marker(painter: &egui::Painter, center: egui::Pos2, order: usize, key_size: f32) {
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        format!("{order}"),
        egui::FontId::monospace((key_size * 0.58).clamp(5.0, 8.0)),
        egui::Color32::BLACK,
    );
}

fn draw_key_label(
    painter: &egui::Painter,
    center: egui::Pos2,
    fill: egui::Color32,
    key: Option<&crate::midi::LumatoneKey>,
    scale: &ScaleState,
    key_index: u32,
    key_size: f32,
) {
    let text_color = if color_luma(fill) > 150 {
        egui::Color32::BLACK
    } else {
        egui::Color32::WHITE
    };
    let label = key
        .map(|key| format!("{}\n{}", key.channel + 1, key.midi_note))
        .or_else(|| {
            scale
                .note_info(key_index as i32)
                .map(|info| format!("{}", info.degree))
        });
    if let Some(label) = label {
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace((key_size * 0.54).clamp(5.0, 7.0)),
            text_color,
        );
    }
}

fn open_scala_file(app: &mut AppState, add_to_library: bool) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Scala", &["scl"])
        .pick_file()
        && let Err(err) = app.load_scale_path(path, add_to_library)
    {
        app.last_status = format!("Scala parse error: {err}");
    }
}

fn import_audio_asset(app: &mut AppState) {
    let kind = app.selected_audio_asset_kind;
    if let Some(path) = rfd::FileDialog::new()
        .add_filter(kind.label(), kind.extensions())
        .pick_file()
    {
        app.import_audio_asset_path(path, kind);
    }
}

fn open_lumatone_file(app: &mut AppState) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Key Map", &["ltn"])
        .pick_file()
    {
        app.load_lumatone_path(path);
    }
}

fn open_project_file(app: &mut AppState) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Orbifold Project", &["orbifold", "mtdaw"])
        .pick_file()
    {
        app.load_project_path(path);
    }
}

fn save_project_as(app: &mut AppState) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Orbifold Project", &["orbifold", "mtdaw"])
        .set_file_name("project.orbifold")
        .save_file()
    {
        app.save_project_to_path(path);
    }
}

fn handle_screenshot_events(ctx: &egui::Context, app: &mut AppState) {
    let events = ctx.input(|input| input.events.clone());
    for event in events {
        if let egui::Event::Screenshot { image, .. } = event {
            match save_screenshot(&image) {
                Ok(path) => {
                    app.last_status = format!("Saved screenshot: {}", path.display());
                    if app.exit_after_screenshot {
                        std::process::exit(0);
                    }
                }
                Err(err) => {
                    app.last_status = format!("Screenshot save error: {err}");
                }
            }
        }
    }
}

fn request_screenshot(ctx: &egui::Context, app: &mut AppState, exit_after_screenshot: bool) {
    app.screenshot_requested = true;
    app.exit_after_screenshot = exit_after_screenshot;
    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
    app.last_status = "Screenshot requested".to_string();
}

fn save_screenshot(image: &egui::ColorImage) -> Result<PathBuf, String> {
    let dir = PathBuf::from("screenshots");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_millis();
    let path = dir.join(format!("ui-{stamp}.png"));
    write_png(image, &path)?;
    let latest = dir.join("latest.png");
    std::fs::copy(&path, &latest).map_err(|err| err.to_string())?;
    Ok(path)
}

fn write_png(image: &egui::ColorImage, path: &std::path::Path) -> Result<(), String> {
    let mut rgba = Vec::with_capacity(image.pixels.len() * 4);
    for pixel in &image.pixels {
        rgba.extend_from_slice(&pixel.to_srgba_unmultiplied());
    }

    let buffer = image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, rgba)
        .ok_or_else(|| "Screenshot buffer size did not match image dimensions".to_string())?;
    buffer.save(path).map_err(|err| err.to_string())
}

fn color_luma(color: egui::Color32) -> u8 {
    ((0.2126 * color.r() as f32) + (0.7152 * color.g() as f32) + (0.0722 * color.b() as f32))
        .round() as u8
}

fn format_freq(freq: f32) -> String {
    format!("{freq:.2} Hz")
}

fn mapping_label(event: &MidiEvent) -> String {
    if event.mapped_from_lumatone {
        if let Some(position) = lumatone_position_for_key_index(event.key_index) {
            format!(
                "key {} B{} R{} C{}",
                event.key_index,
                position.board + 1,
                position.row + 1,
                position.col + 1
            )
        } else {
            format!("key {}", event.key_index)
        }
    } else {
        format!("raw {}", event.key_index)
    }
}

fn visual_position_label(event: &MidiEvent) -> String {
    if !event.mapped_from_lumatone {
        return "-".to_string();
    }
    lumatone_position_for_key_index(event.key_index)
        .map(|position| {
            format!(
                "B{} R{} C{}",
                position.board + 1,
                position.row + 1,
                position.col + 1
            )
        })
        .unwrap_or_else(|| "-".to_string())
}

fn capture_mapping_label(event: &MidiEvent) -> String {
    if event.mapped_from_lumatone {
        format!("key {}", event.key_index)
    } else {
        "unmapped".to_string()
    }
}

fn capture_events_to_tsv(events: &[MidiEvent]) -> String {
    let mut text = "#\tRaw\tMap\tVisual\tPitch\n".to_string();
    for (idx, event) in events.iter().enumerate() {
        text.push_str(&format!(
            "{}\t{}:{}\t{}\t{}\t{}\n",
            idx + 1,
            event.channel + 1,
            event.midi_note,
            capture_mapping_label(event),
            visual_position_label(event),
            event.musical_note
        ));
    }
    text
}

fn captured_key_orders(events: &[MidiEvent]) -> HashMap<u32, usize> {
    let mut captured = HashMap::new();
    for (idx, event) in events.iter().enumerate() {
        if !event.mapped_from_lumatone || event.key_index < 0 {
            continue;
        }
        captured.entry(event.key_index as u32).or_insert(idx + 1);
    }
    captured
}

fn key_size_for_available_width(available_width: f32) -> f32 {
    const NATURAL_KEY_SIZE: f32 = 13.0;
    if !available_width.is_finite() || available_width <= 0.0 {
        return NATURAL_KEY_SIZE;
    }
    let natural_width = lumatone_total_width(NATURAL_KEY_SIZE);
    let fit_size = NATURAL_KEY_SIZE * (available_width / natural_width);
    fit_size.clamp(8.0, NATURAL_KEY_SIZE)
}

fn lumatone_total_width(key_size: f32) -> f32 {
    let width = (3.0_f32).sqrt() * key_size;
    let col_spacing = width;
    let row_skew = col_spacing / 2.0;
    let board_stride = LUMATONE_BOARD_COLUMNS * col_spacing;
    let padding = key_size;
    let max_center_x = (0..LUMATONE_BOARDS)
        .flat_map(|board| {
            LUMATONE_ROW_COUNTS
                .iter()
                .enumerate()
                .map(move |(row, count)| {
                    board as f32 * board_stride
                        + row as f32 * row_skew
                        + (*count as f32 - 1.0) * col_spacing
                })
        })
        .fold(0.0_f32, f32::max);
    max_center_x + width + padding * 2.0
}

fn lumatone_position_for_key_index(key_index: i32) -> Option<LumatoneGridPosition> {
    if key_index < 0 {
        return None;
    }
    let key_index = key_index as usize;
    let board = key_index / LUMATONE_KEYS_PER_BOARD;
    if board >= LUMATONE_BOARDS {
        return None;
    }
    let local = key_index % LUMATONE_KEYS_PER_BOARD;
    lumatone_position_for_local_key(local).map(|(row, col)| LumatoneGridPosition {
        board,
        row,
        col,
    })
}

fn lumatone_position_for_local_key(local: usize) -> Option<(usize, usize)> {
    LUMATONE_ROW_STARTS
        .iter()
        .zip(LUMATONE_ROW_COUNTS.iter())
        .enumerate()
        .find_map(|(row, (start, count))| {
            let end = *start + *count;
            (local >= *start && local < end).then_some((row, local - start))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lumatone_local_keys_follow_physical_row_order() {
        assert_eq!(lumatone_position_for_local_key(0), Some((0, 0)));
        assert_eq!(lumatone_position_for_local_key(1), Some((0, 1)));
        assert_eq!(lumatone_position_for_local_key(2), Some((1, 0)));
        assert_eq!(lumatone_position_for_local_key(7), Some((2, 0)));
        assert_eq!(lumatone_position_for_local_key(23), Some((4, 4)));
        assert_eq!(lumatone_position_for_local_key(24), Some((4, 5)));
        assert_eq!(lumatone_position_for_local_key(55), Some((10, 1)));
        assert_eq!(lumatone_position_for_local_key(56), None);
    }

    #[test]
    fn lumatone_global_keys_include_board_position() {
        assert_eq!(
            lumatone_position_for_key_index(23),
            Some(LumatoneGridPosition {
                board: 0,
                row: 4,
                col: 4,
            })
        );
        assert_eq!(
            lumatone_position_for_key_index(LUMATONE_KEYS_PER_BOARD as i32 + 19),
            Some(LumatoneGridPosition {
                board: 1,
                row: 4,
                col: 0,
            })
        );
        assert_eq!(lumatone_position_for_key_index(-1), None);
        assert_eq!(lumatone_position_for_key_index(280), None);
    }
}
