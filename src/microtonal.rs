//! Microtonal tuning systems: EDO, just intonation, harmonic series, meantone,
//! and fully custom ratio tables.
//!
//! Each system is represented by a [`TuningTable`] that pre-computes every
//! pitch frequency, making real-time lookup O(1).

#![allow(dead_code)]

// ── Just-intonation constants ─────────────────────────────────────────────────

/// Frequency ratios for the pure just major scale (8 degrees, octave-inclusive).
///
/// Degrees: unison, M2, M3, P4, P5, M6, M7, octave.
pub const JUST_MAJOR_RATIOS: [f64; 8] = [
    1.0,          // unison  (1/1)
    9.0 / 8.0,    // major second (9/8)
    5.0 / 4.0,    // major third  (5/4)
    4.0 / 3.0,    // perfect fourth (4/3)
    3.0 / 2.0,    // perfect fifth  (3/2)
    5.0 / 3.0,    // major sixth    (5/3)
    15.0 / 8.0,   // major seventh  (15/8)
    2.0,          // octave         (2/1)
];

// ── TuningSystem ─────────────────────────────────────────────────────────────

/// Choice of tuning system.
#[derive(Debug, Clone, PartialEq)]
pub enum TuningSystem {
    /// n-tone equal temperament: 2^(k/divisions) spacing.
    EqualTemperament(u32),
    /// Pure just intonation (major scale ratios, repeated across octaves).
    JustIntonation,
    /// Harmonic series: integer multiples of a fundamental.
    HarmonicSeries(u32),
    /// Meantone temperament with given fraction of the syntonic comma (e.g. 0.25 for ¼-comma).
    Meantone(f64),
    /// Arbitrary user-supplied frequency ratios relative to root.
    Custom(Vec<f64>),
}

// ── TuningTable ───────────────────────────────────────────────────────────────

/// A pre-computed table of pitches for a given tuning system and root.
#[derive(Debug, Clone)]
pub struct TuningTable {
    /// The tuning system that generated this table.
    pub system: TuningSystem,
    /// Root frequency in Hz (degree 0).
    pub root_hz: f64,
    /// All computed pitch frequencies, in ascending order.
    pub pitches_hz: Vec<f64>,
}

impl TuningTable {
    /// Return the frequency of the given degree, or `None` if out of range.
    pub fn pitch(&self, degree: usize) -> Option<f64> {
        self.pitches_hz.get(degree).copied()
    }
}

// ── MicrotonalGenerator ───────────────────────────────────────────────────────

/// Factory methods for building [`TuningTable`]s.
pub struct MicrotonalGenerator;

impl MicrotonalGenerator {
    // ── Equal temperament ─────────────────────────────────────────────────────

    /// Build an n-EDO table spanning `octaves` octaves.
    ///
    /// Frequency of degree `k` = `root_hz × 2^(k / divisions)`.
    pub fn equal_temperament(root_hz: f64, divisions: u32, octaves: u32) -> TuningTable {
        let n = divisions * octaves + 1; // include final octave pitch
        let pitches_hz = (0..n)
            .map(|k| root_hz * 2.0_f64.powf(k as f64 / divisions as f64))
            .collect();
        TuningTable {
            system: TuningSystem::EqualTemperament(divisions),
            root_hz,
            pitches_hz,
        }
    }

    // ── Just intonation ───────────────────────────────────────────────────────

    /// Build a just-intonation table spanning `octaves` octaves.
    ///
    /// Uses the 7-degree major scale ratios repeated per octave.
    pub fn just_intonation(root_hz: f64, octaves: u32) -> TuningTable {
        // 7 unique degrees per octave (the 8th ratio = 2/1 starts the next).
        let scale_degrees = &JUST_MAJOR_RATIOS[..7];
        let mut pitches_hz = Vec::new();
        for oct in 0..octaves {
            let octave_mult = (1u32 << oct) as f64; // 2^oct
            for &ratio in scale_degrees {
                pitches_hz.push(root_hz * octave_mult * ratio);
            }
        }
        // Final octave top.
        pitches_hz.push(root_hz * (1u32 << octaves) as f64);
        pitches_hz.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
        TuningTable {
            system: TuningSystem::JustIntonation,
            root_hz,
            pitches_hz,
        }
    }

    // ── Harmonic series ───────────────────────────────────────────────────────

    /// Build a harmonic-series table: frequencies = n × fundamental for
    /// n = 1 …= n_partials.
    pub fn harmonic_series(fundamental_hz: f64, n_partials: u32) -> TuningTable {
        let pitches_hz = (1..=n_partials)
            .map(|n| fundamental_hz * n as f64)
            .collect();
        TuningTable {
            system: TuningSystem::HarmonicSeries(n_partials),
            root_hz: fundamental_hz,
            pitches_hz,
        }
    }

    // ── Cents deviation ───────────────────────────────────────────────────────

    /// Compute the interval between two frequencies in cents.
    ///
    /// Result = 1200 × log₂(freq_a / freq_b).
    pub fn cents_deviation(freq_a: f64, freq_b: f64) -> f64 {
        if freq_b <= 0.0 || freq_a <= 0.0 {
            return 0.0;
        }
        1200.0 * (freq_a / freq_b).log2()
    }

    // ── Compare tunings ───────────────────────────────────────────────────────

    /// Compare two tuning tables degree-by-degree, returning the cents
    /// deviation for each shared degree (min length of both tables).
    pub fn compare_tunings(t1: &TuningTable, t2: &TuningTable) -> Vec<f64> {
        t1.pitches_hz
            .iter()
            .zip(t2.pitches_hz.iter())
            .map(|(&a, &b)| Self::cents_deviation(a, b))
            .collect()
    }

    // ── Nearest pitch ─────────────────────────────────────────────────────────

    /// Find the degree in `table` closest to `freq_hz` in cents distance.
    ///
    /// Returns `(degree_index, cents_deviation)`.
    pub fn nearest_pitch(freq_hz: f64, table: &TuningTable) -> (usize, f64) {
        let mut best_idx = 0usize;
        let mut best_cents = f64::INFINITY;
        for (i, &pitch) in table.pitches_hz.iter().enumerate() {
            let cents = Self::cents_deviation(freq_hz, pitch).abs();
            if cents < best_cents {
                best_cents = cents;
                best_idx = i;
            }
        }
        let signed = Self::cents_deviation(freq_hz, table.pitches_hz[best_idx]);
        (best_idx, signed)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_temperament_12edo() {
        let table = MicrotonalGenerator::equal_temperament(440.0, 12, 1);
        // Should have 13 pitches (0..=12).
        assert_eq!(table.pitches_hz.len(), 13);
        // First pitch is root.
        assert!((table.pitch(0).unwrap() - 440.0).abs() < 1e-9);
        // 12th pitch should be an octave above: 880 Hz.
        assert!((table.pitch(12).unwrap() - 880.0).abs() < 1e-6);
    }

    #[test]
    fn test_equal_temperament_semitone_spacing() {
        let table = MicrotonalGenerator::equal_temperament(440.0, 12, 1);
        let cents = MicrotonalGenerator::cents_deviation(
            table.pitch(1).unwrap(),
            table.pitch(0).unwrap(),
        );
        assert!((cents - 100.0).abs() < 1e-6, "expected 100 cents, got {}", cents);
    }

    #[test]
    fn test_just_intonation_perfect_fifth() {
        let table = MicrotonalGenerator::just_intonation(440.0, 1);
        // degree 4 = 3/2 ratio = 660 Hz
        let fifth = table
            .pitches_hz
            .iter()
            .find(|&&f| (f - 660.0).abs() < 1.0);
        assert!(fifth.is_some(), "perfect fifth 660 Hz not found");
    }

    #[test]
    fn test_harmonic_series_partials() {
        let table = MicrotonalGenerator::harmonic_series(100.0, 8);
        assert_eq!(table.pitches_hz.len(), 8);
        assert!((table.pitches_hz[0] - 100.0).abs() < 1e-9);
        assert!((table.pitches_hz[7] - 800.0).abs() < 1e-9);
    }

    #[test]
    fn test_cents_deviation_unison() {
        let cents = MicrotonalGenerator::cents_deviation(440.0, 440.0);
        assert!(cents.abs() < 1e-9);
    }

    #[test]
    fn test_cents_deviation_octave() {
        let cents = MicrotonalGenerator::cents_deviation(880.0, 440.0);
        assert!((cents - 1200.0).abs() < 1e-9);
    }

    #[test]
    fn test_compare_tunings_length() {
        let t1 = MicrotonalGenerator::equal_temperament(440.0, 12, 1);
        let t2 = MicrotonalGenerator::just_intonation(440.0, 1);
        let devs = MicrotonalGenerator::compare_tunings(&t1, &t2);
        // Length = min(t1.len, t2.len).
        assert_eq!(devs.len(), t1.pitches_hz.len().min(t2.pitches_hz.len()));
    }

    #[test]
    fn test_nearest_pitch() {
        let table = MicrotonalGenerator::equal_temperament(440.0, 12, 1);
        // 440 Hz is degree 0.
        let (idx, cents) = MicrotonalGenerator::nearest_pitch(440.0, &table);
        assert_eq!(idx, 0);
        assert!(cents.abs() < 1e-6);
    }

    #[test]
    fn test_nearest_pitch_close() {
        let table = MicrotonalGenerator::equal_temperament(440.0, 12, 1);
        // 442 Hz is close to 440 Hz (degree 0) — about +8 cents.
        let (idx, cents) = MicrotonalGenerator::nearest_pitch(442.0, &table);
        assert_eq!(idx, 0);
        assert!(cents > 0.0);
    }

    #[test]
    fn test_pitch_out_of_range() {
        let table = MicrotonalGenerator::equal_temperament(440.0, 12, 1);
        assert!(table.pitch(999).is_none());
    }
}
