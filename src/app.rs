#[cfg(feature = "native-app")]
use midir::{Ignore, MidiInput, MidiInputPort};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, TryRecvError},
};
use std::time::Duration;

#[cfg(any(
    feature = "native-app",
    all(
        feature = "web-app",
        target_arch = "wasm32",
        not(feature = "native-app")
    )
))]
use crate::audio::build_audio_stream;
use crate::audio::{AudioOutputDevice, AudioStream, AudioStreamInfo, list_audio_outputs};
use crate::midi::parse_lumatone_map_contents;
use crate::midi::{
    MIDI_CHANNEL_FILTER_ALL, MidiEvent, MidiInputConnectionHandle, SharedLumatoneMap,
    SharedMidiCapture, SharedMidiChannelFilter, SharedMidiHeld, SharedMidiLast, SharedMidiLog,
    SharedMidiSustain, list_midi_inputs, load_lumatone_map,
};
#[cfg(any(test, feature = "native-app", feature = "web-app"))]
use crate::midi::{MidiSharedState, handle_midi};
use crate::project::{
    ClipNote, MIN_NOTE_BEATS, MusicProject, ProjectFile, ProjectSnapshot, QuantizeGrid,
    SharedMusicProject, active_key_set, playback_note_id,
};
#[cfg(any(test, feature = "web-app"))]
use crate::sample_preview::decode_wav_preview;
use crate::sample_preview::load_wav_preview;
use crate::scala::{parse_scala, parse_scala_contents};
use crate::scale::ScaleState;
use crate::settings::AppSettings;
use crate::synth::{SamplePreviewBuffer, SynthHandle};
use crate::time::AppInstant;

const MAX_PROJECT_HISTORY: usize = 64;
const MAX_RECENT_PROJECTS: usize = 8;
const MIN_BPM: f32 = 20.0;
const MAX_BPM: f32 = 320.0;
const METRONOME_NOTE_ID: u32 = 1_900_000;
const AUDITION_NOTE_ID: u32 = 1_800_000;
const AUDIO_ASSETS_DIR: &str = "audio_assets";
const DEFAULT_ADDED_NOTE_BEATS: f32 = 1.0;
const GRID_CELL_SNAP_EPSILON: f32 = 0.0001;
const UI_SCALE_MIN: f32 = 0.75;
const UI_SCALE_MAX: f32 = 2.0;
const PROJECT_BACKUP_SLOTS: usize = 3;
const MAX_DIAGNOSTIC_MESSAGES: usize = 8;
const PIANO_DEFAULT_VISIBLE_BEATS: f32 = 16.0;
const PIANO_MIN_VISIBLE_BEATS: f32 = 1.0;
const PIANO_WHEEL_ZOOM_BASE: f32 = 0.85;
const PIANO_DEFAULT_VISIBLE_PITCH_RADIUS: i32 = 12;
const PIANO_MIN_VISIBLE_PITCH_RADIUS: i32 = 2;
const PIANO_MAX_VISIBLE_PITCH_RADIUS: i32 = 64;
pub(crate) const PIANO_MIN_PITCH: i32 = -128;
pub(crate) const PIANO_MAX_PITCH: i32 = 256;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceResizeTarget {
    Left,
    Track,
    Right,
    Bottom,
    Browser,
}

impl AudioAssetKind {
    pub(crate) fn all() -> [Self; 4] {
        [Self::Sample, Self::Instrument, Self::Preset, Self::Impulse]
    }

    #[cfg(any(test, feature = "web-app"))]
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn from_index(index: usize) -> Option<Self> {
        Self::all().get(index).copied()
    }

    pub(crate) fn index(self) -> usize {
        match self {
            Self::Sample => 0,
            Self::Instrument => 1,
            Self::Preset => 2,
            Self::Impulse => 3,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Sample => "Samples",
            Self::Instrument => "Instruments",
            Self::Preset => "Presets",
            Self::Impulse => "Impulses",
        }
    }

    pub(crate) fn singular_label(self) -> &'static str {
        match self {
            Self::Sample => "sample",
            Self::Instrument => "instrument",
            Self::Preset => "preset",
            Self::Impulse => "impulse",
        }
    }

    fn sound_workflow_status(self) -> &'static str {
        match self {
            Self::Sample => "WAV required for preview or project sample",
            Self::Instrument => "library only; no instrument playback yet",
            Self::Preset => "library only; no synth preset loading yet",
            Self::Impulse => "library only; no effects loading yet",
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SampleInstrumentAssignment {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
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

    #[cfg(all(not(test), feature = "native-app"))]
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
    pub(crate) midi_connection: Option<MidiInputConnectionHandle>,
    pub(crate) midi_inputs: Vec<String>,
    pub(crate) selected_input: usize,
    pub(crate) connected_midi_input: String,
    pub(crate) audio_stream: Option<AudioStream>,
    pub(crate) audio_stream_info: Option<AudioStreamInfo>,
    pub(crate) audio_outputs: Vec<AudioOutputDevice>,
    pub(crate) selected_audio_output: usize,
    pub(crate) connected_audio_output: String,
    pub(crate) last_status: String,
    diagnostic_messages: Vec<String>,
    pub(crate) scala_path: Option<PathBuf>,
    pub(crate) scale_library: Vec<ScaleLibraryItem>,
    pub(crate) selected_scale_library: usize,
    scale_library_scroll_start: Option<usize>,
    scale_library_search_query: String,
    pub(crate) audio_assets: Vec<AudioAssetItem>,
    pub(crate) selected_audio_asset: Option<usize>,
    pub(crate) selected_audio_asset_kind: AudioAssetKind,
    pub(crate) sample_instrument_assignment: Option<SampleInstrumentAssignment>,
    pub(crate) missing_sample_instrument_path: Option<PathBuf>,
    browser_sample_assets: HashMap<PathBuf, SamplePreviewBuffer>,
    audio_asset_scroll_starts: [Option<usize>; 4],
    audio_asset_search_query: String,
    pub(crate) show_asset_browser: bool,
    pub(crate) show_scale_browser: bool,
    pub(crate) show_clip_panel: bool,
    pub(crate) show_device_panel: bool,
    pub(crate) show_settings_panel: bool,
    piano_view_start_beats: f32,
    piano_view_visible_beats: f32,
    piano_view_center_pitch: i32,
    piano_view_pitch_radius: i32,
    piano_pitch_labels_show_degrees: bool,
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
    pub(crate) bpm_edit_buffer: Option<String>,
    pub(crate) root_midi_edit_buffer: Option<String>,
    pub(crate) base_freq_edit_buffer: Option<String>,
    last_snap_grid: QuantizeGrid,
    playback_active_notes: HashMap<u64, u32>,
    undo_stack: Vec<ProjectEditSnapshot>,
    redo_stack: Vec<ProjectEditSnapshot>,
    recording_edit_snapshot: Option<ProjectEditSnapshot>,
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

#[derive(Default)]
struct ProjectLoadOptions {
    browser_scala_resource: Option<(PathBuf, String)>,
    browser_lumatone_resource: Option<(PathBuf, String)>,
    missing_file_paths_are_warnings: bool,
}

impl ProjectLoadOptions {
    fn browser_scala_resource(&self, path: &Path) -> Option<&str> {
        self.browser_scala_resource
            .as_ref()
            .and_then(|(resource_path, text)| (resource_path == path).then_some(text.as_str()))
    }

    fn browser_lumatone_resource(&self, path: &Path) -> Option<&str> {
        self.browser_lumatone_resource
            .as_ref()
            .and_then(|(resource_path, text)| (resource_path == path).then_some(text.as_str()))
    }
}

impl AppState {
    #[cfg(test)]
    pub(crate) fn for_layout_tests() -> Self {
        Self::for_ephemeral_state(PathBuf::from("orbifold_layout_test_settings.txt"))
    }

    #[cfg(feature = "web-app")]
    pub(crate) fn for_web() -> Self {
        Self::for_web_with_settings(AppSettings::default())
    }

    #[cfg(feature = "web-app")]
    pub(crate) fn for_web_with_settings(settings: AppSettings) -> Self {
        let mut app = Self::for_ephemeral_state_with_settings(
            PathBuf::from("orbifold_web_settings.txt"),
            settings,
        );
        app.audio_outputs = list_audio_outputs();
        app.connected_audio_output.clear();
        app.select_connected_audio_output();
        app
    }

    #[cfg(any(test, feature = "web-app"))]
    fn for_ephemeral_state(settings_path: PathBuf) -> Self {
        Self::for_ephemeral_state_with_settings(settings_path, AppSettings::default())
    }

    #[cfg(any(test, feature = "web-app"))]
    fn for_ephemeral_state_with_settings(settings_path: PathBuf, settings: AppSettings) -> Self {
        let synth = SynthHandle::new(32);
        if let Err(err) = synth.set_settings(settings.synth_settings()) {
            log::error!("Failed to initialize ephemeral synth settings: {err}");
        }
        let midi_channel_filter = Arc::new(std::sync::atomic::AtomicI8::new(
            settings
                .midi_channel_filter
                .map(|channel| channel as i8)
                .unwrap_or(MIDI_CHANNEL_FILTER_ALL),
        ));
        let mut app = Self {
            scale_state: Arc::new(Mutex::new(ScaleState {
                root_midi: settings.root_midi,
                base_freq: settings.base_freq,
                ..ScaleState::default()
            })),
            synth,
            midi_last: Arc::new(Mutex::new(None)),
            midi_log: Arc::new(Mutex::new(Vec::new())),
            midi_capture: Arc::new(Mutex::new(Default::default())),
            midi_held: Arc::new(Mutex::new(HashMap::new())),
            midi_sustain: Arc::new(Mutex::new(Default::default())),
            midi_channel_filter,
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
            diagnostic_messages: Vec::new(),
            scala_path: None,
            scale_library: Vec::new(),
            selected_scale_library: 0,
            scale_library_scroll_start: None,
            scale_library_search_query: String::new(),
            audio_assets: Vec::new(),
            selected_audio_asset: None,
            selected_audio_asset_kind: AudioAssetKind::Sample,
            sample_instrument_assignment: None,
            missing_sample_instrument_path: None,
            browser_sample_assets: HashMap::new(),
            audio_asset_scroll_starts: [None; 4],
            audio_asset_search_query: String::new(),
            show_asset_browser: settings.show_asset_browser,
            show_scale_browser: settings.show_scale_browser,
            show_clip_panel: settings.show_clip_panel,
            show_device_panel: false,
            show_settings_panel: false,
            piano_view_start_beats: 0.0,
            piano_view_visible_beats: PIANO_DEFAULT_VISIBLE_BEATS,
            piano_view_center_pitch: settings.root_midi,
            piano_view_pitch_radius: PIANO_DEFAULT_VISIBLE_PITCH_RADIUS,
            piano_pitch_labels_show_degrees: false,
            midi_debug: Arc::new(AtomicBool::new(settings.midi_debug)),
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
            bpm_edit_buffer: None,
            root_midi_edit_buffer: None,
            base_freq_edit_buffer: None,
            last_snap_grid: QuantizeGrid::Sixteenth,
            playback_active_notes: HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            recording_edit_snapshot: None,
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
        audio_stream: Option<AudioStream>,
        audio_stream_info: Option<AudioStreamInfo>,
        connected_audio_output: String,
        settings: AppSettings,
        settings_status: Option<String>,
        probe_audio_outputs: bool,
        probe_midi_inputs: bool,
        persist_startup_settings: bool,
    ) -> Self {
        let startup_status = settings_status.clone();
        let diagnostic_messages = initial_diagnostic_messages(settings_status.as_deref());
        let settings_path = AppSettings::default_path();
        let autosave_path = autosave_path_for_settings_path(&settings_path);
        let autosave_available = autosave_recovery_file_exists(&autosave_path);
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
            diagnostic_messages,
            scala_path: None,
            scale_library: Vec::new(),
            selected_scale_library: 0,
            scale_library_scroll_start: None,
            scale_library_search_query: String::new(),
            audio_assets: Vec::new(),
            selected_audio_asset: None,
            selected_audio_asset_kind: AudioAssetKind::Sample,
            sample_instrument_assignment: None,
            missing_sample_instrument_path: None,
            browser_sample_assets: HashMap::new(),
            audio_asset_scroll_starts: [None; 4],
            audio_asset_search_query: String::new(),
            show_asset_browser: settings.show_asset_browser,
            show_scale_browser: settings.show_scale_browser,
            show_clip_panel: settings.show_clip_panel,
            show_device_panel: false,
            show_settings_panel: false,
            piano_view_start_beats: 0.0,
            piano_view_visible_beats: PIANO_DEFAULT_VISIBLE_BEATS,
            piano_view_center_pitch: settings.root_midi,
            piano_view_pitch_radius: PIANO_DEFAULT_VISIBLE_PITCH_RADIUS,
            piano_pitch_labels_show_degrees: false,
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
            bpm_edit_buffer: None,
            root_midi_edit_buffer: None,
            base_freq_edit_buffer: None,
            last_snap_grid: QuantizeGrid::Sixteenth,
            playback_active_notes: HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            recording_edit_snapshot: None,
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
        if let Some(summary) = app.device_setup_summary() {
            app.show_device_panel = true;
            app.show_settings_panel = false;
            app.append_status(&format!("Device setup required: {summary}"));
        }
        app.restore_startup_status(startup_status);
        app.persist_enabled = true;
        if persist_startup_settings {
            app.persist_settings(None);
        }
        app.establish_clean_project_snapshot();
        if app.autosave_available {
            app.append_status("Autosave ready: Recover or Dismiss");
        }
        app.append_keymap_scale_warning();
        app
    }

    fn append_status(&mut self, message: &str) {
        if self.last_status == "Ready" {
            self.last_status = message.to_string();
        } else if !self.last_status.contains(message) {
            self.last_status = format!("{}; {message}", self.last_status);
        }
    }

    pub(crate) fn set_error_status(&mut self, message: impl Into<String>) {
        let message = message.into();
        log::error!("{message}");
        self.record_diagnostic(message.clone());
        self.last_status = message;
    }

    fn record_error_diagnostic(&mut self, message: impl Into<String>) {
        let message = message.into();
        log::error!("{message}");
        self.record_diagnostic(message);
    }

    fn append_error_status(&mut self, message: &str) {
        self.record_error_diagnostic(message);
        self.append_status(message);
    }

    pub(crate) fn diagnostic_messages(&self) -> &[String] {
        &self.diagnostic_messages
    }

    pub(crate) fn clear_diagnostics(&mut self) {
        if self.diagnostic_messages.is_empty() {
            self.last_status = "No diagnostics to clear".to_string();
        } else {
            self.diagnostic_messages.clear();
            self.last_status = "Diagnostics cleared".to_string();
        }
    }

    fn record_diagnostic(&mut self, message: impl Into<String>) {
        let message = message.into();
        let message = message.trim();
        if message.is_empty() {
            return;
        }
        if self
            .diagnostic_messages
            .last()
            .is_some_and(|last| last == message)
        {
            return;
        }
        self.diagnostic_messages.push(message.to_string());
        if self.diagnostic_messages.len() > MAX_DIAGNOSTIC_MESSAGES {
            let excess = self.diagnostic_messages.len() - MAX_DIAGNOSTIC_MESSAGES;
            self.diagnostic_messages.drain(0..excess);
        }
    }

    pub(crate) fn set_status_preserving_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        if status_is_error(&self.last_status) {
            self.append_status(&message);
        } else {
            self.last_status = message;
        }
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
        if let Err(err) = pending.completion_sender.send(selection) {
            log::error!("Failed to deliver test file dialog selection: {err}");
        }
    }

    fn begin_file_dialog(&mut self, request: FileDialogRequest) {
        if self.pending_file_dialog.is_some() {
            self.last_status = "A file dialog is already open".to_string();
            return;
        }
        #[cfg(all(not(test), not(feature = "native-app")))]
        {
            self.set_error_status(format!(
                "Browser {} picker is not connected yet",
                request.label()
            ));
        }
        #[cfg(any(test, feature = "native-app"))]
        {
            let (sender, receiver) = std::sync::mpsc::channel();
            #[cfg(all(not(test), feature = "native-app"))]
            {
                let request_for_thread = request.clone();
                let thread_name = format!("orbifold-{}", request_for_thread.thread_name());
                let thread_sender = sender.clone();
                if let Err(err) = std::thread::Builder::new()
                    .name(thread_name)
                    .spawn(move || {
                        let selection = run_file_dialog(request_for_thread);
                        if let Err(err) = thread_sender.send(selection) {
                            log::error!("Failed to deliver file dialog selection: {err}");
                        }
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
    }

    fn finish_file_dialog(&mut self, request: FileDialogRequest, selection: Option<PathBuf>) {
        let Some(path) = selection else {
            self.last_status = request.cancel_status().to_string();
            return;
        };
        match request {
            FileDialogRequest::OpenProject => self.load_project_path(path),
            FileDialogRequest::SaveProject { .. } => self.save_project_to_path(path),
            FileDialogRequest::OpenScale => match self.load_scale_path(path, true) {
                Ok(true) => self.mark_project_dirty(),
                Ok(false) => {}
                Err(err) => self.set_error_status(format!("Scala parse error: {err}")),
            },
            FileDialogRequest::OpenKeymap => {
                if self.load_lumatone_path(path) {
                    let loaded_status = self.last_status.clone();
                    self.mark_project_dirty();
                    self.set_status_preserving_error(loaded_status);
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
        self.autosave_available = autosave_recovery_file_exists(&self.autosave_path);
    }

    #[cfg(test)]
    pub(crate) fn set_settings_path_for_tests(&mut self, path: PathBuf, persist_enabled: bool) {
        self.settings_path = path;
        self.autosave_path = autosave_path_for_settings_path(&self.settings_path);
        self.autosave_available = autosave_recovery_file_exists(&self.autosave_path);
        self.persist_enabled = persist_enabled;
    }

    pub(crate) fn refresh_audio_outputs(&mut self) {
        self.apply_refreshed_audio_outputs(list_audio_outputs(), true);
    }

    pub(crate) fn apply_refreshed_audio_outputs(
        &mut self,
        outputs: Vec<AudioOutputDevice>,
        report_status: bool,
    ) {
        let previous_connected = self.connected_audio_output.clone();
        let connected_output_missing = !previous_connected.is_empty()
            && !outputs
                .iter()
                .any(|device| device.name == previous_connected);
        self.audio_outputs = outputs;
        if connected_output_missing {
            self.audio_stream = None;
            self.audio_stream_info = None;
            self.connected_audio_output.clear();
            self.synth.clear_sender();
        }
        self.select_connected_audio_output();
        if connected_output_missing {
            self.set_error_status(format!(
                "Audio output unavailable: {previous_connected}; found {}",
                device_count_label(self.audio_outputs.len(), "audio output", "audio outputs")
            ));
        } else if report_status {
            self.last_status = format!(
                "Refreshed audio outputs: {}",
                device_count_label(self.audio_outputs.len(), "audio output", "audio outputs")
            );
        }
    }

    pub(crate) fn connect_audio_output(&mut self) {
        #[cfg(not(any(
            feature = "native-app",
            all(feature = "web-app", target_arch = "wasm32")
        )))]
        {
            self.set_error_status("Browser audio backend is not connected yet");
        }
        #[cfg(any(
            feature = "native-app",
            all(
                feature = "web-app",
                target_arch = "wasm32",
                not(feature = "native-app")
            )
        ))]
        {
            self.connect_selected_audio_output();
        }
    }

    #[cfg(any(
        feature = "native-app",
        all(
            feature = "web-app",
            target_arch = "wasm32",
            not(feature = "native-app")
        )
    ))]
    fn connect_selected_audio_output(&mut self) {
        let Some(name) = self
            .audio_outputs
            .get(self.selected_audio_output)
            .map(|device| device.name.clone())
        else {
            self.set_error_status("No audio output selected");
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
                let status = audio_connection_status(&connected_name, &info);
                self.audio_stream_info = Some(info);
                self.connected_audio_output = connected_name.clone();
                self.last_status = status;
                self.persist_settings(None);
            }
            Err(err) => {
                self.audio_stream_info = None;
                self.set_error_status(format!("Audio output error: {err}"));
            }
        }
    }

    pub(crate) fn refresh_midi_inputs(&mut self, preferred_name: Option<&str>) {
        self.apply_refreshed_midi_inputs(list_midi_inputs(), preferred_name, false);
    }

    pub(crate) fn refresh_midi_inputs_with_status(&mut self, preferred_name: Option<&str>) {
        self.apply_refreshed_midi_inputs(list_midi_inputs(), preferred_name, true);
    }

    pub(crate) fn apply_refreshed_midi_inputs(
        &mut self,
        inputs: Vec<String>,
        preferred_name: Option<&str>,
        report_status: bool,
    ) {
        let previous_connected = self.connected_midi_input.clone();
        let connected_input_missing = !previous_connected.is_empty()
            && !inputs.iter().any(|name| name == &previous_connected);
        self.midi_inputs = inputs;
        if connected_input_missing {
            self.midi_connection = None;
            self.connected_midi_input.clear();
            self.midi_held.lock().clear();
            self.midi_sustain.lock().clear();
            if let Err(err) = self.synth.all_notes_off() {
                log::error!("Audio command error while clearing stale MIDI input: {err}");
            }
        }
        if self.midi_inputs.is_empty() {
            self.selected_input = 0;
            if connected_input_missing {
                self.set_error_status(format!(
                    "MIDI input unavailable: {previous_connected}; found no MIDI inputs"
                ));
            } else if report_status {
                self.last_status = "Refreshed MIDI inputs: no MIDI inputs".to_string();
            }
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
        if connected_input_missing {
            self.set_error_status(format!(
                "MIDI input unavailable: {previous_connected}; found {}",
                device_count_label(self.midi_inputs.len(), "MIDI input", "MIDI inputs")
            ));
        } else if report_status {
            self.last_status = format!(
                "Refreshed MIDI inputs: {}",
                device_count_label(self.midi_inputs.len(), "MIDI input", "MIDI inputs")
            );
        }
    }

    #[cfg(feature = "native-app")]
    pub(crate) fn open_midi_input(&mut self) {
        if self.midi_inputs.is_empty() {
            self.midi_inputs = list_midi_inputs();
        }
        if self.midi_inputs.is_empty() {
            self.set_error_status("No MIDI inputs found");
            self.clear_midi_connection_state("no MIDI inputs found");
            self.persist_settings(None);
            return;
        }
        if self.selected_input >= self.midi_inputs.len() {
            self.selected_input = 0;
        }

        let selected_name = self.midi_inputs.get(self.selected_input).cloned();
        let Ok(mut midi_in) = MidiInput::new("orbifold") else {
            self.set_error_status("Failed to initialize MIDI input");
            self.clear_midi_connection_state("MIDI input initialization failed");
            return;
        };
        midi_in.ignore(Ignore::None);
        let ports = midi_in.ports();
        let (port, port_name) =
            match select_midi_input_port(&midi_in, &ports, selected_name.as_deref()) {
                Ok(selection) => selection,
                Err(err) => {
                    self.set_error_status(err);
                    self.clear_midi_connection_state("MIDI input port selection failed");
                    return;
                }
            };
        if let Some(idx) = self.midi_inputs.iter().position(|name| name == &port_name) {
            self.selected_input = idx;
        }
        let scale_state = self.scale_state.clone();
        let synth = self.synth.clone();
        let midi_state = MidiSharedState {
            last: self.midi_last.clone(),
            log: self.midi_log.clone(),
            capture: self.midi_capture.clone(),
            held: self.midi_held.clone(),
            sustain: self.midi_sustain.clone(),
            channel_filter: self.midi_channel_filter.clone(),
            lumatone_map: self.lumatone_map_for_input_name(&port_name),
            music_project: self.music_project.clone(),
        };
        let midi_debug = self.midi_debug.clone();

        match midi_in.connect(
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
        ) {
            Ok(conn) => {
                self.connected_midi_input = port_name.clone();
                self.last_status = format!("Connected MIDI input: {port_name}");
                self.midi_connection = Some(conn);
            }
            Err(err) => {
                self.clear_midi_connection_state("MIDI input connection failed");
                self.set_error_status(format!("Failed to connect MIDI input {port_name}: {err}"));
            }
        }
        self.persist_settings(None);
    }

    #[cfg(not(feature = "native-app"))]
    pub(crate) fn open_midi_input(&mut self) {
        self.clear_midi_connection_state("browser MIDI backend unavailable");
        self.set_error_status("Browser MIDI backend is not connected yet");
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn apply_browser_midi_inputs(&mut self, inputs: Vec<String>) {
        self.apply_refreshed_midi_inputs(inputs, None, true);
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn connect_browser_midi_input(&mut self, name: String) {
        if let Some(index) = self.midi_inputs.iter().position(|input| input == &name) {
            self.selected_input = index;
        } else if !name.is_empty() {
            self.midi_inputs.insert(0, name.clone());
            self.selected_input = 0;
        }
        self.connected_midi_input = name.clone();
        self.last_status = format!("Connected browser MIDI input: {name}");
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn handle_browser_midi_message(&mut self, message: &[u8]) {
        let input_name = self.connected_midi_input.clone();
        let midi_state = MidiSharedState {
            last: self.midi_last.clone(),
            log: self.midi_log.clone(),
            capture: self.midi_capture.clone(),
            held: self.midi_held.clone(),
            sustain: self.midi_sustain.clone(),
            channel_filter: self.midi_channel_filter.clone(),
            lumatone_map: self.lumatone_map_for_input_name(&input_name),
            music_project: self.music_project.clone(),
        };
        handle_midi(
            message,
            &self.scale_state,
            &self.synth,
            &midi_state,
            self.midi_debug.load(Ordering::Relaxed),
        );
    }

    fn clear_midi_connection_state(&mut self, context: &str) {
        self.midi_connection = None;
        self.connected_midi_input.clear();
        self.midi_held.lock().clear();
        self.midi_sustain.lock().clear();
        if let Err(err) = self.synth.all_notes_off() {
            log::error!("Audio command error while clearing MIDI state after {context}: {err}");
        }
    }

    pub(crate) fn selected_midi_input_uses_lumatone_map(&self) -> bool {
        let input_name = if self.connected_midi_input.is_empty() {
            self.midi_inputs
                .get(self.selected_input)
                .map(String::as_str)
        } else {
            Some(self.connected_midi_input.as_str())
        };
        input_name.is_some_and(midi_input_name_uses_lumatone_map)
    }

    fn lumatone_map_for_input_name(&self, input_name: &str) -> SharedLumatoneMap {
        if midi_input_name_uses_lumatone_map(input_name) {
            self.lumatone_map.clone()
        } else {
            Arc::new(Mutex::new(None))
        }
    }

    pub(crate) fn bpm_edit_text(&self) -> String {
        self.bpm_edit_buffer.clone().unwrap_or_else(|| {
            let bpm = self.music_project.lock().transport.bpm;
            format!("{bpm:.0}")
        })
    }

    pub(crate) fn append_bpm_edit_text(&mut self, text: &str) {
        let buffer = self.bpm_edit_buffer.get_or_insert_with(String::new);
        let mut accepted = false;
        for ch in text.chars() {
            let valid = ch.is_ascii_digit() || (ch == '.' && !buffer.contains('.'));
            if valid && buffer.len() < 6 {
                buffer.push(ch);
                accepted = true;
            }
        }
        self.last_status = if accepted {
            if buffer.is_empty() {
                "Enter BPM".to_string()
            } else {
                format!("BPM {buffer}")
            }
        } else {
            "BPM accepts numbers".to_string()
        };
    }

    pub(crate) fn clear_bpm_edit_text(&mut self) {
        self.bpm_edit_buffer = Some(String::new());
        self.last_status = "Enter BPM".to_string();
    }

    pub(crate) fn backspace_bpm_edit_text(&mut self) {
        if self.bpm_edit_buffer.is_none() {
            let bpm = self.music_project.lock().transport.bpm;
            self.bpm_edit_buffer = Some(format!("{bpm:.0}"));
        }
        let buffer = self
            .bpm_edit_buffer
            .as_mut()
            .expect("BPM edit buffer should exist");
        buffer.pop();
        self.last_status = if buffer.is_empty() {
            "Enter BPM".to_string()
        } else {
            format!("BPM {buffer}")
        };
    }

    pub(crate) fn cancel_bpm_edit_text(&mut self) {
        self.bpm_edit_buffer = None;
        let bpm = self.music_project.lock().transport.bpm;
        self.last_status = format!("BPM {bpm:.2} unchanged");
    }

    pub(crate) fn commit_bpm_edit_text(&mut self) -> bool {
        let Some(raw) = self.bpm_edit_buffer.take() else {
            let bpm = self.music_project.lock().transport.bpm;
            self.last_status = format!("BPM {bpm:.2} unchanged");
            return false;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            let bpm = self.music_project.lock().transport.bpm;
            self.last_status = format!("BPM {bpm:.2} unchanged");
            return false;
        }
        let Ok(value) = trimmed.parse::<f32>() else {
            self.bpm_edit_buffer = Some(raw);
            self.set_error_status("Invalid BPM");
            return false;
        };
        if !value.is_finite() {
            self.bpm_edit_buffer = Some(raw);
            self.set_error_status("Invalid BPM");
            return false;
        }
        self.set_transport_bpm(value)
    }

    pub(crate) fn adjust_transport_bpm(&mut self, delta: f32) -> bool {
        let bpm = self.music_project.lock().transport.bpm + delta;
        self.set_transport_bpm(bpm)
    }

    pub(crate) fn set_transport_bpm(&mut self, bpm: f32) -> bool {
        let bpm = bpm.clamp(MIN_BPM, MAX_BPM);
        let previous = {
            let mut project = self.music_project.lock();
            let previous = project.transport.bpm;
            project.transport.bpm = bpm;
            previous
        };
        if (bpm - previous).abs() <= f32::EPSILON {
            self.last_status = format!("BPM {bpm:.2} unchanged");
            return false;
        }
        self.bpm_edit_buffer = None;
        self.last_status = format!("BPM {bpm:.2}");
        self.mark_project_dirty();
        self.persist_current_settings();
        true
    }

    pub(crate) fn root_midi_edit_text(&self) -> String {
        self.root_midi_edit_buffer
            .clone()
            .unwrap_or_else(|| midi_note_name_for_root(self.scale_state.lock().root_midi))
    }

    pub(crate) fn append_root_midi_edit_text(&mut self, text: &str) {
        let buffer = self.root_midi_edit_buffer.get_or_insert_with(String::new);
        let mut accepted = false;
        for ch in text.chars() {
            let valid = ch.is_ascii_alphanumeric() || matches!(ch, '#' | '-');
            if valid && buffer.len() < 6 {
                buffer.push(ch);
                accepted = true;
            }
        }
        self.last_status = if accepted {
            if buffer.is_empty() {
                "Enter root note".to_string()
            } else {
                format!("Root {buffer}")
            }
        } else {
            "Root accepts C4 or MIDI 60".to_string()
        };
    }

    pub(crate) fn clear_root_midi_edit_text(&mut self) {
        self.root_midi_edit_buffer = Some(String::new());
        self.last_status = "Enter root note".to_string();
    }

    pub(crate) fn backspace_root_midi_edit_text(&mut self) {
        if self.root_midi_edit_buffer.is_none() {
            self.root_midi_edit_buffer =
                Some(midi_note_name_for_root(self.scale_state.lock().root_midi));
        }
        let buffer = self
            .root_midi_edit_buffer
            .as_mut()
            .expect("Root MIDI edit buffer should exist");
        buffer.pop();
        self.last_status = if buffer.is_empty() {
            "Enter root note".to_string()
        } else {
            format!("Root {buffer}")
        };
    }

    pub(crate) fn cancel_root_midi_edit_text(&mut self) {
        self.root_midi_edit_buffer = None;
        let root = self.scale_state.lock().root_midi;
        self.last_status = format!("Root {} ({root}) unchanged", midi_note_name_for_root(root));
    }

    pub(crate) fn commit_root_midi_edit_text(&mut self) -> bool {
        let Some(raw) = self.root_midi_edit_buffer.take() else {
            let root = self.scale_state.lock().root_midi;
            self.last_status = format!("Root {} ({root}) unchanged", midi_note_name_for_root(root));
            return false;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            let root = self.scale_state.lock().root_midi;
            self.last_status = format!("Root {} ({root}) unchanged", midi_note_name_for_root(root));
            return false;
        }
        let Some(root) = parse_root_midi_value(trimmed) else {
            self.root_midi_edit_buffer = Some(raw);
            self.set_error_status("Invalid root note");
            return false;
        };
        self.set_scale_root_midi(root)
    }

    pub(crate) fn adjust_scale_root_midi(&mut self, delta: i32) -> bool {
        let root = self.scale_state.lock().root_midi.saturating_add(delta);
        self.set_scale_root_midi(root)
    }

    pub(crate) fn set_scale_root_midi(&mut self, root_midi: i32) -> bool {
        let root_midi = root_midi.clamp(0, 127);
        let previous = {
            let mut scale = self.scale_state.lock();
            let previous = scale.root_midi;
            scale.root_midi = root_midi;
            previous
        };
        if root_midi == previous {
            self.last_status = format!(
                "Root {} ({root_midi}) unchanged",
                midi_note_name_for_root(root_midi)
            );
            return false;
        }
        self.root_midi_edit_buffer = None;
        let retuned = self.retune_clip_notes_to_current_scale();
        let retuned_sounding = self.retune_sounding_notes_to_current_scale();
        self.last_status = status_with_retune_counts(
            format!("Root {} ({root_midi})", midi_note_name_for_root(root_midi)),
            retuned,
            retuned_sounding,
        );
        self.mark_project_dirty();
        self.persist_current_settings();
        true
    }

    pub(crate) fn base_freq_edit_text(&self) -> String {
        self.base_freq_edit_buffer.clone().unwrap_or_else(|| {
            let base_freq = self.scale_state.lock().base_freq;
            format!("{base_freq:.2}")
        })
    }

    pub(crate) fn append_base_freq_edit_text(&mut self, text: &str) {
        let buffer = self.base_freq_edit_buffer.get_or_insert_with(String::new);
        let mut accepted = false;
        for ch in text.chars() {
            let valid = ch.is_ascii_digit() || (ch == '.' && !buffer.contains('.'));
            if valid && buffer.len() < 9 {
                buffer.push(ch);
                accepted = true;
            }
        }
        self.last_status = if accepted {
            if buffer.is_empty() {
                "Enter base frequency".to_string()
            } else {
                format!("Base frequency {buffer} Hz")
            }
        } else {
            "Base frequency accepts numbers".to_string()
        };
    }

    pub(crate) fn clear_base_freq_edit_text(&mut self) {
        self.base_freq_edit_buffer = Some(String::new());
        self.last_status = "Enter base frequency".to_string();
    }

    pub(crate) fn backspace_base_freq_edit_text(&mut self) {
        if self.base_freq_edit_buffer.is_none() {
            let base_freq = self.scale_state.lock().base_freq;
            self.base_freq_edit_buffer = Some(format!("{base_freq:.2}"));
        }
        let buffer = self
            .base_freq_edit_buffer
            .as_mut()
            .expect("Base frequency edit buffer should exist");
        buffer.pop();
        self.last_status = if buffer.is_empty() {
            "Enter base frequency".to_string()
        } else {
            format!("Base frequency {buffer} Hz")
        };
    }

    pub(crate) fn cancel_base_freq_edit_text(&mut self) {
        self.base_freq_edit_buffer = None;
        let base_freq = self.scale_state.lock().base_freq;
        self.last_status = format!("Base frequency {base_freq:.2} Hz unchanged");
    }

    pub(crate) fn commit_base_freq_edit_text(&mut self) -> bool {
        let Some(raw) = self.base_freq_edit_buffer.take() else {
            let base_freq = self.scale_state.lock().base_freq;
            self.last_status = format!("Base frequency {base_freq:.2} Hz unchanged");
            return false;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            let base_freq = self.scale_state.lock().base_freq;
            self.last_status = format!("Base frequency {base_freq:.2} Hz unchanged");
            return false;
        }
        let Ok(value) = trimmed.parse::<f32>() else {
            self.base_freq_edit_buffer = Some(raw);
            self.set_error_status("Invalid base frequency");
            return false;
        };
        if !value.is_finite() {
            self.base_freq_edit_buffer = Some(raw);
            self.set_error_status("Invalid base frequency");
            return false;
        }
        self.set_scale_base_freq(value)
    }

    pub(crate) fn adjust_scale_base_freq(&mut self, delta: f32) -> bool {
        let base_freq = self.scale_state.lock().base_freq + delta;
        self.set_scale_base_freq(base_freq)
    }

    pub(crate) fn set_scale_base_freq(&mut self, base_freq: f32) -> bool {
        let base_freq = base_freq.clamp(8.0, 20_000.0);
        let previous = {
            let mut scale = self.scale_state.lock();
            let previous = scale.base_freq;
            scale.base_freq = base_freq;
            previous
        };
        if (base_freq - previous).abs() <= f32::EPSILON {
            self.last_status = format!("Base frequency {base_freq:.2} Hz unchanged");
            return false;
        }
        self.base_freq_edit_buffer = None;
        let retuned = self.retune_clip_notes_to_current_scale();
        let retuned_sounding = self.retune_sounding_notes_to_current_scale();
        self.last_status = status_with_retune_counts(
            format!("Base frequency {base_freq:.2} Hz"),
            retuned,
            retuned_sounding,
        );
        self.mark_project_dirty();
        self.persist_current_settings();
        true
    }

    pub(crate) fn all_notes_off(&mut self) {
        self.stop_playback_notes();
        self.midi_held.lock().clear();
        self.midi_sustain.lock().clear();
        match self.synth.all_notes_off() {
            Ok(()) => self.last_status = "All notes off".to_string(),
            Err(err) => self.set_error_status(format!("All notes off error: {err}")),
        }
    }

    pub(crate) fn play_transport(&mut self) {
        self.stop_playback_notes();
        self.music_project.lock().play(AppInstant::now());
        self.last_metronome_beat = None;
        self.last_status = "Transport playing".to_string();
    }

    pub(crate) fn stop_transport(&mut self) {
        self.music_project.lock().stop(AppInstant::now());
        let recorded_notes = self.finish_recording_edit_if_changed();
        self.stop_playback_notes();
        self.last_metronome_beat = None;
        let status = recorded_notes.map_or_else(
            || "Transport stopped".to_string(),
            |count| format!("Transport stopped; recorded {}", note_count_label(count)),
        );
        self.set_status_preserving_error(status);
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
        self.recording_edit_snapshot = Some(self.project_edit_snapshot());
        let now = AppInstant::now();
        let held_notes = self.midi_held.lock().values().cloned().collect::<Vec<_>>();
        let sustain_down = self.midi_sustain.lock().any_down();
        let status = {
            let mut project = self.music_project.lock();
            project.start_recording(now);
            project.seed_recording_state(&held_notes, sustain_down, now);
            recording_start_status(&project)
        };
        self.last_metronome_beat = None;
        self.set_status_preserving_error(status);
    }

    pub(crate) fn stop_recording(&mut self) {
        self.music_project.lock().stop_recording(AppInstant::now());
        let recorded_notes = self.finish_recording_edit_if_changed().unwrap_or(0);
        self.set_status_preserving_error(format!(
            "Recording stopped: {}",
            note_count_label(recorded_notes)
        ));
    }

    pub(crate) fn toggle_recording(&mut self) {
        if self.music_project.lock().transport.recording {
            self.stop_recording();
        } else {
            self.start_recording();
        }
    }

    pub(crate) fn return_transport_to_start(&mut self) {
        let recorded_notes = self.seek_transport_to_inner(0.0);
        let status = recorded_notes.map_or_else(
            || "Returned to start".to_string(),
            |count| format!("Returned to start; recorded {}", note_count_label(count)),
        );
        self.set_status_preserving_error(status);
    }

    pub(crate) fn seek_transport_to(&mut self, beat: f32) {
        let recorded_notes = self.seek_transport_to_inner(beat);
        let status = recorded_notes.map_or_else(
            || format!("Seek {:.2}", beat.max(0.0)),
            |count| {
                format!(
                    "Seek {:.2}; recorded {}",
                    beat.max(0.0),
                    note_count_label(count)
                )
            },
        );
        self.set_status_preserving_error(status);
    }

    fn seek_transport_to_inner(&mut self, beat: f32) -> Option<usize> {
        self.music_project.lock().seek(beat, AppInstant::now());
        let recorded_notes = self.finish_recording_edit_if_changed();
        self.stop_playback_notes();
        self.last_metronome_beat = None;
        recorded_notes
    }

    pub(crate) fn set_loop_beats(&mut self, beats: f32) -> bool {
        let beats = self.snap_loop_length_beats(beats).clamp(1.0, 128.0);
        let previous = {
            let mut project = self.music_project.lock();
            let previous = project.transport.loop_beats;
            project.transport.loop_beats = beats;
            previous
        };
        if (beats - previous).abs() <= f32::EPSILON {
            self.last_status = format!("Loop length {beats:.0} beats unchanged");
            return false;
        }
        let visible = self.piano_view_visible_beats(beats);
        self.piano_view_start_beats =
            clamp_piano_view_start(self.piano_view_start_beats, beats, visible);
        self.mark_project_dirty();
        self.set_status_preserving_error(format!("Loop length {beats:.0} beats"));
        self.persist_current_settings();
        true
    }

    fn snap_loop_length_beats(&self, beats: f32) -> f32 {
        let step = self
            .music_project
            .lock()
            .transport
            .quantize_grid
            .step_beats();
        if let Some(step) = step {
            (beats / step).round() * step
        } else {
            (beats * 100.0).round() / 100.0
        }
    }

    pub(crate) fn toggle_metronome(&mut self) {
        let enabled = {
            let mut project = self.music_project.lock();
            project.transport.metronome_enabled = !project.transport.metronome_enabled;
            project.transport.metronome_enabled
        };
        self.mark_project_dirty();
        if enabled {
            self.set_status_preserving_error("Metronome on");
        } else {
            self.set_status_preserving_error("Metronome off");
        }
    }

    pub(crate) fn toggle_quantize_on_record(&mut self) {
        let enabled = {
            let mut project = self.music_project.lock();
            project.transport.quantize_on_record = !project.transport.quantize_on_record;
            project.transport.quantize_on_record
        };
        self.mark_project_dirty();
        if enabled {
            self.set_status_preserving_error("Record quantize on");
        } else {
            self.set_status_preserving_error("Record quantize off");
        }
    }

    pub(crate) fn toggle_snap_to_grid(&mut self) {
        let grid = {
            let project = self.music_project.lock();
            project.transport.quantize_grid
        };
        if grid == QuantizeGrid::Off {
            self.set_quantize_grid(self.last_snap_grid);
            self.set_status_preserving_error(format!("Snap on {}", self.last_snap_grid.as_str()));
        } else {
            self.last_snap_grid = grid;
            self.set_quantize_grid(QuantizeGrid::Off);
            self.set_status_preserving_error("Snap off");
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
            self.set_status_preserving_error(format!("Grid {}", grid.as_str()));
        } else {
            self.last_status = format!("Grid {} unchanged", grid.as_str());
        }
    }

    pub(crate) fn clear_clip(&mut self) {
        self.stop_playback_notes();
        let history_len = self.push_project_history_attempt();
        if self.music_project.lock().clear_clip() {
            self.selected_clip_note = None;
            self.mark_project_dirty();
            self.set_status_preserving_error("Clip cleared");
        } else {
            self.discard_project_history_attempt(history_len);
            self.last_status = "Clip already empty".to_string();
        }
    }

    pub(crate) fn quantize_clip(&mut self) {
        let history_len = self.push_project_history_attempt();
        if self.music_project.lock().quantize_clip() {
            self.mark_project_dirty();
            self.set_status_preserving_error("Clip quantized");
        } else {
            self.discard_project_history_attempt(history_len);
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
        self.ensure_clip_note_visible(&note);
        self.last_status = self.clip_note_status("Selected note", &note);
        self.audition_clip_note(&note);
    }

    pub(crate) fn selected_clip_note(&self) -> Option<ClipNote> {
        self.selected_clip_note
            .and_then(|id| self.music_project.lock().note_by_id(id))
    }

    fn ensure_clip_note_visible(&mut self, note: &ClipNote) -> bool {
        let loop_beats = self.music_project.lock().transport.loop_beats.max(1.0);
        let visible_beats = self.piano_view_visible_beats(loop_beats);
        let view_start = self.piano_view_start_beats(loop_beats);
        let mut changed = false;

        if !clip_note_intersects_time_view(note, view_start, visible_beats, loop_beats) {
            let note_start = note.start_beats.rem_euclid(loop_beats);
            let view_end = view_start + visible_beats;
            let margin = (visible_beats * 0.08).clamp(0.125, 1.0);
            let target_start = if note_start < view_start {
                note_start - margin
            } else if note_start > view_end {
                note_start - visible_beats * 0.5
            } else if note_start < view_start + margin {
                note_start - margin
            } else {
                note_start - visible_beats + margin
            };
            let start = clamp_piano_view_start(target_start, loop_beats, visible_beats);
            if (start - self.piano_view_start_beats).abs() > f32::EPSILON {
                self.piano_view_start_beats = start;
                changed = true;
            }
        }

        let (min_pitch, max_pitch) = self.piano_pitch_range();
        let pitch = note.musical_note.clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH);
        let pitch_margin = (self.piano_view_pitch_radius / 4).clamp(1, 4);
        let mut center = self.piano_view_center_pitch;
        if pitch < min_pitch + pitch_margin {
            center += pitch - (min_pitch + pitch_margin);
        } else if pitch > max_pitch - pitch_margin {
            center += pitch - (max_pitch - pitch_margin);
        }
        center = center.clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH);
        if center != self.piano_view_center_pitch {
            self.piano_view_center_pitch = center;
            changed = true;
        }

        changed
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
            let beat = project.current_position_beats(AppInstant::now());
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
            self.ensure_clip_note_visible(&note);
            self.set_status_preserving_error(self.clip_note_status("Pasted note", &note));
        }
    }

    pub(crate) fn delete_selected_clip_note(&mut self) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        let history_len = self.push_project_history_attempt();
        if self.music_project.lock().delete_note(note_id) {
            self.selected_clip_note = None;
            self.mark_project_dirty();
            self.set_status_preserving_error("Deleted clip note");
        } else {
            self.discard_project_history_attempt(history_len);
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn duplicate_selected_clip_note(&mut self) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        let history_len = self.push_project_history_attempt();
        let duplicated = { self.music_project.lock().duplicate_note(note_id) };
        match duplicated {
            Some(new_id) => {
                self.selected_clip_note = Some(new_id);
                if let Some(note) = self.selected_clip_note() {
                    self.ensure_clip_note_visible(&note);
                }
                self.mark_project_dirty();
                self.set_status_preserving_error("Duplicated clip note");
            }
            None => {
                self.discard_project_history_attempt(history_len);
                self.last_status = "Selected clip note no longer exists".to_string();
            }
        }
    }

    pub(crate) fn nudge_selected_clip_note(&mut self, direction: f32) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        let step = self.music_project.lock().edit_step_beats() * direction;
        let history_len = self.push_project_history_attempt();
        if self.music_project.lock().nudge_note(note_id, step) {
            if let Some(note) = self.selected_clip_note() {
                self.ensure_clip_note_visible(&note);
            }
            self.mark_project_dirty();
            self.set_status_preserving_error("Moved clip note");
        } else {
            self.discard_project_history_attempt(history_len);
            self.last_status = "Selected clip note no longer exists".to_string();
        }
    }

    pub(crate) fn resize_selected_clip_note(&mut self, direction: f32) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        let step = self.music_project.lock().edit_step_beats() * direction;
        let history_len = self.push_project_history_attempt();
        if self.music_project.lock().resize_note(note_id, step) {
            if let Some(note) = self.selected_clip_note() {
                self.ensure_clip_note_visible(&note);
            }
            self.mark_project_dirty();
            self.set_status_preserving_error("Resized clip note");
        } else {
            self.discard_project_history_attempt(history_len);
            if self.music_project.lock().note_by_id(note_id).is_some() {
                self.last_status = "Clip note length unchanged".to_string();
            } else {
                self.last_status = "Selected clip note no longer exists".to_string();
            }
        }
    }

    pub(crate) fn set_selected_clip_note_velocity(&mut self, velocity: u8) {
        let Some(note_id) = self.selected_existing_clip_note_id() else {
            return;
        };
        if self
            .selected_clip_note()
            .is_some_and(|note| note.velocity == velocity.min(127))
        {
            self.last_status = "Velocity unchanged".to_string();
            return;
        }
        let history_len = self.push_project_history_attempt();
        if self
            .music_project
            .lock()
            .set_note_velocity(note_id, velocity)
        {
            self.mark_project_dirty();
            if let Some(note) = self.selected_clip_note() {
                self.set_status_preserving_error(self.clip_note_status("Changed velocity", &note));
            }
        } else {
            self.discard_project_history_attempt(history_len);
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
        let history_len = self.push_project_history_attempt();
        if self
            .music_project
            .lock()
            .set_note_pitch(note.id, musical_note, info.freq)
        {
            self.mark_project_dirty();
            if let Some(note) = self.selected_clip_note() {
                self.ensure_clip_note_visible(&note);
            }
            self.set_status_preserving_error("Moved clip note pitch");
            self.audition_selected_clip_note();
        } else {
            self.discard_project_history_attempt(history_len);
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
            self.ensure_clip_note_visible(&note);
            self.set_status_preserving_error(self.clip_note_status("Added note", &note));
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
        let history_len = self.push_project_history_attempt();
        if self.music_project.lock().quantize_note(note_id) {
            self.mark_project_dirty();
            if let Some(note) = self.selected_clip_note() {
                self.ensure_clip_note_visible(&note);
                self.set_status_preserving_error(self.clip_note_status("Quantized note", &note));
            }
        } else {
            self.discard_project_history_attempt(history_len);
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
        let start_beats = {
            let project = self.music_project.lock();
            let loop_beats = project.transport.loop_beats.max(1.0);
            project
                .transport
                .quantize_grid
                .step_beats()
                .map(|step| snap_beat_to_grid_line(start_beats, step, loop_beats))
                .unwrap_or_else(|| start_beats.rem_euclid(loop_beats))
        };
        let Some(note) = self.music_project.lock().note_by_id(note_id) else {
            self.last_status = "Selected clip note no longer exists".to_string();
            return false;
        };
        let raw_note = musical_note.clamp(0, 127) as u8;
        if (note.start_beats - start_beats).abs() <= f32::EPSILON
            && note.musical_note == musical_note
            && (note.freq - info.freq).abs() <= f32::EPSILON
            && note.raw_note == raw_note
            && note.key_index == -1
            && !note.mapped_from_lumatone
        {
            return false;
        }
        if push_history {
            self.push_project_history();
        }
        let changed = self.music_project.lock().set_note_start_and_pitch(
            note_id,
            start_beats,
            musical_note,
            info.freq,
        );
        if changed {
            self.selected_clip_note = Some(note_id);
            self.mark_project_dirty();
            self.set_status_preserving_error("Moved clip note");
        }
        changed
    }

    pub(crate) fn resize_clip_note_start_to(
        &mut self,
        note_id: u64,
        start_beats: f32,
        push_history: bool,
    ) -> bool {
        let start_beats = self.snap_clip_edit_beat(start_beats);
        let Some((start_beats, _duration_beats, changed)) =
            self.note_start_resize_preview(note_id, start_beats)
        else {
            self.last_status = "Selected clip note no longer exists".to_string();
            return false;
        };
        if !changed {
            return false;
        }
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
            self.set_status_preserving_error("Resized clip note");
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
        let end_beats = self.snap_clip_edit_beat(end_beats);
        let loop_beats = self.music_project.lock().transport.loop_beats.max(1.0);
        let duration = (end_beats - note.start_beats).rem_euclid(loop_beats);
        let duration = duration.clamp(MIN_NOTE_BEATS, loop_beats.max(MIN_NOTE_BEATS));
        if (duration - note.duration_beats).abs() <= f32::EPSILON {
            return false;
        }
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
            self.set_status_preserving_error("Resized clip note");
        }
        changed
    }

    fn note_start_resize_preview(
        &self,
        note_id: u64,
        start_beats: f32,
    ) -> Option<(f32, f32, bool)> {
        let project = self.music_project.lock();
        let note = project.note_by_id(note_id)?;
        let loop_beats = project.transport.loop_beats.max(MIN_NOTE_BEATS);
        let start_beats = start_beats.rem_euclid(loop_beats);
        let end_beats = note.start_beats + note.duration_beats;
        let duration_beats = (end_beats - start_beats)
            .rem_euclid(loop_beats)
            .clamp(MIN_NOTE_BEATS, loop_beats);
        let changed = (start_beats - note.start_beats).abs() > f32::EPSILON
            || (duration_beats - note.duration_beats).abs() > f32::EPSILON;
        Some((start_beats, duration_beats, changed))
    }

    fn snap_clip_edit_beat(&self, beat: f32) -> f32 {
        let project = self.music_project.lock();
        let loop_beats = project.transport.loop_beats.max(1.0);
        project
            .transport
            .quantize_grid
            .step_beats()
            .map(|step| snap_beat_to_grid_line(beat, step, loop_beats))
            .unwrap_or_else(|| beat.rem_euclid(loop_beats))
    }

    pub(crate) fn set_clip_note_velocity(
        &mut self,
        note_id: u64,
        velocity: u8,
        push_history: bool,
    ) -> bool {
        let velocity = velocity.min(127);
        let Some(note) = self.music_project.lock().note_by_id(note_id) else {
            self.last_status = "Selected clip note no longer exists".to_string();
            return false;
        };
        if note.velocity == velocity {
            return false;
        }
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
            self.set_status_preserving_error("Updated clip note velocity");
        }
        changed
    }

    pub(crate) fn ui_scale(&self) -> f32 {
        self.settings.ui_scale
    }

    pub(crate) fn window_title(&self) -> String {
        match self.project_path.as_ref() {
            Some(path) if self.project_dirty => {
                format!("Orbifold - {} *", project_title_name(path))
            }
            Some(path) => format!("Orbifold - {}", project_title_name(path)),
            None if self.project_dirty => "Orbifold - Untitled *".to_string(),
            None => "Orbifold".to_string(),
        }
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

    pub(crate) fn workspace_left_width(&self) -> Option<f32> {
        self.settings.layout_left_width
    }

    pub(crate) fn workspace_track_width(&self) -> Option<f32> {
        self.settings.layout_track_width
    }

    pub(crate) fn workspace_right_width(&self) -> Option<f32> {
        self.settings.layout_right_width
    }

    pub(crate) fn workspace_bottom_height(&self) -> Option<f32> {
        self.settings.layout_bottom_height
    }

    pub(crate) fn left_browser_split_height(&self) -> Option<f32> {
        self.settings.layout_browser_split_height
    }

    pub(crate) fn set_workspace_layout_size(
        &mut self,
        target: WorkspaceResizeTarget,
        value: f32,
        persist: bool,
    ) -> bool {
        let value = value.max(1.0);
        let slot = match target {
            WorkspaceResizeTarget::Left => &mut self.settings.layout_left_width,
            WorkspaceResizeTarget::Track => &mut self.settings.layout_track_width,
            WorkspaceResizeTarget::Right => &mut self.settings.layout_right_width,
            WorkspaceResizeTarget::Bottom => &mut self.settings.layout_bottom_height,
            WorkspaceResizeTarget::Browser => &mut self.settings.layout_browser_split_height,
        };
        let changed = slot.is_none_or(|previous| (previous - value).abs() > 0.5);
        *slot = Some(value);
        if changed {
            self.last_status = match target {
                WorkspaceResizeTarget::Left => format!("Asset browser width {value:.0}px"),
                WorkspaceResizeTarget::Track => format!("Clip panel width {value:.0}px"),
                WorkspaceResizeTarget::Right => format!("Control panel width {value:.0}px"),
                WorkspaceResizeTarget::Bottom => format!("Piano roll height {value:.0}px"),
                WorkspaceResizeTarget::Browser => format!("Browser split height {value:.0}px"),
            };
        }
        if persist {
            self.persist_settings(None);
        }
        changed
    }

    pub(crate) fn reset_workspace_layout(&mut self) {
        self.settings.layout_left_width = None;
        self.settings.layout_track_width = None;
        self.settings.layout_right_width = None;
        self.settings.layout_bottom_height = None;
        self.settings.layout_browser_split_height = None;
        self.settings.show_asset_browser = true;
        self.settings.show_scale_browser = false;
        self.settings.show_clip_panel = true;
        self.show_asset_browser = true;
        self.show_scale_browser = false;
        self.show_clip_panel = true;
        self.last_status = "Layout reset".to_string();
        self.persist_settings(None);
    }

    pub(crate) fn toggle_asset_browser(&mut self) {
        self.show_asset_browser = !self.show_asset_browser;
        self.settings.show_asset_browser = self.show_asset_browser;
        self.last_status = if self.show_asset_browser {
            "Asset browser shown".to_string()
        } else {
            "Asset browser hidden".to_string()
        };
        self.persist_settings(None);
    }

    pub(crate) fn toggle_scale_browser(&mut self) {
        self.show_scale_browser = !self.show_scale_browser;
        self.settings.show_scale_browser = self.show_scale_browser;
        self.last_status = if self.show_scale_browser {
            "Scale browser shown".to_string()
        } else {
            "Scale browser hidden".to_string()
        };
        self.persist_settings(None);
    }

    pub(crate) fn toggle_clip_panel(&mut self) {
        self.show_clip_panel = !self.show_clip_panel;
        self.settings.show_clip_panel = self.show_clip_panel;
        self.last_status = if self.show_clip_panel {
            "Clip panel shown".to_string()
        } else {
            "Clip panel hidden".to_string()
        };
        self.persist_settings(None);
    }

    pub(crate) fn toggle_device_panel(&mut self) {
        if self.show_device_panel {
            self.show_device_panel = false;
        } else {
            self.show_device_panel = true;
            self.show_settings_panel = false;
        }
        self.last_status = if self.show_device_panel {
            self.device_setup_summary()
                .map(|summary| format!("Device setup shown: {summary}"))
                .unwrap_or_else(|| "Device panel shown".to_string())
        } else {
            "Control panel shown".to_string()
        };
    }

    pub(crate) fn toggle_settings_panel(&mut self) {
        if self.show_settings_panel {
            self.show_settings_panel = false;
            self.last_status = "Control panel shown".to_string();
        } else {
            self.show_settings_panel = true;
            self.show_device_panel = false;
            self.last_status = "Settings panel shown".to_string();
        }
    }

    pub(crate) fn device_setup_required(&self) -> bool {
        self.device_setup_summary().is_some()
    }

    pub(crate) fn device_setup_summary(&self) -> Option<String> {
        let needs = self.device_setup_messages();
        (!needs.is_empty()).then(|| needs.join("; "))
    }

    pub(crate) fn device_setup_messages(&self) -> Vec<&'static str> {
        let mut needs = Vec::new();
        if self.audio_outputs.is_empty() {
            needs.push("audio unavailable");
        } else if self.connected_audio_output.is_empty()
            || !self
                .audio_outputs
                .iter()
                .any(|output| output.name == self.connected_audio_output)
        {
            needs.push("audio not connected");
        }
        if self.midi_inputs.is_empty() {
            needs.push("MIDI unavailable");
        } else if self.connected_midi_input.is_empty()
            || !self
                .midi_inputs
                .iter()
                .any(|input| input == &self.connected_midi_input)
        {
            needs.push("MIDI not connected");
        }
        needs
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
        self.adjust_midi_channel_filter(1);
    }

    pub(crate) fn adjust_midi_channel_filter(&mut self, direction: isize) {
        let current = match self.midi_channel_filter() {
            None => 0,
            Some(channel) => channel as isize + 1,
        };
        let next_index = (current + direction).rem_euclid(17);
        let next = if next_index == 0 {
            MIDI_CHANNEL_FILTER_ALL
        } else {
            (next_index - 1) as i8
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

    pub(crate) fn piano_pitch_labels_show_degrees(&self) -> bool {
        self.piano_pitch_labels_show_degrees
    }

    pub(crate) fn toggle_piano_pitch_label_mode(&mut self) {
        self.piano_pitch_labels_show_degrees = !self.piano_pitch_labels_show_degrees;
        self.last_status = if self.piano_pitch_labels_show_degrees {
            "Piano labels: degrees".to_string()
        } else {
            "Piano labels: notes".to_string()
        };
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

    pub(crate) fn set_piano_time_view_fraction(&mut self, fraction: f32) -> bool {
        let loop_beats = self.music_project.lock().transport.loop_beats.max(1.0);
        let visible = self.piano_view_visible_beats(loop_beats);
        let max_start = (loop_beats - visible).max(0.0);
        let start = max_start * fraction.clamp(0.0, 1.0);
        let changed = (start - self.piano_view_start_beats).abs() > f32::EPSILON;
        self.piano_view_start_beats = start;
        if changed {
            self.last_status = format!("Piano scroll beat {:.2}", self.piano_view_start_beats);
        }
        changed
    }

    pub(crate) fn set_piano_pitch_view_fraction(&mut self, fraction: f32) -> bool {
        let radius = self.piano_view_pitch_radius.clamp(
            PIANO_MIN_VISIBLE_PITCH_RADIUS,
            PIANO_MAX_VISIBLE_PITCH_RADIUS,
        );
        let highest_center = PIANO_MAX_PITCH - radius;
        let lowest_center = PIANO_MIN_PITCH + radius;
        let scroll_range = (highest_center - lowest_center).max(0) as f32;
        let center =
            (highest_center as f32 - scroll_range * fraction.clamp(0.0, 1.0)).round() as i32;
        let center = center.clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH);
        let changed = center != self.piano_view_center_pitch;
        self.piano_view_center_pitch = center;
        if changed {
            self.last_status = format!("Piano pitch scroll {}", self.piano_view_center_pitch);
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

    pub(crate) fn fit_piano_roll_view(&mut self) -> bool {
        let (loop_beats, notes) = {
            let project = self.music_project.lock();
            (
                project.transport.loop_beats.max(1.0),
                project.clip.notes.clone(),
            )
        };
        let (visible_beats, start_beats, center_pitch, pitch_radius, status) = if notes.is_empty() {
            let root = self.scale_state.lock().root_midi;
            (
                PIANO_DEFAULT_VISIBLE_BEATS.min(loop_beats),
                0.0,
                root.clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH),
                PIANO_DEFAULT_VISIBLE_PITCH_RADIUS,
                "Piano view reset".to_string(),
            )
        } else {
            let min_start = notes
                .iter()
                .map(|note| note.start_beats.rem_euclid(loop_beats))
                .fold(f32::INFINITY, f32::min);
            let max_end = notes
                .iter()
                .map(|note| {
                    (note.start_beats.rem_euclid(loop_beats) + note.duration_beats).min(loop_beats)
                })
                .fold(0.0, f32::max);
            let note_span = (max_end - min_start).max(PIANO_MIN_VISIBLE_BEATS);
            let padding = (note_span * 0.15).clamp(0.5, 2.0);
            let visible = (note_span + padding * 2.0).clamp(PIANO_MIN_VISIBLE_BEATS, loop_beats);
            let start = clamp_piano_view_start(
                min_start - (visible - note_span) * 0.5,
                loop_beats,
                visible,
            );
            let min_pitch = notes
                .iter()
                .map(|note| note.musical_note)
                .min()
                .unwrap_or_else(|| self.scale_state.lock().root_midi)
                .clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH);
            let max_pitch = notes
                .iter()
                .map(|note| note.musical_note)
                .max()
                .unwrap_or(min_pitch)
                .clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH);
            let pitch_span = (max_pitch - min_pitch).max(0);
            let radius = ((pitch_span + 1) / 2 + 2)
                .max(PIANO_DEFAULT_VISIBLE_PITCH_RADIUS)
                .clamp(
                    PIANO_MIN_VISIBLE_PITCH_RADIUS,
                    PIANO_MAX_VISIBLE_PITCH_RADIUS,
                );
            let center = ((min_pitch + max_pitch) / 2).clamp(PIANO_MIN_PITCH, PIANO_MAX_PITCH);
            (
                visible,
                start,
                center,
                radius,
                format!("Piano view fit {} notes", notes.len()),
            )
        };

        let changed = (self.piano_view_visible_beats - visible_beats).abs() > f32::EPSILON
            || (self.piano_view_start_beats - start_beats).abs() > f32::EPSILON
            || self.piano_view_center_pitch != center_pitch
            || self.piano_view_pitch_radius != pitch_radius;
        self.piano_view_visible_beats = visible_beats;
        self.piano_view_start_beats = start_beats;
        self.piano_view_center_pitch = center_pitch;
        self.piano_view_pitch_radius = pitch_radius;
        self.last_status = if changed {
            status
        } else {
            "Piano view already fit".to_string()
        };
        changed
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
        let now = AppInstant::now();
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
                self.set_error_status(format!("Playback note-off error: {err}"));
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
                Err(err) => self.set_error_status(format!("Playback note-on error: {err}")),
            }
        }
    }

    pub(crate) fn save_project_to_path(&mut self, path: PathBuf) {
        let project_file = self.project_file_snapshot_for_path(Some(&path));
        let temp_path = temporary_project_save_path(&path);
        let backup_path = project_backup_path(&path);
        let result = std::fs::write(&temp_path, project_file.to_text()).and_then(|()| {
            if path.exists() {
                rotate_project_backups(&path).map_err(|err| {
                    log::error!(
                        "Failed to rotate project backups for {}: {err}",
                        path.display()
                    );
                    err
                })?;
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
                let autosave_cleanup_error = self.clear_autosave_file().err();
                self.last_status = if let Some(err) = autosave_cleanup_error {
                    self.record_error_diagnostic(format!(
                        "Project autosave cleanup error after save ({}): {err}",
                        path.display()
                    ));
                    format!(
                        "Saved project: {}; autosave cleanup error: {err}",
                        path.display()
                    )
                } else {
                    format!("Saved project: {}", path.display())
                };
                self.persist_settings(None);
            }
            Err(err) => {
                if let Err(cleanup_err) = remove_project_save_file_if_exists(&temp_path) {
                    log::error!(
                        "Failed to remove temporary project save file {} after save error: {cleanup_err}",
                        temp_path.display()
                    );
                }
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
        let project = resolve_project_file_paths(project, &path);

        let warnings = match self.apply_project_file(project) {
            Ok(warnings) => warnings,
            Err(err) => {
                self.set_error_status(format!("Project load error ({}): {err}", path.display()));
                return;
            }
        };
        self.project_path = Some(path.clone());
        self.clean_project_file = Some(self.project_file_snapshot());
        self.project_dirty = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.add_recent_project_path(path.clone());
        self.last_status = format!("Loaded project: {}", path.display());
        for warning in warnings {
            self.append_error_status(&warning);
        }
        self.append_keymap_scale_warning();
        self.persist_settings(None);
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn load_browser_project_text(&mut self, data: &str, file_name: &str) -> bool {
        self.load_browser_project_text_with_resources(data, file_name, None, None)
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn load_browser_project_text_with_resources(
        &mut self,
        data: &str,
        file_name: &str,
        browser_scala_resource: Option<(PathBuf, String)>,
        browser_lumatone_resource: Option<(PathBuf, String)>,
    ) -> bool {
        let project = match ProjectFile::from_text(data) {
            Ok(project) => project,
            Err(err) => {
                self.set_error_status(format!("Project parse error ({file_name}): {err}"));
                return false;
            }
        };
        let warnings = match self.apply_project_file_with_options(
            project,
            ProjectLoadOptions {
                browser_scala_resource,
                browser_lumatone_resource,
                missing_file_paths_are_warnings: true,
            },
        ) {
            Ok(warnings) => warnings,
            Err(err) => {
                self.set_error_status(format!("Project load error ({file_name}): {err}"));
                return false;
            }
        };
        self.project_path = Some(PathBuf::from(file_name));
        self.clean_project_file = Some(self.project_file_snapshot_for_path(None));
        self.project_dirty = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.set_status_preserving_error(format!("Loaded browser project: {file_name}"));
        for warning in warnings {
            self.append_error_status(&warning);
        }
        self.append_keymap_scale_warning();
        true
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn browser_project_download_payload(&self) -> (String, String) {
        let file_name = self.browser_project_file_name();
        let text = self.project_file_snapshot_for_path(None).to_text();
        (file_name, text)
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn mark_browser_project_downloaded(&mut self, file_name: &str) {
        let project_file = self.project_file_snapshot_for_path(None);
        self.project_path = Some(PathBuf::from(file_name));
        self.clean_project_file = Some(project_file);
        self.project_dirty = false;
        self.last_status = format!("Downloaded browser project: {file_name}");
    }

    #[cfg(feature = "web-app")]
    pub(crate) fn browser_project_storage_text(&self) -> String {
        self.project_file_snapshot_for_path(None).to_text()
    }

    #[cfg(any(test, feature = "web-app"))]
    fn browser_project_file_name(&self) -> String {
        self.project_path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|file_name| file_name.to_str())
            .filter(|file_name| !file_name.trim().is_empty())
            .map(normalized_browser_project_file_name)
            .unwrap_or_else(|| "project.orbifold".to_string())
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
        if let Err(err) = self.clear_sample_instrument_without_status() {
            self.set_error_status(format!("Clear sample instrument error: {err}"));
            return;
        }
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
                self.autosave_available = autosave_recovery_file_exists(&path);
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
        let warnings = match self.apply_project_file(project) {
            Ok(warnings) => warnings,
            Err(err) => {
                self.set_error_status(format!("Autosave load error ({}): {err}", path.display()));
                return;
            }
        };
        self.project_path = None;
        self.clean_project_file = None;
        self.project_dirty = true;
        self.autosave_available = true;
        self.last_status = "Recovered autosave: use Save to keep it".to_string();
        for warning in warnings {
            self.append_error_status(&warning);
        }
        self.append_keymap_scale_warning();
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
        let removed_missing = self.prune_missing_recent_projects();
        if self.settings.recent_projects.is_empty() {
            self.last_status = if removed_missing == 0 {
                "No recent project".to_string()
            } else {
                missing_recent_removed_status(removed_missing)
            };
            return;
        }
        let Some(path) = self.settings.recent_projects.get(index).cloned() else {
            self.last_status = "No recent project".to_string();
            return;
        };
        self.load_project_path(path.clone());
        if removed_missing > 0
            && self
                .project_path
                .as_ref()
                .is_some_and(|project_path| same_path(project_path, &path))
        {
            self.append_status(&missing_recent_removed_status(removed_missing));
        }
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

    fn prune_missing_recent_projects(&mut self) -> usize {
        let before = self.settings.recent_projects.len();
        self.settings.recent_projects.retain(|path| path.exists());
        let removed = before - self.settings.recent_projects.len();
        if removed > 0 {
            self.persist_settings(None);
        }
        removed
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
        if let Err(err) = synth.note_on(69, 440.0, 0.6) {
            self.set_error_status(format!("Audio test tone error: {err}"));
            return;
        }
        self.last_status = "Test tone A4".to_string();
        schedule_synth_note_off(synth, 69, Duration::from_millis(300), "Audio test tone");
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

    fn push_project_history_snapshot(&mut self, snapshot: ProjectEditSnapshot) {
        if self.undo_stack.last() == Some(&snapshot) {
            return;
        }
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > MAX_PROJECT_HISTORY {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn push_project_history(&mut self) {
        let snapshot = self.project_edit_snapshot();
        self.push_project_history_snapshot(snapshot);
    }

    fn push_project_history_attempt(&mut self) -> usize {
        let previous_len = self.undo_stack.len();
        self.push_project_history();
        previous_len
    }

    fn discard_project_history_attempt(&mut self, previous_len: usize) {
        self.undo_stack.truncate(previous_len);
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

    fn finish_recording_edit_if_changed(&mut self) -> Option<usize> {
        let snapshot = self.recording_edit_snapshot.take()?;
        self.clear_missing_selected_clip_note();
        let current = self.project_edit_snapshot();
        let recorded_notes = recording_note_count(&snapshot, &current);
        if current == snapshot {
            return Some(recorded_notes);
        }
        self.push_project_history_snapshot(snapshot);
        self.mark_project_dirty();
        Some(recorded_notes)
    }

    fn clear_missing_selected_clip_note(&mut self) {
        if let Some(note_id) = self.selected_clip_note
            && self.music_project.lock().note_by_id(note_id).is_none()
        {
            self.selected_clip_note = None;
        }
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
        let project_file = self.project_file_snapshot_for_path(None);
        if self.last_autosave_project_file.as_ref() == Some(&project_file) {
            return;
        }
        if let Some(parent) = self.autosave_path.parent()
            && !parent.as_os_str().is_empty()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            self.autosave_available = autosave_recovery_file_exists(&self.autosave_path);
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
                self.autosave_available = autosave_recovery_file_exists(&self.autosave_path);
                self.set_error_status(format!(
                    "Project autosave error ({}): {err}",
                    self.autosave_path.display()
                ));
            }
        }
    }

    fn clear_autosave_file(&mut self) -> Result<(), String> {
        match std::fs::remove_file(&self.autosave_path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                let message = format!("{}: {err}", self.autosave_path.display());
                log::error!("Failed to remove autosave file {message}");
                self.autosave_available = autosave_recovery_file_exists(&self.autosave_path);
                return Err(message);
            }
        }
        self.autosave_available = false;
        self.last_autosave_project_file = None;
        Ok(())
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
        schedule_synth_note_off(
            synth,
            METRONOME_NOTE_ID,
            Duration::from_millis(45),
            "Metronome",
        );
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

    pub(crate) fn audition_piano_pitch(&mut self, musical_note: i32) {
        if self.audio_stream.is_none() {
            self.set_error_status("Audition unavailable: no audio output connected");
            return;
        }
        let Some(info) = self.scale_state.lock().note_info(musical_note) else {
            self.last_status = "Pitch cannot be tuned".to_string();
            return;
        };
        self.last_status = format!("Audition d{} o{}", info.degree + 1, info.octave);
        self.play_audition_tone(info.freq, 96);
    }

    fn audition_clip_note(&mut self, note: &ClipNote) {
        self.play_audition_tone(note.freq, note.velocity);
    }

    pub(crate) fn retune_clip_notes_to_current_scale(&mut self) -> usize {
        let scale = self.scale_state.lock().clone();
        self.music_project
            .lock()
            .retune_clip_notes(|musical_note| scale.note_info(musical_note).map(|info| info.freq))
    }

    pub(crate) fn retune_sounding_notes_to_current_scale(&mut self) -> usize {
        let scale = self.scale_state.lock().clone();
        let mut targets = HashMap::new();
        {
            let project = self.music_project.lock();
            for note in &project.clip.notes {
                let Some(synth_note) = self.playback_active_notes.get(&note.id).copied() else {
                    continue;
                };
                let Some(info) = scale.note_info(note.musical_note) else {
                    continue;
                };
                targets.insert(synth_note, info.freq);
            }
        }
        {
            let mut held = self.midi_held.lock();
            for event in held.values_mut() {
                let Some(synth_note) = u32::try_from(event.key_index).ok() else {
                    continue;
                };
                let Some(freq) = retune_midi_event_to_scale(event, &scale) else {
                    continue;
                };
                targets.insert(synth_note, freq);
            }
        }
        for event in self.midi_sustain.lock().sustained_events() {
            let Some(synth_note) = u32::try_from(event.key_index).ok() else {
                continue;
            };
            let Some(freq) = midi_event_frequency_for_scale(&event, &scale) else {
                continue;
            };
            targets.insert(synth_note, freq);
        }

        let mut retuned = 0;
        for (synth_note, freq) in targets {
            match self.synth.retune_note(synth_note, freq) {
                Ok(true) => retuned += 1,
                Ok(false) => {}
                Err(err) => self.record_error_diagnostic(format!(
                    "Audio command error while retuning note {synth_note}: {err}"
                )),
            }
        }
        retuned
    }

    fn play_audition_tone(&self, freq: f32, velocity: u8) {
        let synth = self.synth.clone();
        let velocity = (velocity as f32 / 127.0).clamp(0.0, 1.0);
        if let Err(err) = synth.note_on(AUDITION_NOTE_ID, freq, velocity) {
            log::error!("Audition note-on error: {err}");
            return;
        }
        schedule_synth_note_off(
            synth,
            AUDITION_NOTE_ID,
            Duration::from_millis(140),
            "Audition",
        );
    }

    fn project_file_snapshot(&self) -> ProjectFile {
        self.project_file_snapshot_for_path(self.project_path.as_deref())
    }

    fn project_file_snapshot_for_path(&self, project_path: Option<&Path>) -> ProjectFile {
        let scale = self.scale_state.lock().clone();
        let project_dir = project_path.and_then(project_reference_base_dir);
        ProjectFile {
            scala_path: self
                .scala_path
                .clone()
                .map(|path| project_reference_path_for_save(path, project_dir)),
            lumatone_path: self
                .lumatone_path
                .clone()
                .map(|path| project_reference_path_for_save(path, project_dir)),
            sample_instrument_path: self
                .sample_instrument_assignment
                .as_ref()
                .map(|assignment| assignment.path.clone())
                .or_else(|| self.missing_sample_instrument_path.clone())
                .map(|path| project_reference_path_for_save(path, project_dir)),
            root_midi: scale.root_midi,
            base_freq: scale.base_freq,
            synth_settings: self.synth.settings(),
            project: self.music_project.lock().snapshot(),
        }
    }

    fn apply_project_file(&mut self, project: ProjectFile) -> Result<Vec<String>, String> {
        self.apply_project_file_with_options(project, ProjectLoadOptions::default())
    }

    fn apply_project_file_with_options(
        &mut self,
        project: ProjectFile,
        options: ProjectLoadOptions,
    ) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();
        self.stop_playback_notes();
        if let Some(scala_path) = project.scala_path.clone() {
            match options
                .browser_scala_resource(&scala_path)
                .map(parse_scala_contents)
            {
                Some(Ok(scale)) => {
                    self.scale_state.lock().scale = scale;
                    self.scala_path = Some(scala_path.clone());
                    self.add_scale_library_path(scala_path);
                }
                Some(Err(err)) => {
                    self.scale_state.lock().scale = ScaleState::default().scale;
                    self.scala_path = None;
                    warnings.push(format!(
                        "Project Scala resource unavailable ({}): {err}",
                        scala_path.display()
                    ));
                }
                None => match parse_scala(&scala_path) {
                    Ok(scale) => {
                        self.scale_state.lock().scale = scale;
                        self.scala_path = Some(scala_path.clone());
                        self.add_scale_library_path(scala_path);
                    }
                    Err(err) if options.missing_file_paths_are_warnings => {
                        self.scale_state.lock().scale = ScaleState::default().scale;
                        self.scala_path = None;
                        warnings.push(format!(
                            "Scala file unavailable ({}): {err}",
                            scala_path.display()
                        ));
                    }
                    Err(err) => {
                        return Err(format!(
                            "Project Scala load error ({}): {err}",
                            scala_path.display()
                        ));
                    }
                },
            }
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
            if let Some(text) = options.browser_lumatone_resource(&lumatone_path) {
                match parse_lumatone_map_contents(text) {
                    Ok(map) => {
                        let key_count = map.len();
                        *self.lumatone_map.lock() = Some(Arc::new(map));
                        self.lumatone_path = Some(lumatone_path.clone());
                        self.add_lumatone_preset_path(lumatone_path);
                        if let Some(idx) = self.select_lumatone_index_by_current_path() {
                            self.selected_lumatone = idx;
                        }
                        warnings.push(format!("Restored browser key map ({key_count} keys)"));
                    }
                    Err(err) => {
                        self.lumatone_path = None;
                        *self.lumatone_map.lock() = None;
                        warnings.push(format!(
                            "Project key map resource unavailable ({}): {err}",
                            lumatone_path.display()
                        ));
                    }
                }
            } else if !self.load_lumatone_path(lumatone_path.clone()) {
                self.lumatone_path = None;
                *self.lumatone_map.lock() = None;
                warnings.push(format!("Key map unavailable ({})", lumatone_path.display()));
            }
        } else {
            self.lumatone_path = None;
            *self.lumatone_map.lock() = None;
        }
        if let Some(sample_path) = project.sample_instrument_path.clone() {
            if let Err(err) = self.restore_sample_instrument_path(sample_path.clone()) {
                warnings.push(err);
            }
        } else {
            self.clear_sample_instrument_without_status()
                .map_err(|err| format!("Clear sample instrument error: {err}"))?;
        }
        self.music_project.lock().apply_snapshot(project.project);
        self.selected_clip_note = None;
        self.last_metronome_beat = None;
        Ok(warnings)
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
    ) -> Result<bool, String> {
        if self
            .scala_path
            .as_ref()
            .is_some_and(|current| same_path(current, &path))
        {
            if add_to_library {
                self.add_scale_library_path(path.clone());
            }
            self.last_status = format!("Scale already loaded: {}", path_display_name(&path));
            self.append_keymap_scale_warning();
            self.persist_settings(None);
            return Ok(false);
        }

        let scale = parse_scala(&path)?;
        {
            let mut state = self.scale_state.lock();
            state.scale = scale;
        }
        let retuned = self.retune_clip_notes_to_current_scale();
        let retuned_sounding = self.retune_sounding_notes_to_current_scale();
        self.scala_path = Some(path.clone());
        if add_to_library {
            self.add_scale_library_path(path);
        }
        self.last_status =
            status_with_retune_counts("Loaded Scala file", retuned, retuned_sounding);
        self.append_keymap_scale_warning();
        self.persist_settings(None);
        Ok(true)
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn load_browser_scale_text(&mut self, data: &str, file_name: &str) -> bool {
        let scale = match parse_scala_contents(data) {
            Ok(scale) => scale,
            Err(err) => {
                self.set_error_status(format!("Scala parse error ({file_name}): {err}"));
                return false;
            }
        };
        {
            let mut state = self.scale_state.lock();
            state.scale = scale;
        }
        let retuned = self.retune_clip_notes_to_current_scale();
        let retuned_sounding = self.retune_sounding_notes_to_current_scale();
        self.scala_path = Some(PathBuf::from(file_name));
        self.set_status_preserving_error(status_with_retune_counts(
            "Loaded browser Scala file",
            retuned,
            retuned_sounding,
        ));
        self.append_keymap_scale_warning();
        self.mark_project_dirty();
        true
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
            Ok(true) => self.mark_project_dirty(),
            Ok(false) => {}
            Err(err) => self.set_error_status(format!("Scala parse error: {err}")),
        }
    }

    pub(crate) fn remove_selected_library_scale(&mut self) {
        let Some(item) = self.scale_library.get(self.selected_scale_library).cloned() else {
            self.last_status = "No scale selected".to_string();
            return;
        };
        if !self.can_remove_selected_library_scale() {
            self.last_status = format!("Bundled scale cannot be removed: {}", item.name);
            return;
        }
        self.scale_library.remove(self.selected_scale_library);
        if self.selected_scale_library >= self.scale_library.len() {
            self.selected_scale_library = self.scale_library.len().saturating_sub(1);
        }
        self.last_status = format!("Removed scale: {}", item.name);
        self.persist_settings(None);
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
            let name = path_display_name(&path);
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

        self.apply_refreshed_scale_library(library);
    }

    fn apply_refreshed_scale_library(&mut self, mut library: Vec<ScaleLibraryItem>) {
        let previous_index = self.selected_scale_library;
        let selected_path = self
            .scale_library
            .get(self.selected_scale_library)
            .map(|item| item.path.clone());
        library.sort_by(|a, b| a.name.cmp(&b.name));
        self.scale_library = library;
        let preserved_selection = selected_path.as_ref().and_then(|selected| {
            self.scale_library
                .iter()
                .position(|item| same_path(&item.path, selected))
        });
        self.selected_scale_library = preserved_selection
            .unwrap_or_else(|| previous_index.min(self.scale_library.len().saturating_sub(1)));
        let selected_missing = selected_path.is_some() && preserved_selection.is_none();
        self.last_status = if selected_missing {
            format!(
                "Refreshed scale library: {} scales; selected scale unavailable",
                self.scale_library.len()
            )
        } else {
            format!(
                "Refreshed scale library: {} scales",
                self.scale_library.len()
            )
        };
    }

    pub(crate) fn scale_library_list_start(&self, total: usize, visible_rows: usize) -> usize {
        let start = self.scale_library_scroll_start.unwrap_or_else(|| {
            list_start_for_selection(self.selected_scale_library, total, visible_rows)
        });
        clamp_list_start(start, total, visible_rows)
    }

    pub(crate) fn filtered_scale_library_list_start(
        &self,
        indices: &[usize],
        visible_rows: usize,
    ) -> usize {
        if self.scale_library_search_query.is_empty() {
            return self.scale_library_list_start(indices.len(), visible_rows);
        }
        let selected_position = indices
            .iter()
            .position(|index| *index == self.selected_scale_library)
            .unwrap_or(0);
        let start = self.scale_library_scroll_start.unwrap_or_else(|| {
            list_start_for_selection(selected_position, indices.len(), visible_rows)
        });
        clamp_list_start(start, indices.len(), visible_rows)
    }

    pub(crate) fn set_scale_library_list_start(&mut self, start: usize) {
        let start = clamp_list_start(start, self.filtered_scale_library_count(), 1);
        self.scale_library_scroll_start = Some(start);
        self.last_status = format!("Scale list row {}", start + 1);
    }

    pub(crate) fn scale_library_search_query(&self) -> &str {
        &self.scale_library_search_query
    }

    pub(crate) fn set_scale_library_search_query(&mut self, query: impl Into<String>) -> bool {
        let query = sanitize_search_query(&query.into());
        if query == self.scale_library_search_query {
            return false;
        }
        self.scale_library_search_query = query;
        self.scale_library_scroll_start = None;
        if self
            .scale_library
            .get(self.selected_scale_library)
            .is_none_or(|item| {
                !scale_library_matches_search_query(item, &self.scale_library_search_query)
            })
            && let Some(index) = self.scale_library.iter().position(|item| {
                scale_library_matches_search_query(item, &self.scale_library_search_query)
            })
        {
            self.selected_scale_library = index;
        }
        let matches = self.filtered_scale_library_count();
        self.last_status = if self.scale_library_search_query.is_empty() {
            "Scale search cleared".to_string()
        } else {
            format!(
                "Scale search: {} ({matches} matches)",
                self.scale_library_search_query
            )
        };
        true
    }

    pub(crate) fn filtered_scale_library_indices(&self) -> Vec<usize> {
        self.scale_library
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                scale_library_matches_search_query(item, &self.scale_library_search_query)
                    .then_some(idx)
            })
            .collect()
    }

    pub(crate) fn filtered_scale_library_count(&self) -> usize {
        self.scale_library
            .iter()
            .filter(|item| {
                scale_library_matches_search_query(item, &self.scale_library_search_query)
            })
            .count()
    }

    pub(crate) fn audio_asset_list_start(
        &self,
        kind: AudioAssetKind,
        selected_position: usize,
        total: usize,
        visible_rows: usize,
    ) -> usize {
        let start = self.audio_asset_scroll_starts[kind.index()]
            .unwrap_or_else(|| list_start_for_selection(selected_position, total, visible_rows));
        clamp_list_start(start, total, visible_rows)
    }

    pub(crate) fn set_audio_asset_list_start(&mut self, kind: AudioAssetKind, start: usize) {
        let total = self
            .audio_assets
            .iter()
            .filter(|asset| self.audio_asset_matches_browser_filter(asset, kind))
            .count();
        let start = clamp_list_start(start, total, 1);
        self.audio_asset_scroll_starts[kind.index()] = Some(start);
        self.last_status = format!("{} list row {}", kind.label(), start + 1);
    }

    pub(crate) fn audio_asset_search_query(&self) -> &str {
        &self.audio_asset_search_query
    }

    pub(crate) fn set_audio_asset_search_query(&mut self, query: impl Into<String>) -> bool {
        let query = sanitize_search_query(&query.into());
        if query == self.audio_asset_search_query {
            return false;
        }
        self.audio_asset_search_query = query;
        self.audio_asset_scroll_starts = [None; 4];
        self.prune_hidden_audio_asset_selection();
        let matches = self.filtered_audio_asset_count(self.selected_audio_asset_kind);
        self.last_status = if self.audio_asset_search_query.is_empty() {
            "Asset search cleared".to_string()
        } else {
            format!(
                "Asset search: {} ({matches} matches)",
                self.audio_asset_search_query
            )
        };
        true
    }

    pub(crate) fn audio_asset_matches_browser_filter(
        &self,
        asset: &AudioAssetItem,
        kind: AudioAssetKind,
    ) -> bool {
        asset.kind == kind
            && audio_asset_matches_search_query(asset, &self.audio_asset_search_query)
    }

    pub(crate) fn filtered_audio_asset_count(&self, kind: AudioAssetKind) -> usize {
        self.audio_assets
            .iter()
            .filter(|asset| self.audio_asset_matches_browser_filter(asset, kind))
            .count()
    }

    pub(crate) fn total_audio_asset_count(&self, kind: AudioAssetKind) -> usize {
        self.audio_assets
            .iter()
            .filter(|asset| asset.kind == kind)
            .count()
    }

    fn prune_hidden_audio_asset_selection(&mut self) {
        let Some(selected) = self.selected_audio_asset else {
            return;
        };
        if self.audio_assets.get(selected).is_some_and(|asset| {
            self.audio_asset_matches_browser_filter(asset, self.selected_audio_asset_kind)
        }) {
            return;
        }
        self.selected_audio_asset = None;
    }

    pub(crate) fn ensure_audio_asset_dirs(&mut self) {
        for kind in AudioAssetKind::all() {
            let path = Path::new(AUDIO_ASSETS_DIR).join(kind.folder());
            if let Err(err) = std::fs::create_dir_all(&path) {
                self.set_error_status(format!("Asset folder error: {err}"));
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
        assets.extend(
            self.audio_assets
                .iter()
                .filter(|asset| is_browser_audio_asset_path(&asset.path))
                .cloned(),
        );
        self.apply_refreshed_audio_assets(assets);
    }

    fn apply_refreshed_audio_assets(&mut self, mut assets: Vec<AudioAssetItem>) {
        let selected_path = self
            .selected_audio_asset_item()
            .map(|asset| asset.path.clone());
        sort_audio_asset_items(&mut assets);
        self.audio_assets = assets;
        self.selected_audio_asset = selected_path.as_ref().and_then(|selected| {
            self.audio_assets
                .iter()
                .position(|asset| same_path(&asset.path, selected))
        });
        self.prune_hidden_audio_asset_selection();
        let selected_missing = selected_path.is_some() && self.selected_audio_asset.is_none();
        self.last_status = if selected_missing {
            format!(
                "Refreshed asset browser: {} assets; selected asset missing",
                self.audio_assets.len()
            )
        } else {
            format!(
                "Refreshed asset browser: {} assets",
                self.audio_assets.len()
            )
        };
    }

    pub(crate) fn select_audio_asset(&mut self, index: usize) {
        let Some(asset) = self.audio_assets.get(index).cloned() else {
            self.selected_audio_asset = None;
            self.set_error_status("Selected asset unavailable");
            return;
        };
        self.selected_audio_asset = Some(index);
        self.selected_audio_asset_kind = asset.kind;
        let workflow_status = if self
            .sample_instrument_assignment
            .as_ref()
            .is_some_and(|assignment| same_path(&assignment.path, &asset.path))
        {
            "loaded as sample instrument".to_string()
        } else {
            audio_asset_workflow_status(&asset).to_string()
        };
        self.last_status = format!(
            "Selected {}: {} ({})",
            asset.kind.singular_label(),
            asset.name,
            workflow_status
        );
    }

    pub(crate) fn selected_audio_asset_item(&self) -> Option<&AudioAssetItem> {
        self.selected_audio_asset
            .and_then(|index| self.audio_assets.get(index))
    }

    pub(crate) fn can_preview_selected_audio_asset(&self) -> bool {
        self.selected_audio_asset_item().is_some_and(|asset| {
            asset.kind == AudioAssetKind::Sample
                && ((asset.path.exists() && is_wav_preview_file(asset))
                    || self.browser_sample_assets.contains_key(&asset.path))
        })
    }

    pub(crate) fn can_load_selected_sample_instrument(&self) -> bool {
        self.can_preview_selected_audio_asset()
    }

    pub(crate) fn can_clear_sample_instrument(&self) -> bool {
        self.sample_instrument_assignment.is_some() || self.missing_sample_instrument_path.is_some()
    }

    pub(crate) fn selected_sample_instrument_is_loaded(&self) -> bool {
        let Some(asset) = self.selected_audio_asset_item() else {
            return false;
        };
        self.sample_instrument_assignment
            .as_ref()
            .is_some_and(|assignment| same_path(&assignment.path, &asset.path))
    }

    pub(crate) fn load_selected_sample_instrument(&mut self) {
        let Some(asset) = self.selected_audio_asset_item().cloned() else {
            self.last_status = "Select a WAV sample to use as an instrument".to_string();
            return;
        };
        if asset.kind != AudioAssetKind::Sample {
            self.last_status = "Only sample assets can be used as a sample instrument".to_string();
            return;
        }
        if let Some(sample) = self.browser_sample_assets.get(&asset.path).cloned() {
            let already_loaded = self.selected_sample_instrument_is_loaded();
            match self.set_sample_instrument_buffer(asset.name.clone(), asset.path.clone(), sample)
            {
                Ok(()) => {
                    self.last_status = format!("Loaded sample instrument: {}", asset.name);
                    if !already_loaded {
                        self.mark_project_dirty();
                    }
                }
                Err(err) => self.set_error_status(err),
            }
            return;
        }
        if !asset.path.exists() {
            self.set_error_status(format!(
                "Sample instrument file missing: {}",
                asset.path.display()
            ));
            return;
        }
        if !is_wav_preview_file(&asset) {
            self.set_error_status(format!(
                "Sample instrument supports WAV files only: {}",
                asset.name
            ));
            return;
        }
        let already_loaded = self.selected_sample_instrument_is_loaded();
        match self.set_sample_instrument_from_path(asset.name.clone(), asset.path.clone()) {
            Ok(()) => {
                self.last_status = format!("Loaded sample instrument: {}", asset.name);
                if !already_loaded {
                    self.mark_project_dirty();
                }
            }
            Err(err) => self.set_error_status(err),
        }
    }

    pub(crate) fn clear_sample_instrument(&mut self) {
        if !self.can_clear_sample_instrument() {
            self.last_status = "No sample instrument loaded".to_string();
            return;
        }
        match self.clear_sample_instrument_without_status() {
            Ok(()) => {
                self.last_status = "Sample instrument cleared".to_string();
                self.mark_project_dirty();
            }
            Err(err) => self.set_error_status(format!("Clear sample instrument error: {err}")),
        }
    }

    fn restore_sample_instrument_path(&mut self, path: PathBuf) -> Result<(), String> {
        if let Some(sample) = self.browser_sample_assets.get(&path).cloned() {
            let name = path_display_name(&path);
            return self.set_sample_instrument_buffer(name, path, sample);
        }
        if !path.exists() {
            self.clear_loaded_sample_instrument_without_status()
                .map_err(|err| format!("Clear sample instrument error: {err}"))?;
            self.missing_sample_instrument_path = Some(path.clone());
            return Err(format!(
                "Sample instrument unavailable ({})",
                path.display()
            ));
        }
        if !is_wav_file_path(&path) {
            self.clear_sample_instrument_without_status()
                .map_err(|err| format!("Clear sample instrument error: {err}"))?;
            return Err(format!(
                "Sample instrument supports WAV files only ({})",
                path.display()
            ));
        }
        let name = sample_instrument_display_name(&path);
        self.set_sample_instrument_from_path(name, path)
    }

    fn set_sample_instrument_from_path(
        &mut self,
        name: String,
        path: PathBuf,
    ) -> Result<(), String> {
        let sample = load_wav_preview(&path)?;
        self.set_sample_instrument_buffer(name, path, sample)
    }

    fn set_sample_instrument_buffer(
        &mut self,
        name: String,
        path: PathBuf,
        sample: SamplePreviewBuffer,
    ) -> Result<(), String> {
        self.synth
            .set_sample_instrument(Some(sample))
            .map_err(|err| format!("Sample instrument error: {err}"))?;
        self.sample_instrument_assignment = Some(SampleInstrumentAssignment { name, path });
        self.missing_sample_instrument_path = None;
        Ok(())
    }

    fn clear_sample_instrument_without_status(&mut self) -> Result<(), String> {
        self.clear_loaded_sample_instrument_without_status()?;
        self.missing_sample_instrument_path = None;
        Ok(())
    }

    fn clear_loaded_sample_instrument_without_status(&mut self) -> Result<(), String> {
        self.synth.set_sample_instrument(None)?;
        self.sample_instrument_assignment = None;
        Ok(())
    }

    pub(crate) fn preview_selected_audio_asset(&mut self) {
        let Some(asset) = self.selected_audio_asset_item().cloned() else {
            self.last_status = "Select a sample to preview".to_string();
            return;
        };
        if asset.kind != AudioAssetKind::Sample {
            self.last_status = "Only sample assets can be previewed".to_string();
            return;
        }
        if let Some(preview) = self.browser_sample_assets.get(&asset.path).cloned() {
            if self.audio_stream.is_none() {
                self.set_error_status("Sample preview unavailable: no audio output connected");
                return;
            }
            match self.synth.play_sample_preview(preview) {
                Ok(()) => self.last_status = format!("Previewing sample: {}", asset.name),
                Err(err) => self.set_error_status(format!("Sample preview error: {err}")),
            }
            return;
        }
        if !asset.path.exists() {
            self.set_error_status(format!(
                "Sample preview file missing: {}",
                asset.path.display()
            ));
            return;
        }
        if !is_wav_preview_file(&asset) {
            self.set_error_status(format!(
                "Sample preview supports WAV files only: {}",
                asset.name
            ));
            return;
        }
        if self.audio_stream.is_none() {
            self.set_error_status("Sample preview unavailable: no audio output connected");
            return;
        }
        match load_wav_preview(&asset.path) {
            Ok(preview) => match self.synth.play_sample_preview(preview) {
                Ok(()) => self.last_status = format!("Previewing sample: {}", asset.name),
                Err(err) => self.set_error_status(format!("Sample preview error: {err}")),
            },
            Err(err) => self.set_error_status(err),
        }
    }

    pub(crate) fn stop_audio_asset_preview(&mut self) {
        if self.audio_stream.is_none() {
            self.set_error_status("Sample preview unavailable: no audio output connected");
            return;
        }
        if let Err(err) = self.synth.stop_sample_preview() {
            self.set_error_status(format!("Stop sample preview error: {err}"));
            return;
        }
        self.last_status = "Sample preview stopped".to_string();
    }

    pub(crate) fn import_audio_asset_path(&mut self, source: PathBuf, kind: AudioAssetKind) {
        let Some(file_name) = source
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
        else {
            self.set_error_status("Asset import error: invalid file name");
            return;
        };
        if !source.exists() {
            self.set_error_status(format!(
                "Asset import error: source file missing: {}",
                source.display()
            ));
            return;
        }
        if !source.is_file() {
            self.set_error_status(format!(
                "Asset import error: source is not a file: {}",
                source.display()
            ));
            return;
        }
        if !is_supported_audio_asset_file(kind, &source) {
            self.set_error_status(format!(
                "Asset import error: unsupported {}",
                kind.singular_label()
            ));
            return;
        }

        let dir = Path::new(AUDIO_ASSETS_DIR).join(kind.folder());
        if let Err(err) = std::fs::create_dir_all(&dir) {
            self.set_error_status(format!("Asset folder error: {err}"));
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
                self.last_status = asset_import_success_status(kind, &file_name, &target);
            }
            Err(err) => {
                self.set_error_status(format!("Asset import error: {err}"));
            }
        }
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn import_browser_audio_asset(
        &mut self,
        file_name: &str,
        bytes: &[u8],
        kind: AudioAssetKind,
    ) -> Option<PathBuf> {
        let Some(file_name) = sanitize_browser_asset_file_name(file_name) else {
            self.set_error_status("Asset import error: invalid file name");
            return None;
        };
        let source = Path::new(&file_name);
        if !is_supported_audio_asset_file(kind, source) {
            self.set_error_status(format!(
                "Asset import error: unsupported {}",
                kind.singular_label()
            ));
            return None;
        }

        let path = unique_browser_asset_path(&self.audio_assets, kind, &file_name);
        self.restore_browser_audio_asset(path, &file_name, bytes, kind, true)
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn restore_browser_audio_asset(
        &mut self,
        path: PathBuf,
        file_name: &str,
        bytes: &[u8],
        kind: AudioAssetKind,
        select_restored: bool,
    ) -> Option<PathBuf> {
        let Some(file_name) = sanitize_browser_asset_file_name(file_name) else {
            self.set_error_status("Asset restore error: invalid file name");
            return None;
        };
        let expected_root = Path::new("browser_assets").join(kind.folder());
        if !path.starts_with(&expected_root) {
            self.set_error_status(format!(
                "Asset restore error: invalid browser {} path",
                kind.singular_label()
            ));
            return None;
        }
        if !is_supported_audio_asset_file(kind, Path::new(&file_name)) {
            self.set_error_status(format!(
                "Asset restore error: unsupported {}",
                kind.singular_label()
            ));
            return None;
        }
        let name = path_display_name(&path);
        if kind == AudioAssetKind::Sample && is_wav_file_path(&path) {
            match decode_wav_preview(bytes) {
                Ok(preview) => {
                    self.browser_sample_assets.insert(path.clone(), preview);
                }
                Err(err) => {
                    self.set_error_status(format!(
                        "Sample preview decode error: {file_name}: {err}"
                    ));
                    return None;
                }
            }
        }
        if let Some(asset) = self
            .audio_assets
            .iter_mut()
            .find(|asset| asset.path == path)
        {
            asset.name = name;
            asset.kind = kind;
            asset.is_dir = false;
        } else {
            self.audio_assets.push(AudioAssetItem {
                name,
                path: path.clone(),
                kind,
                is_dir: false,
            });
        }
        sort_audio_asset_items(&mut self.audio_assets);
        if select_restored {
            self.selected_audio_asset = self
                .audio_assets
                .iter()
                .position(|asset| asset.path == path);
            self.selected_audio_asset_kind = kind;
            self.last_status = format!("Imported browser {}: {file_name}", kind.singular_label());
        } else if self.selected_audio_asset.is_none() {
            self.selected_audio_asset = self
                .audio_assets
                .iter()
                .position(|asset| asset.path == path);
            self.selected_audio_asset_kind = kind;
        }
        Some(path)
    }

    pub(crate) fn select_lumatone(&mut self, index: usize) {
        if index >= self.lumatone_presets.len() {
            return;
        }
        let path = self.lumatone_presets[index].path.clone();
        self.load_lumatone_path(path);
    }

    pub(crate) fn load_lumatone_path(&mut self, path: PathBuf) -> bool {
        if self
            .lumatone_path
            .as_ref()
            .is_some_and(|current| same_path(current, &path))
            && self.lumatone_map.lock().is_some()
        {
            self.add_lumatone_preset_path(path.clone());
            if let Some(idx) = self.select_lumatone_index_by_current_path() {
                self.selected_lumatone = idx;
            }
            self.last_status = format!("Key map already loaded: {}", path_display_name(&path));
            self.append_keymap_scale_warning();
            self.persist_settings(None);
            return false;
        }
        match load_lumatone_map(&path) {
            Ok(map) => {
                let key_count = map.len();
                let name = path_display_name(&path);
                *self.lumatone_map.lock() = Some(Arc::new(map));
                self.lumatone_path = Some(path.clone());
                self.add_lumatone_preset_path(path);
                if let Some(idx) = self.select_lumatone_index_by_current_path() {
                    self.selected_lumatone = idx;
                }
                self.last_status = format!("Loaded key map: {name} ({key_count} keys)");
                self.append_keymap_scale_warning();
                self.persist_settings(None);
                true
            }
            Err(err) => {
                self.set_error_status(format!("Key map load error: {err}"));
                false
            }
        }
    }

    #[cfg(any(test, feature = "web-app"))]
    pub(crate) fn load_browser_lumatone_text(&mut self, data: &str, file_name: &str) -> bool {
        let map = match parse_lumatone_map_contents(data) {
            Ok(map) => map,
            Err(err) => {
                self.set_error_status(format!("Key map parse error ({file_name}): {err}"));
                return false;
            }
        };
        let path = PathBuf::from(file_name);
        let key_count = map.len();
        *self.lumatone_map.lock() = Some(Arc::new(map));
        self.lumatone_path = Some(path.clone());
        self.add_lumatone_preset_path(path);
        if let Some(idx) = self.select_lumatone_index_by_current_path() {
            self.selected_lumatone = idx;
        }
        self.set_status_preserving_error(format!(
            "Loaded browser key map: {file_name} ({key_count} keys)"
        ));
        self.append_keymap_scale_warning();
        self.mark_project_dirty();
        true
    }

    pub(crate) fn keymap_scale_mismatch_warning(&self) -> Option<String> {
        self.lumatone_map.lock().as_ref()?;
        let scale_label = {
            let scale = self.scale_state.lock();
            equal_division_hint(&scale.scale.description)?
        };
        let keymap_label = self
            .lumatone_path
            .as_ref()
            .and_then(|path| equal_division_hint(&path_display_name(path)))?;
        if scale_label.divisions == keymap_label.divisions {
            return None;
        }
        Some(format!(
            "Scale/key map mismatch: scale {}, key map {}",
            scale_label.label(),
            keymap_label.label()
        ))
    }

    fn append_keymap_scale_warning(&mut self) {
        if let Some(warning) = self.keymap_scale_mismatch_warning() {
            self.append_status(&warning);
        }
    }

    pub(crate) fn reload_lumatone_presets(&mut self) {
        let current = self.lumatone_path.clone();
        self.load_lumatone_presets(Path::new("lumatone_factory_presets"));
        self.last_status = format!("Refreshed key map presets: {}", self.lumatone_presets.len());
        if let Some(path) = current {
            if !path.exists() {
                self.selected_lumatone = self
                    .selected_lumatone
                    .min(self.lumatone_presets.len().saturating_sub(1));
                self.last_status = format!(
                    "Refreshed key map presets: {}; active key map missing: {}",
                    self.lumatone_presets.len(),
                    path_display_name(&path)
                );
                return;
            }
            self.add_lumatone_preset_path(path.clone());
            if self.select_lumatone_by_path(&path) {
                if !status_is_error(&self.last_status) {
                    self.last_status = format!(
                        "Refreshed key map presets: {}; selected {}",
                        self.lumatone_presets.len(),
                        path_display_name(&path)
                    );
                    self.append_keymap_scale_warning();
                }
                return;
            }
        }
        self.select_saved_or_default_lumatone();
        if !status_is_error(&self.last_status) {
            self.last_status = if let Some(path) = self.lumatone_path.as_ref() {
                format!(
                    "Refreshed key map presets: {}; selected {}",
                    self.lumatone_presets.len(),
                    path_display_name(path)
                )
            } else {
                format!("Refreshed key map presets: {}", self.lumatone_presets.len())
            };
            self.append_keymap_scale_warning();
        }
    }

    pub(crate) fn persist_settings_with_status(&mut self) {
        self.persist_settings(Some("Saved settings"));
    }

    pub(crate) fn persist_current_settings(&mut self) {
        self.persist_settings(None);
    }

    #[cfg(feature = "web-app")]
    pub(crate) fn browser_settings_text(&mut self) -> String {
        self.capture_settings();
        self.settings.to_text()
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
            Err(err) => self.set_error_status(format!("Settings save error: {err}")),
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
            let name = path_display_name(&path);
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
            let name = path_display_name(&path);
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

#[cfg(feature = "native-app")]
fn select_midi_input_port(
    midi_in: &MidiInput,
    ports: &[MidiInputPort],
    selected_name: Option<&str>,
) -> Result<(MidiInputPort, String), String> {
    let selected_name = selected_name.filter(|name| !name.is_empty());
    let mut readable_ports = Vec::new();
    let mut unreadable_ports = 0;

    for port in ports {
        match midi_in.port_name(port) {
            Ok(name) => readable_ports.push((port.clone(), name)),
            Err(err) => {
                unreadable_ports += 1;
                if let Some(selected_name) = selected_name {
                    log::error!(
                        "Failed to read MIDI input port name while searching for {selected_name}: {err}"
                    );
                } else {
                    log::error!("Failed to read MIDI input port name while opening input: {err}");
                }
            }
        }
    }

    let readable_names = readable_ports
        .iter()
        .map(|(_, name)| name.as_str())
        .collect::<Vec<_>>();
    let index =
        select_midi_input_candidate_index(&readable_names, unreadable_ports, selected_name)?;
    Ok(readable_ports
        .into_iter()
        .nth(index)
        .expect("selected readable MIDI input index should exist"))
}

fn select_midi_input_candidate_index(
    readable_names: &[&str],
    unreadable_ports: usize,
    selected_name: Option<&str>,
) -> Result<usize, String> {
    let selected_name = selected_name.filter(|name| !name.is_empty());

    if let Some(selected_name) = selected_name {
        if let Some(index) = readable_names
            .iter()
            .position(|name| *name == selected_name)
        {
            return Ok(index);
        }
        if unreadable_ports > 0 {
            Err(format!(
                "Selected MIDI input unavailable: {selected_name}; skipped {}",
                device_count_label(unreadable_ports, "unreadable port", "unreadable ports")
            ))
        } else {
            Err(format!("Selected MIDI input unavailable: {selected_name}"))
        }
    } else if !readable_names.is_empty() {
        Ok(0)
    } else if unreadable_ports > 0 {
        Err(format!(
            "No readable MIDI inputs: skipped {}",
            device_count_label(unreadable_ports, "unreadable port", "unreadable ports")
        ))
    } else {
        Err("No MIDI inputs found".to_string())
    }
}

fn device_count_label(count: usize, singular: &str, plural: &str) -> String {
    match count {
        0 => format!("no {plural}"),
        1 => format!("1 {singular}"),
        count => format!("{count} {plural}"),
    }
}

fn note_count_label(count: usize) -> String {
    device_count_label(count, "note", "notes")
}

fn sounding_note_count_label(count: usize) -> String {
    device_count_label(count, "sounding note", "sounding notes")
}

fn schedule_synth_note_off(synth: SynthHandle, note: u32, delay: Duration, context: &'static str) {
    #[cfg(all(feature = "web-app", target_arch = "wasm32"))]
    {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::prelude::Closure;

        let closure = Closure::<dyn FnMut()>::wrap(Box::new(move || {
            if let Err(err) = synth.note_off(note) {
                log::error!("{context} note-off error: {err}");
            }
        }));
        let delay_ms = delay.as_millis().min(i32::MAX as u128) as i32;
        schedule_timeout_js(closure.as_ref().unchecked_ref(), delay_ms);
        closure.forget();
    }

    #[cfg(not(all(feature = "web-app", target_arch = "wasm32")))]
    {
        if let Err(err) = std::thread::Builder::new()
            .name(format!("orbifold-note-off-{note}"))
            .spawn(move || {
                std::thread::sleep(delay);
                if let Err(err) = synth.note_off(note) {
                    log::error!("{context} note-off error: {err}");
                }
            })
        {
            log::error!("{context} note-off thread error: {err}");
        }
    }
}

#[cfg(all(feature = "web-app", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = window, js_name = setTimeout)]
    fn schedule_timeout_js(callback: &wasm_bindgen::JsValue, delay_ms: i32) -> i32;
}

fn recording_note_count(before: &ProjectEditSnapshot, after: &ProjectEditSnapshot) -> usize {
    let before_count = before.project.clip.notes.len();
    let after_count = after.project.clip.notes.len();
    if before.project.transport.overdub {
        after_count.saturating_sub(before_count)
    } else {
        after_count
    }
}

fn retune_midi_event_to_scale(event: &mut MidiEvent, scale: &ScaleState) -> Option<f32> {
    if !event.mapped_from_lumatone {
        event.musical_note = scale.musical_note_for_midi_note(event.midi_note as i32);
    }
    let info = scale.note_info(event.musical_note)?;
    event.freq = Some(info.freq);
    event.scale_degree = Some(info.degree);
    event.scale_octave = Some(info.octave);
    event.cents_from_root = Some(info.cents_from_root);
    Some(info.freq)
}

fn midi_event_frequency_for_scale(event: &MidiEvent, scale: &ScaleState) -> Option<f32> {
    let musical_note = if event.mapped_from_lumatone {
        event.musical_note
    } else {
        scale.musical_note_for_midi_note(event.midi_note as i32)
    };
    scale.note_info(musical_note).map(|info| info.freq)
}

fn recording_start_status(project: &MusicProject) -> String {
    let mode = if project.transport.overdub {
        "Overdub"
    } else {
        "Replace"
    };
    let timing = if project.transport.quantize_on_record
        && project.transport.quantize_grid.step_beats().is_some()
    {
        format!("quantized {}", project.transport.quantize_grid.as_str())
    } else {
        "free timing".to_string()
    };
    format!("Recording: {mode}, {timing}")
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

fn audio_connection_status(name: &str, info: &AudioStreamInfo) -> String {
    format!(
        "Connected audio output: {name} {}",
        audio_stream_info_label(info)
    )
}

pub(crate) fn status_with_retune_counts(
    status: impl Into<String>,
    retuned: usize,
    retuned_sounding: usize,
) -> String {
    let status = status.into();
    let mut parts = Vec::new();
    if retuned > 0 {
        parts.push(format!("retuned {}", note_count_label(retuned)));
    }
    if retuned_sounding > 0 {
        parts.push(format!(
            "retuned {}",
            sounding_note_count_label(retuned_sounding)
        ));
    }
    if parts.is_empty() {
        status
    } else {
        format!("{status}; {}", parts.join("; "))
    }
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

fn autosave_recovery_file_exists(path: &Path) -> bool {
    path.is_file()
}

fn status_is_error(status: &str) -> bool {
    status.to_ascii_lowercase().contains("error")
}

fn initial_diagnostic_messages(status: Option<&str>) -> Vec<String> {
    status
        .into_iter()
        .flat_map(|status| status.split(';'))
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .filter(|message| status_is_diagnostic(message))
        .take(MAX_DIAGNOSTIC_MESSAGES)
        .map(ToOwned::to_owned)
        .collect()
}

fn status_is_diagnostic(status: &str) -> bool {
    let status = status.to_ascii_lowercase();
    status.contains("error")
        || status.contains("failed")
        || status.contains("unavailable")
        || status.contains("unsupported")
}

fn parse_root_midi_value(value: &str) -> Option<i32> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(note) = value.parse::<i32>() {
        return (0..=127).contains(&note).then_some(note);
    }
    parse_root_note_name(value)
}

fn parse_root_note_name(value: &str) -> Option<i32> {
    let mut chars = value.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let mut semitone = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (accidental, octave) = match rest.chars().next() {
        Some('#') => (1, &rest[1..]),
        Some('b' | 'B') => (-1, &rest[1..]),
        _ => (0, rest),
    };
    semitone += accidental;
    let octave = octave.parse::<i32>().ok()?;
    let note = (octave + 1) * 12 + semitone;
    (0..=127).contains(&note).then_some(note)
}

fn midi_note_name_for_root(note: i32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let semitone = note.rem_euclid(12) as usize;
    let octave = note.div_euclid(12) - 1;
    format!("{}{}", NAMES[semitone], octave)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EqualDivisionHint {
    divisions: u32,
    kind: EqualDivisionKind,
}

impl EqualDivisionHint {
    fn label(self) -> String {
        let suffix = match self.kind {
            EqualDivisionKind::Edo => "EDO",
            EqualDivisionKind::Tet => "TET",
        };
        format!("{}-{suffix}", self.divisions)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EqualDivisionKind {
    Edo,
    Tet,
}

fn equal_division_hint(text: &str) -> Option<EqualDivisionHint> {
    let normalized = text
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    for token in &tokens {
        if let Some(hint) = equal_division_hint_from_compact_token(token) {
            return Some(hint);
        }
    }
    for pair in tokens.windows(2) {
        let Ok(divisions) = pair[0].parse::<u32>() else {
            continue;
        };
        let kind = match pair[1] {
            "edo" => EqualDivisionKind::Edo,
            "tet" => EqualDivisionKind::Tet,
            _ => continue,
        };
        return Some(EqualDivisionHint { divisions, kind });
    }
    None
}

fn equal_division_hint_from_compact_token(token: &str) -> Option<EqualDivisionHint> {
    let (digits, kind) = token
        .strip_suffix("edo")
        .map(|digits| (digits, EqualDivisionKind::Edo))
        .or_else(|| {
            token
                .strip_suffix("tet")
                .map(|digits| (digits, EqualDivisionKind::Tet))
        })?;
    let divisions = digits.parse::<u32>().ok()?;
    Some(EqualDivisionHint { divisions, kind })
}

fn resolve_project_file_paths(mut project: ProjectFile, project_path: &Path) -> ProjectFile {
    let project_dir = project_reference_base_dir(project_path);
    project.scala_path = project
        .scala_path
        .map(|path| project_reference_path_for_load(path, project_dir));
    project.lumatone_path = project
        .lumatone_path
        .map(|path| project_reference_path_for_load(path, project_dir));
    project.sample_instrument_path = project
        .sample_instrument_path
        .map(|path| project_reference_path_for_load(path, project_dir));
    project
}

fn project_reference_base_dir(project_path: &Path) -> Option<&Path> {
    project_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn project_reference_path_for_load(path: PathBuf, project_dir: Option<&Path>) -> PathBuf {
    if path.is_absolute() {
        path
    } else if let Some(project_dir) = project_dir {
        project_dir.join(path)
    } else {
        path
    }
}

fn project_reference_path_for_save(path: PathBuf, project_dir: Option<&Path>) -> PathBuf {
    let Some(project_dir) = project_dir else {
        return path;
    };
    if let Ok(relative) = path.strip_prefix(project_dir)
        && !relative.as_os_str().is_empty()
    {
        return relative.to_path_buf();
    }
    let Ok(canonical_path) = std::fs::canonicalize(&path) else {
        return path;
    };
    let Ok(canonical_project_dir) = std::fs::canonicalize(project_dir) else {
        return canonical_path;
    };
    if let Ok(relative) = canonical_path.strip_prefix(&canonical_project_dir)
        && !relative.as_os_str().is_empty()
    {
        return relative.to_path_buf();
    }
    canonical_path
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
    project_backup_path_at(path, 1)
}

fn project_backup_path_at(path: &Path, slot: usize) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("project.orbifold");
    if slot <= 1 {
        path.with_file_name(format!("{name}.bak"))
    } else {
        path.with_file_name(format!("{name}.bak.{slot}"))
    }
}

fn rotate_project_backups(path: &Path) -> std::io::Result<()> {
    remove_project_save_file_if_exists(&project_backup_path_at(path, PROJECT_BACKUP_SLOTS))?;
    for slot in (1..PROJECT_BACKUP_SLOTS).rev() {
        let source = project_backup_path_at(path, slot);
        if !source.exists() {
            continue;
        }
        if !source.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::IsADirectory,
                format!("backup path is not a file: {}", source.display()),
            ));
        }
        let target = project_backup_path_at(path, slot + 1);
        remove_project_save_file_if_exists(&target)?;
        std::fs::rename(source, target)?;
    }
    Ok(())
}

fn remove_project_save_file_if_exists(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn clamp_piano_view_start(start: f32, loop_beats: f32, visible_beats: f32) -> f32 {
    start.clamp(0.0, (loop_beats - visible_beats).max(0.0))
}

fn list_start_for_selection(selected: usize, total: usize, visible_rows: usize) -> usize {
    if total <= visible_rows || visible_rows == 0 {
        return 0;
    }
    let selected = selected.min(total.saturating_sub(1));
    selected
        .saturating_sub(visible_rows - 1)
        .min(total - visible_rows)
}

fn clamp_list_start(start: usize, total: usize, visible_rows: usize) -> usize {
    if total <= visible_rows || visible_rows == 0 {
        return 0;
    }
    start.min(total - visible_rows)
}

fn clip_note_intersects_time_view(
    note: &ClipNote,
    view_start: f32,
    visible_beats: f32,
    loop_beats: f32,
) -> bool {
    let loop_beats = loop_beats.max(1.0);
    let view_start = view_start.clamp(0.0, loop_beats);
    let view_end = (view_start + visible_beats.max(1.0)).min(loop_beats);
    if view_end <= view_start {
        return false;
    }
    if note.duration_beats >= loop_beats {
        return true;
    }

    let start = note.start_beats.rem_euclid(loop_beats);
    let duration = note.duration_beats.clamp(0.0, loop_beats);
    let end = start + duration;
    let segments = if end <= loop_beats {
        [(start, end), (0.0, 0.0)]
    } else {
        [(start, loop_beats), (0.0, end - loop_beats)]
    };

    segments.iter().any(|(segment_start, segment_end)| {
        segment_end.min(view_end) > segment_start.max(view_start)
    })
}

#[cfg(all(not(test), feature = "native-app"))]
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

fn sort_audio_asset_items(assets: &mut [AudioAssetItem]) {
    assets.sort_by(|a, b| {
        a.kind
            .label()
            .cmp(b.kind.label())
            .then_with(|| a.name.cmp(&b.name))
    });
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

fn snap_beat_to_grid_line(beat: f32, step: f32, loop_beats: f32) -> f32 {
    let loop_beats = loop_beats.max(1.0);
    let step = step.max(f32::EPSILON);
    ((beat.rem_euclid(loop_beats) / step).round() * step).rem_euclid(loop_beats)
}

fn is_supported_audio_asset_file(kind: AudioAssetKind, path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    kind.extensions()
        .iter()
        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
}

#[cfg(any(test, feature = "web-app"))]
fn sanitize_browser_asset_file_name(file_name: &str) -> Option<String> {
    Path::new(file_name)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.starts_with('.'))
        .map(str::to_string)
}

fn sanitize_search_query(query: &str) -> String {
    query
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect::<String>()
}

fn scale_library_matches_search_query(item: &ScaleLibraryItem, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let source = if item.path.starts_with("scales") {
        "bundled"
    } else {
        "user"
    };
    let haystack = format!("{} {} {source}", item.name, item.path.display()).to_lowercase();
    query
        .split_whitespace()
        .map(str::to_lowercase)
        .all(|term| haystack.contains(&term))
}

fn audio_asset_matches_search_query(asset: &AudioAssetItem, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let haystack = format!(
        "{} {} {} {}",
        asset.name,
        asset.path.display(),
        asset.kind.label(),
        asset.kind.singular_label()
    )
    .to_lowercase();
    query
        .split_whitespace()
        .map(str::to_lowercase)
        .all(|term| haystack.contains(&term))
}

pub(crate) fn audio_asset_workflow_status(asset: &AudioAssetItem) -> &'static str {
    if asset.kind == AudioAssetKind::Sample && is_browser_audio_asset_path(&asset.path) {
        if is_wav_preview_file(asset) {
            return "browser WAV preview and project sample instrument available";
        }
        return asset.kind.sound_workflow_status();
    }
    if asset.kind == AudioAssetKind::Sample && !asset.path.exists() {
        return "missing file; preview unavailable";
    }
    if asset.kind == AudioAssetKind::Sample && is_wav_preview_file(asset) {
        return "WAV preview and project sample instrument available";
    }
    asset.kind.sound_workflow_status()
}

fn is_wav_preview_file(asset: &AudioAssetItem) -> bool {
    asset.kind == AudioAssetKind::Sample && !asset.is_dir && is_wav_file_path(&asset.path)
}

fn is_wav_file_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
}

fn is_browser_audio_asset_path(path: &Path) -> bool {
    path.starts_with("browser_assets")
}

fn sample_instrument_display_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("sample")
        .to_string()
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

#[cfg(any(test, feature = "web-app"))]
fn unique_browser_asset_path(
    assets: &[AudioAssetItem],
    kind: AudioAssetKind,
    file_name: &str,
) -> PathBuf {
    let root = Path::new("browser_assets").join(kind.folder());
    let source = Path::new(file_name);
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let extension = source.extension().and_then(|value| value.to_str());
    for index in 1.. {
        let candidate_name = if index == 1 {
            file_name.to_string()
        } else {
            match extension {
                Some(extension) => format!("{stem}_{index}.{extension}"),
                None => format!("{stem}_{index}"),
            }
        };
        let candidate = root.join(candidate_name);
        if !assets.iter().any(|asset| asset.path == candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded browser asset filename search should always return")
}

fn asset_import_success_status(
    kind: AudioAssetKind,
    source_file_name: &str,
    target: &Path,
) -> String {
    let target_file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(source_file_name);
    if target_file_name == source_file_name {
        format!("Imported {}: {source_file_name}", kind.singular_label())
    } else {
        format!(
            "Imported {} as {target_file_name} ({source_file_name} already exists)",
            kind.singular_label()
        )
    }
}

fn missing_recent_removed_status(count: usize) -> String {
    if count == 1 {
        "Removed 1 missing recent project".to_string()
    } else {
        format!("Removed {count} missing recent projects")
    }
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
        let name = path_display_name(&path);
        presets.push(LumatonePreset { name, path });
    }
}

fn midi_input_name_uses_lumatone_map(name: &str) -> bool {
    name.to_ascii_lowercase().contains("lumatone")
}

#[cfg(any(test, feature = "web-app"))]
fn normalized_browser_project_file_name(file_name: &str) -> String {
    let trimmed = file_name.trim();
    if trimmed.ends_with(".orbifold") {
        trimmed.to_string()
    } else if trimmed.ends_with(".mtdaw") {
        trimmed.replace(".mtdaw", ".orbifold")
    } else {
        format!("{trimmed}.orbifold")
    }
}

fn path_display_name(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

fn project_title_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Untitled")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ephemeral_state_applies_loaded_settings() {
        let settings = AppSettings {
            root_midi: 64,
            base_freq: 330.0,
            show_asset_browser: false,
            show_scale_browser: true,
            show_clip_panel: false,
            midi_debug: true,
            midi_channel_filter: Some(3),
            waveform: crate::synth::Waveform::Square,
            master_gain: 0.42,
            ..AppSettings::default()
        };

        let app = AppState::for_ephemeral_state_with_settings(
            PathBuf::from("orbifold_ephemeral_settings_test.txt"),
            settings,
        );
        let scale = app.scale_state.lock();

        assert_eq!(scale.root_midi, 64);
        assert_eq!(scale.base_freq, 330.0);
        drop(scale);
        assert!(!app.show_asset_browser);
        assert!(app.show_scale_browser);
        assert!(!app.show_clip_panel);
        assert!(app.midi_debug.load(Ordering::Relaxed));
        assert_eq!(app.midi_channel_filter.load(Ordering::Relaxed), 3);
        assert_eq!(
            app.synth.settings().waveform,
            crate::synth::Waveform::Square
        );
        assert_eq!(app.synth.settings().master_gain, 0.42);
    }

    #[test]
    fn diagnostic_history_records_error_statuses_and_keeps_recent_entries() {
        let mut app = AppState::for_layout_tests();

        app.set_error_status("Project save error: no space");
        app.set_error_status("Project save error: no space");
        for idx in 0..10 {
            app.set_error_status(format!("Audio output error: {idx}"));
        }

        assert_eq!(app.diagnostic_messages().len(), MAX_DIAGNOSTIC_MESSAGES);
        assert_eq!(
            app.diagnostic_messages().first().map(String::as_str),
            Some("Audio output error: 2")
        );
        assert_eq!(
            app.diagnostic_messages().last().map(String::as_str),
            Some("Audio output error: 9")
        );
    }

    #[test]
    fn clearing_diagnostics_removes_history_without_recording_another_error() {
        let mut app = AppState::for_layout_tests();
        app.set_error_status("Settings save error: read-only");

        app.clear_diagnostics();

        assert!(app.diagnostic_messages().is_empty());
        assert_eq!(app.last_status, "Diagnostics cleared");
    }

    #[test]
    fn failed_midi_connection_cleanup_clears_stale_live_state() {
        let mut app = AppState::for_layout_tests();
        app.connected_midi_input = "Keyboard".to_string();
        app.synth.note_on(60, 261.63, 1.0).unwrap();
        app.midi_sustain.lock().press(0);
        let note = test_midi_event(0x90, 60, 96, 60, 60, Some(261.63));
        app.midi_held
            .lock()
            .insert((note.key_index, note.channel, note.midi_note), note);

        app.clear_midi_connection_state("test failure");

        assert_eq!(app.connected_midi_input, "");
        assert!(app.midi_held.lock().is_empty());
        assert!(!app.midi_sustain.lock().defer_note_off(0, 60));
        assert!(app.synth.active_notes().is_empty());
    }

    #[test]
    fn empty_recording_pass_does_not_dirty_project_or_enable_undo() {
        let mut app = AppState::for_layout_tests();

        app.start_recording();

        assert!(app.music_project.lock().transport.recording);
        assert_eq!(app.last_status, "Recording: Replace, quantized 1/16");
        assert!(!app.project_dirty);
        assert!(!app.can_undo_project_edit());

        app.stop_recording();

        assert!(!app.music_project.lock().transport.recording);
        assert!(app.music_project.lock().clip.notes.is_empty());
        assert!(!app.project_dirty);
        assert!(!app.can_undo_project_edit());
        assert_eq!(app.last_status, "Recording stopped: no notes");
    }

    #[test]
    fn recording_start_status_names_mode_and_timing_policy() {
        let mut app = AppState::for_layout_tests();
        {
            let mut project = app.music_project.lock();
            project.transport.overdub = true;
            project.transport.quantize_on_record = false;
        }

        app.start_recording();

        assert_eq!(app.last_status, "Recording: Overdub, free timing");
    }

    #[test]
    fn recorded_notes_create_one_undo_step_when_recording_stops() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        let freq = app.scale_state.lock().note_info(root).unwrap().freq;

        app.start_recording();
        app.music_project.lock().add_note(1.0, 1.0, root, 96, freq);
        assert!(!app.project_dirty);
        assert!(!app.can_undo_project_edit());

        app.stop_recording();

        assert!(app.project_dirty);
        assert!(app.can_undo_project_edit());
        assert_eq!(app.music_project.lock().clip.notes.len(), 1);
        assert_eq!(app.last_status, "Recording stopped: 1 note");

        app.undo_project_edit();

        assert!(app.music_project.lock().clip.notes.is_empty());
        assert!(!app.project_dirty);
        assert!(!app.can_undo_project_edit());
    }

    #[test]
    fn stop_transport_commits_active_recording_edit() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        let freq = app.scale_state.lock().note_info(root).unwrap().freq;

        app.start_recording();
        app.music_project.lock().add_note(1.0, 1.0, root, 96, freq);

        app.stop_transport();

        assert!(!app.music_project.lock().transport.recording);
        assert!(app.project_dirty);
        assert!(app.can_undo_project_edit());
        assert_eq!(app.last_status, "Transport stopped; recorded 1 note");
    }

    #[test]
    fn seek_commits_active_recording_edit() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        let freq = app.scale_state.lock().note_info(root).unwrap().freq;

        app.start_recording();
        app.music_project.lock().add_note(1.0, 1.0, root, 96, freq);

        app.seek_transport_to(4.0);

        assert!(!app.music_project.lock().transport.recording);
        assert!(app.project_dirty);
        assert!(app.can_undo_project_edit());
        assert_eq!(app.last_status, "Seek 4.00; recorded 1 note");

        app.undo_project_edit();

        assert!(app.music_project.lock().clip.notes.is_empty());
        assert!(!app.project_dirty);
    }

    #[test]
    fn return_to_start_commits_active_recording_edit() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        let freq = app.scale_state.lock().note_info(root).unwrap().freq;

        app.start_recording();
        app.music_project.lock().add_note(1.0, 1.0, root, 96, freq);

        app.return_transport_to_start();

        assert!(!app.music_project.lock().transport.recording);
        assert!(app.project_dirty);
        assert!(app.can_undo_project_edit());
        assert_eq!(app.last_status, "Returned to start; recorded 1 note");
    }

    #[test]
    fn empty_recording_seek_does_not_dirty_project_or_enable_undo() {
        let mut app = AppState::for_layout_tests();

        app.start_recording();
        app.seek_transport_to(4.0);

        assert!(!app.music_project.lock().transport.recording);
        assert!(!app.project_dirty);
        assert!(!app.can_undo_project_edit());
        assert_eq!(app.last_status, "Seek 4.00; recorded no notes");
    }

    #[test]
    fn empty_replace_recording_over_existing_clip_is_undoable() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        let freq = app.scale_state.lock().note_info(root).unwrap().freq;
        let note_id = app.music_project.lock().add_note(1.0, 1.0, root, 96, freq);
        app.selected_clip_note = Some(note_id);
        app.establish_clean_project_snapshot();

        app.start_recording();
        assert!(app.music_project.lock().clip.notes.is_empty());
        app.stop_recording();

        assert!(app.project_dirty);
        assert!(app.can_undo_project_edit());
        assert_eq!(app.selected_clip_note, None);
        assert_eq!(app.last_status, "Recording stopped: no notes");

        app.undo_project_edit();

        assert_eq!(app.music_project.lock().clip.notes.len(), 1);
        assert_eq!(app.selected_clip_note, Some(note_id));
        assert!(!app.project_dirty);
    }

    #[test]
    fn overdub_recording_status_counts_only_new_notes() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        let freq = app.scale_state.lock().note_info(root).unwrap().freq;
        {
            let mut project = app.music_project.lock();
            project.add_note(1.0, 1.0, root, 96, freq);
            project.transport.overdub = true;
        }
        app.establish_clean_project_snapshot();

        app.start_recording();
        app.music_project.lock().add_note(2.0, 1.0, root, 96, freq);
        app.stop_recording();

        assert_eq!(app.music_project.lock().clip.notes.len(), 2);
        assert_eq!(app.last_status, "Recording stopped: 1 note");

        app.undo_project_edit();

        assert_eq!(app.music_project.lock().clip.notes.len(), 1);
        assert!(!app.project_dirty);
    }

    #[test]
    fn recording_stop_status_preserves_autosave_error() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_autosave_recording_failure_test_{}",
            std::process::id()
        ));
        let settings_path = dir.join("settings.txt");
        let autosave_path = dir.join("orbifold_autosave.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&autosave_path)
            .expect("autosave directory should block autosave writes");
        let mut app = AppState::for_layout_tests();
        app.set_settings_path_for_tests(settings_path, true);
        app.set_autosave_path_for_tests(autosave_path.clone());
        let root = app.scale_state.lock().root_midi;
        let freq = app.scale_state.lock().note_info(root).unwrap().freq;

        app.start_recording();
        app.music_project.lock().add_note(1.0, 1.0, root, 96, freq);
        app.stop_recording();

        assert!(app.project_dirty);
        assert!(!app.autosave_available);
        assert!(app.last_status.starts_with("Project autosave error ("));
        assert!(app.last_status.contains("Recording stopped: 1 note"));
        assert!(
            app.last_status
                .contains(&autosave_path.display().to_string())
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn recording_seeds_midi_notes_held_before_recording_starts() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        let freq = app.scale_state.lock().note_info(root).unwrap().freq;
        let note_on = test_midi_event(0x90, root as u8, 96, root, root, Some(freq));
        app.midi_held.lock().insert(
            (note_on.key_index, note_on.channel, note_on.midi_note),
            note_on,
        );

        app.start_recording();
        let note_off = test_midi_event(0x80, root as u8, 0, root, root, Some(freq));
        app.music_project.lock().record_midi_event(&note_off);
        app.stop_recording();

        let notes = app.music_project.lock().clip.notes.clone();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].musical_note, root);
        assert_eq!(notes[0].velocity, 96);
        assert_eq!(app.last_status, "Recording stopped: 1 note");
    }

    #[test]
    fn recording_seeds_sustain_state_for_notes_held_before_recording() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        let freq = app.scale_state.lock().note_info(root).unwrap().freq;
        let note_on = test_midi_event(0x90, root as u8, 96, root, root, Some(freq));
        app.midi_held.lock().insert(
            (note_on.key_index, note_on.channel, note_on.midi_note),
            note_on,
        );
        app.midi_sustain.lock().press(0);

        app.start_recording();
        let note_off = test_midi_event(0x80, root as u8, 0, root, root, Some(freq));
        app.music_project.lock().record_midi_event(&note_off);
        assert!(app.music_project.lock().clip.notes.is_empty());

        let sustain_off = test_midi_event(0xB0, 64, 0, -1, root, None);
        app.music_project.lock().record_midi_event(&sustain_off);
        app.stop_recording();

        assert_eq!(app.music_project.lock().clip.notes.len(), 1);
        assert_eq!(app.last_status, "Recording stopped: 1 note");
    }

    fn test_midi_event(
        raw_status: u8,
        midi_note: u8,
        velocity: u8,
        key_index: i32,
        musical_note: i32,
        freq: Option<f32>,
    ) -> crate::midi::MidiEvent {
        crate::midi::MidiEvent {
            raw_status,
            status: raw_status & 0xF0,
            channel: raw_status & 0x0F,
            midi_note,
            velocity,
            key_index,
            musical_note,
            mapped_from_lumatone: false,
            freq,
            scale_degree: None,
            scale_octave: None,
            cents_from_root: None,
            at: AppInstant::now() + std::time::Duration::from_millis(500),
        }
    }

    #[test]
    fn midi_input_selection_matches_the_visible_selected_name() {
        assert_eq!(
            select_midi_input_candidate_index(&["Keyboard", "Lumatone"], 0, Some("Lumatone")),
            Ok(1)
        );
    }

    #[test]
    fn midi_input_selection_without_a_selected_name_uses_first_readable_port() {
        assert_eq!(
            select_midi_input_candidate_index(&["Keyboard", "Lumatone"], 0, None),
            Ok(0)
        );
        assert_eq!(
            select_midi_input_candidate_index(&["Keyboard"], 0, Some("")),
            Ok(0)
        );
    }

    #[test]
    fn midi_input_selection_refuses_to_connect_a_different_named_port() {
        assert_eq!(
            select_midi_input_candidate_index(&["Keyboard"], 0, Some("Lumatone")),
            Err("Selected MIDI input unavailable: Lumatone".to_string())
        );
    }

    #[test]
    fn midi_input_selection_reports_unreadable_ports_without_fabricated_names() {
        assert_eq!(
            select_midi_input_candidate_index(&[], 2, None),
            Err("No readable MIDI inputs: skipped 2 unreadable ports".to_string())
        );
        assert_eq!(
            select_midi_input_candidate_index(&["Keyboard"], 1, Some("Lumatone")),
            Err("Selected MIDI input unavailable: Lumatone; skipped 1 unreadable port".to_string())
        );
    }

    #[test]
    fn audio_connection_status_includes_stream_details() {
        let info = AudioStreamInfo {
            sample_rate_hz: 48_000,
            channels: 2,
            sample_format: "F32".to_string(),
            buffer_frames: Some(256),
        };

        assert_eq!(
            audio_connection_status("USB Interface", &info),
            "Connected audio output: USB Interface 48 kHz 2ch F32 256f 5.3ms"
        );
    }

    #[test]
    fn startup_status_seeds_visible_diagnostics_from_failures() {
        let diagnostics = initial_diagnostic_messages(Some(
            "Settings load error: bad key; Device setup required: audio unavailable; Ready",
        ));

        assert_eq!(
            diagnostics,
            vec![
                "Settings load error: bad key".to_string(),
                "Device setup required: audio unavailable".to_string()
            ]
        );
    }

    #[test]
    fn path_display_name_prefers_filename_and_falls_back_to_path() {
        assert_eq!(
            path_display_name(Path::new("scales/31-edo.scl")),
            "31-edo.scl"
        );
        assert_eq!(path_display_name(Path::new("/")), "/");
    }

    #[test]
    fn equal_division_hint_reads_edo_and_tet_labels() {
        assert_eq!(
            equal_division_hint("31-EDO (Equal Temperament)"),
            Some(EqualDivisionHint {
                divisions: 31,
                kind: EqualDivisionKind::Edo
            })
        );
        assert_eq!(
            equal_division_hint("12TET"),
            Some(EqualDivisionHint {
                divisions: 12,
                kind: EqualDivisionKind::Tet
            })
        );
        assert_eq!(
            equal_division_hint("8. 31 EDO.ltn"),
            Some(EqualDivisionHint {
                divisions: 31,
                kind: EqualDivisionKind::Edo
            })
        );
        assert_eq!(equal_division_hint("Classic Mode"), None);
    }

    #[test]
    fn lumatone_keymap_is_only_active_for_lumatone_named_inputs() {
        assert!(midi_input_name_uses_lumatone_map(
            "Lumatone Isomorphic Keyboard"
        ));
        assert!(midi_input_name_uses_lumatone_map(
            "LUMATONE Isomorphic Keyboard Long Virtual MIDI Port"
        ));
        assert!(!midi_input_name_uses_lumatone_map("USB MIDI Keyboard"));
        assert!(!midi_input_name_uses_lumatone_map(
            "Midi Through:Midi Through Port-0 14:0"
        ));
    }

    #[test]
    fn regular_midi_inputs_do_not_receive_loaded_lumatone_map() {
        let mut app = AppState::for_layout_tests();
        assert!(app.load_lumatone_path(PathBuf::from(
            "lumatone_factory_presets/1. Classic Mode.ltn"
        )));

        let regular_map = app.lumatone_map_for_input_name("USB MIDI Keyboard");
        assert!(regular_map.lock().is_none());

        let lumatone_map = app.lumatone_map_for_input_name("Lumatone Isomorphic Keyboard");
        assert!(lumatone_map.lock().is_some());
    }

    #[test]
    fn regular_midi_keyboard_keeps_chromatic_chord_when_keymap_is_loaded() {
        let mut app = AppState::for_layout_tests();
        app.load_scale_path(PathBuf::from("scales/31-edo.scl"), true)
            .expect("bundled 31-EDO scale should load");
        assert!(app.load_lumatone_path(PathBuf::from("lumatone_factory_presets/8. 31 EDO.ltn")));
        let midi_log = Arc::new(Mutex::new(Vec::new()));
        let midi_state = MidiSharedState {
            last: Arc::new(Mutex::new(None)),
            log: midi_log.clone(),
            capture: Arc::new(Mutex::new(Default::default())),
            held: Arc::new(Mutex::new(HashMap::new())),
            sustain: Arc::new(Mutex::new(Default::default())),
            channel_filter: Arc::new(std::sync::atomic::AtomicI8::new(MIDI_CHANNEL_FILTER_ALL)),
            lumatone_map: app.lumatone_map_for_input_name("USB MIDI Keyboard"),
            music_project: Arc::new(Mutex::new(MusicProject::default())),
        };

        for midi_note in [72, 76, 79] {
            handle_midi(
                &[0x90, midi_note, 100],
                &app.scale_state,
                &app.synth,
                &midi_state,
                false,
            );
        }

        let events = midi_log.lock().clone();
        let mapped_notes = events
            .iter()
            .map(|event| event.musical_note)
            .collect::<Vec<_>>();
        assert_eq!(mapped_notes, vec![77, 87, 95]);
        assert!((events[0].freq.expect("C5 should resolve") - 523.2511).abs() < 4.0);
        assert!((events[1].freq.expect("E5 should resolve") - 659.2551).abs() < 4.0);
        assert!((events[2].freq.expect("G5 should resolve") - 783.9908).abs() < 4.0);
    }

    #[test]
    fn selected_midi_input_reports_lumatone_keymap_activity() {
        let mut app = AppState::for_layout_tests();
        app.midi_inputs = vec![
            "USB MIDI Keyboard".to_string(),
            "Lumatone Isomorphic Keyboard".to_string(),
        ];
        app.selected_input = 0;
        assert!(!app.selected_midi_input_uses_lumatone_map());

        app.selected_input = 1;
        assert!(app.selected_midi_input_uses_lumatone_map());

        app.connected_midi_input = "USB MIDI Keyboard".to_string();
        assert!(!app.selected_midi_input_uses_lumatone_map());

        app.connected_midi_input = "Lumatone Isomorphic Keyboard".to_string();
        assert!(app.selected_midi_input_uses_lumatone_map());
    }

    #[test]
    fn loading_mismatched_edo_keymap_reports_warning() {
        let mut app = AppState::for_layout_tests();

        assert!(app.load_lumatone_path(PathBuf::from("lumatone_factory_presets/8. 31 EDO.ltn")));

        assert_eq!(
            app.keymap_scale_mismatch_warning().as_deref(),
            Some("Scale/key map mismatch: scale 12-TET, key map 31-EDO")
        );
        assert!(
            app.last_status
                .contains("Scale/key map mismatch: scale 12-TET, key map 31-EDO")
        );
    }

    #[test]
    fn matching_edo_scale_and_keymap_do_not_warn() {
        let mut app = AppState::for_layout_tests();
        assert!(app.load_lumatone_path(PathBuf::from("lumatone_factory_presets/8. 31 EDO.ltn")));

        app.load_scale_path(PathBuf::from("scales/31-edo.scl"), true)
            .expect("bundled 31-EDO scale should load");

        assert_eq!(app.keymap_scale_mismatch_warning(), None);
        assert_eq!(app.last_status, "Loaded Scala file");
    }

    #[test]
    fn loading_scale_retunes_existing_clip_notes() {
        let mut app = AppState::for_layout_tests();
        let note = app.scale_state.lock().root_midi + 12;
        app.add_clip_note_at(1.0, note);
        let note_id = app
            .selected_clip_note
            .expect("added note should be selected");
        let original_freq = app
            .music_project
            .lock()
            .note_by_id(note_id)
            .expect("note should exist")
            .freq;

        assert!(
            app.load_scale_path(PathBuf::from("scales/31-edo.scl"), true)
                .expect("bundled 31-EDO scale should load")
        );

        let retuned_freq = app
            .music_project
            .lock()
            .note_by_id(note_id)
            .expect("note should exist")
            .freq;
        let expected_freq = app.scale_state.lock().note_info(note).unwrap().freq;
        assert!((retuned_freq - expected_freq).abs() < f32::EPSILON);
        assert!((retuned_freq - original_freq).abs() > 1.0);
        assert_eq!(app.last_status, "Loaded Scala file; retuned 1 note");
    }

    #[test]
    fn retuning_current_scale_updates_sounding_playback_and_midi_notes() {
        let mut app = AppState::for_layout_tests();
        let (_engine, receiver, sender) = app.synth.make_engine(44_100.0);
        app.synth.install_sender(sender);
        let root = app.scale_state.lock().root_midi;
        app.add_clip_note_at(1.0, root + 12);
        let clip_note = app.selected_clip_note().expect("added note should exist");
        let playback_synth_note = playback_note_id(clip_note.id);
        app.playback_active_notes
            .insert(clip_note.id, playback_synth_note);
        app.synth
            .note_on(playback_synth_note, clip_note.freq, 1.0)
            .expect("test sender should accept playback note");
        let midi_freq = app.scale_state.lock().note_info(root).unwrap().freq;
        app.midi_held.lock().insert(
            (root, 0, root as u8),
            test_midi_event(0x90, root as u8, 96, root, root, Some(midi_freq)),
        );
        app.synth
            .note_on(root as u32, midi_freq, 1.0)
            .expect("test sender should accept midi note");
        let _ = receiver.try_iter().count();

        app.scale_state.lock().base_freq += 1.0;
        assert_eq!(app.retune_clip_notes_to_current_scale(), 1);
        assert_eq!(app.retune_sounding_notes_to_current_scale(), 2);

        let commands = receiver.try_iter().collect::<Vec<_>>();
        let expected_clip_freq = app
            .scale_state
            .lock()
            .note_info(clip_note.musical_note)
            .unwrap()
            .freq;
        let expected_midi_freq = app.scale_state.lock().note_info(root).unwrap().freq;
        assert!(commands.iter().any(|command| {
            matches!(
                command,
                crate::synth::AudioCommand::RetuneNote { note, freq }
                    if *note == playback_synth_note
                        && (*freq - expected_clip_freq).abs() < f32::EPSILON
            )
        }));
        assert!(commands.iter().any(|command| {
            matches!(
                command,
                crate::synth::AudioCommand::RetuneNote { note, freq }
                    if *note == root as u32 && (*freq - expected_midi_freq).abs() < f32::EPSILON
            )
        }));
        assert_eq!(
            app.midi_held
                .lock()
                .values()
                .next()
                .expect("held MIDI note should remain tracked")
                .freq,
            Some(expected_midi_freq)
        );
    }

    #[test]
    fn explicit_pitch_audition_reports_missing_audio_output() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;

        app.audition_piano_pitch(root);

        assert_eq!(
            app.last_status,
            "Audition unavailable: no audio output connected"
        );
    }

    #[test]
    fn keymap_refresh_keeps_mismatch_warning_visible() {
        let mut app = AppState::for_layout_tests();
        assert!(app.load_lumatone_path(PathBuf::from("lumatone_factory_presets/8. 31 EDO.ltn")));

        app.reload_lumatone_presets();

        assert!(app.last_status.starts_with("Refreshed key map presets:"));
        assert!(
            app.last_status
                .contains("Scale/key map mismatch: scale 12-TET, key map 31-EDO")
        );
    }

    #[test]
    fn project_load_keeps_mismatch_warning_visible() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_project_mismatch_warning_test_{}",
            std::process::id()
        ));
        let project_path = dir.join("project.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("project mismatch test directory");
        let mut source = AppState::for_layout_tests();
        assert!(source.load_lumatone_path(PathBuf::from("lumatone_factory_presets/8. 31 EDO.ltn")));
        source.save_project_to_path(project_path.clone());
        let mut app = AppState::for_layout_tests();

        app.load_project_path(project_path.clone());

        assert!(app.last_status.starts_with("Loaded project:"));
        assert!(
            app.last_status
                .contains("Scale/key map mismatch: scale 12-TET, key map 31-EDO")
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn browser_project_download_payload_uses_project_format_without_marking_clean() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        app.add_clip_note_at(0.0, root);

        let (file_name, text) = app.browser_project_download_payload();

        assert_eq!(file_name, "project.orbifold");
        let parsed = ProjectFile::from_text(&text).expect("download should be valid project text");
        assert_eq!(parsed.project.clip.notes.len(), 1);
        assert!(app.project_path.is_none());
        assert!(app.project_dirty);

        app.mark_browser_project_downloaded(&file_name);

        assert_eq!(
            app.project_path.as_ref(),
            Some(&PathBuf::from("project.orbifold"))
        );
        assert!(!app.project_dirty);
        assert_eq!(
            app.last_status,
            "Downloaded browser project: project.orbifold"
        );
    }

    #[test]
    fn browser_project_load_uses_project_parser_and_existing_apply_path() {
        let mut source = AppState::for_layout_tests();
        let root = source.scale_state.lock().root_midi;
        source.add_clip_note_at(1.0, root + 7);
        let (_, text) = source.browser_project_download_payload();
        let mut app = AppState::for_layout_tests();

        assert!(app.load_browser_project_text(&text, "legacy.mtdaw"));

        assert_eq!(app.music_project.lock().clip.notes.len(), 1);
        assert_eq!(
            app.project_path.as_ref(),
            Some(&PathBuf::from("legacy.mtdaw"))
        );
        assert!(!app.project_dirty);
        assert_eq!(app.last_status, "Loaded browser project: legacy.mtdaw");

        let (file_name, _) = app.browser_project_download_payload();
        assert_eq!(file_name, "legacy.orbifold");
    }

    #[test]
    fn browser_project_load_success_preserves_existing_error_status() {
        let mut source = AppState::for_layout_tests();
        let root = source.scale_state.lock().root_midi;
        source.add_clip_note_at(1.0, root + 7);
        let (_, text) = source.browser_project_download_payload();
        let mut app = AppState::for_layout_tests();
        app.set_error_status("Browser asset storage load error: quota unavailable");

        assert!(app.load_browser_project_text(&text, "session.orbifold"));

        assert!(
            app.last_status
                .starts_with("Browser asset storage load error: quota unavailable;"),
            "unexpected status: {}",
            app.last_status
        );
        assert!(
            app.last_status
                .contains("Loaded browser project: session.orbifold")
        );
    }

    #[test]
    fn browser_project_load_restores_browser_scale_and_keymap_resources() {
        let mut source = AppState::for_layout_tests();
        assert!(
            source.load_browser_scale_text(
                "Browser 5-EDO\n5\n240\n480\n720\n960\n2/1\n",
                "browser.scl",
            )
        );
        assert!(source.load_browser_lumatone_text(
            include_str!("../lumatone_factory_presets/1. Classic Mode.ltn"),
            "classic.ltn",
        ));
        let root = source.scale_state.lock().root_midi;
        source.add_clip_note_at(1.0, root + 7);
        let (_, text) = source.browser_project_download_payload();
        let mut app = AppState::for_layout_tests();

        assert!(app.load_browser_project_text_with_resources(
            &text,
            "session.orbifold",
            Some((
                PathBuf::from("browser.scl"),
                "Browser 5-EDO\n5\n240\n480\n720\n960\n2/1\n".to_string(),
            )),
            Some((
                PathBuf::from("classic.ltn"),
                include_str!("../lumatone_factory_presets/1. Classic Mode.ltn").to_string(),
            )),
        ));

        assert_eq!(app.scale_state.lock().scale.description, "Browser 5-EDO");
        assert_eq!(app.scala_path.as_ref(), Some(&PathBuf::from("browser.scl")));
        assert!(app.lumatone_map.lock().is_some());
        assert_eq!(
            app.lumatone_path.as_ref(),
            Some(&PathBuf::from("classic.ltn"))
        );
        assert_eq!(app.music_project.lock().clip.notes.len(), 1);
        assert!(app.last_status.contains("Loaded browser project"));
    }

    #[test]
    fn invalid_browser_project_load_preserves_current_project_and_error_status() {
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        app.add_clip_note_at(1.0, root + 7);
        let existing_note_count = app.music_project.lock().clip.notes.len();

        assert!(!app.load_browser_project_text("not an orbifold project", "bad.orbifold"));

        assert_eq!(
            app.music_project.lock().clip.notes.len(),
            existing_note_count
        );
        assert!(app.project_path.is_none());
        assert!(app.project_dirty);
        assert!(
            app.last_status
                .starts_with("Project parse error (bad.orbifold):"),
            "unexpected status: {}",
            app.last_status
        );
    }

    #[test]
    fn browser_scale_load_uses_scala_parser_and_marks_project_dirty() {
        let mut app = AppState::for_layout_tests();
        let data = "Browser 5-EDO\n5\n240\n480\n720\n960\n2/1\n";

        assert!(app.load_browser_scale_text(data, "browser.scl"));

        assert_eq!(app.scale_state.lock().scale.description, "Browser 5-EDO");
        assert_eq!(app.scale_state.lock().scale.steps.len(), 5);
        assert_eq!(app.scala_path.as_ref(), Some(&PathBuf::from("browser.scl")));
        assert!(app.project_dirty);
        assert_eq!(app.last_status, "Loaded browser Scala file");
    }

    #[test]
    fn invalid_browser_scale_load_preserves_current_scale_and_error_status() {
        let mut app = AppState::for_layout_tests();
        let original_description = app.scale_state.lock().scale.description.clone();

        assert!(!app.load_browser_scale_text("not a scala file", "bad.scl"));

        assert_eq!(
            app.scale_state.lock().scale.description,
            original_description
        );
        assert!(app.scala_path.is_none());
        assert!(!app.project_dirty);
        assert!(
            app.last_status.starts_with("Scala parse error (bad.scl):"),
            "unexpected status: {}",
            app.last_status
        );
    }

    #[test]
    fn browser_keymap_load_uses_lumatone_parser_and_marks_project_dirty() {
        let mut app = AppState::for_layout_tests();

        assert!(app.load_browser_lumatone_text(
            include_str!("../lumatone_factory_presets/1. Classic Mode.ltn"),
            "classic.ltn",
        ));

        assert!(app.lumatone_map.lock().is_some());
        assert_eq!(
            app.lumatone_path.as_ref(),
            Some(&PathBuf::from("classic.ltn"))
        );
        assert!(app.project_dirty);
        assert!(
            app.last_status
                .starts_with("Loaded browser key map: classic.ltn")
        );
    }

    #[test]
    fn invalid_browser_keymap_load_preserves_current_keymap_and_error_status() {
        let mut app = AppState::for_layout_tests();

        assert!(!app.load_browser_lumatone_text("not a key map", "bad.ltn"));

        assert!(app.lumatone_map.lock().is_none());
        assert!(app.lumatone_path.is_none());
        assert!(!app.project_dirty);
        assert!(
            app.last_status
                .starts_with("Key map parse error (bad.ltn):"),
            "unexpected status: {}",
            app.last_status
        );
    }

    #[test]
    fn browser_midi_inputs_and_messages_use_shared_midi_path() {
        let mut app = AppState::for_layout_tests();
        app.apply_browser_midi_inputs(vec!["Browser Keyboard".to_string()]);
        app.connect_browser_midi_input("Browser Keyboard".to_string());

        app.handle_browser_midi_message(&[0x90, 60, 100]);

        let event = app
            .midi_last
            .lock()
            .clone()
            .expect("browser MIDI event should be recorded");
        assert_eq!(event.raw_status, 0x90);
        assert_eq!(event.midi_note, 60);
        assert_eq!(app.connected_midi_input, "Browser Keyboard");
        assert_eq!(app.midi_inputs, vec!["Browser Keyboard"]);
    }

    #[test]
    fn browser_wav_asset_import_keeps_preview_buffer_in_memory() {
        let mut app = AppState::for_layout_tests();
        let samples = vec![16_384_i16; 2048];
        let wav = pcm16_wav_bytes(1, 48_000, &samples);

        let imported_path =
            app.import_browser_audio_asset("soft_pluck.wav", &wav, AudioAssetKind::Sample);

        let asset = app
            .selected_audio_asset_item()
            .expect("browser asset should be selected");
        assert_eq!(imported_path.as_deref(), Some(asset.path.as_path()));
        assert_eq!(asset.name, "soft_pluck.wav");
        assert!(app.can_preview_selected_audio_asset());
        assert_eq!(app.last_status, "Imported browser sample: soft_pluck.wav");

        app.load_selected_sample_instrument();

        assert_eq!(
            app.sample_instrument_assignment
                .as_ref()
                .map(|assignment| assignment.name.as_str()),
            Some("soft_pluck.wav")
        );
        let (mut engine, _, _) = app.synth.make_engine(48_000.0);
        engine.handle_command(crate::synth::AudioCommand::NoteOn {
            note: 60,
            freq: 261.63,
            velocity: 1.0,
        });
        assert!((0..512).any(|_| engine.next_sample() > 0.1));
    }

    #[test]
    fn restored_browser_wav_asset_can_reload_saved_sample_instrument() {
        let mut source = AppState::for_layout_tests();
        let samples = vec![16_384_i16; 2048];
        let wav = pcm16_wav_bytes(1, 48_000, &samples);
        let path = source
            .import_browser_audio_asset("soft_pluck.wav", &wav, AudioAssetKind::Sample)
            .expect("browser asset import should return a path");
        source.load_selected_sample_instrument();
        let project = source.project_file_snapshot_for_path(None);

        let mut restored = AppState::for_layout_tests();
        assert!(
            restored
                .restore_browser_audio_asset(
                    path.clone(),
                    "soft_pluck.wav",
                    &wav,
                    AudioAssetKind::Sample,
                    false,
                )
                .is_some()
        );
        let warnings = restored
            .apply_project_file(project)
            .expect("restored browser asset should satisfy project reference");

        assert!(warnings.is_empty());
        assert_eq!(
            restored.sample_instrument_assignment,
            Some(SampleInstrumentAssignment {
                name: "soft_pluck.wav".to_string(),
                path,
            })
        );
        let (mut engine, _, _) = restored.synth.make_engine(48_000.0);
        engine.handle_command(crate::synth::AudioCommand::NoteOn {
            note: 60,
            freq: 261.63,
            velocity: 1.0,
        });
        assert!((0..512).any(|_| engine.next_sample() > 0.1));
    }

    #[test]
    fn autosave_recovery_keeps_mismatch_warning_visible() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_autosave_mismatch_warning_test_{}",
            std::process::id()
        ));
        let autosave_path = dir.join("orbifold_autosave.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("autosave mismatch test directory");
        let mut source = AppState::for_layout_tests();
        assert!(source.load_lumatone_path(PathBuf::from("lumatone_factory_presets/8. 31 EDO.ltn")));
        std::fs::write(&autosave_path, source.project_file_snapshot().to_text())
            .expect("autosave fixture should be written");
        let mut app = AppState::for_layout_tests();
        app.set_autosave_path_for_tests(autosave_path.clone());

        app.recover_autosave_project();

        assert!(app.last_status.starts_with("Recovered autosave:"));
        assert!(
            app.last_status
                .contains("Scale/key map mismatch: scale 12-TET, key map 31-EDO")
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn autosave_recovery_records_recoverable_project_warnings_as_diagnostics() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_autosave_missing_sample_warning_test_{}",
            std::process::id()
        ));
        let missing_sample = dir.join("missing.wav");
        let autosave_path = dir.join("orbifold_autosave.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("autosave missing sample test directory");
        let project_file = ProjectFile {
            scala_path: None,
            lumatone_path: None,
            sample_instrument_path: Some(missing_sample.clone()),
            root_midi: 69,
            base_freq: 440.0,
            synth_settings: crate::synth::SynthSettings::default(),
            project: MusicProject::default().snapshot(),
        };
        std::fs::write(&autosave_path, project_file.to_text())
            .expect("autosave fixture should be written");
        let mut app = AppState::for_layout_tests();
        app.set_autosave_path_for_tests(autosave_path);

        app.recover_autosave_project();

        assert!(app.last_status.starts_with("Recovered autosave:"));
        assert!(
            app.last_status.contains("Sample instrument unavailable"),
            "{}",
            app.last_status
        );
        let diagnostic = app
            .diagnostic_messages()
            .last()
            .expect("autosave warning should be recorded as a diagnostic");
        assert!(diagnostic.contains("Sample instrument unavailable"));
        assert!(diagnostic.contains(&missing_sample.display().to_string()));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn asset_import_target_names_avoid_conflicts() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_asset_import_conflict_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("asset test directory should be created");
        std::fs::write(dir.join("kick.wav"), b"old").expect("first asset should exist");
        std::fs::write(dir.join("kick_2.wav"), b"old").expect("second asset should exist");

        assert_eq!(unique_asset_path(&dir, "snare.wav"), dir.join("snare.wav"));
        assert_eq!(unique_asset_path(&dir, "kick.wav"), dir.join("kick_3.wav"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn asset_import_status_reports_conflict_rename() {
        assert_eq!(
            asset_import_success_status(
                AudioAssetKind::Sample,
                "kick.wav",
                Path::new("audio_assets/samples/kick.wav")
            ),
            "Imported sample: kick.wav"
        );
        assert_eq!(
            asset_import_success_status(
                AudioAssetKind::Sample,
                "kick.wav",
                Path::new("audio_assets/samples/kick_2.wav")
            ),
            "Imported sample as kick_2.wav (kick.wav already exists)"
        );
    }

    #[test]
    fn asset_import_reports_missing_source_before_unsupported_format() {
        let source = std::env::temp_dir().join(format!(
            "orbifold_missing_asset_import_{}.mp3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&source);
        let mut app = AppState::for_layout_tests();

        app.import_audio_asset_path(source.clone(), AudioAssetKind::Impulse);

        assert_eq!(
            app.last_status,
            format!(
                "Asset import error: source file missing: {}",
                source.display()
            )
        );
    }

    #[test]
    fn asset_import_rejects_directory_source_before_copying() {
        let source = std::env::temp_dir().join(format!(
            "orbifold_directory_asset_import_{}.wav",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&source);
        std::fs::create_dir_all(&source).expect("directory source should be created");
        let mut app = AppState::for_layout_tests();

        app.import_audio_asset_path(source.clone(), AudioAssetKind::Sample);

        assert_eq!(
            app.last_status,
            format!(
                "Asset import error: source is not a file: {}",
                source.display()
            )
        );

        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn asset_import_rejects_unsupported_existing_file_without_copying() {
        let source = std::env::temp_dir().join(format!(
            "orbifold_unsupported_asset_import_{}.txt",
            std::process::id()
        ));
        std::fs::write(&source, b"not an audio asset").expect("unsupported source should exist");
        let target = Path::new(AUDIO_ASSETS_DIR)
            .join(AudioAssetKind::Sample.folder())
            .join(source.file_name().expect("source should have a file name"));
        let _ = std::fs::remove_file(&target);
        let mut app = AppState::for_layout_tests();

        app.import_audio_asset_path(source.clone(), AudioAssetKind::Sample);

        assert_eq!(app.last_status, "Asset import error: unsupported sample");
        assert!(
            !target.exists(),
            "unsupported asset should not be copied into the asset library"
        );

        let _ = std::fs::remove_file(source);
    }

    fn pcm16_wav_bytes(channels: u16, sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let bits_per_sample = 16_u16;
        let data: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        let block_align = channels * bits_per_sample / 8;
        let riff_size = 36 + data.len() as u32;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&riff_size.to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits_per_sample.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    #[test]
    fn wav_sample_selection_reports_preview_available() {
        let path = std::env::temp_dir().join(format!(
            "orbifold_wav_sample_status_test_{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, b"RIFF----WAVE").expect("placeholder wav path should exist");
        let asset = AudioAssetItem {
            name: "Kick".to_string(),
            path: path.clone(),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        };

        assert_eq!(
            audio_asset_workflow_status(&asset),
            "WAV preview and project sample instrument available"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn non_wav_sample_selection_reports_wav_requirement() {
        let path = std::env::temp_dir().join(format!(
            "orbifold_non_wav_sample_status_test_{}.mp3",
            std::process::id()
        ));
        std::fs::write(&path, b"not actually mp3").expect("placeholder sample path should exist");
        let asset = AudioAssetItem {
            name: "Loop".to_string(),
            path: path.clone(),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        };

        assert_eq!(
            audio_asset_workflow_status(&asset),
            "WAV required for preview or project sample"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn preview_selected_sample_requires_audio_output() {
        let path = std::env::temp_dir().join(format!(
            "orbifold_wav_preview_no_audio_test_{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, b"RIFF----WAVE").expect("placeholder wav path should exist");
        let mut app = AppState::for_layout_tests();
        app.audio_assets = vec![AudioAssetItem {
            name: "Kick".to_string(),
            path: path.clone(),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        }];
        app.selected_audio_asset = Some(0);

        app.preview_selected_audio_asset();

        assert_eq!(
            app.last_status,
            "Sample preview unavailable: no audio output connected"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_selected_sample_instrument_keeps_project_assignment_without_audio_output() {
        let path = std::env::temp_dir().join(format!(
            "orbifold_project_sample_instrument_{}.wav",
            std::process::id()
        ));
        let samples = vec![16_384_i16; 2048];
        std::fs::write(&path, pcm16_wav_bytes(1, 48_000, &samples))
            .expect("sample instrument fixture should exist");
        let mut app = AppState::for_layout_tests();
        app.audio_assets = vec![AudioAssetItem {
            name: "Soft Pluck".to_string(),
            path: path.clone(),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        }];
        app.selected_audio_asset = Some(0);

        assert!(app.can_load_selected_sample_instrument());
        app.load_selected_sample_instrument();

        assert_eq!(
            app.sample_instrument_assignment,
            Some(SampleInstrumentAssignment {
                name: "Soft Pluck".to_string(),
                path: path.clone(),
            })
        );
        assert_eq!(app.last_status, "Loaded sample instrument: Soft Pluck");
        assert!(app.project_dirty);
        let (mut engine, _receiver, _sender) = app.synth.make_engine(48_000.0);
        engine.handle_command(crate::synth::AudioCommand::NoteOn {
            note: 69,
            freq: 440.0,
            velocity: 1.0,
        });
        assert!((0..512).any(|_| engine.next_sample() > 0.1));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn clear_sample_instrument_removes_project_assignment() {
        let mut app = AppState::for_layout_tests();
        app.sample_instrument_assignment = Some(SampleInstrumentAssignment {
            name: "Kick".to_string(),
            path: PathBuf::from("kick.wav"),
        });

        app.clear_sample_instrument();

        assert_eq!(app.sample_instrument_assignment, None);
        assert_eq!(app.last_status, "Sample instrument cleared");
        assert!(app.project_dirty);
    }

    #[test]
    fn clear_sample_instrument_removes_missing_project_reference() {
        let mut app = AppState::for_layout_tests();
        app.missing_sample_instrument_path = Some(PathBuf::from("audio_assets/samples/gone.wav"));
        app.project_dirty = false;

        app.clear_sample_instrument();

        assert_eq!(app.sample_instrument_assignment, None);
        assert_eq!(app.missing_sample_instrument_path, None);
        assert_eq!(app.project_file_snapshot().sample_instrument_path, None);
        assert_eq!(app.last_status, "Sample instrument cleared");
        assert!(app.project_dirty);
    }

    #[test]
    fn project_save_and_load_restores_sample_instrument_assignment() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_project_sample_restore_{}",
            std::process::id()
        ));
        let sample_path = dir.join("soft_pluck.wav");
        let project_path = dir.join("project.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("sample restore directory should be created");
        let samples = vec![16_384_i16; 2048];
        std::fs::write(&sample_path, pcm16_wav_bytes(1, 48_000, &samples))
            .expect("sample fixture should exist");

        let mut source = AppState::for_layout_tests();
        source.audio_assets = vec![AudioAssetItem {
            name: "Soft Pluck".to_string(),
            path: sample_path.clone(),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        }];
        source.selected_audio_asset = Some(0);
        source.load_selected_sample_instrument();
        source.save_project_to_path(project_path.clone());

        let text = std::fs::read_to_string(&project_path).expect("project should be saved");
        assert!(text.contains("sample_instrument_path="));
        assert!(text.contains("soft_pluck.wav"));

        let mut loaded = AppState::for_layout_tests();
        loaded.load_project_path(project_path.clone());

        assert_eq!(
            loaded.sample_instrument_assignment,
            Some(SampleInstrumentAssignment {
                name: "soft_pluck".to_string(),
                path: sample_path.clone(),
            })
        );
        assert!(!loaded.project_dirty);
        assert!(loaded.last_status.starts_with("Loaded project:"));
        let (mut engine, _receiver, _sender) = loaded.synth.make_engine(48_000.0);
        engine.handle_command(crate::synth::AudioCommand::NoteOn {
            note: 69,
            freq: 440.0,
            velocity: 1.0,
        });
        assert!((0..512).any(|_| engine.next_sample() > 0.1));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn project_load_reports_missing_sample_instrument_without_blocking_project() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_project_missing_sample_{}",
            std::process::id()
        ));
        let missing_sample = dir.join("missing.wav");
        let project_path = dir.join("project.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("missing sample project directory should exist");
        let project_file = ProjectFile {
            scala_path: None,
            lumatone_path: None,
            sample_instrument_path: Some(missing_sample.clone()),
            root_midi: 69,
            base_freq: 440.0,
            synth_settings: crate::synth::SynthSettings::default(),
            project: MusicProject::default().snapshot(),
        };
        std::fs::write(&project_path, project_file.to_text()).expect("project fixture saved");
        let mut app = AppState::for_layout_tests();

        app.load_project_path(project_path.clone());

        assert_eq!(app.project_path.as_ref(), Some(&project_path));
        assert_eq!(app.sample_instrument_assignment, None);
        assert_eq!(
            app.missing_sample_instrument_path.as_ref(),
            Some(&missing_sample)
        );
        assert!(app.can_clear_sample_instrument());
        assert_eq!(
            app.project_file_snapshot().sample_instrument_path.as_ref(),
            Some(&PathBuf::from("missing.wav"))
        );
        assert!(!app.project_dirty);
        assert!(
            app.last_status.contains("Sample instrument unavailable"),
            "{}",
            app.last_status
        );
        let diagnostic = app
            .diagnostic_messages()
            .last()
            .expect("missing sample should be recorded as a diagnostic");
        assert!(diagnostic.contains("Sample instrument unavailable"));
        assert!(diagnostic.contains(&missing_sample.display().to_string()));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn project_load_resolves_relative_references_from_project_directory() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_project_relative_refs_{}",
            std::process::id()
        ));
        let project_path = dir.join("project.orbifold");
        let relative_scale = PathBuf::from("scales/local-31-edo.scl");
        let relative_keymap = PathBuf::from("keymaps/local-31-edo.ltn");
        let relative_sample = PathBuf::from("samples/soft_pluck.wav");
        let scale_path = dir.join(&relative_scale);
        let keymap_path = dir.join(&relative_keymap);
        let sample_path = dir.join(&relative_sample);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(scale_path.parent().expect("scale parent"))
            .expect("scale directory should exist");
        std::fs::create_dir_all(keymap_path.parent().expect("keymap parent"))
            .expect("keymap directory should exist");
        std::fs::create_dir_all(sample_path.parent().expect("sample parent"))
            .expect("sample directory should exist");
        std::fs::copy("scales/31-edo.scl", &scale_path).expect("scale fixture copied");
        std::fs::copy("lumatone_factory_presets/8. 31 EDO.ltn", &keymap_path)
            .expect("keymap fixture copied");
        let samples = vec![16_384_i16; 2048];
        std::fs::write(&sample_path, pcm16_wav_bytes(1, 48_000, &samples))
            .expect("sample fixture should exist");

        let project_file = ProjectFile {
            scala_path: Some(relative_scale.clone()),
            lumatone_path: Some(relative_keymap.clone()),
            sample_instrument_path: Some(relative_sample.clone()),
            root_midi: 69,
            base_freq: 440.0,
            synth_settings: crate::synth::SynthSettings::default(),
            project: MusicProject::default().snapshot(),
        };
        std::fs::write(&project_path, project_file.to_text()).expect("project fixture saved");
        let mut app = AppState::for_layout_tests();

        app.load_project_path(project_path.clone());

        assert_eq!(app.project_path.as_ref(), Some(&project_path));
        assert_eq!(app.scala_path.as_ref(), Some(&scale_path));
        assert_eq!(app.lumatone_path.as_ref(), Some(&keymap_path));
        assert_eq!(
            app.sample_instrument_assignment,
            Some(SampleInstrumentAssignment {
                name: "soft_pluck".to_string(),
                path: sample_path.clone(),
            })
        );
        assert!(!app.project_dirty);
        let clean_snapshot = app.project_file_snapshot();
        assert_eq!(clean_snapshot.scala_path, Some(relative_scale.clone()));
        assert_eq!(clean_snapshot.lumatone_path, Some(relative_keymap.clone()));
        assert_eq!(
            clean_snapshot.sample_instrument_path,
            Some(relative_sample.clone())
        );

        app.save_project_to_path(project_path.clone());
        let saved = ProjectFile::from_text(
            &std::fs::read_to_string(&project_path).expect("saved project should be readable"),
        )
        .expect("saved project should parse");
        assert_eq!(saved.scala_path, Some(relative_scale));
        assert_eq!(saved.lumatone_path, Some(relative_keymap));
        assert_eq!(saved.sample_instrument_path, Some(relative_sample));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn stop_sample_preview_requires_audio_output() {
        let mut app = AppState::for_layout_tests();

        app.stop_audio_asset_preview();

        assert_eq!(
            app.last_status,
            "Sample preview unavailable: no audio output connected"
        );
    }

    #[test]
    fn preview_selected_sample_reports_missing_file_before_unsupported_format() {
        let path = std::env::temp_dir().join(format!(
            "orbifold_missing_preview_sample_{}.mp3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut app = AppState::for_layout_tests();
        app.audio_assets = vec![AudioAssetItem {
            name: "Missing MP3".to_string(),
            path: path.clone(),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        }];
        app.selected_audio_asset = Some(0);

        app.preview_selected_audio_asset();

        assert_eq!(
            app.last_status,
            format!("Sample preview file missing: {}", path.display())
        );
    }

    #[test]
    fn scale_refresh_preserves_selection_by_path_after_reorder() {
        let mut app = AppState::for_layout_tests();
        let selected = PathBuf::from("scales/31-edo.scl");
        app.scale_library = vec![ScaleLibraryItem {
            name: "31-EDO".to_string(),
            path: selected.clone(),
        }];
        app.selected_scale_library = 0;

        app.apply_refreshed_scale_library(vec![
            ScaleLibraryItem {
                name: "12-TET".to_string(),
                path: PathBuf::from("scales/12-tet.scl"),
            },
            ScaleLibraryItem {
                name: "31-EDO".to_string(),
                path: selected,
            },
        ]);

        assert_eq!(app.selected_scale_library, 1);
        assert_eq!(app.scale_library[1].name, "31-EDO");
        assert_eq!(app.last_status, "Refreshed scale library: 2 scales");
    }

    #[test]
    fn scale_refresh_reports_when_selection_disappears() {
        let mut app = AppState::for_layout_tests();
        app.scale_library = vec![ScaleLibraryItem {
            name: "Removed Scale".to_string(),
            path: PathBuf::from("scales/removed.scl"),
        }];
        app.selected_scale_library = 0;

        app.apply_refreshed_scale_library(vec![ScaleLibraryItem {
            name: "12-TET".to_string(),
            path: PathBuf::from("scales/12-tet.scl"),
        }]);

        assert_eq!(app.selected_scale_library, 0);
        assert_eq!(
            app.last_status,
            "Refreshed scale library: 1 scales; selected scale unavailable"
        );
    }

    #[test]
    fn already_loaded_scale_path_is_noop() {
        let mut app = AppState::for_layout_tests();
        let path = PathBuf::from("scales/31-edo.scl");

        assert!(
            app.load_scale_path(path.clone(), false)
                .expect("bundled scale should load")
        );
        app.project_dirty = false;

        assert!(
            !app.load_scale_path(path.clone(), false)
                .expect("already-loaded scale should not parse again")
        );
        assert!(!app.project_dirty);
        assert_eq!(app.last_status, "Scale already loaded: 31-edo.scl");
    }

    #[test]
    fn already_loaded_lumatone_path_is_noop() {
        let mut app = AppState::for_layout_tests();
        let path = PathBuf::from("lumatone_factory_presets/1. Classic Mode.ltn");

        assert!(app.load_lumatone_path(path.clone()));
        app.project_dirty = false;

        assert!(!app.load_lumatone_path(path));
        assert!(!app.project_dirty);
        assert_eq!(
            app.last_status,
            "Key map already loaded: 1. Classic Mode.ltn"
        );
    }

    #[test]
    fn open_keymap_file_dialog_already_loaded_does_not_dirty_project() {
        let mut app = AppState::for_layout_tests();
        let path = PathBuf::from("lumatone_factory_presets/1. Classic Mode.ltn");
        assert!(app.load_lumatone_path(path.clone()));
        app.project_dirty = false;

        app.finish_file_dialog(FileDialogRequest::OpenKeymap, Some(path));

        assert!(!app.project_dirty);
        assert_eq!(
            app.last_status,
            "Key map already loaded: 1. Classic Mode.ltn"
        );
    }

    #[test]
    fn load_selected_library_scale_already_loaded_does_not_dirty_project() {
        let mut app = AppState::for_layout_tests();
        let path = PathBuf::from("scales/31-edo.scl");
        app.scala_path = Some(path.clone());
        app.scale_library = vec![ScaleLibraryItem {
            name: "31-EDO".to_string(),
            path,
        }];
        app.selected_scale_library = 0;
        app.project_dirty = false;

        app.load_selected_library_scale();

        assert!(!app.project_dirty);
        assert_eq!(app.last_status, "Scale already loaded: 31-edo.scl");
    }

    #[test]
    fn keymap_refresh_does_not_readd_missing_active_map() {
        let mut app = AppState::for_layout_tests();
        let missing = std::env::temp_dir().join(format!(
            "orbifold_missing_active_keymap_{}.ltn",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&missing);
        app.lumatone_path = Some(missing.clone());
        app.selected_lumatone = usize::MAX;

        app.reload_lumatone_presets();

        assert_eq!(app.lumatone_path.as_ref(), Some(&missing));
        assert!(
            app.lumatone_presets
                .iter()
                .all(|preset| !same_path(&preset.path, &missing))
        );
        assert!(app.selected_lumatone < app.lumatone_presets.len());
        assert_eq!(
            app.last_status,
            format!(
                "Refreshed key map presets: {}; active key map missing: {}",
                app.lumatone_presets.len(),
                path_display_name(&missing)
            )
        );
    }

    #[test]
    fn keymap_refresh_reports_selected_map_after_success() {
        let mut app = AppState::for_layout_tests();

        app.reload_lumatone_presets();

        let selected = app
            .lumatone_path
            .as_ref()
            .map(|path| path_display_name(path))
            .expect("refresh should select a default key map");
        assert_eq!(
            app.last_status,
            format!(
                "Refreshed key map presets: {}; selected {selected}",
                app.lumatone_presets.len()
            )
        );
    }

    #[test]
    fn keymap_refresh_preserves_malformed_active_map_error() {
        let mut app = AppState::for_layout_tests();
        let malformed = std::env::temp_dir().join(format!(
            "orbifold_malformed_active_keymap_{}.ltn",
            std::process::id()
        ));
        std::fs::write(&malformed, "not a valid key map").expect("malformed key map fixture");
        app.lumatone_path = Some(malformed.clone());

        app.reload_lumatone_presets();

        assert!(app.last_status.starts_with("Key map load error:"));
        assert!(!app.last_status.starts_with("Refreshed key map presets:"));

        let _ = std::fs::remove_file(malformed);
    }

    #[test]
    fn open_scale_file_dialog_marks_project_dirty_when_scale_changes() {
        let mut app = AppState::for_layout_tests();
        app.project_dirty = false;

        app.finish_file_dialog(
            FileDialogRequest::OpenScale,
            Some(PathBuf::from("scales/31-edo.scl")),
        );

        assert!(app.project_dirty);
        assert_eq!(app.last_status, "Loaded Scala file");
    }

    #[test]
    fn open_scale_file_dialog_already_loaded_does_not_dirty_project() {
        let mut app = AppState::for_layout_tests();
        let path = PathBuf::from("scales/31-edo.scl");
        app.scala_path = Some(path.clone());
        app.project_dirty = false;

        app.finish_file_dialog(FileDialogRequest::OpenScale, Some(path));

        assert!(!app.project_dirty);
        assert_eq!(app.last_status, "Scale already loaded: 31-edo.scl");
    }

    #[test]
    fn bundled_scale_remove_is_rejected_by_command_layer() {
        let mut app = AppState::for_layout_tests();
        app.scale_library = vec![ScaleLibraryItem {
            name: "12-TET".to_string(),
            path: PathBuf::from("scales/12-tet.scl"),
        }];
        app.selected_scale_library = 0;

        app.remove_selected_library_scale();

        assert_eq!(app.scale_library.len(), 1);
        assert_eq!(app.last_status, "Bundled scale cannot be removed: 12-TET");
    }

    #[test]
    fn asset_refresh_preserves_selection_by_path_after_reorder() {
        let mut app = AppState::for_layout_tests();
        let selected = PathBuf::from("audio_assets/samples/snare.wav");
        app.audio_assets = vec![AudioAssetItem {
            name: "Snare".to_string(),
            path: selected.clone(),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        }];
        app.selected_audio_asset = Some(0);

        app.apply_refreshed_audio_assets(vec![
            AudioAssetItem {
                name: "Kick".to_string(),
                path: PathBuf::from("audio_assets/samples/kick.wav"),
                kind: AudioAssetKind::Sample,
                is_dir: false,
            },
            AudioAssetItem {
                name: "Snare".to_string(),
                path: selected,
                kind: AudioAssetKind::Sample,
                is_dir: false,
            },
        ]);

        assert_eq!(app.selected_audio_asset, Some(1));
        assert_eq!(app.audio_assets[1].name, "Snare");
        assert_eq!(app.last_status, "Refreshed asset browser: 2 assets");
    }

    #[test]
    fn asset_refresh_clears_missing_selection_with_visible_status() {
        let mut app = AppState::for_layout_tests();
        app.audio_assets = vec![AudioAssetItem {
            name: "Gone".to_string(),
            path: PathBuf::from("audio_assets/samples/gone.wav"),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        }];
        app.selected_audio_asset = Some(0);

        app.apply_refreshed_audio_assets(vec![AudioAssetItem {
            name: "Snare".to_string(),
            path: PathBuf::from("audio_assets/samples/snare.wav"),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        }]);

        assert_eq!(app.selected_audio_asset, None);
        assert_eq!(
            app.last_status,
            "Refreshed asset browser: 1 assets; selected asset missing"
        );
    }

    #[test]
    fn project_save_reports_autosave_cleanup_failure_without_fake_recovery_file() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_autosave_cleanup_failure_test_{}",
            std::process::id()
        ));
        let autosave_path = dir.join("orbifold_autosave.orbifold");
        let project_path = dir.join("project.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&autosave_path)
            .expect("autosave directory should block file removal");

        let mut app = AppState::for_layout_tests();
        app.set_autosave_path_for_tests(autosave_path.clone());
        let root_midi = app.scale_state.lock().root_midi;
        app.add_clip_note_at(0.0, root_midi);

        app.save_project_to_path(project_path.clone());

        assert!(!app.project_dirty);
        assert_eq!(app.project_path, Some(project_path.clone()));
        assert!(!app.autosave_available);
        assert!(autosave_path.exists());
        assert!(app.last_status.starts_with("Saved project:"));
        assert!(app.last_status.contains("autosave cleanup error"));
        assert!(
            app.last_status
                .contains(&autosave_path.display().to_string())
        );
        let diagnostic = app
            .diagnostic_messages()
            .last()
            .expect("autosave cleanup failure should be recorded");
        assert!(diagnostic.starts_with("Project autosave cleanup error after save"));
        assert!(diagnostic.contains(&project_path.display().to_string()));
        assert!(diagnostic.contains(&autosave_path.display().to_string()));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn autosave_available_requires_recovery_file_not_directory() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_autosave_directory_test_{}",
            std::process::id()
        ));
        let autosave_path = dir.join("orbifold_autosave.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&autosave_path)
            .expect("autosave directory should exist for availability test");

        let mut app = AppState::for_layout_tests();
        app.set_autosave_path_for_tests(autosave_path.clone());

        assert!(autosave_path.exists());
        assert!(!autosave_recovery_file_exists(&autosave_path));
        assert!(!app.autosave_available);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_autosave_write_does_not_expose_nonfile_recovery_path() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_autosave_write_failure_test_{}",
            std::process::id()
        ));
        let settings_path = dir.join("settings.txt");
        let autosave_path = dir.join("orbifold_autosave.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&autosave_path)
            .expect("autosave directory should block autosave writes");

        let mut app = AppState::for_layout_tests();
        app.set_settings_path_for_tests(settings_path, true);
        app.set_autosave_path_for_tests(autosave_path.clone());
        let root_midi = app.scale_state.lock().root_midi;

        app.add_clip_note_at(0.0, root_midi);

        assert!(app.project_dirty);
        assert!(!app.autosave_available);
        assert!(!autosave_recovery_file_exists(&autosave_path));
        assert!(app.last_status.starts_with("Project autosave error ("));
        assert!(app.last_status.contains("Added note"));
        assert!(
            app.last_status
                .contains(&autosave_path.display().to_string())
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn recover_autosave_missing_file_clears_recovery_availability() {
        let path = std::env::temp_dir().join(format!(
            "orbifold_missing_autosave_recover_test_{}.orbifold",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut app = AppState::for_layout_tests();
        app.set_autosave_path_for_tests(path.clone());
        app.autosave_available = true;

        app.recover_autosave_project();

        assert!(!app.autosave_available);
        assert!(app.last_status.starts_with("Autosave open error ("));
        assert!(app.last_status.contains(&path.display().to_string()));
    }

    #[test]
    fn recover_malformed_autosave_leaves_current_project_untouched() {
        let autosave = std::env::temp_dir().join(format!(
            "orbifold_malformed_autosave_recover_test_{}.orbifold",
            std::process::id()
        ));
        let project = std::env::temp_dir().join(format!(
            "orbifold_malformed_autosave_current_test_{}.orbifold",
            std::process::id()
        ));
        let backup = project.with_file_name(format!(
            "{}.bak",
            project
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("orbifold_malformed_autosave_current.orbifold")
        ));
        let _ = std::fs::remove_file(&autosave);
        let _ = std::fs::remove_file(&project);
        let _ = std::fs::remove_file(&backup);
        let mut app = AppState::for_layout_tests();
        let root = app.scale_state.lock().root_midi;
        app.add_clip_note_at(0.0, root);
        app.save_project_to_path(project.clone());
        std::fs::write(&autosave, "not a valid autosave").expect("malformed autosave fixture");
        app.set_autosave_path_for_tests(autosave.clone());

        app.recover_autosave_project();

        assert!(app.autosave_available);
        assert_eq!(app.music_project.lock().clip.notes.len(), 1);
        assert_eq!(app.project_path.as_ref(), Some(&project));
        assert!(!app.project_dirty);
        assert!(app.last_status.starts_with("Autosave parse error ("));
        assert!(app.last_status.contains(&autosave.display().to_string()));

        let _ = std::fs::remove_file(autosave);
        let _ = std::fs::remove_file(project);
        let _ = std::fs::remove_file(backup);
    }

    #[test]
    fn transport_setting_status_preserves_autosave_error() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_autosave_transport_failure_test_{}",
            std::process::id()
        ));
        let settings_path = dir.join("settings.txt");
        let autosave_path = dir.join("orbifold_autosave.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&autosave_path)
            .expect("autosave directory should block autosave writes");

        let mut app = AppState::for_layout_tests();
        app.set_settings_path_for_tests(settings_path, true);
        app.set_autosave_path_for_tests(autosave_path.clone());

        app.toggle_metronome();

        assert!(app.project_dirty);
        assert!(!app.autosave_available);
        assert!(app.last_status.starts_with("Project autosave error ("));
        assert!(app.last_status.contains("Metronome on"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn loop_length_status_preserves_autosave_error() {
        let dir = std::env::temp_dir().join(format!(
            "orbifold_autosave_loop_failure_test_{}",
            std::process::id()
        ));
        let settings_path = dir.join("settings.txt");
        let autosave_path = dir.join("orbifold_autosave.orbifold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&autosave_path)
            .expect("autosave directory should block autosave writes");

        let mut app = AppState::for_layout_tests();
        app.set_settings_path_for_tests(settings_path, true);
        app.set_autosave_path_for_tests(autosave_path);

        assert!(app.set_loop_beats(8.0));

        assert!(app.project_dirty);
        assert!(app.last_status.starts_with("Project autosave error ("));
        assert!(app.last_status.contains("Loop length 8 beats"));

        let _ = std::fs::remove_dir_all(dir);
    }
}
