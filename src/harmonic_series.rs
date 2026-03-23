//! Overtone analysis, timbre modeling, and formant synthesis.

use std::f64::consts::PI;

/// One sinusoidal component of a complex tone.
#[derive(Clone, Debug)]
pub struct Partial {
    /// Frequency in Hz.
    pub frequency: f64,
    /// Amplitude (linear, 0.0–1.0 typical).
    pub amplitude: f64,
    /// Phase offset in radians.
    pub phase: f64,
}

/// Generate the ideal harmonic series: f, 2f, 3f, … with amplitude 1/n.
pub fn harmonic_series(fundamental: f64, n_partials: u32) -> Vec<Partial> {
    (1..=n_partials)
        .map(|n| Partial {
            frequency: fundamental * n as f64,
            amplitude: 1.0 / n as f64,
            phase: 0.0,
        })
        .collect()
}

/// Generate an inharmonic series where the n-th partial is:
/// f_n = n * f0 * sqrt(1 + B * n^2)
/// where B is the inharmonicity coefficient.
pub fn inharmonic_series(fundamental: f64, n_partials: u32, inharmonicity: f64) -> Vec<Partial> {
    (1..=n_partials)
        .map(|n| {
            let n_f = n as f64;
            let freq = n_f * fundamental * (1.0 + inharmonicity * n_f * n_f).sqrt();
            Partial {
                frequency: freq,
                amplitude: 1.0 / n_f,
                phase: 0.0,
            }
        })
        .collect()
}

/// Synthesize audio samples by summing sinusoidal partials.
pub fn synthesize(partials: &[Partial], sample_rate: f64, duration_sec: f64) -> Vec<f64> {
    let n_samples = (sample_rate * duration_sec).round() as usize;
    let mut buf = vec![0.0f64; n_samples];
    let inv_sr = 1.0 / sample_rate;
    for (i, s) in buf.iter_mut().enumerate() {
        let t = i as f64 * inv_sr;
        *s = partials
            .iter()
            .map(|p| p.amplitude * (2.0 * PI * p.frequency * t + p.phase).sin())
            .sum();
    }
    buf
}

/// A formant filter modeled as a Gaussian bandpass.
pub struct FormantFilter {
    /// Center frequency in Hz.
    pub center_freq: f64,
    /// Bandwidth (standard deviation) in Hz.
    pub bandwidth: f64,
    /// Peak gain.
    pub gain: f64,
}

impl FormantFilter {
    /// Gaussian frequency response at `freq`.
    pub fn response(&self, freq: f64) -> f64 {
        if self.bandwidth == 0.0 {
            return 0.0;
        }
        let x = (freq - self.center_freq) / self.bandwidth;
        self.gain * (-0.5 * x * x).exp()
    }
}

/// A complete timbre model: partials shaped by formant filters.
pub struct TimbreModel {
    /// Source partials.
    pub partials: Vec<Partial>,
    /// Formant filter bank.
    pub formants: Vec<FormantFilter>,
}

impl TimbreModel {
    /// Apply formant filters to a set of partials.
    /// Each partial's amplitude is multiplied by the sum of all formant responses
    /// at that partial's frequency.
    pub fn apply_formants(&self, partials: &[Partial]) -> Vec<Partial> {
        partials
            .iter()
            .map(|p| {
                let gain: f64 = self.formants.iter().map(|f| f.response(p.frequency)).sum();
                Partial {
                    frequency: p.frequency,
                    amplitude: p.amplitude * gain,
                    phase: p.phase,
                }
            })
            .collect()
    }

    /// Spectral centroid: weighted mean frequency = sum(f * a) / sum(a).
    pub fn brightness(&self) -> f64 {
        let sum_a: f64 = self.partials.iter().map(|p| p.amplitude).sum();
        if sum_a == 0.0 {
            return 0.0;
        }
        self.partials
            .iter()
            .map(|p| p.frequency * p.amplitude)
            .sum::<f64>()
            / sum_a
    }

    /// Roughness: sum of beating interactions between pairs of close partials.
    /// Critical bandwidth ≈ 1.72 * (f1 * f2)^0.5 * 0.24
    pub fn roughness(&self) -> f64 {
        let mut total = 0.0f64;
        let n = self.partials.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let f1 = self.partials[i].frequency;
                let f2 = self.partials[j].frequency;
                if f1 <= 0.0 || f2 <= 0.0 {
                    continue;
                }
                let cbw = 1.72 * (f1 * f2).sqrt() * 0.24;
                let diff = (f2 - f1).abs();
                if diff < cbw {
                    let beating = self.partials[i].amplitude * self.partials[j].amplitude;
                    total += beating * (1.0 - diff / cbw);
                }
            }
        }
        total
    }

    /// Return F1/F2/F3 formant filters for vowels: 'a', 'e', 'i', 'o', 'u'.
    /// Frequencies are approximate standard values for a male voice (Hz).
    pub fn vowel_formants(vowel: char) -> Vec<FormantFilter> {
        // (F1, F2, F3) center frequencies in Hz, bandwidth 80 Hz each
        let (f1, f2, f3): (f64, f64, f64) = match vowel {
            'a' | 'A' => (800.0, 1200.0, 2500.0),
            'e' | 'E' => (400.0, 2000.0, 2600.0),
            'i' | 'I' => (300.0, 2300.0, 3200.0),
            'o' | 'O' => (500.0, 900.0, 2500.0),
            'u' | 'U' => (300.0, 800.0, 2300.0),
            _ => (500.0, 1500.0, 2500.0), // neutral
        };
        vec![
            FormantFilter { center_freq: f1, bandwidth: 80.0, gain: 1.0 },
            FormantFilter { center_freq: f2, bandwidth: 80.0, gain: 0.8 },
            FormantFilter { center_freq: f3, bandwidth: 80.0, gain: 0.6 },
        ]
    }
}

/// Compute the RMS difference between `original` and the synthesized version
/// of `partials` at the given sample rate. The synthesized signal is truncated
/// or zero-padded to match `original.len()`.
pub fn resynthesis_error(original: &[f64], partials: &[Partial], sample_rate: f64) -> f64 {
    if original.is_empty() {
        return 0.0;
    }
    let duration_sec = original.len() as f64 / sample_rate;
    let synth = synthesize(partials, sample_rate, duration_sec);
    let n = original.len().min(synth.len());
    let mse: f64 = (0..n)
        .map(|i| {
            let diff = original[i] - synth[i];
            diff * diff
        })
        .sum::<f64>()
        / n as f64;
    mse.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harmonic_series_amplitudes() {
        let partials = harmonic_series(440.0, 5);
        assert_eq!(partials.len(), 5);
        assert!((partials[0].frequency - 440.0).abs() < 1e-9);
        assert!((partials[0].amplitude - 1.0).abs() < 1e-9);
        assert!((partials[4].amplitude - 0.2).abs() < 1e-9);
    }

    #[test]
    fn inharmonic_higher_than_harmonic() {
        let harmonic = harmonic_series(100.0, 4);
        let inharmonic = inharmonic_series(100.0, 4, 0.01);
        // Each inharmonic partial should be >= the harmonic one
        for (h, ih) in harmonic.iter().zip(inharmonic.iter()) {
            assert!(ih.frequency >= h.frequency - 1e-9);
        }
    }

    #[test]
    fn synthesize_length() {
        let partials = harmonic_series(440.0, 3);
        let samples = synthesize(&partials, 44100.0, 0.1);
        assert_eq!(samples.len(), 4410);
    }

    #[test]
    fn formant_response_peak() {
        let f = FormantFilter { center_freq: 1000.0, bandwidth: 100.0, gain: 1.0 };
        let at_center = f.response(1000.0);
        let off_center = f.response(1200.0);
        assert!((at_center - 1.0).abs() < 1e-9);
        assert!(off_center < at_center);
    }

    #[test]
    fn timbre_brightness() {
        let model = TimbreModel {
            partials: harmonic_series(100.0, 5),
            formants: vec![],
        };
        let b = model.brightness();
        // Spectral centroid should be > fundamental
        assert!(b > 100.0);
    }

    #[test]
    fn vowel_formants_count() {
        for v in ['a', 'e', 'i', 'o', 'u'] {
            let fs = TimbreModel::vowel_formants(v);
            assert_eq!(fs.len(), 3);
        }
    }

    #[test]
    fn resynthesis_error_self() {
        let partials = harmonic_series(220.0, 4);
        let sr = 44100.0;
        let dur = 0.05;
        let signal = synthesize(&partials, sr, dur);
        let err = resynthesis_error(&signal, &partials, sr);
        assert!(err < 1e-9, "self-resynthesis error should be ~0, got {err}");
    }
}
