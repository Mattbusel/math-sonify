//! # Polyrhythm Generator
//!
//! LCM-based phase tracking for generating polyrhythmic event sequences.
//! Supports classic patterns (3v2, 4v3, 5v4) and custom voice combinations.

// ── Math helpers ──────────────────────────────────────────────────────────────

/// Greatest common divisor (Euclidean algorithm).
pub fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let tmp = b;
        b = a % b;
        a = tmp;
    }
    a
}

/// Least common multiple.
pub fn lcm(a: u32, b: u32) -> u32 {
    if a == 0 || b == 0 {
        0
    } else {
        a / gcd(a, b) * b
    }
}

// ── RhythmVoice ──────────────────────────────────────────────────────────────

/// One rhythmic voice within a polyrhythm engine.
#[derive(Debug, Clone)]
pub struct RhythmVoice {
    /// Human-readable name, e.g. "high-hat" or "clave".
    pub name: String,
    /// How many evenly-spaced pulses this voice fires per cycle.
    pub pulses_per_cycle: u32,
    /// MIDI pitch for events emitted by this voice.
    pub pitch: u8,
    /// MIDI velocity for events emitted by this voice.
    pub velocity: u8,
}

// ── PolyrhythmEngine ─────────────────────────────────────────────────────────

/// Drives multiple rhythmic voices at different pulse rates, synchronised over
/// their LCM cycle length.
#[derive(Debug, Clone)]
pub struct PolyrhythmEngine {
    pub voices: Vec<RhythmVoice>,
    /// Length of one full cycle in ticks (LCM of all `pulses_per_cycle`).
    pub cycle_length: u32,
    /// Current tick within the cycle (0-based).
    pub current_step: u32,
}

impl PolyrhythmEngine {
    /// Construct an engine from a list of voices; computes `cycle_length` as
    /// the LCM of all pulse counts.
    pub fn new(voices: Vec<RhythmVoice>) -> Self {
        let cycle_length = voices
            .iter()
            .map(|v| v.pulses_per_cycle)
            .fold(1u32, lcm);
        Self {
            voices,
            cycle_length,
            current_step: 0,
        }
    }

    /// Advance one step; return `(name, pitch, velocity)` for every voice that
    /// fires on this step. Wraps at `cycle_length`.
    pub fn tick(&mut self) -> Vec<(String, u8, u8)> {
        let step = self.current_step;
        let mut events = Vec::new();

        for voice in &self.voices {
            // A voice fires when step is a multiple of (cycle / pulses).
            let interval = self.cycle_length / voice.pulses_per_cycle.max(1);
            if step % interval == 0 {
                events.push((voice.name.clone(), voice.pitch, voice.velocity));
            }
        }

        self.current_step = (self.current_step + 1) % self.cycle_length;
        events
    }

    /// Run one complete cycle from the current position; collect `(step, events)`.
    pub fn run_cycle(&mut self) -> Vec<(u32, Vec<(String, u8, u8)>)> {
        let mut out = Vec::new();
        // Reset to step 0 for a clean cycle.
        self.current_step = 0;
        for _ in 0..self.cycle_length {
            let step = self.current_step;
            let events = self.tick();
            if !events.is_empty() {
                out.push((step, events));
            }
        }
        out
    }

    /// Returns `true` if at least two voices have different pulse rates.
    pub fn is_polyrhythm(&self) -> bool {
        let mut it = self.voices.iter().map(|v| v.pulses_per_cycle);
        match it.next() {
            None => false,
            Some(first) => it.any(|p| p != first),
        }
    }

    // ── Classic patterns ──────────────────────────────────────────────────────

    /// 3-against-2 polyrhythm.
    pub fn three_against_two() -> Self {
        Self::new(vec![
            RhythmVoice {
                name: "three".to_string(),
                pulses_per_cycle: 3,
                pitch: 60,
                velocity: 100,
            },
            RhythmVoice {
                name: "two".to_string(),
                pulses_per_cycle: 2,
                pitch: 64,
                velocity: 90,
            },
        ])
    }

    /// 4-against-3 polyrhythm.
    pub fn four_against_three() -> Self {
        Self::new(vec![
            RhythmVoice {
                name: "four".to_string(),
                pulses_per_cycle: 4,
                pitch: 60,
                velocity: 100,
            },
            RhythmVoice {
                name: "three".to_string(),
                pulses_per_cycle: 3,
                pitch: 64,
                velocity: 90,
            },
        ])
    }

    /// 5-against-4 polyrhythm.
    pub fn five_against_four() -> Self {
        Self::new(vec![
            RhythmVoice {
                name: "five".to_string(),
                pulses_per_cycle: 5,
                pitch: 60,
                velocity: 100,
            },
            RhythmVoice {
                name: "four".to_string(),
                pulses_per_cycle: 4,
                pitch: 64,
                velocity: 90,
            },
        ])
    }

    /// African bell pattern approximation — voices \[12, 7, 4\].
    pub fn african_bell() -> Self {
        Self::new(vec![
            RhythmVoice {
                name: "bell-12".to_string(),
                pulses_per_cycle: 12,
                pitch: 76,
                velocity: 110,
            },
            RhythmVoice {
                name: "bell-7".to_string(),
                pulses_per_cycle: 7,
                pitch: 72,
                velocity: 100,
            },
            RhythmVoice {
                name: "bell-4".to_string(),
                pulses_per_cycle: 4,
                pitch: 69,
                velocity: 90,
            },
        ])
    }
}

// ── Phase offset ──────────────────────────────────────────────────────────────

/// Compute the relative phase in \[0, 1) between two voices at `step` within a
/// cycle of `cycle` ticks.
///
/// Returns the fractional difference between where each voice is in its own
/// sub-cycle at this step.
pub fn phase_offset(
    voice1_pulses: u32,
    voice2_pulses: u32,
    step: u32,
    cycle: u32,
) -> f64 {
    if cycle == 0 {
        return 0.0;
    }
    let interval1 = cycle as f64 / voice1_pulses.max(1) as f64;
    let interval2 = cycle as f64 / voice2_pulses.max(1) as f64;
    let phase1 = (step as f64 % interval1) / interval1;
    let phase2 = (step as f64 % interval2) / interval2;
    (phase1 - phase2).abs().min(1.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcd_basic() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(7, 3), 1);
        assert_eq!(gcd(6, 6), 6);
    }

    #[test]
    fn lcm_basic() {
        assert_eq!(lcm(3, 2), 6);
        assert_eq!(lcm(4, 3), 12);
        assert_eq!(lcm(5, 4), 20);
    }

    #[test]
    fn three_against_two_cycle_length() {
        let eng = PolyrhythmEngine::three_against_two();
        assert_eq!(eng.cycle_length, 6, "LCM(3,2) should be 6");
    }

    #[test]
    fn three_against_two_hits_correct_steps() {
        let mut eng = PolyrhythmEngine::three_against_two();
        eng.current_step = 0;

        // In a cycle of 6:
        // "three" fires every 6/3=2 ticks → steps 0, 2, 4
        // "two"   fires every 6/2=3 ticks → steps 0, 3
        let mut three_steps: Vec<u32> = Vec::new();
        let mut two_steps: Vec<u32> = Vec::new();

        eng.current_step = 0;
        for step in 0..6u32 {
            let events = eng.tick();
            for (name, _, _) in &events {
                if name == "three" {
                    three_steps.push(step);
                } else {
                    two_steps.push(step);
                }
            }
        }
        assert_eq!(three_steps, vec![0, 2, 4]);
        assert_eq!(two_steps, vec![0, 3]);
    }

    #[test]
    fn cycle_wraps_correctly() {
        let mut eng = PolyrhythmEngine::three_against_two();
        // Advance past one full cycle.
        for _ in 0..6 {
            eng.tick();
        }
        assert_eq!(eng.current_step, 0, "should wrap back to 0");
    }

    #[test]
    fn is_polyrhythm_true_for_different_pulses() {
        assert!(PolyrhythmEngine::three_against_two().is_polyrhythm());
    }

    #[test]
    fn is_polyrhythm_false_for_equal_pulses() {
        let eng = PolyrhythmEngine::new(vec![
            RhythmVoice {
                name: "a".into(),
                pulses_per_cycle: 4,
                pitch: 60,
                velocity: 100,
            },
            RhythmVoice {
                name: "b".into(),
                pulses_per_cycle: 4,
                pitch: 64,
                velocity: 90,
            },
        ]);
        assert!(!eng.is_polyrhythm());
    }

    #[test]
    fn run_cycle_returns_all_trigger_steps() {
        let mut eng = PolyrhythmEngine::three_against_two();
        let cycle = eng.run_cycle();
        // Steps with events: 0, 2, 3, 4 (both at 0, three at 2, two at 3, three at 4).
        let steps: Vec<u32> = cycle.iter().map(|(s, _)| *s).collect();
        assert!(steps.contains(&0));
        assert!(steps.contains(&2));
        assert!(steps.contains(&3));
        assert!(steps.contains(&4));
    }

    #[test]
    fn phase_offset_in_range() {
        for step in 0..20u32 {
            let p = phase_offset(3, 2, step, 6);
            assert!(p >= 0.0 && p <= 1.0, "phase out of range: {p}");
        }
    }

    #[test]
    fn african_bell_has_three_voices() {
        let eng = PolyrhythmEngine::african_bell();
        assert_eq!(eng.voices.len(), 3);
        assert!(eng.is_polyrhythm());
    }
}
