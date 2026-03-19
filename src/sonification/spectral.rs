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
}

impl SpectralMapping {
    pub fn new() -> Self {
        Self {
            smoothed: [0.0; NUM_PARTIALS],
            alpha: 0.05,
            history: VecDeque::with_capacity(HISTORY_LEN + 1),
        }
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

        let mut raw = [0.0f32; NUM_PARTIALS];

        if self.history.len() >= 32 {
            let hn = self.history.len();
            let n_dims = state.len().min(3);
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

        // Smooth to prevent clicks
        for (k, s) in self.smoothed.iter_mut().enumerate() {
            *s += self.alpha * (raw[k] - *s);
        }

        let base = config.base_frequency as f32;
        // Gain scales with the number of active (non-zero) partials so perceived
        // loudness stays consistent whether 2 or 32 bins are populated.
        let active_partials = self.smoothed.iter().filter(|&&v| v > 0.01).count().max(1);
        let gain = (1.0 / (active_partials as f32).sqrt()).clamp(0.08, 0.5);
        let mut p = AudioParams {
            mode: SonifMode::Spectral,
            partials: self.smoothed,
            partials_base_freq: base,
            gain,
            filter_cutoff: 8000.0,
            filter_q: 0.7,
            ..Default::default()
        };
        p.chaos_level = 0.3;
        p
    }
}
