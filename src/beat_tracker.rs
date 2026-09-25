//! # Module: beat_tracker
//!
//! Onset detection and tempo estimation via spectral flux, adaptive threshold,
//! and autocorrelation of inter-onset intervals.

use std::collections::VecDeque;

// ── spectral_flux ─────────────────────────────────────────────────────────────

/// Compute the half-wave-rectified spectral flux between two magnitude spectra.
///
/// Only positive differences (increases in energy) are accumulated.
pub fn spectral_flux(prev_spectrum: &[f64], curr_spectrum: &[f64]) -> f64 {
    let len = prev_spectrum.len().min(curr_spectrum.len());
    (0..len)
        .map(|i| {
            let diff = curr_spectrum[i] - prev_spectrum[i];
            if diff > 0.0 { diff } else { 0.0 }
        })
        .sum()
}

// ── pick_peaks ────────────────────────────────────────────────────────────────

/// Return indices of local maxima in `signal` that exceed `threshold`,
/// enforcing a minimum spacing of `min_gap` samples between peaks.
pub fn pick_peaks(signal: &[f64], threshold: f64, min_gap: usize) -> Vec<usize> {
    let n = signal.len();
    let mut peaks = Vec::new();
    let mut last_peak: Option<usize> = None;
    for i in 1..n.saturating_sub(1) {
        if signal[i] > threshold
            && signal[i] > signal[i - 1]
            && signal[i] >= signal[i + 1]
        {
            let ok = match last_peak {
                Some(lp) => i - lp >= min_gap,
                None => true,
            };
            if ok {
                peaks.push(i);
                last_peak = Some(i);
            }
        }
    }
    peaks
}

// ── OnsetDetector ─────────────────────────────────────────────────────────────

/// Frame-by-frame onset detector using spectral flux with an adaptive EWMA
/// threshold.
pub struct OnsetDetector {
    /// Magnitude spectrum from the previous frame.
    pub prev_magnitudes: Vec<f64>,
    /// Recent flux values (ring buffer).
    pub flux_history: VecDeque<f64>,
    /// Current adaptive threshold (EWMA of flux).
    pub adaptive_threshold: f64,
    /// EWMA smoothing coefficient (0 < alpha < 1; closer to 1 = faster adapt).
    pub alpha: f64,
}

impl OnsetDetector {
    /// Create a new detector for a spectrum of `n_bins` bins.
    pub fn new(n_bins: usize, alpha: f64) -> Self {
        Self {
            prev_magnitudes: vec![0.0; n_bins],
            flux_history: VecDeque::with_capacity(64),
            adaptive_threshold: 0.0,
            alpha,
        }
    }

    /// Process one frame.  Returns the onset strength if an onset is detected
    /// (strength > adaptive threshold), otherwise `None`.
    ///
    /// Adapts the threshold via EWMA after every frame.
    pub fn process_frame(&mut self, magnitudes: &[f64]) -> Option<f64> {
        let flux = spectral_flux(&self.prev_magnitudes, magnitudes);
        // Update threshold (EWMA).
        self.adaptive_threshold =
            self.alpha * flux + (1.0 - self.alpha) * self.adaptive_threshold;
        // Store flux in history.
        if self.flux_history.len() >= 64 {
            self.flux_history.pop_front();
        }
        self.flux_history.push_back(flux);
        // Copy current magnitudes for next frame.
        let len = self.prev_magnitudes.len().min(magnitudes.len());
        self.prev_magnitudes[..len].copy_from_slice(&magnitudes[..len]);
        // Onset if flux exceeds threshold by a margin.
        let threshold_multiplier = 1.5;
        if flux > self.adaptive_threshold * threshold_multiplier && flux > 0.0 {
            Some(flux)
        } else {
            None
        }
    }
}

// ── TempoEstimator ────────────────────────────────────────────────────────────

/// Estimates BPM from a stream of onset times using autocorrelation of the
/// inter-onset interval (IOI) histogram.
pub struct TempoEstimator {
    /// Onset times in seconds (bounded history).
    pub onset_times: VecDeque<f64>,
    /// Audio sample rate (Hz) — used for IOI quantisation.
    pub sample_rate: f64,
}

impl TempoEstimator {
    /// Create a new estimator.
    pub fn new(sample_rate: f64) -> Self {
        Self {
            onset_times: VecDeque::with_capacity(128),
            sample_rate,
        }
    }

    /// Record a new onset at `time_sec`.
    pub fn add_onset(&mut self, time_sec: f64) {
        if self.onset_times.len() >= 128 {
            self.onset_times.pop_front();
        }
        self.onset_times.push_back(time_sec);
    }

    /// Estimate BPM from the histogram of inter-onset intervals.
    ///
    /// Searches the 60–200 BPM range.  Returns `None` if fewer than 3 onsets
    /// have been recorded.
    pub fn estimate_bpm(&self) -> Option<f64> {
        if self.onset_times.len() < 3 {
            return None;
        }
        // Build IOI list (seconds between consecutive onsets).
        let iois: Vec<f64> = self
            .onset_times
            .iter()
            .zip(self.onset_times.iter().skip(1))
            .map(|(a, b)| b - a)
            .filter(|&d| d > 0.0)
            .collect();
        if iois.is_empty() {
            return None;
        }
        // Bin IOIs into a histogram with 1 ms resolution up to 2 s.
        const N_BINS: usize = 2000;
        let bin_width = 0.001; // 1 ms
        let mut hist = vec![0.0f64; N_BINS];
        for ioi in &iois {
            let bin = (ioi / bin_width).round() as usize;
            if bin < N_BINS {
                hist[bin] += 1.0;
            }
        }
        // Score each candidate period by the IOIs that land within +-10 ms of it
        // (the raw histogram, smoothed so slightly jittered onsets still count).
        const TOL: usize = 10;
        let acf: Vec<f64> = (0..N_BINS)
            .map(|b| hist[b.saturating_sub(TOL)..(b + TOL + 1).min(N_BINS)].iter().sum())
            .collect();
        // Search 60–200 BPM → period range 0.3–1.0 s → bins 300–1000.
        let bpm_min = 60.0_f64;
        let bpm_max = 200.0_f64;
        let period_max = 60.0 / bpm_min; // 1.0 s
        let period_min = 60.0 / bpm_max; // 0.3 s
        let bin_min = (period_min / bin_width) as usize;
        let bin_max = ((period_max / bin_width) as usize).min(N_BINS - 1);
        let best_bin = (bin_min..=bin_max)
            .max_by(|&a, &b| acf[a].partial_cmp(&acf[b]).unwrap_or(std::cmp::Ordering::Equal))?;
        if acf[best_bin] <= 0.0 {
            return None;
        }
        let period = best_bin as f64 * bin_width;
        if period <= 0.0 {
            return None;
        }
        Some(60.0 / period)
    }

    /// Guess the meter (2, 3, or 4) from the onset pattern.
    ///
    /// Uses the ratio of the dominant IOI to its sub-multiples.
    pub fn meter_guess(&self) -> u8 {
        let bpm = match self.estimate_bpm() {
            Some(b) => b,
            None => return 4,
        };
        let beat_period = 60.0 / bpm;
        // Count onsets that align to 2, 3, or 4 divisions.
        let score = |n: u8| -> usize {
            let sub = beat_period / n as f64;
            self.onset_times
                .iter()
                .filter(|&&t| {
                    if sub <= 0.0 {
                        return false;
                    }
                    let phase = (t / sub).rem_euclid(1.0);
                    phase < 0.15 || phase > 0.85
                })
                .count()
        };
        let s2 = score(2);
        let s3 = score(3);
        let s4 = score(4);
        if s3 > s2 && s3 > s4 {
            3
        } else if s2 > s4 {
            2
        } else {
            4
        }
    }
}

// ── BeatInfo ──────────────────────────────────────────────────────────────────

/// Per-frame beat tracking result.
#[derive(Debug, Clone)]
pub struct BeatInfo {
    /// Whether an onset was detected in this frame.
    pub is_onset: bool,
    /// Current BPM estimate.
    pub current_bpm: f64,
    /// Beat phase in [0, 1).
    pub beat_phase: f64,
    /// Meter guess (2, 3, or 4).
    pub meter: u8,
}

// ── BeatTracker ───────────────────────────────────────────────────────────────

/// Combines onset detection and tempo estimation into a single stateful tracker.
pub struct BeatTracker {
    /// Onset detector.
    pub onset_detector: OnsetDetector,
    /// Tempo estimator.
    pub tempo_estimator: TempoEstimator,
    /// Current BPM estimate.
    pub current_bpm: f64,
    /// Current beat phase in [0, 1).
    pub beat_phase: f64,
}

impl BeatTracker {
    /// Create a new tracker.
    pub fn new(n_bins: usize, sample_rate: f64) -> Self {
        Self {
            onset_detector: OnsetDetector::new(n_bins, 0.1),
            tempo_estimator: TempoEstimator::new(sample_rate),
            current_bpm: 120.0,
            beat_phase: 0.0,
        }
    }

    /// Process one analysis frame at time `time_sec`.
    pub fn process_frame(&mut self, magnitudes: &[f64], time_sec: f64) -> BeatInfo {
        let onset_strength = self.onset_detector.process_frame(magnitudes);
        let is_onset = onset_strength.is_some();
        if is_onset {
            self.tempo_estimator.add_onset(time_sec);
        }
        if let Some(bpm) = self.tempo_estimator.estimate_bpm() {
            self.current_bpm = bpm;
        }
        // Advance beat phase.
        if self.current_bpm > 0.0 {
            // Phase advances by (bpm/60) beats per second; but we only know
            // elapsed time if we track last-call time.  Use a simple increment
            // based on the onset density instead.
            let beat_period = 60.0 / self.current_bpm;
            self.beat_phase = (time_sec / beat_period).rem_euclid(1.0);
        }
        let meter = self.tempo_estimator.meter_guess();
        BeatInfo {
            is_onset,
            current_bpm: self.current_bpm,
            beat_phase: self.beat_phase,
            meter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectral_flux_positive_only() {
        let prev = vec![1.0, 2.0, 3.0];
        let curr = vec![2.0, 1.0, 4.0];
        // Only positive diffs: +1 and +1 → 2.0
        assert!((spectral_flux(&prev, &curr) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn pick_peaks_basic() {
        let signal = vec![0.0, 1.0, 3.0, 2.0, 0.0, 2.0, 4.0, 1.0];
        let peaks = pick_peaks(&signal, 0.5, 2);
        assert!(peaks.contains(&2));
        assert!(peaks.contains(&6));
    }

    #[test]
    fn tempo_estimator_120bpm() {
        let mut te = TempoEstimator::new(44100.0);
        // Add onsets every 0.5 s (= 120 BPM).
        for i in 0..20 {
            te.add_onset(i as f64 * 0.5);
        }
        let bpm = te.estimate_bpm().unwrap();
        // Should be close to 120 ± 10.
        assert!((bpm - 120.0).abs() < 15.0, "bpm = {bpm}");
    }

    #[test]
    fn beat_tracker_runs() {
        let mut bt = BeatTracker::new(64, 44100.0);
        let frame = vec![0.5f64; 64];
        let info = bt.process_frame(&frame, 0.0);
        assert!(info.current_bpm > 0.0);
    }
}
