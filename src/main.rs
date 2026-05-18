mod app;
mod audio;
mod logging;
mod midi;
mod project;
mod sample_preview;
mod scala;
mod scale;
mod settings;
mod synth;
mod time;
mod ui;

use parking_lot::Mutex;
use std::sync::{Arc, mpsc::Sender};

use crate::app::AppState;
use crate::audio::{AudioStream, AudioStreamInfo, build_audio_stream};
use crate::midi::{
    MIDI_CHANNEL_FILTER_ALL, MidiCapture, SharedMidiCapture, SharedMidiChannelFilter,
    SharedMidiHeld, SharedMidiLast, SharedMidiLog, SharedMidiSustain,
};
use crate::project::{MusicProject, SharedMusicProject};
use crate::scale::ScaleState;
use crate::settings::AppSettings;
use crate::synth::{AudioCommand, SynthHandle};

type AudioBuildResult =
    Result<(AudioStream, String, Sender<AudioCommand>, AudioStreamInfo), String>;

fn main() -> Result<(), String> {
    logging::init();
    run().inspect_err(|err| log::error!("Application error: {err}"))
}

fn run() -> Result<(), String> {
    let args = std::env::args().collect::<Vec<_>>();
    let screenshot_size = parse_screenshot_size_arg(&args)?;
    if args.iter().any(|arg| arg == "--startup-probe") {
        let _app = build_app_state(AppStartupOptions {
            probe_audio: true,
            probe_midi: true,
        });
        return Ok(());
    }

    let screenshot_on_start =
        args.iter().any(|arg| arg == "--screenshot") || screenshot_size.is_some();
    let probe_hardware = !screenshot_on_start;
    let app = build_app_state(AppStartupOptions {
        probe_audio: probe_hardware,
        probe_midi: probe_hardware,
    });
    ui::run(app, screenshot_on_start, screenshot_size)
}

fn parse_screenshot_size_arg(args: &[String]) -> Result<Option<(f64, f64)>, String> {
    let Some(value) = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--screenshot-size="))
    else {
        return Ok(None);
    };
    let (width, height) = value
        .split_once('x')
        .or_else(|| value.split_once('X'))
        .ok_or_else(|| "Expected --screenshot-size=WIDTHxHEIGHT".to_string())?;
    let width = parse_screenshot_dimension(width, "width")?;
    let height = parse_screenshot_dimension(height, "height")?;
    Ok(Some((width, height)))
}

fn parse_screenshot_dimension(value: &str, label: &str) -> Result<f64, String> {
    let dimension = value
        .parse::<f64>()
        .map_err(|_| format!("Invalid screenshot {label}: {value}"))?;
    if !dimension.is_finite() || dimension <= 0.0 {
        return Err(format!("Screenshot {label} must be positive"));
    }
    Ok(dimension)
}

#[derive(Clone, Copy, Debug)]
struct AppStartupOptions {
    probe_audio: bool,
    probe_midi: bool,
}

fn build_app_state(options: AppStartupOptions) -> AppState {
    let settings_path = AppSettings::default_path();
    let (settings, settings_loaded, mut startup_status) = match AppSettings::load(&settings_path) {
        Ok(settings) => (settings, true, None),
        Err(err) => {
            let message = format!("Settings load error: {err}");
            log::error!("{message}; using default settings");
            (AppSettings::default(), false, Some(message))
        }
    };

    let scale_state = Arc::new(Mutex::new(ScaleState {
        root_midi: settings.root_midi,
        base_freq: settings.base_freq,
        ..ScaleState::default()
    }));
    let synth = SynthHandle::new(32);
    if let Err(err) = synth.set_settings(settings.synth_settings()) {
        fatal(format!("Failed to initialize synth settings: {err}"));
    }
    let midi_last: SharedMidiLast = Arc::new(Mutex::new(None));
    let midi_log: SharedMidiLog = Arc::new(Mutex::new(Vec::new()));
    let midi_capture: SharedMidiCapture = Arc::new(Mutex::new(MidiCapture::default()));
    let midi_held: SharedMidiHeld = Arc::new(Mutex::new(Default::default()));
    let midi_sustain: SharedMidiSustain = Arc::new(Mutex::new(Default::default()));
    let midi_channel_filter: SharedMidiChannelFilter =
        Arc::new(std::sync::atomic::AtomicI8::new(MIDI_CHANNEL_FILTER_ALL));
    let music_project: SharedMusicProject = Arc::new(Mutex::new(MusicProject::default()));

    let (audio_stream, audio_output_name, audio_stream_info) = build_startup_audio(
        options.probe_audio,
        &settings,
        &mut startup_status,
        &synth,
        build_audio_stream,
    );
    if !options.probe_midi {
        append_status(
            &mut startup_status,
            "Screenshot mode: MIDI hardware probing skipped".to_string(),
        );
    }

    AppState::new(
        scale_state,
        synth,
        midi_last,
        midi_log,
        midi_capture,
        midi_held,
        midi_sustain,
        midi_channel_filter,
        music_project,
        audio_stream,
        audio_stream_info,
        audio_output_name,
        settings,
        startup_status,
        options.probe_audio,
        options.probe_midi,
        should_persist_startup_settings(options, settings_loaded),
    )
}

fn build_startup_audio(
    probe_audio: bool,
    settings: &AppSettings,
    startup_status: &mut Option<String>,
    synth: &SynthHandle,
    mut build_audio: impl FnMut(&SynthHandle, Option<&str>) -> AudioBuildResult,
) -> (Option<AudioStream>, String, Option<AudioStreamInfo>) {
    if probe_audio {
        let requested_audio = settings.audio_output_name.as_deref();
        let audio_build = match build_audio(synth, requested_audio) {
            Ok(stream) => Ok(stream),
            Err(err) if requested_audio.is_some() => {
                let message =
                    format!("Saved audio output unavailable: {err}; trying default output");
                log::error!("{message}");
                append_status(startup_status, message);
                build_audio(synth, None)
            }
            Err(err) => Err(err),
        };
        match audio_build {
            Ok((stream, audio_output_name, audio_sender, audio_info)) => {
                if let Err(err) = stream.play() {
                    let message = format!("Audio playback failed: {err}");
                    log::error!("{message}; audio output disabled");
                    append_status(startup_status, message);
                    (None, String::new(), None)
                } else {
                    synth.install_sender(audio_sender);
                    (Some(stream), audio_output_name, Some(audio_info))
                }
            }
            Err(err) => {
                let message = format!("Audio unavailable: {err}");
                log::error!("{message}; audio output disabled");
                append_status(startup_status, message);
                (None, String::new(), None)
            }
        }
    } else {
        append_status(
            startup_status,
            "Screenshot mode: audio hardware probing skipped".to_string(),
        );
        (None, String::new(), None)
    }
}

fn append_status(status: &mut Option<String>, message: String) {
    *status = Some(match status.take() {
        Some(status) => format!("{status}; {message}"),
        None => message,
    });
}

fn should_persist_startup_settings(options: AppStartupOptions, settings_loaded: bool) -> bool {
    settings_loaded && options.probe_audio && options.probe_midi
}

fn fatal(message: String) -> ! {
    log::error!("{message}");
    panic!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_screenshot_size_argument() {
        let args = vec![
            "orbifold".to_string(),
            "--screenshot-size=1200x760".to_string(),
        ];
        assert_eq!(
            parse_screenshot_size_arg(&args).unwrap(),
            Some((1200.0, 760.0))
        );
    }

    #[test]
    fn rejects_invalid_screenshot_size_argument() {
        let args = vec![
            "orbifold".to_string(),
            "--screenshot-size=1200-by-760".to_string(),
        ];
        assert!(parse_screenshot_size_arg(&args).is_err());
    }

    #[test]
    fn screenshot_startup_skips_audio_and_midi_hardware_probes() {
        let app = build_app_state(AppStartupOptions {
            probe_audio: false,
            probe_midi: false,
        });

        assert!(app.audio_stream.is_none());
        assert!(app.audio_outputs.is_empty());
        assert!(app.midi_connection.is_none());
        assert!(app.midi_inputs.is_empty());
        assert!(app.show_device_panel);
        assert!(
            app.last_status
                .contains("Screenshot mode: audio hardware probing skipped")
        );
        assert!(
            app.last_status
                .contains("Screenshot mode: MIDI hardware probing skipped")
        );
        assert!(
            app.last_status
                .contains("Device setup required: audio unavailable; MIDI unavailable")
        );
    }

    #[test]
    fn startup_persists_settings_only_after_successful_load_and_full_probe() {
        assert!(should_persist_startup_settings(
            AppStartupOptions {
                probe_audio: true,
                probe_midi: true,
            },
            true,
        ));
        assert!(!should_persist_startup_settings(
            AppStartupOptions {
                probe_audio: true,
                probe_midi: true,
            },
            false,
        ));
        assert!(!should_persist_startup_settings(
            AppStartupOptions {
                probe_audio: false,
                probe_midi: false,
            },
            true,
        ));
    }

    #[test]
    fn startup_audio_failure_preserves_visible_status() {
        let settings = AppSettings::default();
        let synth = SynthHandle::new(32);
        let mut startup_status = None;

        let (stream, output_name, info) = build_startup_audio(
            true,
            &settings,
            &mut startup_status,
            &synth,
            |_synth, requested| {
                assert_eq!(requested, None);
                Err("No output device".to_string())
            },
        );

        assert!(stream.is_none());
        assert_eq!(output_name, "");
        assert_eq!(info, None);
        assert_eq!(
            startup_status.as_deref(),
            Some("Audio unavailable: No output device")
        );
    }

    #[test]
    fn startup_audio_failure_reports_saved_device_fallback() {
        let settings = AppSettings {
            audio_output_name: Some("Missing Interface".to_string()),
            ..AppSettings::default()
        };
        let synth = SynthHandle::new(32);
        let mut startup_status = None;
        let mut requested = Vec::new();

        let (stream, output_name, info) = build_startup_audio(
            true,
            &settings,
            &mut startup_status,
            &synth,
            |_synth, requested_name| {
                requested.push(requested_name.map(str::to_string));
                Err("No output device".to_string())
            },
        );

        assert!(stream.is_none());
        assert_eq!(output_name, "");
        assert_eq!(info, None);
        assert_eq!(requested, vec![Some("Missing Interface".to_string()), None]);
        assert_eq!(
            startup_status.as_deref(),
            Some(
                "Saved audio output unavailable: No output device; trying default output; Audio unavailable: No output device"
            )
        );
    }
}
