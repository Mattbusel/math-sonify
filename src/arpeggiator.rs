//! Arpeggiator with multiple pattern modes.
//!
//! Generates ordered sequences of MIDI notes from a chord according to
//! configurable melodic patterns, tempo, and optional LFO velocity modulation.

use std::f64::consts::PI;

// ── Chord ─────────────────────────────────────────────────────────────────────

/// A chord described by a root MIDI pitch and a set of semitone intervals.
#[derive(Debug, Clone)]
pub struct Chord {
    /// MIDI pitch of the root note (0–127).
    pub root_midi: u8,
    /// Semitone offsets from root.
    pub intervals: Vec<i8>,
}

impl Chord {
    /// Compute the MIDI pitches of all chord notes, clamped to `0..=127`.
    pub fn notes(&self) -> Vec<u8> {
        self.intervals
            .iter()
            .map(|&iv| {
                let pitch = self.root_midi as i16 + iv as i16;
                pitch.clamp(0, 127) as u8
            })
            .collect()
    }

    /// Major triad (root, maj3, P5).
    pub fn major(root_midi: u8) -> Self {
        Chord { root_midi, intervals: vec![0, 4, 7] }
    }

    /// Minor triad (root, min3, P5).
    pub fn minor(root_midi: u8) -> Self {
        Chord { root_midi, intervals: vec![0, 3, 7] }
    }

    /// Dominant 7th (root, maj3, P5, min7).
    pub fn dominant7(root_midi: u8) -> Self {
        Chord { root_midi, intervals: vec![0, 4, 7, 10] }
    }

    /// Diminished triad (root, min3, dim5).
    pub fn diminished(root_midi: u8) -> Self {
        Chord { root_midi, intervals: vec![0, 3, 6] }
    }

    /// Augmented triad (root, maj3, aug5).
    pub fn augmented(root_midi: u8) -> Self {
        Chord { root_midi, intervals: vec![0, 4, 8] }
    }
}

// ── ArpPattern ────────────────────────────────────────────────────────────────

/// Pattern used to order notes in the arpeggio sequence.
#[derive(Debug, Clone)]
pub enum ArpPattern {
    /// Ascending through all octaves.
    Up,
    /// Descending through all octaves.
    Down,
    /// Ascend then descend (endpoints not repeated).
    UpDown,
    /// Descend then ascend (endpoints not repeated).
    DownUp,
    /// Deterministic random with the given LCG seed.
    Random(u64),
    /// Alternate outermost unused notes: highest, lowest, 2nd-highest, 2nd-lowest…
    OutsideIn,
    /// Start from the middle note, alternate outward.
    InsideOut,
    /// Cycle through chord notes × octave_range ascending in chord order.
    OrderedUp { octaves: u8 },
    /// Bass note then ascending chord.
    Thumb { bass_note: u8 },
}

// ── ArpeggioNote ─────────────────────────────────────────────────────────────

/// A single note event produced by the arpeggiator.
#[derive(Debug, Clone, PartialEq)]
pub struct ArpeggioNote {
    /// MIDI pitch (0–127).
    pub midi_pitch: u8,
    /// MIDI velocity (0–127).
    pub velocity: u8,
    /// Note duration in milliseconds.
    pub duration_ms: u32,
    /// Zero-based step index within the sequence.
    pub step: usize,
}

// ── Arpeggiator ───────────────────────────────────────────────────────────────

/// Generates arpeggio note sequences from a chord.
pub struct Arpeggiator {
    /// Source chord.
    pub chord: Chord,
    /// Melodic pattern.
    pub pattern: ArpPattern,
    /// Tempo in beats per minute.
    pub bpm: f64,
    /// Duration of each note in milliseconds.
    pub note_duration_ms: u32,
    /// Number of octaves to span.
    pub octave_range: u8,
    /// Base MIDI velocity.
    pub velocity: u8,
    /// LFO depth (0 = no modulation, 1 = full ±64 swing).
    pub lfo_depth: f64,
}

impl Arpeggiator {
    /// Milliseconds per step (quarter note at current BPM).
    pub fn step_interval_ms(&self) -> u64 {
        (60_000.0 / self.bpm).round() as u64
    }

    /// Build the full expanded note list across all octaves, sorted ascending.
    fn expanded_notes(&self) -> Vec<u8> {
        let base = self.chord.notes();
        let mut notes: Vec<u8> = Vec::new();
        for oct in 0..self.octave_range as i16 {
            for &n in &base {
                let pitched = (n as i16 + oct * 12).clamp(0, 127) as u8;
                notes.push(pitched);
            }
        }
        notes.sort_unstable();
        notes.dedup();
        notes
    }

    /// Generate `steps` arpeggio notes following the configured pattern.
    pub fn generate_sequence(&self, steps: usize) -> Vec<ArpeggioNote> {
        let notes = self.expanded_notes();
        if notes.is_empty() || steps == 0 {
            return Vec::new();
        }

        let pattern_cycle = self.build_cycle(&notes);
        let cycle_len = pattern_cycle.len();

        (0..steps)
            .map(|i| {
                let midi_pitch = if cycle_len > 0 {
                    pattern_cycle[i % cycle_len]
                } else {
                    notes[0]
                };
                ArpeggioNote {
                    midi_pitch,
                    velocity: self.velocity,
                    duration_ms: self.note_duration_ms,
                    step: i,
                }
            })
            .collect()
    }

    /// Build one complete cycle of pitches for the pattern.
    fn build_cycle(&self, notes: &[u8]) -> Vec<u8> {
        match &self.pattern {
            ArpPattern::Up => notes.to_vec(),
            ArpPattern::Down => {
                let mut v = notes.to_vec();
                v.reverse();
                v
            }
            ArpPattern::UpDown => {
                if notes.len() <= 1 {
                    return notes.to_vec();
                }
                let mut v = notes.to_vec();
                // Descend without repeating endpoints.
                let inner: Vec<u8> = notes[1..notes.len() - 1].iter().cloned().rev().collect();
                v.extend(inner);
                v
            }
            ArpPattern::DownUp => {
                if notes.len() <= 1 {
                    return notes.to_vec();
                }
                let mut v: Vec<u8> = notes.iter().cloned().rev().collect();
                let inner: Vec<u8> = notes[1..notes.len() - 1].to_vec();
                v.extend(inner);
                v
            }
            ArpPattern::Random(seed) => {
                let mut pool = notes.to_vec();
                lcg_shuffle(&mut pool, *seed);
                pool
            }
            ArpPattern::OutsideIn => {
                let mut v = Vec::new();
                let mut sorted = notes.to_vec();
                sorted.sort_unstable();
                let mut lo = 0usize;
                let mut hi = sorted.len().saturating_sub(1);
                let mut turn = 0usize;
                while lo <= hi {
                    if lo == hi {
                        v.push(sorted[lo]);
                        break;
                    }
                    if turn % 2 == 0 {
                        v.push(sorted[hi]);
                        if hi == 0 { break; }
                        hi -= 1;
                    } else {
                        v.push(sorted[lo]);
                        lo += 1;
                    }
                    turn += 1;
                }
                v
            }
            ArpPattern::InsideOut => {
                let mut sorted = notes.to_vec();
                sorted.sort_unstable();
                let len = sorted.len();
                let mut v = Vec::new();
                let mid = len / 2;
                v.push(sorted[mid]);
                let mut lo = if mid > 0 { mid - 1 } else { 0 };
                let mut hi = mid + 1;
                let mut lo_active = mid > 0;
                loop {
                    let mut pushed = false;
                    if hi < len {
                        v.push(sorted[hi]);
                        hi += 1;
                        pushed = true;
                    }
                    if lo_active {
                        v.push(sorted[lo]);
                        if lo == 0 {
                            lo_active = false;
                        } else {
                            lo -= 1;
                        }
                        pushed = true;
                    }
                    if !pushed {
                        break;
                    }
                }
                v
            }
            ArpPattern::OrderedUp { octaves } => {
                let base = self.chord.notes();
                let mut v = Vec::new();
                for oct in 0..*octaves as i16 {
                    for &n in &base {
                        let pitched = (n as i16 + oct * 12).clamp(0, 127) as u8;
                        v.push(pitched);
                    }
                }
                v
            }
            ArpPattern::Thumb { bass_note } => {
                let mut v = vec![*bass_note];
                v.extend_from_slice(notes);
                v
            }
        }
    }

    /// Modulate note velocities with a sine LFO.
    ///
    /// `v = base_velocity + lfo_depth * 64 * sin(2π * step * freq_hz / bpm * 60)`
    pub fn with_lfo_velocity(&self, notes: &mut Vec<ArpeggioNote>, freq_hz: f64) {
        for note in notes.iter_mut() {
            let t = note.step as f64 * freq_hz / self.bpm * 60.0;
            let delta = self.lfo_depth * 64.0 * (2.0 * PI * t).sin();
            let new_vel = (self.velocity as f64 + delta).clamp(0.0, 127.0).round() as u8;
            note.velocity = new_vel;
        }
    }
}

/// Deterministic Fisher-Yates shuffle using a simple LCG.
fn lcg_shuffle(v: &mut Vec<u8>, seed: u64) {
    let mut rng = seed.wrapping_add(1);
    let n = v.len();
    for i in (1..n).rev() {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = (rng >> 33) as usize % (i + 1);
        v.swap(i, j);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_arp(pattern: ArpPattern) -> Arpeggiator {
        Arpeggiator {
            chord: Chord::major(60), // C4, E4, G4
            pattern,
            bpm: 120.0,
            note_duration_ms: 250,
            octave_range: 1,
            velocity: 80,
            lfo_depth: 0.0,
        }
    }

    #[test]
    fn up_pattern_is_ascending() {
        let arp = make_arp(ArpPattern::Up);
        let notes = arp.generate_sequence(3);
        assert_eq!(notes.len(), 3);
        assert!(notes[0].midi_pitch <= notes[1].midi_pitch);
        assert!(notes[1].midi_pitch <= notes[2].midi_pitch);
    }

    #[test]
    fn updown_has_correct_length() {
        let arp = make_arp(ArpPattern::UpDown);
        let notes = arp.generate_sequence(4); // one full cycle = 4 steps (3 up + 1 inner back)
        assert_eq!(notes.len(), 4);
    }

    #[test]
    fn outside_in_alternates_ends() {
        let arp = make_arp(ArpPattern::OutsideIn);
        let notes = arp.generate_sequence(3);
        // First note should be the highest (67 = G4), second lowest (60 = C4).
        assert_eq!(notes[0].midi_pitch, 67, "first should be highest");
        assert_eq!(notes[1].midi_pitch, 60, "second should be lowest");
    }

    #[test]
    fn lfo_modulates_velocity() {
        let arp = Arpeggiator {
            chord: Chord::major(60),
            pattern: ArpPattern::Up,
            bpm: 120.0,
            note_duration_ms: 250,
            octave_range: 1,
            velocity: 80,
            lfo_depth: 1.0, // full modulation
        };
        let mut notes = arp.generate_sequence(8);
        // 1 Hz at 120 BPM samples the sine only at its zero crossings; use 0.3 Hz.
        arp.with_lfo_velocity(&mut notes, 0.3);
        // With lfo_depth=1.0 some velocities should differ from 80.
        let all_same = notes.iter().all(|n| n.velocity == 80);
        assert!(!all_same, "LFO should have modulated velocities away from base");
    }

    #[test]
    fn step_interval_ms_at_120_bpm() {
        let arp = make_arp(ArpPattern::Up);
        assert_eq!(arp.step_interval_ms(), 500);
    }

    #[test]
    fn chord_notes_major() {
        let c = Chord::major(60);
        assert_eq!(c.notes(), vec![60, 64, 67]);
    }
}
