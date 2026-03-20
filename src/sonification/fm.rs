use super::{quantize_to_scale, AudioParams, Scale, SonifMode, Sonification};
use crate::config::SonificationConfig;

/// FM synthesis mapping: attractor state drives carrier frequency, modulator ratio, and index.
///
/// The first state variable is quantized to the configured scale to set the carrier.
/// The second variable controls the modulator-to-carrier ratio (1..8).
/// Trajectory speed modulates the FM index, producing brighter timbre during chaotic bursts.
pub struct FmMapping;

impl FmMapping {
    /// Creates a new `FmMapping` (stateless; no initialization required).
    pub fn new() -> Self {
        Self
    }
}

impl Default for FmMapping {
    fn default() -> Self {
        Self::new()
    }
}

impl Sonification for FmMapping {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let mut params = AudioParams::default();
        params.mode = SonifMode::FM;

        let scale = Scale::from(config.scale.as_str());
        let base_hz = config.base_frequency as f32;
        let octave_range = config.octave_range as f32;

        // Use first state dimension to determine carrier frequency
        let norm0 = if !state.is_empty() {
            // tanh-based soft normalisation: smoothly maps any real value to [0,1]
            // without hard-clipping at ±30, so attractors with large state ranges
            // (three-body, Lorenz ρ=100) still produce musical frequency sweeps.
            let v = state[0] as f32;
            (v / 30.0).tanh() * 0.5 + 0.5
        } else {
            0.5
        };

        let carrier_freq = quantize_to_scale(norm0, base_hz, octave_range, scale);

        // Mod ratio from second state dimension.
        // Quantize to integer harmonic ratios for musical FM spectra.
        // Continuous float ratios produce inharmonic beating; integer ratios
        // produce stable, recognizable timbres (octaves, fifths, thirds, etc.).
        const HARMONIC_RATIOS: [f32; 8] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mod_ratio = if state.len() > 1 {
            // tanh maps state[1] to (-1,1), shift+scale → index in 0..8
            let norm = (state[1] as f32 / 10.0).tanh() * 0.5 + 0.5; // [0, 1)
            let idx = (norm * HARMONIC_RATIOS.len() as f32) as usize;
            HARMONIC_RATIOS[idx.min(HARMONIC_RATIOS.len() - 1)]
        } else {
            2.0
        };

        // Chaos estimate from state magnitude
        let chaos = {
            let mag: f64 = state.iter().take(3).map(|v| v * v).sum::<f64>().sqrt();
            ((mag / 50.0) as f32).clamp(0.0, 1.0)
        };

        // Mod index based on speed and chaos
        let mut mod_index = (speed as f32 / 50.0).clamp(0.1, 20.0) * chaos.max(0.1);

        // Z-dimension (state[2]) drives FM mod index as a feedback-like offset,
        // adding timbral depth when the third state variable is available.
        if state.len() >= 3 {
            mod_index += (state[2].tanh() * 0.3) as f32;
            mod_index = mod_index.max(0.1);
        }

        params.fm_carrier_freq = carrier_freq;
        params.fm_mod_ratio = mod_ratio;
        params.fm_mod_index = mod_index;
        params.gain = 0.5;
        params.chaos_level = chaos;

        // Higher-dimension voice distribution: voices 1-3 use state dims 1-3
        // with tanh normalisation matching voice 0.
        params.freqs[0] = carrier_freq;
        params.amps[0] = 0.8;
        for i in 1..4.min(state.len()) {
            let norm_i = (state[i] as f32 / 30.0).tanh() * 0.5 + 0.5;
            params.freqs[i] = super::quantize_to_scale(
                norm_i,
                base_hz,
                octave_range,
                scale,
            );
            params.amps[i] = 0.4; // quieter secondary voices
        }

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
    fn test_fm_mapping_output_finite() {
        let mut m = FmMapping::new();
        let p = m.map(&[1.0, 2.0, 3.0], 10.0, &default_config());
        assert!(p.fm_carrier_freq.is_finite());
        assert!(p.fm_mod_index.is_finite());
        assert_eq!(p.mode, SonifMode::FM);
    }

    #[test]
    fn test_fm_mapping_empty_state() {
        let mut m = FmMapping::new();
        let p = m.map(&[], 0.0, &default_config());
        assert!(p.fm_carrier_freq.is_finite());
    }

    #[test]
    fn test_fm_mod_ratio_is_integer() {
        // With harmonic ratio quantization, fm_mod_ratio should be an integer 1..8
        let mut m = FmMapping::new();
        let p = m.map(&[0.0, 5.0, 1.0], 20.0, &default_config());
        let ratio = p.fm_mod_ratio;
        assert!(ratio >= 1.0 && ratio <= 8.0, "mod ratio {} out of range", ratio);
        assert!((ratio.round() - ratio).abs() < 0.01, "mod ratio should be integer: {}", ratio);
    }

    #[test]
    fn test_fm_mod_index_positive() {
        let mut m = FmMapping::new();
        let p = m.map(&[1.0, 2.0, 3.0], 50.0, &default_config());
        assert!(p.fm_mod_index > 0.0, "mod_index should be positive");
    }

    #[test]
    fn test_fm_higher_speed_increases_mod_index() {
        // Higher trajectory speed should produce a larger FM mod index (brighter timbre)
        let mut m_slow = FmMapping::new();
        let mut m_fast = FmMapping::new();
        let state = vec![1.0, 2.0, 1.0];
        let p_slow = m_slow.map(&state, 1.0, &default_config());
        let p_fast = m_fast.map(&state, 100.0, &default_config());
        assert!(
            p_fast.fm_mod_index > p_slow.fm_mod_index,
            "higher speed should increase mod_index: slow={}, fast={}",
            p_slow.fm_mod_index, p_fast.fm_mod_index
        );
    }

    #[test]
    fn test_fm_z_dim_affects_mod_index() {
        // Adding a z-dimension (state[2]) should change the mod index
        let mut m_2d = FmMapping::new();
        let mut m_3d = FmMapping::new();
        let speed = 30.0;
        let p_2d = m_2d.map(&[5.0, 3.0], speed, &default_config());
        let p_3d = m_3d.map(&[5.0, 3.0, 10.0], speed, &default_config());
        // With z=10 (tanh contribution = ~0.3), mod_index should differ
        assert!(
            (p_3d.fm_mod_index - p_2d.fm_mod_index).abs() > 0.01,
            "z-dim should change mod_index: 2d={}, 3d={}",
            p_2d.fm_mod_index, p_3d.fm_mod_index
        );
    }

    #[test]
    fn test_fm_carrier_matches_freqs0() {
        // fm_carrier_freq and freqs[0] should be the same value
        let mut m = FmMapping::new();
        let p = m.map(&[3.0, 1.5, 0.5], 20.0, &default_config());
        assert_eq!(
            p.fm_carrier_freq, p.freqs[0],
            "fm_carrier_freq should equal freqs[0]: {} vs {}",
            p.fm_carrier_freq, p.freqs[0]
        );
    }
}
