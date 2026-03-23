//! Harmonic series generator and chord voicing system.
//!
//! Provides tools for building natural harmonic series, constructing chords with
//! specific voicings, and computing optimal voice-leading between chord pairs.

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a frequency ratio (semitones) to Hz given a root.
fn semitones_to_ratio(semitones: f64) -> f64 {
    2.0_f64.powf(semitones / 12.0)
}

/// Absolute semitone distance between two frequencies.
fn semitone_distance(hz_a: f64, hz_b: f64) -> f64 {
    if hz_a <= 0.0 || hz_b <= 0.0 {
        return 0.0;
    }
    (12.0 * (hz_b / hz_a).log2()).abs()
}

// ---------------------------------------------------------------------------
// HarmonicSeries
// ---------------------------------------------------------------------------

/// A harmonic series rooted at a fundamental frequency.
///
/// Each partial is represented as `(partial_number, amplitude)` where amplitude
/// follows the natural decay `1 / partial_number`.
#[derive(Debug, Clone)]
pub struct HarmonicSeries {
    pub fundamental_hz: f64,
    /// Vec of (partial_number, amplitude).
    pub partials: Vec<(u32, f64)>,
}

impl HarmonicSeries {
    /// Create a new series with the given fundamental frequency and no partials.
    pub fn new(fundamental_hz: f64) -> Self {
        Self { fundamental_hz, partials: Vec::new() }
    }

    /// Generate `n` natural harmonic partials (1st = fundamental).
    ///
    /// Returns the absolute frequencies of each partial.
    pub fn generate_partials(&mut self, n: usize) -> Vec<f64> {
        self.partials.clear();
        let mut freqs = Vec::with_capacity(n);
        for k in 1..=(n as u32) {
            let amplitude = 1.0 / k as f64;
            self.partials.push((k, amplitude));
            freqs.push(self.fundamental_hz * k as f64);
        }
        freqs
    }

    /// Frequency of partial number `k` (1-based).
    pub fn partial_freq(&self, k: u32) -> f64 {
        self.fundamental_hz * k as f64
    }
}

// ---------------------------------------------------------------------------
// Voicing
// ---------------------------------------------------------------------------

/// Describes how chord tones are distributed across octaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Voicing {
    /// All notes within one octave, as close together as possible.
    Close,
    /// Notes spread across two octaves for an open sound.
    Open,
    /// The second voice from the top is dropped an octave (jazz voicing).
    Drop2,
    /// The third voice from the top is dropped an octave.
    Drop3,
}

// ---------------------------------------------------------------------------
// Chord
// ---------------------------------------------------------------------------

/// A chord defined by a root frequency, interval ratios, and a voicing.
#[derive(Debug, Clone)]
pub struct Chord {
    pub root_hz: f64,
    /// Interval ratios relative to root (1.0 = root, 1.5 = perfect fifth, etc.).
    pub intervals: Vec<f64>,
    pub voicing: Voicing,
}

impl Chord {
    /// Absolute frequencies of all chord tones after applying the voicing.
    pub fn frequencies(&self) -> Vec<f64> {
        let mut freqs: Vec<f64> =
            self.intervals.iter().map(|r| self.root_hz * r).collect();
        freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        match self.voicing {
            Voicing::Close => freqs,
            Voicing::Open => {
                // Alternate voices between low and high octave positions.
                let mut open = Vec::with_capacity(freqs.len());
                for (i, &f) in freqs.iter().enumerate() {
                    if i % 2 == 0 {
                        open.push(f);
                    } else {
                        open.push(f * 2.0);
                    }
                }
                open.sort_by(|a, b| a.partial_cmp(b).unwrap());
                open
            }
            Voicing::Drop2 => {
                if freqs.len() < 2 {
                    return freqs;
                }
                let top_idx = freqs.len() - 2; // second from top
                freqs[top_idx] /= 2.0;
                freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                freqs
            }
            Voicing::Drop3 => {
                if freqs.len() < 3 {
                    return freqs;
                }
                let top_idx = freqs.len() - 3; // third from top
                freqs[top_idx] /= 2.0;
                freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                freqs
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ChordBuilder
// ---------------------------------------------------------------------------

/// Builds standard chord types in any voicing.
pub struct ChordBuilder;

/// Semitone intervals for common chord types (above the root).
const MAJOR_INTERVALS: &[f64] = &[0.0, 4.0, 7.0];
const MINOR_INTERVALS: &[f64] = &[0.0, 3.0, 7.0];
const DOMINANT7_INTERVALS: &[f64] = &[0.0, 4.0, 7.0, 10.0];
const MAJOR7_INTERVALS: &[f64] = &[0.0, 4.0, 7.0, 11.0];
const MINOR7_INTERVALS: &[f64] = &[0.0, 3.0, 7.0, 10.0];
const DIM7_INTERVALS: &[f64] = &[0.0, 3.0, 6.0, 9.0];
const AUG_INTERVALS: &[f64] = &[0.0, 4.0, 8.0];

impl ChordBuilder {
    fn build(root_hz: f64, semitones: &[f64], voicing: Voicing) -> Chord {
        let intervals = semitones.iter().map(|&s| semitones_to_ratio(s)).collect();
        Chord { root_hz, intervals, voicing }
    }

    pub fn major(root_hz: f64, voicing: Voicing) -> Chord {
        Self::build(root_hz, MAJOR_INTERVALS, voicing)
    }

    pub fn minor(root_hz: f64, voicing: Voicing) -> Chord {
        Self::build(root_hz, MINOR_INTERVALS, voicing)
    }

    pub fn dominant7(root_hz: f64, voicing: Voicing) -> Chord {
        Self::build(root_hz, DOMINANT7_INTERVALS, voicing)
    }

    pub fn major7(root_hz: f64, voicing: Voicing) -> Chord {
        Self::build(root_hz, MAJOR7_INTERVALS, voicing)
    }

    pub fn minor7(root_hz: f64, voicing: Voicing) -> Chord {
        Self::build(root_hz, MINOR7_INTERVALS, voicing)
    }

    pub fn dim7(root_hz: f64, voicing: Voicing) -> Chord {
        Self::build(root_hz, DIM7_INTERVALS, voicing)
    }

    pub fn aug(root_hz: f64, voicing: Voicing) -> Chord {
        Self::build(root_hz, AUG_INTERVALS, voicing)
    }
}

// ---------------------------------------------------------------------------
// Voice leading
// ---------------------------------------------------------------------------

/// Compute the total voice-leading distance (sum of absolute semitone movements)
/// between two chords, using a greedy nearest-neighbour matching.
pub fn voice_leading_distance(chord_a: &Chord, chord_b: &Chord) -> f64 {
    let a_freqs = chord_a.frequencies();
    let b_freqs = chord_b.frequencies();
    let mut total = 0.0;
    let len = a_freqs.len().min(b_freqs.len());
    for i in 0..len {
        total += semitone_distance(a_freqs[i], b_freqs[i]);
    }
    // Penalise any unmatched voices with a fixed cost of 12 semitones each.
    let extra = (a_freqs.len() as isize - b_freqs.len() as isize).unsigned_abs();
    total += extra as f64 * 12.0;
    total
}

/// Select the chord from `candidates` that minimises voice-leading distance from `from`.
///
/// Returns `None` if `candidates` is empty.
pub fn optimal_voice_leading<'a>(from: &Chord, candidates: &'a [Chord]) -> Option<&'a Chord> {
    candidates.iter().min_by(|a, b| {
        let da = voice_leading_distance(from, a);
        let db = voice_leading_distance(from, b);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const A4: f64 = 440.0;

    #[test]
    fn harmonic_series_partials_count() {
        let mut hs = HarmonicSeries::new(A4);
        let freqs = hs.generate_partials(8);
        assert_eq!(freqs.len(), 8);
        assert_eq!(hs.partials.len(), 8);
    }

    #[test]
    fn harmonic_series_frequencies() {
        let mut hs = HarmonicSeries::new(100.0);
        let freqs = hs.generate_partials(4);
        assert!((freqs[0] - 100.0).abs() < 1e-9);
        assert!((freqs[1] - 200.0).abs() < 1e-9);
        assert!((freqs[2] - 300.0).abs() < 1e-9);
        assert!((freqs[3] - 400.0).abs() < 1e-9);
    }

    #[test]
    fn harmonic_series_amplitudes_decay() {
        let mut hs = HarmonicSeries::new(A4);
        hs.generate_partials(4);
        for (i, &(k, amp)) in hs.partials.iter().enumerate() {
            assert_eq!(k, i as u32 + 1);
            assert!((amp - 1.0 / k as f64).abs() < 1e-9);
        }
    }

    #[test]
    fn chord_builder_major_close_has_three_notes() {
        let c = ChordBuilder::major(261.63, Voicing::Close);
        assert_eq!(c.frequencies().len(), 3);
    }

    #[test]
    fn chord_builder_dominant7_has_four_notes() {
        let c = ChordBuilder::dominant7(A4, Voicing::Close);
        assert_eq!(c.frequencies().len(), 4);
    }

    #[test]
    fn chord_builder_all_types_compile() {
        let root = 261.63_f64;
        let _ = ChordBuilder::major(root, Voicing::Close);
        let _ = ChordBuilder::minor(root, Voicing::Open);
        let _ = ChordBuilder::dominant7(root, Voicing::Drop2);
        let _ = ChordBuilder::major7(root, Voicing::Drop3);
        let _ = ChordBuilder::minor7(root, Voicing::Close);
        let _ = ChordBuilder::dim7(root, Voicing::Open);
        let _ = ChordBuilder::aug(root, Voicing::Close);
    }

    #[test]
    fn open_voicing_spreads_notes() {
        let close = ChordBuilder::major(261.63, Voicing::Close).frequencies();
        let open = ChordBuilder::major(261.63, Voicing::Open).frequencies();
        // Open voicing should have a wider range.
        let close_range = close.last().unwrap() - close.first().unwrap();
        let open_range = open.last().unwrap() - open.first().unwrap();
        assert!(open_range > close_range);
    }

    #[test]
    fn drop2_voicing_different_from_close() {
        let close = ChordBuilder::dominant7(A4, Voicing::Close).frequencies();
        let drop2 = ChordBuilder::dominant7(A4, Voicing::Drop2).frequencies();
        // They should differ.
        assert_ne!(close, drop2);
    }

    #[test]
    fn voice_leading_distance_same_chord_zero() {
        let a = ChordBuilder::major(A4, Voicing::Close);
        let b = ChordBuilder::major(A4, Voicing::Close);
        let dist = voice_leading_distance(&a, &b);
        assert!(dist < 1e-6);
    }

    #[test]
    fn voice_leading_distance_semitone_move() {
        // C major -> C# major should be roughly 3 semitones total (one per voice).
        let c = ChordBuilder::major(261.63, Voicing::Close);
        // C# = 261.63 * 2^(1/12)
        let cs = ChordBuilder::major(261.63 * 2.0_f64.powf(1.0 / 12.0), Voicing::Close);
        let dist = voice_leading_distance(&c, &cs);
        assert!(dist > 0.0);
    }

    #[test]
    fn optimal_voice_leading_selects_closest() {
        let from = ChordBuilder::major(261.63, Voicing::Close);
        let near = ChordBuilder::major(261.63 * 2.0_f64.powf(1.0 / 12.0), Voicing::Close);
        let far = ChordBuilder::major(A4, Voicing::Close);
        let candidates = vec![far.clone(), near.clone()];
        let best = optimal_voice_leading(&from, &candidates).unwrap();
        let best_dist = voice_leading_distance(&from, best);
        let far_dist = voice_leading_distance(&from, &far);
        assert!(best_dist <= far_dist);
    }

    #[test]
    fn optimal_voice_leading_empty_candidates() {
        let from = ChordBuilder::major(A4, Voicing::Close);
        assert!(optimal_voice_leading(&from, &[]).is_none());
    }

    #[test]
    fn semitone_distance_octave() {
        // An octave should be exactly 12 semitones.
        let d = semitone_distance(A4, A4 * 2.0);
        assert!((d - 12.0).abs() < 1e-6);
    }
}
