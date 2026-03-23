//! # Audio Convolution and FIR Filtering
//!
//! Provides linear convolution, stateful FIR filter processing, windowed-sinc
//! filter design (low-pass, high-pass, band-pass), and a ring buffer for
//! efficient circular buffering.
//!
//! ## Example
//!
//! ```rust,ignore
//! use math_sonify_plugin::convolution::{low_pass_filter, FirFilter};
//!
//! let coeffs = low_pass_filter(1000.0, 44100.0, 63);
//! let mut filter = FirFilter::new(coeffs);
//! let output = filter.process_block(&input_signal);
//! ```

use std::collections::VecDeque;
use std::f64::consts::PI;

// ── Linear convolution ────────────────────────────────────────────────────────

/// Compute linear (full) convolution of `signal` with `kernel`.
///
/// Output length = `signal.len() + kernel.len() - 1`.
pub fn convolve(signal: &[f64], kernel: &[f64]) -> Vec<f64> {
    if signal.is_empty() || kernel.is_empty() {
        return Vec::new();
    }
    let out_len = signal.len() + kernel.len() - 1;
    let mut out = vec![0.0f64; out_len];
    for (n, o) in out.iter_mut().enumerate() {
        let k_start = if n >= signal.len() { n - signal.len() + 1 } else { 0 };
        let k_end = n.min(kernel.len() - 1);
        for k in k_start..=k_end {
            if n >= k && (n - k) < signal.len() {
                *o += signal[n - k] * kernel[k];
            }
        }
    }
    out
}

/// Apply a FIR filter as a causal linear convolution (same as `convolve`).
///
/// The output length equals `signal.len() + coeffs.len() - 1`.
pub fn fir_filter(signal: &[f64], coeffs: &[f64]) -> Vec<f64> {
    convolve(signal, coeffs)
}

// ── Stateful FIR filter ───────────────────────────────────────────────────────

/// Streaming, stateful FIR filter that maintains inter-block state.
pub struct FirFilter {
    pub coeffs: Vec<f64>,
    state: VecDeque<f64>,
}

impl FirFilter {
    /// Create a new stateful filter with the given coefficients.
    pub fn new(coeffs: Vec<f64>) -> Self {
        let len = coeffs.len();
        let state = VecDeque::from(vec![0.0f64; len]);
        Self { coeffs, state }
    }

    /// Process a single sample; O(N) per call where N = number of taps.
    pub fn process_sample(&mut self, sample: f64) -> f64 {
        self.state.push_front(sample);
        self.state.pop_back();
        self.coeffs
            .iter()
            .zip(self.state.iter())
            .map(|(c, s)| c * s)
            .sum()
    }

    /// Process a block of samples and return the filtered block.
    pub fn process_block(&mut self, samples: &[f64]) -> Vec<f64> {
        samples.iter().map(|&s| self.process_sample(s)).collect()
    }
}

// ── Window functions ──────────────────────────────────────────────────────────

/// Hamming window: 0.54 - 0.46·cos(2π·n/(total-1)).
pub fn hamming_window(n: usize, total: usize) -> f64 {
    if total <= 1 {
        return 1.0;
    }
    0.54 - 0.46 * (2.0 * PI * n as f64 / (total - 1) as f64).cos()
}

/// Hann window: 0.5·(1 - cos(2π·n/(total-1))).
pub fn hann_window(n: usize, total: usize) -> f64 {
    if total <= 1 {
        return 1.0;
    }
    0.5 * (1.0 - (2.0 * PI * n as f64 / (total - 1) as f64).cos())
}

/// Blackman window: 0.42 - 0.5·cos(2π·n/(total-1)) + 0.08·cos(4π·n/(total-1)).
pub fn blackman_window(n: usize, total: usize) -> f64 {
    if total <= 1 {
        return 1.0;
    }
    let t = n as f64 / (total - 1) as f64;
    0.42 - 0.5 * (2.0 * PI * t).cos() + 0.08 * (4.0 * PI * t).cos()
}

// ── Windowed-sinc filter design ───────────────────────────────────────────────

/// Design a windowed-sinc low-pass FIR filter.
///
/// `cutoff_hz` — 3 dB corner frequency in Hz
/// `sample_rate` — samples per second
/// `num_taps` — filter length (should be odd for symmetric, linear-phase response)
pub fn low_pass_filter(cutoff_hz: f64, sample_rate: f64, num_taps: usize) -> Vec<f64> {
    let fc = cutoff_hz / sample_rate; // normalised cutoff (0..0.5)
    let m = num_taps as f64 - 1.0;
    let mut coeffs: Vec<f64> = (0..num_taps)
        .map(|i| {
            let n = i as f64 - m / 2.0;
            let sinc = if n == 0.0 { 2.0 * fc } else { (2.0 * PI * fc * n).sin() / (PI * n) };
            sinc * hamming_window(i, num_taps)
        })
        .collect();
    // Normalise so DC gain = 1.
    let sum: f64 = coeffs.iter().sum();
    if sum.abs() > 1e-12 {
        coeffs.iter_mut().for_each(|c| *c /= sum);
    }
    coeffs
}

/// Design a windowed-sinc high-pass FIR filter via spectral inversion of
/// the corresponding low-pass filter.
pub fn high_pass_filter(cutoff_hz: f64, sample_rate: f64, num_taps: usize) -> Vec<f64> {
    let mut lp = low_pass_filter(cutoff_hz, sample_rate, num_taps);
    // Spectral inversion: negate all coefficients, then add 1 to centre tap.
    lp.iter_mut().for_each(|c| *c = -*c);
    let centre = num_taps / 2;
    lp[centre] += 1.0;
    lp
}

/// Design a band-pass FIR filter as the difference of two low-pass filters.
///
/// `low_hz` — lower cutoff, `high_hz` — upper cutoff.
pub fn band_pass_filter(
    low_hz: f64,
    high_hz: f64,
    sample_rate: f64,
    num_taps: usize,
) -> Vec<f64> {
    let lp_high = low_pass_filter(high_hz, sample_rate, num_taps);
    let lp_low = low_pass_filter(low_hz, sample_rate, num_taps);
    lp_high.iter().zip(lp_low.iter()).map(|(h, l)| h - l).collect()
}

// ── RingBuffer ────────────────────────────────────────────────────────────────

/// A fixed-capacity ring buffer for efficient O(1) push and indexed access.
pub struct RingBuffer {
    data: Vec<f64>,
    head: usize,
    pub size: usize,
}

impl RingBuffer {
    pub fn new(size: usize) -> Self {
        Self { data: vec![0.0; size], head: 0, size }
    }

    /// Push a new sample (overwrites the oldest).
    pub fn push(&mut self, value: f64) {
        self.data[self.head] = value;
        self.head = (self.head + 1) % self.size;
    }

    /// Read sample at logical index `i` (0 = oldest, size-1 = newest).
    pub fn get(&self, i: usize) -> f64 {
        self.data[(self.head + i) % self.size]
    }

    /// Read the most recently pushed sample.
    pub fn latest(&self) -> f64 {
        let idx = (self.head + self.size - 1) % self.size;
        self.data[idx]
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convolution_output_length_is_n_plus_m_minus_one() {
        let signal = vec![1.0, 2.0, 3.0];
        let kernel = vec![0.5, 0.5];
        let out = convolve(&signal, &kernel);
        assert_eq!(out.len(), signal.len() + kernel.len() - 1);
    }

    #[test]
    fn convolution_identity_kernel() {
        let signal = vec![1.0, 2.0, 3.0, 4.0];
        let kernel = vec![1.0];
        let out = convolve(&signal, &kernel);
        assert_eq!(&out[..signal.len()], &signal[..]);
    }

    #[test]
    fn hamming_window_endpoints_near_008() {
        let n = 64;
        // Hamming window endpoints: w(0) = w(N-1) = 0.54 - 0.46 = 0.08
        let w0 = hamming_window(0, n);
        let wn = hamming_window(n - 1, n);
        assert!((w0 - 0.08).abs() < 1e-9, "w(0) = {w0}");
        assert!((wn - 0.08).abs() < 1e-9, "w(N-1) = {wn}");
    }

    #[test]
    fn low_pass_dc_gain_is_one() {
        let coeffs = low_pass_filter(1000.0, 44100.0, 63);
        let dc_gain: f64 = coeffs.iter().sum();
        assert!((dc_gain - 1.0).abs() < 1e-9, "DC gain = {dc_gain}");
    }

    #[test]
    fn fir_streaming_matches_batch() {
        use std::f64::consts::PI;
        let sample_rate = 8000.0;
        // Simple signal: a sine wave.
        let signal: Vec<f64> = (0..128)
            .map(|i| (2.0 * PI * 500.0 * i as f64 / sample_rate).sin())
            .collect();
        let coeffs = low_pass_filter(1000.0, sample_rate, 31);
        let batch = fir_filter(&signal, &coeffs);

        let mut filter = FirFilter::new(coeffs);
        let streaming: Vec<f64> = filter.process_block(&signal);

        // The stateful filter's output should match the first `signal.len()`
        // samples of the linear convolution.
        for (i, (&b, &s)) in batch.iter().zip(streaming.iter()).enumerate() {
            assert!(
                (b - s).abs() < 1e-10,
                "mismatch at sample {i}: batch={b}, streaming={s}"
            );
        }
    }

    #[test]
    fn high_pass_attenuates_dc() {
        let coeffs = high_pass_filter(2000.0, 44100.0, 63);
        // DC gain of a high-pass filter should be ≈ 0.
        let dc_gain: f64 = coeffs.iter().sum();
        assert!(dc_gain.abs() < 0.01, "HP DC gain too large: {dc_gain}");
    }

    #[test]
    fn ring_buffer_push_and_latest() {
        let mut rb = RingBuffer::new(4);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert!((rb.latest() - 3.0).abs() < 1e-12);
    }

    #[test]
    fn ring_buffer_overwrites_oldest() {
        let mut rb = RingBuffer::new(3);
        rb.push(10.0);
        rb.push(20.0);
        rb.push(30.0);
        rb.push(40.0); // Overwrites 10.0
        // Oldest should now be 20.0.
        assert!((rb.get(0) - 20.0).abs() < 1e-12, "oldest = {}", rb.get(0));
    }
}
