//! Markov chain melody generator with phrase structure.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Note {
    pub pitch: u8,        // MIDI note number
    pub duration_ms: u32,
    pub velocity: u8,
    pub is_rest: bool,
}

#[derive(Debug, Clone)]
pub struct Phrase {
    pub notes: Vec<Note>,
    pub tempo_bpm: u32,
    pub time_signature: (u8, u8),
}

#[derive(Debug, Clone)]
pub struct MarkovMelodyConfig {
    pub order: usize,
    pub temperature: f64,
    pub min_phrase_len: usize,
    pub max_phrase_len: usize,
    pub seed: u64,
}

impl Default for MarkovMelodyConfig {
    fn default() -> Self {
        Self {
            order: 2,
            temperature: 1.0,
            min_phrase_len: 4,
            max_phrase_len: 16,
            seed: 42,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MarkovMelody {
    /// Maps n-gram context → (next_pitch → count)
    pub transition_table: HashMap<Vec<u8>, HashMap<u8, u32>>,
    pub config: MarkovMelodyConfig,
    pub lcg_state: u64,
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MelodyStats {
    pub note_count: usize,
    pub unique_pitches: usize,
    pub avg_velocity: f64,
    pub pitch_range: u8,
    pub avg_duration_ms: f64,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl MarkovMelody {
    pub fn new(config: MarkovMelodyConfig) -> Self {
        let lcg_state = config.seed.wrapping_add(1);
        Self {
            transition_table: HashMap::new(),
            config,
            lcg_state,
        }
    }

    /// LCG returning a float in [0, 1).
    pub fn lcg_next_float(&mut self) -> f64 {
        // Parameters from Numerical Recipes
        self.lcg_state = self
            .lcg_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.lcg_state >> 33) as f64 / (u32::MAX as f64 + 1.0)
    }

    /// Train the Markov chain from a pitch sequence.
    pub fn train(&mut self, pitch_sequence: &[u8]) {
        let order = self.config.order;
        if pitch_sequence.len() <= order {
            return;
        }
        for i in 0..(pitch_sequence.len() - order) {
            let context: Vec<u8> = pitch_sequence[i..i + order].to_vec();
            let next = pitch_sequence[i + order];
            let entry = self.transition_table.entry(context).or_default();
            *entry.entry(next).or_insert(0) += 1;
        }
    }

    /// Sample the next pitch given a context, using temperature-scaled probabilities.
    pub fn sample_next(&mut self, context: &[u8]) -> u8 {
        let order = self.config.order;
        let ctx_key: Vec<u8> = if context.len() >= order {
            context[context.len() - order..].to_vec()
        } else {
            context.to_vec()
        };

        if let Some(nexts) = self.transition_table.get(&ctx_key) {
            if !nexts.is_empty() {
                // Apply temperature scaling (log-space)
                let temp = self.config.temperature.max(1e-6);
                let mut weights: Vec<(u8, f64)> = nexts
                    .iter()
                    .map(|(&pitch, &count)| {
                        let w = (count as f64).ln() / temp;
                        (pitch, w)
                    })
                    .collect();

                // Softmax
                let max_w = weights.iter().map(|(_, w)| *w).fold(f64::NEG_INFINITY, f64::max);
                let sum: f64 = weights.iter().map(|(_, w)| (w - max_w).exp()).sum();
                let mut cumulative = 0.0;
                let r = self.lcg_next_float();
                for (pitch, w) in &mut weights {
                    cumulative += ((*w - max_w).exp()) / sum;
                    if r < cumulative {
                        return *pitch;
                    }
                }
                return weights.last().unwrap().0;
            }
        }

        // Context not found — pick uniformly from all known pitches
        let all_pitches: Vec<u8> = self
            .transition_table
            .values()
            .flat_map(|m| m.keys().copied())
            .collect();
        if all_pitches.is_empty() {
            return 60; // Middle C fallback
        }
        let idx = (self.lcg_next_float() * all_pitches.len() as f64) as usize;
        all_pitches[idx.min(all_pitches.len() - 1)]
    }

    /// Generate a phrase, constraining pitches to the provided scale mask.
    pub fn generate_phrase(&mut self, start_context: Vec<u8>, scale_mask: &[u8]) -> Phrase {
        let phrase_len_range = self.config.max_phrase_len - self.config.min_phrase_len;
        let phrase_len = self.config.min_phrase_len
            + (self.lcg_next_float() * phrase_len_range as f64) as usize;

        let durations = [250u32, 500, 750, 1000];
        let mut notes = Vec::with_capacity(phrase_len);
        let mut context = start_context;

        for _ in 0..phrase_len {
            let raw_pitch = self.sample_next(&context);

            // Constrain to scale mask: find nearest pitch in mask
            let pitch = if scale_mask.is_empty() {
                raw_pitch
            } else {
                *scale_mask
                    .iter()
                    .min_by_key(|&&p| {
                        let diff = if p >= raw_pitch {
                            p - raw_pitch
                        } else {
                            raw_pitch - p
                        };
                        diff
                    })
                    .unwrap_or(&raw_pitch)
            };

            // Velocity with slight LCG noise around 80
            let vel_noise = (self.lcg_next_float() * 20.0) as u8;
            let velocity = 70u8.saturating_add(vel_noise);

            // Duration chosen from fixed set
            let dur_idx = (self.lcg_next_float() * durations.len() as f64) as usize;
            let duration_ms = durations[dur_idx.min(durations.len() - 1)];

            notes.push(Note {
                pitch,
                duration_ms,
                velocity,
                is_rest: false,
            });

            // Advance context
            if context.len() >= self.config.order {
                context.remove(0);
            }
            context.push(pitch);
        }

        Phrase {
            notes,
            tempo_bpm: 120,
            time_signature: (4, 4),
        }
    }

    /// Generate a harmony phrase by transposing the melody by a given interval.
    pub fn generate_harmony(melody: &Phrase, interval: u8) -> Phrase {
        Self::transpose_internal(melody, interval as i8)
    }

    fn transpose_internal(phrase: &Phrase, semitones: i8) -> Phrase {
        let notes = phrase
            .notes
            .iter()
            .map(|n| {
                let pitch = if semitones >= 0 {
                    n.pitch.saturating_add(semitones as u8)
                } else {
                    n.pitch.saturating_sub((-semitones) as u8)
                }
                .min(127);
                Note {
                    pitch,
                    duration_ms: n.duration_ms,
                    velocity: n.velocity,
                    is_rest: n.is_rest,
                }
            })
            .collect();
        Phrase {
            notes,
            tempo_bpm: phrase.tempo_bpm,
            time_signature: phrase.time_signature,
        }
    }

    /// Transpose a phrase by semitones, clamping MIDI values to [0, 127].
    pub fn transpose(phrase: &Phrase, semitones: i8) -> Phrase {
        Self::transpose_internal(phrase, semitones)
    }
}

/// Compute statistics for a phrase.
pub fn phrase_stats(phrase: &Phrase) -> MelodyStats {
    let notes: Vec<&Note> = phrase.notes.iter().filter(|n| !n.is_rest).collect();
    let note_count = notes.len();

    if note_count == 0 {
        return MelodyStats {
            note_count: 0,
            unique_pitches: 0,
            avg_velocity: 0.0,
            pitch_range: 0,
            avg_duration_ms: 0.0,
        };
    }

    let unique_pitches: std::collections::HashSet<u8> = notes.iter().map(|n| n.pitch).collect();
    let avg_velocity = notes.iter().map(|n| n.velocity as f64).sum::<f64>() / note_count as f64;
    let min_pitch = notes.iter().map(|n| n.pitch).min().unwrap_or(0);
    let max_pitch = notes.iter().map(|n| n.pitch).max().unwrap_or(0);
    let pitch_range = max_pitch.saturating_sub(min_pitch);
    let avg_duration_ms =
        notes.iter().map(|n| n.duration_ms as f64).sum::<f64>() / note_count as f64;

    MelodyStats {
        note_count,
        unique_pitches: unique_pitches.len(),
        avg_velocity,
        pitch_range,
        avg_duration_ms,
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_scale() -> Vec<u8> {
        // C major scale across octaves
        (0..128u8)
            .filter(|&p| {
                let pc = p % 12;
                [0, 2, 4, 5, 7, 9, 11].contains(&pc)
            })
            .collect()
    }

    #[test]
    fn test_train_and_generate_stays_in_scale() {
        let scale = make_scale();
        let training: Vec<u8> = scale[..20].to_vec();
        let config = MarkovMelodyConfig {
            order: 2,
            temperature: 1.0,
            min_phrase_len: 8,
            max_phrase_len: 8,
            seed: 1234,
        };
        let mut markov = MarkovMelody::new(config);
        markov.train(&training);
        let context = vec![scale[0], scale[1]];
        let phrase = markov.generate_phrase(context, &scale);
        for note in &phrase.notes {
            assert!(
                scale.contains(&note.pitch),
                "Pitch {} not in scale",
                note.pitch
            );
        }
    }

    #[test]
    fn test_harmony_adds_correct_interval() {
        let notes = vec![
            Note { pitch: 60, duration_ms: 500, velocity: 80, is_rest: false },
            Note { pitch: 64, duration_ms: 500, velocity: 80, is_rest: false },
        ];
        let melody = Phrase { notes, tempo_bpm: 120, time_signature: (4, 4) };
        let harmony = MarkovMelody::generate_harmony(&melody, 7);
        assert_eq!(harmony.notes[0].pitch, 67);
        assert_eq!(harmony.notes[1].pitch, 71);
    }

    #[test]
    fn test_transpose_clamps_to_0_127() {
        let notes = vec![
            Note { pitch: 126, duration_ms: 500, velocity: 80, is_rest: false },
            Note { pitch: 1, duration_ms: 500, velocity: 80, is_rest: false },
        ];
        let phrase = Phrase { notes, tempo_bpm: 120, time_signature: (4, 4) };
        let up = MarkovMelody::transpose(&phrase, 10);
        assert_eq!(up.notes[0].pitch, 127); // saturating add
        let down = MarkovMelody::transpose(&phrase, -10);
        assert_eq!(down.notes[1].pitch, 0); // saturating sub
    }

    #[test]
    fn test_phrase_stats() {
        let notes = vec![
            Note { pitch: 60, duration_ms: 500, velocity: 80, is_rest: false },
            Note { pitch: 64, duration_ms: 250, velocity: 90, is_rest: false },
            Note { pitch: 67, duration_ms: 750, velocity: 70, is_rest: false },
        ];
        let phrase = Phrase { notes, tempo_bpm: 120, time_signature: (4, 4) };
        let stats = phrase_stats(&phrase);
        assert_eq!(stats.note_count, 3);
        assert_eq!(stats.unique_pitches, 3);
        assert_eq!(stats.pitch_range, 7); // 67-60
        assert!((stats.avg_velocity - 80.0).abs() < 1e-6);
        assert!((stats.avg_duration_ms - 500.0).abs() < 1e-6);
    }
}
