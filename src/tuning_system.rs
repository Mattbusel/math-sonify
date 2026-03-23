//! Historical and alternative tuning systems.
//!
//! Provides frequency calculations for equal temperament, just intonation,
//! Pythagorean, meantone, and several well-temperaments (Werckmeister III,
//! Kirnberger).  Also includes utilities for comparing tunings and locating
//! the wolf fifth.

use std::fmt;

// ---------------------------------------------------------------------------
// TuningSystem
// ---------------------------------------------------------------------------

/// The mathematical basis for a tuning.
#[derive(Debug, Clone, PartialEq)]
pub enum TuningSystem {
    EqualTemperament,
    JustIntonation,
    /// Meantone with a given comma fraction (e.g. 0.25 for quarter-comma).
    MeantoneTuning { comma_fraction: f64 },
    PythagoreanTuning,
    QuarterComma,
    ThirdComma,
    WerckmeisterIII,
    Kirnberger,
    /// Fully custom 12-interval set (cents from root, ascending).
    Custom(Vec<f64>),
}

impl fmt::Display for TuningSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TuningSystem::EqualTemperament => write!(f, "Equal Temperament"),
            TuningSystem::JustIntonation => write!(f, "Just Intonation"),
            TuningSystem::MeantoneTuning { comma_fraction } => {
                write!(f, "Meantone ({}/comma)", comma_fraction.recip().round() as u32)
            }
            TuningSystem::PythagoreanTuning => write!(f, "Pythagorean"),
            TuningSystem::QuarterComma => write!(f, "Quarter-Comma Meantone"),
            TuningSystem::ThirdComma => write!(f, "Third-Comma Meantone"),
            TuningSystem::WerckmeisterIII => write!(f, "Werckmeister III"),
            TuningSystem::Kirnberger => write!(f, "Kirnberger III"),
            TuningSystem::Custom(_) => write!(f, "Custom"),
        }
    }
}

// ---------------------------------------------------------------------------
// Interval12
// ---------------------------------------------------------------------------

/// A tempered interval expressed in cents (100 cents = 1 ET semitone).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval12 {
    pub cents: f64,
}

impl Interval12 {
    /// Construct from a frequency ratio.  `ratio = 2.0` → one octave (1200 ¢).
    pub fn from_ratio(ratio: f64) -> Self {
        Self { cents: 1200.0 * ratio.log2() }
    }

    /// Convert back to a frequency ratio.
    pub fn to_ratio(self) -> f64 {
        2.0_f64.powf(self.cents / 1200.0)
    }

    /// Absolute frequency when applied to `base_hz`.
    pub fn frequency(self, base_hz: f64) -> f64 {
        base_hz * self.to_ratio()
    }
}

// ---------------------------------------------------------------------------
// Tuning
// ---------------------------------------------------------------------------

/// A complete 12-note tuning — one interval per semitone above the root.
#[derive(Debug, Clone)]
pub struct Tuning {
    pub name: String,
    /// 12 intervals (index 0 = unison = 0 ¢, index 11 = major seventh).
    pub intervals: Vec<Interval12>,
    /// Reference pitch for MIDI note 69 (A4).
    pub reference_a4_hz: f64,
}

impl Tuning {
    fn new(name: impl Into<String>, intervals: Vec<Interval12>, reference_a4_hz: f64) -> Self {
        assert_eq!(intervals.len(), 12, "Exactly 12 intervals required");
        Self { name: name.into(), intervals, reference_a4_hz }
    }

    /// Exact frequency of a MIDI note number (0-127).
    pub fn frequency_of(&self, midi_note: u8) -> f64 {
        // A4 = MIDI 69
        let semitones_from_a4 = midi_note as i32 - 69_i32;
        let octaves = semitones_from_a4.div_euclid(12);
        let degree = semitones_from_a4.rem_euclid(12) as usize;
        // Root of the octave containing A4
        let root_cents = self.intervals[degree].cents;
        // Adjustment for the octave offset — each octave is 1200 ¢
        let total_cents = root_cents + (octaves as f64 * 1200.0);
        // We need the frequency relative to A4, but our intervals are relative
        // to the key root (C by default, which is 9 semitones below A4).
        // Simpler: treat the 12-interval table as relative to A4 directly.
        let cents_from_a4 = if semitones_from_a4 >= 0 {
            let oct = semitones_from_a4 / 12;
            let deg = (semitones_from_a4 % 12) as usize;
            self.intervals[deg].cents + oct as f64 * 1200.0
        } else {
            // Negative offset
            let s = semitones_from_a4.unsigned_abs() as usize;
            let oct = (s + 11) / 12;
            let deg = (12 - (s % 12)) % 12;
            self.intervals[deg].cents - oct as f64 * 1200.0
        };
        // Suppress unused variable
        let _ = (root_cents, total_cents);
        self.reference_a4_hz * 2.0_f64.powf(cents_from_a4 / 1200.0)
    }

    /// Deviation in cents from equal temperament for a given degree (0-11).
    pub fn cents_deviation_from_et(&self, note: u8) -> f64 {
        let degree = (note % 12) as usize;
        let et_cents = degree as f64 * 100.0;
        self.intervals[degree].cents - et_cents
    }

    /// Beat frequency between two MIDI notes (difference of their frequencies).
    /// Returns 0 if identical.
    pub fn beatings(&self, note_a: u8, note_b: u8) -> f64 {
        let fa = self.frequency_of(note_a);
        let fb = self.frequency_of(note_b);
        (fa - fb).abs()
    }
}

// ---------------------------------------------------------------------------
// TuningFactory
// ---------------------------------------------------------------------------

/// Constructs [`Tuning`] instances for various [`TuningSystem`]s.
#[derive(Debug, Default)]
pub struct TuningFactory;

impl TuningFactory {
    pub fn new() -> Self {
        Self
    }

    /// Dispatch to the appropriate builder.
    pub fn build(&self, system: &TuningSystem, a4_hz: f64) -> Tuning {
        match system {
            TuningSystem::EqualTemperament => self.equal_temperament(a4_hz),
            TuningSystem::JustIntonation => self.just_intonation(a4_hz),
            TuningSystem::PythagoreanTuning => self.pythagorean(a4_hz),
            TuningSystem::QuarterComma => self.meantone(a4_hz, 0.25),
            TuningSystem::ThirdComma => self.meantone(a4_hz, 1.0 / 3.0),
            TuningSystem::MeantoneTuning { comma_fraction } => self.meantone(a4_hz, *comma_fraction),
            TuningSystem::WerckmeisterIII => self.werckmeister_iii(a4_hz),
            TuningSystem::Kirnberger => self.kirnberger(a4_hz),
            TuningSystem::Custom(cents) => {
                let intervals = cents.iter().map(|&c| Interval12 { cents: c }).collect();
                Tuning::new("Custom", intervals, a4_hz)
            }
        }
    }

    /// Standard 12-TET: each semitone = 100 ¢ exactly.
    pub fn equal_temperament(&self, a4_hz: f64) -> Tuning {
        let intervals = (0..12).map(|n| Interval12 { cents: n as f64 * 100.0 }).collect();
        Tuning::new("Equal Temperament", intervals, a4_hz)
    }

    /// Pythagorean tuning built from pure perfect fifths (ratio 3:2).
    /// The circle of fifths closes with a wolf fifth between G# and Eb.
    pub fn pythagorean(&self, a4_hz: f64) -> Tuning {
        // Intervals in cents for C D E F G A B and chromatic pitches
        // generated by stacking pure fifths (702 ¢) from C.
        let fifth = 1200.0 * (3.0_f64 / 2.0).log2(); // 701.955 ¢
        // Order of fifths from C: C G D A E B F# C# G# D# A# F
        let fifth_steps: [i32; 12] = [0, 2, 4, -1, 1, 3, 5, -4, -2, 0, 2, -3];
        // Actually use the standard derivation by fifths:
        // C=0, G=7, D=2, A=9(our root A=0), E=4, B=11, F#=6, C#=1, G#=8, D#=3, A#=10, F=5
        // Cents from A4:
        let degrees_from_c: [f64; 12] = {
            // Build from C = 0 ¢
            let mut arr = [0.0f64; 12];
            // Semitone positions (from C) for each degree built by stacking fifths
            // C  C# D  D# E  F  F# G  G# A  A# B
            // 0   1  2   3  4  5   6  7   8  9  10 11
            // Fifths chain: C→G→D→A→E→B→F#→C#→G#→D#→A#→F
            let chain: [usize; 12] = [0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10, 5];
            for (i, &semitone) in chain.iter().enumerate() {
                let raw = (i as f64) * fifth;
                // Reduce to 0-1200 range
                let reduced = raw - (raw / 1200.0).floor() * 1200.0;
                arr[semitone] = reduced;
            }
            arr
        };

        // Now shift so that A (degree 9 from C) = 0 ¢ (our A4 reference)
        let a_cents = degrees_from_c[9];
        let intervals: Vec<Interval12> = degrees_from_c
            .iter()
            .map(|&c| {
                let shifted = c - a_cents;
                let normalised = if shifted < 0.0 { shifted + 1200.0 } else { shifted };
                Interval12 { cents: normalised }
            })
            .collect();

        let _ = fifth_steps; // suppress warning
        Tuning::new("Pythagorean", intervals, a4_hz)
    }

    /// Just intonation with pure small-integer ratios from the root (A).
    pub fn just_intonation(&self, a4_hz: f64) -> Tuning {
        // Standard 5-limit just ratios relative to A:
        // A  A# B   C    C# D    D# E    F    F# G    G#
        // 1  ?  9/8 6/5  5/4 4/3 ?  3/2  8/5  5/3 16/9 15/8
        // Using common C-major ratios, retuned to A = 1:
        let ratios: [f64; 12] = [
            1.0,         // A  (unison)
            16.0 / 15.0, // A# / Bb
            9.0 / 8.0,   // B
            6.0 / 5.0,   // C
            5.0 / 4.0,   // C#
            4.0 / 3.0,   // D
            45.0 / 32.0, // D# / Eb (tritone)
            3.0 / 2.0,   // E
            8.0 / 5.0,   // F
            5.0 / 3.0,   // F#
            16.0 / 9.0,  // G
            15.0 / 8.0,  // G#
        ];
        let intervals = ratios.iter().map(|&r| Interval12::from_ratio(r)).collect();
        Tuning::new("Just Intonation", intervals, a4_hz)
    }

    /// Meantone tuning with a specified fraction of the syntonic comma (81:80)
    /// applied to each fifth.  `comma_fraction = 0.25` → quarter-comma.
    pub fn meantone(&self, a4_hz: f64, comma_fraction: f64) -> Tuning {
        let syntonic_comma_cents = 1200.0 * (81.0_f64 / 80.0).log2(); // ~21.506 ¢
        let fifth_cents = 1200.0 * (3.0_f64 / 2.0).log2() - comma_fraction * syntonic_comma_cents;

        // Build 12 degrees by stacking meantone fifths from A (degree 0)
        let chain: [usize; 12] = [0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10, 5];
        let mut arr = [0.0f64; 12];
        for (i, &semitone) in chain.iter().enumerate() {
            let raw = (i as f64) * fifth_cents;
            let reduced = raw - (raw / 1200.0).floor() * 1200.0;
            arr[semitone] = reduced;
        }
        // Shift so A = 0
        let a_ref = arr[9]; // A is semitone 9 from C; in our chain it's at index 3
        let _ = a_ref;
        // Use chain index 3 (the 4th fifth from C = A):
        let a_raw = 3.0 * fifth_cents;
        let a_reduced = a_raw - (a_raw / 1200.0).floor() * 1200.0;
        let intervals: Vec<Interval12> = arr
            .iter()
            .map(|&c| {
                let shifted = c - a_reduced;
                let n = if shifted < 0.0 { shifted + 1200.0 } else { shifted };
                Interval12 { cents: n }
            })
            .collect();

        let name = format!("Meantone ({:.3} comma)", comma_fraction);
        Tuning::new(name, intervals, a4_hz)
    }

    /// Werckmeister III — a well-temperament that was popular in Bach's era.
    /// Specified in cents deviations from Pythagorean.
    pub fn werckmeister_iii(&self, a4_hz: f64) -> Tuning {
        // Classic values in cents from C:
        let from_c: [f64; 12] = [
            0.0, 90.225, 192.18, 294.135, 390.225,
            498.045, 588.27, 696.09, 792.18, 888.27,
            996.09, 1092.18,
        ];
        let a_cents = from_c[9]; // A
        let intervals: Vec<Interval12> = from_c
            .iter()
            .map(|&c| {
                let shifted = c - a_cents;
                Interval12 { cents: if shifted < 0.0 { shifted + 1200.0 } else { shifted } }
            })
            .collect();
        Tuning::new("Werckmeister III", intervals, a4_hz)
    }

    /// Kirnberger III — another well-temperament with just major thirds in C, G, D, A, E.
    pub fn kirnberger(&self, a4_hz: f64) -> Tuning {
        // Kirnberger III cents from C:
        let from_c: [f64; 12] = [
            0.0, 90.225, 193.157, 294.135, 386.314,
            498.045, 590.224, 696.578, 792.18, 889.735,
            996.09, 1088.269,
        ];
        let a_cents = from_c[9];
        let intervals: Vec<Interval12> = from_c
            .iter()
            .map(|&c| {
                let shifted = c - a_cents;
                Interval12 { cents: if shifted < 0.0 { shifted + 1200.0 } else { shifted } }
            })
            .collect();
        Tuning::new("Kirnberger III", intervals, a4_hz)
    }

    // -----------------------------------------------------------------------
    // Comparison utilities
    // -----------------------------------------------------------------------

    /// Return the cents difference per semitone between two tunings.
    /// Result is `(degree 0..11, cents_diff)`.
    pub fn compare(t1: &Tuning, t2: &Tuning) -> Vec<(u8, f64)> {
        (0u8..12)
            .map(|n| {
                let diff = t1.intervals[n as usize].cents - t2.intervals[n as usize].cents;
                (n, diff)
            })
            .collect()
    }

    /// Find the worst (most out-of-tune) fifth in a tuning.
    /// Returns `Some((note_a, note_b, cents_of_fifth))` or `None` if perfect.
    pub fn wolf_fifth(tuning: &Tuning) -> Option<(u8, u8, f64)> {
        let pure_fifth = 1200.0 * (3.0_f64 / 2.0).log2(); // ~701.955 ¢
        let mut worst: Option<(u8, u8, f64)> = None;
        let mut max_deviation = 0.0_f64;

        for n in 0u8..12 {
            let upper = (n + 7) % 12;
            let fifth_cents = {
                let low = tuning.intervals[n as usize].cents;
                let high = tuning.intervals[upper as usize].cents;
                let diff = high - low;
                if diff < 0.0 { diff + 1200.0 } else { diff }
            };
            let deviation = (fifth_cents - pure_fifth).abs();
            if deviation > max_deviation {
                max_deviation = deviation;
                worst = Some((n, upper, fifth_cents));
            }
        }

        // Only report as wolf if it's significantly out of tune (> 5 ¢)
        worst.filter(|_| max_deviation > 5.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const A4: f64 = 440.0;

    #[test]
    fn test_et_a4() {
        let factory = TuningFactory::new();
        let tuning = factory.equal_temperament(A4);
        let freq = tuning.frequency_of(69); // MIDI 69 = A4
        assert!((freq - A4).abs() < 0.001, "A4 should be 440 Hz, got {}", freq);
    }

    #[test]
    fn test_et_octave() {
        let factory = TuningFactory::new();
        let tuning = factory.equal_temperament(A4);
        let a5 = tuning.frequency_of(81); // A5 = MIDI 81
        assert!((a5 - 880.0).abs() < 0.01, "A5 should be 880 Hz, got {}", a5);
    }

    #[test]
    fn test_et_deviation_zero() {
        let factory = TuningFactory::new();
        let tuning = factory.equal_temperament(A4);
        for n in 0u8..12 {
            let dev = tuning.cents_deviation_from_et(n);
            assert!(dev.abs() < 0.001, "ET deviation should be 0 for note {}", n);
        }
    }

    #[test]
    fn test_just_third_pure() {
        let factory = TuningFactory::new();
        let tuning = factory.just_intonation(A4);
        // C# above A should be ~386.3 ¢ (pure major third 5:4)
        // In our table index 4 = C# (4 semitones above A)
        let expected = 1200.0 * (5.0_f64 / 4.0).log2();
        let actual = tuning.intervals[4].cents;
        assert!((actual - expected).abs() < 0.5, "Just major third should be ~{:.1} ¢, got {:.1}", expected, actual);
    }

    #[test]
    fn test_interval12_round_trip() {
        let ratio = 3.0 / 2.0;
        let iv = Interval12::from_ratio(ratio);
        assert!((iv.to_ratio() - ratio).abs() < 1e-10);
    }

    #[test]
    fn test_compare_et_vs_pythagorean() {
        let factory = TuningFactory::new();
        let et = factory.equal_temperament(A4);
        let pyth = factory.pythagorean(A4);
        let diffs = TuningFactory::compare(&et, &pyth);
        assert_eq!(diffs.len(), 12);
    }

    #[test]
    fn test_build_dispatch() {
        let factory = TuningFactory::new();
        let t = factory.build(&TuningSystem::WerckmeisterIII, A4);
        assert!(t.name.contains("Werckmeister"));
    }

    #[test]
    fn test_tuning_system_display() {
        assert_eq!(TuningSystem::EqualTemperament.to_string(), "Equal Temperament");
        assert_eq!(TuningSystem::PythagoreanTuning.to_string(), "Pythagorean");
    }
}
