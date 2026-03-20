use super::{AudioParams, SonifMode, Sonification};
use crate::config::SonificationConfig;
use std::collections::VecDeque;

const NUM_PARTIALS: usize = 32;
const HISTORY_LEN: usize = 256; // ~2.1s of trajectory at 120 Hz (4× improvement)

/// Spectral mode: trajectory history → DFT magnitudes → additive synthesis.
/// Each control tick, the latest state is pushed onto a ring buffer of length
/// HISTORY_LEN.  A real DFT is computed over that window for each state
/// dimension; the per-bin magnitudes sum across dimensions and drive the 32
/// additive partials.  While the buffer is filling the legacy cyclic mapping
/// is used as a fallback.
pub struct SpectralMapping {
    smoothed: [f32; NUM_PARTIALS],
    alpha: f32,
    history: VecDeque<Vec<f64>>,
    /// Cached DFT result — reused when state hash is unchanged.
    cached_raw: [f32; NUM_PARTIALS],
    /// Hash of the state vector on the previous call.
    prev_state_hash: u64,
}

impl Default for SpectralMapping {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectralMapping {
    pub fn new() -> Self {
        Self {
            smoothed: [0.0; NUM_PARTIALS],
            alpha: 0.05,
            history: VecDeque::with_capacity(HISTORY_LEN + 1),
            cached_raw: [0.0; NUM_PARTIALS],
            prev_state_hash: u64::MAX,
        }
    }

    /// Compute a simple hash of the state vector to detect changes.
    fn hash_state(state: &[f64]) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
        for &v in state {
            let bits = v.to_bits();
            h ^= bits;
            h = h.wrapping_mul(0x100000001b3); // FNV prime
        }
        h
    }

    /// Compute DFT magnitude for bin `k` over `signal` (length N, zero-mean)
    /// with a Hann window applied to reduce spectral leakage.
    ///
    /// Uses incremental angle accumulation (cos/sin recurrence) to avoid
    /// calling transcendental functions inside the inner loop — O(N) adds and
    /// multiplies rather than O(N) cos+sin calls.
    #[inline]
    fn dft_mag(signal: &[f64], k: usize) -> f32 {
        let n = signal.len();
        let nf = n as f64;
        let angle_step = std::f64::consts::TAU * k as f64 / nf;
        let (ca, sa) = (angle_step.cos(), angle_step.sin());
        let mut cos_t = 1.0f64;
        let mut sin_t = 0.0f64;
        let mut re = 0.0f64;
        let mut im = 0.0f64;
        for (i, &v) in signal.iter().enumerate() {
            // Hann window: w(i) = 0.5 * (1 − cos(2π·i/(N−1)))
            let w = 0.5 * (1.0 - (std::f64::consts::TAU * i as f64 / (nf - 1.0)).cos());
            let wv = v * w;
            re += wv * cos_t;
            im -= wv * sin_t;
            let new_cos = cos_t * ca - sin_t * sa;
            let new_sin = sin_t * ca + cos_t * sa;
            cos_t = new_cos;
            sin_t = new_sin;
        }
        // Normalise by N/2 (Hann window sum ≈ N/2)
        ((re * re + im * im) / (nf * nf / 4.0)).sqrt() as f32
    }
}

impl Sonification for SpectralMapping {
    fn map(&mut self, state: &[f64], _speed: f64, config: &SonificationConfig) -> AudioParams {
        if state.is_empty() {
            return AudioParams {
                mode: SonifMode::Spectral,
                ..Default::default()
            };
        }

        self.history.push_back(state.to_vec());
        if self.history.len() > HISTORY_LEN {
            self.history.pop_front();
        }

        // --- DFT caching: only recompute when the state vector has changed ----
        let state_hash = Self::hash_state(state);
        if state_hash != self.prev_state_hash {
            self.prev_state_hash = state_hash;

            let mut raw = [0.0f32; NUM_PARTIALS];

            if self.history.len() >= 32 {
                let hn = self.history.len();
                let n_dims = state.len().min(4);
                let n_bins = NUM_PARTIALS.min(hn / 2 + 1);

                // Build zero-mean signals per dimension
                let signals: Vec<Vec<f64>> = (0..n_dims)
                    .map(|d| {
                        let s: Vec<f64> = self
                            .history
                            .iter()
                            .map(|h| h.get(d).copied().unwrap_or(0.0))
                            .collect();
                        let mean = s.iter().sum::<f64>() / s.len() as f64;
                        s.iter().map(|&v| v - mean).collect()
                    })
                    .collect();

                // Sum DFT magnitudes across all dimensions for each bin
                for k in 0..n_bins {
                    raw[k] = signals.iter().map(|sig| Self::dft_mag(sig, k)).sum();
                }

                // Normalize to [0, 1]
                let max_val = raw.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
                for r in &mut raw {
                    *r /= max_val;
                }
            } else {
                // Fallback while history fills: cyclic state mapping
                let max_abs = state
                    .iter()
                    .map(|v| v.abs())
                    .fold(0.0f64, f64::max)
                    .max(1e-9);
                for (k, slot) in raw.iter_mut().enumerate() {
                    let i = k % state.len();
                    *slot = (state[i].abs() / max_abs) as f32;
                }
            }

            // Spectral roll-off: steeper natural rolloff (~-12 dB/oct)
            for (k, slot) in raw.iter_mut().enumerate() {
                *slot *= 1.0 / (1.0 + (k as f32).powf(1.5) * 0.08);
            }

            self.cached_raw = raw;
        }

        // Smooth to prevent clicks (always run, even on cache hit)
        for (k, s) in self.smoothed.iter_mut().enumerate() {
            *s += self.alpha * (self.cached_raw[k] - *s);
        }

        // Chaos estimate from state magnitude
        let chaos_level = {
            let mag: f64 = state.iter().take(3).map(|v| v * v).sum::<f64>().sqrt();
            ((mag / 50.0) as f32).clamp(0.0, 1.0)
        };

        let base = config.base_frequency as f32;

        // Buzz oscillator frequency with chaos shimmer: during chaotic regions
        // the excitation is slightly detuned, adding shimmer to the texture.
        let buzz_freq = base * (1.0 + chaos_level * 0.08);

        // Gain scales with the number of active (non-zero) partials so perceived
        // loudness stays consistent whether 2 or 32 bins are populated.
        let active_partials = self.smoothed.iter().filter(|&&v| v > 0.01).count().max(1);
        let gain = (1.0 / (active_partials as f32).sqrt()).clamp(0.08, 0.5);
        let mut p = AudioParams {
            mode: SonifMode::Spectral,
            partials: self.smoothed,
            partials_base_freq: buzz_freq,
            gain,
            filter_cutoff: 8000.0,
            filter_q: 0.7,
            ..Default::default()
        };
        p.chaos_level = chaos_level;
        p
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
    fn test_spectral_output_finite() {
        let mut m = SpectralMapping::new();
        let p = m.map(&[1.0, 2.0, 3.0], 10.0, &default_config());
        assert!(p.partials.iter().all(|v| v.is_finite()));
        assert!(p.gain.is_finite());
        assert_eq!(p.mode, SonifMode::Spectral);
    }

    #[test]
    fn test_spectral_empty_state() {
        let mut m = SpectralMapping::new();
        let p = m.map(&[], 0.0, &default_config());
        assert_eq!(p.mode, SonifMode::Spectral);
        assert!(p.gain.is_finite());
    }

    #[test]
    fn test_spectral_history_fills() {
        let mut m = SpectralMapping::new();
        // Feed 300 frames to fill the history buffer (>= 256 entries)
        for i in 0..300 {
            let state = vec![(i as f64 * 0.1).sin() * 10.0, (i as f64 * 0.07).cos() * 5.0];
            let p = m.map(&state, 5.0, &default_config());
            assert!(p.partials.iter().all(|v| v.is_finite()),
                "partials non-finite at step {}", i);
        }
    }

    #[test]
    fn test_spectral_partials_in_range() {
        let mut m = SpectralMapping::new();
        for i in 0..100 {
            let state = vec![(i as f64).sin() * 20.0];
            let p = m.map(&state, 10.0, &default_config());
            for (k, &v) in p.partials.iter().enumerate() {
                assert!(v >= 0.0, "partial {} has negative value {}", k, v);
            }
        }
    }
}
