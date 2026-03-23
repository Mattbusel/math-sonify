//! Stochastic music composition using random walks and Markov chains.

// ---------------------------------------------------------------------------
// Note
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub pitch_hz: f64,
    pub duration_beats: f64,
    /// MIDI velocity 0–127.
    pub velocity: u8,
}

impl Note {
    pub fn new(pitch_hz: f64, duration_beats: f64, velocity: u8) -> Self {
        Self {
            pitch_hz,
            duration_beats,
            velocity: velocity.min(127),
        }
    }
}

// ---------------------------------------------------------------------------
// Minimal LCG for no-dependency seeded pseudo-randomness
// ---------------------------------------------------------------------------

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(1) }
    }

    /// Returns a value in [0, 1).
    fn next_f64(&mut self) -> f64 {
        // Splitmix64
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        (z as f64) / (u64::MAX as f64)
    }

    /// Returns an integer in [0, n).
    #[allow(dead_code)]
    fn next_usize(&mut self, n: usize) -> usize {
        (self.next_f64() * n as f64) as usize % n
    }

    /// Returns an integer in [lo, hi] inclusive.
    #[allow(dead_code)]
    fn next_range(&mut self, lo: i32, hi: i32) -> i32 {
        let range = (hi - lo + 1) as usize;
        lo + self.next_usize(range) as i32
    }
}

// ---------------------------------------------------------------------------
// RandomWalkComposer
// ---------------------------------------------------------------------------

pub struct RandomWalkComposer;

impl RandomWalkComposer {
    /// Random walk on scale degrees.
    /// Step size drawn from {-2, -1, 0, 1, 2} with weights [1, 3, 2, 3, 1].
    pub fn compose(
        steps: usize,
        root_hz: f64,
        scale_intervals: &[u8],
        seed: u64,
    ) -> Vec<Note> {
        if scale_intervals.is_empty() || steps == 0 {
            return Vec::new();
        }

        let mut rng = Lcg::new(seed);
        let scale_len = scale_intervals.len() as i32;

        // Cumulative weights for step choices {-2,-1,0,1,2}: [1,3,2,3,1] -> sum=10
        let step_choices: [(i32, u32); 5] = [(-2, 1), (-1, 3), (0, 2), (1, 3), (2, 1)];
        let total_weight: u32 = step_choices.iter().map(|(_, w)| w).sum();

        let mut degree: i32 = 0; // current scale degree index
        let mut notes = Vec::with_capacity(steps);

        for _ in 0..steps {
            // Convert degree to frequency
            let hz = degree_to_hz(degree, root_hz, scale_intervals);

            notes.push(Note::new(hz, 1.0, 80));

            // Pick step
            let roll = (rng.next_f64() * total_weight as f64) as u32;
            let mut cumulative = 0u32;
            let mut step = 0i32;
            for (s, w) in &step_choices {
                cumulative += w;
                if roll < cumulative {
                    step = *s;
                    break;
                }
            }

            degree += step;
            // Wrap degree within range [-2*scale_len, 2*scale_len] to keep variety
            if degree > 2 * scale_len {
                degree -= scale_len;
            } else if degree < -2 * scale_len {
                degree += scale_len;
            }
        }

        notes
    }

    /// Apply a rhythmic pattern (cycling through durations) to a note sequence.
    pub fn with_rhythm(mut notes: Vec<Note>, pattern: &[f64], seed: u64) -> Vec<Note> {
        if pattern.is_empty() {
            return notes;
        }
        let mut rng = Lcg::new(seed);
        let _ = rng.next_f64(); // mix seed
        for (i, note) in notes.iter_mut().enumerate() {
            note.duration_beats = pattern[i % pattern.len()];
        }
        notes
    }

    /// Transpose all notes by `semitones` semitones (can be negative).
    pub fn transpose(notes: &[Note], semitones: i8) -> Vec<Note> {
        let ratio = 2.0_f64.powf(semitones as f64 / 12.0);
        notes
            .iter()
            .map(|n| Note::new(n.pitch_hz * ratio, n.duration_beats, n.velocity))
            .collect()
    }
}

/// Convert a scale degree index to Hz.
/// Degree 0 = root, positive = ascending, negative = descending.
fn degree_to_hz(degree: i32, root_hz: f64, scale_intervals: &[u8]) -> f64 {
    let n = scale_intervals.len() as i32;
    let total_semitones: i32 = if degree >= 0 {
        let octave = degree / n;
        let idx = (degree % n) as usize;
        let semi: i32 = scale_intervals[..idx].iter().map(|&s| s as i32).sum();
        octave * 12 + semi
    } else {
        // Negative degree: go down
        let abs_deg = (-degree) as usize;
        let octave = (abs_deg as i32 - 1) / n + 1;
        let idx = n as usize - (abs_deg % n as usize);
        let idx = if idx == n as usize { 0 } else { idx };
        let semi: i32 = scale_intervals[idx..].iter().map(|&s| s as i32).sum();
        -(octave * 12 - (12 - semi))
    };
    root_hz * 2.0_f64.powf(total_semitones as f64 / 12.0)
}

// ---------------------------------------------------------------------------
// BrownianMotionComposer
// ---------------------------------------------------------------------------

pub struct BrownianMotionComposer;

impl BrownianMotionComposer {
    /// Brownian motion in log-pitch space, clamped to [80, 4000] Hz.
    pub fn compose_pitch_sequence(
        steps: usize,
        start_hz: f64,
        volatility: f64,
        seed: u64,
    ) -> Vec<f64> {
        if steps == 0 {
            return Vec::new();
        }

        let mut rng = Lcg::new(seed);
        let log_min = 80.0_f64.ln();
        let log_max = 4000.0_f64.ln();
        let start_hz = start_hz.clamp(80.0, 4000.0);
        let mut log_pitch = start_hz.ln();
        let mut pitches = Vec::with_capacity(steps);
        pitches.push(start_hz);

        for _ in 1..steps {
            // Box-Muller for normal-ish noise
            let u1 = rng.next_f64().max(1e-15);
            let u2 = rng.next_f64();
            let normal = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            log_pitch += volatility * normal;
            log_pitch = log_pitch.clamp(log_min, log_max);
            pitches.push(log_pitch.exp());
        }

        pitches
    }

    /// Convert pitch sequence to Note sequence with fixed duration and velocity.
    pub fn to_notes(pitches: &[f64], base_duration: f64, velocity: u8) -> Vec<Note> {
        pitches
            .iter()
            .map(|&hz| Note::new(hz, base_duration, velocity))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// StochasticPhrase
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StochasticPhrase {
    pub notes: Vec<Note>,
    pub tempo_bpm: f64,
}

impl StochasticPhrase {
    pub fn new(notes: Vec<Note>, tempo_bpm: f64) -> Self {
        Self { notes, tempo_bpm }
    }

    /// Total duration in milliseconds.
    pub fn duration_ms(&self) -> f64 {
        let total_beats: f64 = self.notes.iter().map(|n| n.duration_beats).sum();
        // beats * (60_000 ms / bpm)
        total_beats * 60_000.0 / self.tempo_bpm
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Major scale intervals (whole/half steps in semitones)
    const MAJOR: &[u8] = &[2, 2, 1, 2, 2, 2, 1];

    #[test]
    fn test_compose_length() {
        let notes = RandomWalkComposer::compose(16, 440.0, MAJOR, 42);
        assert_eq!(notes.len(), 16);
    }

    #[test]
    fn test_compose_positive_hz() {
        let notes = RandomWalkComposer::compose(32, 261.63, MAJOR, 7);
        for n in &notes {
            assert!(n.pitch_hz > 0.0, "pitch must be positive, got {}", n.pitch_hz);
        }
    }

    #[test]
    fn test_compose_velocity_clamped() {
        let notes = RandomWalkComposer::compose(8, 440.0, MAJOR, 1);
        for n in &notes {
            assert!(n.velocity <= 127);
        }
    }

    #[test]
    fn test_with_rhythm() {
        let notes = RandomWalkComposer::compose(6, 440.0, MAJOR, 1);
        let pattern = vec![0.5, 0.25, 1.0];
        let rhythmic = RandomWalkComposer::with_rhythm(notes, &pattern, 0);
        assert_eq!(rhythmic[0].duration_beats, 0.5);
        assert_eq!(rhythmic[1].duration_beats, 0.25);
        assert_eq!(rhythmic[2].duration_beats, 1.0);
        assert_eq!(rhythmic[3].duration_beats, 0.5);
    }

    #[test]
    fn test_transpose_up() {
        let notes = vec![Note::new(440.0, 1.0, 80)];
        let transposed = RandomWalkComposer::transpose(&notes, 12);
        let expected = 440.0 * 2.0;
        assert!((transposed[0].pitch_hz - expected).abs() < 0.01);
    }

    #[test]
    fn test_transpose_down() {
        let notes = vec![Note::new(440.0, 1.0, 80)];
        let transposed = RandomWalkComposer::transpose(&notes, -12);
        let expected = 220.0;
        assert!((transposed[0].pitch_hz - expected).abs() < 0.01);
    }

    #[test]
    fn test_brownian_length() {
        let pitches = BrownianMotionComposer::compose_pitch_sequence(20, 440.0, 0.1, 99);
        assert_eq!(pitches.len(), 20);
    }

    #[test]
    fn test_brownian_clamp() {
        let pitches = BrownianMotionComposer::compose_pitch_sequence(100, 440.0, 10.0, 5);
        for &p in &pitches {
            assert!(p >= 79.9 && p <= 4001.0, "pitch {} out of range", p);
        }
    }

    #[test]
    fn test_brownian_to_notes() {
        let pitches = vec![220.0, 330.0, 440.0];
        let notes = BrownianMotionComposer::to_notes(&pitches, 0.5, 64);
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].pitch_hz, 220.0);
        assert_eq!(notes[0].duration_beats, 0.5);
        assert_eq!(notes[0].velocity, 64);
    }

    #[test]
    fn test_stochastic_phrase_duration() {
        let notes = vec![
            Note::new(440.0, 1.0, 80),
            Note::new(550.0, 2.0, 80),
        ];
        let phrase = StochasticPhrase::new(notes, 120.0);
        // 3 beats at 120 bpm = 3 * 500ms = 1500ms
        assert!((phrase.duration_ms() - 1500.0).abs() < 0.01);
    }

    #[test]
    fn test_stochastic_phrase_empty() {
        let phrase = StochasticPhrase::new(vec![], 120.0);
        assert_eq!(phrase.duration_ms(), 0.0);
    }

    #[test]
    fn test_compose_empty_scale() {
        let notes = RandomWalkComposer::compose(4, 440.0, &[], 1);
        assert!(notes.is_empty());
    }

    #[test]
    fn test_compose_zero_steps() {
        let notes = RandomWalkComposer::compose(0, 440.0, MAJOR, 1);
        assert!(notes.is_empty());
    }

    #[test]
    fn test_deterministic_seed() {
        let a = RandomWalkComposer::compose(10, 440.0, MAJOR, 12345);
        let b = RandomWalkComposer::compose(10, 440.0, MAJOR, 12345);
        assert_eq!(a.len(), b.len());
        for (na, nb) in a.iter().zip(b.iter()) {
            assert!((na.pitch_hz - nb.pitch_hz).abs() < 1e-9);
        }
    }
}
