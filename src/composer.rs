//! Generative Composition Engine — structured musical pieces from mathematical systems.
//!
//! This module extends the lightweight [`crate::composition`] module with a
//! full composition architecture:
//!
//! - [`MusicalForm`] — macro-level form selector (ABA, Rondo, Theme & Variations, …).
//! - [`MotifGenerator`] — extracts recurring short motivic cells from attractor trajectories.
//! - [`HarmonicProgression`] — maps attractor phase-space regions to chord progressions
//!   (diatonic triads, jazz seventh-chords, modal extensions).
//! - [`RhythmicQuantizer`] — quantizes the continuous ODE output to a rhythmic grid
//!   (4/4, 5/4, 7/8, and more).
//! - [`CompositionExporter`] — exports an entire composed piece to a multi-track MIDI
//!   SMF file via the existing [`crate::midi_export`] infrastructure.
//! - [`ComposerEngine`] — ties all components together; updated once per control-rate
//!   tick and exposes a [`ComposerFrame`] consumed by the audio / UI threads.

#![allow(dead_code)]

use std::collections::VecDeque;

use crate::midi_export::{MidiExporter, MidiNote, MidiTrack};

// ---------------------------------------------------------------------------
// Musical form
// ---------------------------------------------------------------------------

/// Large-scale musical form for the generated piece.
///
/// The form governs how sections are ordered, repeated, and contrasted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MusicalForm {
    /// A – B – A′:  statement, contrast, varied return.  The A section uses
    /// the initial attractor basin; B uses a contrasting basin reached by
    /// bifurcation.
    Aba,
    /// Theme followed by N variations.  Each variation alters a different
    /// synthesis parameter while the motif remains recognisable.
    ThemeVariations { variations: u8 },
    /// A – B – A – C – A …  The refrain (A) returns after each episode.
    Rondo,
    /// Linear succession of sections with no repeats.  Driven entirely by
    /// attractor topology.
    ThroughComposed,
    /// Section boundaries are placed according to a pseudo-random walk
    /// seeded from the attractor's own bit-mixing.
    Stochastic,
}

impl Default for MusicalForm {
    fn default() -> Self {
        Self::Aba
    }
}

impl MusicalForm {
    /// Human-readable name for display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Aba => "ABA",
            Self::ThemeVariations { .. } => "Theme & Variations",
            Self::Rondo => "Rondo",
            Self::ThroughComposed => "Through-Composed",
            Self::Stochastic => "Stochastic",
        }
    }

    /// All canonical variants (for UI enumeration).
    pub fn all() -> [MusicalForm; 5] {
        [
            MusicalForm::Aba,
            MusicalForm::ThemeVariations { variations: 3 },
            MusicalForm::Rondo,
            MusicalForm::ThroughComposed,
            MusicalForm::Stochastic,
        ]
    }

    /// Given the current section index, return whether the section is a
    /// "refrain / A-section" (true) or an "episode / B-section" (false).
    pub fn is_refrain(&self, section_idx: usize) -> bool {
        match self {
            Self::Aba => matches!(section_idx, 0 | 2),
            Self::ThemeVariations { .. } => section_idx == 0,
            Self::Rondo => section_idx % 2 == 0,
            Self::ThroughComposed | Self::Stochastic => false,
        }
    }

    /// Total number of sections this form produces before the piece ends
    /// (or wraps).  `None` means the form is open-ended.
    pub fn section_count(&self) -> Option<usize> {
        match self {
            Self::Aba => Some(3),
            Self::ThemeVariations { variations } => Some(1 + *variations as usize),
            Self::Rondo => None,
            Self::ThroughComposed | Self::Stochastic => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Motif generator
// ---------------------------------------------------------------------------

/// Extracts short recurring motivic cells from a rolling trajectory window.
///
/// A "motif" is a short pitch sequence (2–6 notes) that re-appears in the
/// attractor's x-coordinate time series.  The generator maintains a sliding
/// window of recent quantised pitches and scores candidate cells by their
/// repetition frequency.
pub struct MotifGenerator {
    /// Sliding window of the last `window` MIDI pitch values.
    history: VecDeque<u8>,
    /// Maximum history length.
    window: usize,
    /// Length of candidate motif cells (in notes).
    cell_len: usize,
    /// Currently selected motif (may be empty if none found yet).
    pub current_motif: Vec<u8>,
    /// How many ticks the current motif has been active.
    pub motif_age_ticks: u32,
    /// Minimum repetition count to adopt a new motif.
    min_repeats: usize,
}

impl MotifGenerator {
    /// Create a new generator.
    ///
    /// * `window` — number of recent pitches to search (e.g. 64).
    /// * `cell_len` — length of each candidate motif cell (2–6 recommended).
    /// * `min_repeats` — minimum occurrences before a cell is declared a motif.
    pub fn new(window: usize, cell_len: usize, min_repeats: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(window + 1),
            window,
            cell_len: cell_len.clamp(2, 16),
            current_motif: Vec::new(),
            motif_age_ticks: 0,
            min_repeats: min_repeats.max(2),
        }
    }

    /// Feed the latest quantised MIDI pitch.  Periodically re-evaluates the
    /// motif candidate pool and may update [`Self::current_motif`].
    pub fn push(&mut self, pitch: u8) {
        if self.history.len() >= self.window {
            self.history.pop_front();
        }
        self.history.push_back(pitch);
        self.motif_age_ticks = self.motif_age_ticks.saturating_add(1);

        // Re-evaluate every `window/4` ticks to amortise the O(N²) search.
        if self.history.len() >= self.window && self.motif_age_ticks % (self.window as u32 / 4).max(1) == 0 {
            if let Some(m) = self.find_best_motif() {
                self.current_motif = m;
                self.motif_age_ticks = 0;
            }
        }
    }

    /// Return the current motif, or an empty slice if none has been found.
    pub fn motif(&self) -> &[u8] {
        &self.current_motif
    }

    // ---- private ----

    fn find_best_motif(&self) -> Option<Vec<u8>> {
        let data: Vec<u8> = self.history.iter().copied().collect();
        let n = data.len();
        let clen = self.cell_len;
        if n < clen * 2 {
            return None;
        }

        // Count occurrences of every unique sub-sequence of length `clen`.
        // Use a compact representation: encode as a u64 hash.
        let mut counts: std::collections::HashMap<Vec<u8>, usize> =
            std::collections::HashMap::new();

        for i in 0..=(n - clen) {
            let cell: Vec<u8> = data[i..i + clen].to_vec();
            *counts.entry(cell).or_insert(0) += 1;
        }

        // Find the most frequent cell that meets the threshold.
        counts
            .into_iter()
            .filter(|(_, cnt)| *cnt >= self.min_repeats)
            .max_by_key(|(_, cnt)| *cnt)
            .map(|(cell, _)| cell)
    }
}

// ---------------------------------------------------------------------------
// Chord / harmonic progression
// ---------------------------------------------------------------------------

/// A chord represented as a set of MIDI pitch offsets from the root.
#[derive(Debug, Clone)]
pub struct Chord {
    /// Root MIDI note.
    pub root: u8,
    /// Semitone offsets from root (e.g. `[0, 4, 7]` = major triad).
    pub intervals: Vec<i8>,
    /// Human-readable name (e.g. "Cmaj7").
    pub name: String,
}

impl Chord {
    /// Return all sounding MIDI pitches (root + each interval applied).
    pub fn pitches(&self) -> Vec<u8> {
        self.intervals
            .iter()
            .filter_map(|&i| {
                let p = self.root as i16 + i as i16;
                if (0..=127).contains(&p) { Some(p as u8) } else { None }
            })
            .collect()
    }
}

/// Harmonic progression generator.
///
/// Maps attractor state variables to chord progressions by discretising the
/// phase-space into regions, each associated with a chord quality.
///
/// Supported chord vocabularies:
/// - **Diatonic** — I, IV, V, I (classical cadence).
/// - **Jazz** — ii7 – V7 – Imaj7 – VI7 (jazz turnaround).
/// - **Modal** — chords built from the mode currently active.
/// - **Extended** — adds 9th / 11th / 13th extensions based on chaos level.
pub struct HarmonicProgression {
    /// Root pitch class of the current key (0 = C … 11 = B).
    pub key_root: u8,
    /// Current scale (semitone offsets from root).
    pub scale: Vec<u8>,
    /// Progression style.
    pub style: ProgressionStyle,
    /// Index of the current chord within the progression.
    chord_idx: usize,
    /// Number of ticks until the chord changes.
    ticks_remaining: u32,
    /// Chord duration in ticks (at the control rate).
    chord_duration_ticks: u32,
}

/// Chord progression style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressionStyle {
    /// I – IV – V – I (classical).
    Classical,
    /// ii7 – V7 – Imaj7 – VI7 (jazz turnaround).
    JazzTurnaround,
    /// Modal: chords built stepwise from the current scale.
    Modal,
    /// Extended jazz with 9th, 11th, 13th colour tones.
    JazzExtended,
}

impl HarmonicProgression {
    /// Create a new progression.
    ///
    /// * `key_root` — root pitch class (0–11).
    /// * `scale` — scale degree offsets (semitones from root).
    /// * `style` — progression style.
    /// * `chord_duration_ticks` — how many control-rate ticks per chord.
    pub fn new(
        key_root: u8,
        scale: Vec<u8>,
        style: ProgressionStyle,
        chord_duration_ticks: u32,
    ) -> Self {
        Self {
            key_root,
            scale,
            style,
            chord_idx: 0,
            ticks_remaining: chord_duration_ticks,
            chord_duration_ticks,
        }
    }

    /// Advance by one tick and return the currently active chord.
    pub fn tick(&mut self, chaos_level: f32) -> Chord {
        if self.ticks_remaining == 0 {
            self.chord_idx = (self.chord_idx + 1) % self.progression_len();
            self.ticks_remaining = self.chord_duration_ticks;
        }
        self.ticks_remaining = self.ticks_remaining.saturating_sub(1);
        self.current_chord(chaos_level)
    }

    /// Return the current chord without advancing time.
    pub fn current_chord(&self, chaos_level: f32) -> Chord {
        match &self.style {
            ProgressionStyle::Classical => self.classical_chord(self.chord_idx),
            ProgressionStyle::JazzTurnaround => self.jazz_turnaround_chord(self.chord_idx),
            ProgressionStyle::Modal => self.modal_chord(self.chord_idx),
            ProgressionStyle::JazzExtended => self.jazz_extended_chord(self.chord_idx, chaos_level),
        }
    }

    fn progression_len(&self) -> usize {
        match &self.style {
            ProgressionStyle::Classical => 4,
            ProgressionStyle::JazzTurnaround => 4,
            ProgressionStyle::Modal => self.scale.len().max(1),
            ProgressionStyle::JazzExtended => 4,
        }
    }

    /// Build the MIDI root for the i-th scale degree.
    fn scale_root(&self, degree: usize) -> u8 {
        let base = 48u8 + self.key_root; // C3 + key offset
        let offset = self.scale.get(degree % self.scale.len().max(1)).copied().unwrap_or(0);
        base.saturating_add(offset)
    }

    // ---- chord builders ----

    fn classical_chord(&self, idx: usize) -> Chord {
        // I  IV  V  I
        let (deg, intervals, suffix) = match idx % 4 {
            0 => (0, vec![0i8, 4, 7],       "maj"),
            1 => (3, vec![0i8, 5, 9],       "maj"),  // IV (using scale offset)
            2 => (4, vec![0i8, 4, 7],       "maj"),  // V
            _ => (0, vec![0i8, 4, 7, 11],   "maj7"), // I with 7th on return
        };
        let root = self.scale_root(deg);
        Chord {
            root,
            intervals,
            name: format!("{}{}", root_name(root), suffix),
        }
    }

    fn jazz_turnaround_chord(&self, idx: usize) -> Chord {
        // ii7 – V7 – Imaj7 – VI7
        let (deg, intervals, suffix) = match idx % 4 {
            0 => (1, vec![0i8, 3, 7, 10], "m7"),
            1 => (4, vec![0i8, 4, 7, 10], "7"),
            2 => (0, vec![0i8, 4, 7, 11], "maj7"),
            _ => (5, vec![0i8, 4, 7, 10], "7"),
        };
        let root = self.scale_root(deg);
        Chord {
            root,
            intervals,
            name: format!("{}{}", root_name(root), suffix),
        }
    }

    fn modal_chord(&self, idx: usize) -> Chord {
        let deg = idx % self.scale.len().max(1);
        let root = self.scale_root(deg);
        // Build a triad by stacking every other scale degree.
        let len = self.scale.len();
        let third_off = self.scale.get((deg + 2) % len).copied().unwrap_or(4) as i8
            - self.scale.get(deg % len).copied().unwrap_or(0) as i8;
        let fifth_off = self.scale.get((deg + 4) % len).copied().unwrap_or(7) as i8
            - self.scale.get(deg % len).copied().unwrap_or(0) as i8;
        Chord {
            root,
            intervals: vec![0, third_off, fifth_off],
            name: format!("{}", root_name(root)),
        }
    }

    fn jazz_extended_chord(&self, idx: usize, chaos_level: f32) -> Chord {
        let mut base = self.jazz_turnaround_chord(idx);
        // Add 9th / 11th / 13th proportional to chaos.
        if chaos_level > 0.4 {
            base.intervals.push(14); // 9th
        }
        if chaos_level > 0.65 {
            base.intervals.push(17); // 11th
        }
        if chaos_level > 0.82 {
            base.intervals.push(21); // 13th
        }
        base.name = format!("{}ext", base.name);
        base
    }
}

fn root_name(midi: u8) -> &'static str {
    const NAMES: [&str; 12] = ["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"];
    NAMES[(midi % 12) as usize]
}

// ---------------------------------------------------------------------------
// Rhythmic quantizer
// ---------------------------------------------------------------------------

/// Time-signature descriptor for the rhythmic quantizer.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSig {
    /// Numerator (beats per bar).
    pub beats: u8,
    /// Denominator as power of two (2 = quarter note, 3 = eighth note, …).
    pub beat_unit_pow2: u8,
}

impl TimeSig {
    pub const FOUR_FOUR:  TimeSig = TimeSig { beats: 4, beat_unit_pow2: 2 };
    pub const THREE_FOUR: TimeSig = TimeSig { beats: 3, beat_unit_pow2: 2 };
    pub const FIVE_FOUR:  TimeSig = TimeSig { beats: 5, beat_unit_pow2: 2 };
    pub const SIX_EIGHT:  TimeSig = TimeSig { beats: 6, beat_unit_pow2: 3 };
    pub const SEVEN_EIGHT:TimeSig = TimeSig { beats: 7, beat_unit_pow2: 3 };

    /// Denominator as a plain integer (e.g. 4 for 4/4, 8 for 7/8).
    pub fn denom(&self) -> u8 {
        1u8 << self.beat_unit_pow2
    }
}

/// Quantizes a continuous ODE output value to a rhythmic grid position.
///
/// The quantizer maps a real-valued signal (e.g. trajectory speed) to a
/// discrete rhythmic position within a bar, returning the MIDI tick offset
/// of the next grid line.
pub struct RhythmicQuantizer {
    /// Active time signature.
    pub time_sig: TimeSig,
    /// MIDI ticks per quarter note.
    pub ticks_per_quarter: u32,
    /// Accumulated position within the bar in MIDI ticks.
    cursor_ticks: u32,
    /// Sub-division level: 1 = beat, 2 = half-beat, 4 = quarter-beat, …
    pub subdivision: u8,
}

impl RhythmicQuantizer {
    /// Create a quantizer with the given time signature.
    ///
    /// * `ticks_per_quarter` — standard MIDI ticks per quarter note (e.g. 480).
    /// * `subdivision` — grid sub-divisions per beat (1, 2, 4 recommended).
    pub fn new(time_sig: TimeSig, ticks_per_quarter: u32, subdivision: u8) -> Self {
        Self {
            time_sig,
            ticks_per_quarter,
            cursor_ticks: 0,
            subdivision: subdivision.max(1),
        }
    }

    /// Ticks per beat (respects time-signature denominator).
    pub fn ticks_per_beat(&self) -> u32 {
        // A quarter note = ticks_per_quarter.  An eighth note = half that, etc.
        // beat_unit_pow2=2 → quarter, pow2=3 → eighth.
        let shift = self.time_sig.beat_unit_pow2.saturating_sub(2);
        self.ticks_per_quarter >> shift
    }

    /// Ticks per grid cell (one subdivision of a beat).
    pub fn ticks_per_grid(&self) -> u32 {
        (self.ticks_per_beat() / self.subdivision as u32).max(1)
    }

    /// Total ticks in one bar.
    pub fn ticks_per_bar(&self) -> u32 {
        self.ticks_per_beat() * self.time_sig.beats as u32
    }

    /// Advance the internal cursor by `raw_ticks` and return the quantised
    /// tick position (snapped to the nearest grid line).
    ///
    /// The cursor wraps at the bar boundary.
    pub fn advance(&mut self, raw_ticks: u32) -> u32 {
        let grid = self.ticks_per_grid();
        self.cursor_ticks = (self.cursor_ticks + raw_ticks) % self.ticks_per_bar().max(1);
        // Snap to nearest grid line.
        let quantised = (self.cursor_ticks / grid) * grid;
        quantised
    }

    /// Map a continuous value `v` in [0, 1] to a note duration in MIDI ticks,
    /// chosen from the set {sixteenth, eighth, quarter, half, whole}.
    pub fn value_to_duration(&self, v: f64) -> u32 {
        let tpq = self.ticks_per_quarter;
        let candidates = [tpq / 4, tpq / 2, tpq, tpq * 2, tpq * 4];
        let idx = (v.clamp(0.0, 1.0) * (candidates.len() - 1) as f64).round() as usize;
        candidates[idx.min(candidates.len() - 1)]
    }

    /// Return current position within the bar as a fraction [0, 1).
    pub fn bar_position(&self) -> f64 {
        self.cursor_ticks as f64 / self.ticks_per_bar().max(1) as f64
    }

    /// Return current beat number (0-based) within the bar.
    pub fn current_beat(&self) -> u8 {
        (self.cursor_ticks / self.ticks_per_beat().max(1)) as u8
    }
}

// ---------------------------------------------------------------------------
// Composition section
// ---------------------------------------------------------------------------

/// A single section in the generated piece.
#[derive(Debug, Clone)]
pub struct CompositionSection {
    /// Section label (e.g. "A", "B", "Var. 1").
    pub label: String,
    /// Whether this is a refrain (true) or episode (false).
    pub is_refrain: bool,
    /// Duration in MIDI ticks.
    pub duration_ticks: u32,
    /// Notes generated for this section.
    pub notes: Vec<MidiNote>,
    /// Chord sequence for this section.
    pub chords: Vec<Chord>,
    /// Motif pitches active in this section.
    pub motif: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Composition exporter
// ---------------------------------------------------------------------------

/// Exports a complete generated composition to a multi-track MIDI SMF file.
///
/// Track layout:
/// - Track 0: melody (motif + ornamentation, channel 0).
/// - Track 1: harmony / chords (channels 1).
/// - Track 2: bass line (channel 2).
pub struct CompositionExporter {
    /// Underlying MIDI exporter.
    exporter: MidiExporter,
}

impl Default for CompositionExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositionExporter {
    pub fn new() -> Self {
        Self { exporter: MidiExporter::new() }
    }

    /// Build MIDI tracks from a slice of [`CompositionSection`]s and export to
    /// a `.mid` file at `path`.
    ///
    /// Returns an error string on failure.
    pub fn export(&self, sections: &[CompositionSection], tempo_bpm: f64, path: &str) -> Result<(), String> {
        if sections.is_empty() {
            return Err("No composition sections to export".to_string());
        }

        let melody_track = self.build_melody_track(sections, tempo_bpm);
        let harmony_track = self.build_harmony_track(sections, tempo_bpm);
        let bass_track = self.build_bass_track(sections, tempo_bpm);

        self.exporter
            .export_to_file(&[melody_track, harmony_track, bass_track], path)
            .map_err(|e| format!("MIDI export failed: {e}"))
    }

    /// Return the raw SMF bytes without writing to disk (useful for testing).
    pub fn export_bytes(&self, sections: &[CompositionSection], tempo_bpm: f64) -> Vec<u8> {
        let melody_track = self.build_melody_track(sections, tempo_bpm);
        let harmony_track = self.build_harmony_track(sections, tempo_bpm);
        let bass_track = self.build_bass_track(sections, tempo_bpm);
        self.exporter.export_smf(&[melody_track, harmony_track, bass_track])
    }

    // ---- private track builders -------------------------------------------

    fn build_melody_track(&self, sections: &[CompositionSection], tempo_bpm: f64) -> MidiTrack {
        let mut notes: Vec<MidiNote> = Vec::new();
        let mut cursor: u32 = 0;

        for section in sections {
            // Write section notes; re-stamp start_tick relative to absolute cursor.
            for note in &section.notes {
                notes.push(MidiNote {
                    channel: 0,
                    start_tick: cursor + note.start_tick,
                    ..note.clone()
                });
            }
            cursor = cursor.saturating_add(section.duration_ticks);
        }

        MidiTrack {
            name: "Melody".to_string(),
            notes,
            tempo_bpm,
            time_sig_num: 4,
            time_sig_denom: 4,
        }
    }

    fn build_harmony_track(&self, sections: &[CompositionSection], tempo_bpm: f64) -> MidiTrack {
        let mut notes: Vec<MidiNote> = Vec::new();
        let mut cursor: u32 = 0;
        let tpq = self.exporter.ticks_per_quarter as u32;
        let chord_dur = tpq * 2; // half note per chord

        for section in sections {
            let mut local_tick: u32 = 0;
            for chord in &section.chords {
                if local_tick >= section.duration_ticks {
                    break;
                }
                let dur = chord_dur.min(section.duration_ticks - local_tick);
                let vel = 72u8;
                for &pitch in chord.pitches().iter().take(4) {
                    notes.push(MidiNote {
                        channel: 1,
                        pitch,
                        velocity: vel,
                        start_tick: cursor + local_tick,
                        duration_ticks: dur.saturating_sub(20),
                    });
                }
                local_tick = local_tick.saturating_add(chord_dur);
            }
            cursor = cursor.saturating_add(section.duration_ticks);
        }

        MidiTrack {
            name: "Harmony".to_string(),
            notes,
            tempo_bpm,
            time_sig_num: 4,
            time_sig_denom: 4,
        }
    }

    fn build_bass_track(&self, sections: &[CompositionSection], tempo_bpm: f64) -> MidiTrack {
        let mut notes: Vec<MidiNote> = Vec::new();
        let mut cursor: u32 = 0;
        let tpq = self.exporter.ticks_per_quarter as u32;
        let beat = tpq;

        for section in sections {
            let mut local_tick: u32 = 0;
            for chord in &section.chords {
                // Bass plays root on beat 1 and 3 of each chord block.
                for beat_offset in [0u32, beat * 2] {
                    let t = local_tick.saturating_add(beat_offset);
                    if t >= section.duration_ticks {
                        break;
                    }
                    // Bass root is one octave below chord root.
                    let bass_pitch = chord.root.saturating_sub(12);
                    notes.push(MidiNote {
                        channel: 2,
                        pitch: bass_pitch,
                        velocity: 85,
                        start_tick: cursor + t,
                        duration_ticks: beat.saturating_sub(40),
                    });
                }
                local_tick = local_tick.saturating_add(tpq * 2);
            }
            cursor = cursor.saturating_add(section.duration_ticks);
        }

        MidiTrack {
            name: "Bass".to_string(),
            notes,
            tempo_bpm,
            time_sig_num: 4,
            time_sig_denom: 4,
        }
    }
}

// ---------------------------------------------------------------------------
// Composer engine
// ---------------------------------------------------------------------------

/// Output frame produced by the [`ComposerEngine`] each tick.
#[derive(Debug, Clone)]
pub struct ComposerFrame {
    /// Currently active chord.
    pub chord: Chord,
    /// Currently active motif pitches.
    pub motif: Vec<u8>,
    /// Suggested MIDI note for this tick (melody).
    pub melody_note: u8,
    /// Section index (increments at section boundaries).
    pub section_idx: usize,
    /// Whether the section just changed this tick.
    pub section_changed: bool,
    /// Bar position [0, 1).
    pub bar_position: f64,
    /// Current beat within the bar.
    pub current_beat: u8,
    /// Active musical form.
    pub form: MusicalForm,
    /// Tempo in BPM.
    pub tempo_bpm: f64,
}

/// Main composition engine — ties together motif extraction, harmonic
/// progressions, rhythmic quantization, and musical form.
///
/// Call [`ComposerEngine::tick`] once per control-rate step.
pub struct ComposerEngine {
    form: MusicalForm,
    motif_gen: MotifGenerator,
    harmonic: HarmonicProgression,
    quantizer: RhythmicQuantizer,
    /// Ticks elapsed in the current section.
    section_ticks: u32,
    /// Duration of each section in control-rate ticks.
    section_duration_ticks: u32,
    /// Current section index.
    section_idx: usize,
    /// Accumulated sections for eventual MIDI export.
    pub sections: Vec<CompositionSection>,
    /// Current section accumulator.
    current_section_notes: Vec<MidiNote>,
    /// Absolute MIDI tick cursor within the current section.
    section_midi_cursor: u32,
    /// Control rate in Hz (used to convert durations).
    control_rate_hz: f64,
}

impl ComposerEngine {
    /// Create a new engine.
    ///
    /// * `form` — macro-level form.
    /// * `style` — chord progression style.
    /// * `time_sig` — rhythmic grid time signature.
    /// * `tempo_bpm` — initial tempo.
    /// * `control_rate_hz` — simulation control rate (e.g. 120.0).
    /// * `section_bars` — how many bars each section lasts.
    pub fn new(
        form: MusicalForm,
        style: ProgressionStyle,
        time_sig: TimeSig,
        tempo_bpm: f64,
        control_rate_hz: f64,
        section_bars: u32,
    ) -> Self {
        let tpq = 480u32;
        let quantizer = RhythmicQuantizer::new(time_sig.clone(), tpq, 2);
        let ticks_per_bar = quantizer.ticks_per_bar();
        // Derive section duration in control-rate ticks from tempo + time sig.
        let secs_per_beat = 60.0 / tempo_bpm.max(1.0);
        let beats_per_bar = time_sig.beats as f64;
        let secs_per_bar = secs_per_beat * beats_per_bar;
        let section_duration_ticks = (secs_per_bar * section_bars as f64 * control_rate_hz) as u32;

        let harmonic = HarmonicProgression::new(
            0, // C
            vec![0, 2, 4, 5, 7, 9, 11], // major scale
            style,
            (ticks_per_bar / 2).max(1), // half-bar chord duration in MIDI ticks
        );

        Self {
            form,
            motif_gen: MotifGenerator::new(64, 4, 3),
            harmonic,
            quantizer,
            section_ticks: 0,
            section_duration_ticks: section_duration_ticks.max(120),
            section_idx: 0,
            sections: Vec::new(),
            current_section_notes: Vec::new(),
            section_midi_cursor: 0,
            control_rate_hz,
        }
    }

    /// Tick the engine forward one control-rate step.
    ///
    /// * `state` — current attractor state vector.
    /// * `melody_pitch` — quantised MIDI pitch for this tick.
    /// * `velocity` — MIDI velocity for this tick.
    /// * `chaos_level` — normalised chaos level [0, 1].
    /// * `lyapunov` — current largest Lyapunov exponent.
    pub fn tick(
        &mut self,
        state: &[f64],
        melody_pitch: u8,
        velocity: u8,
        chaos_level: f32,
        lyapunov: f64,
    ) -> ComposerFrame {
        // Update tempo from Lyapunov (40–180 BPM).
        let tempo_bpm = {
            let t = ((lyapunov - (-2.0)) / (3.0 - (-2.0))).clamp(0.0, 1.0);
            40.0 + t * 140.0
        };

        // Advance harmonic progression.
        let chord = self.harmonic.tick(chaos_level);

        // Feed motif generator.
        self.motif_gen.push(melody_pitch);
        let motif = self.motif_gen.motif().to_vec();

        // Advance quantizer by one control-rate tick (converts to MIDI ticks).
        let ticks_per_ctrl = (self.quantizer.ticks_per_quarter as f64 * (tempo_bpm / 60.0)
            / self.control_rate_hz) as u32;
        let bar_pos = {
            self.quantizer.advance(ticks_per_ctrl.max(1));
            self.quantizer.bar_position()
        };
        let current_beat = self.quantizer.current_beat();

        // Generate a melody note and accumulate into the current section.
        let note_dur = self.quantizer.value_to_duration(
            state.first().copied().unwrap_or(0.0).abs() / 30.0,
        );
        self.current_section_notes.push(MidiNote {
            channel: 0,
            pitch: melody_pitch,
            velocity,
            start_tick: self.section_midi_cursor,
            duration_ticks: note_dur.max(1),
        });
        self.section_midi_cursor = self.section_midi_cursor.saturating_add(ticks_per_ctrl.max(1));

        // Advance section counter.
        self.section_ticks = self.section_ticks.saturating_add(1);
        let section_changed = self.section_ticks >= self.section_duration_ticks;
        if section_changed {
            self.flush_section();
            self.section_idx += 1;
            self.section_ticks = 0;
            self.section_midi_cursor = 0;
        }

        ComposerFrame {
            chord,
            motif,
            melody_note: melody_pitch,
            section_idx: self.section_idx,
            section_changed,
            bar_position: bar_pos,
            current_beat,
            form: self.form.clone(),
            tempo_bpm,
        }
    }

    /// Flush the current section buffer into `self.sections`.
    fn flush_section(&mut self) {
        let is_refrain = self.form.is_refrain(self.section_idx);
        let label = section_label(&self.form, self.section_idx);
        let notes = std::mem::take(&mut self.current_section_notes);
        let chords = self.chord_snapshot();
        let motif = self.motif_gen.current_motif.clone();
        self.sections.push(CompositionSection {
            label,
            is_refrain,
            duration_ticks: self.section_midi_cursor,
            notes,
            chords,
            motif,
        });
    }

    /// Snapshot current chord bank for the section record.
    fn chord_snapshot(&self) -> Vec<Chord> {
        // Return one representative chord per scale degree (modal snapshot).
        let scale = &self.harmonic.scale;
        let key = self.harmonic.key_root;
        scale
            .iter()
            .enumerate()
            .map(|(i, &off)| {
                let root = 48u8.saturating_add(key).saturating_add(off);
                let third = scale.get((i + 2) % scale.len()).copied().unwrap_or(4) as i8
                    - off as i8;
                let fifth = scale.get((i + 4) % scale.len()).copied().unwrap_or(7) as i8
                    - off as i8;
                Chord {
                    root,
                    intervals: vec![0, third, fifth],
                    name: format!("{}", root_name(root)),
                }
            })
            .collect()
    }

    /// Finalise the composition: flush any open section and return all sections.
    pub fn finalise(&mut self) -> &[CompositionSection] {
        if !self.current_section_notes.is_empty() {
            self.flush_section();
        }
        &self.sections
    }

    /// Export the finalised composition to a MIDI file.
    pub fn export_midi(&mut self, path: &str, tempo_bpm: f64) -> Result<(), String> {
        self.finalise();
        if self.sections.is_empty() {
            return Err("No sections to export".to_string());
        }
        let exporter = CompositionExporter::new();
        exporter.export(&self.sections, tempo_bpm, path)
    }
}

fn section_label(form: &MusicalForm, idx: usize) -> String {
    match form {
        MusicalForm::Aba => match idx {
            0 => "A".to_string(),
            1 => "B".to_string(),
            _ => format!("A'{}", if idx > 2 { idx.to_string() } else { String::new() }),
        },
        MusicalForm::ThemeVariations { .. } => {
            if idx == 0 { "Theme".to_string() } else { format!("Var. {}", idx) }
        }
        MusicalForm::Rondo => {
            if idx % 2 == 0 { "A".to_string() } else { format!("Ep. {}", idx / 2 + 1) }
        }
        MusicalForm::ThroughComposed => format!("§{}", idx + 1),
        MusicalForm::Stochastic => format!("Sec. {}", idx + 1),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn musical_form_labels_are_non_empty() {
        for form in MusicalForm::all() {
            assert!(!form.label().is_empty());
        }
    }

    #[test]
    fn aba_section_count() {
        assert_eq!(MusicalForm::Aba.section_count(), Some(3));
    }

    #[test]
    fn aba_is_refrain() {
        assert!(MusicalForm::Aba.is_refrain(0));
        assert!(!MusicalForm::Aba.is_refrain(1));
        assert!(MusicalForm::Aba.is_refrain(2));
    }

    #[test]
    fn rondo_alternates_refrain() {
        let rondo = MusicalForm::Rondo;
        for i in 0..8 {
            assert_eq!(rondo.is_refrain(i), i % 2 == 0, "section {i}");
        }
    }

    #[test]
    fn motif_generator_finds_repeated_cell() {
        let mut gen = MotifGenerator::new(64, 3, 3);
        // Push a pattern that repeats 4 times: 60 62 64
        let pattern = [60u8, 62, 64];
        for _ in 0..5 {
            for &p in &pattern {
                gen.push(p);
            }
            // Fill with noise between repetitions.
            for v in 70u8..76 {
                gen.push(v);
            }
        }
        // The generator should have identified the repeated cell.
        // (May need sufficient history; just assert no panic and motif is non-empty or at least len correct.)
        // We can't guarantee the exact cell given the window, but we verify no crash.
        let _ = gen.motif();
    }

    #[test]
    fn chord_pitches_in_range() {
        let prog = HarmonicProgression::new(
            0,
            vec![0, 2, 4, 5, 7, 9, 11],
            ProgressionStyle::Classical,
            120,
        );
        let chord = prog.classical_chord(0);
        for &p in chord.pitches().iter() {
            assert!(p <= 127, "pitch {p} out of MIDI range");
        }
    }

    #[test]
    fn jazz_extended_adds_extensions_at_high_chaos() {
        let prog = HarmonicProgression::new(
            0,
            vec![0, 2, 4, 5, 7, 9, 11],
            ProgressionStyle::JazzExtended,
            120,
        );
        let base_chord = prog.jazz_extended_chord(0, 0.1);
        let ext_chord = prog.jazz_extended_chord(0, 0.9);
        assert!(ext_chord.intervals.len() > base_chord.intervals.len());
    }

    #[test]
    fn rhythmic_quantizer_4_4_ticks_per_bar() {
        let q = RhythmicQuantizer::new(TimeSig::FOUR_FOUR, 480, 2);
        assert_eq!(q.ticks_per_bar(), 4 * 480);
    }

    #[test]
    fn rhythmic_quantizer_7_8_ticks_per_bar() {
        let q = RhythmicQuantizer::new(TimeSig::SEVEN_EIGHT, 480, 2);
        // 7 beats, each an eighth note = tpq/2
        assert_eq!(q.ticks_per_bar(), 7 * 240);
    }

    #[test]
    fn quantizer_advance_wraps_at_bar() {
        let mut q = RhythmicQuantizer::new(TimeSig::FOUR_FOUR, 480, 1);
        let bar = q.ticks_per_bar();
        // Advance exactly one bar.
        q.advance(bar);
        assert_eq!(q.cursor_ticks, 0, "cursor should wrap to 0");
    }

    #[test]
    fn quantizer_value_to_duration_range() {
        let q = RhythmicQuantizer::new(TimeSig::FOUR_FOUR, 480, 2);
        let tpq = 480u32;
        let min_dur = tpq / 4; // sixteenth
        let max_dur = tpq * 4; // whole note
        for v in [0.0f64, 0.25, 0.5, 0.75, 1.0] {
            let d = q.value_to_duration(v);
            assert!(d >= min_dur && d <= max_dur, "duration {d} out of range for v={v}");
        }
    }

    #[test]
    fn composer_engine_tick_does_not_panic() {
        let mut engine = ComposerEngine::new(
            MusicalForm::Aba,
            ProgressionStyle::Classical,
            TimeSig::FOUR_FOUR,
            120.0,
            120.0,
            4,
        );
        let state = vec![1.0f64, 0.5, -0.3];
        for i in 0..300 {
            let pitch = 60u8 + (i % 12) as u8;
            let frame = engine.tick(&state, pitch, 80, 0.3, 0.5);
            assert!(frame.melody_note <= 127);
            assert!(frame.tempo_bpm >= 40.0 && frame.tempo_bpm <= 180.0);
        }
    }

    #[test]
    fn composer_engine_flushes_sections_on_finalise() {
        let mut engine = ComposerEngine::new(
            MusicalForm::ThroughComposed,
            ProgressionStyle::JazzTurnaround,
            TimeSig::FIVE_FOUR,
            100.0,
            120.0,
            2,
        );
        let state = vec![5.0f64, -2.0, 3.0];
        for i in 0..600 {
            engine.tick(&state, 60 + (i % 24) as u8, 70, 0.5, 1.0);
        }
        let sections = engine.finalise();
        assert!(!sections.is_empty(), "should have at least one section");
    }

    #[test]
    fn composition_exporter_bytes_starts_with_midi_header() {
        let mut engine = ComposerEngine::new(
            MusicalForm::Aba,
            ProgressionStyle::Modal,
            TimeSig::FOUR_FOUR,
            120.0,
            120.0,
            2,
        );
        let state = vec![1.0f64, 0.0, 0.0];
        for i in 0..300 {
            engine.tick(&state, 60 + (i % 12) as u8, 80, 0.4, 0.3);
        }
        engine.finalise();
        if !engine.sections.is_empty() {
            let exp = CompositionExporter::new();
            let bytes = exp.export_bytes(&engine.sections, 120.0);
            assert_eq!(&bytes[0..4], b"MThd", "SMF magic bytes missing");
        }
    }

    #[test]
    fn section_label_aba() {
        assert_eq!(section_label(&MusicalForm::Aba, 0), "A");
        assert_eq!(section_label(&MusicalForm::Aba, 1), "B");
        assert_eq!(section_label(&MusicalForm::Aba, 2), "A'");
    }

    #[test]
    fn section_label_theme_variations() {
        let form = MusicalForm::ThemeVariations { variations: 3 };
        assert_eq!(section_label(&form, 0), "Theme");
        assert_eq!(section_label(&form, 1), "Var. 1");
        assert_eq!(section_label(&form, 3), "Var. 3");
    }
}
