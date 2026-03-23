//! Generative composition engine — maps dynamical system state to musical structure.
//!
//! The engine observes the running dynamical system state and derives high-level
//! musical decisions (key, tempo, harmonic density) from mathematical quantities:
//!
//! | Mathematical quantity | Musical parameter |
//! |-----------------------|-------------------|
//! | Attractor basin index | Musical key / tonal centre |
//! | Bifurcation proximity | Key change (modulation) |
//! | Lyapunov exponent     | Tempo (chaotic → faster) |
//! | State norm            | Harmonic density / register |
//!
//! The engine is polled from the simulation thread at the control rate and
//! returns a [`CompositionFrame`] that the audio thread can act on.

#![allow(dead_code)]

// ── Musical constants ─────────────────────────────────────────────────────────

/// 12 pitch-class names (C = 0 … B = 11).
pub const PITCH_CLASS_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Scale degree offsets for common modes (semitones from root).
pub const SCALE_MAJOR: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
pub const SCALE_MINOR: [u8; 7] = [0, 2, 3, 5, 7, 8, 10];
pub const SCALE_DORIAN: [u8; 7] = [0, 2, 3, 5, 7, 9, 10];
pub const SCALE_PHRYGIAN: [u8; 7] = [0, 1, 3, 5, 7, 8, 10];
pub const SCALE_LYDIAN: [u8; 7] = [0, 2, 4, 6, 7, 9, 11];
pub const SCALE_PENTATONIC: [u8; 5] = [0, 2, 4, 7, 9];

/// Tempo range driven by Lyapunov exponent (BPM).
const TEMPO_MIN_BPM: f64 = 40.0;
const TEMPO_MAX_BPM: f64 = 180.0;

/// Lyapunov exponent range expected for the mapped attractors.
const LYAP_MIN: f64 = -2.0;
const LYAP_MAX: f64 = 3.0;

/// Number of attractor basins mapped to distinct keys.
const NUM_BASINS: usize = 12;

// ── Basin classifier ──────────────────────────────────────────────────────────

/// Identifies which attractor basin the current state occupies.
///
/// Uses a simple discretisation of the state norm into `NUM_BASINS` buckets.
/// In practice you would use a more sophisticated method (e.g. nearest
/// fixed-point or kd-tree nearest-attractor query), but this gives musically
/// useful results with near-zero overhead.
pub fn classify_basin(state: &[f64]) -> usize {
    let norm: f64 = state.iter().map(|x| x * x).sum::<f64>().sqrt();
    // Map norm into [0, NUM_BASINS) using a log-ish scale clamped at 50.
    let clamped = norm.min(50.0);
    let idx = (clamped / 50.0 * NUM_BASINS as f64) as usize;
    idx.min(NUM_BASINS - 1)
}

// ── Scale helpers ─────────────────────────────────────────────────────────────

/// Scale descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scale {
    /// Root pitch class (0 = C, 1 = C#, …).
    pub root: u8,
    /// Scale mode name (for display).
    pub mode: &'static str,
    /// Semitone offsets from root.
    pub degrees: Vec<u8>,
}

impl Scale {
    /// Return the MIDI note numbers (in octave 4) for all scale degrees.
    pub fn midi_notes(&self, octave: u8) -> Vec<u8> {
        let base = 12 + 12 * octave.min(8) + self.root;
        self.degrees
            .iter()
            .map(|&d| base.saturating_add(d))
            .collect()
    }

    /// Snap a MIDI note to the nearest degree in this scale.
    pub fn snap(&self, midi: u8) -> u8 {
        let root_class = self.root % 12;
        let note_class = midi % 12;
        let offset = ((note_class as i16 - root_class as i16).rem_euclid(12)) as u8;
        // Find nearest degree.
        let nearest = self
            .degrees
            .iter()
            .min_by_key(|&&d| (d as i8 - offset as i8).unsigned_abs())
            .copied()
            .unwrap_or(0);
        let diff = nearest as i8 - offset as i8;
        midi.saturating_add_signed(diff)
    }
}

/// Map a basin index to a musical scale (cycles through key of circle of fifths).
pub fn basin_to_scale(basin: usize) -> Scale {
    // Circle of fifths: each step up is 7 semitones.
    let root = ((basin as u32 * 7) % 12) as u8;
    // Alternate between major and dorian modes for interest.
    let (mode, degrees) = if basin % 2 == 0 {
        ("major", SCALE_MAJOR.to_vec())
    } else {
        ("dorian", SCALE_DORIAN.to_vec())
    };
    Scale { root, mode, degrees }
}

// ── Bifurcation detector ──────────────────────────────────────────────────────

/// Lightweight online detector for bifurcation-like events.
///
/// Tracks the running variance of the first state variable and fires
/// whenever the variance jumps by more than `threshold` relative to its
/// short-term moving average.
pub struct BifurcationDetector {
    /// Exponential moving average of the state variable.
    ema: f64,
    /// EMA of the squared deviation (variance proxy).
    ema_var: f64,
    /// Smoothing factor (0 < alpha < 1; smaller = longer memory).
    alpha: f64,
    /// Jump threshold multiplier.
    threshold: f64,
    /// True if a bifurcation was detected on the last call to `update`.
    pub triggered: bool,
}

impl BifurcationDetector {
    pub fn new(alpha: f64, threshold: f64) -> Self {
        Self {
            ema: 0.0,
            ema_var: 1.0,
            alpha,
            threshold,
            triggered: false,
        }
    }

    /// Feed the latest value of a state variable.  Sets `triggered` to `true`
    /// if a bifurcation-like jump is detected.
    pub fn update(&mut self, x: f64) {
        let alpha = self.alpha;
        let dev = x - self.ema;
        let inst_var = dev * dev;
        self.ema = (1.0 - alpha) * self.ema + alpha * x;
        let prev_var = self.ema_var;
        self.ema_var = (1.0 - alpha) * self.ema_var + alpha * inst_var;
        // Bifurcation proxy: variance jumped by more than threshold × previous.
        self.triggered = inst_var > self.threshold * prev_var && prev_var > 1e-10;
    }
}

// ── Tempo mapper ──────────────────────────────────────────────────────────────

/// Map a Lyapunov exponent to a tempo in BPM.
///
/// Positive (chaotic) → faster; negative (periodic/stable) → slower.
pub fn lyapunov_to_bpm(lyap: f64) -> f64 {
    let t = ((lyap - LYAP_MIN) / (LYAP_MAX - LYAP_MIN)).clamp(0.0, 1.0);
    TEMPO_MIN_BPM + t * (TEMPO_MAX_BPM - TEMPO_MIN_BPM)
}

// ── Harmonic density ──────────────────────────────────────────────────────────

/// Return the recommended number of active harmonic voices (1–8) based on the
/// magnitude of the state vector.
pub fn state_to_voice_count(state: &[f64]) -> usize {
    let norm: f64 = state.iter().map(|x| x * x).sum::<f64>().sqrt();
    // Map [0, 40] → [1, 8].
    let v = (norm / 40.0 * 7.0) as usize + 1;
    v.clamp(1, 8)
}

// ── Composition frame ─────────────────────────────────────────────────────────

/// A snapshot of musical decisions for one control-rate tick.
#[derive(Debug, Clone)]
pub struct CompositionFrame {
    /// Currently active scale.
    pub scale: Scale,
    /// Recommended tempo in BPM.
    pub tempo_bpm: f64,
    /// Whether a key change occurred this tick.
    pub key_changed: bool,
    /// Number of recommended voices.
    pub voice_count: usize,
    /// MIDI note suggestions (one per voice, already snapped to scale).
    pub notes: Vec<u8>,
    /// Basin index (0–11) — which attractor region we are in.
    pub basin: usize,
}

// ── Composition engine ────────────────────────────────────────────────────────

/// Stateful generative composition engine.
///
/// Feed it state vectors and Lyapunov exponents each tick via [`update`].
pub struct CompositionEngine {
    bifurcation: BifurcationDetector,
    current_basin: usize,
    current_scale: Scale,
    /// Minimum ticks between key changes (avoid rapid modulation).
    key_change_cooldown: u32,
    cooldown_remaining: u32,
    /// Base MIDI octave for note generation.
    base_octave: u8,
}

impl CompositionEngine {
    /// Create a new engine.
    ///
    /// `bifurcation_alpha` controls the EMA memory (0.01 = slow, 0.1 = fast).
    /// `bifurcation_threshold` is the variance-jump multiplier that triggers a
    /// key change (e.g. `5.0` = variance must 5× to trigger).
    /// `key_change_cooldown` is the minimum number of ticks between key changes.
    pub fn new(
        bifurcation_alpha: f64,
        bifurcation_threshold: f64,
        key_change_cooldown: u32,
    ) -> Self {
        let current_basin = 0;
        let current_scale = basin_to_scale(current_basin);
        Self {
            bifurcation: BifurcationDetector::new(bifurcation_alpha, bifurcation_threshold),
            current_basin,
            current_scale,
            key_change_cooldown,
            cooldown_remaining: 0,
            base_octave: 4,
        }
    }

    /// Update the engine with the current state vector and Lyapunov exponent.
    ///
    /// Returns a [`CompositionFrame`] describing the musical decisions for this
    /// tick.
    pub fn update(&mut self, state: &[f64], lyapunov: f64) -> CompositionFrame {
        // Compute basin.
        let basin = classify_basin(state);

        // Feed bifurcation detector.
        let first = state.first().copied().unwrap_or(0.0);
        self.bifurcation.update(first);

        // Decide whether to change key.
        let mut key_changed = false;
        if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
        }
        let basin_changed = basin != self.current_basin;
        let bifurcation_fired = self.bifurcation.triggered;

        if (basin_changed || bifurcation_fired) && self.cooldown_remaining == 0 {
            self.current_basin = basin;
            self.current_scale = basin_to_scale(basin);
            key_changed = true;
            self.cooldown_remaining = self.key_change_cooldown;
            log::debug!(
                "[composition] key change → {} {} (basin {basin}, lyap {lyapunov:.2})",
                PITCH_CLASS_NAMES[self.current_scale.root as usize],
                self.current_scale.mode
            );
        }

        // Derive musical parameters.
        let tempo_bpm = lyapunov_to_bpm(lyapunov);
        let voice_count = state_to_voice_count(state);

        // Generate note suggestions.
        let scale_notes = self.current_scale.midi_notes(self.base_octave);
        let notes: Vec<u8> = (0..voice_count)
            .map(|i| {
                // Distribute voices across scale degrees, slightly above base octave.
                let degree_idx = i % scale_notes.len();
                let octave_shift = (i / scale_notes.len()) as u8;
                scale_notes[degree_idx].saturating_add(12 * octave_shift)
            })
            .collect();

        CompositionFrame {
            scale: self.current_scale.clone(),
            tempo_bpm,
            key_changed,
            voice_count,
            notes,
            basin,
        }
    }

    /// Current active scale.
    pub fn scale(&self) -> &Scale {
        &self.current_scale
    }

    /// Set the base MIDI octave for note generation (default: 4).
    pub fn set_base_octave(&mut self, octave: u8) {
        self.base_octave = octave.min(8);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_basin_zero() {
        assert_eq!(classify_basin(&[0.0, 0.0, 0.0]), 0);
    }

    #[test]
    fn test_classify_basin_max() {
        let basin = classify_basin(&[100.0, 100.0, 100.0]);
        assert_eq!(basin, NUM_BASINS - 1);
    }

    #[test]
    fn test_basin_to_scale_root_cycles() {
        let s0 = basin_to_scale(0);
        let s12 = basin_to_scale(12);
        assert_eq!(s0.root, s12.root, "roots should cycle every 12 basins");
    }

    #[test]
    fn test_lyapunov_to_bpm_range() {
        let slow = lyapunov_to_bpm(LYAP_MIN);
        let fast = lyapunov_to_bpm(LYAP_MAX);
        assert!((slow - TEMPO_MIN_BPM).abs() < 1.0);
        assert!((fast - TEMPO_MAX_BPM).abs() < 1.0);
    }

    #[test]
    fn test_composition_engine_runs() {
        let mut engine = CompositionEngine::new(0.05, 5.0, 60);
        let state = vec![10.0, 5.0, -3.0];
        let frame = engine.update(&state, 1.2);
        assert!(frame.tempo_bpm >= TEMPO_MIN_BPM);
        assert!(frame.tempo_bpm <= TEMPO_MAX_BPM);
        assert!(!frame.notes.is_empty());
    }

    #[test]
    fn test_scale_snap() {
        // C major: C D E F G A B
        let scale = Scale {
            root: 0,
            mode: "major",
            degrees: SCALE_MAJOR.to_vec(),
        };
        // MIDI 61 = C#4 → should snap to C4 (60) or D4 (62)
        let snapped = scale.snap(61);
        assert!(snapped == 60 || snapped == 62, "snapped={snapped}");
    }

    #[test]
    fn test_bifurcation_detector_triggers_on_jump() {
        let mut det = BifurcationDetector::new(0.1, 3.0);
        // Feed stable signal.
        for _ in 0..50 {
            det.update(1.0);
        }
        // Large sudden jump.
        det.update(100.0);
        assert!(det.triggered, "large jump should trigger bifurcation detector");
    }

    #[test]
    fn test_voice_count_grows_with_norm() {
        let v1 = state_to_voice_count(&[1.0, 0.0, 0.0]);
        let v2 = state_to_voice_count(&[30.0, 20.0, 10.0]);
        assert!(v2 >= v1, "larger state should yield more voices");
    }
}
