use std::fs;
use std::io::ErrorKind;
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
    pub(crate) root_midi: i32,
    pub(crate) base_freq: f32,
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
            root_midi: 69,
            base_freq: 440.0,
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
        }
    }
}

impl AppSettings {
    pub(crate) fn default_path() -> PathBuf {
        PathBuf::from(SETTINGS_FILE)
    }

    pub(crate) fn load(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(data) => Self::from_text(&data),
            Err(err) if err.kind() == ErrorKind::NotFound => Self::load_legacy_or_default(path),
            Err(err) => Err(err.to_string()),
        }
    }

    fn load_legacy_or_default(path: &Path) -> Result<Self, String> {
        if path == Path::new(SETTINGS_FILE) {
            match fs::read_to_string(LEGACY_SETTINGS_FILE) {
                Ok(data) => return Self::from_text(&data),
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(err.to_string()),
            }
        }
        Ok(Self::default())
    }

    pub(crate) fn save(&self, path: &Path) -> Result<(), String> {
        fs::write(path, self.to_text()).map_err(|err| err.to_string())
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
        for path in &self.scale_library {
            if let Some(path) = path.to_str() {
                out.push_str("scale_library=");
                out.push_str(path);
                out.push('\n');
            }
        }
        out
    }

    fn from_text(data: &str) -> Result<Self, String> {
        let mut settings = Self::default();
        settings.scale_library.clear();

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
                "scale_library" => {
                    if let Some(path) = optional_path(value) {
                        settings.scale_library.push(path);
                    }
                }
                _ => return Err(format!("Unknown settings key: {key}")),
            }
        }

        Ok(settings)
    }
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

    #[test]
    fn settings_round_trip() {
        let mut settings = AppSettings {
            audio_output_name: Some("Output".to_string()),
            midi_input_name: Some("Lumatone".to_string()),
            scala_path: Some(PathBuf::from("scales/test.scl")),
            lumatone_path: Some(PathBuf::from("lumatone.ltn")),
            root_midi: 60,
            base_freq: 261.6256,
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
            ..AppSettings::default()
        };
        settings.scale_library.push(PathBuf::from("a.scl"));
        settings.scale_library.push(PathBuf::from("b.scl"));

        let parsed = AppSettings::from_text(&settings.to_text()).expect("settings should parse");

        assert_eq!(parsed, settings);
    }

    #[test]
    fn settings_reject_invalid_numbers() {
        let err = AppSettings::from_text("base_freq=-1").expect_err("settings should fail");
        assert_eq!(err, "base_freq must be positive");
    }
}
