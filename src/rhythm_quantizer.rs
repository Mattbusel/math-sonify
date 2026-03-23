//! Rhythm quantization to a musical grid with swing and humanization.
//!
//! Provides snapping of arbitrary event times to a configurable beat grid,
//! swing feel adjustment, and deterministic micro-timing humanization via a
//! linear-congruential generator.

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Duration of one beat in milliseconds at the given BPM.
pub fn beat_duration_ms(bpm: f64) -> f64 {
    60_000.0 / bpm
}

// ---------------------------------------------------------------------------
// QuantizeGrid
// ---------------------------------------------------------------------------

/// Defines the rhythmic grid used for quantization.
///
/// `swing_ratio` controls the amount of swing applied to off-beat subdivisions:
/// * `0.5` = perfectly straight (no swing)
/// * `0.67` ≈ triplet swing (2:1 ratio)
#[derive(Debug, Clone)]
pub struct QuantizeGrid {
    pub bpm: f64,
    /// Number of subdivisions per beat (e.g. 4 = 16th notes at 4/4).
    pub subdivisions: u32,
    /// Swing ratio in [0.5, 1.0); 0.5 is straight.
    pub swing_ratio: f64,
}

impl QuantizeGrid {
    pub fn new(bpm: f64, subdivisions: u32, swing_ratio: f64) -> Self {
        let swing_ratio = swing_ratio.clamp(0.5, 0.99);
        Self { bpm, subdivisions, swing_ratio }
    }

    /// Duration of one subdivision in milliseconds (straight, no swing).
    pub fn subdivision_ms(&self) -> f64 {
        beat_duration_ms(self.bpm) / self.subdivisions as f64
    }

    /// Whether `subdivision_index` (0-based within a beat) is an "off-beat"
    /// subdivision that receives swing delay.
    fn is_offbeat(subdivision_index: u64) -> bool {
        subdivision_index % 2 == 1
    }
}

// ---------------------------------------------------------------------------
// Quantize
// ---------------------------------------------------------------------------

/// Snap `time_ms` to the nearest grid position (straight, no swing).
pub fn quantize_event(time_ms: f64, grid: &QuantizeGrid) -> f64 {
    let sub = grid.subdivision_ms();
    if sub <= 0.0 {
        return time_ms;
    }
    let grid_index = (time_ms / sub).round() as i64;
    grid_index as f64 * sub
}

/// Apply swing feel to an already-quantized time.
///
/// Off-beat subdivisions are delayed so that the duration of an on-beat
/// subdivision becomes `swing_ratio * beat_duration` and the off-beat becomes
/// `(1 - swing_ratio) * beat_duration`.
pub fn swing_adjust(time_ms: f64, grid: &QuantizeGrid) -> f64 {
    let sub = grid.subdivision_ms();
    if sub <= 0.0 || grid.subdivisions < 2 {
        return time_ms;
    }

    let beat_ms = beat_duration_ms(grid.bpm);
    // Determine which subdivision index this time falls on.
    let total_sub_index = (time_ms / sub).round() as u64;
    let beat_index = total_sub_index / grid.subdivisions as u64;
    let sub_within_beat = total_sub_index % grid.subdivisions as u64;

    if QuantizeGrid::is_offbeat(sub_within_beat) {
        // Straight position of this subdivision.
        let straight_pos = beat_index as f64 * beat_ms + sub_within_beat as f64 * sub;
        // Swing position: on-beat sub gets swing_ratio * beat_ms, off-beat gets the rest.
        let on_beat_dur = grid.swing_ratio * beat_ms / (grid.subdivisions as f64 / 2.0);
        let swing_pos = beat_index as f64 * beat_ms
            + ((sub_within_beat / 2) as f64) * beat_ms / (grid.subdivisions as f64 / 2.0)
            + on_beat_dur;
        let _ = straight_pos; // already encoded in the calculation above
        swing_pos
    } else {
        time_ms
    }
}

// ---------------------------------------------------------------------------
// RhythmPattern
// ---------------------------------------------------------------------------

/// A sequence of event onset times in milliseconds.
#[derive(Debug, Clone, PartialEq)]
pub struct RhythmPattern {
    /// Event times in milliseconds, in chronological order.
    pub events: Vec<f64>,
}

impl RhythmPattern {
    pub fn new(events: Vec<f64>) -> Self {
        Self { events }
    }

    /// Quantize all events to the grid.
    pub fn quantize(&self, grid: &QuantizeGrid) -> Self {
        Self::new(self.events.iter().map(|&t| quantize_event(t, grid)).collect())
    }

    /// Apply swing to all events (assumes they are already quantized).
    pub fn apply_swing(&self, grid: &QuantizeGrid) -> Self {
        Self::new(self.events.iter().map(|&t| swing_adjust(t, grid)).collect())
    }

    /// Total duration from first to last event.
    pub fn duration_ms(&self) -> f64 {
        match (self.events.first(), self.events.last()) {
            (Some(&first), Some(&last)) => last - first,
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Humanize (LCG noise)
// ---------------------------------------------------------------------------

/// A minimal linear-congruential generator for deterministic pseudo-random noise.
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(1) }
    }

    /// Return the next value in [-1.0, 1.0].
    fn next_f64(&mut self) -> f64 {
        // Knuth LCG constants.
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // Map to [-1, 1].
        let bits = (self.state >> 11) as f64;
        bits / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/// Add deterministic micro-timing variation to each event in `pattern`.
///
/// Each event is shifted by a pseudo-random amount in `[-amount_ms, +amount_ms]`
/// derived from the given `seed` via an LCG.
pub fn humanize(pattern: &RhythmPattern, amount_ms: f64, seed: u64) -> RhythmPattern {
    let mut lcg = Lcg::new(seed);
    let events = pattern.events.iter().map(|&t| t + lcg.next_f64() * amount_ms).collect();
    RhythmPattern::new(events)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const BPM: f64 = 120.0;

    #[test]
    fn beat_duration_120bpm() {
        // At 120 BPM one beat = 500 ms.
        assert!((beat_duration_ms(120.0) - 500.0).abs() < 1e-9);
    }

    #[test]
    fn beat_duration_60bpm() {
        assert!((beat_duration_ms(60.0) - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn subdivision_ms_16th_at_120bpm() {
        let grid = QuantizeGrid::new(BPM, 4, 0.5);
        // 16th note at 120 BPM = 125 ms.
        assert!((grid.subdivision_ms() - 125.0).abs() < 1e-9);
    }

    #[test]
    fn quantize_event_snaps_to_nearest_grid() {
        let grid = QuantizeGrid::new(BPM, 4, 0.5); // 125 ms grid
        // 130 ms is closer to 125 than to 250.
        let q = quantize_event(130.0, &grid);
        assert!((q - 125.0).abs() < 1e-9);
    }

    #[test]
    fn quantize_event_on_grid_unchanged() {
        let grid = QuantizeGrid::new(BPM, 4, 0.5);
        let q = quantize_event(500.0, &grid);
        assert!((q - 500.0).abs() < 1e-9);
    }

    #[test]
    fn quantize_event_rounds_up() {
        let grid = QuantizeGrid::new(BPM, 4, 0.5); // 125 ms grid
        // 190 ms is closer to 250 than to 125.
        let q = quantize_event(190.0, &grid);
        assert!((q - 250.0).abs() < 1e-9);
    }

    #[test]
    fn swing_adjust_on_beat_unchanged() {
        let grid = QuantizeGrid::new(BPM, 2, 0.67);
        // The 0th subdivision in each beat is on-beat.
        let on_beat = 0.0;
        let adj = swing_adjust(on_beat, &grid);
        assert!((adj - on_beat).abs() < 1e-9);
    }

    #[test]
    fn swing_adjust_offbeat_delayed() {
        let grid = QuantizeGrid::new(BPM, 2, 0.67);
        // Sub 1 (250 ms at 120 BPM) is the off-beat of the first beat.
        let off_beat_straight = 250.0;
        let adj = swing_adjust(off_beat_straight, &grid);
        // With swing 0.67 the off-beat should be pushed later.
        assert!(adj >= off_beat_straight);
    }

    #[test]
    fn pattern_quantize() {
        let grid = QuantizeGrid::new(BPM, 4, 0.5);
        let pat = RhythmPattern::new(vec![0.0, 127.0, 253.0, 380.0]);
        let q = pat.quantize(&grid);
        assert!((q.events[0] - 0.0).abs() < 1e-9);
        assert!((q.events[1] - 125.0).abs() < 1e-9);
        assert!((q.events[2] - 250.0).abs() < 1e-9);
        assert!((q.events[3] - 375.0).abs() < 1e-9);
    }

    #[test]
    fn pattern_duration() {
        let pat = RhythmPattern::new(vec![0.0, 100.0, 500.0]);
        assert!((pat.duration_ms() - 500.0).abs() < 1e-9);
    }

    #[test]
    fn pattern_duration_empty() {
        let pat = RhythmPattern::new(vec![]);
        assert!((pat.duration_ms() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn humanize_deterministic() {
        let pat = RhythmPattern::new(vec![0.0, 500.0, 1000.0]);
        let h1 = humanize(&pat, 10.0, 42);
        let h2 = humanize(&pat, 10.0, 42);
        assert_eq!(h1, h2);
    }

    #[test]
    fn humanize_different_seeds_differ() {
        let pat = RhythmPattern::new(vec![0.0, 500.0, 1000.0]);
        let h1 = humanize(&pat, 10.0, 1);
        let h2 = humanize(&pat, 10.0, 2);
        assert_ne!(h1, h2);
    }

    #[test]
    fn humanize_stays_within_amount() {
        let pat = RhythmPattern::new(vec![1000.0]);
        let amount = 15.0;
        for seed in 0..50u64 {
            let h = humanize(&pat, amount, seed);
            let delta = (h.events[0] - 1000.0).abs();
            assert!(delta <= amount + 1e-9, "delta={} amount={}", delta, amount);
        }
    }

    #[test]
    fn lcg_produces_varied_output() {
        let mut lcg = Lcg::new(99);
        let vals: Vec<f64> = (0..10).map(|_| lcg.next_f64()).collect();
        // Not all the same.
        assert!(vals.windows(2).any(|w| (w[0] - w[1]).abs() > 1e-6));
    }
}
