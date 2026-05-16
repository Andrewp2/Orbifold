use cpal::traits::StreamTrait;
use midir::{Ignore, MidiInput, MidiInputConnection};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, TryRecvError},
};

use crate::audio::{AudioOutputDevice, AudioStreamInfo, build_audio_stream, list_audio_outputs};
use crate::midi::{
    MIDI_CHANNEL_FILTER_ALL, MidiSharedState, SharedLumatoneMap, SharedMidiCapture,
    SharedMidiChannelFilter, SharedMidiHeld, SharedMidiLast, SharedMidiLog, SharedMidiSustain,
    handle_midi, list_midi_inputs, load_lumatone_map,
};
use crate::project::{
    ClipNote, MusicProject, ProjectFile, ProjectSnapshot, QuantizeGrid, SharedMusicProject,
    active_key_set, playback_note_id,
};
use crate::scala::parse_scala;
use crate::scale::ScaleState;
use crate::settings::AppSettings;
use crate::synth::SynthHandle;

const MAX_PROJECT_HISTORY: usize = 64;
const MAX_RECENT_PROJECTS: usize = 8;
const METRONOME_NOTE_ID: u32 = 1_900_000;
const AUDITION_NOTE_ID: u32 = 1_800_000;
const AUDIO_ASSETS_DIR: &str = "audio_assets";
const DEFAULT_ADDED_NOTE_BEATS: f32 = 1.0;
const GRID_CELL_SNAP_EPSILON: f32 = 0.0001;
const UI_SCALE_MIN: f32 = 0.75;
const UI_SCALE_MAX: f32 = 2.0;
const PIANO_DEFAULT_VISIBLE_BEATS: f32 = 16.0;
const PIANO_MIN_VISIBLE_BEATS: f32 = 1.0;
const PIANO_WHEEL_ZOOM_BASE: f32 = 0.85;
const PIANO_DEFAULT_VISIBLE_PITCH_RADIUS: i32 = 12;
const PIANO_MIN_VISIBLE_PITCH_RADIUS: i32 = 2;
const PIANO_MAX_VISIBLE_PITCH_RADIUS: i32 = 64;
const PIANO_MIN_PITCH: i32 = -128;
const PIANO_MAX_PITCH: i32 = 256;

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

#[cfg_attr(test, allow(dead_code))]
#[derive(Clone, Debug)]
enum FileDialogRequest {
    OpenProject,
    SaveProject {
        file_name: String,
        cancel_status: String,
    },
    OpenScale,
    OpenKeymap,
    ImportAudioAsset {
        kind: AudioAssetKind,
    },
}

struct PendingFileDialog {
    request: FileDialogRequest,
    receiver: Receiver<Option<PathBuf>>,
    #[cfg(test)]
    completion_sender: std::sync::mpsc::Sender<Option<PathBuf>>,
}

impl FileDialogRequest {
    fn label(&self) -> &'static str {
        match self {
            Self::OpenProject => "project",
            Self::SaveProject { .. } => "project save",
            Self::OpenScale => "scale",
            Self::OpenKeymap => "key map",
            Self::ImportAudioAsset { kind } => kind.singular_label(),
        }
    }

    fn opening_status(&self) -> String {
        match self {
            Self::OpenProject => "Opening project dialog".to_string(),
            Self::SaveProject { .. } => "Opening save dialog".to_string(),
            Self::OpenScale => "Opening scale dialog".to_string(),
            Self::OpenKeymap => "Opening key map dialog".to_string(),
            Self::ImportAudioAsset { kind } => format!("Opening {} import dialog", kind.label()),
        }
    }

    fn cancel_status(&self) -> &str {
        match self {
            Self::OpenProject => "Open cancelled",
            Self::SaveProject { cancel_status, .. } => cancel_status,
            Self::OpenScale => "Scale open cancelled",
            Self::OpenKeymap => "Key map open cancelled",
            Self::ImportAudioAsset { .. } => "Asset import cancelled",
        }
    }

    #[cfg(not(test))]
    fn thread_name(&self) -> &'static str {
        match self {
            Self::OpenProject => "open-project",
            Self::SaveProject { .. } => "save-project",
            Self::OpenScale => "open-scale",
            Self::OpenKeymap => "open-keymap",
            Self::ImportAudioAsset { .. } => "import-asset",
        }
    }
}

pub(crate) struct AppState {
    pub(crate) scale_state: Arc<Mutex<ScaleState>>,
    pub(crate) synth: SynthHandle,
    pub(crate) midi_last: SharedMidiLast,
    pub(crate) midi_log: SharedMidiLog,
    pub(crate) midi_capture: SharedMidiCapture,
    pub(crate) midi_held: SharedMidiHeld,
    pub(crate) midi_sustain: SharedMidiSustain,
    pub(crate) midi_channel_filter: SharedMidiChannelFilter,
    pub(crate) music_project: SharedMusicProject,
    pub(crate) midi_connection: Option<MidiInputConnection<()>>,
    pub(crate) midi_inputs: Vec<String>,
    pub(crate) selected_input: usize,
    pub(crate) connected_midi_input: String,
    pub(crate) audio_stream: Option<cpal::Stream>,
    pub(crate) audio_stream_info: Option<AudioStreamInfo>,
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
    pub(crate) show_asset_browser: bool,
    pub(crate) show_scale_browser: bool,
    piano_view_start_beats: f32,
    piano_view_visible_beats: f32,
    piano_view_center_pitch: i32,
    piano_view_pitch_radius: i32,
    pub(crate) midi_debug: Arc<AtomicBool>,
    pub(crate) lumatone_map: SharedLumatoneMap,
    pub(crate) lumatone_path: Option<PathBuf>,
    pub(crate) lumatone_presets: Vec<LumatonePreset>,
    pub(crate) selected_lumatone: usize,
    pub(crate) project_path: Option<PathBuf>,
    pub(crate) project_dirty: bool,
    clean_project_file: Option<ProjectFile>,
    last_autosave_project_file: Option<ProjectFile>,
    autosave_path: PathBuf,
    pub(crate) autosave_available: bool,
    pub(crate) playback_active_keys: HashSet<u32>,
    pub(crate) selected_clip_note: Option<u64>,
    copied_clip_note: Option<ClipNote>,
    last_snap_grid: QuantizeGrid,
    playback_active_notes: HashMap<u64, u32>,
    undo_stack: Vec<ProjectEditSnapshot>,
    redo_stack: Vec<ProjectEditSnapshot>,
    new_project_confirm_pending: bool,
    open_project_confirm_pending: bool,
    quit_confirm_pending: bool,
    last_metronome_beat: Option<u32>,
    settings: AppSettings,
    settings_path: PathBuf,
    persist_enabled: bool,
    pending_file_dialog: Option<PendingFileDialog>,
}

#[derive(Clone, Debug, PartialEq)]
struct ProjectEditSnapshot {
    project: ProjectSnapshot,
    selected_clip_note: Option<u64>,
}

impl AppState {
    #[cfg(test)]
    pub(crate) fn for_layout_tests() -> Self {
        let settings = AppSettings::default();
        let settings_path = PathBuf::from("orbifold_layout_test_settings.txt");
        let mut app = Self {
            scale_state: Arc::new(Mutex::new(ScaleState::default())),
            synth: SynthHandle::new(32),
            midi_last: Arc::new(Mutex::new(None)),
            midi_log: Arc::new(Mutex::new(Vec::new())),
            midi_capture: Arc::new(Mutex::new(Default::default())),
            midi_held: Arc::new(Mutex::new(HashMap::new())),
            midi_sustain: Arc::new(Mutex::new(Default::default())),
            midi_channel_filter: Arc::new(std::sync::atomic::AtomicI8::new(
                MIDI_CHANNEL_FILTER_ALL,
            )),
            music_project: Arc::new(Mutex::new(Default::default())),
            midi_connection: None,
            midi_inputs: Vec::new(),
            selected_input: 0,
            connected_midi_input: String::new(),
            audio_stream: None,
            audio_stream_info: None,
            audio_outputs: Vec::new(),
            selected_audio_output: 0,
            connected_audio_output: "default".to_string(),
            last_status: "Ready".to_string(),
            scala_path: None,
            scale_library: Vec::new(),
            selected_scale_library: 0,
            audio_assets: Vec::new(),
            selected_audio_asset: None,
            selected_audio_asset_kind: AudioAssetKind::Sample,
            show_asset_browser: true,
            show_scale_browser: false,
            piano_view_start_beats: 0.0,
            piano_view_visible_beats: PIANO_DEFAULT_VISIBLE_BEATS,
            piano_view_center_pitch: settings.root_midi,
            piano_view_pitch_radius: PIANO_DEFAULT_VISIBLE_PITCH_RADIUS,
            midi_debug: Arc::new(AtomicBool::new(false)),
            lumatone_map: Arc::new(Mutex::new(None)),
            lumatone_path: None,
            lumatone_presets: Vec::new(),
            selected_lumatone: 0,
            project_path: None,
            project_dirty: false,
            clean_project_file: None,
            last_autosave_project_file: None,
            autosave_path: autosave_path_for_settings_path(&settings_path),
            autosave_available: false,
            playback_active_keys: HashSet::new(),
            selected_clip_note: None,
            copied_clip_note: None,
            last_snap_grid: QuantizeGrid::Sixteenth,
            playback_active_notes: HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            new_project_confirm_pending: false,
            open_project_confirm_pending: false,
            quit_confirm_pending: false,
            last_metronome_beat: None,
            settings,
            settings_path,
            persist_enabled: false,
            pending_file_dialog: None,
        };
        app.establish_clean_project_snapshot();
        app
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        scale_state: Arc<Mutex<ScaleState>>,
        synth: SynthHandle,
        midi_last: SharedMidiLast,
        midi_log: SharedMidiLog,
        midi_capture: SharedMidiCapture,
        midi_held: SharedMidiHeld,
        midi_sustain: SharedMidiSustain,
        midi_channel_filter: SharedMidiChannelFilter,
        music_project: SharedMusicProject,
        audio_stream: Option<cpal::Stream>,
        audio_stream_info: Option<AudioStreamInfo>,
        connected_audio_output: String,
        settings: AppSettings,
        settings_status: Option<String>,
        probe_audio_outputs: bool,
        probe_midi_inputs: bool,
    ) -> Self {
        let startup_status = settings_status.clone();
        let settings_path = AppSettings::default_path();
        let autosave_path = autosave_path_for_settings_path(&settings_path);
        let autosave_available = autosave_path.exists();
        midi_channel_filter.store(
            settings
                .midi_channel_filter
                .map(|channel| channel as i8)
                .unwrap_or(MIDI_CHANNEL_FILTER_ALL),
            Ordering::Relaxed,
        );
        let mut app = Self {
            scale_state,
            synth,
            midi_last,
            midi_log,
            midi_capture,
            midi_held,
            midi_sustain,
            midi_channel_filter,
            music_project,
            midi_connection: None,
            midi_inputs: Vec::new(),
            selected_input: 0,
            connected_midi_input: String::new(),
            audio_stream,
            audio_stream_info,
            audio_outputs: if probe_audio_outputs {
                list_audio_outputs()
            } else {
                Vec::new()
            },
            selected_audio_output: 0,
            connected_audio_output,
            last_status: settings_status.unwrap_or_else(|| "Ready".to_string()),
            scala_path: None,
            scale_library: Vec::new(),
            selected_scale_library: 0,
            audio_assets: Vec::new(),
            selected_audio_asset: None,
            selected_audio_asset_kind: AudioAssetKind::Sample,
            show_asset_browser: true,
            show_scale_browser: false,
            piano_view_start_beats: 0.0,
            piano_view_visible_beats: PIANO_DEFAULT_VISIBLE_BEATS,
            piano_view_center_pitch: settings.root_midi,
            piano_view_pitch_radius: PIANO_DEFAULT_VISIBLE_PITCH_RADIUS,
            midi_debug: Arc::new(AtomicBool::new(settings.midi_debug)),
            lumatone_map: Arc::new(Mutex::new(None)),
            lumatone_path: None,
            lumatone_presets: Vec::new(),
            selected_lumatone: 0,
            project_path: None,
            project_dirty: false,
            clean_project_file: None,
            last_autosave_project_file: None,
            autosave_path,
            autosave_available,
            playback_active_keys: HashSet::new(),
            selected_clip_note: None,
            copied_clip_note: None,
            last_snap_grid: QuantizeGrid::Sixteenth,
            playback_active_notes: HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            new_project_confirm_pending: false,
            open_project_confirm_pending: false,
            quit_confirm_pending: false,
            last_metronome_beat: None,
            settings,
            settings_path,
            persist_enabled: false,
            pending_file_dialog: None,
        };

        app.select_connected_audio_output();
        app.load_lumatone_presets(Path::new("lumatone_factory_presets"));
        app.select_saved_or_default_lumatone();
        app.refresh_scale_library();
        app.refresh_audio_assets();
        if let Some(path) = app.settings.scala_path.clone()
            && let Err(err) = app.load_scale_path(path, true)
        {
            app.set_error_status(format!("Saved Scala load error: {err}"));
        }
        if probe_midi_inputs {
            let preferred_midi = app.settings.midi_input_name.clone();
            app.refresh_midi_inputs(preferred_midi.as_deref());
            app.open_midi_input();
        }
        app.restore_startup_status(startup_status);
        app.persist_enabled = true;
        if probe_audio_outputs && probe_midi_inputs {
            app.persist_settings(None);
        }
        app.establish_clean_project_snapshot();
        if app.autosave_available {
            app.append_status("Autosave available: click Recover or Dismiss");
        }
        app
    }

    fn append_status(&mut self, message: &str) {
        if self.last_status == "Ready" {
            self.last_status = message.to_string();
        } else if !self.last_status.contains(message) {
            self.last_status = format!("{}; {message}", self.last_status);
        }
    }

    fn set_error_status(&mut self, message: impl Into<String>) {
        let message = message.into();
        log::error!("{message}");
        self.last_status = message;
    }

    fn restore_startup_status(&mut self, startup_status: Option<String>) {
        let Some(startup_status) = startup_status else {
            return;
        };
        if self.last_status == "Ready" {
            self.last_status = startup_status;
            return;
        }
        if self.last_status != startup_status && !self.last_status.contains(&startup_status) {
            self.last_status = format!("{startup_status}; {}", self.last_status);
        }
    }

    pub(crate) fn has_pending_file_dialog(&self) -> bool {
        self.pending_file_dialog.is_some()
    }

    pub(crate) fn poll_pending_file_dialog(&mut self) {
        let Some(pending) = self.pending_file_dialog.as_ref() else {
            return;
        };
        match pending.receiver.try_recv() {
            Ok(selection) => {
                let pending = self
                    .pending_file_dialog
                    .take()
                    .expect("pending dialog should exist");
                self.finish_file_dialog(pending.request, selection);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                let pending = self
                    .pending_file_dialog
                    .take()
                    .expect("pending dialog should exist");
                self.set_error_status(format!(
                    "{} dialog failed before returning a result",
                    pending.request.label()
                ));
            }
        }
    }

    pub(crate) fn request_open_project_dialog(&mut self) {
        if !self.request_open_project() {
            return;
        }
        self.begin_file_dialog(FileDialogRequest::OpenProject);
    }

    pub(crate) fn request_save_project_dialog(
        &mut self,
        file_name: impl Into<String>,
        cancel_status: impl Into<String>,
    ) {
        self.begin_file_dialog(FileDialogRequest::SaveProject {
            file_name: file_name.into(),
            cancel_status: cancel_status.into(),
        });
    }

    pub(crate) fn request_open_scale_dialog(&mut self) {
        self.begin_file_dialog(FileDialogRequest::OpenScale);
    }

    pub(crate) fn request_open_keymap_dialog(&mut self) {
        self.begin_file_dialog(FileDialogRequest::OpenKeymap);
    }

    pub(crate) fn request_import_audio_asset_dialog(&mut self, kind: AudioAssetKind) {
        self.begin_file_dialog(FileDialogRequest::ImportAudioAsset { kind });
    }

    #[cfg(test)]
    pub(crate) fn pending_file_dialog_label_for_tests(&self) -> Option<&'static str> {
        self.pending_file_dialog
            .as_ref()
            .map(|pending| pending.request.label())
    }

    #[cfg(test)]
    pub(crate) fn complete_pending_file_dialog_for_tests(&mut self, selection: Option<PathBuf>) {
        let Some(pending) = self.pending_file_dialog.as_ref() else {
            return;
        };
        let _ = pending.completion_sender.send(selection);
    }

    fn begin_file_dialog(&mut self, request: FileDialogRequest) {
        if self.pending_file_dialog.is_some() {
            self.last_status = "A file dialog is already open".to_string();
            return;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        #[cfg(not(test))]
        {
            let request_for_thread = request.clone();
            let thread_name = format!("orbifold-{}", request_for_thread.thread_name());
            if let Err(err) = std::thread::Builder::new()
                .name(thread_name)
                .spawn(move || {
                    let selection = run_file_dialog(request_for_thread);
                    let _ = sender.send(selection);
                })
            {
                self.set_error_status(format!("Failed to open file dialog: {err}"));
                return;
            }
        }
        self.last_status = request.opening_status();
        self.pending_file_dialog = Some(PendingFileDialog {
            request,
            receiver,
            #[cfg(test)]
            completion_sender: sender,
        });
    }

    fn finish_file_dialog(&mut self, request: FileDialogRequest, selection: Option<PathBuf>) {
        let Some(path) = selection else {
            self.last_status = request.cancel_status().to_string();
            return;
        };
        match request {
            FileDialogRequest::OpenProject => self.load_project_path(path),
            FileDialogRequest::SaveProject { .. } => self.save_project_to_path(path),
            FileDialogRequest::OpenScale => {
                if let Err(err) = self.load_scale_path(path, true) {
                    self.set_error_status(format!("Scala parse error: {err}"));
                }
            }
            FileDialogRequest::OpenKeymap => {
                if self.load_lumatone_path(path) {
                    self.mark_project_dirty();
                }
            }
            FileDialogRequest::ImportAudioAsset { kind } => {
                self.import_audio_asset_path(path, kind)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn set_autosave_path_for_tests(&mut self, path: PathBuf) {
        self.last_autosave_project_file = None;
        self.autosave_path = path;
        self.autosave_available = self.autosave_path.exists();
    }

    #[cfg(test)]
    pub(crate) fn set_settings_path_for_tests(&mut self, path: PathBuf, persist_enabled: bool) {
        self.settings_path = path;
        self.autosave_path = autosave_path_for_settings_path(&self.settings_path);
        self.autosave_available = self.autosave_path.exists();
        self.persist_enabled = persist_enabled;
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
            Ok((stream, connected_name, sender, info)) => {
                if let Err(err) = stream.play() {
                    self.set_error_status(format!("Audio playback failed: {err}"));
                    return;
                }
                self.synth.install_sender(sender);
                self.audio_stream = Some(stream);
                self.audio_stream_info = Some(info);
                self.connected_audio_output = connected_name.clone();
                self.last_status = format!("Connected audio output: {connected_name}");
                self.persist_settings(None);
            }
            Err(err) => {
                self.audio_stream_info = None;
                self.set_error_status(format!("Audio output error: {err}"));
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
            held: self.midi_held.clone(),
            sustain: self.midi_sustain.clone(),
            channel_filter: self.midi_channel_filter.clone(),
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
            self.connected_midi_input = port_name.clone();
            self.last_status = format!("Connected MIDI input: {port_name}");
        } else {
            self.connected_midi_input.clear();
            self.set_error_status(format!("Failed to connect MIDI input: {port_name}"));
        }
        self.midi_connection = conn;
        self.persist_settings(None);
    }

    pub(crate) fn all_notes_off(&mut self) {
        self.stop_playback_notes();
        self.midi_held.lock().clear();
        self.midi_sustain.lock().clear();
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
        self.mark_project_dirty();
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

    pub(crate) fn return_transport_to_start(&mut self) {
        self.seek_transport_to(0.0);
        self.last_status = "Returned to start".to_string();
    }

    pub(crate) fn seek_transport_to(&mut self, beat: f32) {
        self.music_project
            .lock()
            .seek(beat, std::time::Instant::now());
        self.stop_playback_notes();
        self.last_metronome_beat = None;
        self.last_status = format!("Seek {:.2}", beat.max(0.0));
    }

    pub(crate) fn toggle_metronome(&mut self) {
        let enabled = {
            let mut project = self.music_project.lock();
            project.transport.metronome_enabled = !project.transport.metronome_enabled;
            project.transport.metronome_enabled
        };
        self.mark_project_dirty();
        self.last_status = if enabled {
            "Metronome on".to_string()
        } else {
            "Metronome off".to_string()
        };
    }

    pub(crate) fn toggle_quantize_on_record(&mut self) {
        let enabled = {
            let mut project = self.music_project.lock();
            project.transport.quantize_on_record = !project.transport.quantize_on_record;
            project.transport.quantize_on_record
        };
        self.mark_project_dirty();
        self.last_status = if enabled {
            "Record quantize on".to_string()
        } else {
            "Record quantize off".to_string()
        };
    }

    pub(crate) fn toggle_snap_to_grid(&mut self) {
        let grid = {
            let project = self.music_project.lock();
            project.transport.quantize_grid
        };
        if grid == QuantizeGrid::Off {
            self.set_quantize_grid(self.last_snap_grid);
            self.last_status = format!("Snap on {}", self.last_snap_grid.as_str());
        } else {
            self.last_snap_grid = grid;
            self.set_quantize_grid(QuantizeGrid::Off);
            self.last_status = "Snap off".to_string();
        }
    }

    pub(crate) fn set_quantize_grid(&mut self, grid: QuantizeGrid) {
        let previous = {
            let mut project = self.music_project.lock();
            let previous = project.transport.quantize_grid;
            project.transport.quantize_grid = grid;
            previous
        };
        if grid != QuantizeGrid::Off {
            self.last_snap_grid = grid;
        }
        if previous != grid {
            self.mark_project_dirty();
            self.last_status = format!("Grid {}", grid.as_str());
        } else {
            self.last_status = format!("Grid {} unchanged", grid.as_str());
        }
    }

    pub(crate) fn clear_clip(&mut self) {
        self.stop_playback_notes();
        self.push_project_history();
        if self.music_project.lock().clear_clip() {
            self.selected_clip_note = None;
            self.mark_project_dirty();
            self.last_status = "Clip cleared".to_string();
        } else {
            self.last_status = "Clip already empty".to_string();
        }
    }

    pub(crate) fn quantize_clip(&mut self) {
        self.push_project_history();
        if self.music_project.lock().quantize_clip() {
            self.mark_project_dirty();
            self.last_status = "Clip quantized".to_string();
        } else {
            self.last_status = "Clip quantize unchanged".to_string();
        }
    }

    pub(crate) fn select_clip_note(&mut self, note_id: Option<u64>) {
        let Some(note_id) = note_id else {
            self.selected_clip_note = None;
            return;
        };
        let Some(note) = self.music_project.lock().note_by_id(note_id) else {
            self.selected_clip_note = None;
            self.last_status = "Selected clip note no longer exists".to_string();
            return;
        };
        self.selected_clip_note = Some(note_id);
        self.last_status = self.clip_note_status("Selected note", &note);
        self.audition_clip_note(&note);
    }

    pub(crate) fn selected_clip_note(&self) -> Option<ClipNote> {
        self.selected_clip_note
            .and_then(|id| self.music_project.lock().note_by_id(id))
    }

    pub(crate) fn select_current_clip(&mut self) {
        self.selected_clip_note = None;
        let note_count = self.music_project.lock().clip.notes.len();
        self.last_status = match note_count {
            0 => "Current clip empty".to_string(),
            1 => "Selected current clip: 1 note".to_string(),
            count => format!("Selected current clip: {count} notes"),
        };
    }

    pub(crate) fn clear_clip_note_selection(&mut self) -> bool {
        if self.selected_clip_note.take().is_none() {
            return false;
        }
        self.last_status = "Note selection cleared".to_string();
        true
    }

    fn selected_existing_clip_note_id(&mut self) -> Option<u64> {
        let Some(note_id) = self.selected_clip_note else {
            self.last_status = "No clip note selected".to_string();
            return None;
        };
        if self.music_project.lock().note_by_id(note_id).is_none() {
            self.selected_clip_note = None;
            self.last_status = "Selected clip note no longer exists".to_string();
            return None;
        }
        Some(note_id)
    }

    fn selected_existing_clip_note(&mut self) -> Option<ClipNote> {
        let note_id = self.selected_existing_clip_note_id()?;
        self.music_project.lock().note_by_id(note_id)
    }

    pub(crate) fn copy_selected_clip_note(&mut self) {
        let Some(note) = self.selected_existing_clip_note() else {
            return;
        };
        self.copied_clip_note = Some(note.clone());
        self.last_status = self.clip_note_status("Copied note", &note);
    }

    pub(crate) fn can_paste_clip_note(&self) -> bool {
        self.copied_clip_note.is_some()
    }

    pub(crate) fn paste_copied_clip_note_at_playhead(&mut self) {
        let Some(template) = self.copied_clip_note.clone() else {
            self.last_status = "No copied clip note".to_string();
            return;
        };
        let Some(info) = self.scale_state.lock().note_info(template.musical_note) else {
            self.last_status = "Copied pitch cannot be tuned".to_string();
            return;
        };
        let start = {
            let project = self.music_project.lock();
            let beat = project.current_position_beats(std::time::Instant::now());
            let loop_beats = project.transport.loop_beats.max(1.0);
            project
                .transport
                .quantize_grid
                .step_beats()
                .map(|step| ((beat / step).round() * step).rem_euclid(loop_beats))
                .unwrap_or_else(|| beat.rem_euclid(loop_beats))
        };
        self.push_project_history();
        let note_id = self.music_project.lock().add_note(
            start,
            template.duration_beats,
            template.musical_note,
            template.velocity,
            info.freq,
        );
        self.selected_clip_note = Some(note_id);
        self.mark_project_dirty();
        if let Some(note) = self.selected_clip_note() {
            self.last_status = self.clip_note_status("Pasted note", &note);
        }
    }

    pub(crate) fn delete_selected_clip_note(&mut self) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        self.push_project_history();
        if self.music_project.lock().delete_note(note_id) {
            self.selected_clip_note = None;
            self.mark_project_dirty();
            self.last_status = "Deleted clip note".to_string();
        } else {
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn duplicate_selected_clip_note(&mut self) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        self.push_project_history();
        let duplicated = { self.music_project.lock().duplicate_note(note_id) };
        match duplicated {
            Some(new_id) => {
                self.selected_clip_note = Some(new_id);
                self.mark_project_dirty();
                self.last_status = "Duplicated clip note".to_string();
            }
            None => self.last_status = "Selected clip note no longer exists".to_string(),
        }
    }

    pub(crate) fn nudge_selected_clip_note(&mut self, direction: f32) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        let step = self.music_project.lock().edit_step_beats() * direction;
        self.push_project_history();
        if self.music_project.lock().nudge_note(note_id, step) {
            self.mark_project_dirty();
            self.last_status = "Moved clip note".to_string();
        } else {
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn resize_selected_clip_note(&mut self, direction: f32) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        let step = self.music_project.lock().edit_step_beats() * direction;
        self.push_project_history();
        if self.music_project.lock().resize_note(note_id, step) {
            self.mark_project_dirty();
            self.last_status = "Resized clip note".to_string();
        } else {
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn set_selected_clip_note_velocity(&mut self, velocity: u8) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        self.push_project_history();
        if self
            .music_project
            .lock()
            .set_note_velocity(note_id, velocity)
        {
            self.mark_project_dirty();
            if let Some(note) = self.selected_clip_note() {
                self.last_status = self.clip_note_status("Changed velocity", &note);
            }
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
            self.mark_project_dirty();
            self.last_status = "Moved clip note pitch".to_string();
            self.audition_selected_clip_note();
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
                .map(|step| snap_beat_to_grid_cell(beat, step, loop_beats))
                .unwrap_or_else(|| beat.rem_euclid(loop_beats));
            (start, DEFAULT_ADDED_NOTE_BEATS.min(loop_beats))
        };
        self.push_project_history();
        let note_id =
            self.music_project
                .lock()
                .add_note(start, duration, musical_note, 96, info.freq);
        self.selected_clip_note = Some(note_id);
        self.mark_project_dirty();
        if let Some(note) = self.selected_clip_note() {
            self.last_status = self.clip_note_status("Added note", &note);
        }
        self.audition_selected_clip_note();
    }

    pub(crate) fn quantize_selected_or_clip(&mut self) {
        if self.selected_clip_note.is_some() {
            self.quantize_selected_clip_note();
        } else {
            self.quantize_clip();
        }
    }

    pub(crate) fn quantize_selected_clip_note(&mut self) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        self.push_project_history();
        if self.music_project.lock().quantize_note(note_id) {
            self.mark_project_dirty();
            if let Some(note) = self.selected_clip_note() {
                self.last_status = self.clip_note_status("Quantized note", &note);
            }
        } else {
            self.last_status = "Note quantize unchanged".to_string();
        }
    }

    pub(crate) fn drag_clip_note_to(
        &mut self,
        note_id: u64,
        start_beats: f32,
        musical_note: i32,
        push_history: bool,
    ) -> bool {
        let Some(info) = self.scale_state.lock().note_info(musical_note) else {
            self.last_status = "Dragged pitch cannot be tuned".to_string();
            return false;
        };
        if push_history {
            self.push_project_history();
        }
        let start_beats = {
            let project = self.music_project.lock();
            let loop_beats = project.transport.loop_beats.max(1.0);
            project
                .transport
                .quantize_grid
                .step_beats()
                .map(|step| ((start_beats / step).round() * step).rem_euclid(loop_beats))
                .unwrap_or_else(|| start_beats.rem_euclid(loop_beats))
        };
        let changed = self.music_project.lock().set_note_start_and_pitch(
            note_id,
            start_beats,
            musical_note,
            info.freq,
        );
        if changed {
            self.selected_clip_note = Some(note_id);
            self.mark_project_dirty();
            self.last_status = "Moved clip note".to_string();
        }
        changed
    }

    pub(crate) fn resize_clip_note_start_to(
        &mut self,
        note_id: u64,
        start_beats: f32,
        push_history: bool,
    ) -> bool {
        if push_history {
            self.push_project_history();
        }
        let changed = self
            .music_project
            .lock()
            .set_note_start_preserving_end(note_id, start_beats);
        if changed {
            self.selected_clip_note = Some(note_id);
            self.mark_project_dirty();
            self.last_status = "Resized clip note".to_string();
        }
        changed
    }

    pub(crate) fn resize_clip_note_end_to(
        &mut self,
        note_id: u64,
        end_beats: f32,
        push_history: bool,
    ) -> bool {
        let Some(note) = self.music_project.lock().note_by_id(note_id) else {
            self.last_status = "Selected clip note no longer exists".to_string();
            return false;
        };
        let loop_beats = self.music_project.lock().transport.loop_beats.max(1.0);
        let duration = (end_beats - note.start_beats).rem_euclid(loop_beats);
        if push_history {
            self.push_project_history();
        }
        let changed = self
            .music_project
            .lock()
            .set_note_duration(note_id, duration);
        if changed {
            self.selected_clip_note = Some(note_id);
            self.mark_project_dirty();
            self.last_status = "Resized clip note".to_string();
        }
        changed
    }

    pub(crate) fn set_clip_note_velocity(
        &mut self,
        note_id: u64,
        velocity: u8,
        push_history: bool,
    ) -> bool {
        if push_history {
            self.push_project_history();
        }
        let changed = self
            .music_project
            .lock()
            .set_note_velocity(note_id, velocity);
        if changed {
            self.selected_clip_note = Some(note_id);
            self.mark_project_dirty();
            self.last_status = "Updated clip note velocity".to_string();
        }
        changed
    }

    pub(crate) fn ui_scale(&self) -> f32 {
        self.settings.ui_scale
    }

    pub(crate) fn adjust_ui_scale(&mut self, delta: f32) {
        let previous = self.settings.ui_scale;
        self.settings.ui_scale = (self.settings.ui_scale + delta).clamp(UI_SCALE_MIN, UI_SCALE_MAX);
        if (self.settings.ui_scale - previous).abs() <= f32::EPSILON {
            self.last_status = format!("Zoom {:.0}% unchanged", self.settings.ui_scale * 100.0);
            return;
        }
        self.last_status = format!("Zoom {:.0}%", self.settings.ui_scale * 100.0);
        self.persist_settings(None);
    }

    pub(crate) fn reset_ui_scale(&mut self) {
        self.settings.ui_scale = 1.0;
        self.last_status = "Zoom 100%".to_string();
        self.persist_settings(None);
    }

    pub(crate) fn toggle_asset_browser(&mut self) {
        self.show_asset_browser = !self.show_asset_browser;
        self.last_status = if self.show_asset_browser {
            "Asset browser shown".to_string()
        } else {
            "Asset browser hidden".to_string()
        };
    }

    pub(crate) fn toggle_scale_browser(&mut self) {
        self.show_scale_browser = !self.show_scale_browser;
        self.last_status = if self.show_scale_browser {
            "Scale browser shown".to_string()
        } else {
            "Scale browser hidden".to_string()
        };
    }

    pub(crate) fn toggle_audio_mute(&mut self) {
        let muted = !self.synth.muted();
        match self.synth.set_muted(muted) {
            Ok(()) => {
                self.last_status = if muted {
                    "Audio muted".to_string()
                } else {
                    "Audio unmuted".to_string()
                };
            }
            Err(err) => self.set_error_status(format!("Audio mute error: {err}")),
        }
    }

    pub(crate) fn midi_channel_filter(&self) -> Option<u8> {
        let value = self.midi_channel_filter.load(Ordering::Relaxed);
        (value != MIDI_CHANNEL_FILTER_ALL).then_some(value as u8)
    }

    pub(crate) fn midi_channel_filter_label(&self) -> String {
        self.midi_channel_filter()
            .map(|channel| format!("Ch {}", channel + 1))
            .unwrap_or_else(|| "All".to_string())
    }

    pub(crate) fn cycle_midi_channel_filter(&mut self) {
        let next = match self.midi_channel_filter() {
            None => 0,
            Some(15) => MIDI_CHANNEL_FILTER_ALL,
            Some(channel) => channel as i8 + 1,
        };
        self.midi_channel_filter.store(next, Ordering::Relaxed);
        self.settings.midi_channel_filter = (next != MIDI_CHANNEL_FILTER_ALL).then_some(next as u8);
        self.last_status = format!("MIDI filter {}", self.midi_channel_filter_label());
        self.persist_settings(None);
    }

    pub(crate) fn piano_view_start_beats(&self, loop_beats: f32) -> f32 {
        clamp_piano_view_start(
            self.piano_view_start_beats,
            loop_beats.max(1.0),
            self.piano_view_visible_beats(loop_beats),
        )
    }

    pub(crate) fn piano_view_visible_beats(&self, loop_beats: f32) -> f32 {
        self.piano_view_visible_beats.clamp(
            PIANO_MIN_VISIBLE_BEATS,
            loop_beats.max(PIANO_MIN_VISIBLE_BEATS),
        )
    }

    pub(crate) fn piano_pitch_range(&self) -> (i32, i32) {
        let radius = self.piano_view_pitch_radius.clamp(
            PIANO_MIN_VISIBLE_PITCH_RADIUS,
            PIANO_MAX_VISIBLE_PITCH_RADIUS,
        );
        (
            (self.piano_view_center_pitch - radius).clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH),
            (self.piano_view_center_pitch + radius).clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH),
        )
    }

    pub(crate) fn scroll_piano_roll(&mut self, delta_beats: f32, delta_pitches: i32) -> bool {
        let loop_beats = self.music_project.lock().transport.loop_beats.max(1.0);
        let visible = self.piano_view_visible_beats(loop_beats);
        let start = clamp_piano_view_start(
            self.piano_view_start_beats + delta_beats,
            loop_beats,
            visible,
        );
        let center =
            (self.piano_view_center_pitch + delta_pitches).clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH);
        let changed = (start - self.piano_view_start_beats).abs() > f32::EPSILON
            || center != self.piano_view_center_pitch;
        self.piano_view_start_beats = start;
        self.piano_view_center_pitch = center;
        if changed {
            self.last_status = format!(
                "Piano scroll beat {:.2} pitch {}",
                self.piano_view_start_beats, self.piano_view_center_pitch
            );
        }
        changed
    }

    pub(crate) fn zoom_piano_roll(&mut self, delta: f32, anchor_beat: f32) -> bool {
        let loop_beats = self.music_project.lock().transport.loop_beats.max(1.0);
        let old_visible = self.piano_view_visible_beats(loop_beats);
        let factor = PIANO_WHEEL_ZOOM_BASE.powf(delta.signum());
        let new_visible = (old_visible * factor).clamp(PIANO_MIN_VISIBLE_BEATS, loop_beats);
        if (new_visible - old_visible).abs() <= f32::EPSILON {
            return false;
        }
        let old_start = self.piano_view_start_beats(loop_beats);
        let anchor_fraction = ((anchor_beat - old_start) / old_visible).clamp(0.0, 1.0);
        self.piano_view_visible_beats = new_visible;
        self.piano_view_start_beats = clamp_piano_view_start(
            anchor_beat - new_visible * anchor_fraction,
            loop_beats,
            new_visible,
        );
        self.last_status = format!("Piano zoom {new_visible:.2} beats");
        true
    }

    pub(crate) fn zoom_piano_roll_pitches(&mut self, delta: f32, anchor_pitch: i32) -> bool {
        let old_radius = self.piano_view_pitch_radius;
        let step = if delta > 0.0 {
            -1
        } else if delta < 0.0 {
            1
        } else {
            0
        };
        let new_radius = (old_radius + step).clamp(
            PIANO_MIN_VISIBLE_PITCH_RADIUS,
            PIANO_MAX_VISIBLE_PITCH_RADIUS,
        );
        if new_radius == old_radius {
            return false;
        }
        let old_center = self.piano_view_center_pitch;
        self.piano_view_pitch_radius = new_radius;
        self.piano_view_center_pitch = (anchor_pitch
            + ((old_center - anchor_pitch) as f32 * new_radius as f32 / old_radius as f32).round()
                as i32)
            .clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH);
        self.last_status = format!("Piano pitch zoom {} rows", new_radius * 2 + 1);
        true
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
        let current = self.project_edit_snapshot();
        self.redo_stack.push(current);
        self.apply_project_history_snapshot(snapshot);
        self.refresh_project_dirty_state();
        self.last_status = "Undid clip edit".to_string();
    }

    pub(crate) fn redo_project_edit(&mut self) {
        let Some(snapshot) = self.redo_stack.pop() else {
            self.last_status = "Nothing to redo".to_string();
            return;
        };
        let current = self.project_edit_snapshot();
        self.undo_stack.push(current);
        self.apply_project_history_snapshot(snapshot);
        self.refresh_project_dirty_state();
        self.last_status = "Redid clip edit".to_string();
    }

    pub(crate) fn update_music_playback(&mut self) {
        let now = std::time::Instant::now();
        let (desired_notes, current_beat, metronome_enabled) = {
            let project = self.music_project.lock();
            if !project.transport.playing {
                (Vec::new(), None, false)
            } else {
                let beat = project.current_position_beats(now);
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
        let temp_path = temporary_project_save_path(&path);
        let backup_path = project_backup_path(&path);
        let result = std::fs::write(&temp_path, project_file.to_text()).and_then(|()| {
            if path.exists() {
                let _ = std::fs::remove_file(&backup_path);
                std::fs::rename(&path, &backup_path)?;
            }
            std::fs::rename(&temp_path, &path)
        });
        match result {
            Ok(()) => {
                self.project_path = Some(path.clone());
                self.clean_project_file = Some(project_file);
                self.project_dirty = false;
                self.add_recent_project_path(path.clone());
                self.clear_autosave_file();
                self.last_status = format!("Saved project: {}", path.display());
                self.persist_settings(None);
            }
            Err(err) => {
                let _ = std::fs::remove_file(&temp_path);
                self.set_error_status(format!("Project save error ({}): {err}", path.display()));
            }
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
                self.set_error_status(format!("Project open error ({}): {err}", path.display()));
                return;
            }
        };
        let project = match ProjectFile::from_text(&data) {
            Ok(project) => project,
            Err(err) => {
                self.set_error_status(format!("Project parse error ({}): {err}", path.display()));
                return;
            }
        };

        if let Err(err) = self.apply_project_file(project.clone()) {
            self.set_error_status(format!("Project load error ({}): {err}", path.display()));
            return;
        }
        self.project_path = Some(path.clone());
        self.clean_project_file = Some(project);
        self.project_dirty = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.add_recent_project_path(path.clone());
        self.last_status = format!("Loaded project: {}", path.display());
        self.persist_settings(None);
    }

    pub(crate) fn start_new_project(&mut self) {
        if self.project_dirty && !self.new_project_confirm_pending {
            self.new_project_confirm_pending = true;
            self.open_project_confirm_pending = false;
            self.last_status = "Unsaved changes: click New again to discard".to_string();
            return;
        }
        let discarded_dirty_project = self.project_dirty;
        self.stop_playback_notes();
        self.music_project
            .lock()
            .apply_snapshot(MusicProject::default().snapshot());
        self.selected_clip_note = None;
        self.project_path = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.establish_clean_project_snapshot();
        self.new_project_confirm_pending = false;
        self.open_project_confirm_pending = false;
        self.quit_confirm_pending = false;
        self.last_status = if discarded_dirty_project {
            "Discarded changes and started new project".to_string()
        } else {
            "New project".to_string()
        };
    }

    pub(crate) fn request_open_project(&mut self) -> bool {
        if self.project_dirty && !self.open_project_confirm_pending {
            self.open_project_confirm_pending = true;
            self.new_project_confirm_pending = false;
            self.last_status = "Unsaved changes: click Open again to discard".to_string();
            return false;
        }
        self.open_project_confirm_pending = false;
        true
    }

    pub(crate) fn request_quit(&mut self) -> bool {
        if self.project_dirty && !self.quit_confirm_pending {
            self.quit_confirm_pending = true;
            self.last_status = "Unsaved changes: close again to quit".to_string();
            return false;
        }
        true
    }

    pub(crate) fn cancel_discard_confirmation(&mut self) -> bool {
        if !(self.new_project_confirm_pending
            || self.open_project_confirm_pending
            || self.quit_confirm_pending)
        {
            return false;
        }
        self.new_project_confirm_pending = false;
        self.open_project_confirm_pending = false;
        self.quit_confirm_pending = false;
        self.last_status = "Discard cancelled".to_string();
        true
    }

    pub(crate) fn new_project_confirm_pending(&self) -> bool {
        self.new_project_confirm_pending
    }

    pub(crate) fn open_project_confirm_pending(&self) -> bool {
        self.open_project_confirm_pending
    }

    pub(crate) fn recover_autosave_project(&mut self) {
        if self.project_dirty {
            self.last_status =
                "Unsaved changes: save or discard before recovering autosave".to_string();
            return;
        }
        let path = self.autosave_path.clone();
        let data = match std::fs::read_to_string(&path) {
            Ok(data) => data,
            Err(err) => {
                self.set_error_status(format!("Autosave open error ({}): {err}", path.display()));
                return;
            }
        };
        let project = match ProjectFile::from_text(&data) {
            Ok(project) => project,
            Err(err) => {
                self.set_error_status(format!("Autosave parse error ({}): {err}", path.display()));
                return;
            }
        };
        if let Err(err) = self.apply_project_file(project) {
            self.set_error_status(format!("Autosave load error ({}): {err}", path.display()));
            return;
        }
        self.project_path = None;
        self.clean_project_file = None;
        self.project_dirty = true;
        self.autosave_available = true;
        self.last_status = "Recovered autosave: use Save to keep it".to_string();
    }

    pub(crate) fn dismiss_autosave_project(&mut self) {
        if self.project_dirty {
            self.last_status =
                "Unsaved changes: save or discard before dismissing autosave".to_string();
            return;
        }
        match std::fs::remove_file(&self.autosave_path) {
            Ok(()) => {
                self.autosave_available = false;
                self.last_autosave_project_file = None;
                self.last_status = "Autosave dismissed".to_string();
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.autosave_available = false;
                self.last_status = "Autosave dismissed".to_string();
            }
            Err(err) => self.set_error_status(format!(
                "Autosave dismiss error ({}): {err}",
                self.autosave_path.display()
            )),
        }
    }

    pub(crate) fn recent_project_paths(&self) -> &[PathBuf] {
        &self.settings.recent_projects
    }

    pub(crate) fn open_most_recent_project(&mut self) {
        self.open_recent_project_at(0);
    }

    pub(crate) fn open_recent_project_at(&mut self, index: usize) {
        if self.project_dirty {
            self.last_status = "Unsaved changes: save or discard before opening recent".to_string();
            return;
        }
        let Some(path) = self.settings.recent_projects.get(index).cloned() else {
            self.last_status = "No recent project".to_string();
            return;
        };
        self.load_project_path(path);
    }

    pub(crate) fn forget_most_recent_project(&mut self) {
        self.forget_recent_project_at(0);
    }

    pub(crate) fn forget_recent_project_at(&mut self, index: usize) {
        if index >= self.settings.recent_projects.len() {
            self.last_status = "No recent project".to_string();
            return;
        }
        let path = self.settings.recent_projects.remove(index);
        self.last_status = format!("Forgot recent project: {}", path.display());
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
                log::error!("Audio command error: {err}");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
            if let Err(err) = synth.note_off(69) {
                log::error!("Audio command error: {err}");
            }
        });
    }

    fn stop_playback_notes(&mut self) {
        let active: Vec<u32> = self.playback_active_notes.values().copied().collect();
        self.playback_active_notes.clear();
        self.playback_active_keys.clear();
        for note in active {
            if let Err(err) = self.synth.note_off(note) {
                self.set_error_status(format!("Playback note-off error: {err}"));
            }
        }
    }

    fn project_edit_snapshot(&self) -> ProjectEditSnapshot {
        ProjectEditSnapshot {
            project: self.music_project.lock().snapshot(),
            selected_clip_note: self.selected_clip_note,
        }
    }

    fn push_project_history(&mut self) {
        let snapshot = self.project_edit_snapshot();
        if self.undo_stack.last() == Some(&snapshot) {
            return;
        }
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > MAX_PROJECT_HISTORY {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn apply_project_history_snapshot(&mut self, snapshot: ProjectEditSnapshot) {
        self.stop_playback_notes();
        self.music_project.lock().apply_snapshot(snapshot.project);
        self.selected_clip_note = snapshot.selected_clip_note;
        if let Some(note_id) = self.selected_clip_note
            && self.music_project.lock().note_by_id(note_id).is_none()
        {
            self.selected_clip_note = None;
        }
        self.last_metronome_beat = None;
    }

    pub(crate) fn mark_project_dirty(&mut self) {
        self.refresh_project_dirty_state();
        if !self.project_dirty {
            self.project_dirty = true;
        }
        self.write_project_autosave();
        self.new_project_confirm_pending = false;
        self.open_project_confirm_pending = false;
        self.quit_confirm_pending = false;
    }

    fn refresh_project_dirty_state(&mut self) {
        self.project_dirty = self
            .clean_project_file
            .as_ref()
            .is_none_or(|clean| clean != &self.project_file_snapshot());
    }

    fn establish_clean_project_snapshot(&mut self) {
        self.clean_project_file = Some(self.project_file_snapshot());
        self.project_dirty = false;
        self.last_autosave_project_file = None;
    }

    fn write_project_autosave(&mut self) {
        if !self.persist_enabled {
            return;
        }
        let project_file = self.project_file_snapshot();
        if self.last_autosave_project_file.as_ref() == Some(&project_file) {
            return;
        }
        if let Some(parent) = self.autosave_path.parent()
            && !parent.as_os_str().is_empty()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            self.autosave_available = false;
            self.set_error_status(format!(
                "Project autosave error ({}): {err}",
                self.autosave_path.display()
            ));
            return;
        }
        match std::fs::write(&self.autosave_path, project_file.to_text()) {
            Ok(()) => {
                self.autosave_available = true;
                self.last_autosave_project_file = Some(project_file);
            }
            Err(err) => {
                self.autosave_available = false;
                self.set_error_status(format!(
                    "Project autosave error ({}): {err}",
                    self.autosave_path.display()
                ));
            }
        }
    }

    fn clear_autosave_file(&mut self) {
        match std::fs::remove_file(&self.autosave_path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => log::error!(
                "Failed to remove autosave file {}: {err}",
                self.autosave_path.display()
            ),
        }
        self.autosave_available = false;
        self.last_autosave_project_file = None;
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
            self.set_error_status(format!("Metronome error: {err}"));
            return;
        }
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(45));
            if let Err(err) = synth.note_off(METRONOME_NOTE_ID) {
                log::error!("Metronome note-off error: {err}");
            }
        });
    }

    fn clip_note_status(&self, prefix: &str, note: &ClipNote) -> String {
        let scale = self.scale_state.lock();
        let (degree, octave) = scale
            .note_info(note.musical_note)
            .map(|info| (info.degree + 1, info.octave))
            .unwrap_or((0, 0));
        format!(
            "{prefix} d{degree} o{octave} beat {:.2} len {:.2} vel {}",
            note.start_beats, note.duration_beats, note.velocity
        )
    }

    fn audition_selected_clip_note(&mut self) {
        if let Some(note) = self.selected_clip_note() {
            self.audition_clip_note(&note);
        }
    }

    fn audition_clip_note(&mut self, note: &ClipNote) {
        let synth = self.synth.clone();
        let freq = note.freq;
        let velocity = (note.velocity as f32 / 127.0).clamp(0.0, 1.0);
        if let Err(err) = synth.note_on(AUDITION_NOTE_ID, freq, velocity) {
            log::error!("Audition note-on error: {err}");
            return;
        }
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(140));
            if let Err(err) = synth.note_off(AUDITION_NOTE_ID) {
                log::error!("Audition note-off error: {err}");
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

    fn apply_project_file(&mut self, project: ProjectFile) -> Result<(), String> {
        self.stop_playback_notes();
        if let Some(scala_path) = project.scala_path.clone() {
            let scale = parse_scala(&scala_path).map_err(|err| {
                format!("Project Scala load error ({}): {err}", scala_path.display())
            })?;
            self.scale_state.lock().scale = scale;
            self.scala_path = Some(scala_path.clone());
            self.add_scale_library_path(scala_path);
        } else {
            let mut state = self.scale_state.lock();
            state.scale = ScaleState::default().scale;
            self.scala_path = None;
        }
        {
            let mut state = self.scale_state.lock();
            state.root_midi = project.root_midi;
            state.base_freq = project.base_freq;
        }
        self.synth
            .set_settings(project.synth_settings)
            .map_err(|err| format!("Synth settings error: {err}"))?;
        if let Some(lumatone_path) = project.lumatone_path.clone() {
            if !self.load_lumatone_path(lumatone_path.clone()) {
                self.lumatone_path = None;
                *self.lumatone_map.lock() = None;
                self.append_status(&format!(
                    "Key map unavailable ({})",
                    lumatone_path.display()
                ));
            }
        } else {
            self.lumatone_path = None;
            *self.lumatone_map.lock() = None;
        }
        self.music_project.lock().apply_snapshot(project.project);
        self.selected_clip_note = None;
        self.last_metronome_beat = None;
        Ok(())
    }

    fn add_recent_project_path(&mut self, path: PathBuf) {
        self.settings
            .recent_projects
            .retain(|recent| !same_path(recent, &path));
        self.settings.recent_projects.insert(0, path);
        self.settings.recent_projects.truncate(MAX_RECENT_PROJECTS);
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
        let Some((index, item)) = self
            .scale_library
            .get(self.selected_scale_library)
            .cloned()
            .map(|item| (self.selected_scale_library, item))
        else {
            self.last_status = "No scale selected".to_string();
            return;
        };
        if !item.path.exists() {
            self.scale_library.remove(index);
            if self.selected_scale_library >= self.scale_library.len() {
                self.selected_scale_library = self.scale_library.len().saturating_sub(1);
            }
            self.last_status = format!("Removed missing scale: {}", item.name);
            self.persist_settings(None);
            return;
        }
        match self.load_scale_path(item.path, true) {
            Ok(()) => self.mark_project_dirty(),
            Err(err) => self.set_error_status(format!("Scala parse error: {err}")),
        }
    }

    pub(crate) fn remove_selected_library_scale(&mut self) {
        if self.selected_scale_library < self.scale_library.len() {
            let item = self.scale_library.remove(self.selected_scale_library);
            if self.selected_scale_library >= self.scale_library.len() {
                self.selected_scale_library = self.scale_library.len().saturating_sub(1);
            }
            self.last_status = format!("Removed scale: {}", item.name);
            self.persist_settings(None);
        }
    }

    pub(crate) fn selected_library_scale_is_loaded(&self) -> bool {
        let Some(item) = self.scale_library.get(self.selected_scale_library) else {
            return false;
        };
        self.scala_path
            .as_ref()
            .is_some_and(|path| same_path(path, &item.path))
    }

    pub(crate) fn can_remove_selected_library_scale(&self) -> bool {
        self.scale_library
            .get(self.selected_scale_library)
            .is_some_and(|item| !item.path.starts_with("scales"))
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
        self.last_status = format!(
            "Refreshed scale library: {} scales",
            self.scale_library.len()
        );
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
        self.last_status = format!(
            "Refreshed asset browser: {} assets",
            self.audio_assets.len()
        );
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

    pub(crate) fn load_lumatone_path(&mut self, path: PathBuf) -> bool {
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
                true
            }
            Err(err) => {
                self.set_error_status(format!("Key map load error: {err}"));
                false
            }
        }
    }

    pub(crate) fn reload_lumatone_presets(&mut self) {
        let current = self.lumatone_path.clone();
        self.load_lumatone_presets(Path::new("lumatone_factory_presets"));
        self.last_status = format!("Refreshed key map presets: {}", self.lumatone_presets.len());
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
        self.settings.midi_channel_filter = self.midi_channel_filter();
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

pub(crate) fn audio_stream_info_label(info: &AudioStreamInfo) -> String {
    let khz = info.sample_rate_hz as f32 / 1000.0;
    let rate = if (khz - khz.round()).abs() < 0.01 {
        format!("{khz:.0} kHz")
    } else {
        format!("{khz:.1} kHz")
    };
    let buffer = info
        .buffer_frames
        .map(|frames| {
            let ms = frames as f32 * 1000.0 / info.sample_rate_hz.max(1) as f32;
            format!(" {frames}f {ms:.1}ms")
        })
        .unwrap_or_default();
    format!(
        "{rate} {}ch {}{}",
        info.channels, info.sample_format, buffer
    )
}

fn autosave_path_for_settings_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("orbifold_settings")
        .replace("_settings", "");
    path.with_file_name(format!("{stem}_autosave.orbifold"))
}

fn temporary_project_save_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("project.orbifold");
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

fn project_backup_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("project.orbifold");
    path.with_file_name(format!("{name}.bak"))
}

fn clamp_piano_view_start(start: f32, loop_beats: f32, visible_beats: f32) -> f32 {
    start.clamp(0.0, (loop_beats - visible_beats).max(0.0))
}

#[cfg(not(test))]
fn run_file_dialog(request: FileDialogRequest) -> Option<PathBuf> {
    match request {
        FileDialogRequest::OpenProject => rfd::FileDialog::new()
            .add_filter("Orbifold project", &["orbifold", "mtdaw"])
            .pick_file(),
        FileDialogRequest::SaveProject { file_name, .. } => rfd::FileDialog::new()
            .add_filter("Orbifold project", &["orbifold"])
            .set_file_name(&file_name)
            .save_file(),
        FileDialogRequest::OpenScale => rfd::FileDialog::new()
            .add_filter("Scala scale", &["scl"])
            .pick_file(),
        FileDialogRequest::OpenKeymap => rfd::FileDialog::new()
            .add_filter("Lumatone key map", &["ltn"])
            .pick_file(),
        FileDialogRequest::ImportAudioAsset { kind } => rfd::FileDialog::new()
            .add_filter(kind.label(), kind.extensions())
            .pick_file(),
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

fn snap_beat_to_grid_cell(beat: f32, step: f32, loop_beats: f32) -> f32 {
    let loop_beats = loop_beats.max(1.0);
    let step = step.max(f32::EPSILON);
    let wrapped = beat.rem_euclid(loop_beats);
    (((wrapped + GRID_CELL_SNAP_EPSILON) / step).floor() * step).rem_euclid(loop_beats)
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
