//! Real-time FFT spectral analyser.
//!
//! Computes a magnitude spectrum from the most recent audio waveform buffer.
//! Called from the UI thread each frame (not the audio thread) to avoid
//! allocation in the real-time path.
//!
//! The FFT size is fixed at 2048 samples. A Hann window is applied to reduce
//! spectral leakage. Outputs are in dBFS (0 dBFS = peak, floor = -80 dBFS).

use rustfft::{num_complex::Complex, FftPlanner};

/// Number of FFT points. Must be a power of two.
pub const FFT_SIZE: usize = 2048;

/// Number of magnitude bins output (FFT_SIZE / 2 + 1, DC to Nyquist).
pub const SPECTRUM_BINS: usize = FFT_SIZE / 2 + 1;

/// Compute a magnitude spectrum (in linear scale, 0..1) from a mono waveform.
///
/// `samples` is a recent audio buffer (any length ≥ 2); if shorter than
/// `FFT_SIZE` the buffer is zero-padded. Returns a `SPECTRUM_BINS`-length
/// vector where each entry is the normalised magnitude (0.0 = silence, 1.0 = full scale).
pub fn compute_spectrum(samples: &[f32]) -> Vec<f32> {
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    // Fill input buffer: take the last FFT_SIZE samples (most recent)
    let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    let n = samples.len().min(FFT_SIZE);
    let offset = if samples.len() >= FFT_SIZE { samples.len() - FFT_SIZE } else { 0 };
    for i in 0..n {
        // Hann window: 0.5 * (1 - cos(2π*k/(N-1)))
        let w = 0.5 * (1.0 - (std::f32::consts::TAU * i as f32 / (FFT_SIZE as f32 - 1.0)).cos());
        buf[i] = Complex::new(samples[offset + i] * w, 0.0);
    }

    fft.process(&mut buf);

    // Compute magnitudes for bins 0..SPECTRUM_BINS and normalise
    // The Hann window has coherent gain 0.5 — compensate by multiplying by 2.
    let scale = 2.0 / FFT_SIZE as f32;
    buf[..SPECTRUM_BINS]
        .iter()
        .map(|c| {
            let mag = (c.re * c.re + c.im * c.im).sqrt() * scale;
            mag.clamp(0.0, 1.0)
        })
        .collect()
}

/// Convert linear magnitude to dBFS (decibels relative to full scale).
/// Returns values in the range [-80.0, 0.0].
pub fn mag_to_db(mag: f32) -> f32 {
    if mag < 1e-4 {
        -80.0
    } else {
        20.0 * mag.log10()
    }
}

/// Group `SPECTRUM_BINS` bins into `n_bars` logarithmically-spaced bar values.
/// Each bar's value is the peak magnitude of the bins that fall within its range.
///
/// `sample_rate` is used to map bin index → Hz → log-spaced bar.
/// `f_min` / `f_max` define the displayed frequency range in Hz.
pub fn bins_to_bars(spectrum: &[f32], n_bars: usize, sample_rate: f32, f_min: f32, f_max: f32) -> Vec<f32> {
    if spectrum.is_empty() || n_bars == 0 {
        return vec![0.0; n_bars];
    }
    let bin_hz = sample_rate / (2.0 * (SPECTRUM_BINS as f32 - 1.0));
    let log_min = f_min.max(1.0).log10();
    let log_max = f_max.log10();

    (0..n_bars)
        .map(|bar| {
            let lo_hz = 10.0f32.powf(log_min + (bar as f32 / n_bars as f32) * (log_max - log_min));
            let hi_hz = 10.0f32.powf(log_min + ((bar + 1) as f32 / n_bars as f32) * (log_max - log_min));
            let lo_bin = (lo_hz / bin_hz).floor() as usize;
            let hi_bin = (hi_hz / bin_hz).ceil() as usize;
            let lo_bin = lo_bin.clamp(0, spectrum.len() - 1);
            let hi_bin = hi_bin.clamp(lo_bin, spectrum.len() - 1);
            spectrum[lo_bin..=hi_bin]
                .iter()
                .copied()
                .fold(0.0f32, f32::max)
        })
        .collect()
}
