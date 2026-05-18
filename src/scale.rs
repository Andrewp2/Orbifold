#[derive(Clone, Debug, Default)]
pub(crate) struct ScalaScale {
    pub(crate) description: String,
    pub(crate) steps: Vec<f32>, // ratios within an octave
}

#[derive(Clone, Debug)]
pub(crate) struct ScaleNoteInfo {
    pub(crate) degree: usize,
    pub(crate) octave: i32,
    pub(crate) freq: f32,
    pub(crate) cents_from_root: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct ScaleState {
    pub(crate) scale: ScalaScale,
    pub(crate) root_midi: i32,
    pub(crate) base_freq: f32,
}

impl Default for ScaleState {
    fn default() -> Self {
        Self {
            scale: ScalaScale {
                description: "12-TET".to_string(),
                steps: (0..12).map(|i| 2.0_f32.powf(i as f32 / 12.0)).collect(),
            },
            root_midi: 69,
            base_freq: 440.0,
        }
    }
}

impl ScaleState {
    pub(crate) fn musical_note_for_midi_note(&self, midi_note: i32) -> i32 {
        if self.scale.steps.is_empty() {
            return midi_note;
        }
        let target_cents = (midi_note - self.root_midi) as f32 * 100.0;
        let scale_len = self.scale.steps.len() as i32;
        let base_octave = (target_cents as i32).div_euclid(1200);
        let mut best_note = midi_note;
        let mut best_error = f32::INFINITY;

        for octave in (base_octave - 1)..=(base_octave + 1) {
            for (degree, ratio) in self.scale.steps.iter().copied().enumerate() {
                if !ratio.is_finite() || ratio <= 0.0 {
                    continue;
                }
                let degree_cents = 1200.0 * ratio.log2() + octave as f32 * 1200.0;
                let error = (degree_cents - target_cents).abs();
                if error < best_error {
                    best_error = error;
                    best_note = self.root_midi + octave * scale_len + degree as i32;
                }
            }
        }

        best_note
    }

    pub(crate) fn note_info(&self, note: i32) -> Option<ScaleNoteInfo> {
        if self.scale.steps.is_empty() {
            return None;
        }
        let n = note - self.root_midi;
        let scale_len = self.scale.steps.len() as i32;
        let mut octave = n / scale_len;
        let mut step = n % scale_len;
        if step < 0 {
            step += scale_len;
            octave -= 1;
        }
        let ratio = self.scale.steps[step as usize];
        let freq = self.base_freq * ratio * 2.0_f32.powi(octave);
        if !freq.is_finite() || freq <= 0.0 {
            return None;
        }
        let cents_from_root = 1200.0 * (freq / self.base_freq).log2();
        Some(ScaleNoteInfo {
            degree: step as usize,
            octave,
            freq,
            cents_from_root,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_info_reports_degree_octave_and_frequency() {
        let state = ScaleState::default();
        let info = state.note_info(69).expect("root should resolve");

        assert_eq!(info.degree, 0);
        assert_eq!(info.octave, 0);
        assert_eq!(info.freq, 440.0);
        assert_eq!(info.cents_from_root, 0.0);
    }

    #[test]
    fn note_info_wraps_negative_degrees() {
        let state = ScaleState::default();
        let info = state.note_info(68).expect("note should resolve");

        assert_eq!(info.degree, 11);
        assert_eq!(info.octave, -1);
    }

    #[test]
    fn midi_note_mapping_keeps_standard_twelve_tet_identity() {
        let state = ScaleState::default();

        for note in [48, 60, 69, 72, 84] {
            assert_eq!(state.musical_note_for_midi_note(note), note);
        }
    }

    #[test]
    fn midi_note_mapping_approximates_chromatic_keys_in_large_edo_scales() {
        let state = ScaleState {
            scale: ScalaScale {
                description: "31-EDO".to_string(),
                steps: (0..31).map(|i| 2.0_f32.powf(i as f32 / 31.0)).collect(),
            },
            root_midi: 69,
            base_freq: 440.0,
        };

        assert_eq!(state.musical_note_for_midi_note(72), 77);
        assert_eq!(state.musical_note_for_midi_note(76), 87);
        assert_eq!(state.musical_note_for_midi_note(79), 95);

        let c5 = state.note_info(77).expect("mapped note should resolve");
        let e5 = state.note_info(87).expect("mapped note should resolve");
        let g5 = state.note_info(95).expect("mapped note should resolve");
        assert!((c5.cents_from_root - 309.6774).abs() < 0.01);
        assert!((e5.cents_from_root - 696.7742).abs() < 0.01);
        assert!((g5.cents_from_root - 1006.4516).abs() < 0.01);
    }
}
