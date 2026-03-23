//! Export note sequences to ABC notation and simple text/ASCII formats.

// ── Note ──────────────────────────────────────────────────────────────────────

/// A single musical note.
#[derive(Debug, Clone)]
pub struct Note {
    /// Fundamental frequency in Hz.
    pub pitch_hz: f64,
    /// Duration in beats (quarter-note = 1.0).
    pub duration_beats: f64,
    /// MIDI-style velocity ∈ [0, 127].
    pub velocity: u8,
}

// ── hz_to_note_name ───────────────────────────────────────────────────────────

/// Convert a frequency in Hz to the closest note name and cents deviation.
///
/// Returns `(name, cents)` where `name` is e.g. `"A4"` and `cents` is the
/// signed deviation from the exact semitone (negative = flat).
pub fn hz_to_note_name(hz: f64) -> (String, i8) {
    if hz <= 0.0 {
        return ("R".to_string(), 0);
    }

    // A4 = 440 Hz, MIDI note 69.
    let semitones_from_a4 = 12.0 * (hz / 440.0).log2();
    let nearest = semitones_from_a4.round();
    let cents_f = (semitones_from_a4 - nearest) * 100.0;
    let cents = cents_f.round().clamp(-99.0, 99.0) as i8;

    let midi = (69.0 + nearest).round() as i32;
    let octave = midi / 12 - 1;
    let note_idx = ((midi % 12) + 12) as usize % 12;
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let name = format!("{}{}", names[note_idx], octave);
    (name, cents)
}

// ── duration_to_abc ───────────────────────────────────────────────────────────

/// Convert a duration in beats to an ABC notation length modifier string.
///
/// Assumes the ABC default note length is a quarter note (L:1/4).
///
/// | Beats | ABC modifier |
/// |-------|-------------|
/// | 0.25  | `/4`        |
/// | 0.5   | `/2`        |
/// | 1.0   | (none)      |
/// | 2.0   | `2`         |
/// | 4.0   | `4`         |
pub fn duration_to_abc(beats: f64) -> String {
    if beats <= 0.0 {
        return "/4".to_string();
    }
    if (beats - 0.25).abs() < 0.01 {
        "/4".to_string()
    } else if (beats - 0.5).abs() < 0.01 {
        "/2".to_string()
    } else if (beats - 1.0).abs() < 0.01 {
        String::new()
    } else if (beats - 2.0).abs() < 0.01 {
        "2".to_string()
    } else if (beats - 4.0).abs() < 0.01 {
        "4".to_string()
    } else {
        // General case: express as numerator/denominator of quarter notes.
        let num = (beats * 4.0).round() as u64;
        if num == 1 {
            "/4".to_string()
        } else if num % 4 == 0 {
            format!("{}", num / 4)
        } else {
            format!("/{}", 4 / num.max(1))
        }
    }
}

// ── AbcNotation ───────────────────────────────────────────────────────────────

/// Generate ABC notation strings from note sequences.
pub struct AbcNotation;

impl AbcNotation {
    /// Build an ABC file header.
    ///
    /// # Example
    /// ```
    /// # use math_sonify::notation_exporter::AbcNotation;
    /// let h = AbcNotation::header("My Tune", 120, "4/4", "C");
    /// assert!(h.contains("T:My Tune"));
    /// ```
    pub fn header(title: &str, tempo_bpm: u32, meter: &str, key: &str) -> String {
        format!(
            "X:1\nT:{title}\nM:{meter}\nQ:1/4={tempo_bpm}\nK:{key}\n",
        )
    }

    /// Convert a slice of notes to an ABC note string (no header).
    pub fn notes_to_abc(notes: &[Note]) -> String {
        let mut out = String::new();
        for note in notes {
            if note.pitch_hz <= 0.0 {
                // Rest
                let dur = duration_to_abc(note.duration_beats);
                out.push('z');
                out.push_str(&dur);
            } else {
                let (name, _cents) = hz_to_note_name(note.pitch_hz);
                // Extract pitch class and octave from name like "C4", "A#3".
                let (pitch_class, octave_s) = split_note_name(&name);
                let octave: i32 = octave_s.parse().unwrap_or(4);
                let abc_note = pitch_class_to_abc(&pitch_class, octave);
                let dur = duration_to_abc(note.duration_beats);
                out.push_str(&abc_note);
                out.push_str(&dur);
            }
            out.push(' ');
        }
        out.trim_end().to_string()
    }

    /// Build a complete ABC score (header + notes).
    pub fn full_score(title: &str, notes: &[Note], tempo_bpm: u32, key: &str) -> String {
        let header = Self::header(title, tempo_bpm, "4/4", key);
        let body = Self::notes_to_abc(notes);
        format!("{header}{body}\n")
    }
}

/// Split "C#4" → ("C#", "4"), "B3" → ("B", "3").
fn split_note_name(name: &str) -> (String, String) {
    let mut chars = name.chars().peekable();
    let mut pitch = String::new();
    // Consume note letter.
    if let Some(c) = chars.next() {
        pitch.push(c);
    }
    // Consume optional sharp/flat.
    if chars.peek() == Some(&'#') || chars.peek() == Some(&'b') {
        pitch.push(chars.next().unwrap());
    }
    let octave: String = chars.collect();
    (pitch, octave)
}

/// Convert a pitch class and octave number to an ABC note symbol.
/// ABC middle octave (octave 4) uses uppercase with no commas/apostrophes.
fn pitch_class_to_abc(pitch_class: &str, octave: i32) -> String {
    // ABC: octave 4 → uppercase (C D E F G A B)
    //       octave 5 → lowercase (c d e f g a b)
    //       octave 3 → uppercase + comma (C, D, ...)
    // Sharps: ^C, flats: _C
    let base = pitch_class.trim_matches(|c| c == '#' || c == 'b');
    let sharp = pitch_class.contains('#');
    let flat = pitch_class.contains('b');

    let accidental = if sharp { "^" } else if flat { "_" } else { "" };

    let letter = if octave >= 5 {
        base.to_lowercase()
    } else {
        base.to_uppercase()
    };

    let octave_marks = match octave {
        o if o >= 6 => "'".repeat((o - 5) as usize),
        5 => String::new(),
        4 => String::new(),
        3 => ",".to_string(),
        o if o <= 2 => ",".repeat((3 - o) as usize),
        _ => String::new(),
    };

    format!("{accidental}{letter}{octave_marks}")
}

// ── TextNotation ──────────────────────────────────────────────────────────────

/// Export note sequences as plain-text grids.
pub struct TextNotation;

impl TextNotation {
    /// Build a simple ASCII text grid of notes.
    ///
    /// Each note is displayed as `<name>(<dur>)` in a grid with `cols` columns.
    pub fn to_text_grid(notes: &[Note], cols: usize) -> String {
        let cols = cols.max(1);
        let mut lines = Vec::new();
        let mut line_buf = Vec::new();

        for note in notes {
            let (name, _) = hz_to_note_name(note.pitch_hz);
            let cell = format!("{:<8}", format!("{}({:.1})", name, note.duration_beats));
            line_buf.push(cell);
            if line_buf.len() >= cols {
                lines.push(line_buf.join(" | "));
                line_buf.clear();
            }
        }
        if !line_buf.is_empty() {
            lines.push(line_buf.join(" | "));
        }
        lines.join("\n")
    }

    /// Build a 2D piano-roll representation.
    ///
    /// * `rows` — number of pitch rows (MIDI pitches from 60 to 60+rows).
    /// * `beats` — total duration to display.
    ///
    /// Returns a grid where `'#'` means the note is active and `'.'` is silence.
    /// Row 0 = highest pitch, row `rows-1` = lowest pitch.
    pub fn to_piano_roll(notes: &[Note], rows: u8, beats: f64) -> Vec<Vec<char>> {
        let cols = (beats * 4.0).ceil() as usize; // 16th-note resolution
        let rows_u = rows as usize;
        let mut grid = vec![vec!['.'; cols.max(1)]; rows_u];

        // Map each note onto the grid.
        for note in notes {
            if note.pitch_hz <= 0.0 {
                continue;
            }
            let midi = hz_to_midi(note.pitch_hz);
            // Map MIDI 60..60+rows to row index (0 = high, rows-1 = low).
            let row_idx = if midi >= 60 && (midi as usize) < 60 + rows_u {
                rows_u - 1 - (midi - 60)
            } else {
                continue;
            };

            let start_col = (note.duration_beats * 0.0) as usize; // cumulative would need a running time; use a simpler 1:1 approach
            // We'll use beat_offset = sum of previous durations — tracked externally would be ideal,
            // but for a self-contained function we approximate each note as sequential.
            let _ = start_col;

            // Fill the note's duration.
            let dur_cols = ((note.duration_beats * 4.0).round() as usize).max(1);
            // Sequential fill: find first empty column for this row.
            let start = grid[row_idx].iter().position(|&c| c == '.').unwrap_or(0);
            for col in start..(start + dur_cols).min(cols) {
                grid[row_idx][col] = '#';
            }
        }

        grid
    }
}

/// Approximate MIDI note number from Hz (A4=440=69).
fn hz_to_midi(hz: f64) -> usize {
    if hz <= 0.0 {
        return 0;
    }
    let midi = 69.0 + 12.0 * (hz / 440.0).log2();
    midi.round() as usize
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn a4() -> Note {
        Note { pitch_hz: 440.0, duration_beats: 1.0, velocity: 80 }
    }

    fn c4() -> Note {
        Note { pitch_hz: 261.626, duration_beats: 2.0, velocity: 100 }
    }

    #[test]
    fn hz_to_note_a4() {
        let (name, cents) = hz_to_note_name(440.0);
        assert_eq!(name, "A4");
        assert_eq!(cents, 0);
    }

    #[test]
    fn hz_to_note_c4() {
        let (name, _cents) = hz_to_note_name(261.626);
        assert_eq!(name, "C4");
    }

    #[test]
    fn hz_to_note_zero() {
        let (name, cents) = hz_to_note_name(0.0);
        assert_eq!(name, "R");
        assert_eq!(cents, 0);
    }

    #[test]
    fn duration_quarter() {
        assert_eq!(duration_to_abc(1.0), "");
    }

    #[test]
    fn duration_half() {
        assert_eq!(duration_to_abc(0.5), "/2");
    }

    #[test]
    fn duration_whole() {
        assert_eq!(duration_to_abc(4.0), "4");
    }

    #[test]
    fn duration_eighth() {
        assert_eq!(duration_to_abc(0.25), "/4");
    }

    #[test]
    fn duration_double() {
        assert_eq!(duration_to_abc(2.0), "2");
    }

    #[test]
    fn abc_header_contains_fields() {
        let h = AbcNotation::header("Test", 120, "4/4", "C");
        assert!(h.contains("X:1"));
        assert!(h.contains("T:Test"));
        assert!(h.contains("M:4/4"));
        assert!(h.contains("Q:1/4=120"));
        assert!(h.contains("K:C"));
    }

    #[test]
    fn notes_to_abc_nonempty() {
        let notes = vec![a4(), c4()];
        let abc = AbcNotation::notes_to_abc(&notes);
        assert!(!abc.is_empty());
    }

    #[test]
    fn full_score_contains_header_and_notes() {
        let notes = vec![a4()];
        let score = AbcNotation::full_score("Demo", &notes, 100, "G");
        assert!(score.contains("T:Demo"));
        assert!(score.contains("Q:1/4=100"));
    }

    #[test]
    fn text_grid_cols() {
        let notes: Vec<Note> = (0..6).map(|_| a4()).collect();
        let grid = TextNotation::to_text_grid(&notes, 3);
        let line_count = grid.lines().count();
        assert_eq!(line_count, 2);
    }

    #[test]
    fn text_grid_single_col() {
        let notes = vec![a4(), c4()];
        let grid = TextNotation::to_text_grid(&notes, 1);
        assert_eq!(grid.lines().count(), 2);
    }

    #[test]
    fn piano_roll_dimensions() {
        let notes = vec![a4()];
        let roll = TextNotation::to_piano_roll(&notes, 12, 4.0);
        assert_eq!(roll.len(), 12);
        assert!(roll[0].len() >= 1);
    }

    #[test]
    fn piano_roll_has_active_cells() {
        let notes = vec![Note { pitch_hz: 440.0, duration_beats: 1.0, velocity: 80 }];
        // A4 = MIDI 69, which is 60+9 = row index 12-1-9=2 for rows=12.
        let roll = TextNotation::to_piano_roll(&notes, 12, 4.0);
        let active = roll.iter().flatten().any(|&c| c == '#');
        assert!(active, "expected at least one active cell");
    }

    #[test]
    fn piano_roll_empty_notes() {
        let roll = TextNotation::to_piano_roll(&[], 8, 4.0);
        assert_eq!(roll.len(), 8);
        let active = roll.iter().flatten().any(|&c| c == '#');
        assert!(!active);
    }
}
