use operad::{UiDocument, UiNode, UiNodeId};

pub(super) fn apply_focus_name(document: &mut UiDocument, focused_name: Option<&str>) {
    let Some(name) = focused_name else {
        return;
    };
    let Some(id) = document
        .nodes()
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            (node.name == name && node_is_focusable_action(node)).then_some(UiNodeId(index))
        })
    else {
        return;
    };
    let mut focus = document.focus.clone();
    focus.focused = Some(id);
    document.set_focus_state(focus);
}

#[cfg(test)]
pub(super) fn focused_node_name(document: &UiDocument) -> Option<String> {
    let id = document.focus.focused?;
    let node = document.node(id);
    node_is_focusable_action(node).then(|| node.name.clone())
}

#[cfg(test)]
pub(super) fn keyboard_focus_status(document: &UiDocument, id: UiNodeId) -> String {
    format!(
        "Focused {} - Enter activates",
        focusable_node_label(document.node(id))
    )
}

#[cfg(test)]
fn focusable_node_label(node: &UiNode) -> String {
    node.accessibility
        .as_ref()
        .and_then(|accessibility| accessibility.label.as_deref())
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| node.name.replace('.', " "))
}

pub(super) fn node_is_focusable_action(node: &UiNode) -> bool {
    node.input.focusable
        && node.accessibility.as_ref().is_some_and(|accessibility| {
            accessibility.enabled && accessibility.focusable && !accessibility.hidden
        })
}

pub(super) fn button_accessibility_label(name: &str, label: &str) -> String {
    if name.starts_with("asset.kind.") {
        return match label {
            "Samp" | "Samples" => "Show samples",
            "Instr" | "Instruments" => "Show instruments",
            "Pres" | "Presets" => "Show presets",
            "IRs" | "Impulses" => "Show impulses",
            _ => return fallback_button_accessibility_label(name, label),
        }
        .to_string();
    }
    if name.starts_with("scale.scroll_up.") {
        return "Scroll scale list up".to_string();
    }
    if name.starts_with("scale.scroll_down.") {
        return "Scroll scale list down".to_string();
    }
    if name.starts_with("asset.scroll_up.") {
        return "Scroll asset list up".to_string();
    }
    if name.starts_with("asset.scroll_down.") {
        return "Scroll asset list down".to_string();
    }
    if let Some(parameter) = name
        .strip_prefix("synth.")
        .and_then(|name| name.strip_suffix("_down"))
    {
        return format!(
            "Decrease {}",
            synth_parameter_accessibility_label(parameter)
        );
    }
    if let Some(parameter) = name
        .strip_prefix("synth.")
        .and_then(|name| name.strip_suffix("_up"))
    {
        return format!(
            "Increase {}",
            synth_parameter_accessibility_label(parameter)
        );
    }
    match name {
        "transport.bpm_down" => "Decrease BPM",
        "transport.bpm_up" => "Increase BPM",
        "transport.loop_down" => "Shorten loop",
        "transport.loop_up" => "Lengthen loop",
        "transport.quantize_grid" | "piano.transport.quantize_grid" => "Cycle quantize grid",
        "transport.quantize_grid_prev" | "piano.transport.quantize_grid_prev" => {
            "Previous quantize grid"
        }
        "transport.quantize_grid_next" | "piano.transport.quantize_grid_next" => {
            "Next quantize grid"
        }
        "transport.snap" | "piano.transport.snap" => "Toggle snap",
        "scale.root_down" => "Lower root note",
        "scale.root_up" => "Raise root note",
        "scale.base_down" => "Lower base frequency",
        "scale.base_up" => "Raise base frequency",
        "ui.scale_down" => "Decrease UI zoom",
        "ui.scale_reset" => "Reset UI zoom",
        "ui.scale_up" => "Increase UI zoom",
        "settings.ui.scale_down" => "Decrease UI zoom",
        "settings.ui.scale_reset" => "Reset UI zoom",
        "settings.ui.scale_up" => "Increase UI zoom",
        "settings.view.assets" => "Toggle asset browser",
        "settings.view.scales" => "Toggle scale browser",
        "settings.view.clip" => "Toggle clip panel",
        "settings.view.reset_layout" => "Reset workspace layout",
        "settings.panel.save" => "Save settings",
        "settings.diagnostics.clear" => "Clear diagnostics",
        "settings.view.devices" => "Open device setup panel",
        "view.clip" | "piano.view.clip" => "Toggle clip panel",
        "view.devices" if label == "Setup" => "Open device setup panel",
        "view.devices" => "Toggle device panel",
        "view.reset_layout" => "Reset workspace layout",
        "view.settings" => "Toggle settings panel",
        "transport.prev" => "Return to loop start",
        "piano.zoom_in" => "Zoom piano roll time in",
        "piano.zoom_out" => "Zoom piano roll time out",
        "piano.fit_view" => "Fit piano roll to clip",
        "piano.pitch_zoom_in" => "Zoom piano roll rows in",
        "piano.pitch_zoom_out" => "Zoom piano roll rows out",
        "piano.pitch_labels" => "Toggle piano pitch labels",
        "audio.all_off" => "Panic: all notes off",
        "audio.test_a4" => "Test A4 tone",
        "midi.prev" => "Previous MIDI input",
        "midi.next" => "Next MIDI input",
        "midi.refresh" => "Refresh MIDI inputs",
        "midi.connect" => "Connect MIDI input",
        "midi.channel_filter" => "Cycle MIDI channel filter",
        "midi.channel_filter_prev" => "Previous MIDI channel filter",
        "midi.channel_filter_next" => "Next MIDI channel filter",
        "audio.prev" => "Previous audio output",
        "audio.next" => "Next audio output",
        "audio.refresh" => "Refresh audio outputs",
        "audio.connect" => "Connect audio output",
        "keymap.prev" => "Previous key map preset",
        "keymap.next" => "Next key map preset",
        "keymap.refresh" => "Refresh key map presets",
        "scale.import" => "Import Scala scale",
        "scale.scroll_up" => "Scroll scale list up",
        "scale.scroll_down" => "Scroll scale list down",
        "scale.search_clear" => "Clear scale search",
        "asset.scroll_up" => "Scroll asset list up",
        "asset.scroll_down" => "Scroll asset list down",
        "asset.preview" => "Preview selected sample",
        "asset.stop_preview" => "Stop sample preview",
        "asset.use_sample" => "Use selected sample as instrument",
        "asset.clear_sample" => "Clear sample instrument",
        "asset.search_clear" => "Clear asset search",
        "asset.refresh" => "Refresh asset browser",
        "asset.import" => "Import asset",
        "clip.nudge_left" => "Nudge note left",
        "clip.nudge_right" => "Nudge note right",
        "clip.pitch_down" => "Lower note pitch",
        "clip.pitch_up" => "Raise note pitch",
        "clip.shorter" => "Shorten note",
        "clip.longer" => "Lengthen note",
        "clip.velocity_down" => "Lower note velocity",
        "clip.velocity_up" => "Raise note velocity",
        "clip.duplicate_note" => "Duplicate note",
        "clip.quantize" => "Quantize notes",
        "clip.clear" => "Clear clip",
        "synth.waveform" => "Cycle waveform",
        "synth.waveform_prev" => "Previous waveform",
        "synth.waveform_next" => "Next waveform",
        "synth.clear_sample" => "Clear sample instrument",
        "synth.mute" => "Toggle audio mute",
        "synth.reset" => "Reset synth settings",
        _ => return fallback_button_accessibility_label(name, label),
    }
    .to_string()
}

fn synth_parameter_accessibility_label(parameter: &str) -> &str {
    match parameter {
        "gain" => "synth gain",
        "attack" => "attack time",
        "release" => "release time",
        "filter" => "filter cutoff",
        "delay" => "delay mix",
        "drive" => "drive",
        _ => parameter,
    }
}

fn fallback_button_accessibility_label(name: &str, label: &str) -> String {
    if label.trim().is_empty() {
        name.replace('.', " ")
    } else {
        label.to_string()
    }
}
