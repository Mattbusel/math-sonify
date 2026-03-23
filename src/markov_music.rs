//! Markov-chain music generation.
//!
//! Trains n-gram transition models on MIDI note sequences and uses them to
//! generate new melodies, optionally constrained to a musical scale.
//! All randomness uses a simple LCG so the module has zero external dependencies.

use std::collections::HashMap;

// ── LCG helper ────────────────────────────────────────────────────────────────

/// Linear congruential generator — deterministic, allocation-free RNG.
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg { state: seed.wrapping_add(1) }
    }

    /// Next value in [0, 1).
    fn next_f64(&mut self) -> f64 {
        self.state = self.state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // Use upper 32 bits for the fraction
        let upper = (self.state >> 32) as f64;
        upper / (u32::MAX as f64 + 1.0)
    }

    /// Next signed integer jitter in [-range, range].
    fn next_jitter_i32(&mut self, range: i32) -> i32 {
        if range == 0 {
            return 0;
        }
        let r = self.next_f64();
        let span = (2 * range + 1) as f64;
        (r * span) as i32 - range
    }
}

// ── MarkovOrder ───────────────────────────────────────────────────────────────

/// N-gram history length for the Markov chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkovOrder {
    First,
    Second,
    Third,
}

impl MarkovOrder {
    fn n(&self) -> usize {
        match self {
            MarkovOrder::First => 1,
            MarkovOrder::Second => 2,
            MarkovOrder::Third => 3,
        }
    }
}

// ── NoteTransition ────────────────────────────────────────────────────────────

/// A single observed transition from a note history to a following note.
#[derive(Debug, Clone)]
pub struct NoteTransition {
    /// History of MIDI notes that precede `to`.
    pub from: Vec<u8>,
    /// The following MIDI note.
    pub to: u8,
    /// Number of times this transition was observed.
    pub count: u32,
}

// ── MarkovChain ───────────────────────────────────────────────────────────────

/// N-gram Markov chain for MIDI note sequences.
pub struct MarkovChain {
    pub order: MarkovOrder,
    /// transitions[history] = { next_note → count }
    pub transitions: HashMap<Vec<u8>, HashMap<u8, u32>>,
}

impl MarkovChain {
    /// Create an empty Markov chain with the given order.
    pub fn new(order: MarkovOrder) -> Self {
        MarkovChain {
            order,
            transitions: HashMap::new(),
        }
    }

    /// Train the chain on a sequence of MIDI notes.
    ///
    /// Accumulates transition counts; can be called multiple times.
    pub fn train(&mut self, notes: &[u8]) {
        let n = self.order.n();
        if notes.len() <= n {
            return;
        }
        for i in 0..notes.len() - n {
            let history: Vec<u8> = notes[i..i + n].to_vec();
            let next = notes[i + n];
            *self
                .transitions
                .entry(history)
                .or_default()
                .entry(next)
                .or_insert(0) += 1;
        }
    }

    /// Sample the next note from the transition distribution for `history`.
    ///
    /// Uses an LCG seeded with `rng_seed`.
    pub fn next_note(&self, history: &[u8], rng_seed: u64) -> Option<u8> {
        let n = self.order.n();
        let key: Vec<u8> = if history.len() >= n {
            history[history.len() - n..].to_vec()
        } else {
            return None;
        };

        let dist = self.transitions.get(&key)?;
        let total: u32 = dist.values().sum();
        if total == 0 {
            return None;
        }

        let mut lcg = Lcg::new(rng_seed);
        let threshold = (lcg.next_f64() * total as f64) as u32;
        let mut cumulative = 0u32;
        for (&note, &count) in dist {
            cumulative += count;
            if cumulative > threshold {
                return Some(note);
            }
        }
        // Fallback: return the last key
        dist.keys().last().copied()
    }

    /// Generate a sequence of `length` notes by extending `seed_notes`.
    pub fn generate(&self, seed_notes: &[u8], length: usize, rng_seed: u64) -> Vec<u8> {
        let mut result: Vec<u8> = seed_notes.to_vec();
        let mut lcg = Lcg::new(rng_seed);

        while result.len() < length {
            let seed = (lcg.next_f64() * u64::MAX as f64) as u64;
            match self.next_note(&result, seed) {
                Some(note) => result.push(note),
                None => break,
            }
        }
        result.truncate(length);
        result
    }

    /// Return the most likely next note for `history` (argmax of counts).
    pub fn most_likely_next(&self, history: &[u8]) -> Option<u8> {
        let n = self.order.n();
        let key: Vec<u8> = if history.len() >= n {
            history[history.len() - n..].to_vec()
        } else {
            return None;
        };

        let dist = self.transitions.get(&key)?;
        dist.iter()
            .max_by_key(|(_, &count)| count)
            .map(|(&note, _)| note)
    }

    /// Shannon entropy (bits) of the next-note distribution for `history`.
    pub fn transition_entropy(&self, history: &[u8]) -> f64 {
        let n = self.order.n();
        let key: Vec<u8> = if history.len() >= n {
            history[history.len() - n..].to_vec()
        } else {
            return 0.0;
        };

        let dist = match self.transitions.get(&key) {
            Some(d) => d,
            None => return 0.0,
        };

        let total: u32 = dist.values().sum();
        if total == 0 {
            return 0.0;
        }
        let total_f = total as f64;

        dist.values()
            .filter(|&&c| c > 0)
            .map(|&c| {
                let p = c as f64 / total_f;
                -p * p.log2()
            })
            .sum()
    }
}

// ── NoteSequence ──────────────────────────────────────────────────────────────

/// A sequence of MIDI notes with timing and velocity.
#[derive(Debug, Clone)]
pub struct NoteSequence {
    pub notes: Vec<u8>,
    pub durations: Vec<f64>,
    pub velocities: Vec<u8>,
}

impl NoteSequence {
    /// Assign uniform durations from a tempo and resolution.
    pub fn quantize(notes: Vec<u8>, tempo_bpm: f64, resolution_ms: f64) -> NoteSequence {
        let beat_ms = 60_000.0 / tempo_bpm.max(1.0);
        // Round duration to nearest resolution
        let raw_dur_ms = beat_ms;
        let steps = (raw_dur_ms / resolution_ms).round().max(1.0);
        let duration = steps * resolution_ms / 1000.0; // convert to seconds

        let n = notes.len();
        NoteSequence {
            notes,
            durations: vec![duration; n],
            velocities: vec![100u8; n],
        }
    }

    /// Add small random variations to timing and velocity using an LCG.
    pub fn humanize(&mut self, timing_jitter_ms: f64, velocity_variance: u8, rng_seed: u64) {
        let mut lcg = Lcg::new(rng_seed);
        let range_v = velocity_variance as i32;

        for i in 0..self.notes.len() {
            // Duration jitter
            let jitter_s = timing_jitter_ms / 1000.0;
            let r = lcg.next_f64();
            let sign = if r < 0.5 { -1.0 } else { 1.0 };
            let amount = (r * 2.0 - 1.0).abs() * jitter_s;
            self.durations[i] = (self.durations[i] + sign * amount).max(0.001);

            // Velocity variance
            let dv = lcg.next_jitter_i32(range_v);
            let new_vel = (self.velocities[i] as i32 + dv).clamp(1, 127) as u8;
            self.velocities[i] = new_vel;
        }
    }
}

// ── ScaleType ─────────────────────────────────────────────────────────────────

/// Musical scale type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleType {
    Major,
    Minor,
    Pentatonic,
    Blues,
    Chromatic,
}

impl ScaleType {
    /// Semitone intervals from root that define this scale.
    pub fn intervals(&self) -> Vec<u8> {
        match self {
            ScaleType::Major => vec![0, 2, 4, 5, 7, 9, 11],
            ScaleType::Minor => vec![0, 2, 3, 5, 7, 8, 10],
            ScaleType::Pentatonic => vec![0, 2, 4, 7, 9],
            ScaleType::Blues => vec![0, 3, 5, 6, 7, 10],
            ScaleType::Chromatic => (0u8..12).collect(),
        }
    }
}

// ── ScaleConstraint ───────────────────────────────────────────────────────────

/// Constrains generated notes to a specific root + scale.
#[derive(Debug, Clone)]
pub struct ScaleConstraint {
    /// MIDI note number for the scale root (e.g. 60 = middle C).
    pub root: u8,
    pub scale_type: ScaleType,
}

impl ScaleConstraint {
    /// Snap `note` to the nearest note in this scale.
    pub fn snap_to_scale(&self, note: u8) -> u8 {
        let intervals = self.scale_type.intervals();
        let root = self.root % 12;
        let note_class = note % 12;

        // Find the closest scale degree by semitone distance
        let best_interval = intervals.iter().min_by_key(|&&interval| {
            let scale_class = (root + interval) % 12;
            let dist = (note_class as i16 - scale_class as i16).unsigned_abs();
            dist.min(12u16.saturating_sub(dist))
        });

        match best_interval {
            Some(&interval) => {
                let target_class = (root + interval) % 12;
                // Preserve octave
                let octave_base = note - note_class;
                let candidate = octave_base + target_class;
                // Pick the octave closest to the original note
                if note_class > target_class && note_class - target_class > 6 {
                    candidate.saturating_add(12).min(127)
                } else if target_class > note_class && target_class - note_class > 6 {
                    candidate.saturating_sub(12)
                } else {
                    candidate.min(127)
                }
            }
            None => note,
        }
    }
}

// ── MarkovMelodyGenerator ─────────────────────────────────────────────────────

/// High-level melody generator combining Markov chains with scale constraints.
pub struct MarkovMelodyGenerator {
    pub chain: MarkovChain,
    pub scale: ScaleConstraint,
}

impl MarkovMelodyGenerator {
    /// Create a new generator with the given order and scale.
    pub fn new(order: MarkovOrder, scale: ScaleConstraint) -> Self {
        MarkovMelodyGenerator {
            chain: MarkovChain::new(order),
            scale,
        }
    }

    /// Generate a training melody by random walk on the scale, then train.
    pub fn train_from_scale(&mut self, root: u8, length: usize, rng_seed: u64) {
        let intervals = self.scale.scale_type.intervals();
        let mut lcg = Lcg::new(rng_seed);
        let mut melody: Vec<u8> = Vec::with_capacity(length);

        for _ in 0..length {
            let idx = (lcg.next_f64() * intervals.len() as f64) as usize;
            let interval = intervals[idx.min(intervals.len() - 1)];
            // Choose an octave offset: 0 or +12
            let octave = if lcg.next_f64() < 0.5 { 0u8 } else { 12u8 };
            let note = root.saturating_add(interval).saturating_add(octave).min(127);
            melody.push(note);
        }

        self.chain.train(&melody);
    }

    /// Generate a melody of `length` notes using the Markov chain, then snap
    /// each note to the scale.
    pub fn generate_melody(&self, length: usize, rng_seed: u64) -> NoteSequence {
        let intervals = self.scale.scale_type.intervals();
        let root = self.scale.root;
        // Seed notes: first few scale degrees
        let seed: Vec<u8> = intervals[..intervals.len().min(self.chain.order.n())]
            .iter()
            .map(|&i| (root + i).min(127))
            .collect();

        let raw_notes = self.chain.generate(&seed, length, rng_seed);

        // Snap to scale
        let snapped: Vec<u8> = raw_notes
            .iter()
            .map(|&n| self.scale.snap_to_scale(n))
            .collect();

        NoteSequence::quantize(snapped, 120.0, 50.0)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn training_increases_transition_counts() {
        let mut chain = MarkovChain::new(MarkovOrder::First);
        let notes: Vec<u8> = vec![60, 62, 64, 65, 67, 69, 71, 72];
        chain.train(&notes);
        // Should have transitions for each consecutive pair
        let total: u32 = chain.transitions.values().flat_map(|m| m.values()).sum();
        assert_eq!(total, (notes.len() - 1) as u32);
    }

    #[test]
    fn generation_produces_correct_length() {
        let mut chain = MarkovChain::new(MarkovOrder::First);
        let notes: Vec<u8> = (60u8..84).collect();
        chain.train(&notes);
        let seed = vec![60u8];
        let gen = chain.generate(&seed, 16, 42);
        assert_eq!(gen.len(), 16);
    }

    #[test]
    fn scale_snapping_stays_in_scale() {
        let constraint = ScaleConstraint {
            root: 60, // middle C
            scale_type: ScaleType::Major,
        };
        let intervals = ScaleType::Major.intervals();
        // Build set of valid scale notes across all octaves
        let valid: std::collections::HashSet<u8> = (0u8..=127)
            .filter(|&n| {
                let note_class = n % 12;
                let root_class = 60u8 % 12;
                intervals.iter().any(|&i| (root_class + i) % 12 == note_class)
            })
            .collect();

        for note in 0u8..=127 {
            let snapped = constraint.snap_to_scale(note);
            assert!(
                valid.contains(&snapped),
                "snap_to_scale({}) = {} is not in C Major scale",
                note,
                snapped
            );
        }
    }

    #[test]
    fn entropy_positive_for_trained_chain() {
        let mut chain = MarkovChain::new(MarkovOrder::First);
        // Train with a sequence that has multiple possible successors for 60
        let notes: Vec<u8> = vec![60, 62, 60, 64, 60, 67, 60, 65];
        chain.train(&notes);
        let entropy = chain.transition_entropy(&[60]);
        assert!(entropy > 0.0, "entropy should be positive for non-deterministic chain, got {}", entropy);
    }

    #[test]
    fn most_likely_next_returns_most_common() {
        let mut chain = MarkovChain::new(MarkovOrder::First);
        // 60 → 62 appears 3 times, 60 → 64 appears 1 time
        let notes: Vec<u8> = vec![60, 62, 60, 62, 60, 62, 60, 64];
        chain.train(&notes);
        let next = chain.most_likely_next(&[60]);
        assert_eq!(next, Some(62));
    }

    #[test]
    fn note_sequence_quantize_has_correct_length() {
        let notes: Vec<u8> = vec![60, 62, 64, 65, 67];
        let seq = NoteSequence::quantize(notes.clone(), 120.0, 50.0);
        assert_eq!(seq.notes.len(), 5);
        assert_eq!(seq.durations.len(), 5);
        assert_eq!(seq.velocities.len(), 5);
    }

    #[test]
    fn humanize_modifies_velocities_within_bounds() {
        let notes: Vec<u8> = vec![60, 62, 64, 65, 67];
        let mut seq = NoteSequence::quantize(notes, 120.0, 50.0);
        seq.humanize(10.0, 20, 1234);
        for &v in &seq.velocities {
            assert!(v >= 1 && v <= 127, "velocity out of range: {}", v);
        }
    }

    #[test]
    fn melody_generator_produces_correct_length() {
        let scale = ScaleConstraint { root: 60, scale_type: ScaleType::Major };
        let mut gen = MarkovMelodyGenerator::new(MarkovOrder::First, scale);
        gen.train_from_scale(60, 32, 42);
        let melody = gen.generate_melody(16, 99);
        assert_eq!(melody.notes.len(), 16);
    }

    #[test]
    fn second_order_chain_works() {
        let mut chain = MarkovChain::new(MarkovOrder::Second);
        let notes: Vec<u8> = (60u8..84).collect();
        chain.train(&notes);
        let seed = vec![60u8, 62u8];
        let gen = chain.generate(&seed, 10, 7);
        assert!(!gen.is_empty());
    }
}
