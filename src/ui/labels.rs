use crate::app::{AppState, audio_stream_info_label};

use super::text::compact_label;

pub(super) fn selected_midi_input_name(app: &AppState) -> String {
    let connected = app.midi_connection.is_some();
    let selected_input = app.midi_inputs.get(app.selected_input).map(String::as_str);
    let label = midi_input_status_label(connected, selected_input, &app.connected_midi_input);
    device_label_with_position(label, "MIDI", app.selected_input, app.midi_inputs.len())
}

pub(super) fn midi_input_diagnostic_label(app: &AppState) -> String {
    device_diagnostic_label(
        app.midi_inputs.get(app.selected_input).map(String::as_str),
        &app.connected_midi_input,
        app.midi_connection.is_some(),
        None,
        "Refresh to scan MIDI",
    )
}

pub(super) fn midi_input_status_label(
    connected: bool,
    selected_input: Option<&str>,
    connected_input: &str,
) -> String {
    let compact_chars = if connected { 25 } else { 22 };
    let input_name = if connected && !connected_input.is_empty() {
        Some(connected_input)
    } else {
        selected_input
    };
    device_status_label("MIDI", connected, "no input", input_name, compact_chars)
}

pub(super) fn midi_connect_label(app: &AppState, compact: bool) -> &'static str {
    let selected_input = app.midi_inputs.get(app.selected_input).map(String::as_str);
    let connected_to_selected = app.midi_connection.is_some()
        && selected_name_matches_connected(selected_input, &app.connected_midi_input);
    device_connect_label(connected_to_selected, compact, "Connect MIDI")
}

pub(super) fn device_connect_label(
    connected: bool,
    compact: bool,
    expanded_connect_label: &'static str,
) -> &'static str {
    match (connected, compact) {
        (true, _) => "Reconnect",
        (false, true) => "Connect",
        (false, false) => expanded_connect_label,
    }
}

pub(super) fn selected_audio_output_name(app: &AppState) -> String {
    let connected = app.audio_stream.is_some();
    let selected_device = app
        .audio_outputs
        .get(app.selected_audio_output)
        .map(|device| {
            let marker = if device.is_default && !device.name.eq_ignore_ascii_case("default") {
                " default"
            } else {
                ""
            };
            format!("{}{}", device.name, marker)
        });
    let label = audio_output_status_label(
        connected,
        selected_device.as_deref(),
        &app.connected_audio_output,
        app.audio_stream_info.as_ref(),
    );
    device_label_with_position(
        label,
        "Audio",
        app.selected_audio_output,
        app.audio_outputs.len(),
    )
}

pub(super) fn audio_output_diagnostic_label(app: &AppState) -> String {
    let selected_output = app
        .audio_outputs
        .get(app.selected_audio_output)
        .map(|device| device.name.as_str());
    device_diagnostic_label(
        selected_output,
        &app.connected_audio_output,
        app.audio_stream.is_some(),
        app.audio_stream_info.as_ref().map(audio_stream_info_label),
        "Refresh to scan audio",
    )
}

pub(super) fn audio_output_status_label(
    connected: bool,
    selected_device: Option<&str>,
    connected_output: &str,
    stream_info: Option<&crate::audio::AudioStreamInfo>,
) -> String {
    let connected_device = (connected && !connected_output.is_empty()).then_some(connected_output);
    let output_name = if connected {
        connected_device.or(selected_device)
    } else {
        selected_device.or_else(|| (!connected_output.is_empty()).then_some(connected_output))
    };
    let mut label = device_status_label(
        "Audio",
        connected,
        "no output",
        output_name,
        if stream_info.is_some() { 16 } else { 25 },
    );
    if connected && let Some(info) = stream_info {
        label = format!("{label} {}", audio_stream_info_label(info));
    }
    label
}

fn device_diagnostic_label(
    selected_device: Option<&str>,
    connected_device: &str,
    connected: bool,
    stream_info: Option<String>,
    empty_message: &'static str,
) -> String {
    let Some(selected) = selected_device else {
        return empty_message.to_string();
    };
    let selected = compact_label(selected, 12);
    let connected_name =
        (!connected_device.is_empty()).then(|| compact_label(connected_device, 12));
    if connected {
        if let Some(info) = stream_info {
            return format!("Live: {info}");
        }
        return match connected_name {
            Some(name) if name != selected => format!("Live {name}; select {selected}"),
            Some(name) => format!("Live: {name}"),
            None => format!("Live: {selected}"),
        };
    }
    match connected_name {
        Some(name) if name != selected => format!("Off {name}; select {selected}"),
        Some(name) => format!("Off: {name}"),
        None => format!("Select {selected}; Connect"),
    }
}

pub(super) fn device_status_label(
    kind: &str,
    connected: bool,
    missing_status: &str,
    device_name: Option<&str>,
    compact_chars: usize,
) -> String {
    device_name
        .map(|name| {
            let status = if connected {
                "connected"
            } else {
                "disconnected"
            };
            format!("{kind} {status} {}", compact_label(name, compact_chars))
        })
        .unwrap_or_else(|| format!("{kind} {missing_status}"))
}

pub(super) fn device_label_with_position(
    label: String,
    kind: &str,
    index: usize,
    total: usize,
) -> String {
    if total == 0 {
        return label;
    }
    let position = index.min(total - 1) + 1;
    label.replacen(kind, &format!("{kind} {position}/{total}"), 1)
}

pub(super) fn audio_connect_label(app: &AppState, compact: bool) -> &'static str {
    let selected_output = app
        .audio_outputs
        .get(app.selected_audio_output)
        .map(|device| device.name.as_str());
    let connected_to_selected = app.audio_stream.is_some()
        && selected_name_matches_connected(selected_output, &app.connected_audio_output);
    device_connect_label(connected_to_selected, compact, "Connect Audio")
}

pub(super) fn selected_name_matches_connected(
    selected_name: Option<&str>,
    connected_name: &str,
) -> bool {
    selected_name.is_some_and(|selected| !connected_name.is_empty() && selected == connected_name)
}

pub(super) fn midi_event_label(event: &crate::midi::MidiEvent) -> String {
    let remapped_note = event.musical_note != event.midi_note as i32;
    let tuned = match (
        event.scale_degree,
        event.scale_octave,
        event.cents_from_root,
    ) {
        (Some(degree), Some(octave), Some(cents)) if !remapped_note => {
            format!(" D{degree} O{octave} {cents:+.0}c")
        }
        _ => String::new(),
    };
    let event_data = match event.status {
        0x80 => format!("off {} vel{}", midi_event_note_label(event), event.velocity),
        0x90 if event.velocity == 0 => format!("off {} vel0", midi_event_note_label(event)),
        0x90 => format!(
            "note {} vel{}",
            midi_event_note_label(event),
            event.velocity
        ),
        0xB0 => midi_control_change_label(event.midi_note, event.velocity),
        0xE0 => midi_pitch_bend_label(event.midi_note, event.velocity),
        _ => format!("data{} value{}", event.midi_note, event.velocity),
    };
    format!(
        "Last MIDI ch{} {} status {:02X}{}",
        event.channel + 1,
        event_data,
        event.raw_status,
        tuned
    )
}

fn midi_event_note_label(event: &crate::midi::MidiEvent) -> String {
    if event.musical_note == event.midi_note as i32 {
        return format!(
            "{} ({})",
            midi_note_name(event.musical_note),
            event.musical_note
        );
    }
    let tuned = match (event.scale_degree, event.cents_from_root) {
        (Some(degree), Some(cents)) => format!("d{} {cents:+.0}c", degree + 1),
        _ => midi_note_name(event.musical_note),
    };
    format!(
        "{}->{tuned} ({})",
        midi_note_name(event.midi_note as i32),
        event.musical_note
    )
}

fn midi_control_change_label(controller: u8, value: u8) -> String {
    match controller {
        1 => format!("mod wheel ignored value{value}"),
        7 => format!("volume ignored value{value}"),
        10 => format!("pan ignored value{value}"),
        11 => format!("expression ignored value{value}"),
        64 => format!(
            "sustain {} value{value}",
            if value >= 64 { "on" } else { "off" }
        ),
        _ => format!("cc{controller} ignored value{value}"),
    }
}

fn midi_pitch_bend_label(lsb: u8, msb: u8) -> String {
    let value = (((msb as u16) << 7) | lsb as u16) as i32 - 8_192;
    format!("bend {value:+} ignored")
}

pub(super) fn lumatone_map_label(app: &AppState) -> String {
    if let Some(warning) = app.keymap_scale_mismatch_warning() {
        return compact_label(&warning, 42);
    }
    let map = app.lumatone_map.lock().clone();
    let Some(map) = map else {
        return "Key map none".to_string();
    };
    if !app.midi_inputs.is_empty() && !app.selected_midi_input_uses_lumatone_map() {
        return "Key map inactive".to_string();
    }
    let (name, missing) = app
        .lumatone_path
        .as_ref()
        .map(|path| {
            let name = path
                .file_stem()
                .or_else(|| path.file_name())
                .and_then(|value| value.to_str())
                .map(|value| compact_label(value, 22))
                .unwrap_or_else(|| "loaded".to_string());
            (name, !path.exists())
        })
        .unwrap_or_else(|| ("loaded".to_string(), false));
    let prefix = if missing {
        "Key map missing"
    } else {
        "Key map"
    };
    format!(
        "{prefix} {name} ({} {})",
        map.len(),
        if map.len() == 1 { "key" } else { "keys" }
    )
}

pub(super) fn pitch_label(app: &AppState, pitch: i32) -> String {
    let scale = app.scale_state.lock();
    let Some(info) = scale.note_info(pitch) else {
        return pitch.to_string();
    };
    if scale.scale.steps.len() == 12 && !app.piano_pitch_labels_show_degrees() {
        midi_note_name(pitch)
    } else {
        format!("d{} {:+.0}c", info.degree + 1, info.cents_from_root)
    }
}

pub(super) fn midi_note_name(note: i32) -> String {
    const NOTE_NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let pitch_class = note.rem_euclid(12) as usize;
    let octave = note.div_euclid(12) - 1;
    format!("{}{}", NOTE_NAMES[pitch_class], octave)
}
