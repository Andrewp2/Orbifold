use cpal::traits::StreamTrait;
use midir::{Ignore, MidiInput, MidiInputConnection};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::audio::{AudioOutputDevice, build_audio_stream, list_audio_outputs};
use crate::midi::{
    MidiSharedState, SharedLumatoneMap, SharedMidiCapture, SharedMidiLast, SharedMidiLog,
    handle_midi, list_midi_inputs, load_lumatone_map,
};
use crate::project::{
    ClipNote, ProjectFile, ProjectSnapshot, SharedMusicProject, active_key_set, playback_note_id,
};
use crate::scala::parse_scala;
use crate::scale::ScaleState;
use crate::settings::AppSettings;
use crate::synth::{SynthHandle, SynthSettings};

const MAX_PROJECT_HISTORY: usize = 64;
const METRONOME_NOTE_ID: u32 = 1_900_000;
const AUDIO_ASSETS_DIR: &str = "audio_assets";

#[derive(Clone, Debug)]
pub(crate) struct LumatonePreset {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct ScaleLibraryItem {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioAssetKind {
    Sample,
    Instrument,
    Preset,
    Impulse,
}

impl AudioAssetKind {
    pub(crate) fn all() -> [Self; 4] {
        [Self::Sample, Self::Instrument, Self::Preset, Self::Impulse]
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Sample => "Samples",
            Self::Instrument => "Instruments",
            Self::Preset => "Presets",
            Self::Impulse => "Impulses",
        }
    }

    fn singular_label(self) -> &'static str {
        match self {
            Self::Sample => "sample",
            Self::Instrument => "instrument",
            Self::Preset => "preset",
            Self::Impulse => "impulse",
        }
    }

    fn folder(self) -> &'static str {
        match self {
            Self::Sample => "samples",
            Self::Instrument => "instruments",
            Self::Preset => "presets",
            Self::Impulse => "impulses",
        }
    }

    pub(crate) fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Sample => &["wav", "aif", "aiff", "flac", "ogg", "mp3"],
            Self::Instrument => &["sfz", "sf2", "json", "toml", "yaml", "yml"],
            Self::Preset => &["json", "toml", "yaml", "yml", "ron", "preset"],
            Self::Impulse => &["wav", "aif", "aiff", "flac"],
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AudioAssetItem {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) kind: AudioAssetKind,
    pub(crate) is_dir: bool,
}

pub(crate) struct AppState {
    pub(crate) scale_state: Arc<Mutex<ScaleState>>,
    pub(crate) synth: SynthHandle,
    pub(crate) midi_last: SharedMidiLast,
    pub(crate) midi_log: SharedMidiLog,
    pub(crate) midi_capture: SharedMidiCapture,
    pub(crate) music_project: SharedMusicProject,
    pub(crate) midi_connection: Option<MidiInputConnection<()>>,
    pub(crate) midi_inputs: Vec<String>,
    pub(crate) selected_input: usize,
    pub(crate) audio_stream: Option<cpal::Stream>,
    pub(crate) audio_outputs: Vec<AudioOutputDevice>,
    pub(crate) selected_audio_output: usize,
    pub(crate) connected_audio_output: String,
    pub(crate) last_status: String,
    pub(crate) scala_path: Option<PathBuf>,
    pub(crate) scale_library: Vec<ScaleLibraryItem>,
    pub(crate) selected_scale_library: usize,
    pub(crate) audio_assets: Vec<AudioAssetItem>,
    pub(crate) selected_audio_asset: Option<usize>,
    pub(crate) selected_audio_asset_kind: AudioAssetKind,
    pub(crate) midi_debug: Arc<AtomicBool>,
    pub(crate) lumatone_map: SharedLumatoneMap,
    pub(crate) lumatone_path: Option<PathBuf>,
    pub(crate) lumatone_presets: Vec<LumatonePreset>,
    pub(crate) selected_lumatone: usize,
    pub(crate) show_scale_library: bool,
    pub(crate) show_inspector: bool,
    pub(crate) show_key_labels: bool,
    pub(crate) screenshot_on_start: bool,
    pub(crate) screenshot_requested: bool,
    pub(crate) exit_after_screenshot: bool,
    pub(crate) project_path: Option<PathBuf>,
    pub(crate) playback_active_keys: HashSet<u32>,
    pub(crate) selected_clip_note: Option<u64>,
    playback_active_notes: HashMap<u64, u32>,
    undo_stack: Vec<ProjectSnapshot>,
    redo_stack: Vec<ProjectSnapshot>,
    last_metronome_beat: Option<u32>,
    settings: AppSettings,
    settings_path: PathBuf,
    persist_enabled: bool,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        scale_state: Arc<Mutex<ScaleState>>,
        synth: SynthHandle,
        midi_last: SharedMidiLast,
        midi_log: SharedMidiLog,
        midi_capture: SharedMidiCapture,
        music_project: SharedMusicProject,
        audio_stream: cpal::Stream,
        connected_audio_output: String,
        settings: AppSettings,
        settings_status: Option<String>,
        screenshot_on_start: bool,
    ) -> Self {
        let mut app = Self {
            scale_state,
            synth,
            midi_last,
            midi_log,
            midi_capture,
            music_project,
            midi_connection: None,
            midi_inputs: Vec::new(),
            selected_input: 0,
            audio_stream: Some(audio_stream),
            audio_outputs: list_audio_outputs(),
            selected_audio_output: 0,
            connected_audio_output,
            last_status: settings_status.unwrap_or_else(|| "Ready".to_string()),
            scala_path: None,
            scale_library: Vec::new(),
            selected_scale_library: 0,
            audio_assets: Vec::new(),
            selected_audio_asset: None,
            selected_audio_asset_kind: AudioAssetKind::Sample,
            midi_debug: Arc::new(AtomicBool::new(settings.midi_debug)),
            lumatone_map: Arc::new(Mutex::new(None)),
            lumatone_path: None,
            lumatone_presets: Vec::new(),
            selected_lumatone: 0,
            show_scale_library: true,
            show_inspector: false,
            show_key_labels: true,
            screenshot_on_start,
            screenshot_requested: false,
            exit_after_screenshot: screenshot_on_start,
            project_path: None,
            playback_active_keys: HashSet::new(),
            selected_clip_note: None,
            playback_active_notes: HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_metronome_beat: None,
            settings,
            settings_path: AppSettings::default_path(),
            persist_enabled: false,
        };

        app.select_connected_audio_output();
        app.load_lumatone_presets(Path::new("lumatone_factory_presets"));
        app.select_saved_or_default_lumatone();
        app.refresh_scale_library();
        app.ensure_audio_asset_dirs();
        app.refresh_audio_assets();
        if let Some(path) = app.settings.scala_path.clone()
            && let Err(err) = app.load_scale_path(path, true)
        {
            app.last_status = format!("Saved Scala load error: {err}");
        }
        let preferred_midi = app.settings.midi_input_name.clone();
        app.refresh_midi_inputs(preferred_midi.as_deref());
        app.open_midi_input();
        app.persist_enabled = true;
        app.persist_settings(None);
        app
    }

    pub(crate) fn refresh_audio_outputs(&mut self) {
        self.audio_outputs = list_audio_outputs();
        self.select_connected_audio_output();
    }

    pub(crate) fn connect_audio_output(&mut self) {
        let Some(name) = self
            .audio_outputs
            .get(self.selected_audio_output)
            .map(|device| device.name.clone())
        else {
            self.last_status = "No audio output selected".to_string();
            return;
        };

        match build_audio_stream(&self.synth, Some(&name)) {
            Ok((stream, connected_name, sender)) => {
                if let Err(err) = stream.play() {
                    self.last_status = format!("Audio playback failed: {err}");
                    return;
                }
                self.synth.install_sender(sender);
                self.audio_stream = Some(stream);
                self.connected_audio_output = connected_name.clone();
                self.last_status = format!("Connected audio output: {connected_name}");
                self.persist_settings(None);
            }
            Err(err) => {
                self.last_status = format!("Audio output error: {err}");
            }
        }
    }

    pub(crate) fn refresh_midi_inputs(&mut self, preferred_name: Option<&str>) {
        self.midi_inputs = list_midi_inputs();
        if self.midi_inputs.is_empty() {
            self.selected_input = 0;
            return;
        }
        if let Some(idx) = preferred_name
            .and_then(|preferred| self.midi_inputs.iter().position(|name| name == preferred))
        {
            self.selected_input = idx;
        } else if let Some(idx) = self
            .midi_inputs
            .iter()
            .position(|name| name.to_lowercase().contains("lumatone"))
        {
            self.selected_input = idx;
        } else if self.selected_input >= self.midi_inputs.len() {
            self.selected_input = 0;
        }
    }

    pub(crate) fn open_midi_input(&mut self) {
        if self.midi_inputs.is_empty() {
            self.midi_inputs = list_midi_inputs();
        }
        if self.midi_inputs.is_empty() {
            self.last_status = "No MIDI inputs found".to_string();
            self.midi_connection = None;
            self.persist_settings(None);
            return;
        }
        if self.selected_input >= self.midi_inputs.len() {
            self.selected_input = 0;
        }

        let idx = self.selected_input;
        let scale_state = self.scale_state.clone();
        let synth = self.synth.clone();
        let midi_state = MidiSharedState {
            last: self.midi_last.clone(),
            log: self.midi_log.clone(),
            capture: self.midi_capture.clone(),
            lumatone_map: self.lumatone_map.clone(),
            music_project: self.music_project.clone(),
        };
        let midi_debug = self.midi_debug.clone();

        let Ok(mut midi_in) = MidiInput::new("orbifold") else {
            self.last_status = "Failed to initialize MIDI input".to_string();
            self.midi_connection = None;
            return;
        };
        midi_in.ignore(Ignore::None);
        let ports = midi_in.ports();
        if idx >= ports.len() {
            self.last_status = "Selected MIDI input not available".to_string();
            self.midi_connection = None;
            return;
        }
        let port = ports[idx].clone();
        let port_name = midi_in
            .port_name(&port)
            .unwrap_or_else(|_| "Unknown".to_string());
        let conn = midi_in
            .connect(
                &port,
                "orbifold-input",
                move |_, message, _| {
                    handle_midi(
                        message,
                        &scale_state,
                        &synth,
                        &midi_state,
                        midi_debug.load(Ordering::Relaxed),
                    );
                },
                (),
            )
            .ok();

        if conn.is_some() {
            self.last_status = format!("Connected MIDI input: {port_name}");
        } else {
            self.last_status = format!("Failed to connect MIDI input: {port_name}");
        }
        self.midi_connection = conn;
        self.persist_settings(None);
    }

    pub(crate) fn set_synth_settings(&mut self, settings: SynthSettings) {
        match self.synth.set_settings(settings) {
            Ok(()) => self.persist_settings(None),
            Err(err) => self.last_status = format!("Synth settings error: {err}"),
        }
    }

    pub(crate) fn all_notes_off(&mut self) {
        self.stop_playback_notes();
        match self.synth.all_notes_off() {
            Ok(()) => self.last_status = "All notes off".to_string(),
            Err(err) => self.last_status = format!("All notes off error: {err}"),
        }
    }

    pub(crate) fn play_transport(&mut self) {
        self.stop_playback_notes();
        self.music_project.lock().play(std::time::Instant::now());
        self.last_metronome_beat = None;
        self.last_status = "Transport playing".to_string();
    }

    pub(crate) fn stop_transport(&mut self) {
        self.music_project.lock().stop(std::time::Instant::now());
        self.stop_playback_notes();
        self.last_metronome_beat = None;
        self.last_status = "Transport stopped".to_string();
    }

    pub(crate) fn toggle_transport(&mut self) {
        if self.music_project.lock().transport.playing {
            self.stop_transport();
        } else {
            self.play_transport();
        }
    }

    pub(crate) fn start_recording(&mut self) {
        self.stop_playback_notes();
        self.push_project_history();
        self.music_project
            .lock()
            .start_recording(std::time::Instant::now());
        self.last_metronome_beat = None;
        self.last_status = "Recording".to_string();
    }

    pub(crate) fn stop_recording(&mut self) {
        self.music_project
            .lock()
            .stop_recording(std::time::Instant::now());
        self.last_status = "Recording stopped".to_string();
    }

    pub(crate) fn toggle_recording(&mut self) {
        if self.music_project.lock().transport.recording {
            self.stop_recording();
        } else {
            self.start_recording();
        }
    }

    pub(crate) fn clear_clip(&mut self) {
        self.stop_playback_notes();
        self.push_project_history();
        self.music_project.lock().clear_clip();
        self.selected_clip_note = None;
        self.last_status = "Clip cleared".to_string();
    }

    pub(crate) fn quantize_clip(&mut self) {
        self.push_project_history();
        self.music_project.lock().quantize_clip();
        self.last_status = "Clip quantized".to_string();
    }

    pub(crate) fn select_clip_note(&mut self, note_id: Option<u64>) {
        self.selected_clip_note =
            note_id.filter(|id| self.music_project.lock().note_by_id(*id).is_some());
    }

    pub(crate) fn selected_clip_note(&self) -> Option<ClipNote> {
        self.selected_clip_note
            .and_then(|id| self.music_project.lock().note_by_id(id))
    }

    pub(crate) fn delete_selected_clip_note(&mut self) {
        let Some(note_id) = self.selected_clip_note else {
            self.last_status = "No clip note selected".to_string();
            return;
        };
        self.push_project_history();
        if self.music_project.lock().delete_note(note_id) {
            self.selected_clip_note = None;
            self.last_status = "Deleted clip note".to_string();
        } else {
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn duplicate_selected_clip_note(&mut self) {
        let Some(note_id) = self.selected_clip_note else {
            self.last_status = "No clip note selected".to_string();
            return;
        };
        self.push_project_history();
        match self.music_project.lock().duplicate_note(note_id) {
            Some(new_id) => {
                self.selected_clip_note = Some(new_id);
                self.last_status = "Duplicated clip note".to_string();
            }
            None => self.last_status = "Selected clip note no longer exists".to_string(),
        }
    }

    pub(crate) fn nudge_selected_clip_note(&mut self, direction: f32) {
        let Some(note_id) = self.selected_clip_note else {
            self.last_status = "No clip note selected".to_string();
            return;
        };
        let step = self.music_project.lock().edit_step_beats() * direction;
        self.push_project_history();
        if self.music_project.lock().nudge_note(note_id, step) {
            self.last_status = "Moved clip note".to_string();
        } else {
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn resize_selected_clip_note(&mut self, direction: f32) {
        let Some(note_id) = self.selected_clip_note else {
            self.last_status = "No clip note selected".to_string();
            return;
        };
        let step = self.music_project.lock().edit_step_beats() * direction;
        self.push_project_history();
        if self.music_project.lock().resize_note(note_id, step) {
            self.last_status = "Resized clip note".to_string();
        } else {
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn set_selected_clip_note_velocity(&mut self, velocity: u8) {
        let Some(note_id) = self.selected_clip_note else {
            return;
        };
        self.push_project_history();
        if self
            .music_project
            .lock()
            .set_note_velocity(note_id, velocity)
        {
            self.last_status = "Updated clip note velocity".to_string();
        } else {
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn transpose_selected_clip_note(&mut self, direction: i32) {
        let Some(note) = self.selected_clip_note() else {
            self.last_status = "No clip note selected".to_string();
            return;
        };
        let musical_note = note.musical_note.saturating_add(direction);
        let Some(info) = self.scale_state.lock().note_info(musical_note) else {
            self.last_status = "Selected pitch cannot be tuned".to_string();
            return;
        };
        self.push_project_history();
        if self
            .music_project
            .lock()
            .set_note_pitch(note.id, musical_note, info.freq)
        {
            self.last_status = "Moved clip note pitch".to_string();
        } else {
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn add_clip_note_at(&mut self, beat: f32, musical_note: i32) {
        let Some(info) = self.scale_state.lock().note_info(musical_note) else {
            self.last_status = "Pitch cannot be tuned".to_string();
            return;
        };
        let (start, duration) = {
            let project = self.music_project.lock();
            let loop_beats = project.transport.loop_beats.max(1.0);
            let start = project
                .transport
                .quantize_grid
                .step_beats()
                .map(|step| ((beat / step).round() * step).rem_euclid(loop_beats))
                .unwrap_or_else(|| beat.rem_euclid(loop_beats));
            (start, project.edit_step_beats())
        };
        self.push_project_history();
        let note_id =
            self.music_project
                .lock()
                .add_note(start, duration, musical_note, 96, info.freq);
        self.selected_clip_note = Some(note_id);
        self.last_status = "Added note".to_string();
    }

    pub(crate) fn can_undo_project_edit(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub(crate) fn can_redo_project_edit(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub(crate) fn undo_project_edit(&mut self) {
        let Some(snapshot) = self.undo_stack.pop() else {
            self.last_status = "Nothing to undo".to_string();
            return;
        };
        let current = self.music_project.lock().snapshot();
        self.redo_stack.push(current);
        self.apply_project_history_snapshot(snapshot);
        self.last_status = "Undid clip edit".to_string();
    }

    pub(crate) fn redo_project_edit(&mut self) {
        let Some(snapshot) = self.redo_stack.pop() else {
            self.last_status = "Nothing to redo".to_string();
            return;
        };
        let current = self.music_project.lock().snapshot();
        self.undo_stack.push(current);
        self.apply_project_history_snapshot(snapshot);
        self.last_status = "Redid clip edit".to_string();
    }

    pub(crate) fn update_music_playback(&mut self) {
        let now = std::time::Instant::now();
        let (desired_notes, current_beat, metronome_enabled) = {
            let mut project = self.music_project.lock();
            if !project.transport.playing {
                (Vec::new(), None, false)
            } else {
                let beat = project.current_position_beats(now);
                project.set_last_position(beat);
                (
                    project.active_notes_at(beat),
                    Some(beat),
                    project.transport.metronome_enabled,
                )
            }
        };

        if let Some(beat) = current_beat {
            self.update_metronome(beat, metronome_enabled);
        } else {
            self.last_metronome_beat = None;
        }

        self.playback_active_keys = active_key_set(&desired_notes);
        let desired_ids: HashSet<u64> = desired_notes.iter().map(|note| note.id).collect();
        let active_ids: Vec<u64> = self.playback_active_notes.keys().copied().collect();
        for note_id in active_ids {
            if !desired_ids.contains(&note_id)
                && let Some(synth_note) = self.playback_active_notes.remove(&note_id)
                && let Err(err) = self.synth.note_off(synth_note)
            {
                self.last_status = format!("Playback note-off error: {err}");
            }
        }

        for note in desired_notes {
            if self.playback_active_notes.contains_key(&note.id) {
                continue;
            }
            let synth_note = playback_note_id(note.id);
            let velocity = (note.velocity as f32 / 127.0).clamp(0.0, 1.0);
            match self.synth.note_on(synth_note, note.freq, velocity) {
                Ok(()) => {
                    self.playback_active_notes.insert(note.id, synth_note);
                }
                Err(err) => self.last_status = format!("Playback note-on error: {err}"),
            }
        }
    }

    pub(crate) fn save_project_to_path(&mut self, path: PathBuf) {
        let project_file = self.project_file_snapshot();
        match std::fs::write(&path, project_file.to_text()) {
            Ok(()) => {
                self.project_path = Some(path.clone());
                self.last_status = format!("Saved project: {}", path.display());
            }
            Err(err) => self.last_status = format!("Project save error: {err}"),
        }
    }

    pub(crate) fn save_project(&mut self) -> bool {
        if let Some(path) = self.project_path.clone() {
            self.save_project_to_path(path);
            true
        } else {
            false
        }
    }

    pub(crate) fn load_project_path(&mut self, path: PathBuf) {
        let data = match std::fs::read_to_string(&path) {
            Ok(data) => data,
            Err(err) => {
                self.last_status = format!("Project open error: {err}");
                return;
            }
        };
        let project = match ProjectFile::from_text(&data) {
            Ok(project) => project,
            Err(err) => {
                self.last_status = format!("Project parse error: {err}");
                return;
            }
        };

        self.stop_playback_notes();
        {
            let mut state = self.scale_state.lock();
            state.root_midi = project.root_midi;
            state.base_freq = project.base_freq;
        }
        if let Err(err) = self.synth.set_settings(project.synth_settings) {
            self.last_status = format!("Synth settings error: {err}");
            return;
        }
        if let Some(scala_path) = project.scala_path.clone()
            && let Err(err) = self.load_scale_path(scala_path, true)
        {
            self.last_status = format!("Project Scala load error: {err}");
            return;
        }
        if let Some(lumatone_path) = project.lumatone_path.clone() {
            self.load_lumatone_path(lumatone_path);
        }
        self.music_project.lock().apply_snapshot(project.project);
        self.selected_clip_note = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_metronome_beat = None;
        self.project_path = Some(path.clone());
        self.last_status = format!("Loaded project: {}", path.display());
        self.persist_settings(None);
    }

    pub(crate) fn start_mapping_capture(&mut self) {
        self.midi_capture.lock().start();
        self.last_status = "Mapping capture armed: play the keys to record note-ons".to_string();
    }

    pub(crate) fn stop_mapping_capture(&mut self) {
        let count = {
            let mut capture = self.midi_capture.lock();
            capture.stop();
            capture.len()
        };
        self.last_status = format!("Mapping capture stopped: {count} note-ons recorded");
    }

    pub(crate) fn clear_mapping_capture(&mut self) {
        self.midi_capture.lock().clear();
        self.last_status = "Mapping capture cleared".to_string();
    }

    pub(crate) fn test_tone(&mut self) {
        let synth = self.synth.clone();
        std::thread::spawn(move || {
            if let Err(err) = synth.note_on(69, 440.0, 0.6) {
                eprintln!("Audio command error: {err}");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
            if let Err(err) = synth.note_off(69) {
                eprintln!("Audio command error: {err}");
            }
        });
    }

    fn stop_playback_notes(&mut self) {
        let active: Vec<u32> = self.playback_active_notes.values().copied().collect();
        self.playback_active_notes.clear();
        self.playback_active_keys.clear();
        for note in active {
            if let Err(err) = self.synth.note_off(note) {
                self.last_status = format!("Playback note-off error: {err}");
            }
        }
    }

    fn push_project_history(&mut self) {
        let snapshot = self.music_project.lock().snapshot();
        if self.undo_stack.last() == Some(&snapshot) {
            return;
        }
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > MAX_PROJECT_HISTORY {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn apply_project_history_snapshot(&mut self, snapshot: ProjectSnapshot) {
        self.stop_playback_notes();
        self.music_project.lock().apply_snapshot(snapshot);
        if let Some(note_id) = self.selected_clip_note
            && self.music_project.lock().note_by_id(note_id).is_none()
        {
            self.selected_clip_note = None;
        }
        self.last_metronome_beat = None;
    }

    fn update_metronome(&mut self, beat: f32, enabled: bool) {
        if !enabled {
            self.last_metronome_beat = None;
            return;
        }
        let beat_idx = beat.floor().max(0.0) as u32;
        if self.last_metronome_beat == Some(beat_idx) {
            return;
        }
        self.last_metronome_beat = Some(beat_idx);
        self.trigger_metronome_click(beat_idx % 4 == 0);
    }

    fn trigger_metronome_click(&mut self, accented: bool) {
        let synth = self.synth.clone();
        let freq = if accented { 1760.0 } else { 1174.66 };
        let velocity = if accented { 0.34 } else { 0.22 };
        if let Err(err) = synth.note_on(METRONOME_NOTE_ID, freq, velocity) {
            self.last_status = format!("Metronome error: {err}");
            return;
        }
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(45));
            if let Err(err) = synth.note_off(METRONOME_NOTE_ID) {
                eprintln!("Metronome note-off error: {err}");
            }
        });
    }

    fn project_file_snapshot(&self) -> ProjectFile {
        let scale = self.scale_state.lock().clone();
        ProjectFile {
            scala_path: self.scala_path.clone(),
            lumatone_path: self.lumatone_path.clone(),
            root_midi: scale.root_midi,
            base_freq: scale.base_freq,
            synth_settings: self.synth.settings(),
            project: self.music_project.lock().snapshot(),
        }
    }

    pub(crate) fn load_scale_path(
        &mut self,
        path: PathBuf,
        add_to_library: bool,
    ) -> Result<(), String> {
        let scale = parse_scala(&path)?;
        {
            let mut state = self.scale_state.lock();
            state.scale = scale;
        }
        self.scala_path = Some(path.clone());
        if add_to_library {
            self.add_scale_library_path(path);
        }
        self.last_status = "Loaded Scala file".to_string();
        self.persist_settings(None);
        Ok(())
    }

    pub(crate) fn load_selected_library_scale(&mut self) {
        let Some(path) = self
            .scale_library
            .get(self.selected_scale_library)
            .map(|item| item.path.clone())
        else {
            self.last_status = "No scale selected".to_string();
            return;
        };
        if let Err(err) = self.load_scale_path(path, true) {
            self.last_status = format!("Scala parse error: {err}");
        }
    }

    pub(crate) fn remove_selected_library_scale(&mut self) {
        if self.selected_scale_library < self.scale_library.len() {
            self.scale_library.remove(self.selected_scale_library);
            if self.selected_scale_library >= self.scale_library.len() {
                self.selected_scale_library = self.scale_library.len().saturating_sub(1);
            }
            self.persist_settings(None);
        }
    }

    pub(crate) fn refresh_scale_library(&mut self) {
        let mut seen = HashSet::new();
        let mut library = Vec::new();
        let mut add_path = |path: PathBuf| {
            if path.extension().and_then(|value| value.to_str()) != Some("scl") {
                return;
            }
            if !seen.insert(path.clone()) {
                return;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("Unknown")
                .to_string();
            library.push(ScaleLibraryItem { name, path });
        };

        if let Some(path) = self.scala_path.clone() {
            add_path(path);
        }
        for path in self.settings.scale_library.clone() {
            add_path(path);
        }
        if let Ok(entries) = std::fs::read_dir("scales") {
            for entry in entries.flatten() {
                add_path(entry.path());
            }
        }

        library.sort_by(|a, b| a.name.cmp(&b.name));
        self.scale_library = library;
        if self.selected_scale_library >= self.scale_library.len() {
            self.selected_scale_library = self.scale_library.len().saturating_sub(1);
        }
    }

    pub(crate) fn ensure_audio_asset_dirs(&mut self) {
        for kind in AudioAssetKind::all() {
            let path = Path::new(AUDIO_ASSETS_DIR).join(kind.folder());
            if let Err(err) = std::fs::create_dir_all(&path) {
                self.last_status = format!("Asset folder error: {err}");
                return;
            }
        }
    }

    pub(crate) fn refresh_audio_assets(&mut self) {
        self.ensure_audio_asset_dirs();
        let mut assets = Vec::new();
        for kind in AudioAssetKind::all() {
            let root = Path::new(AUDIO_ASSETS_DIR).join(kind.folder());
            collect_audio_assets(kind, &root, &mut assets);
        }
        assets.sort_by(|a, b| {
            a.kind
                .label()
                .cmp(b.kind.label())
                .then_with(|| a.name.cmp(&b.name))
        });
        self.audio_assets = assets;
        if self
            .selected_audio_asset
            .is_some_and(|idx| idx >= self.audio_assets.len())
        {
            self.selected_audio_asset = None;
        }
    }

    pub(crate) fn select_audio_asset(&mut self, index: usize) {
        let Some(asset) = self.audio_assets.get(index) else {
            self.selected_audio_asset = None;
            return;
        };
        self.selected_audio_asset = Some(index);
        self.last_status = format!("Selected {}: {}", asset.kind.singular_label(), asset.name);
    }

    pub(crate) fn selected_audio_asset_item(&self) -> Option<&AudioAssetItem> {
        self.selected_audio_asset
            .and_then(|index| self.audio_assets.get(index))
    }

    pub(crate) fn import_audio_asset_path(&mut self, source: PathBuf, kind: AudioAssetKind) {
        let Some(file_name) = source
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
        else {
            self.last_status = "Asset import error: invalid file name".to_string();
            return;
        };
        if !is_supported_audio_asset_file(kind, &source) {
            self.last_status = format!("Asset import error: unsupported {}", kind.singular_label());
            return;
        }

        let dir = Path::new(AUDIO_ASSETS_DIR).join(kind.folder());
        if let Err(err) = std::fs::create_dir_all(&dir) {
            self.last_status = format!("Asset folder error: {err}");
            return;
        }
        let target = unique_asset_path(&dir, &file_name);
        match std::fs::copy(&source, &target) {
            Ok(_) => {
                self.refresh_audio_assets();
                self.selected_audio_asset = self
                    .audio_assets
                    .iter()
                    .position(|asset| asset.path == target);
                self.last_status = format!("Imported {}: {file_name}", kind.singular_label());
            }
            Err(err) => {
                self.last_status = format!("Asset import error: {err}");
            }
        }
    }

    pub(crate) fn select_lumatone(&mut self, index: usize) {
        if index >= self.lumatone_presets.len() {
            return;
        }
        let path = self.lumatone_presets[index].path.clone();
        self.load_lumatone_path(path);
    }

    pub(crate) fn load_lumatone_path(&mut self, path: PathBuf) {
        match load_lumatone_map(&path) {
            Ok(map) => {
                let key_count = map.len();
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Unknown")
                    .to_string();
                *self.lumatone_map.lock() = Some(Arc::new(map));
                self.lumatone_path = Some(path.clone());
                self.add_lumatone_preset_path(path);
                if let Some(idx) = self.select_lumatone_index_by_current_path() {
                    self.selected_lumatone = idx;
                }
                self.last_status = format!("Loaded key map: {name} ({key_count} keys)");
                self.persist_settings(None);
            }
            Err(err) => {
                self.last_status = format!("Key map load error: {err}");
            }
        }
    }

    pub(crate) fn reload_lumatone_presets(&mut self) {
        let current = self.lumatone_path.clone();
        self.load_lumatone_presets(Path::new("lumatone_factory_presets"));
        if let Some(path) = current {
            self.add_lumatone_preset_path(path.clone());
            if self.select_lumatone_by_path(&path) {
                return;
            }
        }
        self.select_saved_or_default_lumatone();
    }

    pub(crate) fn persist_settings_with_status(&mut self) {
        self.persist_settings(Some("Saved settings"));
    }

    pub(crate) fn persist_current_settings(&mut self) {
        self.persist_settings(None);
    }

    fn persist_settings(&mut self, success_status: Option<&str>) {
        if !self.persist_enabled {
            return;
        }
        self.capture_settings();
        match self.settings.save(&self.settings_path) {
            Ok(()) => {
                if let Some(status) = success_status {
                    self.last_status = status.to_string();
                }
            }
            Err(err) => self.last_status = format!("Settings save error: {err}"),
        }
    }

    fn capture_settings(&mut self) {
        {
            let state = self.scale_state.lock();
            self.settings.root_midi = state.root_midi;
            self.settings.base_freq = state.base_freq;
        }
        self.settings.audio_output_name = self
            .audio_outputs
            .get(self.selected_audio_output)
            .map(|device| device.name.clone())
            .or_else(|| Some(self.connected_audio_output.clone()));
        self.settings.midi_input_name = self.midi_inputs.get(self.selected_input).cloned();
        self.settings.scala_path = self.scala_path.clone();
        self.settings.lumatone_path = self.lumatone_path.clone();
        self.settings.midi_debug = self.midi_debug.load(Ordering::Relaxed);
        self.settings.apply_synth_settings(self.synth.settings());
        self.settings.scale_library = self
            .scale_library
            .iter()
            .map(|item| item.path.clone())
            .collect();
    }

    fn add_scale_library_path(&mut self, path: PathBuf) {
        if !self.scale_library.iter().any(|item| item.path == path) {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("Unknown")
                .to_string();
            self.scale_library.push(ScaleLibraryItem { name, path });
            self.scale_library.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }

    fn load_lumatone_presets(&mut self, dir: &Path) {
        let mut presets = Vec::new();
        let mut seen = HashSet::new();
        add_lumatone_presets_from_dir(dir, &mut presets, &mut seen);
        presets.sort_by(|a, b| a.name.cmp(&b.name));
        self.lumatone_presets = presets;
    }

    fn select_saved_or_default_lumatone(&mut self) {
        if let Some(path) = self.settings.lumatone_path.clone()
            && self.select_lumatone_by_path(&path)
        {
            return;
        } else if let Some(path) = self.settings.lumatone_path.clone()
            && path.exists()
        {
            self.load_lumatone_path(path);
            return;
        }
        let default_idx = self
            .lumatone_presets
            .iter()
            .position(|preset| preset.name == "1. Classic Mode.ltn");
        if let Some(idx) = default_idx {
            self.select_lumatone(idx);
        } else if !self.lumatone_presets.is_empty() {
            self.select_lumatone(0);
        }
    }

    fn select_lumatone_by_path(&mut self, path: &Path) -> bool {
        if let Some(idx) = self
            .lumatone_presets
            .iter()
            .position(|preset| same_path(&preset.path, path))
        {
            self.select_lumatone(idx);
            return true;
        }
        false
    }

    fn add_lumatone_preset_path(&mut self, path: PathBuf) {
        if !self
            .lumatone_presets
            .iter()
            .any(|preset| same_path(&preset.path, &path))
        {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("Unknown")
                .to_string();
            self.lumatone_presets.push(LumatonePreset { name, path });
            self.lumatone_presets.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }

    fn select_lumatone_index_by_current_path(&self) -> Option<usize> {
        let path = self.lumatone_path.as_ref()?;
        self.lumatone_presets
            .iter()
            .position(|preset| same_path(&preset.path, path))
    }

    fn select_connected_audio_output(&mut self) {
        if let Some(idx) = self
            .audio_outputs
            .iter()
            .position(|device| device.name == self.connected_audio_output)
        {
            self.selected_audio_output = idx;
        } else if let Some(idx) = self
            .audio_outputs
            .iter()
            .position(|device| device.is_default)
        {
            self.selected_audio_output = idx;
        } else if self.selected_audio_output >= self.audio_outputs.len() {
            self.selected_audio_output = 0;
        }
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn collect_audio_assets(kind: AudioAssetKind, root: &Path, assets: &mut Vec<AudioAssetItem>) {
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if file_name.starts_with('.') {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if kind == AudioAssetKind::Instrument && path != root {
                    assets.push(AudioAssetItem {
                        name: audio_asset_name(root, &path, true),
                        path: path.clone(),
                        kind,
                        is_dir: true,
                    });
                }
                pending.push(path);
            } else if file_type.is_file() && is_supported_audio_asset_file(kind, &path) {
                assets.push(AudioAssetItem {
                    name: audio_asset_name(root, &path, false),
                    path,
                    kind,
                    is_dir: false,
                });
            }
        }
    }
}

fn audio_asset_name(root: &Path, path: &Path, is_dir: bool) -> String {
    let mut name = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    if is_dir {
        name.push('/');
    }
    name
}

fn is_supported_audio_asset_file(kind: AudioAssetKind, path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    kind.extensions()
        .iter()
        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
}

fn unique_asset_path(dir: &Path, file_name: &str) -> PathBuf {
    let first = dir.join(file_name);
    if !first.exists() {
        return first;
    }

    let source = Path::new(file_name);
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let extension = source.extension().and_then(|value| value.to_str());
    for index in 2.. {
        let candidate_name = match extension {
            Some(extension) => format!("{stem}_{index}.{extension}"),
            None => format!("{stem}_{index}"),
        };
        let candidate = dir.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("unbounded asset filename search should always return")
}

fn add_lumatone_presets_from_dir(
    dir: &Path,
    presets: &mut Vec<LumatonePreset>,
    seen: &mut HashSet<PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("ltn") {
            continue;
        }
        if !seen.insert(path.clone()) {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Unknown")
            .to_string();
        presets.push(LumatonePreset { name, path });
    }
}
