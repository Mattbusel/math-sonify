use std::collections::VecDeque;
use super::{AudioParams, Sonification, SonifMode};
use crate::config::SonificationConfig;

const NUM_PARTIALS: usize = 32;
const HISTORY_LEN: usize = 64; // ~0.53s of trajectory at 120 Hz

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

    /// Compute DFT magnitude for bin `k` over `signal` (length N, zero-mean).
    fn dft_mag(signal: &[f64], k: usize) -> f32 {
        let n = signal.len() as f64;
        let twopi_k_over_n = std::f64::consts::TAU * k as f64 / n;
        let re: f64 = signal.iter().enumerate()
            .map(|(t, &v)| v * (twopi_k_over_n * t as f64).cos()).sum();
        let im: f64 = signal.iter().enumerate()
            .map(|(t, &v)| -v * (twopi_k_over_n * t as f64).sin()).sum();
        ((re * re + im * im) / n).sqrt() as f32
    }
}

impl Sonification for SpectralMapping {
    fn map(&mut self, state: &[f64], _speed: f64, config: &SonificationConfig) -> AudioParams {
        if state.is_empty() {
            return AudioParams { mode: SonifMode::Spectral, ..Default::default() };
        }

        self.history.push_back(state.to_vec());
        if self.history.len() > HISTORY_LEN {
            self.history.pop_front();
        }

        let mut raw = [0.0f32; NUM_PARTIALS];

        if self.history.len() >= 8 {
            let hn = self.history.len();
            let n_dims = state.len().min(3);
            let n_bins = NUM_PARTIALS.min(hn / 2 + 1);

            // Build zero-mean signals per dimension
            let signals: Vec<Vec<f64>> = (0..n_dims).map(|d| {
                let s: Vec<f64> = self.history.iter()
                    .map(|h| h.get(d).copied().unwrap_or(0.0)).collect();
                let mean = s.iter().sum::<f64>() / s.len() as f64;
                s.iter().map(|&v| v - mean).collect()
            }).collect();

            // Sum DFT magnitudes across all dimensions for each bin
            for k in 0..n_bins {
                raw[k] = signals.iter().map(|sig| Self::dft_mag(sig, k)).sum();
            }

            // Normalize to [0, 1]
            let max_val = raw.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
            for r in &mut raw { *r /= max_val; }
        } else {
            // Fallback while history fills: cyclic state mapping
            let max_abs = state.iter().map(|v| v.abs()).fold(0.0f64, f64::max).max(1e-9);
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
        let mut p = AudioParams {
            mode: SonifMode::Spectral,
            partials: self.smoothed,
            partials_base_freq: base,
            gain: 0.30, // was 0.15 — spectral DFT bins were normalized so output was 6 dB too quiet
            filter_cutoff: 8000.0,
            filter_q: 0.7,
            ..Default::default()
        };
        p.chaos_level = 0.3;
        p
    }
}
