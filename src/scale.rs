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
}
