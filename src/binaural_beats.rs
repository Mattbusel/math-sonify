//! Binaural beat frequency generator for brainwave entrainment.

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// BrainwaveState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrainwaveState {
    /// Delta: deep sleep, 0.5–4 Hz.
    Delta,
    /// Theta: drowsiness / meditation, 4–8 Hz.
    Theta,
    /// Alpha: relaxed alertness, 8–12 Hz.
    Alpha,
    /// Beta: active thinking / focus, 12–30 Hz.
    Beta,
    /// Gamma: higher cognition, 30–100 Hz.
    Gamma,
}

impl BrainwaveState {
    /// Returns a representative / typical frequency for this state (Hz).
    pub fn typical_frequency(&self) -> f64 {
        match self {
            BrainwaveState::Delta => 2.0,
            BrainwaveState::Theta => 6.0,
            BrainwaveState::Alpha => 10.0,
            BrainwaveState::Beta => 20.0,
            BrainwaveState::Gamma => 40.0,
        }
    }

    /// Short human-readable description of the state.
    pub fn description(&self) -> &str {
        match self {
            BrainwaveState::Delta => "Deep sleep and restorative rest (0.5–4 Hz)",
            BrainwaveState::Theta => "Drowsiness, meditation, and creativity (4–8 Hz)",
            BrainwaveState::Alpha => "Relaxed alertness and calm focus (8–12 Hz)",
            BrainwaveState::Beta => "Active thinking, focus, and problem solving (12–30 Hz)",
            BrainwaveState::Gamma => "Higher cognition and peak concentration (30–100 Hz)",
        }
    }
}

// ---------------------------------------------------------------------------
// BinauralBeat
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct BinauralBeat {
    /// Carrier frequency (left channel), in Hz.
    pub carrier_hz: f64,
    /// Beat frequency (difference between left and right), in Hz.
    pub beat_frequency_hz: f64,
}

impl BinauralBeat {
    pub fn new(carrier_hz: f64, beat_frequency_hz: f64) -> Self {
        Self { carrier_hz, beat_frequency_hz }
    }

    /// Left channel frequency = carrier.
    pub fn left_channel_hz(&self) -> f64 {
        self.carrier_hz
    }

    /// Right channel frequency = carrier + beat_frequency.
    pub fn right_channel_hz(&self) -> f64 {
        self.carrier_hz + self.beat_frequency_hz
    }

    /// Construct a BinauralBeat targeting a brainwave state with the given carrier.
    pub fn for_state(state: BrainwaveState, carrier: f64) -> BinauralBeat {
        BinauralBeat::new(carrier, state.typical_frequency())
    }
}

// ---------------------------------------------------------------------------
// BinauralGenerator
// ---------------------------------------------------------------------------

pub struct BinauralGenerator;

impl BinauralGenerator {
    /// Generate stereo sine wave channels for the given binaural beat.
    /// Returns (left_samples, right_samples), each of length `duration_samples`.
    pub fn generate(
        beat: &BinauralBeat,
        duration_samples: usize,
        sample_rate: u32,
    ) -> (Vec<f64>, Vec<f64>) {
        let sr = sample_rate as f64;
        let left_freq = beat.left_channel_hz();
        let right_freq = beat.right_channel_hz();

        let mut left = Vec::with_capacity(duration_samples);
        let mut right = Vec::with_capacity(duration_samples);

        for i in 0..duration_samples {
            let t = i as f64 / sr;
            left.push((2.0 * PI * left_freq * t).sin());
            right.push((2.0 * PI * right_freq * t).sin());
        }

        (left, right)
    }

    /// Apply a linear fade-in and fade-out envelope.
    /// `fade_samples` controls the length of each fade (clamped to half the signal).
    pub fn fade_in_out(channel: &[f64], fade_samples: usize) -> Vec<f64> {
        let n = channel.len();
        if n == 0 {
            return Vec::new();
        }
        let fade = fade_samples.min(n / 2);
        let mut output = channel.to_vec();
        for i in 0..fade {
            let gain = i as f64 / fade as f64;
            output[i] *= gain;
            output[n - 1 - i] *= gain;
        }
        output
    }

    /// Generate a mono amplitude-modulated (isochronic) tone.
    /// The carrier at `freq_hz` is modulated by a square-ish envelope at `pulse_rate_hz`.
    pub fn isochronic_tone(
        freq_hz: f64,
        pulse_rate_hz: f64,
        duration_samples: usize,
        sample_rate: u32,
    ) -> Vec<f64> {
        let sr = sample_rate as f64;
        let mut output = Vec::with_capacity(duration_samples);

        for i in 0..duration_samples {
            let t = i as f64 / sr;
            let carrier = (2.0 * PI * freq_hz * t).sin();
            // Amplitude envelope: raised cosine that pulses at pulse_rate_hz
            // Using (1 + cos(2π*pulse*t)) / 2 for smooth on/off
            let envelope = 0.5 * (1.0 + (2.0 * PI * pulse_rate_hz * t).cos());
            output.push(carrier * envelope);
        }

        output
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brainwave_typical_frequencies() {
        assert!((BrainwaveState::Delta.typical_frequency() - 2.0).abs() < 1e-9);
        assert!((BrainwaveState::Theta.typical_frequency() - 6.0).abs() < 1e-9);
        assert!((BrainwaveState::Alpha.typical_frequency() - 10.0).abs() < 1e-9);
        assert!((BrainwaveState::Beta.typical_frequency() - 20.0).abs() < 1e-9);
        assert!((BrainwaveState::Gamma.typical_frequency() - 40.0).abs() < 1e-9);
    }

    #[test]
    fn test_brainwave_descriptions_nonempty() {
        for state in [
            BrainwaveState::Delta,
            BrainwaveState::Theta,
            BrainwaveState::Alpha,
            BrainwaveState::Beta,
            BrainwaveState::Gamma,
        ] {
            assert!(!state.description().is_empty());
        }
    }

    #[test]
    fn test_binaural_beat_channels() {
        let beat = BinauralBeat::new(200.0, 10.0);
        assert_eq!(beat.left_channel_hz(), 200.0);
        assert_eq!(beat.right_channel_hz(), 210.0);
    }

    #[test]
    fn test_binaural_beat_for_state() {
        let beat = BinauralBeat::for_state(BrainwaveState::Alpha, 200.0);
        assert_eq!(beat.carrier_hz, 200.0);
        assert!((beat.beat_frequency_hz - 10.0).abs() < 1e-9);
        assert_eq!(beat.right_channel_hz(), 210.0);
    }

    #[test]
    fn test_generate_length() {
        let beat = BinauralBeat::new(200.0, 10.0);
        let (left, right) = BinauralGenerator::generate(&beat, 1024, 44100);
        assert_eq!(left.len(), 1024);
        assert_eq!(right.len(), 1024);
    }

    #[test]
    fn test_generate_amplitude_bounded() {
        let beat = BinauralBeat::new(440.0, 5.0);
        let (left, right) = BinauralGenerator::generate(&beat, 4410, 44100);
        for &s in left.iter().chain(right.iter()) {
            assert!(s >= -1.0 && s <= 1.0, "sample {} out of [-1, 1]", s);
        }
    }

    #[test]
    fn test_generate_channels_differ() {
        let beat = BinauralBeat::new(200.0, 10.0);
        let (left, right) = BinauralGenerator::generate(&beat, 44100, 44100);
        // They must not be identical since the frequencies differ
        let differs = left.iter().zip(right.iter()).any(|(l, r)| (l - r).abs() > 1e-9);
        assert!(differs, "Left and right channels should differ");
    }

    #[test]
    fn test_fade_in_out_edges() {
        let samples = vec![1.0_f64; 100];
        let faded = BinauralGenerator::fade_in_out(&samples, 10);
        assert_eq!(faded.len(), 100);
        // First and last samples should be near 0
        assert!(faded[0].abs() < 0.2, "first sample should be near 0, got {}", faded[0]);
        assert!(faded[99].abs() < 0.2, "last sample should be near 0, got {}", faded[99]);
        // Middle should be near 1.0
        assert!((faded[50] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_fade_in_out_empty() {
        let result = BinauralGenerator::fade_in_out(&[], 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_isochronic_tone_length() {
        let tone = BinauralGenerator::isochronic_tone(200.0, 10.0, 512, 44100);
        assert_eq!(tone.len(), 512);
    }

    #[test]
    fn test_isochronic_tone_bounded() {
        let tone = BinauralGenerator::isochronic_tone(440.0, 10.0, 4410, 44100);
        for &s in &tone {
            assert!(s >= -1.0 && s <= 1.0, "isochronic sample {} out of range", s);
        }
    }

    #[test]
    fn test_beat_frequency_ordering() {
        // Beat frequency range checks for states
        assert!(BrainwaveState::Delta.typical_frequency() < BrainwaveState::Theta.typical_frequency());
        assert!(BrainwaveState::Theta.typical_frequency() < BrainwaveState::Alpha.typical_frequency());
        assert!(BrainwaveState::Alpha.typical_frequency() < BrainwaveState::Beta.typical_frequency());
        assert!(BrainwaveState::Beta.typical_frequency() < BrainwaveState::Gamma.typical_frequency());
    }
}
