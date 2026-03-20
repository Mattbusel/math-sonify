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
