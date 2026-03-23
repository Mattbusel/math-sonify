//! YIN pitch detection algorithm implementation.

/// Result of pitch detection for a single frame.
#[derive(Debug, Clone)]
pub struct PitchResult {
    pub frequency_hz: f64,
    pub confidence: f64,
    pub midi_note: u8,
    pub cents_deviation: f64,
}

/// YIN pitch detector with configurable parameters.
pub struct YinDetector {
    pub sample_rate: f64,
    pub threshold: f64,
    pub min_freq: f64,
    pub max_freq: f64,
}

impl YinDetector {
    pub fn new(sample_rate: f64, threshold: f64) -> Self {
        Self {
            sample_rate,
            threshold,
            min_freq: 50.0,
            max_freq: 2000.0,
        }
    }

    /// Computes the YIN difference function d(tau).
    pub fn difference_function(frame: &[f64], tau_max: usize) -> Vec<f64> {
        let n = frame.len();
        let tau_max = tau_max.min(n / 2);
        let mut d = vec![0.0_f64; tau_max + 1];
        // d[0] = 0 by definition (set below)
        for tau in 1..=tau_max {
            let mut sum = 0.0;
            for t in 0..(n - tau) {
                let diff = frame[t] - frame[t + tau];
                sum += diff * diff;
            }
            d[tau] = sum;
        }
        d
    }

    /// Computes the cumulative mean normalized difference function.
    pub fn cumulative_mean_normalized_difference(d: &[f64]) -> Vec<f64> {
        let n = d.len();
        if n == 0 {
            return Vec::new();
        }
        let mut cmnd = vec![0.0_f64; n];
        cmnd[0] = 1.0;
        let mut running_sum = 0.0;
        for tau in 1..n {
            running_sum += d[tau];
            if running_sum == 0.0 {
                cmnd[tau] = 1.0;
            } else {
                cmnd[tau] = d[tau] * tau as f64 / running_sum;
            }
        }
        cmnd
    }

    /// Finds the first tau where cmnd[tau] < threshold.
    pub fn absolute_threshold(cmnd: &[f64], threshold: f64) -> Option<usize> {
        // Start from tau=2 to skip trivial zero
        for tau in 2..cmnd.len() {
            if cmnd[tau] < threshold {
                // Find local minimum in this dip
                let mut t = tau;
                while t + 1 < cmnd.len() && cmnd[t + 1] < cmnd[t] {
                    t += 1;
                }
                return Some(t);
            }
        }
        None
    }

    /// Refines the tau estimate using parabolic interpolation.
    pub fn parabolic_interpolation(cmnd: &[f64], tau: usize) -> f64 {
        if tau == 0 || tau + 1 >= cmnd.len() {
            return tau as f64;
        }
        let s0 = cmnd[tau - 1];
        let s1 = cmnd[tau];
        let s2 = cmnd[tau + 1];
        let denom = 2.0 * (2.0 * s1 - s0 - s2);
        if denom.abs() < 1e-10 {
            return tau as f64;
        }
        tau as f64 + (s0 - s2) / denom
    }

    /// Runs the full YIN pipeline on a single frame.
    pub fn detect(&self, frame: &[f64]) -> Option<PitchResult> {
        if frame.is_empty() {
            return None;
        }

        let tau_max_from_min = (self.sample_rate / self.min_freq) as usize;
        let tau_max = tau_max_from_min.min(frame.len() / 2);

        let d = Self::difference_function(frame, tau_max);
        let cmnd = Self::cumulative_mean_normalized_difference(&d);

        let tau_estimate = Self::absolute_threshold(&cmnd, self.threshold)?;

        // Ensure tau is within valid frequency range
        let tau_min_from_max = (self.sample_rate / self.max_freq) as usize;
        if tau_estimate < tau_min_from_max.max(2) {
            return None;
        }

        let refined_tau = Self::parabolic_interpolation(&cmnd, tau_estimate);
        if refined_tau <= 0.0 {
            return None;
        }

        let frequency_hz = self.sample_rate / refined_tau;
        if frequency_hz < self.min_freq || frequency_hz > self.max_freq {
            return None;
        }

        let confidence = 1.0 - cmnd[tau_estimate].min(1.0).max(0.0);
        let (midi_note, cents_deviation) = midi_from_freq(frequency_hz);

        Some(PitchResult {
            frequency_hz,
            confidence,
            midi_note,
            cents_deviation,
        })
    }

    /// Applies sliding-window pitch detection across a buffer.
    pub fn detect_buffer(
        &self,
        samples: &[f64],
        frame_size: usize,
        hop_size: usize,
    ) -> Vec<Option<PitchResult>> {
        if frame_size == 0 || hop_size == 0 {
            return Vec::new();
        }
        let mut results = Vec::new();
        let mut start = 0;
        while start + frame_size <= samples.len() {
            let frame = &samples[start..start + frame_size];
            results.push(self.detect(frame));
            start += hop_size;
        }
        results
    }
}

/// Converts a frequency to (nearest MIDI note, cents deviation).
pub fn midi_from_freq(freq: f64) -> (u8, f64) {
    if freq <= 0.0 {
        return (0, 0.0);
    }
    // A4 = 440 Hz = MIDI 69
    let midi_f = 69.0 + 12.0 * (freq / 440.0).log2();
    let midi_rounded = midi_f.round() as i32;
    let midi_note = midi_rounded.clamp(0, 127) as u8;
    let cents_deviation = (midi_f - midi_rounded as f64) * 100.0;
    (midi_note, cents_deviation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn difference_function_tau0_is_zero() {
        let frame: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
        let d = YinDetector::difference_function(&frame, 128);
        assert_eq!(d[0], 0.0);
    }

    #[test]
    fn cmnd_first_value_is_one() {
        let frame: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
        let d = YinDetector::difference_function(&frame, 128);
        let cmnd = YinDetector::cumulative_mean_normalized_difference(&d);
        assert!((cmnd[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn pure_sine_440hz_detected() {
        let sample_rate = 44100.0;
        let freq = 440.0;
        let frame_size = 2048;
        let frame: Vec<f64> = (0..frame_size)
            .map(|i| (2.0 * PI * freq * i as f64 / sample_rate).sin())
            .collect();

        let detector = YinDetector::new(sample_rate, 0.15);
        let result = detector.detect(&frame);

        assert!(result.is_some(), "440 Hz sine should be detected");
        let r = result.unwrap();
        let error = (r.frequency_hz - freq).abs();
        assert!(error < 5.0, "Detected freq {} Hz, expected ~440 Hz", r.frequency_hz);
    }

    #[test]
    fn midi_from_freq_a4_is_69() {
        let (midi, cents) = midi_from_freq(440.0);
        assert_eq!(midi, 69);
        assert!(cents.abs() < 0.01);
    }

    #[test]
    fn empty_frame_returns_none() {
        let detector = YinDetector::new(44100.0, 0.15);
        assert!(detector.detect(&[]).is_none());
    }

    #[test]
    fn detect_buffer_sliding_window() {
        let sample_rate = 44100.0;
        let freq = 440.0;
        let samples: Vec<f64> = (0..8192)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / sample_rate).sin())
            .collect();
        let detector = YinDetector::new(sample_rate, 0.15);
        let results = detector.detect_buffer(&samples, 2048, 1024);
        assert!(!results.is_empty());
    }
}
