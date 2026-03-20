/// Resonator bank: 8 high-Q bandpass filters.
///
/// By default the filters are tuned to an A major chord across two octaves.
/// Call [`ResonatorBank::tune_to_scale`] to retune to any scale and base
/// frequency from the sonification configuration, so the resonator mode
/// stays harmonically consistent with all other synthesis modes.
///
/// Noise excitation is run through all 8 filters and summed with stereo spread.
use crate::synth::BiquadFilter;

/// Default tuning: A major chord across two octaves.
const DEFAULT_FREQS: [f32; 8] = [220.0, 277.0, 330.0, 370.0, 440.0, 554.0, 660.0, 880.0];

pub struct ResonatorBank {
    filters: Vec<BiquadFilter>,
    /// Tuned frequencies for each filter (Hz).
    pub frequencies: [f32; 8],
    /// Filter resonance Q (default 12.0).
    pub q: f32,
    /// Noise excitation level scalar.
    pub excite_level: f32,
    sr: f32,
    /// xorshift64 state for noise generation.
    rng_state: u64,
}

impl ResonatorBank {
    /// Create a new `ResonatorBank` tuned to a major chord at the given sample rate.
    pub fn new(sr: f32) -> Self {
        let q = 12.0f32;
        let frequencies = DEFAULT_FREQS;
        let filters = frequencies
            .iter()
            .map(|&f| BiquadFilter::band_pass(f, q, sr))
            .collect();
        Self {
            filters,
            frequencies,
            q,
            excite_level: 1.0,
            sr,
            rng_state: 0xDEAD_BEEF_CAFE_BABE_u64,
        }
    }

    /// Retune all filters to the current `frequencies` and `q` values.
    pub fn update(&mut self) {
        for (filter, &freq) in self.filters.iter_mut().zip(self.frequencies.iter()) {
            filter.update_bp(freq, self.q, self.sr);
        }
    }

    /// Retune the 8 resonators to evenly-spaced pitches across the given scale
    /// and octave range.  This keeps the resonator mode harmonically consistent
    /// with the currently selected musical scale.
    ///
    /// # Parameters
    /// - `base_hz`: Lowest resonator frequency in Hz.
    /// - `octave_range`: Number of octaves to span across all 8 resonators.
    /// - `scale_intervals`: Scale semitone intervals (e.g. `[0,2,4,7,9]` for pentatonic).
    pub fn tune_to_scale(&mut self, base_hz: f32, octave_range: f32, scale_intervals: &[f32]) {
        if scale_intervals.is_empty() {
            return;
        }
        let n = self.frequencies.len();
        for i in 0..n {
            let t = i as f32 / (n - 1).max(1) as f32; // 0..1 across all resonators
            let total_semitones = octave_range * 12.0;
            let semitone_pos = t * total_semitones;
            let octave = (semitone_pos / 12.0).floor();
            let semitone_in_oct = semitone_pos % 12.0;
            // Find the nearest scale degree
            let nearest = scale_intervals
                .iter()
                .min_by(|a, b| {
                    let da = ((*a) - semitone_in_oct).abs();
                    let db = ((*b) - semitone_in_oct).abs();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied()
                .unwrap_or(0.0);
            let freq = base_hz * 2.0f32.powf(octave + nearest / 12.0);
            self.frequencies[i] = freq.clamp(20.0, self.sr * 0.45);
        }
        self.update();
    }

    /// xorshift64 — returns a noise sample in [-1, 1].
    fn next_noise(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        // Map top 32 bits to [-1, 1]
        (self.rng_state >> 32) as f32 / (u32::MAX as f32 / 2.0) - 1.0
    }

    /// Process one noise sample through all resonator filters.
    ///
    /// Returns a stereo (L, R) pair with alternate filters spread left and right.
    pub fn process(&mut self, noise_sample: f32) -> (f32, f32) {
        let mut l = 0.0f32;
        let mut r = 0.0f32;
        for (i, filter) in self.filters.iter_mut().enumerate() {
            let out = filter.process(noise_sample);
            if i % 2 == 0 {
                l += out;
            } else {
                r += out;
            }
        }
        // Normalize by number of filters per channel (4 each)
        (l * 0.25, r * 0.25)
    }

    /// Generate internal noise and process it, scaling by `excite_level`.
    pub fn next_sample(&mut self) -> (f32, f32) {
        let noise = self.next_noise() * self.excite_level;
        self.process(noise)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_resonator_output_finite() {
        let mut bank = ResonatorBank::new(SR);
        for _ in 0..2000 {
            let (l, r) = bank.next_sample();
            assert!(l.is_finite(), "Resonator L output non-finite");
            assert!(r.is_finite(), "Resonator R output non-finite");
        }
    }

    #[test]
    fn test_resonator_silent_with_zero_excitation() {
        let mut bank = ResonatorBank::new(SR);
        bank.excite_level = 0.0;
        // Warm up any stored state
        for _ in 0..1000 {
            bank.next_sample();
        }
        // With zero excitation, filters should drain to silence
        let (l, r) = bank.next_sample();
        assert!(
            l.abs() < 1e-4 && r.abs() < 1e-4,
            "Zero excitation should produce silence: l={}, r={}",
            l,
            r
        );
    }

    #[test]
    fn test_resonator_produces_output_with_excitation() {
        let mut bank = ResonatorBank::new(SR);
        bank.excite_level = 1.0;
        let mut max_abs = 0.0_f32;
        for _ in 0..2000 {
            let (l, r) = bank.next_sample();
            max_abs = max_abs.max(l.abs()).max(r.abs());
        }
        assert!(max_abs > 0.0, "Non-zero excitation should produce output");
    }

    #[test]
    fn test_tune_to_scale_empty_intervals_is_noop() {
        let mut bank = ResonatorBank::new(SR);
        let freqs_before = bank.frequencies;
        bank.tune_to_scale(220.0, 2.0, &[]);
        assert_eq!(bank.frequencies, freqs_before, "Empty intervals should leave freqs unchanged");
    }

    #[test]
    fn test_tune_to_scale_frequencies_in_valid_range() {
        let mut bank = ResonatorBank::new(SR);
        let pentatonic = [0.0f32, 2.0, 4.0, 7.0, 9.0];
        bank.tune_to_scale(220.0, 2.0, &pentatonic);
        let nyquist = SR * 0.45;
        for &f in bank.frequencies.iter() {
            assert!(f >= 20.0, "Frequency {} below 20 Hz", f);
            assert!(f <= nyquist, "Frequency {} above Nyquist ({})", f, nyquist);
            assert!(f.is_finite(), "Frequency {} is non-finite", f);
        }
    }

    #[test]
    fn test_tune_to_scale_frequencies_monotone() {
        // Pentatonic across 2 octaves: resonators should be non-decreasing
        let mut bank = ResonatorBank::new(SR);
        let pentatonic = [0.0f32, 2.0, 4.0, 7.0, 9.0];
        bank.tune_to_scale(220.0, 2.0, &pentatonic);
        for i in 1..bank.frequencies.len() {
            assert!(
                bank.frequencies[i] >= bank.frequencies[i - 1],
                "Frequencies not monotone at [{}]: {} < {}",
                i,
                bank.frequencies[i],
                bank.frequencies[i - 1]
            );
        }
    }

    #[test]
    fn test_tune_to_scale_base_frequency_is_lowest() {
        let mut bank = ResonatorBank::new(SR);
        bank.tune_to_scale(440.0, 2.0, &[0.0, 4.0, 7.0]);
        // First resonator (t=0) should be at base frequency
        assert!(
            (bank.frequencies[0] - 440.0).abs() < 1.0,
            "First resonator should be near base 440 Hz, got {}",
            bank.frequencies[0]
        );
    }

    #[test]
    fn test_tune_to_scale_output_still_finite_after_retune() {
        let mut bank = ResonatorBank::new(SR);
        bank.tune_to_scale(330.0, 3.0, &[0.0, 2.0, 4.0, 7.0, 9.0]);
        for _ in 0..1000 {
            let (l, r) = bank.next_sample();
            assert!(l.is_finite() && r.is_finite(), "Output non-finite after retune");
        }
    }
}
