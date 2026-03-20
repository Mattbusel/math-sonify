//! Waveguide sonification mapper.
//!
//! Maps dynamical-system state variables to waveguide physical model parameters,
//! driving [`crate::synth::WaveguideString`] via [`AudioParams`].

use super::{AudioParams, SonifMode, Sonification};
use crate::config::SonificationConfig;

/// Sonification mapper that drives the waveguide physical model.
///
/// - `state[0]` normalized → waveguide tension (0.1..2.0, remapped to the 0..1 range the
///   audio thread expects and the WaveguideString applies via exponential scaling).
/// - `state[1]` normalized → damping (0.001..0.1 mapped to the feedback coefficient range
///   0.9..0.999 that `WaveguideString` uses internally).
/// - Attractor energy/speed → excitation amount (controls gain).
pub struct WaveguideMapping {
    min: Vec<f64>,
    max: Vec<f64>,
    alpha: f64, // exponential moving average decay for min/max tracking
    excite_cooldown: u32,
}

impl WaveguideMapping {
    pub fn new() -> Self {
        Self {
            min: Vec::new(),
            max: Vec::new(),
            alpha: 0.002,
            excite_cooldown: 0,
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
                ((v - self.min[i]) / range).clamp(0.0, 1.0) as f32
            })
            .collect()
    }
}

impl Default for WaveguideMapping {
    fn default() -> Self {
        Self::new()
    }
}

impl Sonification for WaveguideMapping {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let norm = self.normalize(state);

        // state[0] → tension: normalized 0..1 maps directly (audio thread applies exp scaling).
        let tension = norm.first().copied().unwrap_or(0.5);

        // state[1] → damping feedback coefficient: 0..1 norm → 0.90..0.999
        let damping_norm = if norm.len() >= 2 { norm[1] } else { 0.5 };
        let damping = 0.90 + damping_norm * 0.099; // 0.90..0.999

        // Energy metric: speed normalized to 0..1 (typical Lorenz range 0..200)
        let energy = (speed.abs() as f32 / 200.0).clamp(0.0, 1.0);

        // Excite on speed spikes (cooldown prevents double-triggers)
        let excite = if self.excite_cooldown == 0 && energy > 0.6 {
            self.excite_cooldown = 30; // ~250ms at 120 Hz
            true
        } else {
            if self.excite_cooldown > 0 {
                self.excite_cooldown -= 1;
            }
            false
        };

        // Base frequency from config; tension modulates via WaveguideString.set_freq
        let base_hz = config.base_frequency as f32;

        AudioParams {
            mode: SonifMode::Waveguide,
            gain: 0.5 + energy * 0.5,
            waveguide_tension: tension,
            waveguide_damping: damping,
            waveguide_excite: excite,
            ks_freq: base_hz.max(40.0),
            chaos_level: energy,
            filter_cutoff: 2000.0 + energy * 8000.0,
            filter_q: 0.7,
            ..Default::default()
        }
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
    fn test_waveguide_mapping_output_finite() {
        let mut m = WaveguideMapping::new();
        let p = m.map(&[1.0, 2.0, 3.0], 10.0, &default_config());
        assert!(p.waveguide_tension.is_finite());
        assert!(p.waveguide_damping.is_finite());
        assert!(p.gain.is_finite());
        assert_eq!(p.mode, SonifMode::Waveguide);
    }

    #[test]
    fn test_waveguide_damping_in_range() {
        let mut m = WaveguideMapping::new();
        let p = m.map(&[0.0, 1.0], 5.0, &default_config());
        assert!(p.waveguide_damping >= 0.90 && p.waveguide_damping <= 0.999,
            "damping {} out of [0.90, 0.999]", p.waveguide_damping);
    }

    #[test]
    fn test_waveguide_excite_triggers_on_speed_spike() {
        let mut m = WaveguideMapping::new();
        // Low speed: no excitation
        let p_low = m.map(&[1.0, 1.0], 0.0, &default_config());
        assert!(!p_low.waveguide_excite, "should not excite at low speed");
        // High speed: should excite
        let p_high = m.map(&[1.0, 1.0], 200.0, &default_config());
        assert!(p_high.waveguide_excite, "should excite at high speed");
    }

    #[test]
    fn test_waveguide_tension_in_range() {
        let mut m = WaveguideMapping::new();
        // After normalization adapts, tension should stay in [0, 1]
        for i in 0..20 {
            let p = m.map(&[i as f64 * 0.5, 0.0], 5.0, &default_config());
            assert!(p.waveguide_tension >= 0.0 && p.waveguide_tension <= 1.0,
                "tension {} out of [0,1]", p.waveguide_tension);
        }
    }
}
