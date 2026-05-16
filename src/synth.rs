use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    mpsc::{self, Receiver, Sender},
};

const OUTPUT_LIMIT_THRESHOLD: f32 = 0.48;
const OUTPUT_LIMIT_HOLD_MS: f32 = 180.0;
const OUTPUT_METER_DECAY: f32 = 0.9995;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Waveform {
    Sine,
    Triangle,
    Saw,
    Square,
}

impl Waveform {
    pub(crate) fn all() -> [Self; 4] {
        [Self::Sine, Self::Triangle, Self::Saw, Self::Square]
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Sine => "Sine",
            Self::Triangle => "Triangle",
            Self::Saw => "Saw",
            Self::Square => "Square",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "Sine" => Some(Self::Sine),
            "Triangle" => Some(Self::Triangle),
            "Saw" => Some(Self::Saw),
            "Square" => Some(Self::Square),
            _ => None,
        }
    }
}

impl Default for Waveform {
    fn default() -> Self {
        Self::Sine
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SynthSettings {
    pub(crate) master_gain: f32,
    pub(crate) attack_ms: f32,
    pub(crate) release_ms: f32,
    pub(crate) waveform: Waveform,
    pub(crate) drive: f32,
    pub(crate) filter_cutoff_hz: f32,
    pub(crate) delay_mix: f32,
    pub(crate) delay_feedback: f32,
    pub(crate) delay_time_ms: f32,
}

impl Default for SynthSettings {
    fn default() -> Self {
        Self {
            master_gain: 0.35,
            attack_ms: 5.0,
            release_ms: 100.0,
            waveform: Waveform::Sine,
            drive: 1.0,
            filter_cutoff_hz: 18_000.0,
            delay_mix: 0.0,
            delay_feedback: 0.25,
            delay_time_ms: 250.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum AudioCommand {
    NoteOn { note: u32, freq: f32, velocity: f32 },
    NoteOff { note: u32 },
    AllNotesOff,
    SetSettings(SynthSettings),
    SetMuted(bool),
}

#[derive(Clone)]
pub(crate) struct SynthHandle {
    sender: Arc<Mutex<Option<Sender<AudioCommand>>>>,
    active_notes: Arc<Mutex<HashSet<u32>>>,
    active_voice_count: Arc<AtomicUsize>,
    settings: Arc<Mutex<SynthSettings>>,
    muted: Arc<AtomicBool>,
    output_level: Arc<AtomicU32>,
    output_limited: Arc<AtomicBool>,
    voice_count: usize,
}

impl SynthHandle {
    pub(crate) fn new(voice_count: usize) -> Self {
        Self {
            sender: Arc::new(Mutex::new(None)),
            active_notes: Arc::new(Mutex::new(HashSet::new())),
            active_voice_count: Arc::new(AtomicUsize::new(0)),
            settings: Arc::new(Mutex::new(SynthSettings::default())),
            muted: Arc::new(AtomicBool::new(false)),
            output_level: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
            output_limited: Arc::new(AtomicBool::new(false)),
            voice_count,
        }
    }

    pub(crate) fn make_engine(
        &self,
        sample_rate: f32,
    ) -> (SynthEngine, Receiver<AudioCommand>, Sender<AudioCommand>) {
        let (sender, receiver) = mpsc::channel();
        let settings = *self.settings.lock();
        let muted = self.muted.load(Ordering::Relaxed);
        let engine = SynthEngine::new(
            sample_rate,
            self.voice_count,
            settings,
            muted,
            self.active_voice_count.clone(),
            self.output_level.clone(),
            self.output_limited.clone(),
        );
        (engine, receiver, sender)
    }

    pub(crate) fn install_sender(&self, sender: Sender<AudioCommand>) {
        *self.sender.lock() = Some(sender);
        self.active_notes.lock().clear();
        self.active_voice_count.store(0, Ordering::Relaxed);
        self.reset_output_meter();
    }

    pub(crate) fn note_on(&self, note: u32, freq: f32, velocity: f32) -> Result<(), String> {
        self.active_notes.lock().insert(note);
        if let Err(err) = self.send(AudioCommand::NoteOn {
            note,
            freq,
            velocity,
        }) {
            self.active_notes.lock().remove(&note);
            return Err(err);
        }
        Ok(())
    }

    pub(crate) fn note_off(&self, note: u32) -> Result<(), String> {
        self.active_notes.lock().remove(&note);
        self.send(AudioCommand::NoteOff { note })
    }

    pub(crate) fn all_notes_off(&self) -> Result<(), String> {
        self.active_notes.lock().clear();
        self.send(AudioCommand::AllNotesOff)
    }

    pub(crate) fn set_settings(&self, settings: SynthSettings) -> Result<(), String> {
        *self.settings.lock() = settings;
        self.send(AudioCommand::SetSettings(settings))
    }

    pub(crate) fn settings(&self) -> SynthSettings {
        *self.settings.lock()
    }

    pub(crate) fn set_muted(&self, muted: bool) -> Result<(), String> {
        self.muted.store(muted, Ordering::Relaxed);
        self.send(AudioCommand::SetMuted(muted))
    }

    pub(crate) fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub(crate) fn active_notes(&self) -> Vec<u32> {
        self.active_notes.lock().iter().copied().collect()
    }

    pub(crate) fn active_voice_count(&self) -> usize {
        self.active_voice_count.load(Ordering::Relaxed)
    }

    pub(crate) fn output_level(&self) -> f32 {
        f32::from_bits(self.output_level.load(Ordering::Relaxed)).clamp(0.0, 1.0)
    }

    pub(crate) fn output_limited(&self) -> bool {
        self.output_limited.load(Ordering::Relaxed)
    }

    fn reset_output_meter(&self) {
        self.output_level
            .store(0.0_f32.to_bits(), Ordering::Relaxed);
        self.output_limited.store(false, Ordering::Relaxed);
    }

    fn send(&self, command: AudioCommand) -> Result<(), String> {
        let sender = self.sender.lock().clone();
        let Some(sender) = sender else {
            log::trace!("Audio command ignored because no audio sender is installed: {command:?}");
            return Ok(());
        };
        sender
            .send(command)
            .map_err(|_| "Audio command queue is disconnected".to_string())
    }
}

#[derive(Clone, Copy, Debug)]
struct Voice {
    note: u32,
    freq: f32,
    phase: f32,
    active: bool,
    last_used: u64,
    amp: f32,
    target_amp: f32,
    attack_remaining: u32,
    attack_step: f32,
    release_remaining: u32,
    release_step: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            note: 0,
            freq: 440.0,
            phase: 0.0,
            active: false,
            last_used: 0,
            amp: 0.0,
            target_amp: 0.0,
            attack_remaining: 0,
            attack_step: 0.0,
            release_remaining: 0,
            release_step: 0.0,
        }
    }
}

pub(crate) struct SynthEngine {
    voices: Vec<Voice>,
    note_map: HashMap<u32, usize>,
    sample_rate: f32,
    settings: SynthSettings,
    muted: bool,
    counter: u64,
    active_voice_count: Arc<AtomicUsize>,
    output_level: Arc<AtomicU32>,
    output_limited: Arc<AtomicBool>,
    meter_level: f32,
    limit_hold_samples: u32,
    filter_state: f32,
    delay_buffer: Vec<f32>,
    delay_index: usize,
}

impl SynthEngine {
    fn new(
        sample_rate: f32,
        voice_count: usize,
        settings: SynthSettings,
        muted: bool,
        active_voice_count: Arc<AtomicUsize>,
        output_level: Arc<AtomicU32>,
        output_limited: Arc<AtomicBool>,
    ) -> Self {
        Self {
            voices: vec![Voice::default(); voice_count],
            note_map: HashMap::new(),
            sample_rate,
            settings,
            muted,
            counter: 1,
            active_voice_count,
            output_level,
            output_limited,
            meter_level: 0.0,
            limit_hold_samples: 0,
            filter_state: 0.0,
            delay_buffer: vec![0.0; sample_rate.max(1.0).round() as usize],
            delay_index: 0,
        }
    }

    pub(crate) fn handle_command(&mut self, command: AudioCommand) {
        match command {
            AudioCommand::NoteOn {
                note,
                freq,
                velocity,
            } => self.note_on(note, freq, velocity),
            AudioCommand::NoteOff { note } => self.note_off(note),
            AudioCommand::AllNotesOff => self.all_notes_off(),
            AudioCommand::SetSettings(settings) => self.settings = settings,
            AudioCommand::SetMuted(muted) => self.muted = muted,
        }
    }

    pub(crate) fn next_sample(&mut self) -> f32 {
        let mut out = 0.0_f32;
        let mut active_voices = 0_usize;
        for voice in &mut self.voices {
            if !voice.active {
                continue;
            }

            if voice.release_remaining > 0 {
                voice.amp = (voice.amp - voice.release_step).max(0.0);
                voice.release_remaining -= 1;
                if voice.release_remaining == 0 {
                    voice.active = false;
                    voice.amp = 0.0;
                    continue;
                }
            } else if voice.attack_remaining > 0 {
                voice.amp = (voice.amp + voice.attack_step).min(voice.target_amp);
                voice.attack_remaining -= 1;
            } else {
                voice.amp = voice.target_amp;
            }

            active_voices += 1;
            out += waveform_sample(self.settings.waveform, voice.phase) * voice.amp;
            voice.phase = (voice.phase + voice.freq / self.sample_rate).fract();
        }

        if active_voices > 1 {
            out /= (active_voices as f32).sqrt();
        }
        let filtered = self.filter_sample(out);
        let driven = drive_sample(filtered, self.settings.drive);
        let delayed = self.delay_sample(driven);
        let gain = if self.muted {
            0.0
        } else {
            self.settings.master_gain
        };
        let sample = limit_output_sample(delayed * gain);
        self.update_output_meter(sample);
        sample
    }

    pub(crate) fn update_meter(&self) {
        self.active_voice_count.store(
            self.voices.iter().filter(|voice| voice.active).count(),
            Ordering::Relaxed,
        );
    }

    fn note_on(&mut self, note: u32, freq: f32, velocity: f32) {
        let idx = if let Some(&idx) = self.note_map.get(&note) {
            idx
        } else {
            self.allocate_voice()
        };

        self.counter = self.counter.wrapping_add(1);
        self.note_map.retain(|_, mapped_idx| *mapped_idx != idx);
        self.note_map.insert(note, idx);

        let attack_samples = samples_for_ms(self.sample_rate, self.settings.attack_ms);
        let velocity = velocity.clamp(0.0, 1.0);
        let voice = &mut self.voices[idx];
        voice.note = note;
        voice.freq = freq.max(1.0);
        voice.phase = 0.0;
        voice.active = true;
        voice.last_used = self.counter;
        voice.amp = if attack_samples == 0 { velocity } else { 0.0 };
        voice.target_amp = velocity;
        voice.attack_remaining = attack_samples;
        voice.attack_step = if attack_samples == 0 {
            velocity
        } else {
            velocity / attack_samples as f32
        };
        voice.release_remaining = 0;
        voice.release_step = 0.0;
    }

    fn note_off(&mut self, note: u32) {
        if let Some(idx) = self.note_map.remove(&note)
            && let Some(voice) = self.voices.get_mut(idx)
        {
            let release_samples = samples_for_ms(self.sample_rate, self.settings.release_ms).max(1);
            voice.release_remaining = release_samples;
            voice.release_step = voice.amp / release_samples as f32;
            voice.attack_remaining = 0;
        }
    }

    fn all_notes_off(&mut self) {
        self.note_map.clear();
        for voice in &mut self.voices {
            voice.active = false;
            voice.amp = 0.0;
            voice.release_remaining = 0;
        }
    }

    fn update_output_meter(&mut self, sample: f32) {
        let level = sample.abs().clamp(0.0, 1.0);
        self.meter_level = (self.meter_level * OUTPUT_METER_DECAY).max(level);
        if self.meter_level < 0.0001 {
            self.meter_level = 0.0;
        }
        self.output_level
            .store(self.meter_level.to_bits(), Ordering::Relaxed);

        if level >= OUTPUT_LIMIT_THRESHOLD {
            self.limit_hold_samples = samples_for_ms(self.sample_rate, OUTPUT_LIMIT_HOLD_MS).max(1);
        } else if self.limit_hold_samples > 0 {
            self.limit_hold_samples -= 1;
        }
        self.output_limited
            .store(self.limit_hold_samples > 0, Ordering::Relaxed);
    }

    fn allocate_voice(&mut self) -> usize {
        if let Some((idx, _)) = self
            .voices
            .iter()
            .enumerate()
            .find(|(_, voice)| !voice.active)
        {
            return idx;
        }

        self.voices
            .iter()
            .enumerate()
            .min_by_key(|(_, voice)| voice.last_used)
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    fn filter_sample(&mut self, sample: f32) -> f32 {
        let cutoff = self
            .settings
            .filter_cutoff_hz
            .clamp(20.0, self.sample_rate * 0.45);
        let rc = 1.0 / (TAU * cutoff);
        let dt = 1.0 / self.sample_rate.max(1.0);
        let alpha = dt / (rc + dt);
        self.filter_state += alpha * (sample - self.filter_state);
        self.filter_state
    }

    fn delay_sample(&mut self, sample: f32) -> f32 {
        if self.delay_buffer.is_empty() {
            return sample;
        }
        let max_delay_samples = self.delay_buffer.len().saturating_sub(1).max(1);
        let delay_samples = samples_for_ms(self.sample_rate, self.settings.delay_time_ms)
            .clamp(1, max_delay_samples as u32) as usize;
        let read_idx =
            (self.delay_index + self.delay_buffer.len() - delay_samples) % self.delay_buffer.len();
        let delayed = self.delay_buffer[read_idx];
        let feedback = self.settings.delay_feedback.clamp(0.0, 0.95);
        self.delay_buffer[self.delay_index] = sample + delayed * feedback;
        self.delay_index = (self.delay_index + 1) % self.delay_buffer.len();
        let mix = self.settings.delay_mix.clamp(0.0, 1.0);
        sample * (1.0 - mix) + delayed * mix
    }
}

fn samples_for_ms(sample_rate: f32, ms: f32) -> u32 {
    if ms <= 0.0 || !ms.is_finite() {
        return 0;
    }
    (sample_rate * (ms / 1000.0)).round().max(0.0) as u32
}

fn waveform_sample(waveform: Waveform, phase: f32) -> f32 {
    match waveform {
        Waveform::Sine => (phase * TAU).sin(),
        Waveform::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        Waveform::Saw => 2.0 * phase - 1.0,
        Waveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    }
}

fn drive_sample(sample: f32, drive: f32) -> f32 {
    let drive = drive.max(0.0);
    if drive <= 1.0 {
        sample * drive
    } else {
        (sample * drive).tanh() / drive.tanh()
    }
}

fn limit_output_sample(sample: f32) -> f32 {
    sample.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_engine_outputs_silence_while_muted_without_stopping_voice() {
        let settings = SynthSettings {
            master_gain: 1.0,
            attack_ms: 0.0,
            delay_mix: 0.0,
            ..SynthSettings::default()
        };
        let mut engine = SynthEngine::new(
            44_100.0,
            4,
            settings,
            false,
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicU32::new(0.0_f32.to_bits())),
            Arc::new(AtomicBool::new(false)),
        );

        engine.handle_command(AudioCommand::NoteOn {
            note: 69,
            freq: 440.0,
            velocity: 1.0,
        });
        assert!((0..64).any(|_| engine.next_sample().abs() > 0.0001));

        engine.handle_command(AudioCommand::SetMuted(true));
        assert!((0..64).all(|_| engine.next_sample().abs() <= f32::EPSILON));

        engine.handle_command(AudioCommand::SetMuted(false));
        assert!((0..64).any(|_| engine.next_sample().abs() > 0.0001));
    }

    #[test]
    fn synth_handle_reports_output_level_and_limiting_from_engine() {
        let synth = SynthHandle::new(8);
        let settings = SynthSettings {
            master_gain: 1.0,
            attack_ms: 0.0,
            waveform: Waveform::Square,
            drive: 8.0,
            delay_mix: 0.0,
            ..SynthSettings::default()
        };
        synth.set_settings(settings).unwrap();
        let (mut engine, _receiver, _sender) = synth.make_engine(44_100.0);
        engine.handle_command(AudioCommand::NoteOn {
            note: 69,
            freq: 440.0,
            velocity: 1.0,
        });

        let limited = (0..512).any(|_| {
            engine.next_sample();
            synth.output_limited()
        });

        assert!(synth.output_level() > 0.0);
        assert!(limited);
    }

    #[test]
    fn default_drive_is_clean_at_unity() {
        for sample in [-2.0_f32, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0] {
            assert!((drive_sample(sample, 1.0) - sample).abs() <= f32::EPSILON);
        }
    }

    #[test]
    fn default_polyphonic_sine_chord_stays_below_limiter_threshold_at_midi_velocity() {
        let synth = SynthHandle::new(8);
        let settings = SynthSettings {
            attack_ms: 0.0,
            delay_mix: 0.0,
            ..SynthSettings::default()
        };
        synth.set_settings(settings).unwrap();
        let (mut engine, _receiver, _sender) = synth.make_engine(44_100.0);
        for (note, freq) in [(72, 523.2511), (76, 659.2551), (79, 783.9908)] {
            engine.handle_command(AudioCommand::NoteOn {
                note,
                freq,
                velocity: 100.0 / 127.0,
            });
        }

        let max_level = (0..4096)
            .map(|_| engine.next_sample().abs())
            .fold(0.0_f32, f32::max);

        assert!(
            max_level < OUTPUT_LIMIT_THRESHOLD,
            "default triad should stay below limiter threshold, got {max_level}"
        );
        assert!(!synth.output_limited());
    }
}
