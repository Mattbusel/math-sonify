//! Rule-based algorithmic composition using L-systems and cellular automata.
//!
//! # L-systems
//! An [`LSystemComposer`] expands a string axiom through production rules and
//! then interprets characters as pitch offsets (in semitones) to produce a
//! sequence of [`Note`]s.
//!
//! # Cellular automata
//! A [`MusicalCellularAutomaton`] evolves a 1-D binary state with Wolfram
//! Rule 30 or Rule 110, then maps each row of the evolution to simultaneous
//! notes (active cells → pitches from a supplied [`crate::microtonal::TuningTable`]).

#![allow(dead_code)]

use std::collections::HashMap;

// ── Note (local) ──────────────────────────────────────────────────────────────

/// A single musical note produced by the algorithmic engine.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// Fundamental frequency in Hz.
    pub pitch_hz: f64,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// MIDI-style velocity (0–127).
    pub velocity: u8,
}

// ── MusicalLSystem ────────────────────────────────────────────────────────────

/// An L-system configured for musical use.
///
/// Each character in the produced string that appears in `variables` is
/// interpreted as a pitch offset (semitones) relative to `root_hz`.
#[derive(Debug, Clone)]
pub struct MusicalLSystem {
    /// Starting string for the rewriting system.
    pub axiom: String,
    /// Production rules: each char maps to a replacement string.
    pub rules: HashMap<char, String>,
    /// Pitch semantics: char → offset in semitones from root.
    pub variables: HashMap<char, f64>,
}

// ── LSystemComposer ──────────────────────────────────────────────────────────

/// Expands and interprets L-systems into [`Note`] sequences.
pub struct LSystemComposer;

impl LSystemComposer {
    // ── Expansion ─────────────────────────────────────────────────────────────

    /// Expand `axiom` by applying `rules` for `steps` iterations.
    pub fn iterate(axiom: &str, rules: &HashMap<char, String>, steps: u32) -> String {
        let mut current = axiom.to_string();
        for _ in 0..steps {
            let mut next = String::with_capacity(current.len() * 2);
            for ch in current.chars() {
                if let Some(replacement) = rules.get(&ch) {
                    next.push_str(replacement);
                } else {
                    next.push(ch);
                }
            }
            current = next;
        }
        current
    }

    // ── Note generation ───────────────────────────────────────────────────────

    /// Convert an expanded L-system string into a sequence of [`Note`]s.
    ///
    /// Only characters present in `variables` generate notes; others are
    /// treated as articulation/structural markers and skipped.
    ///
    /// The frequency for an offset of `n` semitones is
    /// `root_hz × 2^(n / 12)`.
    pub fn to_notes(
        s: &str,
        variables: &HashMap<char, f64>,
        root_hz: f64,
        base_duration: f64,
    ) -> Vec<Note> {
        s.chars()
            .filter_map(|ch| {
                variables.get(&ch).map(|&semitones| {
                    let pitch_hz = root_hz * 2.0_f64.powf(semitones / 12.0);
                    Note {
                        pitch_hz,
                        duration_secs: base_duration,
                        velocity: 80,
                    }
                })
            })
            .collect()
    }

    // ── Built-in definitions ──────────────────────────────────────────────────

    /// Sierpinski-triangle melody L-system definition.
    ///
    /// Produces a fractal, self-similar melodic pattern when interpreted with
    /// the included variable map.
    pub fn sierpinski_melody() -> MusicalLSystem {
        let mut rules = HashMap::new();
        rules.insert('A', "B-A-B".to_string());
        rules.insert('B', "A+B+A".to_string());

        let mut variables = HashMap::new();
        variables.insert('A', 0.0);   // root
        variables.insert('B', 7.0);   // perfect fifth

        MusicalLSystem {
            axiom: "A".to_string(),
            rules,
            variables,
        }
    }

    /// Dragon-curve melody L-system definition.
    ///
    /// The dragon curve grammar yields a winding melodic line when pitch
    /// offsets are assigned to its characters.
    pub fn dragon_curve_melody() -> MusicalLSystem {
        let mut rules = HashMap::new();
        rules.insert('X', "X+YF+".to_string());
        rules.insert('Y', "-FX-Y".to_string());

        let mut variables = HashMap::new();
        variables.insert('F', 0.0);   // root (forward step = play note)
        variables.insert('X', 4.0);   // major third
        variables.insert('Y', 7.0);   // perfect fifth

        MusicalLSystem {
            axiom: "FX".to_string(),
            rules,
            variables,
        }
    }
}

// ── MusicalCellularAutomaton ──────────────────────────────────────────────────

/// 1-D binary cellular automata with musical output interpretation.
pub struct MusicalCellularAutomaton;

impl MusicalCellularAutomaton {
    // ── Rule 30 ───────────────────────────────────────────────────────────────

    /// Apply one step of Wolfram Rule 30 to `state`.
    ///
    /// Boundary conditions are periodic (toroidal).
    pub fn rule_30(state: &[bool]) -> Vec<bool> {
        Self::apply_rule(state, 30)
    }

    // ── Rule 110 ──────────────────────────────────────────────────────────────

    /// Apply one step of Wolfram Rule 110 to `state`.
    pub fn rule_110(state: &[bool]) -> Vec<bool> {
        Self::apply_rule(state, 110)
    }

    // ── Evolve ────────────────────────────────────────────────────────────────

    /// Evolve `state` for `steps` generations using the supplied rule function.
    ///
    /// Returns all generations including the initial state.
    pub fn evolve(
        state: &[bool],
        rule_fn: fn(&[bool]) -> Vec<bool>,
        steps: usize,
    ) -> Vec<Vec<bool>> {
        let mut history = Vec::with_capacity(steps + 1);
        history.push(state.to_vec());
        for i in 0..steps {
            let next = rule_fn(&history[i]);
            history.push(next);
        }
        history
    }

    // ── CA to notes ───────────────────────────────────────────────────────────

    /// Map a CA evolution to a sequence of [`Note`]s.
    ///
    /// Each row of `evolution` is a time step. Active cells (`true`) are
    /// sounded simultaneously; inactive cells are silent. Pitches are drawn
    /// from `pitches_hz` by wrapping the cell index into the pitch table.
    ///
    /// Returns one [`Note`] per active cell across all time steps.
    pub fn ca_to_notes(
        evolution: &[Vec<bool>],
        pitches_hz: &[f64],
        duration: f64,
    ) -> Vec<Note> {
        if pitches_hz.is_empty() {
            return Vec::new();
        }
        let mut notes = Vec::new();
        for row in evolution {
            for (col, &active) in row.iter().enumerate() {
                if active {
                    let pitch = pitches_hz[col % pitches_hz.len()];
                    notes.push(Note {
                        pitch_hz: pitch,
                        duration_secs: duration,
                        velocity: 90,
                    });
                }
            }
        }
        notes
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    /// Generic Wolfram elementary CA step for the given rule number (0–255).
    fn apply_rule(state: &[bool], rule: u8) -> Vec<bool> {
        let n = state.len();
        if n == 0 {
            return Vec::new();
        }
        (0..n)
            .map(|i| {
                let left = if i == 0 { state[n - 1] } else { state[i - 1] };
                let center = state[i];
                let right = if i == n - 1 { state[0] } else { state[i + 1] };
                let pattern = ((left as u8) << 2) | ((center as u8) << 1) | (right as u8);
                (rule >> pattern) & 1 == 1
            })
            .collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── L-system tests ────────────────────────────────────────────────────────

    #[test]
    fn test_iterate_zero_steps() {
        let rules = HashMap::new();
        let result = LSystemComposer::iterate("AB", &rules, 0);
        assert_eq!(result, "AB");
    }

    #[test]
    fn test_iterate_no_matching_rule() {
        let rules = HashMap::new();
        let result = LSystemComposer::iterate("XYZ", &rules, 3);
        assert_eq!(result, "XYZ");
    }

    #[test]
    fn test_iterate_algae() {
        // Classic Lindenmayer algae: A→AB, B→A.
        let mut rules = HashMap::new();
        rules.insert('A', "AB".to_string());
        rules.insert('B', "A".to_string());
        let result = LSystemComposer::iterate("A", &rules, 4);
        // Step 0: A, 1: AB, 2: ABA, 3: ABAAB, 4: ABAABABA
        assert_eq!(result, "ABAABABA");
    }

    #[test]
    fn test_to_notes_maps_variables() {
        let mut variables = HashMap::new();
        variables.insert('A', 0.0);
        variables.insert('B', 7.0);
        let notes = LSystemComposer::to_notes("ABA", &variables, 440.0, 0.5);
        assert_eq!(notes.len(), 3);
        assert!((notes[0].pitch_hz - 440.0).abs() < 1e-6);
        // 7 semitones up from 440 = 440 * 2^(7/12) ≈ 659.26 Hz
        assert!((notes[1].pitch_hz - 659.255).abs() < 1.0);
        assert!((notes[2].pitch_hz - 440.0).abs() < 1e-6);
    }

    #[test]
    fn test_to_notes_ignores_non_variables() {
        let mut variables = HashMap::new();
        variables.insert('F', 0.0);
        let notes = LSystemComposer::to_notes("F+F-F", &variables, 440.0, 1.0);
        // Only 'F' chars generate notes.
        assert_eq!(notes.len(), 3);
    }

    #[test]
    fn test_sierpinski_melody_definition() {
        let sys = LSystemComposer::sierpinski_melody();
        assert!(!sys.axiom.is_empty());
        assert!(!sys.rules.is_empty());
        let expanded = LSystemComposer::iterate(&sys.axiom, &sys.rules, 3);
        let notes = LSystemComposer::to_notes(&expanded, &sys.variables, 440.0, 0.25);
        assert!(!notes.is_empty());
    }

    #[test]
    fn test_dragon_curve_melody_definition() {
        let sys = LSystemComposer::dragon_curve_melody();
        let expanded = LSystemComposer::iterate(&sys.axiom, &sys.rules, 3);
        let notes = LSystemComposer::to_notes(&expanded, &sys.variables, 440.0, 0.25);
        assert!(!notes.is_empty());
    }

    // ── CA tests ─────────────────────────────────────────────────────────────

    #[test]
    fn test_rule_30_single_cell() {
        // Start with a single active cell in the center.
        let mut state = vec![false; 7];
        state[3] = true;
        let next = MusicalCellularAutomaton::rule_30(&state);
        assert_eq!(next.len(), 7);
        // After one step the pattern should have changed.
        assert_ne!(next, state);
    }

    #[test]
    fn test_rule_110_length_preserved() {
        let state = vec![false, true, false, true, true, false];
        let next = MusicalCellularAutomaton::rule_110(&state);
        assert_eq!(next.len(), state.len());
    }

    #[test]
    fn test_evolve_step_count() {
        let state = vec![false; 8];
        let evolution = MusicalCellularAutomaton::evolve(
            &state,
            MusicalCellularAutomaton::rule_30,
            5,
        );
        // Initial + 5 steps = 6 rows.
        assert_eq!(evolution.len(), 6);
    }

    #[test]
    fn test_ca_to_notes_active_cells() {
        // Row with 3 active cells.
        let evolution = vec![vec![true, false, true, true]];
        let pitches = vec![220.0, 330.0, 440.0, 550.0];
        let notes = MusicalCellularAutomaton::ca_to_notes(&evolution, &pitches, 0.5);
        assert_eq!(notes.len(), 3); // cells 0, 2, 3 are active
    }

    #[test]
    fn test_ca_to_notes_empty_pitches() {
        let evolution = vec![vec![true, false, true]];
        let notes = MusicalCellularAutomaton::ca_to_notes(&evolution, &[], 0.5);
        assert!(notes.is_empty());
    }

    #[test]
    fn test_ca_to_notes_pitch_wrapping() {
        // More cells than pitches → wrap.
        let evolution = vec![vec![true, true, true]];
        let pitches = vec![440.0]; // only one pitch
        let notes = MusicalCellularAutomaton::ca_to_notes(&evolution, &pitches, 0.5);
        assert_eq!(notes.len(), 3);
        for n in &notes {
            assert!((n.pitch_hz - 440.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_rule_30_all_false() {
        let state = vec![false; 8];
        let next = MusicalCellularAutomaton::rule_30(&state);
        // All false input → rule 30 pattern 000 = bit 0 = 0 → all false.
        assert!(next.iter().all(|&b| !b));
    }

    #[test]
    fn test_note_duration_preserved() {
        let mut variables = HashMap::new();
        variables.insert('A', 0.0);
        let notes = LSystemComposer::to_notes("A", &variables, 440.0, 1.23);
        assert!((notes[0].duration_secs - 1.23).abs() < 1e-9);
    }
}
