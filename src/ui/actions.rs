use operad::UiPoint;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::app::{AppState, AudioAssetKind};
use crate::project::QuantizeGrid;
use crate::synth::{SynthSettings, Waveform};

use super::labels::{midi_note_name, selected_name_matches_connected};
use super::native::SurfaceRects;

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

fn shortcut_help_key(value: &str, shift: bool) -> bool {
    value == "?" || (shift && value == "/")
}

pub(super) fn shortcut_help_status() -> &'static str {
    "Shortcuts: Space play/pause | R record | N add note | +/- piano zoom | Arrows move/pitch | Shift+Arrows resize/velocity | Ctrl/Cmd+S save"
}

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
    if let Some(id) = action.strip_prefix("note.select.") {
        if let Ok(id) = id.parse::<u64>() {
            app.select_clip_note(Some(id));
        }
        return;
    }
    if let Some(index) = action.strip_prefix("scale.select.") {
        if let Ok(index) = index.parse::<usize>()
            && index < app.scale_library.len()
        {
            app.selected_scale_library = index;
            app.last_status = format!("Selected scale: {}", app.scale_library[index].name);
        }
        return;
    }
    if let Some(index) = action.strip_prefix("asset.select.") {
        if let Ok(index) = index.parse::<usize>() {
            app.select_audio_asset(index);
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
    match action {
        "file.new" => app.start_new_project(),
        "file.open" => open_project(app),
        "file.open_recent" => app.open_most_recent_project(),
        "file.forget_recent" => app.forget_most_recent_project(),
        "file.save" => save_project(app),
        "file.save_as" => save_project_as(app),
        "file.recover" => app.recover_autosave_project(),
        "file.dismiss_autosave" => app.dismiss_autosave_project(),
        "scale.open" => open_scale(app),
        "keymap.open" => open_keymap(app),
        "settings.save" => app.persist_settings_with_status(),
        "edit.undo" => app.undo_project_edit(),
        "edit.redo" => app.redo_project_edit(),
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
        "transport.snap" => app.toggle_snap_to_grid(),
        "ui.scale_down" => app.adjust_ui_scale(-0.1),
        "ui.scale_reset" => app.reset_ui_scale(),
        "ui.scale_up" => app.adjust_ui_scale(0.1),
        "view.assets" => app.toggle_asset_browser(),
        "view.scales" => app.toggle_scale_browser(),
        "audio.all_off" => app.all_notes_off(),
        "audio.test_a4" => app.test_tone(),
        "scale.root_down" => adjust_root_midi(app, -1),
        "scale.root_up" => adjust_root_midi(app, 1),
        "scale.base_down" => adjust_base_freq(app, -1.0),
        "scale.base_up" => adjust_base_freq(app, 1.0),
        "scale.load_selected" => app.load_selected_library_scale(),
        "scale.refresh" => app.refresh_scale_library(),
        "scale.remove_selected" => app.remove_selected_library_scale(),
        "asset.refresh" => app.refresh_audio_assets(),
        "asset.import" => import_audio_asset(app),
        "capture.start" => app.start_mapping_capture(),
        "capture.stop" => app.stop_mapping_capture(),
        "capture.clear" => app.clear_mapping_capture(),
        "keymap.refresh" => app.reload_lumatone_presets(),
        "synth.waveform" => cycle_waveform(app),
        "synth.mute" => app.toggle_audio_mute(),
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
        "midi.refresh" => app.refresh_midi_inputs(None),
        "midi.connect" => app.open_midi_input(),
        "midi.channel_filter" => app.cycle_midi_channel_filter(),
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
        "piano.grid" => {
            if let (Some(point), Some(layout)) = (cursor, layout) {
                app.add_clip_note_at(layout.beat_at(point), layout.pitch_at(point));
            }
        }
        _ => {}
    }
}

fn canonical_action_name(action: &str) -> &str {
    match action {
        "piano.view.scales" => "view.scales",
        "piano.transport.quantize_grid" => "transport.quantize_grid",
        "project.file.open" => "file.open",
        "project.file.save" => "file.save",
        _ => action,
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
    if !app
        .selected_audio_asset_item()
        .is_some_and(|asset| asset.kind == kind)
    {
        app.selected_audio_asset = app.audio_assets.iter().position(|asset| asset.kind == kind);
    }
    app.last_status = format!("Showing {}", kind.label());
}

fn import_audio_asset(app: &mut AppState) {
    let kind = app.selected_audio_asset_kind;
    app.request_import_audio_asset_dialog(kind);
}

fn select_midi_input(app: &mut AppState, direction: isize) {
    if app.midi_inputs.is_empty() {
        app.last_status = "No MIDI inputs found".to_string();
        return;
    }
    app.selected_input = wrap_index(app.selected_input, app.midi_inputs.len(), direction);
    let connected_name = if app.midi_connection.is_some() {
        app.connected_midi_input.as_str()
    } else {
        ""
    };
    app.last_status = selected_device_status(
        "MIDI input",
        &app.midi_inputs[app.selected_input],
        connected_name,
    );
    app.persist_current_settings();
}

fn select_audio_output(app: &mut AppState, direction: isize) {
    if app.audio_outputs.is_empty() {
        app.last_status = "No audio outputs found".to_string();
        return;
    }
    app.selected_audio_output = wrap_index(
        app.selected_audio_output,
        app.audio_outputs.len(),
        direction,
    );
    let connected_name = if app.audio_stream.is_some() {
        app.connected_audio_output.as_str()
    } else {
        ""
    };
    app.last_status = selected_device_status(
        "audio output",
        &app.audio_outputs[app.selected_audio_output].name,
        connected_name,
    );
    app.persist_current_settings();
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

#[cfg(test)]
pub(super) fn clamp_index(current: usize, len: usize, direction: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as isize + direction).clamp(0, len.saturating_sub(1) as isize) as usize
}

fn adjust_root_midi(app: &mut AppState, delta: i32) {
    let (previous, root) = {
        let mut scale = app.scale_state.lock();
        let previous = scale.root_midi;
        scale.root_midi = scale.root_midi.saturating_add(delta).clamp(0, 127);
        (previous, scale.root_midi)
    };
    if root == previous {
        app.last_status = format!("Root {} ({root}) unchanged", midi_note_name(root));
        return;
    }
    app.last_status = format!("Root {} ({root})", midi_note_name(root));
    app.mark_project_dirty();
    app.persist_current_settings();
}

fn adjust_base_freq(app: &mut AppState, delta: f32) {
    let (previous, freq) = {
        let mut scale = app.scale_state.lock();
        let previous = scale.base_freq;
        scale.base_freq = (scale.base_freq + delta).clamp(8.0, 20_000.0);
        (previous, scale.base_freq)
    };
    if (freq - previous).abs() <= f32::EPSILON {
        app.last_status = format!("Base frequency {freq:.2} Hz unchanged");
        return;
    }
    app.last_status = format!("Base frequency {freq:.2} Hz");
    app.mark_project_dirty();
    app.persist_current_settings();
}

fn cycle_waveform(app: &mut AppState) {
    adjust_synth(app, |settings| {
        let all = Waveform::all();
        let index = all
            .iter()
            .position(|waveform| *waveform == settings.waveform)
            .unwrap_or(0);
        settings.waveform = all[(index + 1) % all.len()];
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
        Err(err) => app.last_status = format!("Synth settings error: {err}"),
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
    let (previous, bpm) = {
        let mut project = app.music_project.lock();
        let previous = project.transport.bpm;
        project.transport.bpm = (project.transport.bpm + delta).clamp(20.0, 320.0);
        (previous, project.transport.bpm)
    };
    if (bpm - previous).abs() <= f32::EPSILON {
        app.last_status = format!("BPM {bpm:.2} unchanged");
        return;
    }
    app.last_status = format!("BPM {bpm:.2}");
    app.mark_project_dirty();
    app.persist_current_settings();
}

fn adjust_loop_beats(app: &mut AppState, delta: f32) {
    let (previous, beats) = {
        let mut project = app.music_project.lock();
        let previous = project.transport.loop_beats;
        project.transport.loop_beats = (project.transport.loop_beats + delta).clamp(1.0, 128.0);
        (previous, project.transport.loop_beats)
    };
    if (beats - previous).abs() <= f32::EPSILON {
        app.last_status = format!("Loop length {beats:.0} beats unchanged");
        return;
    }
    app.last_status = format!("Loop length {beats:.0} beats");
    app.mark_project_dirty();
    app.persist_current_settings();
}

fn cycle_quantize_grid(app: &mut AppState) {
    let grid = next_quantize_grid(app.music_project.lock().transport.quantize_grid);
    app.set_quantize_grid(grid);
}

fn add_note_at_playhead(app: &mut AppState) {
    let root_midi = app.scale_state.lock().root_midi;
    let beat = app
        .music_project
        .lock()
        .current_position_beats(std::time::Instant::now());
    app.add_clip_note_at(beat, root_midi);
}

fn zoom_piano_roll_at_playhead(app: &mut AppState, direction: f32) {
    let beat = app
        .music_project
        .lock()
        .current_position_beats(std::time::Instant::now());
    if !app.zoom_piano_roll(direction, beat) {
        let loop_beats = app.music_project.lock().transport.loop_beats;
        app.last_status = format!(
            "Piano zoom unchanged {:.2} beats",
            app.piano_view_visible_beats(loop_beats)
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
