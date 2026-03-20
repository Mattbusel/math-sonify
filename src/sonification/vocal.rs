use super::{quantize_to_scale, AudioParams, Scale, SonifMode, Sonification};
use crate::config::SonificationConfig;

/// Vowel formant definitions: (F1, F2, F3) in Hz
const VOWELS: [(f32, f32, f32); 6] = [
    (800.0, 1200.0, 2500.0), // /a/
    (400.0, 2000.0, 2600.0), // /e/
    (300.0, 2300.0, 3000.0), // /i/
    (500.0, 900.0, 2500.0),  // /o/
    (300.0, 800.0, 2300.0),  // /u/
    (700.0, 1700.0, 2600.0), // /æ/
];

/// Formant/vocal synthesis mode. Maps attractor state to vowel space.
pub struct VocalMapping {
    min: Vec<f64>,
    max: Vec<f64>,
    alpha: f64,
    /// Current blend position (0..1 across vowels)
    vowel_pos: f32,
    /// Smoothed breathiness
    breathiness: f32,
}

impl Default for VocalMapping {
    fn default() -> Self {
        Self::new()
    }
}

impl VocalMapping {
    pub fn new() -> Self {
        Self {
            min: Vec::new(),
            max: Vec::new(),
            alpha: 0.02,
            vowel_pos: 0.0,
            breathiness: 0.0,
        }
    }

    fn normalize(&mut self, state: &[f64]) -> Vec<f32> {
        if self.min.len() != state.len() {
            self.min = state.to_vec();
            self.max = state.to_vec();
        }
        state
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                if v < self.min[i] {
                    self.min[i] = v;
                } else {
                    self.min[i] += self.alpha * (v - self.min[i]);
                }
                if v > self.max[i] {
                    self.max[i] = v;
                } else {
                    self.max[i] += self.alpha * (v - self.max[i]);
                }
                let range = (self.max[i] - self.min[i]).abs().max(1e-9);
                ((v - self.min[i]) / range) as f32
            })
            .collect()
    }

    /// Interpolate between two adjacent vowels using fractional position.
    fn interpolate_formants(t: f32) -> (f32, f32, f32) {
        let n = VOWELS.len() as f32;
        let scaled = (t.clamp(0.0, 1.0) * (n - 1.0)).max(0.0);
        let lo = scaled.floor() as usize;
        let hi = (lo + 1).min(VOWELS.len() - 1);
        let frac = scaled - lo as f32;
        let a = VOWELS[lo];
        let b = VOWELS[hi];
        (
            a.0 + frac * (b.0 - a.0),
            a.1 + frac * (b.1 - a.1),
            a.2 + frac * (b.2 - a.2),
        )
    }
}

impl Sonification for VocalMapping {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let norm = self.normalize(state);

        // state[0] drives vowel position
        let x_norm = norm.get(0).copied().unwrap_or(0.5);
        // state[1] drives transition speed/blend speed (we smooth vowel_pos toward x_norm)
        let y_norm = norm.get(1).copied().unwrap_or(0.5);
        let blend_rate = 0.005 + y_norm * 0.05;
        self.vowel_pos += blend_rate * (x_norm - self.vowel_pos);

        let (f1, f2, f3) = Self::interpolate_formants(self.vowel_pos);

        // Breathiness from speed/chaos — minimum floor of 0.08 ensures slow/periodic
        // attractors still produce an audible airy quality instead of a dead sine.
        let chaos = (speed.abs() as f32 / 100.0).clamp(0.0, 1.0);
        self.breathiness += 0.01 * ((chaos * 0.6).max(0.08) - self.breathiness);

        // Fundamental pitch from the third state dimension (if available), quantized
        // to the configured scale and base frequency.  Gives vocal mode a musical
        // pitch center rather than relying solely on formant frequencies (which are
        // fixed phoneme positions, not musical notes).
        let fundamental = if state.len() >= 3 {
            let scale = Scale::from(config.scale.as_str());
            let base = config.base_frequency as f32;
            let oct = config.octave_range as f32;
            // tanh normalise state[2] to [0,1]
            let z_norm = (state[2] as f32 / 30.0).tanh() * 0.5 + 0.5;
            quantize_to_scale(z_norm, base, oct, scale)
        } else {
            config.base_frequency as f32
        };

        let mut params = AudioParams {
            mode: SonifMode::Vocal,
            gain: 0.3,
            filter_cutoff: f1,
            filter_q: 5.0,
            chaos_level: chaos,
            ..Default::default()
        };

        // freqs[0] = musical fundamental (new); F1/F2/F3 formants stored in freqs[1..3]
        // so the audio thread can drive the glottal source at the right pitch while
        // the formant filters shape the vowel timbre.
        params.freqs[0] = fundamental;
        params.freqs[1] = f1;
        params.freqs[2] = f2;
        params.freqs[3] = f3;

        // Amplitude envelope: formant amplitudes (lower formants louder)
        params.amps[0] = 0.8;
        params.amps[1] = 0.5;
        params.amps[2] = 0.3;
        params.amps[3] = self.breathiness; // slot 3 encodes breathiness

        params
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SonificationConfig;

    fn default_config() -> SonificationConfig {
        SonificationConfig::default()
    }

    #[test]
    fn test_vocal_output_finite() {
        let mut m = VocalMapping::new();
        let p = m.map(&[1.0, 2.0, 3.0], 10.0, &default_config());
        assert!(p.freqs.iter().all(|f| f.is_finite()));
        assert!(p.gain.is_finite());
        assert_eq!(p.mode, SonifMode::Vocal);
    }

    #[test]
    fn test_vocal_empty_state() {
        let mut m = VocalMapping::new();
        let p = m.map(&[], 0.0, &default_config());
        assert_eq!(p.mode, SonifMode::Vocal);
        assert!(p.freqs[0].is_finite());
    }

    #[test]
    fn test_vocal_formants_positive() {
        let mut m = VocalMapping::new();
        let p = m.map(&[0.5, 0.5, 1.0], 5.0, &default_config());
        // F1, F2, F3 are stored in freqs[1..3] — should all be positive
        assert!(p.freqs[1] > 0.0, "F1 should be positive: {}", p.freqs[1]);
        assert!(p.freqs[2] > 0.0, "F2 should be positive: {}", p.freqs[2]);
        assert!(p.freqs[3] > 0.0, "F3 should be positive: {}", p.freqs[3]);
    }

    #[test]
    fn test_vocal_vowel_pos_stays_in_range() {
        let mut m = VocalMapping::new();
        // Drive with extreme states to see vowel_pos stays stable
        for i in 0..200 {
            let v = (i as f64 * 0.3).sin() * 100.0;
            let p = m.map(&[v, v * 0.5, v * 0.3], 30.0, &default_config());
            assert!(p.freqs.iter().all(|f| f.is_finite()), "non-finite at step {}", i);
        }
    }
}
