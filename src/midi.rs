use midir::MidiInput;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicI8, Ordering},
};
use std::time::Instant;

use crate::project::SharedMusicProject;
use crate::scale::ScaleState;
use crate::synth::SynthHandle;

#[derive(Clone, Debug)]
pub(crate) struct MidiEvent {
    pub(crate) raw_status: u8,
    pub(crate) status: u8,
    pub(crate) channel: u8,
    pub(crate) midi_note: u8,
    pub(crate) velocity: u8,
    pub(crate) key_index: i32,
    pub(crate) musical_note: i32,
    pub(crate) mapped_from_lumatone: bool,
    pub(crate) freq: Option<f32>,
    pub(crate) scale_degree: Option<usize>,
    pub(crate) scale_octave: Option<i32>,
    pub(crate) cents_from_root: Option<f32>,
    pub(crate) at: Instant,
}

pub(crate) type SharedMidiLast = Arc<Mutex<Option<MidiEvent>>>;
pub(crate) type SharedMidiLog = Arc<Mutex<Vec<MidiEvent>>>;
pub(crate) type SharedMidiCapture = Arc<Mutex<MidiCapture>>;
pub(crate) type SharedMidiHeld = Arc<Mutex<HashMap<(i32, u8, u8), MidiEvent>>>;
pub(crate) type SharedMidiSustain = Arc<Mutex<MidiSustainState>>;
pub(crate) type SharedMidiChannelFilter = Arc<AtomicI8>;
pub(crate) const MIDI_CHANNEL_FILTER_ALL: i8 = -1;
const LUMATONE_KEYS_PER_BOARD: usize = 56;
pub(crate) type SharedLumatoneMap = Arc<Mutex<Option<Arc<LumatoneMap>>>>;

#[derive(Clone)]
pub(crate) struct MidiSharedState {
    pub(crate) last: SharedMidiLast,
    pub(crate) log: SharedMidiLog,
    pub(crate) capture: SharedMidiCapture,
    pub(crate) held: SharedMidiHeld,
    pub(crate) sustain: SharedMidiSustain,
    pub(crate) channel_filter: SharedMidiChannelFilter,
    pub(crate) lumatone_map: SharedLumatoneMap,
    pub(crate) music_project: SharedMusicProject,
}

const MAX_CAPTURE_EVENTS: usize = 512;

impl MidiEvent {
    pub(crate) fn is_note_on(&self) -> bool {
        self.status == 0x90 && self.velocity > 0
    }

    pub(crate) fn is_note_off(&self) -> bool {
        self.status == 0x80 || (self.status == 0x90 && self.velocity == 0)
    }

    pub(crate) fn is_sustain_on(&self) -> bool {
        self.status == 0xB0 && self.midi_note == 64 && self.velocity >= 64
    }

    pub(crate) fn is_sustain_off(&self) -> bool {
        self.status == 0xB0 && self.midi_note == 64 && self.velocity < 64
    }
}

pub(crate) fn midi_channel_filter_allows(filter: i8, channel: u8) -> bool {
    filter == MIDI_CHANNEL_FILTER_ALL || filter == channel as i8
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MidiSustainState {
    down_channels: HashSet<u8>,
    sustained_notes: HashSet<(u8, u32)>,
}

impl MidiSustainState {
    pub(crate) fn press(&mut self, channel: u8) {
        self.down_channels.insert(channel);
    }

    fn release(&mut self, channel: u8) -> Vec<u32> {
        self.down_channels.remove(&channel);
        let notes = self
            .sustained_notes
            .iter()
            .filter_map(|(note_channel, note)| (*note_channel == channel).then_some(*note))
            .collect::<Vec<_>>();
        self.sustained_notes
            .retain(|(note_channel, _)| *note_channel != channel);
        notes
    }

    fn note_on(&mut self, channel: u8, note: u32) {
        self.sustained_notes.remove(&(channel, note));
    }

    pub(crate) fn defer_note_off(&mut self, channel: u8, note: u32) -> bool {
        if !self.down_channels.contains(&channel) {
            return false;
        }
        self.sustained_notes.insert((channel, note));
        true
    }

    pub(crate) fn clear(&mut self) {
        self.down_channels.clear();
        self.sustained_notes.clear();
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MidiCapture {
    armed: bool,
    events: Vec<MidiEvent>,
}

impl MidiCapture {
    pub(crate) fn start(&mut self) {
        self.events.clear();
        self.armed = true;
    }

    pub(crate) fn stop(&mut self) {
        self.armed = false;
    }

    pub(crate) fn clear(&mut self) {
        self.events.clear();
    }

    pub(crate) fn is_armed(&self) -> bool {
        self.armed
    }

    pub(crate) fn len(&self) -> usize {
        self.events.len()
    }

    pub(crate) fn events(&self) -> Vec<MidiEvent> {
        self.events.clone()
    }

    fn record(&mut self, event: &MidiEvent) {
        if !self.armed || !event.is_note_on() {
            return;
        }
        self.events.push(event.clone());
        if self.events.len() > MAX_CAPTURE_EVENTS {
            let drain = self.events.len() - MAX_CAPTURE_EVENTS;
            self.events.drain(0..drain);
        }
    }
}

pub(crate) fn list_midi_inputs() -> Vec<String> {
    let midi_in = MidiInput::new("orbifold");
    let midi_in = match midi_in {
        Ok(midi_in) => midi_in,
        Err(err) => {
            log::error!("Failed to initialize MIDI input while listing ports: {err}");
            return Vec::new();
        }
    };
    midi_in
        .ports()
        .iter()
        .map(|p| match midi_in.port_name(p) {
            Ok(name) => name,
            Err(err) => {
                log::error!("Failed to read MIDI input port name: {err}; using Unknown");
                "Unknown".to_string()
            }
        })
        .collect()
}

pub(crate) fn handle_midi(
    message: &[u8],
    scale_state: &Arc<Mutex<ScaleState>>,
    synth: &SynthHandle,
    midi_state: &MidiSharedState,
    debug_log: bool,
) {
    if message.len() < 3 {
        return;
    }
    let raw_status = message[0];
    let status = raw_status & 0xF0;
    let channel = raw_status & 0x0F;
    let midi_note = message[1];
    let velocity = message[2];
    let now = Instant::now();
    if debug_log {
        log::debug!("MIDI {raw_status:02X} {midi_note:02X} {velocity:02X}");
    }

    let map = midi_state.lumatone_map.lock().clone();
    let mapped_key = map
        .as_deref()
        .and_then(|map| map.key_for_message(channel, midi_note))
        .map(|key| key as i32);
    let mapped_from_lumatone = mapped_key.is_some();
    let key_index = mapped_key.unwrap_or(midi_note as i32);
    let musical_note = midi_note as i32;

    let note_info = {
        let scale = scale_state.lock();
        scale.note_info(musical_note)
    };

    let event = MidiEvent {
        raw_status,
        status,
        channel,
        midi_note,
        velocity,
        key_index,
        musical_note,
        mapped_from_lumatone,
        freq: note_info.as_ref().map(|info| info.freq),
        scale_degree: note_info.as_ref().map(|info| info.degree),
        scale_octave: note_info.as_ref().map(|info| info.octave),
        cents_from_root: note_info.as_ref().map(|info| info.cents_from_root),
        at: now,
    };
    *midi_state.last.lock() = Some(event.clone());
    {
        let mut log = midi_state.log.lock();
        log.push(event.clone());
        if log.len() > 32 {
            let drain = log.len() - 32;
            log.drain(0..drain);
        }
    }
    let channel_filter = midi_state.channel_filter.load(Ordering::Relaxed);
    if !midi_channel_filter_allows(channel_filter, channel) {
        return;
    }
    midi_state.capture.lock().record(&event);
    let is_note_on = event.is_note_on();
    let is_note_off = event.is_note_off();
    let held_key = (event.key_index, event.channel, event.midi_note);
    if is_note_on {
        midi_state.held.lock().insert(held_key, event.clone());
    } else if is_note_off {
        midi_state.held.lock().remove(&held_key);
    }
    midi_state.music_project.lock().record_midi_event(&event);

    if event.is_sustain_on() {
        midi_state.sustain.lock().press(channel);
    } else if event.is_sustain_off() {
        let sustained_notes = midi_state.sustain.lock().release(channel);
        for note in sustained_notes {
            if let Err(err) = synth.note_off(note) {
                log::error!(
                    "Audio command error while releasing sustained MIDI note {note}: {err}"
                );
            }
        }
    } else if is_note_on {
        midi_state.sustain.lock().note_on(channel, key_index as u32);
        if let Some(info) = note_info {
            let vel = (velocity as f32 / 127.0).clamp(0.0, 1.0);
            if let Err(err) = synth.note_on(key_index as u32, info.freq, vel) {
                log::error!("Audio command error while starting MIDI note {key_index}: {err}");
            }
        }
    } else if is_note_off
        && !midi_state
            .sustain
            .lock()
            .defer_note_off(channel, key_index as u32)
        && let Err(err) = synth.note_off(key_index as u32)
    {
        log::error!("Audio command error while stopping MIDI note {key_index}: {err}");
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LumatoneKey {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) midi_note: u8,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) color: Option<[u8; 3]>,
}

#[derive(Debug)]
pub(crate) struct LumatoneMap {
    by_chan_note: HashMap<(u8, u8), u32>,
    keys: HashMap<u32, LumatoneKey>,
}

impl LumatoneMap {
    pub(crate) fn key_for_message(&self, channel: u8, note: u8) -> Option<u32> {
        self.by_chan_note.get(&(channel, note)).copied()
    }

    #[cfg(test)]
    pub(crate) fn key(&self, index: u32) -> Option<&LumatoneKey> {
        self.keys.get(&index)
    }

    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }
}

pub(crate) fn load_lumatone_map(path: &Path) -> Result<LumatoneMap, String> {
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_lumatone_map_contents(&data)
}

fn parse_lumatone_map_contents(data: &str) -> Result<LumatoneMap, String> {
    let mut notes: HashMap<usize, u8> = HashMap::new();
    let mut chans: HashMap<usize, u8> = HashMap::new();
    let mut colors: HashMap<usize, [u8; 3]> = HashMap::new();
    let mut board = 0usize;
    let mut board_offsets: HashMap<usize, usize> = HashMap::new();
    board_offsets.insert(0, 0);

    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            if let Some(rest) = line.strip_prefix("[Board") {
                let num = rest
                    .strip_suffix(']')
                    .ok_or_else(|| format!("Invalid board header in line: {line}"))?
                    .parse::<usize>()
                    .map_err(|_| format!("Invalid board number in line: {line}"))?;
                board = num;
                board_offsets.insert(board, board * LUMATONE_KEYS_PER_BOARD);
            }
            continue;
        }

        let offset = board_offsets
            .get(&board)
            .copied()
            .unwrap_or(board * LUMATONE_KEYS_PER_BOARD);
        if let Some(rest) = line.strip_prefix("Key_") {
            let (idx, value) = parse_indexed_value(rest, "Key", line)?;
            validate_lumatone_key_index(idx, "Key", line)?;
            if value > 127 {
                return Err(format!("Invalid MIDI note {value} in line: {line}"));
            }
            notes.insert(offset + idx, value);
        } else if let Some(rest) = line.strip_prefix("Chan_") {
            let (idx, value) = parse_indexed_value(rest, "Chan", line)?;
            validate_lumatone_key_index(idx, "Chan", line)?;
            if !(1..=16).contains(&value) {
                return Err(format!("Invalid MIDI channel {value} in line: {line}"));
            }
            chans.insert(offset + idx, value);
        } else if let Some(rest) = line.strip_prefix("Col_") {
            let mut parts = rest.splitn(2, '=');
            let idx = parts
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| format!("Invalid Col index in line: {line}"))?;
            validate_lumatone_key_index(idx, "Col", line)?;
            let value = parts
                .next()
                .ok_or_else(|| format!("Invalid Col value in line: {line}"))?;
            colors.insert(offset + idx, parse_hex_color(value, line)?);
        }
    }

    let mut by_chan_note = HashMap::new();
    let mut keys = HashMap::new();
    for (idx, midi_note) in notes {
        if let Some(chan_raw) = chans.get(&idx).copied() {
            let channel = chan_raw - 1;
            by_chan_note.insert((channel, midi_note), idx as u32);
            keys.insert(
                idx as u32,
                LumatoneKey {
                    midi_note,
                    color: colors.get(&idx).copied(),
                },
            );
        }
    }

    if by_chan_note.is_empty() {
        return Err("No Key/Chan pairs found in key map".to_string());
    }

    Ok(LumatoneMap { by_chan_note, keys })
}

fn parse_indexed_value(rest: &str, kind: &str, line: &str) -> Result<(usize, u8), String> {
    let mut parts = rest.splitn(2, '=');
    let idx = parts
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| format!("Invalid {kind} index in line: {line}"))?;
    let value = parts
        .next()
        .and_then(|value| value.parse::<u8>().ok())
        .ok_or_else(|| format!("Invalid {kind} value in line: {line}"))?;
    Ok((idx, value))
}

fn validate_lumatone_key_index(idx: usize, kind: &str, line: &str) -> Result<(), String> {
    if idx >= LUMATONE_KEYS_PER_BOARD {
        return Err(format!("Invalid {kind} index {idx} in line: {line}"));
    }
    Ok(())
}

fn parse_hex_color(value: &str, line: &str) -> Result<[u8; 3], String> {
    if value.len() != 6 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("Invalid color in line: {line}"));
    }
    let red = u8::from_str_radix(&value[0..2], 16)
        .map_err(|_| format!("Invalid color in line: {line}"))?;
    let green = u8::from_str_radix(&value[2..4], 16)
        .map_err(|_| format!("Invalid color in line: {line}"))?;
    let blue = u8::from_str_radix(&value[4..6], 16)
        .map_err(|_| format!("Invalid color in line: {line}"))?;
    Ok([red, green, blue])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::MusicProject;

    #[test]
    fn parses_lumatone_keys_channels_and_colors() {
        let map = parse_lumatone_map_contents(
            r#"
[Board0]
Key_0=60
Chan_0=1
Col_0=FF00AA
[Board1]
Key_0=61
Chan_0=2
Col_0=00FF00
"#,
        )
        .expect("map should parse");

        assert_eq!(map.key_for_message(0, 60), Some(0));
        assert_eq!(map.key_for_message(1, 61), Some(56));
        assert_eq!(map.key(0).expect("key exists").color, Some([255, 0, 170]));
        assert_eq!(map.key(56).expect("key exists").midi_note, 61);
    }

    #[test]
    fn rejects_invalid_lumatone_channels() {
        let err = parse_lumatone_map_contents(
            r#"
[Board0]
Key_0=60
Chan_0=17
"#,
        )
        .expect_err("map should fail");

        assert_eq!(err, "Invalid MIDI channel 17 in line: Chan_0=17");
    }

    #[test]
    fn rejects_invalid_lumatone_notes() {
        let err = parse_lumatone_map_contents(
            r#"
[Board0]
Key_0=128
Chan_0=1
"#,
        )
        .expect_err("map should fail");

        assert_eq!(err, "Invalid MIDI note 128 in line: Key_0=128");
    }

    #[test]
    fn rejects_lumatone_entries_without_key_channel_pairs() {
        let err = parse_lumatone_map_contents(
            r#"
[Board0]
Key_0=60
"#,
        )
        .expect_err("map should fail");

        assert_eq!(err, "No Key/Chan pairs found in key map");
    }

    #[test]
    fn rejects_invalid_lumatone_board_headers() {
        let err = parse_lumatone_map_contents(
            r#"
[BoardX]
Key_0=60
Chan_0=1
"#,
        )
        .expect_err("map should fail");

        assert_eq!(err, "Invalid board number in line: [BoardX]");
    }

    #[test]
    fn rejects_out_of_range_lumatone_key_indexes() {
        let err = parse_lumatone_map_contents(
            r#"
[Board0]
Key_56=60
Chan_56=1
"#,
        )
        .expect_err("map should fail");

        assert_eq!(err, "Invalid Key index 56 in line: Key_56=60");
    }

    #[test]
    fn rejects_invalid_lumatone_colors() {
        let err = parse_lumatone_map_contents(
            r#"
[Board0]
Key_0=60
Chan_0=1
Col_0=XYZ
"#,
        )
        .expect_err("map should fail");

        assert_eq!(err, "Invalid color in line: Col_0=XYZ");
    }

    #[test]
    fn classic_mode_reverse_maps_known_raw_notes() {
        let map = parse_lumatone_map_contents(include_str!(
            "../lumatone_factory_presets/1. Classic Mode.ltn"
        ))
        .expect("classic map should parse");

        assert_eq!(map.key_for_message(0, 44), Some(23));
        assert_eq!(map.key_for_message(0, 46), Some(24));
        assert_eq!(map.key_for_message(0, 48), Some(75));
        assert_eq!(map.key_for_message(5, 48), Some(56));
    }

    #[test]
    fn mapped_lumatone_event_keeps_raw_note_as_musical_pitch() {
        let scale_state = Arc::new(Mutex::new(ScaleState {
            root_midi: 44,
            base_freq: 440.0,
            ..ScaleState::default()
        }));
        let synth = SynthHandle::new(4);
        let midi_last = Arc::new(Mutex::new(None));
        let midi_log = Arc::new(Mutex::new(Vec::new()));
        let midi_capture = Arc::new(Mutex::new(MidiCapture::default()));
        let midi_held = Arc::new(Mutex::new(HashMap::new()));
        let midi_sustain = Arc::new(Mutex::new(MidiSustainState::default()));
        let music_project = Arc::new(Mutex::new(MusicProject::default()));
        let lumatone_map = Arc::new(Mutex::new(Some(Arc::new(
            parse_lumatone_map_contents(include_str!(
                "../lumatone_factory_presets/1. Classic Mode.ltn"
            ))
            .expect("classic map should parse"),
        ))));
        let midi_state = MidiSharedState {
            last: midi_last.clone(),
            log: midi_log,
            capture: midi_capture,
            held: midi_held,
            sustain: midi_sustain,
            channel_filter: Arc::new(AtomicI8::new(MIDI_CHANNEL_FILTER_ALL)),
            lumatone_map,
            music_project,
        };

        handle_midi(&[0x90, 44, 100], &scale_state, &synth, &midi_state, false);

        let event = midi_last
            .lock()
            .clone()
            .expect("MIDI event should be recorded");
        assert_eq!(event.key_index, 23);
        assert_eq!(event.musical_note, 44);
        assert_eq!(event.scale_degree, Some(0));
        assert_eq!(event.freq, Some(440.0));
        assert_eq!(synth.active_notes(), vec![23]);
    }

    #[test]
    fn mapping_capture_records_note_ons_only_when_armed() {
        let scale_state = Arc::new(Mutex::new(ScaleState::default()));
        let synth = SynthHandle::new(4);
        let midi_last = Arc::new(Mutex::new(None));
        let midi_log = Arc::new(Mutex::new(Vec::new()));
        let midi_capture = Arc::new(Mutex::new(MidiCapture::default()));
        let midi_held = Arc::new(Mutex::new(HashMap::new()));
        let midi_sustain = Arc::new(Mutex::new(MidiSustainState::default()));
        let music_project = Arc::new(Mutex::new(MusicProject::default()));
        let lumatone_map = Arc::new(Mutex::new(None));
        let midi_state = MidiSharedState {
            last: midi_last,
            log: midi_log,
            capture: midi_capture.clone(),
            held: midi_held,
            sustain: midi_sustain,
            channel_filter: Arc::new(AtomicI8::new(MIDI_CHANNEL_FILTER_ALL)),
            lumatone_map,
            music_project,
        };

        handle_midi(&[0x90, 60, 100], &scale_state, &synth, &midi_state, false);
        assert_eq!(midi_capture.lock().len(), 0);

        midi_capture.lock().start();
        handle_midi(&[0x90, 60, 100], &scale_state, &synth, &midi_state, false);
        handle_midi(&[0x80, 60, 0], &scale_state, &synth, &midi_state, false);
        handle_midi(&[0x90, 61, 0], &scale_state, &synth, &midi_state, false);

        let events = midi_capture.lock().events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].midi_note, 60);
        assert_eq!(events[0].key_index, 60);
    }

    #[test]
    fn channel_filter_monitors_but_blocks_disallowed_note_behavior() {
        let scale_state = Arc::new(Mutex::new(ScaleState::default()));
        let synth = SynthHandle::new(4);
        let midi_last = Arc::new(Mutex::new(None));
        let midi_log = Arc::new(Mutex::new(Vec::new()));
        let midi_capture = Arc::new(Mutex::new(MidiCapture::default()));
        midi_capture.lock().start();
        let midi_held = Arc::new(Mutex::new(HashMap::new()));
        let midi_sustain = Arc::new(Mutex::new(MidiSustainState::default()));
        let music_project = Arc::new(Mutex::new(MusicProject::default()));
        let lumatone_map = Arc::new(Mutex::new(None));
        let channel_filter = Arc::new(AtomicI8::new(1));
        let midi_state = MidiSharedState {
            last: midi_last.clone(),
            log: midi_log.clone(),
            capture: midi_capture.clone(),
            held: midi_held.clone(),
            sustain: midi_sustain,
            channel_filter,
            lumatone_map,
            music_project,
        };

        handle_midi(&[0x90, 60, 100], &scale_state, &synth, &midi_state, false);

        assert_eq!(
            midi_last.lock().as_ref().map(|event| event.channel),
            Some(0)
        );
        assert_eq!(midi_log.lock().len(), 1);
        assert_eq!(midi_capture.lock().len(), 0);
        assert!(midi_held.lock().is_empty());
        assert!(synth.active_notes().is_empty());

        handle_midi(&[0x91, 60, 100], &scale_state, &synth, &midi_state, false);

        assert_eq!(midi_capture.lock().len(), 1);
        assert_eq!(midi_held.lock().len(), 1);
        assert_eq!(synth.active_notes(), vec![60]);
    }

    #[test]
    fn held_midi_notes_track_note_on_and_off() {
        let scale_state = Arc::new(Mutex::new(ScaleState::default()));
        let synth = SynthHandle::new(4);
        let midi_last = Arc::new(Mutex::new(None));
        let midi_log = Arc::new(Mutex::new(Vec::new()));
        let midi_capture = Arc::new(Mutex::new(MidiCapture::default()));
        let midi_held = Arc::new(Mutex::new(HashMap::new()));
        let midi_sustain = Arc::new(Mutex::new(MidiSustainState::default()));
        let music_project = Arc::new(Mutex::new(MusicProject::default()));
        let lumatone_map = Arc::new(Mutex::new(None));
        let midi_state = MidiSharedState {
            last: midi_last,
            log: midi_log,
            capture: midi_capture,
            held: midi_held.clone(),
            sustain: midi_sustain,
            channel_filter: Arc::new(AtomicI8::new(MIDI_CHANNEL_FILTER_ALL)),
            lumatone_map,
            music_project,
        };

        handle_midi(&[0x90, 60, 100], &scale_state, &synth, &midi_state, false);
        assert_eq!(midi_held.lock().len(), 1);

        handle_midi(&[0x80, 60, 0], &scale_state, &synth, &midi_state, false);
        assert!(midi_held.lock().is_empty());
    }

    #[test]
    fn sustain_pedal_defers_live_note_off_until_release() {
        let scale_state = Arc::new(Mutex::new(ScaleState::default()));
        let synth = SynthHandle::new(4);
        let midi_last = Arc::new(Mutex::new(None));
        let midi_log = Arc::new(Mutex::new(Vec::new()));
        let midi_capture = Arc::new(Mutex::new(MidiCapture::default()));
        let midi_held = Arc::new(Mutex::new(HashMap::new()));
        let midi_sustain = Arc::new(Mutex::new(MidiSustainState::default()));
        let music_project = Arc::new(Mutex::new(MusicProject::default()));
        let lumatone_map = Arc::new(Mutex::new(None));
        let midi_state = MidiSharedState {
            last: midi_last,
            log: midi_log,
            capture: midi_capture,
            held: midi_held,
            sustain: midi_sustain,
            channel_filter: Arc::new(AtomicI8::new(MIDI_CHANNEL_FILTER_ALL)),
            lumatone_map,
            music_project,
        };

        handle_midi(&[0x90, 60, 100], &scale_state, &synth, &midi_state, false);
        handle_midi(&[0xB0, 64, 127], &scale_state, &synth, &midi_state, false);
        handle_midi(&[0x80, 60, 0], &scale_state, &synth, &midi_state, false);

        assert_eq!(synth.active_notes(), vec![60]);

        handle_midi(&[0xB0, 64, 0], &scale_state, &synth, &midi_state, false);

        assert!(synth.active_notes().is_empty());
    }
}
