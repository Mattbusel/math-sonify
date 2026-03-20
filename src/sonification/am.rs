use super::{quantize_to_scale, AudioParams, Scale, SonifMode, Sonification};
use crate::config::SonificationConfig;

/// AM synthesis mapping: attractor state drives carrier frequency, AM depth, and modulator ratio.
///
/// The first state variable is quantized to the configured scale to set the carrier.
/// The second variable controls AM depth (0..1).
/// The modulator-to-carrier ratio is derived from chaos level (1.0 at order, 2.0 at chaos).
pub struct AmMapping;

impl AmMapping {
    /// Creates a new `AmMapping` (stateless; no initialization required).
    pub fn new() -> Self {
        Self
    }
}

impl Default for AmMapping {
    fn default() -> Self {
        Self::new()
    }
}

impl Sonification for AmMapping {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let mut params = AudioParams::default();
        params.mode = SonifMode::AM;

        let scale = Scale::from(config.scale.as_str());
        let base_hz = config.base_frequency as f32;
        let octave_range = config.octave_range as f32;

        // Use first state dimension to determine carrier frequency (same normalization as FM)
        let norm0 = if !state.is_empty() {
            let v = state[0] as f32;
            (v / 30.0).tanh() * 0.5 + 0.5
        } else {
            0.5
        };

        let carrier_freq = quantize_to_scale(norm0, base_hz, octave_range, scale);

        // AM depth from second state dimension, normalized to [0..1]
        let am_depth = if state.len() > 1 {
            let v = state[1] as f32;
            (v / 30.0).tanh() * 0.5 + 0.5
        } else {
            0.5
        };

        // Chaos estimate from state magnitude
        let chaos = {
            let mag: f64 = state.iter().take(3).map(|v| v * v).sum::<f64>().sqrt();
            ((mag / 50.0) as f32).clamp(0.0, 1.0)
        };

        // Modulator ratio: 1.0 at order, 2.0 at chaos
        let mod_ratio = 1.0 + chaos;

        // Use fm_mod_index for AM depth, fm_ratio for modulator ratio
        params.fm_carrier_freq = carrier_freq;
        params.fm_mod_ratio = mod_ratio;
        params.fm_mod_index = am_depth;
        params.gain = 0.5;
        params.chaos_level = chaos;

        // Voice 0: primary carrier
        params.freqs[0] = carrier_freq;
        params.amps[0] = 0.8;

        // Higher-dimension voice distribution: voices 1-3 use state dims 1-3
        // with the same tanh normalisation, giving systems with many state
        // variables (Kuramoto, Lorenz96) more timbral richness in AM mode.
        for i in 1..4.min(state.len()) {
            let norm_i = (state[i] as f32 / 30.0).tanh() * 0.5 + 0.5;
            params.freqs[i] = super::quantize_to_scale(norm_i, base_hz, octave_range, scale);
            params.amps[i] = 0.4; // quieter secondary voices to avoid clipping
        }

        let _ = speed; // speed not used directly but available for future use

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
    fn test_am_mapping_output_finite() {
        let mut m = AmMapping::new();
        let state = vec![1.0, 2.0, 3.0];
        let p = m.map(&state, 10.0, &default_config());
        assert!(p.freqs[0].is_finite());
        assert!(p.gain.is_finite());
        assert_eq!(p.mode, SonifMode::AM);
    }

    #[test]
    fn test_am_mapping_empty_state() {
        let mut m = AmMapping::new();
        let p = m.map(&[], 0.0, &default_config());
        assert!(p.freqs[0].is_finite());
    }

    #[test]
    fn test_am_mapping_carrier_in_range() {
        let mut m = AmMapping::new();
        let config = default_config();
        let base = config.base_frequency as f32;
        let state = vec![5.0, 1.0, -3.0];
        let p = m.map(&state, 5.0, &config);
        assert!(p.fm_carrier_freq >= base * 0.25, "carrier below base/4");
        assert!(p.fm_carrier_freq <= base * 32.0, "carrier above base*32");
    }

    #[test]
    fn test_am_mod_ratio_positive() {
        let mut m = AmMapping::new();
        let p = m.map(&[10.0, 5.0, 2.0], 20.0, &default_config());
        assert!(p.fm_mod_ratio >= 1.0, "mod_ratio should be >= 1");
        assert!(p.fm_mod_ratio <= 2.1, "mod_ratio should be <= 2.1");
    }

    #[test]
    fn test_am_chaos_level_clamped() {
        // Very large state magnitude → chaos should be clamped to [0, 1]
        let mut m = AmMapping::new();
        let p = m.map(&[1000.0, 1000.0, 1000.0], 50.0, &default_config());
        assert!(p.chaos_level >= 0.0 && p.chaos_level <= 1.0,
            "chaos_level {} out of [0,1]", p.chaos_level);
        assert_eq!(p.chaos_level, 1.0, "large magnitude should saturate chaos to 1.0");
    }

    #[test]
    fn test_am_secondary_voices_populated() {
        // 4-dimensional state should fill freqs[0..3] with finite, positive values
        let mut m = AmMapping::new();
        let p = m.map(&[1.0, 5.0, -3.0, 8.0], 10.0, &default_config());
        for i in 0..4 {
            assert!(p.freqs[i].is_finite() && p.freqs[i] > 0.0,
                "freqs[{}] should be finite positive: {}", i, p.freqs[i]);
        }
    }

    #[test]
    fn test_am_depth_in_range() {
        // AM depth (stored in fm_mod_index) is derived from tanh → should be in [0, 1]
        let mut m = AmMapping::new();
        for v in [-100.0, -10.0, 0.0, 10.0, 100.0_f64] {
            let p = m.map(&[1.0, v, 0.0], 5.0, &default_config());
            assert!(p.fm_mod_index >= 0.0 && p.fm_mod_index <= 1.0,
                "fm_mod_index {} out of [0,1] for state[1]={}", p.fm_mod_index, v);
        }
    }
}
