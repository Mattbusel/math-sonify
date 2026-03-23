//! Interlocking rhythms, Euclidean rhythms, and metric modulation.

/// Greatest common divisor (Euclidean algorithm).
pub fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Least common multiple.
pub fn lcm(a: u32, b: u32) -> u32 {
    if a == 0 || b == 0 {
        0
    } else {
        a / gcd(a, b) * b
    }
}

/// Bjorklund / Euclidean rhythm: distribute `pulses` onsets as evenly as
/// possible across `steps` slots.
///
/// Uses the Bresenham-inspired rotation algorithm:
/// - Start with `pulses` groups of [true] and `(steps - pulses)` groups of [false].
/// - Repeatedly fold the shorter list into the longer until one list has length ≤ 1.
pub fn euclidean_rhythm(pulses: u32, steps: u32) -> Vec<bool> {
    if steps == 0 {
        return vec![];
    }
    if pulses == 0 {
        return vec![false; steps as usize];
    }
    let pulses = pulses.min(steps);
    // Start with `pulses` groups of [true] and `remainder` groups of [false]
    let mut ones: Vec<Vec<bool>> = (0..pulses).map(|_| vec![true]).collect();
    let mut zeros: Vec<Vec<bool>> = (0..(steps - pulses)).map(|_| vec![false]).collect();

    loop {
        if zeros.len() <= 1 {
            break;
        }
        // Zip the shorter into the longer
        let (shorter, longer) = if ones.len() <= zeros.len() {
            (&mut ones, &mut zeros)
        } else {
            (&mut zeros, &mut ones)
        };
        let take = shorter.len().min(longer.len());
        let drain: Vec<Vec<bool>> = longer.drain(longer.len() - take..).collect();
        for (s, d) in shorter.iter_mut().zip(drain.into_iter()) {
            s.extend(d);
        }
        if ones.len() == zeros.len() {
            break;
        }
    }

    ones.into_iter().chain(zeros).flatten().collect()
}

/// One layer in a polyrhythm.
pub struct PolyrhythmLayer {
    /// Human-readable name.
    pub name: String,
    /// Boolean pulse pattern (length = number of steps in one cycle).
    pub pulse_pattern: Vec<bool>,
    /// Tempo for this layer in beats per minute.
    pub bpm: f64,
    /// Swing factor (0.0 = none, 1.0 = full swing).
    pub swing: f64,
    /// How many steps this layer is offset from the downbeat.
    pub offset_steps: u32,
}

impl PolyrhythmLayer {
    /// Compute the timestamp (in seconds) of each true beat for `n_bars` repetitions.
    pub fn beat_times_sec(&self, n_bars: u32) -> Vec<f64> {
        if self.pulse_pattern.is_empty() || self.bpm <= 0.0 {
            return vec![];
        }
        let steps = self.pulse_pattern.len() as u32;
        let step_dur = 60.0 / self.bpm; // seconds per step
        let total_steps = steps * n_bars;
        let offset = self.offset_steps as usize % self.pulse_pattern.len().max(1);
        let mut times = Vec::new();
        for i in 0..total_steps as usize {
            let pat_idx = (i + offset) % self.pulse_pattern.len();
            if self.pulse_pattern[pat_idx] {
                let mut t = i as f64 * step_dur;
                // Apply swing on even steps: lengthen odd subdivisions
                if self.swing > 0.0 && (i % 2 == 1) {
                    t += step_dur * self.swing * 0.5;
                }
                times.push(t);
            }
        }
        times
    }
}

/// Engine combining multiple polyrhythm layers.
pub struct PolyrhythmEngine {
    /// Individual rhythm layers.
    pub layers: Vec<PolyrhythmLayer>,
    /// Global master tempo in BPM.
    pub master_bpm: f64,
}

impl PolyrhythmEngine {
    /// Create a new engine with the given master tempo.
    pub fn new(master_bpm: f64) -> Self {
        Self {
            layers: Vec::new(),
            master_bpm,
        }
    }

    /// Add a layer to the engine.
    pub fn add_layer(&mut self, layer: PolyrhythmLayer) {
        self.layers.push(layer);
    }

    /// LCM of all layer step counts. 1 if no layers.
    pub fn period_steps(&self) -> u32 {
        if self.layers.is_empty() {
            return 1;
        }
        self.layers
            .iter()
            .map(|l| l.pulse_pattern.len() as u32)
            .fold(1u32, lcm)
    }

    /// `[step][layer]` grid of beats for `n_bars` of the period.
    pub fn collision_grid(&self, n_bars: u32) -> Vec<Vec<bool>> {
        if self.layers.is_empty() {
            return vec![];
        }
        let period = self.period_steps() as usize;
        let total_steps = period * n_bars as usize;
        let mut grid: Vec<Vec<bool>> = vec![vec![false; self.layers.len()]; total_steps];
        for (li, layer) in self.layers.iter().enumerate() {
            let pat_len = layer.pulse_pattern.len();
            if pat_len == 0 {
                continue;
            }
            let offset = layer.offset_steps as usize % pat_len;
            for step in 0..total_steps {
                let pat_idx = (step + offset) % pat_len;
                grid[step][li] = layer.pulse_pattern[pat_idx];
            }
        }
        grid
    }

    /// Return timestamps (seconds) where 2 or more layers coincide.
    pub fn coincidence_points(&self, n_bars: u32) -> Vec<f64> {
        if self.master_bpm <= 0.0 {
            return vec![];
        }
        let grid = self.collision_grid(n_bars);
        let step_dur = 60.0 / self.master_bpm;
        grid.iter()
            .enumerate()
            .filter(|(_, beats)| beats.iter().filter(|&&b| b).count() >= 2)
            .map(|(i, _)| i as f64 * step_dur)
            .collect()
    }

    /// Render an ASCII grid: one line per layer, 'X' = beat, '.' = rest.
    pub fn render_ascii(&self) -> String {
        if self.layers.is_empty() {
            return String::new();
        }
        let period = self.period_steps() as usize;
        let mut out = String::new();
        for layer in &self.layers {
            let pat_len = layer.pulse_pattern.len();
            let label = format!("{:<12} |", layer.name);
            out.push_str(&label);
            for step in 0..period {
                let pat_idx = if pat_len > 0 { step % pat_len } else { 0 };
                let ch = if pat_len > 0 && layer.pulse_pattern[pat_idx] {
                    'X'
                } else {
                    '.'
                };
                out.push(ch);
            }
            out.push('\n');
        }
        out
    }
}

/// Type of metric modulation.
pub enum ModulationType {
    /// Instant tempo change.
    Abrupt,
    /// Linearly interpolated tempo change.
    GradualLinear,
    /// Pivot on a shared note value.
    PivotNote,
}

impl ModulationType {
    /// Return the effective BPM at `beat` (0-based) within the modulation window.
    pub fn new_bpm_at_beat(&self, beat: u32, mm: &MetricModulation) -> f64 {
        let total = mm.duration_beats.max(1) as f64;
        let t = (beat as f64 / total).clamp(0.0, 1.0);
        match self {
            ModulationType::Abrupt => {
                if beat == 0 {
                    mm.from_bpm
                } else {
                    mm.to_bpm
                }
            }
            ModulationType::GradualLinear => mm.from_bpm + (mm.to_bpm - mm.from_bpm) * t,
            ModulationType::PivotNote => {
                // Pivot: stays at from_bpm until midpoint, then snaps to to_bpm
                if t < 0.5 {
                    mm.from_bpm
                } else {
                    mm.to_bpm
                }
            }
        }
    }
}

/// A metric modulation: transition from one tempo to another over a span.
pub struct MetricModulation {
    /// Starting tempo.
    pub from_bpm: f64,
    /// Target tempo.
    pub to_bpm: f64,
    /// Duration of the transition in beats.
    pub duration_beats: u32,
    /// Modulation curve type.
    pub modulation_type: ModulationType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euclidean_examples() {
        // E(3,8) = [1,0,0,1,0,0,1,0]
        let r = euclidean_rhythm(3, 8);
        assert_eq!(r.len(), 8);
        assert_eq!(r.iter().filter(|&&b| b).count(), 3);

        // E(4,4) = all beats
        let r = euclidean_rhythm(4, 4);
        assert!(r.iter().all(|&b| b));

        // E(0,4) = silence
        let r = euclidean_rhythm(0, 4);
        assert!(r.iter().all(|&b| !b));
    }

    #[test]
    fn lcm_gcd() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(lcm(3, 4), 12);
        assert_eq!(lcm(6, 4), 12);
    }

    #[test]
    fn engine_period_and_coincidence() {
        let mut engine = PolyrhythmEngine::new(120.0);
        engine.add_layer(PolyrhythmLayer {
            name: "3-beat".to_string(),
            pulse_pattern: euclidean_rhythm(3, 3),
            bpm: 120.0,
            swing: 0.0,
            offset_steps: 0,
        });
        engine.add_layer(PolyrhythmLayer {
            name: "4-beat".to_string(),
            pulse_pattern: euclidean_rhythm(4, 4),
            bpm: 120.0,
            swing: 0.0,
            offset_steps: 0,
        });
        assert_eq!(engine.period_steps(), 12); // lcm(3,4)
        let pts = engine.coincidence_points(1);
        assert!(!pts.is_empty()); // they coincide at step 0 at least
    }

    #[test]
    fn metric_modulation_gradual() {
        let mm = MetricModulation {
            from_bpm: 100.0,
            to_bpm: 200.0,
            duration_beats: 10,
            modulation_type: ModulationType::GradualLinear,
        };
        let mid = mm.modulation_type.new_bpm_at_beat(5, &mm);
        assert!((mid - 150.0).abs() < 1.0);
    }
}
