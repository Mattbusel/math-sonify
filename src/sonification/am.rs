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
