use operad::{KeyCode, UiInputEvent, UiPoint, WidgetTextEdit};
#[cfg(feature = "native-app")]
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::app::{AppState, AudioAssetKind};
use crate::project::QuantizeGrid;
use crate::synth::{SynthSettings, Waveform};
use crate::time::AppInstant;

use super::labels::selected_name_matches_connected;
#[cfg(any(feature = "native-app", feature = "web-app"))]
use super::native::SurfaceRects;

#[cfg(not(any(feature = "native-app", feature = "web-app")))]
#[derive(Clone, Copy, Debug)]
pub(super) struct SurfaceRects;

#[cfg(not(any(feature = "native-app", feature = "web-app")))]
impl SurfaceRects {
    fn arrangement_beat_at(self, _point: UiPoint) -> f32 {
        0.0
    }

    fn piano_ruler_beat_at(self, _point: UiPoint) -> f32 {
        0.0
    }

    fn beat_at(self, _point: UiPoint) -> f32 {
        0.0
    }

    fn pitch_at(self, _point: UiPoint) -> i32 {
        60
    }
}

#[cfg(feature = "native-app")]
pub(super) fn handle_key(
    app: &mut AppState,
    key: &Key,
    modifiers: ModifiersState,
    repeat: bool,
) -> bool {
    let command = modifiers.control_key() || modifiers.super_key();
    let alt = modifiers.alt_key();
    let command_shortcut = command && !alt;
    let plain_shortcut = !command && !alt;
    let shift = modifiers.shift_key();
    if repeat && !key_repeat_allowed(key, modifiers) {
        return false;
    }
    match key {
        Key::Character(value)
            if command_shortcut && shift && value.as_ref().eq_ignore_ascii_case("s") =>
        {
            save_project_as(app);
            true
        }
        Key::Character(value)
            if command_shortcut && !shift && value.as_ref().eq_ignore_ascii_case("s") =>
        {
            save_project(app);
            true
        }
        Key::Character(value) if command_shortcut && value.as_ref().eq_ignore_ascii_case("n") => {
            app.start_new_project();
            true
        }
        Key::Character(value) if command_shortcut && value.as_ref().eq_ignore_ascii_case("o") => {
            open_project(app);
            true
        }
        Key::Character(value)
            if command_shortcut && !shift && value.as_ref().eq_ignore_ascii_case("c") =>
        {
            app.copy_selected_clip_note();
            true
        }
        Key::Character(value)
            if command_shortcut && !shift && value.as_ref().eq_ignore_ascii_case("v") =>
        {
            app.paste_copied_clip_note_at_playhead();
            true
        }
        Key::Character(value) if command_shortcut && matches!(value.as_ref(), "+" | "=") => {
            app.adjust_ui_scale(0.1);
            true
        }
        Key::Character(value) if command_shortcut && value.as_ref() == "-" => {
            app.adjust_ui_scale(-0.1);
            true
        }
        Key::Character(value) if command_shortcut && value.as_ref() == "0" => {
            app.reset_ui_scale();
            true
        }
        Key::Character(value) if plain_shortcut && matches!(value.as_ref(), "+" | "=") => {
            zoom_piano_roll_at_playhead(app, 1.0);
            true
        }
        Key::Character(value) if plain_shortcut && value.as_ref() == "-" => {
            zoom_piano_roll_at_playhead(app, -1.0);
            true
        }
        Key::Character(value)
            if command_shortcut && !shift && value.as_ref().eq_ignore_ascii_case("z") =>
        {
            app.undo_project_edit();
            true
        }
        Key::Character(value)
            if command_shortcut
                && (value.as_ref().eq_ignore_ascii_case("y")
                    || (shift && value.as_ref().eq_ignore_ascii_case("z"))) =>
        {
            app.redo_project_edit();
            true
        }
        Key::Character(value) if plain_shortcut && shortcut_help_key(value.as_ref(), shift) => {
            app.last_status = shortcut_help_status().to_string();
            true
        }
        Key::Character(value) if plain_shortcut && value.as_ref().eq_ignore_ascii_case("r") => {
            app.toggle_recording();
            true
        }
        Key::Character(value) if plain_shortcut && value.as_ref().eq_ignore_ascii_case("m") => {
            app.toggle_metronome();
            true
        }
        Key::Character(value)
            if plain_shortcut && shift && value.as_ref().eq_ignore_ascii_case("q") =>
        {
            app.toggle_quantize_on_record();
            true
        }
        Key::Character(value)
            if plain_shortcut && !shift && value.as_ref().eq_ignore_ascii_case("q") =>
        {
            app.quantize_selected_or_clip();
            true
        }
        Key::Character(value) if plain_shortcut && value.as_ref().eq_ignore_ascii_case("g") => {
            app.toggle_snap_to_grid();
            true
        }
        Key::Character(value) if plain_shortcut && value.as_ref().eq_ignore_ascii_case("p") => {
            app.all_notes_off();
            true
        }
        Key::Character(value) if plain_shortcut && value.as_ref().eq_ignore_ascii_case("d") => {
            app.duplicate_selected_clip_note();
            true
        }
        Key::Character(value)
            if plain_shortcut && !shift && value.as_ref().eq_ignore_ascii_case("n") =>
        {
            add_note_at_playhead(app);
            true
        }
        Key::Named(NamedKey::Space) if plain_shortcut => {
            app.toggle_transport();
            true
        }
        Key::Named(NamedKey::Home) if plain_shortcut => {
            app.return_transport_to_start();
            true
        }
        Key::Named(NamedKey::Escape) if plain_shortcut => {
            app.cancel_discard_confirmation() || app.clear_clip_note_selection()
        }
        Key::Named(NamedKey::Delete | NamedKey::Backspace) if plain_shortcut => {
            app.delete_selected_clip_note();
            true
        }
        Key::Named(NamedKey::ArrowLeft) if plain_shortcut && shift => {
            app.resize_selected_clip_note(-1.0);
            true
        }
        Key::Named(NamedKey::ArrowRight) if plain_shortcut && shift => {
            app.resize_selected_clip_note(1.0);
            true
        }
        Key::Named(NamedKey::ArrowDown) if plain_shortcut && shift => {
            adjust_selected_velocity(app, -8);
            true
        }
        Key::Named(NamedKey::ArrowUp) if plain_shortcut && shift => {
            adjust_selected_velocity(app, 8);
            true
        }
        Key::Named(NamedKey::ArrowLeft) if plain_shortcut => {
            app.nudge_selected_clip_note(-1.0);
            true
        }
        Key::Named(NamedKey::ArrowRight) if plain_shortcut => {
            app.nudge_selected_clip_note(1.0);
            true
        }
        Key::Named(NamedKey::ArrowDown) if plain_shortcut => {
            app.transpose_selected_clip_note(-1);
            true
        }
        Key::Named(NamedKey::ArrowUp) if plain_shortcut => {
            app.transpose_selected_clip_note(1);
            true
        }
        _ => false,
    }
}

#[cfg(feature = "native-app")]
fn shortcut_help_key(value: &str, shift: bool) -> bool {
    value == "?" || (shift && value == "/")
}

pub(super) fn shortcut_help_status() -> &'static str {
    "Shortcuts: Space play/pause | R record | N add note | +/- piano zoom | Arrows move/pitch | Shift+Arrows resize/velocity | Ctrl/Cmd+S save"
}

#[cfg(feature = "native-app")]
fn key_repeat_allowed(key: &Key, modifiers: ModifiersState) -> bool {
    let command = modifiers.control_key() || modifiers.super_key();
    let alt = modifiers.alt_key();
    !command
        && !alt
        && matches!(
            key,
            Key::Named(
                NamedKey::ArrowLeft
                    | NamedKey::ArrowRight
                    | NamedKey::ArrowDown
                    | NamedKey::ArrowUp
            )
        )
}

pub(super) fn dispatch_action(
    app: &mut AppState,
    action: &str,
    cursor: Option<UiPoint>,
    layout: Option<SurfaceRects>,
) {
    let action = canonical_action_name(action);
    log::trace!(
        target: "orbifold::ui::actions",
        "dispatch action={action} cursor={cursor:?}"
    );
    if let Some(id) = action.strip_prefix("note.select.") {
        if let Ok(id) = id.parse::<u64>() {
            app.select_clip_note(Some(id));
        }
        return;
    }
    if let Some(index) = action.strip_prefix("scale.select.") {
        if let Ok(index) = index.parse::<usize>() {
            if index < app.scale_library.len() {
                app.selected_scale_library = index;
                app.last_status = format!("Selected scale: {}", app.scale_library[index].name);
            } else {
                app.set_error_status("Selected scale unavailable");
            }
        }
        return;
    }
    if action == "scale.scroll_up" {
        select_scale_library_relative(app, -1);
        return;
    }
    if action == "scale.scroll_down" {
        select_scale_library_relative(app, 1);
        return;
    }
    if let Some(start) = action
        .strip_prefix("scale.scroll_up.")
        .or_else(|| action.strip_prefix("scale.scroll_down."))
    {
        if let Ok(start) = start.parse::<usize>() {
            app.set_scale_library_list_start(start);
        }
        return;
    }
    if let Some(index) = action.strip_prefix("asset.select.") {
        if let Ok(index) = index.parse::<usize>() {
            app.select_audio_asset(index);
        }
        return;
    }
    if action == "asset.scroll_up" {
        select_audio_asset_relative(app, -1);
        return;
    }
    if action == "asset.scroll_down" {
        select_audio_asset_relative(app, 1);
        return;
    }
    if let Some(start) = action
        .strip_prefix("asset.scroll_up.")
        .or_else(|| action.strip_prefix("asset.scroll_down."))
    {
        if let Ok(start) = start.parse::<usize>() {
            app.set_audio_asset_list_start(app.selected_audio_asset_kind, start);
        }
        return;
    }
    if let Some(index) = action.strip_prefix("asset.kind.") {
        if let Ok(index) = index.parse::<usize>() {
            select_audio_asset_kind(app, index);
        }
        return;
    }
    if let Some(index) = action.strip_prefix("file.open_recent.") {
        if let Ok(index) = index.parse::<usize>() {
            app.open_recent_project_at(index);
        }
        return;
    }
    if let Some(index) = action.strip_prefix("file.forget_recent.") {
        if let Ok(index) = index.parse::<usize>() {
            app.forget_recent_project_at(index);
        }
        return;
    }
    if let Some(index) = action.strip_prefix("midi.select.") {
        if let Ok(index) = index.parse::<usize>() {
            select_midi_input_at(app, index);
        }
        return;
    }
    if let Some(index) = action.strip_prefix("audio.select.") {
        if let Ok(index) = index.parse::<usize>() {
            select_audio_output_at(app, index);
        }
        return;
    }
    match action {
        "file.new" => app.start_new_project(),
        "file.open" => open_project(app),
        "file.open_recent" => app.open_most_recent_project(),
        "file.forget_recent" => app.forget_most_recent_project(),
        "file.save" => save_project(app),
        "file.save_as" => save_project_as(app),
        "file.recover" => app.recover_autosave_project(),
        "file.dismiss_autosave" => app.dismiss_autosave_project(),
        "scale.open" | "scale.import" => open_scale(app),
        "keymap.open" => open_keymap(app),
        "settings.save" => app.persist_settings_with_status(),
        "diagnostics.clear" => app.clear_diagnostics(),
        "edit.undo" => app.undo_project_edit(),
        "edit.redo" => app.redo_project_edit(),
        "edit.escape" => {
            let _ = app.cancel_discard_confirmation() || app.clear_clip_note_selection();
        }
        "help.shortcuts" => {
            app.last_status = shortcut_help_status().to_string();
        }
        "transport.prev" => app.return_transport_to_start(),
        "transport.stop" => app.stop_transport(),
        "transport.play_stop" => app.toggle_transport(),
        "transport.record" => app.toggle_recording(),
        "transport.loop" => toggle_overdub(app),
        "transport.metronome" => app.toggle_metronome(),
        "transport.record_quantize" => app.toggle_quantize_on_record(),
        "transport.seek" => {
            if let (Some(point), Some(layout)) = (cursor, layout) {
                app.seek_transport_to(layout.arrangement_beat_at(point));
            }
        }
        "clip.select_current" => app.select_current_clip(),
        "piano.seek" => {
            if let (Some(point), Some(layout)) = (cursor, layout) {
                app.seek_transport_to(layout.piano_ruler_beat_at(point));
            }
        }
        "transport.bpm_down" => adjust_bpm(app, -1.0),
        "transport.bpm_up" => adjust_bpm(app, 1.0),
        "transport.loop_down" => adjust_loop_beats(app, -4.0),
        "transport.loop_up" => adjust_loop_beats(app, 4.0),
        "transport.quantize_grid" => cycle_quantize_grid(app),
        "transport.quantize_grid_prev" => adjust_quantize_grid(app, -1),
        "transport.quantize_grid_next" => adjust_quantize_grid(app, 1),
        "transport.snap" => app.toggle_snap_to_grid(),
        "ui.scale_down" => app.adjust_ui_scale(-0.1),
        "ui.scale_reset" => app.reset_ui_scale(),
        "ui.scale_up" => app.adjust_ui_scale(0.1),
        "view.assets" => app.toggle_asset_browser(),
        "view.clip" => app.toggle_clip_panel(),
        "view.devices" => app.toggle_device_panel(),
        "view.reset_layout" => app.reset_workspace_layout(),
        "view.scales" => app.toggle_scale_browser(),
        "view.settings" => app.toggle_settings_panel(),
        "piano.pitch_labels" => app.toggle_piano_pitch_label_mode(),
        "audio.all_off" => app.all_notes_off(),
        "audio.test_a4" => app.test_tone(),
        "scale.root_down" => adjust_root_midi(app, -1),
        "scale.root_up" => adjust_root_midi(app, 1),
        "scale.base_down" => adjust_base_freq(app, -1.0),
        "scale.base_up" => adjust_base_freq(app, 1.0),
        "scale.load_selected" => app.load_selected_library_scale(),
        "scale.refresh" => app.refresh_scale_library(),
        "scale.remove_selected" => app.remove_selected_library_scale(),
        "scale.search_clear" => {
            let _ = app.set_scale_library_search_query("");
        }
        "asset.preview" => app.preview_selected_audio_asset(),
        "asset.stop_preview" => app.stop_audio_asset_preview(),
        "asset.use_sample" => app.load_selected_sample_instrument(),
        "asset.clear_sample" => app.clear_sample_instrument(),
        "asset.search_clear" => {
            let _ = app.set_audio_asset_search_query("");
        }
        "asset.refresh" => app.refresh_audio_assets(),
        "asset.import" => import_audio_asset(app),
        "capture.start" => app.start_mapping_capture(),
        "capture.stop" => app.stop_mapping_capture(),
        "capture.clear" => app.clear_mapping_capture(),
        "keymap.prev" => select_lumatone_preset(app, -1),
        "keymap.next" => select_lumatone_preset(app, 1),
        "keymap.refresh" => app.reload_lumatone_presets(),
        "synth.waveform" => cycle_waveform(app),
        "synth.waveform_prev" => adjust_waveform(app, -1),
        "synth.waveform_next" => adjust_waveform(app, 1),
        "synth.clear_sample" => app.clear_sample_instrument(),
        "synth.mute" => app.toggle_audio_mute(),
        "synth.reset" => reset_synth_settings(app),
        "synth.gain_down" => adjust_synth(app, |settings| {
            settings.master_gain = (settings.master_gain - 0.05).clamp(0.0, 1.0);
        }),
        "synth.gain_up" => adjust_synth(app, |settings| {
            settings.master_gain = (settings.master_gain + 0.05).clamp(0.0, 1.0);
        }),
        "synth.attack_down" => adjust_synth(app, |settings| {
            settings.attack_ms = (settings.attack_ms - 5.0).clamp(0.0, 2_000.0);
        }),
        "synth.attack_up" => adjust_synth(app, |settings| {
            settings.attack_ms = (settings.attack_ms + 5.0).clamp(0.0, 2_000.0);
        }),
        "synth.release_down" => adjust_synth(app, |settings| {
            settings.release_ms = (settings.release_ms - 10.0).clamp(5.0, 5_000.0);
        }),
        "synth.release_up" => adjust_synth(app, |settings| {
            settings.release_ms = (settings.release_ms + 10.0).clamp(5.0, 5_000.0);
        }),
        "synth.filter_down" => adjust_synth(app, |settings| {
            settings.filter_cutoff_hz = (settings.filter_cutoff_hz * 0.9).clamp(80.0, 20_000.0);
        }),
        "synth.filter_up" => adjust_synth(app, |settings| {
            settings.filter_cutoff_hz = (settings.filter_cutoff_hz * 1.1).clamp(80.0, 20_000.0);
        }),
        "synth.delay_down" => adjust_synth(app, |settings| {
            settings.delay_mix = (settings.delay_mix - 0.05).clamp(0.0, 1.0);
        }),
        "synth.delay_up" => adjust_synth(app, |settings| {
            settings.delay_mix = (settings.delay_mix + 0.05).clamp(0.0, 1.0);
        }),
        "synth.drive_down" => adjust_synth(app, |settings| {
            settings.drive = (settings.drive - 0.1).clamp(0.5, 8.0);
        }),
        "synth.drive_up" => adjust_synth(app, |settings| {
            settings.drive = (settings.drive + 0.1).clamp(0.5, 8.0);
        }),
        "midi.prev" => select_midi_input(app, -1),
        "midi.next" => select_midi_input(app, 1),
        "midi.refresh" => app.refresh_midi_inputs_with_status(None),
        "midi.connect" => app.open_midi_input(),
        "midi.channel_filter" => app.cycle_midi_channel_filter(),
        "midi.channel_filter_prev" => app.adjust_midi_channel_filter(-1),
        "midi.channel_filter_next" => app.adjust_midi_channel_filter(1),
        "audio.prev" => select_audio_output(app, -1),
        "audio.next" => select_audio_output(app, 1),
        "audio.refresh" => app.refresh_audio_outputs(),
        "audio.connect" => app.connect_audio_output(),
        "clip.add_note" => add_note_at_playhead(app),
        "clip.copy_note" => app.copy_selected_clip_note(),
        "clip.paste_note" => app.paste_copied_clip_note_at_playhead(),
        "clip.delete_note" => app.delete_selected_clip_note(),
        "clip.duplicate_note" => app.duplicate_selected_clip_note(),
        "clip.nudge_left" => app.nudge_selected_clip_note(-1.0),
        "clip.nudge_right" => app.nudge_selected_clip_note(1.0),
        "clip.pitch_down" => app.transpose_selected_clip_note(-1),
        "clip.pitch_up" => app.transpose_selected_clip_note(1),
        "clip.shorter" => app.resize_selected_clip_note(-1.0),
        "clip.longer" => app.resize_selected_clip_note(1.0),
        "clip.velocity_down" => adjust_selected_velocity(app, -8),
        "clip.velocity_up" => adjust_selected_velocity(app, 8),
        "clip.quantize" => app.quantize_selected_or_clip(),
        "clip.clear" => app.clear_clip(),
        "piano.zoom_in" => zoom_piano_roll_at_playhead(app, 1.0),
        "piano.zoom_out" => zoom_piano_roll_at_playhead(app, -1.0),
        "piano.fit_view" => {
            app.fit_piano_roll_view();
        }
        "piano.pitch_zoom_in" => zoom_piano_roll_pitches_around_selection(app, 1.0),
        "piano.pitch_zoom_out" => zoom_piano_roll_pitches_around_selection(app, -1.0),
        "piano.grid" => {
            if let (Some(point), Some(layout)) = (cursor, layout) {
                app.add_clip_note_at(layout.beat_at(point), layout.pitch_at(point));
            }
        }
        _ => {}
    }
}

pub(super) fn canonical_action_name(action: &str) -> &str {
    match action {
        "piano.view.clip" => "view.clip",
        "piano.view.scales" => "view.scales",
        "piano.transport.quantize_grid" => "transport.quantize_grid",
        "piano.transport.quantize_grid_prev" => "transport.quantize_grid_prev",
        "piano.transport.quantize_grid_next" => "transport.quantize_grid_next",
        "piano.transport.snap" => "transport.snap",
        "project.file.open" => "file.open",
        "project.file.save" => "file.save",
        "settings.panel.save" => "settings.save",
        "settings.ui.scale_down" => "ui.scale_down",
        "settings.ui.scale_reset" => "ui.scale_reset",
        "settings.ui.scale_up" => "ui.scale_up",
        "settings.view.assets" => "view.assets",
        "settings.view.clip" => "view.clip",
        "settings.view.devices" => "view.devices",
        "settings.view.reset_layout" => "view.reset_layout",
        "settings.view.scales" => "view.scales",
        "settings.diagnostics.clear" => "diagnostics.clear",
        _ => action,
    }
}

pub(super) fn handle_text_edit_action(app: &mut AppState, action: &str, edit: WidgetTextEdit) {
    match action {
        "scale.search" => handle_scale_search_text_edit(app, edit),
        "scale.root_input" => handle_root_midi_text_edit(app, edit),
        "scale.base_input" => handle_base_freq_text_edit(app, edit),
        "asset.search" => handle_asset_search_text_edit(app, edit),
        "transport.bpm_input" => handle_bpm_text_edit(app, edit),
        _ => {}
    }
}

fn handle_scale_search_text_edit(app: &mut AppState, edit: WidgetTextEdit) {
    if edit.local_position.is_some() {
        return;
    }
    let mut query = app.scale_library_search_query().to_string();
    match edit.event {
        UiInputEvent::TextInput(text) => {
            query.push_str(&text);
            let _ = app.set_scale_library_search_query(query);
        }
        UiInputEvent::Key {
            key: KeyCode::Backspace,
            ..
        } => {
            query.pop();
            let _ = app.set_scale_library_search_query(query);
        }
        UiInputEvent::Key {
            key: KeyCode::Delete | KeyCode::Escape,
            ..
        } => {
            let _ = app.set_scale_library_search_query("");
        }
        UiInputEvent::Key {
            key: KeyCode::Enter,
            ..
        } => {
            let matches = app.filtered_scale_library_count();
            app.last_status = if app.scale_library_search_query().is_empty() {
                "Scale search ready".to_string()
            } else {
                format!(
                    "Scale search: {} ({matches} matches)",
                    app.scale_library_search_query()
                )
            };
        }
        _ => {}
    }
}

fn handle_asset_search_text_edit(app: &mut AppState, edit: WidgetTextEdit) {
    if edit.local_position.is_some() {
        return;
    }
    let mut query = app.audio_asset_search_query().to_string();
    match edit.event {
        UiInputEvent::TextInput(text) => {
            query.push_str(&text);
            let _ = app.set_audio_asset_search_query(query);
        }
        UiInputEvent::Key {
            key: KeyCode::Backspace,
            ..
        } => {
            query.pop();
            let _ = app.set_audio_asset_search_query(query);
        }
        UiInputEvent::Key {
            key: KeyCode::Delete | KeyCode::Escape,
            ..
        } => {
            let _ = app.set_audio_asset_search_query("");
        }
        UiInputEvent::Key {
            key: KeyCode::Enter,
            ..
        } => {
            let matches = app.filtered_audio_asset_count(app.selected_audio_asset_kind);
            app.last_status = if app.audio_asset_search_query().is_empty() {
                format!("{} search ready", app.selected_audio_asset_kind.label())
            } else {
                format!(
                    "Asset search: {} ({matches} matches)",
                    app.audio_asset_search_query()
                )
            };
        }
        _ => {}
    }
}

fn handle_base_freq_text_edit(app: &mut AppState, edit: WidgetTextEdit) {
    if edit.local_position.is_some() {
        app.clear_base_freq_edit_text();
        return;
    }
    match edit.event {
        UiInputEvent::TextInput(text) => app.append_base_freq_edit_text(&text),
        UiInputEvent::Key {
            key: KeyCode::Backspace,
            ..
        } => app.backspace_base_freq_edit_text(),
        UiInputEvent::Key {
            key: KeyCode::Delete,
            ..
        } => app.clear_base_freq_edit_text(),
        UiInputEvent::Key {
            key: KeyCode::Escape,
            ..
        } => app.cancel_base_freq_edit_text(),
        UiInputEvent::Key {
            key: KeyCode::Enter,
            ..
        } => {
            let _ = app.commit_base_freq_edit_text();
        }
        _ => {}
    }
}

fn handle_root_midi_text_edit(app: &mut AppState, edit: WidgetTextEdit) {
    if edit.local_position.is_some() {
        app.clear_root_midi_edit_text();
        return;
    }
    match edit.event {
        UiInputEvent::TextInput(text) => app.append_root_midi_edit_text(&text),
        UiInputEvent::Key {
            key: KeyCode::Backspace,
            ..
        } => app.backspace_root_midi_edit_text(),
        UiInputEvent::Key {
            key: KeyCode::Delete,
            ..
        } => app.clear_root_midi_edit_text(),
        UiInputEvent::Key {
            key: KeyCode::Escape,
            ..
        } => app.cancel_root_midi_edit_text(),
        UiInputEvent::Key {
            key: KeyCode::Enter,
            ..
        } => {
            let _ = app.commit_root_midi_edit_text();
        }
        _ => {}
    }
}

fn handle_bpm_text_edit(app: &mut AppState, edit: WidgetTextEdit) {
    if edit.local_position.is_some() {
        app.clear_bpm_edit_text();
        return;
    }
    match edit.event {
        UiInputEvent::TextInput(text) => app.append_bpm_edit_text(&text),
        UiInputEvent::Key {
            key: KeyCode::Backspace,
            ..
        } => app.backspace_bpm_edit_text(),
        UiInputEvent::Key {
            key: KeyCode::Delete,
            ..
        } => app.clear_bpm_edit_text(),
        UiInputEvent::Key {
            key: KeyCode::Escape,
            ..
        } => app.cancel_bpm_edit_text(),
        UiInputEvent::Key {
            key: KeyCode::Enter,
            ..
        } => {
            let _ = app.commit_bpm_edit_text();
        }
        _ => {}
    }
}

fn save_project(app: &mut AppState) {
    if app.save_project() {
        return;
    }
    save_project_with_dialog(app, "project.orbifold", "Save cancelled");
}

fn save_project_as(app: &mut AppState) {
    let file_name = app
        .project_path
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("project.orbifold")
        .to_string();
    save_project_with_dialog(app, &file_name, "Save As cancelled");
}

fn save_project_with_dialog(app: &mut AppState, file_name: &str, cancel_status: &str) {
    app.request_save_project_dialog(file_name, cancel_status);
}

fn open_project(app: &mut AppState) {
    app.request_open_project_dialog();
}

fn open_scale(app: &mut AppState) {
    app.request_open_scale_dialog();
}

fn open_keymap(app: &mut AppState) {
    app.request_open_keymap_dialog();
}

fn select_audio_asset_kind(app: &mut AppState, index: usize) {
    let Some(kind) = AudioAssetKind::all().get(index).copied() else {
        return;
    };
    app.selected_audio_asset_kind = kind;
    if app
        .selected_audio_asset_item()
        .is_some_and(|asset| app.audio_asset_matches_browser_filter(asset, kind))
    {
        app.last_status = format!("Showing {}", kind.label());
        return;
    }
    let Some(index) = app
        .audio_assets
        .iter()
        .position(|asset| app.audio_asset_matches_browser_filter(asset, kind))
    else {
        app.selected_audio_asset = None;
        app.last_status = if app.audio_asset_search_query().is_empty() {
            format!("No {} found", kind.label())
        } else {
            format!(
                "No {} match {}",
                kind.label(),
                app.audio_asset_search_query()
            )
        };
        return;
    };
    app.select_audio_asset(index);
}

fn select_scale_library_relative(app: &mut AppState, direction: isize) {
    if app.scale_library_search_query().is_empty() {
        select_unfiltered_scale_library_relative(app, direction);
        return;
    }
    let indices = app.filtered_scale_library_indices();
    if indices.is_empty() {
        app.last_status = if app.scale_library_search_query().is_empty() {
            "No scales found".to_string()
        } else {
            format!("No scales match {}", app.scale_library_search_query())
        };
        return;
    }
    let Some(current_pos) = indices
        .iter()
        .position(|index| *index == app.selected_scale_library)
    else {
        let index = indices[0];
        app.selected_scale_library = index;
        app.last_status = format!("Selected scale: {}", app.scale_library[index].name);
        return;
    };
    let next_pos = clamp_index(current_pos, indices.len(), direction);
    let current = indices[current_pos];
    if next_pos == current_pos {
        let boundary = if direction < 0 { "First" } else { "Last" };
        app.last_status = format!(
            "{boundary} scale selected: {}",
            app.scale_library[current].name
        );
        return;
    }
    let next = indices[next_pos];
    app.selected_scale_library = next;
    app.last_status = format!("Selected scale: {}", app.scale_library[next].name);
}

fn select_unfiltered_scale_library_relative(app: &mut AppState, direction: isize) {
    if app.scale_library.is_empty() {
        app.last_status = "No scales found".to_string();
        return;
    }
    let current = app
        .selected_scale_library
        .min(app.scale_library.len().saturating_sub(1));
    if app.selected_scale_library != current {
        app.selected_scale_library = current;
        app.last_status = format!("Selected scale: {}", app.scale_library[current].name);
        return;
    }
    let next = clamp_index(current, app.scale_library.len(), direction);
    if next == current {
        let boundary = if direction < 0 { "First" } else { "Last" };
        app.last_status = format!(
            "{boundary} scale selected: {}",
            app.scale_library[current].name
        );
        return;
    }
    app.selected_scale_library = next;
    app.last_status = format!("Selected scale: {}", app.scale_library[next].name);
}

fn select_audio_asset_relative(app: &mut AppState, direction: isize) {
    let indices = app
        .audio_assets
        .iter()
        .enumerate()
        .filter_map(|(idx, asset)| {
            app.audio_asset_matches_browser_filter(asset, app.selected_audio_asset_kind)
                .then_some(idx)
        })
        .collect::<Vec<_>>();
    if indices.is_empty() {
        app.last_status = if app.audio_asset_search_query().is_empty() {
            format!("No {} found", app.selected_audio_asset_kind.label())
        } else {
            format!(
                "No {} match {}",
                app.selected_audio_asset_kind.label(),
                app.audio_asset_search_query()
            )
        };
        return;
    }
    let Some(current_pos) = app
        .selected_audio_asset
        .and_then(|selected| indices.iter().position(|idx| *idx == selected))
    else {
        app.select_audio_asset(indices[0]);
        return;
    };
    let next_pos = clamp_index(current_pos, indices.len(), direction);
    if next_pos == current_pos {
        let asset = &app.audio_assets[indices[current_pos]];
        let boundary = if direction < 0 { "First" } else { "Last" };
        app.last_status = format!(
            "{boundary} {} selected: {}",
            asset.kind.singular_label(),
            asset.name
        );
        return;
    }
    app.select_audio_asset(indices[next_pos]);
}

fn import_audio_asset(app: &mut AppState) {
    let kind = app.selected_audio_asset_kind;
    app.request_import_audio_asset_dialog(kind);
}

fn select_midi_input(app: &mut AppState, direction: isize) {
    if app.midi_inputs.is_empty() {
        app.set_error_status("No MIDI inputs found");
        return;
    }
    let index = wrap_index(app.selected_input, app.midi_inputs.len(), direction);
    select_midi_input_at(app, index);
}

fn select_midi_input_at(app: &mut AppState, index: usize) {
    let Some(name) = app.midi_inputs.get(index).cloned() else {
        if app.midi_inputs.is_empty() {
            app.set_error_status("No MIDI inputs found");
        } else {
            app.set_error_status("Selected MIDI input unavailable");
        }
        return;
    };
    app.selected_input = index;
    let connected_name = if app.midi_connection.is_some() {
        app.connected_midi_input.as_str()
    } else {
        ""
    };
    app.last_status = selected_device_status("MIDI input", &name, connected_name);
    app.persist_current_settings();
}

fn select_audio_output(app: &mut AppState, direction: isize) {
    if app.audio_outputs.is_empty() {
        app.set_error_status("No audio outputs found");
        return;
    }
    let index = wrap_index(
        app.selected_audio_output,
        app.audio_outputs.len(),
        direction,
    );
    select_audio_output_at(app, index);
}

fn select_audio_output_at(app: &mut AppState, index: usize) {
    let Some(device) = app.audio_outputs.get(index) else {
        if app.audio_outputs.is_empty() {
            app.set_error_status("No audio outputs found");
        } else {
            app.set_error_status("Selected audio output unavailable");
        }
        return;
    };
    let name = device.name.clone();
    app.selected_audio_output = index;
    let connected_name = if app.audio_stream.is_some() {
        app.connected_audio_output.as_str()
    } else {
        ""
    };
    app.last_status = selected_device_status("audio output", &name, connected_name);
    app.persist_current_settings();
}

fn select_lumatone_preset(app: &mut AppState, direction: isize) {
    if app.lumatone_presets.is_empty() {
        app.set_error_status("No key maps found");
        return;
    }
    let previous_path = app.lumatone_path.clone();
    let index = wrap_index(app.selected_lumatone, app.lumatone_presets.len(), direction);
    app.select_lumatone(index);
    if app.lumatone_path != previous_path {
        app.mark_project_dirty();
    }
}

pub(super) fn selected_device_status(
    kind: &str,
    selected_name: &str,
    connected_name: &str,
) -> String {
    if selected_name_matches_connected(Some(selected_name), connected_name) {
        format!("Selected {kind}: {selected_name} (connected)")
    } else if !connected_name.is_empty() {
        format!("Selected {kind}: {selected_name}; click Connect to switch")
    } else {
        format!("Selected {kind}: {selected_name}")
    }
}

fn wrap_index(current: usize, len: usize, direction: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as isize;
    (current as isize + direction).rem_euclid(len) as usize
}

pub(super) fn clamp_index(current: usize, len: usize, direction: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as isize + direction).clamp(0, len.saturating_sub(1) as isize) as usize
}

fn adjust_root_midi(app: &mut AppState, delta: i32) {
    let _ = app.adjust_scale_root_midi(delta);
}

fn adjust_base_freq(app: &mut AppState, delta: f32) {
    let _ = app.adjust_scale_base_freq(delta);
}

fn cycle_waveform(app: &mut AppState) {
    adjust_waveform(app, 1);
}

fn adjust_waveform(app: &mut AppState, direction: isize) {
    adjust_synth(app, |settings| {
        let all = Waveform::all();
        let index = all
            .iter()
            .position(|waveform| *waveform == settings.waveform)
            .unwrap_or(0);
        settings.waveform = all[wrap_index(index, all.len(), direction)];
    });
}

fn reset_synth_settings(app: &mut AppState) {
    adjust_synth(app, |settings| {
        *settings = SynthSettings::default();
    });
}

fn adjust_synth(app: &mut AppState, edit: impl FnOnce(&mut SynthSettings)) {
    let previous = app.synth.settings();
    let mut settings = previous;
    edit(&mut settings);
    if settings == previous {
        app.last_status = format!("Synth unchanged {}", synth_status(settings));
        return;
    }
    match app.synth.set_settings(settings) {
        Ok(()) => {
            app.mark_project_dirty();
            app.persist_current_settings();
            app.last_status = format!("Synth {}", synth_status(settings));
        }
        Err(err) => app.set_error_status(format!("Synth settings error: {err}")),
    }
}

fn synth_status(settings: SynthSettings) -> String {
    format!(
        "{} gain {:.0}% atk {:.0}ms rel {:.0}ms",
        settings.waveform.as_str(),
        settings.master_gain * 100.0,
        settings.attack_ms,
        settings.release_ms
    )
}

fn toggle_overdub(app: &mut AppState) {
    let value = {
        let mut project = app.music_project.lock();
        project.transport.overdub = !project.transport.overdub;
        project.transport.overdub
    };
    app.last_status = if value {
        "Recording mode: overdub".to_string()
    } else {
        "Recording mode: replace".to_string()
    };
    app.mark_project_dirty();
    app.persist_current_settings();
}

fn adjust_bpm(app: &mut AppState, delta: f32) {
    let _ = app.adjust_transport_bpm(delta);
}

fn adjust_loop_beats(app: &mut AppState, delta: f32) {
    let current = app.music_project.lock().transport.loop_beats;
    app.set_loop_beats(current + delta);
}

fn cycle_quantize_grid(app: &mut AppState) {
    let grid = next_quantize_grid(app.music_project.lock().transport.quantize_grid);
    app.set_quantize_grid(grid);
}

fn adjust_quantize_grid(app: &mut AppState, direction: isize) {
    let all = QuantizeGrid::all();
    let current = app.music_project.lock().transport.quantize_grid;
    let index = all
        .iter()
        .position(|candidate| *candidate == current)
        .unwrap_or(0);
    let next = index
        .saturating_add_signed(direction)
        .min(all.len().saturating_sub(1));
    app.set_quantize_grid(all[next]);
}

fn add_note_at_playhead(app: &mut AppState) {
    let root_midi = app.scale_state.lock().root_midi;
    let beat = app
        .music_project
        .lock()
        .current_position_beats(AppInstant::now());
    app.add_clip_note_at(beat, root_midi);
}

fn zoom_piano_roll_at_playhead(app: &mut AppState, direction: f32) {
    let beat = app
        .music_project
        .lock()
        .current_position_beats(AppInstant::now());
    if !app.zoom_piano_roll(direction, beat) {
        let loop_beats = app.music_project.lock().transport.loop_beats;
        app.last_status = format!(
            "Piano zoom unchanged {:.2} beats",
            app.piano_view_visible_beats(loop_beats)
        );
    }
}

fn zoom_piano_roll_pitches_around_selection(app: &mut AppState, direction: f32) {
    let anchor_pitch = app
        .selected_clip_note()
        .map(|note| note.musical_note)
        .unwrap_or_else(|| app.scale_state.lock().root_midi);
    if !app.zoom_piano_roll_pitches(direction, anchor_pitch) {
        let (min_pitch, max_pitch) = app.piano_pitch_range();
        app.last_status = format!(
            "Piano pitch zoom unchanged {} rows",
            max_pitch - min_pitch + 1
        );
    }
}

fn adjust_selected_velocity(app: &mut AppState, delta: i16) {
    let Some(note) = app.selected_clip_note() else {
        app.last_status = "No clip note selected".to_string();
        return;
    };
    let velocity = (note.velocity as i16 + delta).clamp(1, 127) as u8;
    app.set_selected_clip_note_velocity(velocity);
}

fn next_quantize_grid(grid: QuantizeGrid) -> QuantizeGrid {
    let all = QuantizeGrid::all();
    let index = all
        .iter()
        .position(|candidate| *candidate == grid)
        .unwrap_or(0);
    all[(index + 1) % all.len()]
}
