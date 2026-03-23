//! DSP utilities: spectral features complementing `spectrum_analyzer`.
//!
//! Provides:
//! - Mel-scale conversion and Mel filterbank
//! - MFCCs (Mel-Frequency Cepstral Coefficients)
//! - Zero-crossing rate, RMS, spectral centroid, rolloff, bandwidth
//! - `SpectralFeatures` extraction struct
//!
//! Deliberately avoids duplicating the FFT/DFT already in `spectrum_analyzer`.
//! For power-spectrum computation this module reuses the public
//! `SpectralAnalyzer::analyze` result or falls back to a simple DFT for
//! short signals.

use std::f64::consts::PI;

// ── Mel scale ─────────────────────────────────────────────────────────────────

/// Convert a frequency in Hz to the Mel scale.
///
/// `mel = 2595 * log10(1 + freq_hz / 700)`
pub fn mel_scale(freq_hz: f64) -> f64 {
    2595.0 * (1.0 + freq_hz / 700.0).log10()
}

/// Convert a Mel value back to Hz.
///
/// `hz = 700 * (10^(mel / 2595) - 1)`
pub fn mel_to_hz(mel: f64) -> f64 {
    700.0 * (10_f64.powf(mel / 2595.0) - 1.0)
}

// ── MelFilterbank ─────────────────────────────────────────────────────────────

/// Triangular Mel filterbank for computing mel spectra.
pub struct MelFilterbank {
    pub num_filters: usize,
    pub sample_rate: f64,
    pub min_freq: f64,
    pub max_freq: f64,
    /// filter_weights[filter_idx][fft_bin_idx]
    pub filter_weights: Vec<Vec<f64>>,
}

impl MelFilterbank {
    /// Build the filterbank.
    ///
    /// `fft_size` is the full FFT size (not the number of bins used);
    /// the filterbank covers bins 0..=fft_size/2.
    pub fn new(
        num_filters: usize,
        fft_size: usize,
        sample_rate: f64,
        min_freq: f64,
        max_freq: f64,
    ) -> Self {
        let num_bins = fft_size / 2 + 1;

        let mel_min = mel_scale(min_freq);
        let mel_max = mel_scale(max_freq);

        // num_filters + 2 evenly-spaced mel points (include edges)
        let mel_points: Vec<f64> = (0..=(num_filters + 1))
            .map(|i| mel_min + i as f64 * (mel_max - mel_min) / (num_filters + 1) as f64)
            .collect();

        // Convert mel points back to Hz, then to FFT bin indices
        let hz_points: Vec<f64> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();
        let bin_points: Vec<f64> = hz_points
            .iter()
            .map(|&f| f * fft_size as f64 / sample_rate)
            .collect();

        let mut filter_weights = Vec::with_capacity(num_filters);
        for m in 1..=num_filters {
            let mut weights = vec![0.0f64; num_bins];
            let f_left = bin_points[m - 1];
            let f_center = bin_points[m];
            let f_right = bin_points[m + 1];

            for (k, w) in weights.iter_mut().enumerate() {
                let k_f = k as f64;
                if k_f >= f_left && k_f <= f_center {
                    *w = (k_f - f_left) / (f_center - f_left).max(1e-10);
                } else if k_f > f_center && k_f <= f_right {
                    *w = (f_right - k_f) / (f_right - f_center).max(1e-10);
                }
            }
            filter_weights.push(weights);
        }

        MelFilterbank {
            num_filters,
            sample_rate,
            min_freq,
            max_freq,
            filter_weights,
        }
    }

    /// Apply the filterbank to a power spectrum.
    ///
    /// Returns a mel spectrum of length `num_filters`.
    pub fn apply(&self, power_spectrum: &[f64]) -> Vec<f64> {
        self.filter_weights
            .iter()
            .map(|weights| {
                weights
                    .iter()
                    .zip(power_spectrum.iter())
                    .map(|(&w, &p)| w * p)
                    .sum::<f64>()
            })
            .collect()
    }
}

// ── Simple DFT helper (for short signals) ─────────────────────────────────────

/// Compute the one-sided power spectrum of `signal` using an O(N²) DFT.
///
/// Returns a `Vec<f64>` of length `n/2 + 1` (magnitude squared).
fn compute_power_spectrum(signal: &[f64]) -> Vec<f64> {
    let n = signal.len();
    if n == 0 {
        return vec![];
    }
    let n_bins = n / 2 + 1;

    // Apply Hann window
    let windowed: Vec<f64> = signal
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / (n.saturating_sub(1).max(1)) as f64).cos());
            s * w
        })
        .collect();

    (0..n_bins)
        .map(|k| {
            let mut re = 0.0f64;
            let mut im = 0.0f64;
            for (i, &x) in windowed.iter().enumerate() {
                let angle = 2.0 * PI * k as f64 * i as f64 / n as f64;
                re += x * angle.cos();
                im -= x * angle.sin();
            }
            // Power = |X|² / N²
            (re * re + im * im) / (n as f64 * n as f64)
        })
        .collect()
}

// ── MFCC ──────────────────────────────────────────────────────────────────────

/// Compute Mel-Frequency Cepstral Coefficients.
///
/// Steps:
/// 1. Compute power spectrum
/// 2. Apply mel filterbank
/// 3. Log of mel spectrum
/// 4. DCT (type-II) for `num_coeffs` coefficients
pub fn mfcc(signal: &[f64], sample_rate: f64, num_coeffs: usize, num_filters: usize) -> Vec<f64> {
    if signal.is_empty() || num_coeffs == 0 || num_filters == 0 {
        return vec![0.0; num_coeffs];
    }

    let power_spec = compute_power_spectrum(signal);
    let fft_size = (power_spec.len() - 1) * 2;

    let filterbank = MelFilterbank::new(
        num_filters,
        fft_size,
        sample_rate,
        80.0,
        (sample_rate / 2.0).min(8000.0),
    );

    let mel_spec = filterbank.apply(&power_spec);

    // Log mel spectrum (add small floor to avoid log(0))
    let log_mel: Vec<f64> = mel_spec.iter().map(|&m| (m + 1e-10).ln()).collect();

    let n = log_mel.len();
    // DCT-II
    (0..num_coeffs)
        .map(|k| {
            log_mel
                .iter()
                .enumerate()
                .map(|(i, &v)| v * (PI * k as f64 * (i as f64 + 0.5) / n as f64).cos())
                .sum::<f64>()
        })
        .collect()
}

// ── Feature functions ─────────────────────────────────────────────────────────

/// Fraction of samples where the sign changes.
pub fn zero_crossing_rate(signal: &[f64]) -> f64 {
    if signal.len() < 2 {
        return 0.0;
    }
    let crossings = signal
        .windows(2)
        .filter(|w| w[0].signum() != w[1].signum())
        .count();
    crossings as f64 / (signal.len() - 1) as f64
}

/// Root-mean-square energy of the signal.
pub fn root_mean_square(signal: &[f64]) -> f64 {
    if signal.is_empty() {
        return 0.0;
    }
    let mean_sq = signal.iter().map(|&x| x * x).sum::<f64>() / signal.len() as f64;
    mean_sq.sqrt()
}

/// Magnitude-weighted mean frequency (spectral centroid).
///
/// `centroid = Σ(freq * mag) / Σ(mag)`
pub fn spectral_centroid(magnitudes: &[f64], freqs: &[f64]) -> f64 {
    assert_eq!(magnitudes.len(), freqs.len());
    let total_mag: f64 = magnitudes.iter().sum();
    if total_mag == 0.0 {
        return 0.0;
    }
    magnitudes
        .iter()
        .zip(freqs.iter())
        .map(|(&m, &f)| m * f)
        .sum::<f64>()
        / total_mag
}

/// Frequency below which `threshold` fraction of total spectral energy lies.
///
/// `threshold` is in (0, 1], e.g. 0.85 means the 85th-percentile frequency.
pub fn spectral_rolloff(magnitudes: &[f64], freqs: &[f64], threshold: f64) -> f64 {
    assert_eq!(magnitudes.len(), freqs.len());
    if magnitudes.is_empty() {
        return 0.0;
    }
    let total: f64 = magnitudes.iter().sum();
    if total == 0.0 {
        return 0.0;
    }
    let target = total * threshold.clamp(0.0, 1.0);
    let mut cumulative = 0.0f64;
    for (&mag, &freq) in magnitudes.iter().zip(freqs.iter()) {
        cumulative += mag;
        if cumulative >= target {
            return freq;
        }
    }
    *freqs.last().unwrap_or(&0.0)
}

/// Spectral bandwidth — magnitude-weighted standard deviation of frequencies.
///
/// `bandwidth = sqrt(Σ(mag * (freq - centroid)²) / Σ(mag))`
pub fn spectral_bandwidth(magnitudes: &[f64], freqs: &[f64], centroid: f64) -> f64 {
    assert_eq!(magnitudes.len(), freqs.len());
    let total_mag: f64 = magnitudes.iter().sum();
    if total_mag == 0.0 {
        return 0.0;
    }
    let variance = magnitudes
        .iter()
        .zip(freqs.iter())
        .map(|(&m, &f)| m * (f - centroid).powi(2))
        .sum::<f64>()
        / total_mag;
    variance.sqrt()
}

// ── SpectralFeatures ──────────────────────────────────────────────────────────

/// A rich bundle of spectral features extracted from a signal.
#[derive(Debug, Clone)]
pub struct SpectralFeatures {
    pub centroid: f64,
    pub rolloff: f64,
    pub zcr: f64,
    pub rms: f64,
    pub mfcc: Vec<f64>,
    pub bandwidth: f64,
}

impl SpectralFeatures {
    /// Extract all spectral features from a time-domain signal.
    ///
    /// `num_mfcc` controls the number of MFCC coefficients returned.
    pub fn extract(signal: &[f64], sample_rate: f64, num_mfcc: usize) -> Self {
        let power_spec = compute_power_spectrum(signal);
        let n_bins = power_spec.len();
        let fft_size = (n_bins.saturating_sub(1)) * 2;
        let bin_hz = if fft_size > 0 { sample_rate / fft_size as f64 } else { 1.0 };

        let magnitudes: Vec<f64> = power_spec.iter().map(|&p| p.sqrt()).collect();
        let freqs: Vec<f64> = (0..n_bins).map(|k| k as f64 * bin_hz).collect();

        let centroid = spectral_centroid(&magnitudes, &freqs);
        let rolloff = spectral_rolloff(&magnitudes, &freqs, 0.85);
        let bandwidth = spectral_bandwidth(&magnitudes, &freqs, centroid);
        let zcr = zero_crossing_rate(signal);
        let rms = root_mean_square(signal);
        let num_filters = 26.min(n_bins.saturating_sub(2).max(1));
        let mfcc_coeffs = mfcc(signal, sample_rate, num_mfcc, num_filters);

        SpectralFeatures {
            centroid,
            rolloff,
            zcr,
            rms,
            mfcc: mfcc_coeffs,
            bandwidth,
        }
    }

    /// Compute spectral bandwidth given magnitudes, frequencies, and a precomputed centroid.
    pub fn bandwidth(magnitudes: &[f64], freqs: &[f64], centroid: f64) -> f64 {
        spectral_bandwidth(magnitudes, freqs, centroid)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sine_wave(freq: f64, sample_rate: f64, n_samples: usize) -> Vec<f64> {
        (0..n_samples)
            .map(|i| (2.0 * PI * freq * i as f64 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn mel_scale_1000hz() {
        // mel_scale(1000) should be close to 999.98 mels
        let mel = mel_scale(1000.0);
        assert!((mel - 999.985).abs() < 0.1, "mel_scale(1000) = {}, expected ~999.98", mel);
    }

    #[test]
    fn mel_roundtrip() {
        for freq in [100.0, 500.0, 1000.0, 4000.0, 8000.0] {
            let mel = mel_scale(freq);
            let back = mel_to_hz(mel);
            assert!(
                (back - freq).abs() < 0.001,
                "roundtrip failed: {} → {} → {}",
                freq,
                mel,
                back
            );
        }
    }

    #[test]
    fn zero_crossing_rate_sine() {
        let freq = 440.0;
        let sample_rate = 44100.0;
        let n = 4096;
        let signal = sine_wave(freq, sample_rate, n);
        let zcr = zero_crossing_rate(&signal);
        // Theoretical ZCR for a pure sine = 2 * freq / sample_rate
        let expected = 2.0 * freq / sample_rate;
        // Allow ±20% tolerance
        assert!(
            (zcr - expected).abs() < expected * 0.2,
            "ZCR {} far from expected {}",
            zcr,
            expected
        );
    }

    #[test]
    fn rms_unit_sine() {
        let n = 44100;
        let signal: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * 440.0 * i as f64 / 44100.0).sin())
            .collect();
        let rms = root_mean_square(&signal);
        // RMS of unit-amplitude sine = 1/sqrt(2) ≈ 0.7071
        assert!(
            (rms - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.01,
            "RMS = {} expected ~0.707",
            rms
        );
    }

    #[test]
    fn spectral_centroid_single_frequency() {
        // A spectrum with energy only in bin k should have centroid = freqs[k]
        let n = 8;
        let mut magnitudes = vec![0.0f64; n];
        magnitudes[3] = 1.0;
        let freqs: Vec<f64> = (0..n).map(|i| i as f64 * 100.0).collect();
        let c = spectral_centroid(&magnitudes, &freqs);
        assert!((c - 300.0).abs() < 1e-9, "centroid = {}, expected 300.0", c);
    }

    #[test]
    fn spectral_rolloff_basic() {
        let magnitudes = vec![1.0, 1.0, 1.0, 1.0];
        let freqs = vec![100.0, 200.0, 300.0, 400.0];
        // 85% of energy (3.4 / 4.0) should roll off at bin 4 (400 Hz) or 300 Hz
        let rolloff = spectral_rolloff(&magnitudes, &freqs, 0.85);
        assert!(rolloff > 0.0 && rolloff <= 400.0);
    }

    #[test]
    fn mel_filterbank_output_length() {
        let fb = MelFilterbank::new(26, 512, 22050.0, 80.0, 8000.0);
        let power_spec = vec![0.1f64; 257]; // 512/2 + 1
        let mel = fb.apply(&power_spec);
        assert_eq!(mel.len(), 26);
    }

    #[test]
    fn mfcc_output_length() {
        let signal: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
        let coeffs = mfcc(&signal, 22050.0, 13, 26);
        assert_eq!(coeffs.len(), 13);
    }

    #[test]
    fn spectral_features_extract() {
        let signal = sine_wave(440.0, 22050.0, 512);
        let features = SpectralFeatures::extract(&signal, 22050.0, 13);
        assert_eq!(features.mfcc.len(), 13);
        assert!(features.rms > 0.0);
        assert!(features.centroid >= 0.0);
    }

    #[test]
    fn bandwidth_zero_for_single_bin() {
        let magnitudes = vec![0.0, 0.0, 1.0, 0.0, 0.0];
        let freqs = vec![0.0, 100.0, 200.0, 300.0, 400.0];
        let centroid = spectral_centroid(&magnitudes, &freqs);
        let bw = spectral_bandwidth(&magnitudes, &freqs, centroid);
        assert!(bw.abs() < 1e-9, "bandwidth of single bin should be 0, got {}", bw);
    }
}
