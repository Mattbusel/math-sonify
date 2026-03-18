use super::{AudioParams, Sonification, SonifMode, Scale, quantize_to_scale};
use crate::config::SonificationConfig;

/// Direct frequency mapping: each state variable → a voice frequency.
/// Variables are normalized to [0,1] via a rolling min/max window, then
/// quantized to the selected scale.
pub struct DirectMapping {
    min: Vec<f64>,
    max: Vec<f64>,
    alpha: f64, // exponential moving average for min/max tracking
}

impl DirectMapping {
    /// Creates a new `DirectMapping` with an empty min/max window.
    ///
    /// # Returns
    /// A `DirectMapping` ready to accept state vectors of any dimension.
    pub fn new() -> Self {
        Self { min: Vec::new(), max: Vec::new(), alpha: 0.001 }
    }

    fn normalize(&mut self, state: &[f64]) -> Vec<f32> {
        if self.min.len() != state.len() {
            self.min = state.to_vec();
            self.max = state.to_vec();
        }
        state.iter().enumerate().map(|(i, &v)| {
            // Soft min/max tracking
            if v < self.min[i] { self.min[i] = v; }
            else { self.min[i] += self.alpha * (v - self.min[i]); }
            if v > self.max[i] { self.max[i] = v; }
            else { self.max[i] += self.alpha * (v - self.max[i]); }
            let range = (self.max[i] - self.min[i]).abs().max(1e-9);
            ((v - self.min[i]) / range) as f32
        }).collect()
    }
}

/// Map a normalized value `t ∈ [0,1]` to a frequency in the audible range [20, 20000] Hz.
///
/// Uses the configured scale and octave range. The returned value is guaranteed
/// to be in `[base_frequency, base_frequency * 2^octave_range]`, which is always
/// within [20 Hz, 20 000 Hz] for default config values.
pub fn map_to_frequency(t: f32, base_hz: f32, octave_range: f32, scale: super::Scale) -> f32 {
    super::quantize_to_scale(t.clamp(0.0, 1.0), base_hz, octave_range, scale)
}

/// Map a normalized state value to an amplitude in [0, 1].
///
/// Uses a linear mapping: `0.5 + 0.5 * t`, so the minimum amplitude is 0.5
/// (always audible) and the maximum is 1.0. The result is clamped to [0, 1].
pub fn map_to_amplitude(t: f32) -> f32 {
    (0.5 + 0.5 * t).clamp(0.0, 1.0)
}

impl Sonification for DirectMapping {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let norm = self.normalize(state);
        let scale: Scale = config.scale.clone().into();
        let base = config.base_frequency as f32;
        let oct = config.octave_range as f32;

        let mut params = AudioParams {
            mode: SonifMode::Direct,
            gain: 0.25,
            filter_cutoff: 2000.0,
            filter_q: 0.7,
            ..Default::default()
        };

        // Up to 4 voices from the first 4 state variables
        for i in 0..4.min(norm.len()) {
            params.freqs[i] = quantize_to_scale(norm[i], base, oct, scale);
            params.amps[i] = if i < norm.len() { 0.5 + 0.5 * norm[i] } else { 0.0 };
            params.pans[i] = norm[i] * 2.0 - 1.0;
        }
        // Use last dimension to modulate filter cutoff
        if let Some(&last) = norm.last() {
            params.filter_cutoff = 300.0 + 3700.0 * last;
        }
        params.chaos_level = (speed.abs() as f32 / 100.0).clamp(0.0, 1.0);
        params
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sonification::Scale;

    #[test]
    fn test_map_to_frequency_in_audible_range() {
        // For any t in [0, 1] the frequency must be in [20 Hz, 20 000 Hz].
        let base = 220.0_f32;
        let octave_range = 3.0_f32;
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let f = map_to_frequency(t, base, octave_range, Scale::Pentatonic);
            assert!(f >= 20.0 && f <= 20000.0,
                "Frequency {} out of audible range at t={}", f, t);
            assert!(f.is_finite(), "Frequency is non-finite at t={}", t);
        }
    }

    #[test]
    fn test_map_to_amplitude_in_unit_range() {
        // map_to_amplitude must always return a value in [0, 1].
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let a = map_to_amplitude(t);
            assert!(a >= 0.0 && a <= 1.0, "Amplitude {} out of [0,1] at t={}", a, t);
        }
    }

    #[test]
    fn test_monotone_input_produces_monotone_frequency() {
        // A strictly increasing sequence of normalized state values should yield
        // a non-decreasing frequency sequence (scale quantization is monotone).
        let base = 220.0_f32;
        let octave_range = 3.0_f32;
        let mut prev = 0.0_f32;
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let f = map_to_frequency(t, base, octave_range, Scale::Pentatonic);
            assert!(f >= prev, "Frequency not monotone: {} < {} at t={}", f, prev, t);
            prev = f;
        }
    }

    #[test]
    fn test_direct_mapping_produces_valid_params() {
        // Full DirectMapping::map() should return finite, non-negative frequencies.
        let mut mapper = DirectMapping::new();
        let state = vec![1.0_f64, 5.0, -3.0, 2.0];
        let config = crate::config::SonificationConfig::default();
        let params = mapper.map(&state, 10.0, &config);
        for (i, &f) in params.freqs.iter().enumerate() {
            assert!(f >= 0.0 && f.is_finite(),
                "Voice {} frequency {} is invalid", i, f);
        }
        for (i, &a) in params.amps.iter().enumerate() {
            assert!(a >= 0.0 && a <= 1.0,
                "Voice {} amplitude {} out of [0,1]", i, a);
        }
    }
}

