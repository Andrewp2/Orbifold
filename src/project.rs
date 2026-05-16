use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::midi::MidiEvent;
use crate::synth::{SynthSettings, Waveform};

pub(crate) type SharedMusicProject = Arc<Mutex<MusicProject>>;

const DEFAULT_BPM: f32 = 120.0;
const DEFAULT_LOOP_BEATS: f32 = 16.0;
const MIN_NOTE_BEATS: f32 = 0.0625;
pub(crate) const PLAYBACK_NOTE_BASE: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QuantizeGrid {
    Off,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
}

impl QuantizeGrid {
    pub(crate) fn all() -> [Self; 5] {
        [
            Self::Off,
            Self::Quarter,
            Self::Eighth,
            Self::Sixteenth,
            Self::ThirtySecond,
        ]
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Quarter => "1/4",
            Self::Eighth => "1/8",
            Self::Sixteenth => "1/16",
            Self::ThirtySecond => "1/32",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "Off" => Some(Self::Off),
            "1/4" => Some(Self::Quarter),
            "1/8" => Some(Self::Eighth),
            "1/16" => Some(Self::Sixteenth),
            "1/32" => Some(Self::ThirtySecond),
            _ => None,
        }
    }

    pub(crate) fn step_beats(self) -> Option<f32> {
        match self {
            Self::Off => None,
            Self::Quarter => Some(1.0),
            Self::Eighth => Some(0.5),
            Self::Sixteenth => Some(0.25),
            Self::ThirtySecond => Some(0.125),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClipNote {
    pub(crate) id: u64,
    pub(crate) start_beats: f32,
    pub(crate) duration_beats: f32,
    pub(crate) key_index: i32,
    pub(crate) musical_note: i32,
    pub(crate) raw_channel: u8,
    pub(crate) raw_note: u8,
    pub(crate) velocity: u8,
    pub(crate) freq: f32,
    pub(crate) mapped_from_lumatone: bool,
}

impl ClipNote {
    pub(crate) fn end_beats(&self, loop_beats: f32) -> f32 {
        wrap_beat(self.start_beats + self.duration_beats, loop_beats)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Clip {
    pub(crate) notes: Vec<ClipNote>,
}

#[derive(Clone, Debug)]
pub(crate) struct Transport {
    pub(crate) bpm: f32,
    pub(crate) loop_beats: f32,
    pub(crate) playing: bool,
    pub(crate) recording: bool,
    pub(crate) overdub: bool,
    pub(crate) quantize_grid: QuantizeGrid,
    pub(crate) quantize_on_record: bool,
    pub(crate) metronome_enabled: bool,
    started_at: Option<Instant>,
    last_position_beats: f32,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            bpm: DEFAULT_BPM,
            loop_beats: DEFAULT_LOOP_BEATS,
            playing: false,
            recording: false,
            overdub: false,
            quantize_grid: QuantizeGrid::Sixteenth,
            quantize_on_record: true,
            metronome_enabled: false,
            started_at: None,
            last_position_beats: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
struct PendingNote {
    start_beats: f32,
    key_index: i32,
    musical_note: i32,
    raw_channel: u8,
    raw_note: u8,
    velocity: u8,
    freq: f32,
    mapped_from_lumatone: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct MusicProject {
    pub(crate) clip: Clip,
    pub(crate) transport: Transport,
    pending_notes: HashMap<(i32, u8, u8), PendingNote>,
    sustained_note_keys: HashSet<(i32, u8, u8)>,
    sustain_down: bool,
    next_note_id: u64,
}

impl Default for MusicProject {
    fn default() -> Self {
        Self {
            clip: Clip::default(),
            transport: Transport::default(),
            pending_notes: HashMap::new(),
            sustained_note_keys: HashSet::new(),
            sustain_down: false,
            next_note_id: 1,
        }
    }
}

impl MusicProject {
    pub(crate) fn play(&mut self, now: Instant) {
        self.transport.playing = true;
        self.transport.recording = false;
        self.transport.started_at = Some(now);
        self.clear_pending_recording_state();
    }

    #[cfg(test)]
    pub(crate) fn pause(&mut self, now: Instant) {
        self.finish_pending_notes(now);
        self.transport.last_position_beats = self.current_position_beats(now);
        self.transport.playing = false;
        self.transport.recording = false;
        self.transport.started_at = None;
    }

    pub(crate) fn seek(&mut self, position_beats: f32, now: Instant) {
        if self.transport.recording {
            self.finish_pending_notes(now);
            self.transport.recording = false;
        }
        self.transport.last_position_beats = wrap_beat(position_beats, self.transport.loop_beats);
        if self.transport.playing {
            self.transport.started_at = Some(now);
        }
    }

    pub(crate) fn stop(&mut self, now: Instant) {
        self.finish_pending_notes(now);
        self.transport.playing = false;
        self.transport.recording = false;
        self.transport.started_at = None;
        self.transport.last_position_beats = 0.0;
    }

    pub(crate) fn start_recording(&mut self, now: Instant) {
        if !self.transport.overdub {
            self.clip.notes.clear();
            self.next_note_id = 1;
        }
        self.transport.playing = true;
        self.transport.recording = true;
        self.transport.started_at = Some(now);
        self.transport.last_position_beats = 0.0;
        self.clear_pending_recording_state();
    }

    pub(crate) fn stop_recording(&mut self, now: Instant) {
        self.finish_pending_notes(now);
        self.transport.recording = false;
    }

    pub(crate) fn clear_clip(&mut self) -> bool {
        if self.clip.notes.is_empty() && self.pending_notes.is_empty() {
            return false;
        }
        self.clip.notes.clear();
        self.clear_pending_recording_state();
        self.next_note_id = 1;
        true
    }

    pub(crate) fn current_position_beats(&self, now: Instant) -> f32 {
        let Some(started_at) = self.transport.started_at else {
            return self.transport.last_position_beats;
        };
        let elapsed = now.saturating_duration_since(started_at);
        let beat = self.transport.last_position_beats
            + elapsed.as_secs_f32() * self.transport.bpm.max(1.0) / 60.0;
        wrap_beat(beat, self.transport.loop_beats)
    }

    pub(crate) fn record_midi_event(&mut self, event: &MidiEvent) {
        if !self.transport.recording {
            return;
        }
        let now = event.at;
        if event.is_sustain_on() {
            self.sustain_down = true;
        } else if event.is_sustain_off() {
            self.sustain_down = false;
            self.finish_sustained_notes(now);
        } else if event.is_note_on() {
            if let Some(freq) = event.freq {
                let key = (event.key_index, event.channel, event.midi_note);
                if let Some(previous) = self.pending_notes.remove(&key) {
                    self.sustained_note_keys.remove(&key);
                    self.push_finished_note(previous, self.current_position_beats(now));
                }
                let pending = PendingNote {
                    start_beats: self.current_position_beats(now),
                    key_index: event.key_index,
                    musical_note: event.musical_note,
                    raw_channel: event.channel,
                    raw_note: event.midi_note,
                    velocity: event.velocity,
                    freq,
                    mapped_from_lumatone: event.mapped_from_lumatone,
                };
                self.pending_notes.insert(key, pending);
            }
        } else if event.is_note_off() {
            let key = (event.key_index, event.channel, event.midi_note);
            if self.sustain_down && self.pending_notes.contains_key(&key) {
                self.sustained_note_keys.insert(key);
            } else if let Some(pending) = self.pending_notes.remove(&key) {
                self.sustained_note_keys.remove(&key);
                self.push_finished_note(pending, self.current_position_beats(now));
            }
        }
    }

    pub(crate) fn active_notes_at(&self, beat: f32) -> Vec<ClipNote> {
        self.clip
            .notes
            .iter()
            .filter(|note| note_active_at(note, beat, self.transport.loop_beats))
            .cloned()
            .collect()
    }

    pub(crate) fn quantize_clip(&mut self) -> bool {
        let Some(step) = self.transport.quantize_grid.step_beats() else {
            return false;
        };
        let mut changed = false;
        for note in &mut self.clip.notes {
            let start_beats = quantize_beat(note.start_beats, step, self.transport.loop_beats);
            let duration_beats = quantize_duration(note.duration_beats, step);
            changed |= (note.start_beats - start_beats).abs() > f32::EPSILON
                || (note.duration_beats - duration_beats).abs() > f32::EPSILON;
            note.start_beats = start_beats;
            note.duration_beats = duration_beats;
        }
        if changed {
            self.sort_clip_notes();
        }
        changed
    }

    pub(crate) fn quantize_note(&mut self, note_id: u64) -> bool {
        let Some(step) = self.transport.quantize_grid.step_beats() else {
            return false;
        };
        let Some(note) = self.clip.notes.iter_mut().find(|note| note.id == note_id) else {
            return false;
        };
        let start_beats = quantize_beat(note.start_beats, step, self.transport.loop_beats);
        let duration_beats = quantize_duration(note.duration_beats, step);
        let changed = (note.start_beats - start_beats).abs() > f32::EPSILON
            || (note.duration_beats - duration_beats).abs() > f32::EPSILON;
        note.start_beats = start_beats;
        note.duration_beats = duration_beats;
        if changed {
            self.sort_clip_notes();
        }
        changed
    }

    pub(crate) fn note_by_id(&self, note_id: u64) -> Option<ClipNote> {
        self.clip
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .cloned()
    }

    pub(crate) fn delete_note(&mut self, note_id: u64) -> bool {
        let before = self.clip.notes.len();
        self.clip.notes.retain(|note| note.id != note_id);
        self.pending_notes.clear();
        before != self.clip.notes.len()
    }

    pub(crate) fn duplicate_note(&mut self, note_id: u64) -> Option<u64> {
        let mut note = self.note_by_id(note_id)?;
        let duplicated_id = self.next_note_id;
        self.next_note_id = self.next_note_id.saturating_add(1);
        note.id = duplicated_id;
        note.start_beats = wrap_beat(
            note.start_beats + self.edit_step_beats(),
            self.transport.loop_beats,
        );
        self.clip.notes.push(note);
        self.sort_clip_notes();
        Some(duplicated_id)
    }

    pub(crate) fn add_note(
        &mut self,
        start_beats: f32,
        duration_beats: f32,
        musical_note: i32,
        velocity: u8,
        freq: f32,
    ) -> u64 {
        let note_id = self.next_note_id;
        self.next_note_id = self.next_note_id.saturating_add(1);
        self.clip.notes.push(ClipNote {
            id: note_id,
            start_beats: wrap_beat(start_beats, self.transport.loop_beats),
            duration_beats: duration_beats.clamp(MIN_NOTE_BEATS, self.transport.loop_beats),
            key_index: -1,
            musical_note,
            raw_channel: 0,
            raw_note: musical_note.clamp(0, 127) as u8,
            velocity: velocity.min(127),
            freq,
            mapped_from_lumatone: false,
        });
        self.sort_clip_notes();
        note_id
    }

    pub(crate) fn nudge_note(&mut self, note_id: u64, delta_beats: f32) -> bool {
        let Some(note) = self.clip.notes.iter_mut().find(|note| note.id == note_id) else {
            return false;
        };
        note.start_beats = wrap_beat(note.start_beats + delta_beats, self.transport.loop_beats);
        self.sort_clip_notes();
        true
    }

    pub(crate) fn resize_note(&mut self, note_id: u64, delta_beats: f32) -> bool {
        let Some(note) = self.clip.notes.iter_mut().find(|note| note.id == note_id) else {
            return false;
        };
        let duration = (note.duration_beats + delta_beats).clamp(
            MIN_NOTE_BEATS,
            self.transport.loop_beats.max(MIN_NOTE_BEATS),
        );
        if (duration - note.duration_beats).abs() <= f32::EPSILON {
            return false;
        }
        note.duration_beats = duration;
        true
    }

    pub(crate) fn set_note_duration(&mut self, note_id: u64, duration_beats: f32) -> bool {
        let Some(note) = self.clip.notes.iter_mut().find(|note| note.id == note_id) else {
            return false;
        };
        note.duration_beats = duration_beats.clamp(
            MIN_NOTE_BEATS,
            self.transport.loop_beats.max(MIN_NOTE_BEATS),
        );
        true
    }

    pub(crate) fn set_note_start_preserving_end(&mut self, note_id: u64, start_beats: f32) -> bool {
        let Some(note) = self.clip.notes.iter_mut().find(|note| note.id == note_id) else {
            return false;
        };
        let loop_beats = self.transport.loop_beats.max(MIN_NOTE_BEATS);
        let end_beats = note.start_beats + note.duration_beats;
        let start_beats = wrap_beat(start_beats, loop_beats);
        note.start_beats = start_beats;
        note.duration_beats = (end_beats - start_beats)
            .rem_euclid(loop_beats)
            .clamp(MIN_NOTE_BEATS, loop_beats);
        self.sort_clip_notes();
        true
    }

    pub(crate) fn set_note_velocity(&mut self, note_id: u64, velocity: u8) -> bool {
        let Some(note) = self.clip.notes.iter_mut().find(|note| note.id == note_id) else {
            return false;
        };
        note.velocity = velocity.min(127);
        true
    }

    pub(crate) fn set_note_pitch(&mut self, note_id: u64, musical_note: i32, freq: f32) -> bool {
        let Some(note) = self.clip.notes.iter_mut().find(|note| note.id == note_id) else {
            return false;
        };
        note.musical_note = musical_note;
        note.freq = freq;
        note.raw_note = musical_note.clamp(0, 127) as u8;
        note.key_index = -1;
        note.mapped_from_lumatone = false;
        self.sort_clip_notes();
        true
    }

    pub(crate) fn set_note_start_and_pitch(
        &mut self,
        note_id: u64,
        start_beats: f32,
        musical_note: i32,
        freq: f32,
    ) -> bool {
        let Some(note) = self.clip.notes.iter_mut().find(|note| note.id == note_id) else {
            return false;
        };
        note.start_beats = wrap_beat(start_beats, self.transport.loop_beats);
        note.musical_note = musical_note;
        note.freq = freq;
        note.raw_note = musical_note.clamp(0, 127) as u8;
        note.key_index = -1;
        note.mapped_from_lumatone = false;
        self.sort_clip_notes();
        true
    }

    pub(crate) fn edit_step_beats(&self) -> f32 {
        self.transport
            .quantize_grid
            .step_beats()
            .unwrap_or(0.25)
            .max(MIN_NOTE_BEATS)
    }

    pub(crate) fn snapshot(&self) -> ProjectSnapshot {
        ProjectSnapshot {
            transport: TransportSnapshot {
                bpm: self.transport.bpm,
                loop_beats: self.transport.loop_beats,
                overdub: self.transport.overdub,
                quantize_grid: self.transport.quantize_grid,
                quantize_on_record: self.transport.quantize_on_record,
                metronome_enabled: self.transport.metronome_enabled,
            },
            clip: self.clip.clone(),
            next_note_id: self.next_note_id,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, snapshot: ProjectSnapshot) {
        self.transport.bpm = snapshot.transport.bpm;
        self.transport.loop_beats = snapshot.transport.loop_beats;
        self.transport.overdub = snapshot.transport.overdub;
        self.transport.quantize_grid = snapshot.transport.quantize_grid;
        self.transport.quantize_on_record = snapshot.transport.quantize_on_record;
        self.transport.metronome_enabled = snapshot.transport.metronome_enabled;
        self.transport.playing = false;
        self.transport.recording = false;
        self.transport.started_at = None;
        self.transport.last_position_beats = 0.0;
        self.clip = snapshot.clip;
        self.clear_pending_recording_state();
        self.next_note_id = snapshot.next_note_id.max(
            self.clip
                .notes
                .iter()
                .map(|note| note.id)
                .max()
                .unwrap_or(0)
                + 1,
        );
    }

    fn finish_pending_notes(&mut self, now: Instant) {
        let end = self.current_position_beats(now);
        let pending = std::mem::take(&mut self.pending_notes);
        for note in pending.into_values() {
            self.push_finished_note(note, end);
        }
        self.sustained_note_keys.clear();
        self.sustain_down = false;
    }

    fn finish_sustained_notes(&mut self, now: Instant) {
        let end = self.current_position_beats(now);
        let keys = std::mem::take(&mut self.sustained_note_keys);
        for key in keys {
            if let Some(note) = self.pending_notes.remove(&key) {
                self.push_finished_note(note, end);
            }
        }
    }

    fn clear_pending_recording_state(&mut self) {
        self.pending_notes.clear();
        self.sustained_note_keys.clear();
        self.sustain_down = false;
    }

    fn push_finished_note(&mut self, pending: PendingNote, end_beats: f32) {
        let mut start = wrap_beat(pending.start_beats, self.transport.loop_beats);
        let mut duration = beat_distance(start, end_beats, self.transport.loop_beats);
        if duration <= MIN_NOTE_BEATS {
            duration = self
                .transport
                .quantize_grid
                .step_beats()
                .unwrap_or(0.25)
                .max(MIN_NOTE_BEATS);
        }
        if self.transport.quantize_on_record
            && let Some(step) = self.transport.quantize_grid.step_beats()
        {
            start = quantize_beat(start, step, self.transport.loop_beats);
            duration = quantize_duration(duration, step);
        }
        duration = duration.min(self.transport.loop_beats).max(MIN_NOTE_BEATS);
        self.clip.notes.push(ClipNote {
            id: self.next_note_id,
            start_beats: start,
            duration_beats: duration,
            key_index: pending.key_index,
            musical_note: pending.musical_note,
            raw_channel: pending.raw_channel,
            raw_note: pending.raw_note,
            velocity: pending.velocity,
            freq: pending.freq,
            mapped_from_lumatone: pending.mapped_from_lumatone,
        });
        self.next_note_id = self.next_note_id.saturating_add(1);
        self.sort_clip_notes();
    }

    fn sort_clip_notes(&mut self) {
        self.clip.notes.sort_by(|a, b| {
            a.start_beats
                .total_cmp(&b.start_beats)
                .then(a.musical_note.cmp(&b.musical_note))
                .then(a.id.cmp(&b.id))
        });
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TransportSnapshot {
    pub(crate) bpm: f32,
    pub(crate) loop_beats: f32,
    pub(crate) overdub: bool,
    pub(crate) quantize_grid: QuantizeGrid,
    pub(crate) quantize_on_record: bool,
    pub(crate) metronome_enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProjectSnapshot {
    pub(crate) transport: TransportSnapshot,
    pub(crate) clip: Clip,
    pub(crate) next_note_id: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProjectFile {
    pub(crate) scala_path: Option<PathBuf>,
    pub(crate) lumatone_path: Option<PathBuf>,
    pub(crate) root_midi: i32,
    pub(crate) base_freq: f32,
    pub(crate) synth_settings: SynthSettings,
    pub(crate) project: ProjectSnapshot,
}

impl ProjectFile {
    pub(crate) fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("orbifold_project=1\n");
        push_optional_path(&mut out, "scala_path", self.scala_path.as_ref());
        push_optional_path(&mut out, "lumatone_path", self.lumatone_path.as_ref());
        out.push_str(&format!("root_midi={}\n", self.root_midi));
        out.push_str(&format!("base_freq={}\n", self.base_freq));
        out.push_str(&format!(
            "waveform={}\n",
            self.synth_settings.waveform.as_str()
        ));
        out.push_str(&format!(
            "master_gain={}\n",
            self.synth_settings.master_gain
        ));
        out.push_str(&format!("attack_ms={}\n", self.synth_settings.attack_ms));
        out.push_str(&format!("release_ms={}\n", self.synth_settings.release_ms));
        out.push_str(&format!("drive={}\n", self.synth_settings.drive));
        out.push_str(&format!(
            "filter_cutoff_hz={}\n",
            self.synth_settings.filter_cutoff_hz
        ));
        out.push_str(&format!("delay_mix={}\n", self.synth_settings.delay_mix));
        out.push_str(&format!(
            "delay_feedback={}\n",
            self.synth_settings.delay_feedback
        ));
        out.push_str(&format!(
            "delay_time_ms={}\n",
            self.synth_settings.delay_time_ms
        ));
        out.push_str(&format!("bpm={}\n", self.project.transport.bpm));
        out.push_str(&format!(
            "loop_beats={}\n",
            self.project.transport.loop_beats
        ));
        out.push_str(&format!("overdub={}\n", self.project.transport.overdub));
        out.push_str(&format!(
            "quantize_grid={}\n",
            self.project.transport.quantize_grid.as_str()
        ));
        out.push_str(&format!(
            "quantize_on_record={}\n",
            self.project.transport.quantize_on_record
        ));
        out.push_str(&format!(
            "metronome_enabled={}\n",
            self.project.transport.metronome_enabled
        ));
        out.push_str(&format!("next_note_id={}\n", self.project.next_note_id));
        for note in &self.project.clip.notes {
            out.push_str(&format!(
                "note\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                note.id,
                note.start_beats,
                note.duration_beats,
                note.key_index,
                note.musical_note,
                note.raw_channel,
                note.raw_note,
                note.velocity,
                note.freq,
                note.mapped_from_lumatone
            ));
        }
        out
    }

    pub(crate) fn from_text(data: &str) -> Result<Self, String> {
        let mut version_seen = false;
        let mut scala_path = None;
        let mut lumatone_path = None;
        let mut root_midi = 69;
        let mut base_freq = 440.0;
        let mut synth_settings = SynthSettings::default();
        let mut transport = TransportSnapshot {
            bpm: DEFAULT_BPM,
            loop_beats: DEFAULT_LOOP_BEATS,
            overdub: false,
            quantize_grid: QuantizeGrid::Sixteenth,
            quantize_on_record: true,
            metronome_enabled: false,
        };
        let mut next_note_id = 1;
        let mut notes = Vec::new();

        for (line_idx, line) in data.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("note\t") {
                notes.push(parse_note(rest, line_idx + 1)?);
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("Invalid project line {}: {line}", line_idx + 1))?;
            match key {
                "orbifold_project" | "microtonal_daw_project" => version_seen = value == "1",
                "scala_path" => scala_path = optional_path(value),
                "lumatone_path" => lumatone_path = optional_path(value),
                "root_midi" => root_midi = parse_i32(value, key)?,
                "base_freq" => base_freq = parse_positive_f32(value, key)?,
                "waveform" => {
                    synth_settings.waveform = Waveform::from_str(value)
                        .ok_or_else(|| format!("Invalid waveform: {value}"))?;
                }
                "master_gain" => synth_settings.master_gain = parse_non_negative_f32(value, key)?,
                "attack_ms" => synth_settings.attack_ms = parse_non_negative_f32(value, key)?,
                "release_ms" => synth_settings.release_ms = parse_non_negative_f32(value, key)?,
                "drive" => synth_settings.drive = parse_non_negative_f32(value, key)?,
                "filter_cutoff_hz" => {
                    synth_settings.filter_cutoff_hz = parse_positive_f32(value, key)?
                }
                "delay_mix" => synth_settings.delay_mix = parse_non_negative_f32(value, key)?,
                "delay_feedback" => {
                    synth_settings.delay_feedback = parse_non_negative_f32(value, key)?
                }
                "delay_time_ms" => {
                    synth_settings.delay_time_ms = parse_non_negative_f32(value, key)?
                }
                "bpm" => transport.bpm = parse_positive_f32(value, key)?,
                "loop_beats" => transport.loop_beats = parse_positive_f32(value, key)?,
                "overdub" => transport.overdub = parse_bool(value, key)?,
                "quantize_grid" => {
                    transport.quantize_grid = QuantizeGrid::from_str(value)
                        .ok_or_else(|| format!("Invalid quantize grid: {value}"))?;
                }
                "quantize_on_record" => transport.quantize_on_record = parse_bool(value, key)?,
                "metronome_enabled" => transport.metronome_enabled = parse_bool(value, key)?,
                "next_note_id" => next_note_id = parse_u64(value, key)?,
                _ => return Err(format!("Unknown project key: {key}")),
            }
        }
        if !version_seen {
            return Err("Not an Orbifold project file".to_string());
        }
        Ok(Self {
            scala_path,
            lumatone_path,
            root_midi,
            base_freq,
            synth_settings,
            project: ProjectSnapshot {
                transport,
                clip: Clip { notes },
                next_note_id,
            },
        })
    }
}

pub(crate) fn playback_note_id(note_id: u64) -> u32 {
    PLAYBACK_NOTE_BASE.saturating_add((note_id % 500_000) as u32)
}

pub(crate) fn active_key_set(notes: &[ClipNote]) -> HashSet<u32> {
    notes
        .iter()
        .filter_map(|note| (note.key_index >= 0).then_some(note.key_index as u32))
        .collect()
}

fn note_active_at(note: &ClipNote, beat: f32, loop_beats: f32) -> bool {
    if note.duration_beats >= loop_beats {
        return true;
    }
    let start = wrap_beat(note.start_beats, loop_beats);
    let end = note.end_beats(loop_beats);
    if start <= end {
        beat >= start && beat < end
    } else {
        beat >= start || beat < end
    }
}

fn beat_distance(start: f32, end: f32, loop_beats: f32) -> f32 {
    if end >= start {
        end - start
    } else {
        (loop_beats - start) + end
    }
}

fn wrap_beat(beat: f32, loop_beats: f32) -> f32 {
    let loop_beats = loop_beats.max(1.0);
    beat.rem_euclid(loop_beats)
}

fn quantize_beat(beat: f32, step: f32, loop_beats: f32) -> f32 {
    wrap_beat((beat / step).round() * step, loop_beats)
}

fn quantize_duration(duration: f32, step: f32) -> f32 {
    ((duration / step).round() * step)
        .max(step)
        .max(MIN_NOTE_BEATS)
}

fn push_optional_path(out: &mut String, key: &str, value: Option<&PathBuf>) {
    out.push_str(key);
    out.push('=');
    if let Some(path) = value.and_then(|path| path.to_str()) {
        out.push_str(path);
    }
    out.push('\n');
}

fn optional_path(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn parse_note(rest: &str, line: usize) -> Result<ClipNote, String> {
    let parts: Vec<&str> = rest.split('\t').collect();
    if parts.len() != 10 {
        return Err(format!("Invalid note line {line}"));
    }
    Ok(ClipNote {
        id: parse_u64(parts[0], "note id")?,
        start_beats: parse_non_negative_f32(parts[1], "note start")?,
        duration_beats: parse_positive_f32(parts[2], "note duration")?,
        key_index: parse_i32(parts[3], "note key")?,
        musical_note: parse_i32(parts[4], "note pitch")?,
        raw_channel: parse_u8(parts[5], "note channel")?,
        raw_note: parse_u8(parts[6], "note raw")?,
        velocity: parse_u8(parts[7], "note velocity")?,
        freq: parse_positive_f32(parts[8], "note freq")?,
        mapped_from_lumatone: parse_bool(parts[9], "note mapped")?,
    })
}

fn parse_u8(value: &str, key: &str) -> Result<u8, String> {
    value
        .parse::<u8>()
        .map_err(|_| format!("Invalid integer for {key}: {value}"))
}

fn parse_u64(value: &str, key: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("Invalid integer for {key}: {value}"))
}

fn parse_i32(value: &str, key: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .map_err(|_| format!("Invalid integer for {key}: {value}"))
}

fn parse_positive_f32(value: &str, key: &str) -> Result<f32, String> {
    let value = parse_f32(value, key)?;
    if value <= 0.0 {
        return Err(format!("{key} must be positive"));
    }
    Ok(value)
}

fn parse_non_negative_f32(value: &str, key: &str) -> Result<f32, String> {
    let value = parse_f32(value, key)?;
    if value < 0.0 {
        return Err(format!("{key} must be non-negative"));
    }
    Ok(value)
}

fn parse_f32(value: &str, key: &str) -> Result<f32, String> {
    let value = value
        .parse::<f32>()
        .map_err(|_| format!("Invalid number for {key}: {value}"))?;
    if !value.is_finite() {
        return Err(format!("{key} must be finite"));
    }
    Ok(value)
}

fn parse_bool(value: &str, key: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("Invalid boolean for {key}: {value}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::MidiEvent;
    use std::time::{Duration, Instant};

    fn note_event(status: u8, note: u8, velocity: u8, at: Instant) -> MidiEvent {
        MidiEvent {
            raw_status: status,
            status: status & 0xF0,
            channel: status & 0x0F,
            midi_note: note,
            velocity,
            key_index: note as i32,
            musical_note: note as i32,
            mapped_from_lumatone: false,
            freq: Some(440.0),
            scale_degree: Some(0),
            scale_octave: Some(0),
            cents_from_root: Some(0.0),
            at,
        }
    }

    #[test]
    fn recorder_quantizes_note_start_and_duration() {
        let mut project = MusicProject::default();
        project.transport.quantize_grid = QuantizeGrid::Sixteenth;
        project.transport.quantize_on_record = true;
        let start = Instant::now();
        project.start_recording(start);
        project.record_midi_event(&note_event(
            0x90,
            60,
            100,
            start + Duration::from_millis(70),
        ));
        project.record_midi_event(&note_event(0x80, 60, 0, start + Duration::from_millis(390)));

        assert_eq!(project.clip.notes.len(), 1);
        assert_eq!(project.clip.notes[0].start_beats, 0.25);
        assert_eq!(project.clip.notes[0].duration_beats, 0.75);
    }

    #[test]
    fn recorder_holds_note_until_sustain_releases() {
        let mut project = MusicProject::default();
        project.transport.quantize_on_record = false;
        let start = Instant::now();
        project.start_recording(start);

        project.record_midi_event(&note_event(0x90, 60, 100, start));
        project.record_midi_event(&note_event(
            0xB0,
            64,
            127,
            start + Duration::from_millis(100),
        ));
        project.record_midi_event(&note_event(0x80, 60, 0, start + Duration::from_millis(200)));

        assert!(project.clip.notes.is_empty());

        project.record_midi_event(&note_event(0xB0, 64, 0, start + Duration::from_millis(600)));

        assert_eq!(project.clip.notes.len(), 1);
        assert!((project.clip.notes[0].duration_beats - 1.2).abs() < 0.001);
    }

    #[test]
    fn transport_pause_preserves_position_and_stop_resets() {
        let mut project = MusicProject::default();
        project.transport.bpm = 120.0;
        let start = Instant::now();
        let one_beat_later = start + Duration::from_millis(500);
        let two_beats_later = start + Duration::from_millis(1_000);

        project.play(start);
        assert_eq!(project.current_position_beats(start), 0.0);
        assert_eq!(project.current_position_beats(one_beat_later), 1.0);

        project.pause(one_beat_later);
        assert!(!project.transport.playing);
        assert_eq!(project.current_position_beats(two_beats_later), 1.0);

        project.play(two_beats_later);
        assert_eq!(project.current_position_beats(two_beats_later), 1.0);

        project.seek(5.0, two_beats_later);
        assert_eq!(project.current_position_beats(two_beats_later), 5.0);
        assert_eq!(
            project.current_position_beats(two_beats_later + Duration::from_millis(500)),
            6.0
        );

        project.stop(two_beats_later);
        assert_eq!(project.current_position_beats(two_beats_later), 0.0);
    }

    #[test]
    fn active_notes_wrap_across_loop_boundary() {
        let mut project = MusicProject::default();
        project.transport.loop_beats = 4.0;
        project.clip.notes.push(ClipNote {
            id: 1,
            start_beats: 3.5,
            duration_beats: 1.0,
            key_index: 60,
            musical_note: 60,
            raw_channel: 0,
            raw_note: 60,
            velocity: 100,
            freq: 440.0,
            mapped_from_lumatone: false,
        });

        assert_eq!(project.active_notes_at(3.75).len(), 1);
        assert_eq!(project.active_notes_at(0.25).len(), 1);
        assert_eq!(project.active_notes_at(1.0).len(), 0);
    }

    #[test]
    fn clip_note_editing_wraps_and_clamps_to_transport() {
        let mut project = MusicProject::default();
        project.transport.loop_beats = 4.0;
        project.transport.quantize_grid = QuantizeGrid::Quarter;
        project.next_note_id = 2;
        project.clip.notes.push(ClipNote {
            id: 1,
            start_beats: 0.5,
            duration_beats: 0.5,
            key_index: 42,
            musical_note: 60,
            raw_channel: 0,
            raw_note: 60,
            velocity: 99,
            freq: 261.63,
            mapped_from_lumatone: true,
        });

        let duplicate_id = project.duplicate_note(1).expect("note should duplicate");
        assert_eq!(duplicate_id, 2);
        assert_eq!(project.note_by_id(2).unwrap().start_beats, 1.5);

        assert!(project.nudge_note(2, -2.0));
        assert_eq!(project.note_by_id(2).unwrap().start_beats, 3.5);

        assert!(project.resize_note(2, -10.0));
        assert_eq!(
            project.note_by_id(2).unwrap().duration_beats,
            MIN_NOTE_BEATS
        );

        assert!(project.set_note_duration(2, 10.0));
        assert_eq!(project.note_by_id(2).unwrap().duration_beats, 4.0);

        assert!(project.set_note_start_preserving_end(2, 2.5));
        let start_resized = project.note_by_id(2).unwrap();
        assert_eq!(start_resized.start_beats, 2.5);
        assert_eq!(start_resized.duration_beats, 1.0);

        assert!(project.set_note_velocity(2, 200));
        assert_eq!(project.note_by_id(2).unwrap().velocity, 127);

        assert!(project.set_note_pitch(2, 64, 329.63));
        let edited = project.note_by_id(2).unwrap();
        assert_eq!(edited.musical_note, 64);
        assert_eq!(edited.raw_note, 64);
        assert_eq!(edited.key_index, -1);
        assert!(!edited.mapped_from_lumatone);

        assert!(project.set_note_start_and_pitch(2, 4.75, 65, 349.23));
        let dragged = project.note_by_id(2).unwrap();
        assert_eq!(dragged.start_beats, 0.75);
        assert_eq!(dragged.musical_note, 65);
        assert_eq!(dragged.freq, 349.23);
        assert_eq!(dragged.raw_note, 65);
        assert_eq!(dragged.key_index, -1);
        assert!(!dragged.mapped_from_lumatone);

        let inserted_id = project.add_note(4.25, 0.5, 67, 96, 392.0);
        let inserted = project.note_by_id(inserted_id).unwrap();
        assert_eq!(inserted.start_beats, 0.25);
        assert_eq!(inserted.musical_note, 67);

        assert!(project.delete_note(1));
        assert!(!project.delete_note(1));
    }

    #[test]
    fn project_file_round_trips_notes_and_transport() {
        let mut project = MusicProject::default();
        project.transport.bpm = 96.0;
        project.transport.loop_beats = 8.0;
        project.transport.metronome_enabled = true;
        project.clip.notes.push(ClipNote {
            id: 7,
            start_beats: 1.0,
            duration_beats: 0.5,
            key_index: 42,
            musical_note: 60,
            raw_channel: 0,
            raw_note: 60,
            velocity: 99,
            freq: 261.63,
            mapped_from_lumatone: true,
        });
        let file = ProjectFile {
            scala_path: Some(PathBuf::from("scale.scl")),
            lumatone_path: Some(PathBuf::from("classic.ltn")),
            root_midi: 60,
            base_freq: 261.63,
            synth_settings: SynthSettings::default(),
            project: project.snapshot(),
        };

        let parsed = ProjectFile::from_text(&file.to_text()).expect("project should parse");
        assert_eq!(parsed, file);
    }

    #[test]
    fn legacy_microtonal_daw_project_fixture_loads() {
        let parsed = ProjectFile::from_text(include_str!(
            "../tests/fixtures/projects/legacy_microtonal_daw_project.mtdaw"
        ))
        .expect("legacy project fixture should parse");

        assert_eq!(parsed.root_midi, 69);
        assert_eq!(parsed.base_freq, 440.0);
        assert_eq!(parsed.project.transport.loop_beats, 8.0);
        assert!(parsed.project.transport.quantize_on_record);
        assert!(!parsed.project.transport.metronome_enabled);
        assert_eq!(parsed.project.next_note_id, 8);
        assert_eq!(parsed.project.clip.notes.len(), 1);
        let note = &parsed.project.clip.notes[0];
        assert_eq!(note.id, 7);
        assert_eq!(note.key_index, 42);
        assert_eq!(note.musical_note, 60);
        assert!(note.mapped_from_lumatone);
    }

    #[test]
    fn orbifold_v1_project_fixture_loads() {
        let parsed = ProjectFile::from_text(include_str!(
            "../tests/fixtures/projects/orbifold_v1_project.orbifold"
        ))
        .expect("current project fixture should parse");

        assert_eq!(parsed.root_midi, 60);
        assert_eq!(parsed.base_freq, 261.63);
        assert_eq!(parsed.synth_settings.waveform, Waveform::Triangle);
        assert_eq!(parsed.project.transport.bpm, 96.0);
        assert_eq!(parsed.project.transport.loop_beats, 12.0);
        assert!(parsed.project.transport.overdub);
        assert_eq!(parsed.project.transport.quantize_grid, QuantizeGrid::Eighth);
        assert!(!parsed.project.transport.quantize_on_record);
        assert!(parsed.project.transport.metronome_enabled);
        assert_eq!(parsed.project.next_note_id, 10);
        assert_eq!(parsed.project.clip.notes.len(), 1);
        let note = &parsed.project.clip.notes[0];
        assert_eq!(note.id, 9);
        assert_eq!(note.start_beats, 2.5);
        assert_eq!(note.duration_beats, 0.75);
        assert_eq!(note.key_index, -1);
        assert_eq!(note.musical_note, 64);
        assert_eq!(note.raw_channel, 1);
        assert_eq!(note.raw_note, 61);
        assert_eq!(note.velocity, 88);
        assert!(!note.mapped_from_lumatone);
    }
}
