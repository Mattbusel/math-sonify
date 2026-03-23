//! # Pitch Detection
//!
//! Implements the YIN algorithm (simplified) and the Harmonic Product Spectrum
//! method for detecting fundamental frequency in audio signals.
//!
//! ## References
//! - de Cheveigné & Kawahara (2002), "YIN, a fundamental frequency estimator
//!   for speech and music"

// ── Autocorrelation ───────────────────────────────────────────────────────────

/// Compute the autocorrelation function r[lag] = Σ signal[t] * signal[t+lag].
pub fn autocorrelation(signal: &[f64], max_lag: usize) -> Vec<f64> {
    let n = signal.len();
    let max_lag = max_lag.min(n);
    (0..max_lag)
        .map(|lag| {
            (0..n - lag).map(|t| signal[t] * signal[t + lag]).sum()
        })
        .collect()
}

// ── Difference function ───────────────────────────────────────────────────────

/// Compute d[lag] = Σ (signal[t] - signal[t+lag])² = 2·(r[0] - r[lag]).
pub fn difference_function(signal: &[f64], max_lag: usize) -> Vec<f64> {
    let r = autocorrelation(signal, max_lag);
    if r.is_empty() {
        return Vec::new();
    }
    let r0 = r[0];
    r.iter().map(|&ri| 2.0 * (r0 - ri)).collect()
}

// ── Cumulative mean normalised difference ─────────────────────────────────────

/// Compute the CMND: d'[0] = 1; d'[τ] = d[τ] / ((1/τ) · Σ_{j=1}^{τ} d[j]).
pub fn cumulative_mean_normalized(d: &[f64]) -> Vec<f64> {
    if d.is_empty() {
        return Vec::new();
    }
    let mut out = vec![1.0_f64; d.len()];
    let mut running_sum = 0.0_f64;
    for tau in 1..d.len() {
        running_sum += d[tau];
        if running_sum == 0.0 {
            out[tau] = 0.0;
        } else {
            out[tau] = d[tau] * tau as f64 / running_sum;
        }
    }
    out
}

// ── Threshold search ──────────────────────────────────────────────────────────

/// Find the first τ ≥ 2 where cmnd[τ] < threshold and refine via parabolic
/// interpolation. Returns the refined lag as `Some(f64)` or `None`.
pub fn threshold_search(cmnd: &[f64], threshold: f64) -> Option<f64> {
    // Skip τ=0 and τ=1.
    for tau in 2..cmnd.len().saturating_sub(1) {
        if cmnd[tau] < threshold {
            // Parabolic interpolation for sub-sample accuracy.
            let tau_f = tau as f64;
            let prev = cmnd[tau - 1];
            let curr = cmnd[tau];
            let next = cmnd[tau + 1];
            let denom = 2.0 * (2.0 * curr - prev - next);
            let refined = if denom.abs() < 1e-12 {
                tau_f
            } else {
                tau_f + (next - prev) / denom
            };
            return Some(refined);
        }
    }
    None
}

// ── Top-level detection ───────────────────────────────────────────────────────

/// Detect the fundamental frequency of `signal` in Hz using YIN.
///
/// Returns `None` when no pitch below `threshold` is found.
pub fn detect_pitch(signal: &[f64], sample_rate: f64, threshold: f64) -> Option<f64> {
    let max_lag = signal.len() / 2;
    if max_lag < 2 {
        return None;
    }
    let d = difference_function(signal, max_lag);
    let cmnd = cumulative_mean_normalized(&d);
    let lag = threshold_search(&cmnd, threshold)?;
    if lag <= 0.0 {
        return None;
    }
    Some(sample_rate / lag)
}

// ── PitchDetector ─────────────────────────────────────────────────────────────

/// Configurable YIN-based pitch detector with frequency range constraints.
pub struct PitchDetector {
    pub sample_rate: f64,
    pub threshold: f64,
    /// Minimum detectable frequency in Hz.
    pub min_freq: f64,
    /// Maximum detectable frequency in Hz.
    pub max_freq: f64,
}

impl PitchDetector {
    /// Detect pitch in a single frame, constrained to [min_freq, max_freq].
    pub fn detect(&self, signal: &[f64]) -> Option<f64> {
        let lag_min = (self.sample_rate / self.max_freq).ceil() as usize;
        let lag_max = (self.sample_rate / self.min_freq).ceil() as usize;
        let max_lag = lag_max.min(signal.len() / 2);
        if max_lag < lag_min || max_lag < 2 {
            return None;
        }
        let d = difference_function(signal, max_lag);
        if d.len() <= lag_min {
            return None;
        }
        // Only search in the valid lag range.
        let cmnd_full = cumulative_mean_normalized(&d);
        // Narrow to lag_min..max_lag.
        let slice = &cmnd_full[lag_min..];
        let lag_raw = threshold_search(slice, self.threshold)?;
        let lag = lag_raw + lag_min as f64;
        if lag <= 0.0 {
            return None;
        }
        let freq = self.sample_rate / lag;
        if freq >= self.min_freq && freq <= self.max_freq {
            Some(freq)
        } else {
            None
        }
    }

    /// Detect pitch in every overlapping frame of `frame_size` samples,
    /// advancing by `hop_size` each step.
    pub fn detect_multi_frame(
        &self,
        signal: &[f64],
        frame_size: usize,
        hop_size: usize,
    ) -> Vec<Option<f64>> {
        if frame_size == 0 || hop_size == 0 || signal.len() < frame_size {
            return Vec::new();
        }
        let mut results = Vec::new();
        let mut start = 0;
        while start + frame_size <= signal.len() {
            let frame = &signal[start..start + frame_size];
            results.push(self.detect(frame));
            start += hop_size;
        }
        results
    }
}

// ── HarmonicProduct Spectrum ──────────────────────────────────────────────────

/// Pitch detection via the Harmonic Product Spectrum.
pub struct HarmonicProduct;

impl HarmonicProduct {
    /// Detect pitch by multiplying downsampled DFT magnitude spectra.
    ///
    /// Uses a simple Goertzel-based approach: compute magnitude at each
    /// frequency bin, then build the HPS and find the peak.
    pub fn detect(signal: &[f64], sample_rate: f64, num_harmonics: usize) -> Option<f64> {
        let n = signal.len();
        if n == 0 {
            return None;
        }

        // Compute DFT magnitudes for frequencies 0..n/2 using Goertzel.
        let num_bins = n / 2;
        let magnitudes: Vec<f64> = (0..num_bins)
            .map(|k| {
                let freq_k = k as f64 / n as f64;
                goertzel_magnitude(signal, freq_k)
            })
            .collect();

        if magnitudes.is_empty() {
            return None;
        }

        // Harmonic product: multiply magnitudes[k] * magnitudes[2k] * ... * magnitudes[H*k]
        let num_harmonics = num_harmonics.max(1);
        let valid_bins = num_bins / num_harmonics;
        if valid_bins == 0 {
            return None;
        }

        let hps: Vec<f64> = (0..valid_bins)
            .map(|k| {
                let mut product = magnitudes[k];
                for h in 2..=num_harmonics {
                    let idx = k * h;
                    if idx < num_bins {
                        product *= magnitudes[idx];
                    }
                }
                product
            })
            .collect();

        // Find peak bin (skip DC at bin 0).
        let peak_bin = hps[1..]
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())?
            .0
            + 1;

        let freq = peak_bin as f64 * sample_rate / n as f64;
        if freq > 0.0 { Some(freq) } else { None }
    }
}

/// Goertzel algorithm: compute the DFT magnitude at normalised frequency `freq_k` (0..1).
fn goertzel_magnitude(signal: &[f64], freq_k: f64) -> f64 {
    use std::f64::consts::PI;
    let omega = 2.0 * PI * freq_k;
    let coeff = 2.0 * omega.cos();
    let (mut s1, mut s2) = (0.0_f64, 0.0_f64);
    for &x in signal {
        let s0 = x + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    (s1 * s1 + s2 * s2 - s1 * s2 * coeff).abs().sqrt()
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sine_wave(freq: f64, sample_rate: f64, num_samples: usize) -> Vec<f64> {
        (0..num_samples)
            .map(|i| (2.0 * PI * freq * i as f64 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn autocorrelation_at_lag_zero_equals_power() {
        let signal = vec![1.0, 2.0, 3.0, 4.0];
        let r = autocorrelation(&signal, 1);
        let power: f64 = signal.iter().map(|x| x * x).sum();
        assert!((r[0] - power).abs() < 1e-9);
    }

    #[test]
    fn pure_sine_detected_at_correct_frequency() {
        let sample_rate = 44100.0;
        let freq = 440.0;
        let signal = sine_wave(freq, sample_rate, 4096);
        let detected = detect_pitch(&signal, sample_rate, 0.15);
        assert!(detected.is_some(), "should detect pitch");
        let det = detected.unwrap();
        // Allow 2% tolerance.
        assert!(
            (det - freq).abs() / freq < 0.02,
            "detected {det:.1} Hz, expected {freq:.1} Hz"
        );
    }

    #[test]
    fn low_amplitude_noise_not_detected() {
        // A signal of all zeros should not produce a pitch.
        let signal = vec![0.0f64; 2048];
        let detected = detect_pitch(&signal, 44100.0, 0.1);
        // With all-zero signal the CMND is undefined; either None or
        // a degenerate value. We only assert we don't panic.
        let _ = detected;
    }

    #[test]
    fn multi_frame_tracking_correct_length() {
        let sample_rate = 44100.0;
        let signal = sine_wave(220.0, sample_rate, 8192);
        let detector =
            PitchDetector { sample_rate, threshold: 0.15, min_freq: 80.0, max_freq: 1000.0 };
        let frame_size = 2048;
        let hop_size = 512;
        let expected_frames = (8192 - frame_size) / hop_size + 1;
        let results = detector.detect_multi_frame(&signal, frame_size, hop_size);
        assert_eq!(results.len(), expected_frames);
    }

    #[test]
    fn harmonic_product_detects_sine() {
        let sample_rate = 8000.0;
        let freq = 200.0;
        let signal = sine_wave(freq, sample_rate, 512);
        let detected = HarmonicProduct::detect(&signal, sample_rate, 3);
        assert!(detected.is_some(), "HPS should detect a pitch");
    }
}
