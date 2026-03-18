/// ADSR envelope generator (per-sample) with exponential curves.
///
/// **Why exponential?**
/// Real-world instrument envelopes follow RC-circuit physics: decay and release
/// are exponential (each sample is a fixed fraction of the remaining distance
/// to zero).  Linear ADSR — the original implementation — sounds mechanical
/// because no acoustic instrument has a straight-line decay.  Exponential
/// curves give the characteristic "fast start, gentle tail" that makes a note
/// feel like it was struck rather than switched.
///
/// Attack remains linear (convex ramp): most percussive and plucked instruments
/// have near-linear onsets; exponential attack can sound sluggish.
///
/// Decay and Release use first-order IIR smoothing toward their targets:
///   `level = level * coeff + target * (1 - coeff)`
/// where `coeff = exp(-ln(1000) / time_in_samples)` ensures the envelope
/// reaches within 0.1% of target in exactly the specified time.

#[derive(Clone, Copy, PartialEq, Default)]
enum Stage { #[default] Idle, Attack, Decay, Sustain, Release }

#[derive(Clone, Default)]
/// ADSR envelope generator with exponential decay and release curves.
///
/// Typical use:
/// 1. Call [`Adsr::trigger`] when a note starts.
/// 2. Call [`Adsr::next_sample`] once per audio sample to get the envelope level.
/// 3. Call [`Adsr::release`] when the note ends.
/// 4. Continue calling [`Adsr::next_sample`] until [`Adsr::is_idle`] returns `true`.
pub struct Adsr {
    stage: Stage,
    level: f32,
    // Linear attack
    attack_rate: f32,
    // Exponential decay/release coefficients and targets
    decay_coeff: f32,
    decay_target: f32,
    release_coeff: f32,
    sustain_level: f32,
    sample_rate: f32,
}

/// Compute the per-sample coefficient for a first-order IIR that reaches
/// within 0.1% of its target in `time_ms` milliseconds.
fn exp_coeff(time_ms: f32, sample_rate: f32) -> f32 {
    let samples = (time_ms * 0.001 * sample_rate).max(1.0);
    (-6.908 / samples).exp()  // ln(1000) ≈ 6.908
}

/// Helper: run the ADSR for `n` samples and return the last value.
#[cfg(test)]
fn run_for(env: &mut Adsr, n: usize) -> f32 {
    let mut v = 0.0;
    for _ in 0..n { v = env.next_sample(); }
    v
}

impl Adsr {
    /// Create a new ADSR with the given timing parameters.
    ///
    /// # Parameters
    /// - `attack_ms`: Linear attack time in milliseconds.
    /// - `decay_ms`: Exponential decay time to reach sustain level.
    /// - `sustain`: Sustain amplitude level (0..1).
    /// - `release_ms`: Exponential release time to reach silence.
    /// - `sample_rate`: Audio sample rate in Hz.
    pub fn new(attack_ms: f32, decay_ms: f32, sustain: f32, release_ms: f32, sample_rate: f32) -> Self {
        let sustain = sustain.clamp(0.0, 1.0);
        let mut s = Self {
            stage: Stage::Idle,
            level: 0.0,
            attack_rate: 1.0 / (attack_ms * 0.001 * sample_rate).max(1.0),
            decay_coeff: exp_coeff(decay_ms, sample_rate),
            decay_target: sustain,
            release_coeff: exp_coeff(release_ms, sample_rate),
            sustain_level: sustain,
            sample_rate,
        };
        s.decay_target = sustain;
        s
    }

    /// Update timing parameters without resetting the envelope stage.
    pub fn set_params(&mut self, attack_ms: f32, decay_ms: f32, sustain: f32, release_ms: f32) {
        let sr = self.sample_rate.max(1.0);
        let sustain = sustain.clamp(0.0, 1.0);
        self.attack_rate   = 1.0 / (attack_ms * 0.001 * sr).max(1.0);
        self.decay_coeff   = exp_coeff(decay_ms, sr);
        self.decay_target  = sustain;
        self.release_coeff = exp_coeff(release_ms, sr);
        self.sustain_level = sustain;
    }

    /// Begin the attack stage.
    pub fn trigger(&mut self) { self.stage = Stage::Attack; }
    /// Begin the release stage (no-op if already idle).
    pub fn release(&mut self) { if self.stage != Stage::Idle { self.stage = Stage::Release; } }
    /// Returns `true` when the envelope has fully released and is producing silence.
    pub fn is_idle(&self) -> bool { self.stage == Stage::Idle }
    /// Returns the current envelope level without advancing the state.
    pub fn level(&self) -> f32 { self.level }

    /// Advance the envelope by one sample and return the current level.
    pub fn next_sample(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {
                self.level = 0.0;
            }
            Stage::Attack => {
                // Linear ramp to 1.0
                self.level += self.attack_rate;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                // Exponential decay toward sustain
                self.level = self.level * self.decay_coeff
                    + self.decay_target * (1.0 - self.decay_coeff);
                if (self.level - self.sustain_level).abs() < 0.0001 {
                    self.level = self.sustain_level;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {
                // Hold at sustain level until release() is called
            }
            Stage::Release => {
                // Exponential decay toward zero
                self.level *= self.release_coeff;
                if self.level < 1e-6 { // −120 dBFS — no audible tail, no DC accumulation
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_adsr_at_t0_output_is_zero() {
        // Before triggering, the envelope should produce 0.0.
        let mut env = Adsr::new(10.0, 100.0, 0.7, 200.0, SR);
        let s = env.next_sample();
        assert!(s.abs() < 1e-10, "Idle envelope should produce 0, got {}", s);
    }

    #[test]
    fn test_adsr_attack_rises() {
        let attack_ms = 50.0_f32;
        let attack_samples = (attack_ms * 0.001 * SR) as usize;
        let mut env = Adsr::new(attack_ms, 200.0, 0.7, 200.0, SR);
        env.trigger();
        // After a few samples the level should be rising from 0.
        let early = run_for(&mut env, attack_samples / 4);
        assert!(early > 0.0 && early < 1.0, "Level should be rising during attack, got {}", early);
        let peak = run_for(&mut env, attack_samples);
        assert!(peak >= 0.9, "Level should reach near 1.0 after attack, got {}", peak);
    }

    #[test]
    fn test_adsr_decay_falls_to_sustain() {
        let sustain = 0.5_f32;
        let mut env = Adsr::new(1.0, 200.0, sustain, 500.0, SR);
        env.trigger();
        // Advance past attack (very short: 1 ms = 44 samples).
        run_for(&mut env, 100);
        // Now in decay; wait long enough to reach sustain.
        let after_decay = run_for(&mut env, 20000);
        assert!((after_decay - sustain).abs() < 0.01,
            "After decay level should be at sustain {}, got {}", sustain, after_decay);
    }

    #[test]
    fn test_adsr_sustain_is_constant() {
        let sustain = 0.6_f32;
        let mut env = Adsr::new(1.0, 50.0, sustain, 500.0, SR);
        env.trigger();
        // Fast-forward past attack + decay.
        run_for(&mut env, 5000);
        // Now the envelope should be in Sustain; level must be constant.
        let s1 = env.next_sample();
        let s2 = run_for(&mut env, 100);
        assert!((s1 - s2).abs() < 0.005,
            "Sustain level should be constant: {} vs {}", s1, s2);
    }

    #[test]
    fn test_adsr_release_falls_to_zero() {
        let mut env = Adsr::new(1.0, 50.0, 0.7, 200.0, SR);
        env.trigger();
        // Skip to sustain stage.
        run_for(&mut env, 5000);
        env.release();
        // After enough release samples, the level must reach 0.
        let after_release = run_for(&mut env, 20000);
        assert!(after_release.abs() < 1e-5,
            "Level should reach 0 after release, got {}", after_release);
        assert!(env.is_idle(), "Envelope should be idle after release completes");
    }

    #[test]
    fn test_adsr_zero_duration_stages_do_not_panic() {
        // Zero-duration attack, decay, and release should not cause division by zero or panic.
        let mut env = Adsr::new(0.0, 0.0, 0.5, 0.0, SR);
        env.trigger();
        for _ in 0..1000 { env.next_sample(); }
        env.release();
        for _ in 0..1000 { env.next_sample(); }
        // Just check that we did not panic and the output is finite.
        assert!(env.level().is_finite());
    }
}
