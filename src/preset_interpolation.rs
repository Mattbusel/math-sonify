// Preset interpolation and morphing module.
//
// Provides linear interpolation between two named presets and a scheduled
// morph timeline that steps through a sequence of (preset_name, duration_ms)
// pairs in real time.
#![allow(dead_code)]

use std::time::{Duration, Instant};

use crate::config::Config;
use crate::patches::load_preset;

// ---------------------------------------------------------------------------
// Interpolation state
// ---------------------------------------------------------------------------

/// Current interpolation position between a source and target preset.
#[derive(Debug, Clone, PartialEq)]
pub struct MorphState {
    /// Interpolation position in [0.0, 1.0].
    /// 0.0 = fully at source; 1.0 = fully at target.
    pub t: f64,
    /// Name of the source preset.
    pub source_name: String,
    /// Name of the target preset.
    pub target_name: String,
}

impl MorphState {
    /// Create a `MorphState` positioned at the source (t = 0).
    pub fn new(source_name: impl Into<String>, target_name: impl Into<String>) -> Self {
        Self {
            t: 0.0,
            source_name: source_name.into(),
            target_name: target_name.into(),
        }
    }

    /// Is the morph complete?
    pub fn is_complete(&self) -> bool {
        self.t >= 1.0
    }
}

// ---------------------------------------------------------------------------
// PresetInterpolator
// ---------------------------------------------------------------------------

/// Interpolates between two [`Config`] instances (loaded from named presets).
pub struct PresetInterpolator {
    source: Config,
    target: Config,
}

impl PresetInterpolator {
    /// Create an interpolator between two named presets.
    pub fn from_names(source_name: &str, target_name: &str) -> Self {
        Self {
            source: load_preset(source_name),
            target: load_preset(target_name),
        }
    }

    /// Create an interpolator from two existing configs.
    pub fn from_configs(source: Config, target: Config) -> Self {
        Self { source, target }
    }

    /// Linearly interpolate at position `t` in [0.0, 1.0].
    ///
    /// All numeric fields are linearly blended.  String fields (system name,
    /// mode, scale, chord_mode) switch at `t = 0.5`.
    pub fn interpolate(&self, t: f64) -> Config {
        interpolate(&self.source, &self.target, t)
    }
}

/// Free-standing interpolation function.
///
/// Numeric fields are linearly blended; string fields switch at `t = 0.5`.
pub fn interpolate(a: &Config, b: &Config, t: f64) -> Config {
    use crate::arrangement::lerp_config;
    lerp_config(a, b, t as f32)
}

// ---------------------------------------------------------------------------
// Morph timeline
// ---------------------------------------------------------------------------

/// A single entry in a morph schedule.
#[derive(Debug, Clone)]
pub struct MorphStep {
    /// Name of the preset to morph *to*.
    pub preset_name: String,
    /// How long to spend morphing into this preset (milliseconds).
    pub duration_ms: u64,
}

impl MorphStep {
    pub fn new(preset_name: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            preset_name: preset_name.into(),
            duration_ms,
        }
    }
}

/// A scheduled sequence of preset morphs.
///
/// Call [`MorphTimeline::tick`] each frame with the elapsed duration to get
/// the current interpolated [`Config`].
#[derive(Debug, Clone)]
pub struct PresetMorphSchedule {
    /// Ordered list of morph steps.
    pub steps: Vec<MorphStep>,
    /// Loop the schedule when all steps are finished.
    pub looping: bool,
}

impl PresetMorphSchedule {
    /// Build a schedule from (preset_name, duration_ms) pairs.
    pub fn from_pairs(pairs: &[(&str, u64)]) -> Self {
        Self {
            steps: pairs
                .iter()
                .map(|(n, d)| MorphStep::new(*n, *d))
                .collect(),
            looping: false,
        }
    }

    /// Total duration of one pass through the schedule.
    pub fn total_duration_ms(&self) -> u64 {
        self.steps.iter().map(|s| s.duration_ms).sum()
    }
}

/// Stateful player for a [`PresetMorphSchedule`].
pub struct MorphTimeline {
    schedule: PresetMorphSchedule,
    /// Index of the current step being played.
    current_step: usize,
    /// Config at the start of the current step (the "from" preset).
    step_source: Config,
    /// Config at the end of the current step (the "to" preset).
    step_target: Config,
    /// When this step began.
    step_start: Instant,
    /// Whether playback has finished (only meaningful when !looping).
    finished: bool,
}

impl MorphTimeline {
    /// Create a player for the given schedule, starting immediately.
    pub fn new(schedule: PresetMorphSchedule) -> Self {
        if schedule.steps.is_empty() {
            // Degenerate case: nothing to play.
            return Self {
                schedule,
                current_step: 0,
                step_source: Config::default(),
                step_target: Config::default(),
                step_start: Instant::now(),
                finished: true,
            };
        }
        let first = &schedule.steps[0];
        let step_source = Config::default();
        let step_target = load_preset(&first.preset_name);
        Self {
            schedule,
            current_step: 0,
            step_source,
            step_target,
            step_start: Instant::now(),
            finished: false,
        }
    }

    /// Advance the timeline and return the current interpolated config.
    ///
    /// Call this every UI frame (or at whatever rate you need).
    pub fn tick(&mut self) -> Config {
        if self.finished || self.schedule.steps.is_empty() {
            return self.step_target.clone();
        }

        let step = &self.schedule.steps[self.current_step];
        let elapsed = self.step_start.elapsed();
        let duration = Duration::from_millis(step.duration_ms.max(1));
        let t = (elapsed.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0);

        if t >= 1.0 {
            // Advance to next step.
            let next = self.current_step + 1;
            if next >= self.schedule.steps.len() {
                if self.schedule.looping {
                    self.current_step = 0;
                    self.step_source = self.step_target.clone();
                    let first_name = &self.schedule.steps[0].preset_name.clone();
                    self.step_target = load_preset(first_name);
                    self.step_start = Instant::now();
                } else {
                    self.finished = true;
                    return self.step_target.clone();
                }
            } else {
                self.current_step = next;
                self.step_source = self.step_target.clone();
                let next_name = self.schedule.steps[next].preset_name.clone();
                self.step_target = load_preset(&next_name);
                self.step_start = Instant::now();
            }
        }

        interpolate(&self.step_source, &self.step_target, t)
    }

    /// Is the timeline finished (only meaningful when not looping)?
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Current step index (0-based).
    pub fn current_step(&self) -> usize {
        self.current_step
    }

    /// Current interpolation position in [0.0, 1.0] within the active step.
    pub fn step_t(&self) -> f64 {
        if self.finished || self.schedule.steps.is_empty() {
            return 1.0;
        }
        let step = &self.schedule.steps[self.current_step];
        let elapsed = self.step_start.elapsed();
        let duration = Duration::from_millis(step.duration_ms.max(1));
        (elapsed.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0)
    }

    /// Reset the timeline to the beginning.
    pub fn reset(&mut self) {
        self.current_step = 0;
        self.finished = false;
        self.step_start = Instant::now();
        if let Some(first) = self.schedule.steps.first() {
            self.step_target = load_preset(&first.preset_name.clone());
            self.step_source = Config::default();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_at_zero_returns_source() {
        let a = load_preset("Lorenz Ambience");
        let b = load_preset("Rössler Drift");
        let result = interpolate(&a, &b, 0.0);
        assert_eq!(result.system.name, a.system.name);
    }

    #[test]
    fn test_interpolate_at_one_returns_target() {
        let a = load_preset("Lorenz Ambience");
        let b = load_preset("Rössler Drift");
        let result = interpolate(&a, &b, 1.0);
        assert_eq!(result.system.name, b.system.name);
    }

    #[test]
    fn test_interpolate_numeric_midpoint() {
        let mut a = Config::default();
        let mut b = Config::default();
        a.audio.reverb_wet = 0.0;
        b.audio.reverb_wet = 1.0;
        let mid = interpolate(&a, &b, 0.5);
        let expected = 0.5_f32;
        assert!(
            (mid.audio.reverb_wet - expected).abs() < 1e-5,
            "expected reverb_wet ≈ 0.5, got {}",
            mid.audio.reverb_wet
        );
    }

    #[test]
    fn test_morph_state_new() {
        let ms = MorphState::new("A", "B");
        assert_eq!(ms.source_name, "A");
        assert_eq!(ms.target_name, "B");
        assert!(!ms.is_complete());
    }

    #[test]
    fn test_morph_state_complete_at_one() {
        let ms = MorphState { t: 1.0, source_name: "A".into(), target_name: "B".into() };
        assert!(ms.is_complete());
    }

    #[test]
    fn test_preset_interpolator_from_names() {
        let interp = PresetInterpolator::from_names("Lorenz Ambience", "FM Chaos");
        let cfg_0 = interp.interpolate(0.0);
        let cfg_1 = interp.interpolate(1.0);
        assert_eq!(cfg_0.system.name, "lorenz");
        assert_eq!(cfg_1.system.name, "lorenz"); // both use lorenz
    }

    #[test]
    fn test_morph_schedule_total_duration() {
        let sched = PresetMorphSchedule::from_pairs(&[
            ("Lorenz Ambience", 1000),
            ("FM Chaos", 2000),
        ]);
        assert_eq!(sched.total_duration_ms(), 3000);
    }

    #[test]
    fn test_morph_timeline_empty_schedule_finishes_immediately() {
        let sched = PresetMorphSchedule { steps: vec![], looping: false };
        let mut tl = MorphTimeline::new(sched);
        assert!(tl.is_finished());
        let _ = tl.tick(); // must not panic
    }

    #[test]
    fn test_morph_timeline_tick_returns_config() {
        let sched = PresetMorphSchedule::from_pairs(&[
            ("Lorenz Ambience", 100_000),
        ]);
        let mut tl = MorphTimeline::new(sched);
        let cfg = tl.tick();
        assert!(!cfg.system.name.is_empty());
    }
}
