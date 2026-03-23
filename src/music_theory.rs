//! Comprehensive music theory: scales, modes, intervals, harmonic analysis.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Interval
// ---------------------------------------------------------------------------

/// Diatonic and chromatic intervals expressed in semitones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interval {
    Unison,
    MinorSecond,
    MajorSecond,
    MinorThird,
    MajorThird,
    PerfectFourth,
    Tritone,
    PerfectFifth,
    MinorSixth,
    MajorSixth,
    MinorSeventh,
    MajorSeventh,
    Octave,
}

impl Interval {
    /// Number of semitones in this interval.
    pub fn semitones(self) -> u8 {
        match self {
            Interval::Unison => 0,
            Interval::MinorSecond => 1,
            Interval::MajorSecond => 2,
            Interval::MinorThird => 3,
            Interval::MajorThird => 4,
            Interval::PerfectFourth => 5,
            Interval::Tritone => 6,
            Interval::PerfectFifth => 7,
            Interval::MinorSixth => 8,
            Interval::MajorSixth => 9,
            Interval::MinorSeventh => 10,
            Interval::MajorSeventh => 11,
            Interval::Octave => 12,
        }
    }

    /// Tonal quality label.
    pub fn quality(self) -> &'static str {
        match self {
            Interval::Unison | Interval::Octave | Interval::PerfectFourth | Interval::PerfectFifth => "perfect",
            Interval::MinorSecond | Interval::MinorThird | Interval::MinorSixth | Interval::MinorSeventh => "minor",
            Interval::MajorSecond | Interval::MajorThird | Interval::MajorSixth | Interval::MajorSeventh => "major",
            Interval::Tritone => "augmented/diminished",
        }
    }
}

// ---------------------------------------------------------------------------
// Mode
// ---------------------------------------------------------------------------

/// The seven diatonic modes derived from the major scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Ionian,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Mode {
    /// Semitone steps from the root for each scale degree.
    pub fn characteristic_intervals(self) -> Vec<u8> {
        match self {
            Mode::Ionian =>     vec![0, 2, 4, 5, 7, 9, 11],
            Mode::Dorian =>     vec![0, 2, 3, 5, 7, 9, 10],
            Mode::Phrygian =>   vec![0, 1, 3, 5, 7, 8, 10],
            Mode::Lydian =>     vec![0, 2, 4, 6, 7, 9, 11],
            Mode::Mixolydian => vec![0, 2, 4, 5, 7, 9, 10],
            Mode::Aeolian =>    vec![0, 2, 3, 5, 7, 8, 10],
            Mode::Locrian =>    vec![0, 1, 3, 5, 6, 8, 10],
        }
    }
}

// ---------------------------------------------------------------------------
// Scale
// ---------------------------------------------------------------------------

/// A musical scale defined by a root MIDI note and a mode.
#[derive(Debug, Clone)]
pub struct Scale {
    /// MIDI note number of the root (0–127).
    pub root: u8,
    pub mode: Mode,
}

impl Scale {
    pub fn new(root: u8, mode: Mode) -> Self {
        Self { root, mode }
    }

    /// All MIDI notes belonging to this scale (one octave above root).
    pub fn notes(&self) -> Vec<u8> {
        self.mode
            .characteristic_intervals()
            .iter()
            .filter_map(|&interval| self.root.checked_add(interval))
            .collect()
    }

    /// Return the MIDI note for the nth scale degree (1-indexed, wraps at octave).
    pub fn degree(&self, n: u8) -> u8 {
        let intervals = self.mode.characteristic_intervals();
        let idx = ((n.saturating_sub(1)) as usize) % intervals.len();
        let semitones = intervals[idx];
        self.root.saturating_add(semitones)
    }

    /// Whether a MIDI note belongs to this scale (any octave).
    pub fn contains(&self, note: u8) -> bool {
        let pitch_class = note % 12;
        let root_class = self.root % 12;
        self.mode.characteristic_intervals().iter().any(|&interval| {
            (root_class + interval) % 12 == pitch_class
        })
    }

    /// Relative minor of an Ionian scale (shift root to degree 6, Aeolian mode).
    pub fn relative_minor(&self) -> Scale {
        let intervals = self.mode.characteristic_intervals();
        // Degree 6 is at index 5 (0-indexed).
        let sixth_interval = intervals.get(5).copied().unwrap_or(9);
        Scale {
            root: self.root.saturating_add(sixth_interval),
            mode: Mode::Aeolian,
        }
    }

    /// Parallel minor — same root, Aeolian mode.
    pub fn parallel_minor(&self) -> Scale {
        Scale {
            root: self.root,
            mode: Mode::Aeolian,
        }
    }
}

// ---------------------------------------------------------------------------
// HarmonicFunction
// ---------------------------------------------------------------------------

/// Functional role of a chord within a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmonicFunction {
    Tonic,
    Subdominant,
    Dominant,
    Supertonic,
    Mediant,
    Submediant,
    LeadingTone,
}

impl HarmonicFunction {
    /// Typical chord-root scale degrees for this function (1-indexed).
    pub fn typical_chords(self) -> Vec<u8> {
        match self {
            HarmonicFunction::Tonic => vec![1, 6],
            HarmonicFunction::Subdominant => vec![4, 2],
            HarmonicFunction::Dominant => vec![5, 7],
            HarmonicFunction::Supertonic => vec![2],
            HarmonicFunction::Mediant => vec![3],
            HarmonicFunction::Submediant => vec![6],
            HarmonicFunction::LeadingTone => vec![7],
        }
    }
}

// ---------------------------------------------------------------------------
// RomanNumeral
// ---------------------------------------------------------------------------

/// Roman-numeral chord label used in harmonic analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomanNumeral {
    I,
    II,
    III,
    IV,
    V,
    VI,
    VII,
}

impl RomanNumeral {
    /// Scale degree (1-indexed).
    pub fn degree(self) -> u8 {
        match self {
            RomanNumeral::I => 1,
            RomanNumeral::II => 2,
            RomanNumeral::III => 3,
            RomanNumeral::IV => 4,
            RomanNumeral::V => 5,
            RomanNumeral::VI => 6,
            RomanNumeral::VII => 7,
        }
    }

    /// Chord quality in a major key context.
    pub fn quality_in_major(self) -> &'static str {
        match self {
            RomanNumeral::I | RomanNumeral::IV | RomanNumeral::V => "major",
            RomanNumeral::II | RomanNumeral::III | RomanNumeral::VI => "minor",
            RomanNumeral::VII => "diminished",
        }
    }

    fn from_degree(degree: u8) -> Option<RomanNumeral> {
        match degree {
            1 => Some(RomanNumeral::I),
            2 => Some(RomanNumeral::II),
            3 => Some(RomanNumeral::III),
            4 => Some(RomanNumeral::IV),
            5 => Some(RomanNumeral::V),
            6 => Some(RomanNumeral::VI),
            7 => Some(RomanNumeral::VII),
            _ => None,
        }
    }
}

impl fmt::Display for RomanNumeral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RomanNumeral::I => "I",
            RomanNumeral::II => "II",
            RomanNumeral::III => "III",
            RomanNumeral::IV => "IV",
            RomanNumeral::V => "V",
            RomanNumeral::VI => "VI",
            RomanNumeral::VII => "VII",
        };
        write!(f, "{}", s)
    }
}

// ---------------------------------------------------------------------------
// HarmonicAnalyzer
// ---------------------------------------------------------------------------

/// Analyses chord progressions within a scale context.
pub struct HarmonicAnalyzer;

impl HarmonicAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Identify the roman numeral and function of a chord (set of MIDI notes) in a scale.
    pub fn analyze_chord(&self, notes: &[u8], scale: &Scale) -> (RomanNumeral, HarmonicFunction) {
        if notes.is_empty() {
            return (RomanNumeral::I, HarmonicFunction::Tonic);
        }

        let intervals = scale.mode.characteristic_intervals();
        let root_class = (scale.root % 12) as i32;

        // Find the scale degree whose root is closest to the chord's lowest note.
        let bass = (notes.iter().min().copied().unwrap_or(0) % 12) as i32;
        let mut best_degree = 1usize;
        let mut best_dist = 12i32;
        for (idx, &interval) in intervals.iter().enumerate() {
            let note_class = (root_class + interval as i32) % 12;
            let dist = (note_class - bass).abs().min(12 - (note_class - bass).abs());
            if dist < best_dist {
                best_dist = dist;
                best_degree = idx + 1;
            }
        }

        let rn = RomanNumeral::from_degree(best_degree as u8).unwrap_or(RomanNumeral::I);
        let function = Self::degree_to_function(best_degree as u8);
        (rn, function)
    }

    /// Analyse a sequence of chords.
    pub fn identify_progression(
        &self,
        chords: &[Vec<u8>],
        scale: &Scale,
    ) -> Vec<(RomanNumeral, HarmonicFunction)> {
        chords.iter().map(|c| self.analyze_chord(c, scale)).collect()
    }

    /// Return well-known chord progressions by name.
    pub fn common_progressions() -> Vec<(String, Vec<RomanNumeral>)> {
        vec![
            ("I–V–vi–IV (pop)".to_string(), vec![RomanNumeral::I, RomanNumeral::V, RomanNumeral::VI, RomanNumeral::IV]),
            ("ii–V–I (jazz)".to_string(), vec![RomanNumeral::II, RomanNumeral::V, RomanNumeral::I]),
            ("I–IV–V–I (blues/rock)".to_string(), vec![RomanNumeral::I, RomanNumeral::IV, RomanNumeral::V, RomanNumeral::I]),
            ("I–vi–IV–V (50s)".to_string(), vec![RomanNumeral::I, RomanNumeral::VI, RomanNumeral::IV, RomanNumeral::V]),
            ("12-bar blues I".to_string(), vec![
                RomanNumeral::I, RomanNumeral::I, RomanNumeral::I, RomanNumeral::I,
                RomanNumeral::IV, RomanNumeral::IV, RomanNumeral::I, RomanNumeral::I,
                RomanNumeral::V, RomanNumeral::IV, RomanNumeral::I, RomanNumeral::V,
            ]),
            ("I–IV–vi–V".to_string(), vec![RomanNumeral::I, RomanNumeral::IV, RomanNumeral::VI, RomanNumeral::V]),
            ("vi–IV–I–V (minor pop)".to_string(), vec![RomanNumeral::VI, RomanNumeral::IV, RomanNumeral::I, RomanNumeral::V]),
        ]
    }

    /// Suggest next chords based on current chord and style.
    pub fn next_chord_suggestions(current: &RomanNumeral, style: &str) -> Vec<RomanNumeral> {
        match style {
            "jazz" => match current {
                RomanNumeral::I  => vec![RomanNumeral::VI, RomanNumeral::II, RomanNumeral::IV],
                RomanNumeral::II => vec![RomanNumeral::V, RomanNumeral::VII],
                RomanNumeral::V  => vec![RomanNumeral::I, RomanNumeral::VI],
                RomanNumeral::VI => vec![RomanNumeral::II, RomanNumeral::IV],
                _ => vec![RomanNumeral::I, RomanNumeral::V],
            },
            "classical" => match current {
                RomanNumeral::I  => vec![RomanNumeral::IV, RomanNumeral::V, RomanNumeral::II],
                RomanNumeral::IV => vec![RomanNumeral::V, RomanNumeral::I],
                RomanNumeral::V  => vec![RomanNumeral::I, RomanNumeral::VI],
                RomanNumeral::VII => vec![RomanNumeral::I],
                _ => vec![RomanNumeral::I, RomanNumeral::IV, RomanNumeral::V],
            },
            _ /* pop */ => match current {
                RomanNumeral::I   => vec![RomanNumeral::V, RomanNumeral::VI, RomanNumeral::IV],
                RomanNumeral::V   => vec![RomanNumeral::VI, RomanNumeral::I, RomanNumeral::IV],
                RomanNumeral::VI  => vec![RomanNumeral::IV, RomanNumeral::I],
                RomanNumeral::IV  => vec![RomanNumeral::I, RomanNumeral::V],
                _ => vec![RomanNumeral::I, RomanNumeral::V],
            },
        }
    }

    /// Modal brightness score: Locrian = -3, Lydian = +3.
    pub fn modal_brightness(mode: &Mode) -> i32 {
        match mode {
            Mode::Locrian    => -3,
            Mode::Phrygian   => -2,
            Mode::Aeolian    => -1,
            Mode::Dorian     =>  0,
            Mode::Mixolydian =>  1,
            Mode::Ionian     =>  2,
            Mode::Lydian     =>  3,
        }
    }

    /// Dissonance score for a chord within a scale (0.0 = consonant, 1.0 = very dissonant).
    pub fn tension_score(chord: &[u8], scale: &Scale) -> f64 {
        if chord.len() < 2 {
            return 0.0;
        }
        let mut total_dissonance = 0.0;
        let mut pairs = 0usize;

        for i in 0..chord.len() {
            for j in (i + 1)..chord.len() {
                let semitones = (chord[j] as i32 - chord[i] as i32).unsigned_abs() as u8 % 12;
                let d = semitone_dissonance(semitones);
                total_dissonance += d;
                pairs += 1;
            }
        }

        // Penalise notes outside the scale.
        let out_of_scale = chord.iter().filter(|&&n| !scale.contains(n)).count();
        let out_penalty = out_of_scale as f64 * 0.15;

        let base = if pairs > 0 { total_dissonance / pairs as f64 } else { 0.0 };
        (base + out_penalty).min(1.0)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn degree_to_function(degree: u8) -> HarmonicFunction {
        match degree {
            1 => HarmonicFunction::Tonic,
            2 => HarmonicFunction::Supertonic,
            3 => HarmonicFunction::Mediant,
            4 => HarmonicFunction::Subdominant,
            5 => HarmonicFunction::Dominant,
            6 => HarmonicFunction::Submediant,
            7 => HarmonicFunction::LeadingTone,
            _ => HarmonicFunction::Tonic,
        }
    }
}

impl Default for HarmonicAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Perceptual dissonance for an interval in semitones (0 = consonant, 1 = most dissonant).
fn semitone_dissonance(semitones: u8) -> f64 {
    match semitones % 12 {
        0  => 0.0,  // unison / octave
        7  => 0.05, // perfect fifth
        5  => 0.1,  // perfect fourth
        4  => 0.15, // major third
        3  => 0.2,  // minor third
        9  => 0.2,  // major sixth
        8  => 0.25, // minor sixth
        2  => 0.35, // major second
        10 => 0.4,  // minor seventh
        11 => 0.55, // major seventh
        1  => 0.65, // minor second
        6  => 0.7,  // tritone
        _  => 0.5,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ionian_has_seven_notes() {
        let scale = Scale::new(60, Mode::Ionian); // C4
        assert_eq!(scale.notes().len(), 7);
    }

    #[test]
    fn scale_contains_root() {
        let scale = Scale::new(60, Mode::Ionian);
        assert!(scale.contains(60));
        assert!(scale.contains(72)); // C5
    }

    #[test]
    fn scale_excludes_chromatic_note() {
        let scale = Scale::new(60, Mode::Ionian);
        assert!(!scale.contains(61)); // C# not in C major
    }

    #[test]
    fn relative_minor_root() {
        let major = Scale::new(60, Mode::Ionian); // C major
        let rel_minor = major.relative_minor();
        assert_eq!(rel_minor.root, 69); // A
        assert_eq!(rel_minor.mode, Mode::Aeolian);
    }

    #[test]
    fn interval_semitones() {
        assert_eq!(Interval::PerfectFifth.semitones(), 7);
        assert_eq!(Interval::Octave.semitones(), 12);
        assert_eq!(Interval::Tritone.semitones(), 6);
    }

    #[test]
    fn modal_brightness_order() {
        assert!(HarmonicAnalyzer::modal_brightness(&Mode::Lydian) > HarmonicAnalyzer::modal_brightness(&Mode::Locrian));
    }

    #[test]
    fn common_progressions_not_empty() {
        let progs = HarmonicAnalyzer::common_progressions();
        assert!(!progs.is_empty());
    }

    #[test]
    fn tension_score_consonant_fifth() {
        let scale = Scale::new(60, Mode::Ionian);
        let score = HarmonicAnalyzer::tension_score(&[60, 67], &scale); // C + G
        assert!(score < 0.3);
    }

    #[test]
    fn tension_score_dissonant_tritone() {
        let scale = Scale::new(60, Mode::Ionian);
        let score = HarmonicAnalyzer::tension_score(&[60, 66], &scale); // C + F#
        assert!(score > 0.4);
    }

    #[test]
    fn degree_method() {
        let scale = Scale::new(60, Mode::Ionian);
        assert_eq!(scale.degree(1), 60);
        assert_eq!(scale.degree(5), 67); // G
    }
}
