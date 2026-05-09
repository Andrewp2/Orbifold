mod app;
mod audio;
mod gui;
mod midi;
mod operad_bridge;
mod operad_ui;
mod project;
mod scala;
mod scale;
mod settings;
mod synth;

use cpal::traits::StreamTrait;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::app::AppState;
use crate::audio::build_audio_stream;
use crate::midi::{MidiCapture, SharedMidiCapture, SharedMidiLast, SharedMidiLog};
use crate::project::{MusicProject, SharedMusicProject};
use crate::scale::ScaleState;
use crate::settings::AppSettings;
use crate::synth::SynthHandle;

fn main() -> eframe::Result<()> {
    let screenshot_on_start = std::env::args().any(|arg| arg == "--screenshot");
    let settings_path = AppSettings::default_path();
    let (settings, mut startup_status) = match AppSettings::load(&settings_path) {
        Ok(settings) => (settings, None),
        Err(err) => {
            let message = format!("Settings load error: {err}");
            eprintln!("{message}");
            (AppSettings::default(), Some(message))
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
    let music_project: SharedMusicProject = Arc::new(Mutex::new(MusicProject::default()));

    let requested_audio = settings.audio_output_name.as_deref();
    let audio_build = match build_audio_stream(&synth, requested_audio) {
        Ok(stream) => Ok(stream),
        Err(err) if requested_audio.is_some() => {
            let message = format!("Saved audio output unavailable: {err}");
            eprintln!("{message}");
            append_status(&mut startup_status, message);
            build_audio_stream(&synth, None)
        }
        Err(err) => Err(err),
    };
    let (audio_stream, audio_output_name, audio_sender) = match audio_build {
        Ok(stream) => stream,
        Err(err) => fatal(format!("Audio unavailable: {err}")),
    };
    if let Err(err) = audio_stream.play() {
        fatal(format!("Audio playback failed: {err}"));
    }
    synth.install_sender(audio_sender);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 760.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Orbifold",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(AppState::new(
                scale_state,
                synth,
                midi_last,
                midi_log,
                midi_capture,
                music_project,
                audio_stream,
                audio_output_name,
                settings,
                startup_status,
                screenshot_on_start,
            )))
        }),
    )
}

fn append_status(status: &mut Option<String>, message: String) {
    *status = Some(match status.take() {
        Some(status) => format!("{status}; {message}"),
        None => message,
    });
}

fn fatal(message: String) -> ! {
    eprintln!("{message}");
    panic!("{message}");
}
