use crate::config::SonificationConfig;
use super::{AudioParams, Sonification, SonifMode, quantize_to_scale, Scale};

pub struct FmMapping;

impl FmMapping {
    pub fn new() -> Self { Self }
}

impl Sonification for FmMapping {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let mut params = AudioParams::default();
        params.mode = SonifMode::FM;

        let scale = Scale::from(config.scale.as_str());
        let base_hz = config.base_frequency as f32;
        let octave_range = config.octave_range as f32;

        // Use first state dimension to determine carrier frequency
        let norm0 = if state.len() > 0 {
            // tanh-based soft normalisation: smoothly maps any real value to [0,1]
            // without hard-clipping at ±30, so attractors with large state ranges
            // (three-body, Lorenz ρ=100) still produce musical frequency sweeps.
            let v = state[0] as f32;
            (v / 30.0).tanh() * 0.5 + 0.5
        } else { 0.5 };

        let carrier_freq = quantize_to_scale(norm0, base_hz, octave_range, scale);

        // Mod ratio from second state dimension
        // Bound mod_ratio to musical range [1.0, 7.0]
        let mod_ratio = if state.len() > 1 {
            1.0 + (state[1].abs() as f32 % 6.0)
        } else { 2.0 };

        // Chaos estimate from state magnitude
        let chaos = {
            let mag: f64 = state.iter().take(3).map(|v| v * v).sum::<f64>().sqrt();
            ((mag / 50.0) as f32).clamp(0.0, 1.0)
        };

        // Mod index based on speed and chaos
        let mod_index = (speed as f32 / 50.0).clamp(0.1, 20.0) * chaos.max(0.1);

        params.fm_carrier_freq = carrier_freq;
        params.fm_mod_ratio = mod_ratio;
        params.fm_mod_index = mod_index;
        params.gain = 0.5;
        params.chaos_level = chaos;

        // Also set freqs[0] for display purposes
        params.freqs[0] = carrier_freq;
        params.amps[0] = 0.8;

        params
    }
}
