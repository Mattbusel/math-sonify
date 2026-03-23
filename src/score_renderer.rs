//! Musical score rendering to ASCII/text output.

/// The clef used for a staff.
#[derive(Debug, Clone, PartialEq)]
pub enum Clef {
    Treble,
    Bass,
    Alto,
    Tenor,
}

impl Clef {
    /// Returns the MIDI note number of the middle staff line.
    pub fn middle_line_midi(&self) -> u8 {
        match self {
            Clef::Treble => 71, // B4
            Clef::Bass => 50,   // D3
            Clef::Alto => 60,   // C4
            Clef::Tenor => 57,  // A3
        }
    }
}

/// Note duration.
#[derive(Debug, Clone, PartialEq)]
pub enum NoteDuration {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}

impl NoteDuration {
    /// Duration in beats at the given tempo (beats per minute).
    pub fn beats(&self, _tempo_bpm: f64) -> f64 {
        match self {
            NoteDuration::Whole => 4.0,
            NoteDuration::Half => 2.0,
            NoteDuration::Quarter => 1.0,
            NoteDuration::Eighth => 0.5,
            NoteDuration::Sixteenth => 0.25,
        }
    }
}

/// A single note (or rest) in a score.
#[derive(Debug, Clone)]
pub struct ScoreNote {
    /// MIDI note number; `None` = rest.
    pub pitch: Option<u8>,
    pub duration: NoteDuration,
    pub octave: u8,
    /// -1 = flat, 0 = natural (None), +1 = sharp.
    pub accidental: Option<i8>,
    pub dotted: bool,
    pub tie: bool,
}

/// Time signature.
#[derive(Debug, Clone)]
pub struct TimeSignature {
    pub numerator: u8,
    pub denominator: u8,
}

impl TimeSignature {
    pub fn beats_per_bar(&self) -> f64 {
        f64::from(self.numerator)
    }

    pub fn beat_value(&self) -> f64 {
        4.0 / f64::from(self.denominator)
    }
}

/// Key signature.
#[derive(Debug, Clone)]
pub struct KeySignature {
    /// Positive = sharps, negative = flats (-7..=+7).
    pub sharps_or_flats: i8,
    pub name: String,
}

impl KeySignature {
    /// Convert a MIDI note number to a note name respecting the key signature.
    pub fn note_name(&self, midi_note: u8) -> String {
        let names_sharp = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let names_flat = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
        let idx = (midi_note % 12) as usize;
        if self.sharps_or_flats >= 0 {
            names_sharp[idx].to_string()
        } else {
            names_flat[idx].to_string()
        }
    }
}

/// A single measure.
#[derive(Debug, Clone)]
pub struct Measure {
    pub notes: Vec<ScoreNote>,
    pub time_sig: TimeSignature,
    pub key_sig: KeySignature,
}

impl Measure {
    /// Total duration of all notes in beats.
    pub fn duration_beats(&self) -> f64 {
        let tempo = 120.0;
        self.notes.iter().map(|n| {
            let mut d = n.duration.beats(tempo);
            if n.dotted { d *= 1.5; }
            d
        }).sum()
    }

    /// True if the measure is exactly full (duration == beats_per_bar).
    pub fn is_full(&self) -> bool {
        (self.duration_beats() - self.time_sig.beats_per_bar()).abs() < 1e-9
    }
}

/// A staff (one instrument line).
#[derive(Debug, Clone)]
pub struct Staff {
    pub clef: Clef,
    pub measures: Vec<Measure>,
}

/// A complete musical score.
#[derive(Debug, Clone)]
pub struct Score {
    pub title: String,
    pub composer: String,
    pub tempo_bpm: f64,
    pub staves: Vec<Staff>,
}

/// Renders a `Score` to ASCII text.
pub struct ScoreRenderer;

impl ScoreRenderer {
    /// Render a single measure as a 5-line ASCII staff.
    ///
    /// Returns a `Vec<String>` with one element per staff line (5 lines + note line).
    pub fn render_measure_ascii(measure: &Measure, width: usize) -> Vec<String> {
        let num_notes = measure.notes.len().max(1);
        let col_width = (width.saturating_sub(2)) / num_notes;

        // 7 rows: lines 4 (top) down to 0 (bottom), plus note symbols row
        let mut rows: Vec<String> = vec![String::new(); 7];

        // Staff lines: rows 1, 2, 3, 4, 5 correspond to staff lines
        for row in 0..7 {
            let mut line = String::new();
            line.push('|');
            for note_idx in 0..num_notes {
                let note = &measure.notes[note_idx];
                let pos = Self::note_to_staff_position(note, &Clef::Treble);
                // Staff lines are at positions 0, 2, 4, 6, 8 (even numbers)
                // Row 0 = position 8 (top), row 4 = position 0 (bottom)
                let staff_row_pos = (4 - row as i32) * 2;
                let symbol = if row == 6 {
                    // Note name row
                    let sym = Self::render_note_symbol(note);
                    format!("{:<width$}", sym, width = col_width)
                } else if pos == staff_row_pos {
                    // Note sits on this line
                    let sym = Self::render_note_symbol(note);
                    format!("{:-<width$}", sym, width = col_width)
                } else if staff_row_pos % 2 == 0 && row < 5 {
                    // Staff line (even positions 0..8)
                    format!("{:-<width$}", "-", width = col_width)
                } else {
                    format!("{: <width$}", " ", width = col_width)
                };
                line.push_str(&symbol);
            }
            line.push('|');
            rows[row] = line;
        }

        rows
    }

    /// Render a full staff to ASCII, `measures_per_line` measures per row.
    pub fn render_staff_ascii(staff: &Staff, measures_per_line: usize) -> String {
        let mut out = String::new();
        let clef_label = match staff.clef {
            Clef::Treble => "𝄞 ",
            Clef::Bass => "𝄢 ",
            Clef::Alto => "B ",
            Clef::Tenor => "T ",
        };

        let chunks: Vec<&[Measure]> = staff.measures.chunks(measures_per_line).collect();
        for chunk in chunks {
            // Build rows across all measures in this line
            let mut combined_rows: Vec<String> = vec![String::new(); 7];
            combined_rows[0].push_str(clef_label);
            for measure in chunk {
                let rows = Self::render_measure_ascii(measure, 24);
                for (i, row) in rows.iter().enumerate() {
                    combined_rows[i].push_str(row);
                }
            }
            for row in &combined_rows {
                out.push_str(row);
                out.push('\n');
            }
            out.push('\n');
        }

        out
    }

    /// Render a complete score with header.
    pub fn render_score(score: &Score) -> String {
        let mut out = String::new();
        let width = 80;
        let sep = "=".repeat(width);

        out.push_str(&sep);
        out.push('\n');
        out.push_str(&format!("  {}\n", score.title));
        out.push_str(&format!("  Composer: {}   Tempo: {} BPM\n", score.composer, score.tempo_bpm));
        out.push_str(&sep);
        out.push('\n');

        for (i, staff) in score.staves.iter().enumerate() {
            out.push_str(&format!("--- Staff {} ---\n", i + 1));
            out.push_str(&Self::render_staff_ascii(staff, 4));
        }

        out.push_str(&sep);
        out.push('\n');
        out
    }

    /// Compute the staff position of a note relative to the clef's middle line.
    ///
    /// Returns an integer: 0 = middle staff line, positive = above.
    pub fn note_to_staff_position(note: &ScoreNote, clef: &Clef) -> i32 {
        let Some(pitch) = note.pitch else {
            return i32::MIN; // rests have no staff position
        };
        let middle = clef.middle_line_midi() as i32;
        let semitones = pitch as i32 - middle;
        // Approximate: each diatonic step ≈ ~2 semitones on average
        semitones / 2
    }

    /// Return an ASCII character representing the note's duration/type.
    pub fn render_note_symbol(note: &ScoreNote) -> char {
        if note.pitch.is_none() {
            return 'r'; // rest
        }
        match note.duration {
            NoteDuration::Whole => 'o',
            NoteDuration::Half => 'd',
            NoteDuration::Quarter => 'q',
            NoteDuration::Eighth => 'e',
            NoteDuration::Sixteenth => 's',
        }
    }

    /// Convert a MIDI note number and duration into a `ScoreNote`.
    pub fn midi_to_score_note(midi: u8, duration: NoteDuration) -> ScoreNote {
        let octave = midi / 12;
        ScoreNote {
            pitch: Some(midi),
            duration,
            octave,
            accidental: None,
            dotted: false,
            tie: false,
        }
    }

    /// Build a `Score` from parallel arrays of MIDI notes and durations.
    pub fn from_midi_sequence(notes: &[u8], durations: &[NoteDuration], tempo: f64) -> Score {
        let time_sig = TimeSignature { numerator: 4, denominator: 4 };
        let key_sig = KeySignature { sharps_or_flats: 0, name: "C major".to_string() };

        let score_notes: Vec<ScoreNote> = notes
            .iter()
            .zip(durations.iter())
            .map(|(&midi, dur)| Self::midi_to_score_note(midi, dur.clone()))
            .collect();

        // Pack notes into measures of 4 beats
        let mut measures: Vec<Measure> = Vec::new();
        let mut current_notes: Vec<ScoreNote> = Vec::new();
        let mut beats = 0.0f64;

        for note in score_notes {
            let dur = note.duration.beats(tempo);
            if beats + dur > 4.0 + 1e-9 {
                if !current_notes.is_empty() {
                    measures.push(Measure {
                        notes: current_notes.clone(),
                        time_sig: time_sig.clone(),
                        key_sig: key_sig.clone(),
                    });
                    current_notes.clear();
                    beats = 0.0;
                }
            }
            beats += dur;
            current_notes.push(note);
        }
        if !current_notes.is_empty() {
            measures.push(Measure {
                notes: current_notes,
                time_sig: time_sig.clone(),
                key_sig: key_sig.clone(),
            });
        }

        Score {
            title: "Untitled".to_string(),
            composer: "Unknown".to_string(),
            tempo_bpm: tempo,
            staves: vec![Staff {
                clef: Clef::Treble,
                measures,
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_duration_beats() {
        assert_eq!(NoteDuration::Whole.beats(120.0), 4.0);
        assert_eq!(NoteDuration::Quarter.beats(120.0), 1.0);
    }

    #[test]
    fn test_time_signature() {
        let ts = TimeSignature { numerator: 3, denominator: 4 };
        assert_eq!(ts.beats_per_bar(), 3.0);
    }

    #[test]
    fn test_key_signature_note_name() {
        let ks = KeySignature { sharps_or_flats: 1, name: "G major".to_string() };
        assert_eq!(ks.note_name(60), "C");
        assert_eq!(ks.note_name(61), "C#");
    }

    #[test]
    fn test_midi_to_score_note() {
        let note = ScoreRenderer::midi_to_score_note(60, NoteDuration::Quarter);
        assert_eq!(note.pitch, Some(60));
    }

    #[test]
    fn test_from_midi_sequence() {
        let notes = [60u8, 62, 64, 65, 67];
        let durs = [
            NoteDuration::Quarter,
            NoteDuration::Quarter,
            NoteDuration::Quarter,
            NoteDuration::Quarter,
            NoteDuration::Quarter,
        ];
        let score = ScoreRenderer::from_midi_sequence(&notes, &durs, 120.0);
        assert!(!score.staves.is_empty());
        assert!(!score.staves[0].measures.is_empty());
    }

    #[test]
    fn test_render_score() {
        let score = ScoreRenderer::from_midi_sequence(&[60, 62, 64], &[NoteDuration::Quarter, NoteDuration::Quarter, NoteDuration::Half], 120.0);
        let rendered = ScoreRenderer::render_score(&score);
        assert!(rendered.contains("Untitled"));
    }

    #[test]
    fn test_measure_is_full() {
        let ts = TimeSignature { numerator: 4, denominator: 4 };
        let ks = KeySignature { sharps_or_flats: 0, name: "C major".to_string() };
        let notes = vec![
            ScoreNote { pitch: Some(60), duration: NoteDuration::Quarter, octave: 5, accidental: None, dotted: false, tie: false },
            ScoreNote { pitch: Some(62), duration: NoteDuration::Quarter, octave: 5, accidental: None, dotted: false, tie: false },
            ScoreNote { pitch: Some(64), duration: NoteDuration::Quarter, octave: 5, accidental: None, dotted: false, tie: false },
            ScoreNote { pitch: Some(65), duration: NoteDuration::Quarter, octave: 5, accidental: None, dotted: false, tie: false },
        ];
        let m = Measure { notes, time_sig: ts, key_sig: ks };
        assert!(m.is_full());
    }
}
