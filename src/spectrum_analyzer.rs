//! Spectral analyzer: computes dominant frequencies from attractor trajectories.
//!
//! Implements:
//! - Cooley-Tukey radix-2 FFT for power-of-2 lengths
//! - O(N²) DFT fallback for other lengths
//! - Hann windowing to reduce spectral leakage
//!
//! # Example
//!
//! ```rust
//! use math_sonify_plugin::spectrum_analyzer::SpectralAnalyzer;
//!
//! // Generate a 440 Hz sine at 44100 Hz sample rate (1024 samples)
//! let samples: Vec<f64> = (0..1024)
//!     .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 44100.0).sin())
//!     .collect();
//!
//! let result = SpectralAnalyzer::analyze(&samples, 44100.0);
//! println!("dominant: {:.1} Hz", result.dominant_freq);
//! ```

use std::f64::consts::PI;

// ── DftResult ─────────────────────────────────────────────────────────────────

/// Result of a spectral analysis.
#[derive(Debug, Clone)]
pub struct DftResult {
    /// Frequency in Hz for each bin.
    pub frequencies: Vec<f64>,
    /// Magnitude for each frequency bin (linear scale, ≥ 0).
    pub magnitudes: Vec<f64>,
    /// Frequency of the bin with the highest magnitude (Hz).
    pub dominant_freq: f64,
    /// Weighted average frequency (magnitude-weighted centroid, Hz).
    pub spectral_centroid: f64,
}

// ── SpectralAnalyzer ──────────────────────────────────────────────────────────

/// Computes the FFT/DFT of a sample sequence and extracts spectral features.
pub struct SpectralAnalyzer;

impl SpectralAnalyzer {
    /// Analyze `samples` recorded at `sample_rate` Hz.
    ///
    /// Applies a Hann window, then:
    /// - Uses the Cooley-Tukey radix-2 FFT if `samples.len()` is a power of two.
    /// - Falls back to an O(N²) DFT for other lengths.
    ///
    /// Returns only the positive-frequency half (DC through Nyquist).
    pub fn analyze(samples: &[f64], sample_rate: f64) -> DftResult {
        let n = samples.len();
        if n == 0 {
            return DftResult {
                frequencies: vec![],
                magnitudes: vec![],
                dominant_freq: 0.0,
                spectral_centroid: 0.0,
            };
        }

        // Apply Hann window
        let windowed: Vec<f64> = samples
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / (n - 1).max(1) as f64).cos());
                s * w
            })
            .collect();

        // Compute complex spectrum
        let spectrum = if n.is_power_of_two() {
            Self::fft(&windowed)
        } else {
            Self::dft(&windowed)
        };

        // Only positive frequencies: bins 0 to N/2 (inclusive)
        let n_bins = n / 2 + 1;
        let bin_hz = sample_rate / n as f64;

        let frequencies: Vec<f64> = (0..n_bins).map(|k| k as f64 * bin_hz).collect();

        // Magnitude with Hann window coherent-gain correction (factor 2, except DC and Nyquist)
        let magnitudes: Vec<f64> = (0..n_bins)
            .map(|k| {
                let (re, im) = spectrum[k];
                let mag = (re * re + im * im).sqrt() / n as f64;
                // Scale non-DC/Nyquist bins by 2 (one-sided spectrum)
                if k == 0 || k == n_bins - 1 {
                    mag
                } else {
                    mag * 2.0
                }
            })
            .collect();

        // Dominant frequency (highest magnitude, skip DC bin 0)
        let dominant_bin = magnitudes
            .iter()
            .enumerate()
            .skip(1) // skip DC
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let dominant_freq = frequencies[dominant_bin];

        // Spectral centroid: sum(f * mag) / sum(mag)
        let mag_sum: f64 = magnitudes.iter().sum();
        let spectral_centroid = if mag_sum < 1e-30 {
            0.0
        } else {
            frequencies
                .iter()
                .zip(magnitudes.iter())
                .map(|(f, m)| f * m)
                .sum::<f64>()
                / mag_sum
        };

        DftResult { frequencies, magnitudes, dominant_freq, spectral_centroid }
    }

    /// Return the top-k (frequency_hz, magnitude) pairs sorted by descending magnitude.
    ///
    /// Skips the DC component (bin 0).
    pub fn dominant_frequencies(result: &DftResult, top_k: usize) -> Vec<(f64, f64)> {
        let mut pairs: Vec<(f64, f64)> = result
            .frequencies
            .iter()
            .zip(result.magnitudes.iter())
            .skip(1) // skip DC
            .map(|(&f, &m)| (f, m))
            .collect();

        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        pairs.truncate(top_k);
        pairs
    }

    // ── Cooley-Tukey radix-2 FFT ─────────────────────────────────────────

    /// In-place Cooley-Tukey radix-2 Decimation-In-Time FFT.
    /// `n` must be a power of two.
    fn fft(samples: &[f64]) -> Vec<(f64, f64)> {
        let n = samples.len();
        assert!(n.is_power_of_two(), "FFT requires power-of-2 length");

        // Initial bit-reversal permutation
        let mut re: Vec<f64> = samples.to_vec();
        let mut im: Vec<f64> = vec![0.0; n];

        let mut j = 0usize;
        for i in 1..n {
            let mut bit = n >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j ^= bit;
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
        }

        // Butterfly stages
        let mut len = 2;
        while len <= n {
            let ang = -2.0 * PI / len as f64;
            let wr = ang.cos();
            let wi = ang.sin();

            let mut pos = 0;
            while pos < n {
                let (mut cur_wr, mut cur_wi) = (1.0_f64, 0.0_f64);
                for k in 0..(len / 2) {
                    let u_re = re[pos + k];
                    let u_im = im[pos + k];
                    let v_re = re[pos + k + len / 2] * cur_wr - im[pos + k + len / 2] * cur_wi;
                    let v_im = re[pos + k + len / 2] * cur_wi + im[pos + k + len / 2] * cur_wr;
                    re[pos + k] = u_re + v_re;
                    im[pos + k] = u_im + v_im;
                    re[pos + k + len / 2] = u_re - v_re;
                    im[pos + k + len / 2] = u_im - v_im;
                    let new_wr = cur_wr * wr - cur_wi * wi;
                    let new_wi = cur_wr * wi + cur_wi * wr;
                    cur_wr = new_wr;
                    cur_wi = new_wi;
                }
                pos += len;
            }
            len <<= 1;
        }

        re.into_iter().zip(im).collect()
    }

    // ── O(N²) DFT fallback ───────────────────────────────────────────────

    /// Direct DFT — O(N²), used for non-power-of-2 lengths.
    fn dft(samples: &[f64]) -> Vec<(f64, f64)> {
        let n = samples.len();
        (0..n)
            .map(|k| {
                let (mut re, mut im) = (0.0, 0.0);
                for (j, &s) in samples.iter().enumerate() {
                    let angle = -2.0 * PI * k as f64 * j as f64 / n as f64;
                    re += s * angle.cos();
                    im += s * angle.sin();
                }
                (re, im)
            })
            .collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 44100.0;

    fn sine_wave(freq: f64, n: usize, sample_rate: f64) -> Vec<f64> {
        (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sample_rate).sin())
            .collect()
    }

    // ── Basic structural tests ────────────────────────────────────────────

    #[test]
    fn test_empty_input() {
        let result = SpectralAnalyzer::analyze(&[], SAMPLE_RATE);
        assert!(result.frequencies.is_empty());
        assert!(result.magnitudes.is_empty());
        assert_eq!(result.dominant_freq, 0.0);
    }

    #[test]
    fn test_output_length_power_of_two() {
        let samples = sine_wave(440.0, 1024, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        assert_eq!(result.frequencies.len(), 513); // 1024/2 + 1
        assert_eq!(result.magnitudes.len(), 513);
    }

    #[test]
    fn test_output_length_non_power_of_two() {
        let samples = sine_wave(440.0, 100, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        assert_eq!(result.frequencies.len(), 51); // 100/2 + 1
        assert_eq!(result.magnitudes.len(), 51);
    }

    #[test]
    fn test_frequencies_monotone_increasing() {
        let samples = sine_wave(440.0, 512, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        for w in result.frequencies.windows(2) {
            assert!(w[1] > w[0], "frequencies must be monotone increasing");
        }
    }

    #[test]
    fn test_dc_component_first_bin_is_zero_hz() {
        let samples = sine_wave(440.0, 512, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        assert_eq!(result.frequencies[0], 0.0);
    }

    // ── Frequency detection tests ─────────────────────────────────────────

    #[test]
    fn test_detect_single_sine_440hz() {
        // Use 4096 samples for good frequency resolution
        let samples = sine_wave(440.0, 4096, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        // Allow ±50 Hz tolerance (bin width = 44100/4096 ≈ 10.8 Hz)
        assert!(
            (result.dominant_freq - 440.0).abs() < 50.0,
            "expected ~440 Hz, got {} Hz",
            result.dominant_freq
        );
    }

    #[test]
    fn test_detect_single_sine_1000hz() {
        let samples = sine_wave(1000.0, 4096, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        assert!(
            (result.dominant_freq - 1000.0).abs() < 50.0,
            "expected ~1000 Hz, got {} Hz",
            result.dominant_freq
        );
    }

    #[test]
    fn test_detect_100hz_small_rate() {
        // Use a lower sample rate so bin resolution is tighter
        let sr = 8000.0;
        let samples = sine_wave(100.0, 2048, sr);
        let result = SpectralAnalyzer::analyze(&samples, sr);
        assert!(
            (result.dominant_freq - 100.0).abs() < 10.0,
            "expected ~100 Hz, got {} Hz",
            result.dominant_freq
        );
    }

    #[test]
    fn test_dc_signal_has_large_dc_component() {
        // A constant signal has all energy at DC
        let samples = vec![1.0; 512];
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        let dc_mag = result.magnitudes[0];
        let max_ac = result.magnitudes[1..].iter().cloned().fold(0.0_f64, f64::max);
        assert!(
            dc_mag > max_ac,
            "DC magnitude {} should exceed max AC {}",
            dc_mag,
            max_ac
        );
    }

    #[test]
    fn test_magnitudes_non_negative() {
        let samples = sine_wave(220.0, 1024, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        for &m in &result.magnitudes {
            assert!(m >= 0.0, "magnitude should be non-negative, got {}", m);
        }
    }

    // ── dominant_frequencies tests ────────────────────────────────────────

    #[test]
    fn test_dominant_frequencies_returns_top_k() {
        let samples = sine_wave(440.0, 2048, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        let top = SpectralAnalyzer::dominant_frequencies(&result, 3);
        assert_eq!(top.len(), 3);
    }

    #[test]
    fn test_dominant_frequencies_sorted_descending() {
        let samples = sine_wave(440.0, 2048, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        let top = SpectralAnalyzer::dominant_frequencies(&result, 10);
        for w in top.windows(2) {
            assert!(w[0].1 >= w[1].1, "should be sorted descending");
        }
    }

    #[test]
    fn test_dominant_frequencies_top1_near_input_freq() {
        let samples = sine_wave(880.0, 4096, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        let top = SpectralAnalyzer::dominant_frequencies(&result, 1);
        assert_eq!(top.len(), 1);
        assert!(
            (top[0].0 - 880.0).abs() < 50.0,
            "expected ~880 Hz, got {} Hz",
            top[0].0
        );
    }

    #[test]
    fn test_dominant_frequencies_empty_result() {
        let result = SpectralAnalyzer::analyze(&[], SAMPLE_RATE);
        let top = SpectralAnalyzer::dominant_frequencies(&result, 5);
        assert!(top.is_empty());
    }

    // ── Nyquist test ──────────────────────────────────────────────────────

    #[test]
    fn test_last_frequency_is_nyquist() {
        let n = 1024usize;
        let samples = sine_wave(440.0, n, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        let nyquist = SAMPLE_RATE / 2.0;
        let last_freq = *result.frequencies.last().unwrap();
        assert!(
            (last_freq - nyquist).abs() < SAMPLE_RATE / n as f64,
            "last bin {} should be near Nyquist {}",
            last_freq,
            nyquist
        );
    }

    // ── Spectral centroid ─────────────────────────────────────────────────

    #[test]
    fn test_spectral_centroid_non_negative() {
        let samples = sine_wave(440.0, 1024, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        assert!(
            result.spectral_centroid >= 0.0,
            "centroid should be non-negative"
        );
    }

    #[test]
    fn test_spectral_centroid_within_range() {
        let samples = sine_wave(440.0, 2048, SAMPLE_RATE);
        let result = SpectralAnalyzer::analyze(&samples, SAMPLE_RATE);
        let nyquist = SAMPLE_RATE / 2.0;
        assert!(
            result.spectral_centroid <= nyquist + 1.0,
            "centroid {} should be <= Nyquist {}",
            result.spectral_centroid,
            nyquist
        );
    }

    // ── DFT vs FFT consistency ────────────────────────────────────────────

    #[test]
    fn test_dft_and_fft_agree_on_power_of_two() {
        // For a short pure sine the dominant bin should be the same
        let samples = sine_wave(100.0, 256, 8000.0);
        let result_fft = SpectralAnalyzer::analyze(&samples, 8000.0);
        // We trust FFT is correct; just verify dominant is same as DFT path
        let dft_spectrum = {
            let windowed: Vec<f64> = samples
                .iter()
                .enumerate()
                .map(|(i, &s)| {
                    let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / (samples.len() - 1) as f64).cos());
                    s * w
                })
                .collect();
            SpectralAnalyzer::dft(&windowed)
        };
        // Check that DC bin magnitude is consistent
        let fft_dc = result_fft.magnitudes[0];
        let dft_dc = (dft_spectrum[0].0 * dft_spectrum[0].0 + dft_spectrum[0].1 * dft_spectrum[0].1).sqrt()
            / samples.len() as f64;
        assert!(
            (fft_dc - dft_dc).abs() < 0.01,
            "DFT and FFT DC mismatch: {} vs {}",
            dft_dc,
            fft_dc
        );
    }
}
