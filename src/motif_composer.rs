//! Rule-based algorithmic composition with motif development.
//!
//! Provides a pipeline for generating musical motifs, applying classical
//! developmental techniques (inversion, retrograde, augmentation, etc.),
//! and assembling full compositions in common structural forms (ABA, Rondo…).

use std::fmt;

// ---------------------------------------------------------------------------
// MotifShape
// ---------------------------------------------------------------------------

/// The contour profile used when generating a new motif.
#[derive(Debug, Clone, PartialEq)]
pub enum MotifShape {
    Ascending,
    Descending,
    ArchUp,
    ArchDown,
    Wave,
    Static,
    Random { seed: u64 },
}

// ---------------------------------------------------------------------------
// Motif
// ---------------------------------------------------------------------------

/// A short melodic figure: a sequence of MIDI note numbers and durations.
#[derive(Debug, Clone, PartialEq)]
pub struct Motif {
    /// MIDI note numbers (0-127).
    pub notes: Vec<u8>,
    /// Duration of each note in ticks (e.g. 480 = quarter note at 480 PPQN).
    pub durations: Vec<u32>,
    pub name: String,
}

impl Motif {
    pub fn new(name: impl Into<String>, notes: Vec<u8>, durations: Vec<u32>) -> Self {
        Self { name: name.into(), notes, durations }
    }

    /// Melodic inversion: reflect each interval around `axis`.
    pub fn invert(&self, axis: u8) -> Motif {
        let inverted: Vec<u8> = self.notes.iter().map(|&n| {
            let diff = n as i32 - axis as i32;
            (axis as i32 - diff).clamp(0, 127) as u8
        }).collect();
        Motif::new(format!("{}_inv", self.name), inverted, self.durations.clone())
    }

    /// Retrograde: reverse the note (and duration) sequence.
    pub fn retrograde(&self) -> Motif {
        let mut notes = self.notes.clone();
        let mut durs = self.durations.clone();
        notes.reverse();
        durs.reverse();
        Motif::new(format!("{}_retro", self.name), notes, durs)
    }

    /// Transpose all notes by `semitones` (clamped to 0-127).
    pub fn transpose(&self, semitones: i32) -> Motif {
        let transposed: Vec<u8> = self.notes.iter()
            .map(|&n| (n as i32 + semitones).clamp(0, 127) as u8)
            .collect();
        let label = if semitones >= 0 { format!("{}_t+{}", self.name, semitones) }
                    else { format!("{}_t{}", self.name, semitones) };
        Motif::new(label, transposed, self.durations.clone())
    }

    /// Augmentation: multiply all durations by `factor`.
    pub fn augment(&self, factor: u32) -> Motif {
        let durs: Vec<u32> = self.durations.iter().map(|&d| d.saturating_mul(factor)).collect();
        Motif::new(format!("{}_aug{}", self.name, factor), self.notes.clone(), durs)
    }

    /// Diminution: divide all durations by `factor` (minimum 1 tick).
    pub fn diminish(&self, factor: u32) -> Motif {
        let f = factor.max(1);
        let durs: Vec<u32> = self.durations.iter().map(|&d| (d / f).max(1)).collect();
        Motif::new(format!("{}_dim{}", self.name, factor), self.notes.clone(), durs)
    }
}

// ---------------------------------------------------------------------------
// PhrasePurpose
// ---------------------------------------------------------------------------

/// The structural role of a phrase within a composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhrasePurpose {
    Opening,
    Continuation,
    Climax,
    Resolution,
    Bridge,
}

impl fmt::Display for PhrasePurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhrasePurpose::Opening => write!(f, "Opening"),
            PhrasePurpose::Continuation => write!(f, "Continuation"),
            PhrasePurpose::Climax => write!(f, "Climax"),
            PhrasePurpose::Resolution => write!(f, "Resolution"),
            PhrasePurpose::Bridge => write!(f, "Bridge"),
        }
    }
}

// ---------------------------------------------------------------------------
// Phrase
// ---------------------------------------------------------------------------

/// A musical phrase consisting of one or more motifs plus a harmonic context.
#[derive(Debug, Clone)]
pub struct Phrase {
    pub motifs: Vec<Motif>,
    pub purpose: PhrasePurpose,
    /// Supporting chord notes (MIDI).
    pub harmony_notes: Vec<u8>,
}

impl Phrase {
    pub fn new(motifs: Vec<Motif>, purpose: PhrasePurpose, harmony_notes: Vec<u8>) -> Self {
        Self { motifs, purpose, harmony_notes }
    }

    /// Collect all note numbers from all motifs in order.
    pub fn all_notes(&self) -> Vec<u8> {
        self.motifs.iter().flat_map(|m| m.notes.iter().copied()).collect()
    }

    /// Total duration in ticks.
    pub fn total_duration(&self) -> u64 {
        self.motifs.iter()
            .flat_map(|m| m.durations.iter().map(|&d| d as u64))
            .sum()
    }
}

// ---------------------------------------------------------------------------
// CompositionForm
// ---------------------------------------------------------------------------

/// High-level formal structure for a multi-phrase composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionForm {
    ABA,
    ABAB,
    Rondo,
    ThroughComposed,
    Binary,
    Ternary,
}

impl fmt::Display for CompositionForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompositionForm::ABA => write!(f, "ABA"),
            CompositionForm::ABAB => write!(f, "ABAB"),
            CompositionForm::Rondo => write!(f, "Rondo (ABACADA)"),
            CompositionForm::ThroughComposed => write!(f, "Through-Composed"),
            CompositionForm::Binary => write!(f, "Binary (AB)"),
            CompositionForm::Ternary => write!(f, "Ternary (ABA')"),
        }
    }
}

// ---------------------------------------------------------------------------
// AlgorithmicComposer
// ---------------------------------------------------------------------------

/// Generates motifs and assembles them into structured compositions.
#[derive(Debug, Default)]
pub struct AlgorithmicComposer;

impl AlgorithmicComposer {
    pub fn new() -> Self { Self }

    // -----------------------------------------------------------------------
    // Motif generation
    // -----------------------------------------------------------------------

    /// Generate a motif from a scale, using the given contour shape.
    pub fn generate_motif(
        scale_notes: &[u8],
        shape: MotifShape,
        length: usize,
        seed: u64,
    ) -> Motif {
        if scale_notes.is_empty() || length == 0 {
            return Motif::new("empty", vec![], vec![]);
        }

        let scale_len = scale_notes.len();
        let default_duration = 480u32; // quarter note

        let notes: Vec<u8> = match shape {
            MotifShape::Ascending => {
                (0..length).map(|i| scale_notes[i % scale_len]).collect()
            }
            MotifShape::Descending => {
                (0..length).map(|i| {
                    let idx = (scale_len as isize - 1 - (i as isize)) % scale_len as isize;
                    let idx = idx.rem_euclid(scale_len as isize) as usize;
                    scale_notes[idx]
                }).collect()
            }
            MotifShape::ArchUp => {
                let peak = length / 2;
                (0..length).map(|i| {
                    let idx = if i <= peak {
                        (i * (scale_len - 1)) / peak.max(1)
                    } else {
                        let down = i - peak;
                        let range = (length - 1 - peak).max(1);
                        scale_len - 1 - (down * (scale_len - 1)) / range
                    };
                    scale_notes[idx.min(scale_len - 1)]
                }).collect()
            }
            MotifShape::ArchDown => {
                let valley = length / 2;
                (0..length).map(|i| {
                    let idx = if i <= valley {
                        scale_len - 1 - (i * (scale_len - 1)) / valley.max(1)
                    } else {
                        let up = i - valley;
                        let range = (length - 1 - valley).max(1);
                        (up * (scale_len - 1)) / range
                    };
                    scale_notes[idx.min(scale_len - 1)]
                }).collect()
            }
            MotifShape::Wave => {
                (0..length).map(|i| {
                    let phase = (i as f64 / length as f64) * std::f64::consts::TAU;
                    let normalized = (phase.sin() + 1.0) / 2.0; // 0..1
                    let idx = (normalized * (scale_len - 1) as f64).round() as usize;
                    scale_notes[idx.min(scale_len - 1)]
                }).collect()
            }
            MotifShape::Static => {
                vec![scale_notes[0]; length]
            }
            MotifShape::Random { seed: s } => {
                // Simple LCG
                let mut rng = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (0..length).map(|_| {
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    scale_notes[(rng as usize) % scale_len]
                }).collect()
            }
        };

        // Durations: vary slightly based on seed for interest
        let mut rng = seed.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
        let durations: Vec<u32> = (0..notes.len()).map(|_| {
            rng = rng.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
            let choices = [240u32, 480, 480, 480, 960]; // eighth, quarter×3, half
            choices[(rng as usize) % choices.len()]
        }).collect();

        Motif::new(format!("motif_{}", seed), notes, durations)
    }

    // -----------------------------------------------------------------------
    // Motif development
    // -----------------------------------------------------------------------

    /// Apply a named development technique to a motif.
    ///
    /// Supported techniques: `"invert"`, `"retrograde"`, `"sequence"`,
    /// `"fragment"`, `"augment"`.
    pub fn develop_motif(motif: &Motif, technique: &str, seed: u64) -> Motif {
        match technique {
            "invert" => {
                let axis = motif.notes.iter().copied().sum::<u8>()
                    .wrapping_div(motif.notes.len().max(1) as u8);
                motif.invert(axis)
            }
            "retrograde" => motif.retrograde(),
            "sequence" => {
                // Transpose the motif upward by a 3rd (5 semitones) and append
                let interval: i32 = (seed % 7 + 2) as i32;
                let transposed = motif.transpose(interval);
                let mut notes = motif.notes.clone();
                let mut durs = motif.durations.clone();
                notes.extend_from_slice(&transposed.notes);
                durs.extend_from_slice(&transposed.durations);
                Motif::new(format!("{}_seq", motif.name), notes, durs)
            }
            "fragment" => {
                // Take first half of notes
                let half = (motif.notes.len() / 2).max(1);
                Motif::new(
                    format!("{}_frag", motif.name),
                    motif.notes[..half].to_vec(),
                    motif.durations[..half].to_vec(),
                )
            }
            "augment" => motif.augment(2),
            _ => motif.clone(),
        }
    }

    // -----------------------------------------------------------------------
    // Phrase building
    // -----------------------------------------------------------------------

    /// Build a phrase by repeating / developing a motif to fill `bars` bars.
    pub fn build_phrase(
        motif: &Motif,
        purpose: PhrasePurpose,
        chord: &[u8],
        bars: u32,
    ) -> Phrase {
        let ticks_per_bar = 1920u64; // 4/4 at 480 PPQN
        let target_ticks = ticks_per_bar * bars as u64;

        let mut motifs = Vec::new();
        let mut filled: u64 = 0;
        let motif_ticks: u64 = motif.durations.iter().map(|&d| d as u64).sum();
        let motif_ticks = motif_ticks.max(1);

        // Vary development based on purpose
        let mut iteration = 0usize;
        while filled < target_ticks {
            let m = match purpose {
                PhrasePurpose::Climax => {
                    // Build tension with sequences
                    if iteration % 2 == 0 {
                        motif.transpose((iteration as i32) * 2)
                    } else {
                        motif.augment(1)
                    }
                }
                PhrasePurpose::Resolution => {
                    // Calm descent
                    motif.transpose(-(iteration as i32))
                }
                PhrasePurpose::Bridge => motif.retrograde(),
                _ => motif.clone(),
            };
            filled += motif_ticks;
            motifs.push(m);
            iteration += 1;
            if iteration > 32 { break; } // safety
        }

        Phrase::new(motifs, purpose, chord.to_vec())
    }

    // -----------------------------------------------------------------------
    // Full composition
    // -----------------------------------------------------------------------

    /// Compose a multi-phrase piece in the given form.
    pub fn compose(
        form: CompositionForm,
        key_root: u8,
        mode_name: &str,
        _tempo: f64,
        seed: u64,
    ) -> Vec<Phrase> {
        let scale = Self::build_scale(key_root, mode_name);
        let chord_i = Self::triad(key_root, 0);   // tonic
        let chord_iv = Self::triad(key_root, 5);  // subdominant
        let chord_v = Self::triad(key_root, 7);   // dominant

        // Generate primary (A) and secondary (B) motifs
        let motif_a = Self::generate_motif(&scale, MotifShape::Ascending, 4, seed);
        let motif_b = Self::generate_motif(&scale, MotifShape::ArchUp, 4, seed ^ 0xDEAD);

        // Build stock phrases
        let phrase_a_open = Self::build_phrase(&motif_a, PhrasePurpose::Opening, &chord_i, 2);
        let phrase_a_cont = Self::build_phrase(&motif_a, PhrasePurpose::Continuation, &chord_i, 2);
        let phrase_b = Self::build_phrase(&motif_b, PhrasePurpose::Bridge, &chord_iv, 2);
        let phrase_climax = Self::build_phrase(
            &Self::develop_motif(&motif_a, "sequence", seed),
            PhrasePurpose::Climax,
            &chord_v,
            2,
        );
        let phrase_res = Self::build_phrase(
            &Self::develop_motif(&motif_a, "retrograde", seed),
            PhrasePurpose::Resolution,
            &chord_i,
            2,
        );

        match form {
            CompositionForm::ABA => vec![
                phrase_a_open, phrase_b, phrase_res,
            ],
            CompositionForm::ABAB => vec![
                phrase_a_open.clone(), phrase_b.clone(),
                phrase_a_cont, phrase_b,
            ],
            CompositionForm::Rondo => vec![
                phrase_a_open.clone(),
                phrase_b.clone(),
                Self::build_phrase(&motif_a, PhrasePurpose::Continuation, &chord_i, 2),
                phrase_climax,
                Self::build_phrase(&motif_a, PhrasePurpose::Continuation, &chord_i, 2),
                Self::build_phrase(&motif_b, PhrasePurpose::Bridge, &chord_iv, 2),
                phrase_res,
            ],
            CompositionForm::Binary => vec![phrase_a_open, phrase_b],
            CompositionForm::Ternary => vec![phrase_a_open, phrase_b, phrase_res],
            CompositionForm::ThroughComposed => vec![
                phrase_a_open, phrase_a_cont, phrase_climax, phrase_res,
            ],
        }
    }

    // -----------------------------------------------------------------------
    // Voice leading
    // -----------------------------------------------------------------------

    /// Smooth the melody of a phrase by minimising large leaps.
    pub fn voice_leading_melody(phrase: &Phrase, prev_phrase: Option<&Phrase>) -> Vec<u8> {
        let mut melody = phrase.all_notes();
        if melody.is_empty() {
            return melody;
        }

        // Start from the last note of the previous phrase if available
        let mut prev_note = prev_phrase
            .and_then(|p| p.all_notes().last().copied())
            .unwrap_or(melody[0]);

        for note in melody.iter_mut() {
            let diff = *note as i32 - prev_note as i32;
            if diff.abs() > 7 {
                // Bring closer by an octave
                let adjusted = if diff > 0 {
                    (*note as i32 - 12).clamp(0, 127) as u8
                } else {
                    (*note as i32 + 12).clamp(0, 127) as u8
                };
                *note = adjusted;
            }
            prev_note = *note;
        }
        melody
    }

    // -----------------------------------------------------------------------
    // Tension arc
    // -----------------------------------------------------------------------

    /// Compute a tension value (0.0-1.0) for each phrase, forming an arc.
    pub fn tension_arc(phrases: &[Phrase]) -> Vec<f64> {
        phrases.iter().enumerate().map(|(i, phrase)| {
            let n = phrases.len().max(1);
            // Base tension from position in form
            let positional = i as f64 / n as f64;

            // Melodic tension: higher notes → more tension
            let notes = phrase.all_notes();
            let avg_pitch = if notes.is_empty() {
                60.0
            } else {
                notes.iter().map(|&n| n as f64).sum::<f64>() / notes.len() as f64
            };
            let pitch_tension = (avg_pitch - 60.0).clamp(0.0, 36.0) / 36.0;

            // Purpose-based tension
            let purpose_tension = match phrase.purpose {
                PhrasePurpose::Opening => 0.3,
                PhrasePurpose::Continuation => 0.5,
                PhrasePurpose::Climax => 0.9,
                PhrasePurpose::Resolution => 0.1,
                PhrasePurpose::Bridge => 0.6,
            };

            // Weighted combination
            (positional * 0.3 + pitch_tension * 0.3 + purpose_tension * 0.4).clamp(0.0, 1.0)
        }).collect()
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn build_scale(root: u8, mode: &str) -> Vec<u8> {
        let intervals: &[u8] = match mode {
            "major" | "ionian" => &[0, 2, 4, 5, 7, 9, 11],
            "minor" | "aeolian" => &[0, 2, 3, 5, 7, 8, 10],
            "dorian" => &[0, 2, 3, 5, 7, 9, 10],
            "phrygian" => &[0, 1, 3, 5, 7, 8, 10],
            "lydian" => &[0, 2, 4, 6, 7, 9, 11],
            "mixolydian" => &[0, 2, 4, 5, 7, 9, 10],
            "locrian" => &[0, 1, 3, 5, 6, 8, 10],
            "pentatonic" => &[0, 2, 4, 7, 9],
            "blues" => &[0, 3, 5, 6, 7, 10],
            _ => &[0, 2, 4, 5, 7, 9, 11], // default to major
        };
        intervals.iter()
            .map(|&i| root.saturating_add(i))
            .filter(|&n| n <= 127)
            .collect()
    }

    fn triad(root: u8, offset: u8) -> Vec<u8> {
        let base = root.saturating_add(offset);
        vec![base, base.saturating_add(4), base.saturating_add(7)]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_ascending_motif() {
        let scale = vec![60u8, 62, 64, 65, 67, 69, 71, 72];
        let motif = AlgorithmicComposer::generate_motif(&scale, MotifShape::Ascending, 4, 42);
        assert_eq!(motif.notes.len(), 4);
        // Ascending: first note should be <= last
        assert!(motif.notes[0] <= motif.notes[3]);
    }

    #[test]
    fn test_invert() {
        let m = Motif::new("test", vec![60, 64, 67], vec![480, 480, 480]);
        let inv = m.invert(64);
        assert_eq!(inv.notes[0], 68); // 64 + (64 - 60) = 68
    }

    #[test]
    fn test_retrograde() {
        let m = Motif::new("test", vec![60, 62, 64], vec![480, 240, 960]);
        let r = m.retrograde();
        assert_eq!(r.notes, vec![64, 62, 60]);
        assert_eq!(r.durations, vec![960, 240, 480]);
    }

    #[test]
    fn test_transpose() {
        let m = Motif::new("test", vec![60, 64], vec![480, 480]);
        let t = m.transpose(7);
        assert_eq!(t.notes, vec![67, 71]);
    }

    #[test]
    fn test_augment_diminish() {
        let m = Motif::new("test", vec![60], vec![480]);
        assert_eq!(m.augment(2).durations[0], 960);
        assert_eq!(m.diminish(2).durations[0], 240);
    }

    #[test]
    fn test_develop_motif_fragment() {
        let m = Motif::new("test", vec![60, 62, 64, 65], vec![480, 480, 480, 480]);
        let frag = AlgorithmicComposer::develop_motif(&m, "fragment", 1);
        assert!(frag.notes.len() < m.notes.len());
    }

    #[test]
    fn test_compose_aba() {
        let phrases = AlgorithmicComposer::compose(
            CompositionForm::ABA, 60, "major", 120.0, 12345,
        );
        assert_eq!(phrases.len(), 3);
    }

    #[test]
    fn test_tension_arc_climax() {
        let scale = vec![60u8, 62, 64, 65, 67];
        let m = AlgorithmicComposer::generate_motif(&scale, MotifShape::Ascending, 4, 1);
        let phrases = vec![
            Phrase::new(vec![m.clone()], PhrasePurpose::Opening, vec![60, 64, 67]),
            Phrase::new(vec![m.clone()], PhrasePurpose::Climax, vec![67, 71, 74]),
            Phrase::new(vec![m.clone()], PhrasePurpose::Resolution, vec![60, 64, 67]),
        ];
        let arc = AlgorithmicComposer::tension_arc(&phrases);
        assert_eq!(arc.len(), 3);
        // Climax should have highest tension
        assert!(arc[1] > arc[2]);
    }

    #[test]
    fn test_composition_form_display() {
        assert_eq!(CompositionForm::ABA.to_string(), "ABA");
        assert_eq!(CompositionForm::Rondo.to_string(), "Rondo (ABACADA)");
    }
}
