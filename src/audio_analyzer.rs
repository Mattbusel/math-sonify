//! FFT-based spectral analysis, key detection, and tempo estimation.

use std::f64::consts::PI;

/// A single bin of a spectrum.
#[derive(Debug, Clone)]
pub struct SpectralBin {
    pub frequency_hz: f64,
    pub magnitude: f64,
    pub phase: f64,
}

/// Computes the DFT (O(N²)) of a real-valued sample buffer.
pub fn fft_dft(samples: &[f64]) -> Vec<SpectralBin> {
    let n = samples.len();
    if n == 0 {
        return Vec::new();
    }
    // We only return the first N/2+1 bins (positive frequencies)
    let num_bins = n / 2 + 1;
    let mut bins = Vec::with_capacity(num_bins);

    for k in 0..num_bins {
        let mut re = 0.0_f64;
        let mut im = 0.0_f64;
        for (t, &x) in samples.iter().enumerate() {
            let angle = -2.0 * PI * k as f64 * t as f64 / n as f64;
            re += x * angle.cos();
            im += x * angle.sin();
        }
        let magnitude = (re * re + im * im).sqrt();
        let phase = im.atan2(re);
        bins.push(SpectralBin {
            frequency_hz: k as f64, // normalized; caller scales by sample_rate/N
            magnitude,
            phase,
        });
    }
    bins
}

/// Computes the spectral centroid of a spectrum.
pub fn spectral_centroid(bins: &[SpectralBin]) -> f64 {
    let total_mag: f64 = bins.iter().map(|b| b.magnitude).sum();
    if total_mag == 0.0 {
        return 0.0;
    }
    bins.iter().map(|b| b.frequency_hz * b.magnitude).sum::<f64>() / total_mag
}

/// Computes the spectral rolloff frequency (below which `threshold` fraction of energy lies).
pub fn spectral_rolloff(bins: &[SpectralBin], threshold: f64) -> f64 {
    let total_energy: f64 = bins.iter().map(|b| b.magnitude * b.magnitude).sum();
    if total_energy == 0.0 {
        return 0.0;
    }
    let target = total_energy * threshold;
    let mut cumulative = 0.0;
    for bin in bins {
        cumulative += bin.magnitude * bin.magnitude;
        if cumulative >= target {
            return bin.frequency_hz;
        }
    }
    bins.last().map(|b| b.frequency_hz).unwrap_or(0.0)
}

/// Computes spectral flux (sum of positive magnitude differences between frames).
pub fn spectral_flux(current: &[SpectralBin], previous: &[SpectralBin]) -> f64 {
    let len = current.len().min(previous.len());
    let mut flux = 0.0;
    for i in 0..len {
        let diff = current[i].magnitude - previous[i].magnitude;
        if diff > 0.0 {
            flux += diff;
        }
    }
    flux
}

/// Krumhansl-Kessler key profiles: 12 major then 12 minor, starting from C.
pub const KEY_PROFILES: [[f64; 12]; 24] = [
    // Major profiles (C, C#, D, D#, E, F, F#, G, G#, A, A#, B)
    [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88], // C major
    [2.88, 6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29], // C# major
    [2.29, 2.88, 6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66], // D major
    [3.66, 2.29, 2.88, 6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39], // D# major
    [2.39, 3.66, 2.29, 2.88, 6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19], // E major
    [5.19, 2.39, 3.66, 2.29, 2.88, 6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52], // F major
    [2.52, 5.19, 2.39, 3.66, 2.29, 2.88, 6.35, 2.23, 3.48, 2.33, 4.38, 4.09], // F# major
    [4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88, 6.35, 2.23, 3.48, 2.33, 4.38], // G major
    [4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88, 6.35, 2.23, 3.48, 2.33], // G# major
    [2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88, 6.35, 2.23, 3.48], // A major
    [3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88, 6.35, 2.23], // A# major
    [2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88, 6.35], // B major
    // Minor profiles
    [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17], // C minor
    [3.17, 6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34], // C# minor
    [3.34, 3.17, 6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69], // D minor
    [2.69, 3.34, 3.17, 6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98], // D# minor
    [3.98, 2.69, 3.34, 3.17, 6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75], // E minor
    [4.75, 3.98, 2.69, 3.34, 3.17, 6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54], // F minor
    [2.54, 4.75, 3.98, 2.69, 3.34, 3.17, 6.33, 2.68, 3.52, 5.38, 2.60, 3.53], // F# minor
    [3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17, 6.33, 2.68, 3.52, 5.38, 2.60], // G minor
    [2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17, 6.33, 2.68, 3.52, 5.38], // G# minor
    [5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17, 6.33, 2.68, 3.52], // A minor
    [3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17, 6.33, 2.68], // A# minor
    [2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17, 6.33], // B minor
];

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Computes a 12-element chroma vector from spectral bins.
pub fn chroma_vector(bins: &[SpectralBin], sample_rate: f64) -> [f64; 12] {
    let mut chroma = [0.0_f64; 12];
    let n = (bins.len().saturating_sub(1)) * 2; // approximate original frame size
    if n == 0 || sample_rate <= 0.0 {
        return chroma;
    }

    for bin in bins {
        if bin.magnitude == 0.0 || bin.frequency_hz <= 0.0 {
            continue;
        }
        // Convert normalized bin index to actual frequency
        let freq_hz = bin.frequency_hz * sample_rate / n as f64;
        if freq_hz < 20.0 || freq_hz > 20000.0 {
            continue;
        }
        // Convert to MIDI note then to pitch class
        let midi = 69.0 + 12.0 * (freq_hz / 440.0).log2();
        let pitch_class = ((midi.round() as i32).rem_euclid(12)) as usize;
        chroma[pitch_class] += bin.magnitude * bin.magnitude;
    }
    chroma
}

/// Detects the musical key by correlating the chroma vector with Krumhansl-Kessler profiles.
pub fn detect_key(chroma: &[f64; 12]) -> (String, bool) {
    let chroma_mean = chroma.iter().sum::<f64>() / 12.0;
    let chroma_std = {
        let var = chroma.iter().map(|&v| (v - chroma_mean).powi(2)).sum::<f64>() / 12.0;
        var.sqrt()
    };

    let mut best_corr = f64::NEG_INFINITY;
    let mut best_idx = 0;

    for (i, profile) in KEY_PROFILES.iter().enumerate() {
        let profile_mean = profile.iter().sum::<f64>() / 12.0;
        let profile_std = {
            let var = profile.iter().map(|&v| (v - profile_mean).powi(2)).sum::<f64>() / 12.0;
            var.sqrt()
        };

        let denom = chroma_std * profile_std * 12.0;
        let corr = if denom < 1e-10 {
            0.0
        } else {
            profile
                .iter()
                .zip(chroma.iter())
                .map(|(&p, &c)| (p - profile_mean) * (c - chroma_mean))
                .sum::<f64>()
                / denom
        };

        if corr > best_corr {
            best_corr = corr;
            best_idx = i;
        }
    }

    let is_major = best_idx < 12;
    let note_idx = best_idx % 12;
    (NOTE_NAMES[note_idx].to_string(), is_major)
}

/// Estimates tempo from a list of onset times (in milliseconds).
pub fn tempo_from_onsets(onset_times_ms: &[f64]) -> f64 {
    if onset_times_ms.len() < 2 {
        return 0.0;
    }

    // Compute inter-onset intervals
    let iois: Vec<f64> = onset_times_ms
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&ioi| ioi > 0.0)
        .collect();

    if iois.is_empty() {
        return 0.0;
    }

    // Build BPM histogram from 60-180 BPM
    let bpm_min = 60.0_f64;
    let bpm_max = 180.0_f64;
    let num_bins = 121_usize; // 60..180 inclusive, 1 BPM resolution
    let mut histogram = vec![0.0_f64; num_bins];

    for &ioi in &iois {
        let bpm = 60_000.0 / ioi;
        if bpm >= bpm_min && bpm <= bpm_max {
            let bin = ((bpm - bpm_min).round() as usize).min(num_bins - 1);
            histogram[bin] += 1.0;
        }
    }

    let (peak_bin, _) = histogram
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0, &0.0));

    bpm_min + peak_bin as f64
}

/// Summary of audio analysis results.
#[derive(Debug, Clone)]
pub struct AudioAnalysis {
    pub key: String,
    pub is_major: bool,
    pub tempo_bpm: f64,
    pub spectral_centroid: f64,
    pub spectral_rolloff: f64,
}

/// Analyzes a complete audio buffer.
pub fn analyze(samples: &[f64], sample_rate: f64) -> AudioAnalysis {
    let bins = fft_dft(samples);

    // Scale bin frequencies to Hz
    let n = samples.len();
    let scaled_bins: Vec<SpectralBin> = bins
        .iter()
        .map(|b| SpectralBin {
            frequency_hz: b.frequency_hz * sample_rate / n as f64,
            magnitude: b.magnitude,
            phase: b.phase,
        })
        .collect();

    let centroid = spectral_centroid(&scaled_bins);
    let rolloff = spectral_rolloff(&scaled_bins, 0.85);
    let chroma = chroma_vector(&bins, sample_rate);
    let (key, is_major) = detect_key(&chroma);

    AudioAnalysis {
        key,
        is_major,
        tempo_bpm: 0.0, // requires onset detection from outside
        spectral_centroid: centroid,
        spectral_rolloff: rolloff,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sine(freq: f64, sample_rate: f64, num_samples: usize) -> Vec<f64> {
        (0..num_samples)
            .map(|i| (2.0 * PI * freq * i as f64 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn dft_pure_tone_peak_at_correct_bin() {
        let n = 64_usize;
        let k_target = 4_usize;
        // Generate a pure cosine at bin k_target
        let samples: Vec<f64> = (0..n)
            .map(|t| (2.0 * PI * k_target as f64 * t as f64 / n as f64).cos())
            .collect();
        let bins = fft_dft(&samples);
        let peak = bins
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.magnitude.partial_cmp(&b.1.magnitude).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(peak, k_target, "Peak bin should be at k={}", k_target);
    }

    #[test]
    fn spectral_centroid_between_min_max() {
        let sample_rate = 44100.0;
        let samples = make_sine(440.0, sample_rate, 512);
        let bins = fft_dft(&samples);
        if bins.is_empty() {
            return;
        }
        let centroid = spectral_centroid(&bins);
        let min_freq = bins.iter().map(|b| b.frequency_hz).fold(f64::INFINITY, f64::min);
        let max_freq = bins.iter().map(|b| b.frequency_hz).fold(f64::NEG_INFINITY, f64::max);
        assert!(centroid >= min_freq && centroid <= max_freq);
    }

    #[test]
    fn chroma_vector_sums_positive() {
        let sample_rate = 44100.0;
        let samples = make_sine(440.0, sample_rate, 2048);
        let bins = fft_dft(&samples);
        let chroma = chroma_vector(&bins, sample_rate);
        let sum: f64 = chroma.iter().sum();
        assert!(sum > 0.0, "Chroma vector should have positive energy");
    }

    #[test]
    fn detect_key_returns_valid_note_name() {
        let chroma = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
        let (key, is_major) = detect_key(&chroma);
        assert!(NOTE_NAMES.contains(&key.as_str()), "Key '{}' should be a valid note name", key);
        assert!(is_major, "C major profile should detect as major");
    }

    #[test]
    fn tempo_from_onsets_120bpm() {
        // 120 BPM = 500ms intervals
        let onsets: Vec<f64> = (0..20).map(|i| i as f64 * 500.0).collect();
        let bpm = tempo_from_onsets(&onsets);
        assert!((bpm - 120.0).abs() < 2.0, "Expected ~120 BPM, got {}", bpm);
    }

    #[test]
    fn spectral_rolloff_in_range() {
        let n = 64_usize;
        let samples: Vec<f64> = (0..n)
            .map(|t| (2.0 * PI * 4.0 * t as f64 / n as f64).cos())
            .collect();
        let bins = fft_dft(&samples);
        let rolloff = spectral_rolloff(&bins, 0.85);
        let max_freq = bins.iter().map(|b| b.frequency_hz).fold(0.0_f64, f64::max);
        assert!(rolloff <= max_freq + 0.001);
    }
}
