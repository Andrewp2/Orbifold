use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use crate::synth::{SynthSettings, Waveform};

pub(crate) const SETTINGS_FILE: &str = "orbifold_settings.txt";
const LEGACY_SETTINGS_FILE: &str = "microtonal_daw_settings.txt";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AppSettings {
    pub(crate) audio_output_name: Option<String>,
    pub(crate) midi_input_name: Option<String>,
    pub(crate) scala_path: Option<PathBuf>,
    pub(crate) lumatone_path: Option<PathBuf>,
    pub(crate) scale_library: Vec<PathBuf>,
    pub(crate) recent_projects: Vec<PathBuf>,
    pub(crate) root_midi: i32,
    pub(crate) base_freq: f32,
    pub(crate) ui_scale: f32,
    pub(crate) master_gain: f32,
    pub(crate) attack_ms: f32,
    pub(crate) release_ms: f32,
    pub(crate) waveform: Waveform,
    pub(crate) drive: f32,
    pub(crate) filter_cutoff_hz: f32,
    pub(crate) delay_mix: f32,
    pub(crate) delay_feedback: f32,
    pub(crate) delay_time_ms: f32,
    pub(crate) midi_debug: bool,
    pub(crate) midi_channel_filter: Option<u8>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let synth = SynthSettings::default();
        Self {
            audio_output_name: None,
            midi_input_name: None,
            scala_path: None,
            lumatone_path: None,
            scale_library: Vec::new(),
            recent_projects: Vec::new(),
            root_midi: 69,
            base_freq: 440.0,
            ui_scale: 1.0,
            master_gain: synth.master_gain,
            attack_ms: synth.attack_ms,
            release_ms: synth.release_ms,
            waveform: synth.waveform,
            drive: synth.drive,
            filter_cutoff_hz: synth.filter_cutoff_hz,
            delay_mix: synth.delay_mix,
            delay_feedback: synth.delay_feedback,
            delay_time_ms: synth.delay_time_ms,
            midi_debug: false,
            midi_channel_filter: None,
        }
    }
}

impl AppSettings {
    pub(crate) fn default_path() -> PathBuf {
        PathBuf::from(SETTINGS_FILE)
    }

    pub(crate) fn load(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(data) => Self::from_text(&data).map_err(|err| settings_file_error(path, err)),
            Err(err) if err.kind() == ErrorKind::NotFound => Self::load_legacy_or_default(path),
            Err(err) => Err(settings_file_error(path, err)),
        }
    }

    fn load_legacy_or_default(path: &Path) -> Result<Self, String> {
        if path == Path::new(SETTINGS_FILE) {
            match fs::read_to_string(LEGACY_SETTINGS_FILE) {
                Ok(data) => {
                    log::error!(
                        "Settings file {} not found; loading legacy settings from {}",
                        path.display(),
                        LEGACY_SETTINGS_FILE
                    );
                    return Self::from_text(&data)
                        .map_err(|err| settings_file_error(Path::new(LEGACY_SETTINGS_FILE), err));
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    log::error!(
                        "Settings file {} not found; using default settings",
                        path.display()
                    );
                }
                Err(err) => return Err(settings_file_error(Path::new(LEGACY_SETTINGS_FILE), err)),
            }
        } else {
            log::error!(
                "Settings file {} not found; using default settings",
                path.display()
            );
        }
        Ok(Self::default())
    }

    pub(crate) fn save(&self, path: &Path) -> Result<(), String> {
        write_settings_file_safely(path, &self.to_text())
            .map_err(|err| settings_file_error(path, err))
    }

    pub(crate) fn synth_settings(&self) -> SynthSettings {
        SynthSettings {
            master_gain: self.master_gain,
            attack_ms: self.attack_ms,
            release_ms: self.release_ms,
            waveform: self.waveform,
            drive: self.drive,
            filter_cutoff_hz: self.filter_cutoff_hz,
            delay_mix: self.delay_mix,
            delay_feedback: self.delay_feedback,
            delay_time_ms: self.delay_time_ms,
        }
    }

    pub(crate) fn apply_synth_settings(&mut self, settings: SynthSettings) {
        self.master_gain = settings.master_gain;
        self.attack_ms = settings.attack_ms;
        self.release_ms = settings.release_ms;
        self.waveform = settings.waveform;
        self.drive = settings.drive;
        self.filter_cutoff_hz = settings.filter_cutoff_hz;
        self.delay_mix = settings.delay_mix;
        self.delay_feedback = settings.delay_feedback;
        self.delay_time_ms = settings.delay_time_ms;
    }

    fn to_text(&self) -> String {
        let mut out = String::new();
        push_optional(
            &mut out,
            "audio_output_name",
            self.audio_output_name.as_deref(),
        );
        push_optional(&mut out, "midi_input_name", self.midi_input_name.as_deref());
        push_optional(
            &mut out,
            "scala_path",
            self.scala_path.as_ref().and_then(|path| path.to_str()),
        );
        push_optional(
            &mut out,
            "lumatone_path",
            self.lumatone_path.as_ref().and_then(|path| path.to_str()),
        );
        out.push_str(&format!("root_midi={}\n", self.root_midi));
        out.push_str(&format!("base_freq={}\n", self.base_freq));
        out.push_str(&format!("ui_scale={}\n", self.ui_scale));
        out.push_str(&format!("master_gain={}\n", self.master_gain));
        out.push_str(&format!("attack_ms={}\n", self.attack_ms));
        out.push_str(&format!("release_ms={}\n", self.release_ms));
        out.push_str(&format!("waveform={}\n", self.waveform.as_str()));
        out.push_str(&format!("drive={}\n", self.drive));
        out.push_str(&format!("filter_cutoff_hz={}\n", self.filter_cutoff_hz));
        out.push_str(&format!("delay_mix={}\n", self.delay_mix));
        out.push_str(&format!("delay_feedback={}\n", self.delay_feedback));
        out.push_str(&format!("delay_time_ms={}\n", self.delay_time_ms));
        out.push_str(&format!("midi_debug={}\n", self.midi_debug));
        out.push_str(&format!(
            "midi_channel_filter={}\n",
            midi_channel_filter_text(self.midi_channel_filter)
        ));
        for path in &self.scale_library {
            if let Some(path) = path.to_str() {
                out.push_str("scale_library=");
                out.push_str(path);
                out.push('\n');
            }
        }
        for path in &self.recent_projects {
            if let Some(path) = path.to_str() {
                out.push_str("recent_project=");
                out.push_str(path);
                out.push('\n');
            }
        }
        out
    }

    fn from_text(data: &str) -> Result<Self, String> {
        let mut settings = Self::default();
        settings.scale_library.clear();
        settings.recent_projects.clear();

        for (line_idx, line) in data.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("Invalid settings line {}: {line}", line_idx + 1))?;
            match key {
                "audio_output_name" => settings.audio_output_name = optional_string(value),
                "midi_input_name" => settings.midi_input_name = optional_string(value),
                "scala_path" => settings.scala_path = optional_path(value),
                "lumatone_path" => settings.lumatone_path = optional_path(value),
                "root_midi" => settings.root_midi = parse_i32(value, key)?,
                "base_freq" => settings.base_freq = parse_positive_f32(value, key)?,
                "ui_scale" => settings.ui_scale = parse_ui_scale(value, key)?,
                "master_gain" => settings.master_gain = parse_non_negative_f32(value, key)?,
                "attack_ms" => settings.attack_ms = parse_non_negative_f32(value, key)?,
                "release_ms" => settings.release_ms = parse_non_negative_f32(value, key)?,
                "drive" => settings.drive = parse_non_negative_f32(value, key)?,
                "filter_cutoff_hz" => settings.filter_cutoff_hz = parse_positive_f32(value, key)?,
                "delay_mix" => settings.delay_mix = parse_non_negative_f32(value, key)?,
                "delay_feedback" => settings.delay_feedback = parse_non_negative_f32(value, key)?,
                "delay_time_ms" => settings.delay_time_ms = parse_non_negative_f32(value, key)?,
                "waveform" => {
                    settings.waveform = Waveform::from_str(value)
                        .ok_or_else(|| format!("Invalid waveform: {value}"))?;
                }
                "midi_debug" => settings.midi_debug = parse_bool(value, key)?,
                "midi_channel_filter" => {
                    settings.midi_channel_filter = parse_midi_channel_filter(value, key)?
                }
                "scale_library" => {
                    if let Some(path) = optional_path(value) {
                        settings.scale_library.push(path);
                    }
                }
                "recent_project" => {
                    if let Some(path) = optional_path(value) {
                        settings.recent_projects.push(path);
                    }
                }
                _ => return Err(format!("Unknown settings key: {key}")),
            }
        }

        Ok(settings)
    }
}

fn settings_file_error(path: &Path, err: impl std::fmt::Display) -> String {
    format!("{}: {err}", path.display())
}

fn write_settings_file_safely(path: &Path, contents: &str) -> io::Result<()> {
    ensure_settings_parent_dir(path)?;
    let temp_path = temporary_settings_save_path(path);
    fs::write(&temp_path, contents)?;
    if let Err(err) = fs::rename(&temp_path, path) {
        if let Err(cleanup_err) = fs::remove_file(&temp_path) {
            log::error!(
                "Failed to remove temporary settings file {} after save failure: {cleanup_err}",
                temp_path.display()
            );
        }
        return Err(err);
    }
    Ok(())
}

fn ensure_settings_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn temporary_settings_save_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(SETTINGS_FILE);
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

fn push_optional(out: &mut String, key: &str, value: Option<&str>) {
    out.push_str(key);
    out.push('=');
    if let Some(value) = value {
        out.push_str(value);
    }
    out.push('\n');
}

fn optional_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn optional_path(value: &str) -> Option<PathBuf> {
    optional_string(value).map(PathBuf::from)
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

fn parse_ui_scale(value: &str, key: &str) -> Result<f32, String> {
    let value = parse_positive_f32(value, key)?;
    Ok(value.clamp(0.75, 2.0))
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

fn midi_channel_filter_text(filter: Option<u8>) -> String {
    filter
        .map(|channel| (channel + 1).to_string())
        .unwrap_or_else(|| "all".to_string())
}

fn parse_midi_channel_filter(value: &str, key: &str) -> Result<Option<u8>, String> {
    if value.eq_ignore_ascii_case("all") || value.is_empty() {
        return Ok(None);
    }
    let channel = value
        .parse::<u8>()
        .map_err(|_| format!("Invalid MIDI channel filter for {key}: {value}"))?;
    if !(1..=16).contains(&channel) {
        return Err(format!("Invalid MIDI channel filter for {key}: {value}"));
    }
    Ok(Some(channel - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let mut settings = AppSettings {
            audio_output_name: Some("Output".to_string()),
            midi_input_name: Some("Lumatone".to_string()),
            scala_path: Some(PathBuf::from("scales/test.scl")),
            lumatone_path: Some(PathBuf::from("lumatone.ltn")),
            root_midi: 60,
            base_freq: 261.6256,
            ui_scale: 1.25,
            master_gain: 0.5,
            attack_ms: 12.0,
            release_ms: 250.0,
            waveform: Waveform::Triangle,
            drive: 1.4,
            filter_cutoff_hz: 9000.0,
            delay_mix: 0.2,
            delay_feedback: 0.4,
            delay_time_ms: 320.0,
            midi_debug: true,
            midi_channel_filter: Some(2),
            ..AppSettings::default()
        };
        settings.scale_library.push(PathBuf::from("a.scl"));
        settings.scale_library.push(PathBuf::from("b.scl"));
        settings
            .recent_projects
            .push(PathBuf::from("projects/alpha.orbifold"));
        settings
            .recent_projects
            .push(PathBuf::from("projects/beta.orbifold"));

        let parsed = AppSettings::from_text(&settings.to_text()).expect("settings should parse");

        assert_eq!(parsed, settings);
    }

    #[test]
    fn legacy_settings_fixture_loads_with_defaults() {
        let settings = AppSettings::from_text(include_str!(
            "../tests/fixtures/settings/legacy_microtonal_daw_settings.txt"
        ))
        .expect("legacy settings fixture should parse");
        let defaults = AppSettings::default();

        assert_eq!(settings.audio_output_name.as_deref(), Some("Old Output"));
        assert_eq!(settings.midi_input_name.as_deref(), Some("Old Keyboard"));
        assert_eq!(
            settings.scala_path,
            Some(PathBuf::from("scales/31-edo.scl"))
        );
        assert_eq!(settings.lumatone_path, None);
        assert_eq!(settings.root_midi, 60);
        assert_eq!(settings.base_freq, 261.63);
        assert_eq!(settings.ui_scale, defaults.ui_scale);
        assert_eq!(settings.waveform, Waveform::Saw);
        assert_eq!(settings.drive, defaults.drive);
        assert_eq!(settings.filter_cutoff_hz, defaults.filter_cutoff_hz);
        assert_eq!(settings.delay_mix, defaults.delay_mix);
        assert_eq!(settings.midi_channel_filter, None);
        assert!(settings.midi_debug);
        assert_eq!(
            settings.scale_library,
            vec![PathBuf::from("scales/31-edo.scl")]
        );
        assert!(settings.recent_projects.is_empty());
    }

    #[test]
    fn orbifold_settings_v1_fixture_loads() {
        let settings = AppSettings::from_text(include_str!(
            "../tests/fixtures/settings/orbifold_settings_v1.txt"
        ))
        .expect("current settings fixture should parse");

        assert_eq!(settings.audio_output_name.as_deref(), Some("USB Interface"));
        assert_eq!(settings.midi_input_name.as_deref(), Some("Lumatone"));
        assert_eq!(
            settings.scala_path,
            Some(PathBuf::from("scales/31-edo.scl"))
        );
        assert_eq!(
            settings.lumatone_path,
            Some(PathBuf::from("lumatone_factory_presets/8. 31 EDO.ltn"))
        );
        assert_eq!(settings.root_midi, 60);
        assert_eq!(settings.base_freq, 261.63);
        assert_eq!(settings.ui_scale, 1.5);
        assert_eq!(settings.master_gain, 0.42);
        assert_eq!(settings.attack_ms, 12.0);
        assert_eq!(settings.release_ms, 180.0);
        assert_eq!(settings.waveform, Waveform::Square);
        assert_eq!(settings.drive, 1.5);
        assert_eq!(settings.filter_cutoff_hz, 14000.0);
        assert_eq!(settings.delay_mix, 0.2);
        assert_eq!(settings.delay_feedback, 0.4);
        assert_eq!(settings.delay_time_ms, 320.0);
        assert!(settings.midi_debug);
        assert_eq!(settings.midi_channel_filter, Some(2));
        assert_eq!(
            settings.scale_library,
            vec![
                PathBuf::from("scales/31-edo.scl"),
                PathBuf::from("scales/19-edo.scl")
            ]
        );
        assert!(settings.recent_projects.is_empty());
    }

    #[test]
    fn settings_reject_invalid_numbers() {
        let err = AppSettings::from_text("base_freq=-1").expect_err("settings should fail");
        assert_eq!(err, "base_freq must be positive");
    }

    #[test]
    fn settings_load_error_reports_path() {
        let path = std::env::temp_dir().join(format!(
            "orbifold_settings_load_bad_test_{}.txt",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        fs::write(&path, "base_freq=-1").expect("write bad settings");

        let err = AppSettings::load(&path).expect_err("settings should fail");

        assert!(err.contains(&path.display().to_string()));
        assert!(err.contains("base_freq must be positive"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn settings_reject_invalid_midi_channel_filter() {
        let err =
            AppSettings::from_text("midi_channel_filter=17").expect_err("settings should fail");
        assert_eq!(
            err,
            "Invalid MIDI channel filter for midi_channel_filter: 17"
        );
    }

    #[test]
    fn settings_clamp_ui_scale_to_supported_range() {
        let small = AppSettings::from_text("ui_scale=0.5").expect("settings should parse");
        assert_eq!(small.ui_scale, 0.75);

        let large = AppSettings::from_text("ui_scale=3.0").expect("settings should parse");
        assert_eq!(large.ui_scale, 2.0);
    }

    #[test]
    fn settings_save_writes_through_temp_file() {
        let path = std::env::temp_dir().join(format!(
            "orbifold_settings_save_test_{}.txt",
            std::process::id()
        ));
        let temp = temporary_settings_save_path(&path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&temp);
        let settings = AppSettings {
            midi_input_name: Some("Keyboard".to_string()),
            ui_scale: 1.25,
            ..AppSettings::default()
        };

        settings.save(&path).expect("settings save should work");

        assert!(!temp.exists());
        let loaded = AppSettings::load(&path).expect("settings should reload");
        assert_eq!(loaded.midi_input_name.as_deref(), Some("Keyboard"));
        assert_eq!(loaded.ui_scale, 1.25);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn failed_settings_save_removes_temp_file() {
        let path = std::env::temp_dir().join(format!(
            "orbifold_settings_directory_target_{}",
            std::process::id()
        ));
        let temp = temporary_settings_save_path(&path);
        let _ = fs::remove_dir_all(&path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&temp);
        fs::create_dir(&path).expect("directory target");

        let err = AppSettings::default()
            .save(&path)
            .expect_err("directory target should fail");

        assert!(err.contains(&path.display().to_string()));
        assert!(path.is_dir());
        assert!(!temp.exists());
        let _ = fs::remove_dir_all(path);
    }
}
