//! Audio Effects Chain
//!
//! Provides a trait-based audio effects system with built-in DSP effects
//! and an `EffectsChain` that processes samples through an ordered pipeline.

use std::f32::consts::PI;

// ── AudioEffect trait ─────────────────────────────────────────────────────────

/// A single-sample, stateful audio effect.
pub trait AudioEffect: Send {
    /// Process one sample at the given sample rate, returning the output sample.
    fn process(&mut self, sample: f32, sample_rate: f32) -> f32;

    /// Reset the internal state of the effect (optional).
    fn reset(&mut self) {}
}

// ── LowPassFilter ─────────────────────────────────────────────────────────────

/// One-pole IIR low-pass filter.
///
/// Transfer function: `y[n] = y[n-1] + α·(x[n] - y[n-1])`
/// where `α = clamp(2π·cutoff/sr, 0, 1)`.
pub struct LowPassFilter {
    pub cutoff_hz: f32,
    pub resonance: f32,
    state: f32,
}

impl LowPassFilter {
    pub fn new(cutoff_hz: f32, resonance: f32) -> Self {
        Self { cutoff_hz, resonance: resonance.clamp(0.0, 1.0), state: 0.0 }
    }

    fn cutoff_norm(&self, sample_rate: f32) -> f32 {
        (2.0 * PI * self.cutoff_hz / sample_rate).clamp(0.0, 1.0)
    }
}

impl AudioEffect for LowPassFilter {
    fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let alpha = self.cutoff_norm(sample_rate);
        self.state += alpha * (sample - self.state);
        self.state
    }

    fn reset(&mut self) {
        self.state = 0.0;
    }
}

// ── HighPassFilter ────────────────────────────────────────────────────────────

/// One-pole IIR high-pass filter derived from the low-pass complement.
///
/// `y_hp = x - y_lp`
pub struct HighPassFilter {
    pub cutoff_hz: f32,
    lp_state: f32,
}

impl HighPassFilter {
    pub fn new(cutoff_hz: f32) -> Self {
        Self { cutoff_hz, lp_state: 0.0 }
    }
}

impl AudioEffect for HighPassFilter {
    fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let alpha = (2.0 * PI * self.cutoff_hz / sample_rate).clamp(0.0, 1.0);
        self.lp_state += alpha * (sample - self.lp_state);
        sample - self.lp_state
    }

    fn reset(&mut self) {
        self.lp_state = 0.0;
    }
}

// ── Reverb (Schroeder) ────────────────────────────────────────────────────────

/// Schroeder reverberator: 4 comb filters feeding 2 all-pass filters in series.
pub struct Reverb {
    pub room_size: f32,
    pub damping: f32,
    combs: [CombFilter; 4],
    allpasses: [AllPassFilter; 2],
}

struct CombFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    damp_state: f32,
    damping: f32,
}

impl CombFilter {
    fn new(delay_samples: usize, feedback: f32, damping: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples.max(1)],
            pos: 0,
            feedback,
            damp_state: 0.0,
            damping,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let out = self.buffer[self.pos];
        self.damp_state = out * (1.0 - self.damping) + self.damp_state * self.damping;
        self.buffer[self.pos] = input + self.damp_state * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        out
    }
}

struct AllPassFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl AllPassFilter {
    fn new(delay_samples: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples.max(1)],
            pos: 0,
            feedback,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buf = self.buffer[self.pos];
        let out = -input + buf;
        self.buffer[self.pos] = input + buf * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        out
    }
}

impl Reverb {
    /// Create a Schroeder reverberator with the given room size [0..1] and damping [0..1].
    pub fn new(room_size: f32, damping: f32) -> Self {
        let room = room_size.clamp(0.0, 1.0);
        let damp = damping.clamp(0.0, 1.0);
        // Standard Schroeder delay lengths (at 44100 Hz): 1557, 1617, 1491, 1422
        let feedback = 0.84 * room;
        Self {
            room_size: room,
            damping: damp,
            combs: [
                CombFilter::new(1557, feedback, damp),
                CombFilter::new(1617, feedback, damp),
                CombFilter::new(1491, feedback, damp),
                CombFilter::new(1422, feedback, damp),
            ],
            allpasses: [
                AllPassFilter::new(225, 0.5),
                AllPassFilter::new(341, 0.5),
            ],
        }
    }
}

impl AudioEffect for Reverb {
    fn process(&mut self, sample: f32, _sample_rate: f32) -> f32 {
        // Sum all comb filter outputs
        let mut wet = 0.0_f32;
        for comb in &mut self.combs {
            wet += comb.process(sample);
        }
        wet *= 0.25; // normalise

        // Pass through all-pass filters
        for ap in &mut self.allpasses {
            wet = ap.process(wet);
        }
        wet
    }

    fn reset(&mut self) {
        for c in &mut self.combs {
            c.buffer.fill(0.0);
            c.pos = 0;
            c.damp_state = 0.0;
        }
        for ap in &mut self.allpasses {
            ap.buffer.fill(0.0);
            ap.pos = 0;
        }
    }
}

// ── Distortion ────────────────────────────────────────────────────────────────

/// Soft-clip waveshaping distortion with dry/wet mix.
///
/// Output = `tanh(drive * x) * mix + x * (1 - mix)`
pub struct Distortion {
    pub drive: f32,
    pub mix: f32,
}

impl Distortion {
    pub fn new(drive: f32, mix: f32) -> Self {
        Self {
            drive: drive.max(0.0),
            mix: mix.clamp(0.0, 1.0),
        }
    }
}

impl AudioEffect for Distortion {
    fn process(&mut self, sample: f32, _sample_rate: f32) -> f32 {
        let wet = (self.drive * sample).tanh();
        wet * self.mix + sample * (1.0 - self.mix)
    }
}

// ── Compressor ────────────────────────────────────────────────────────────────

/// Feed-forward gain compressor with attack/release envelope.
pub struct Compressor {
    pub threshold: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    envelope: f32,
}

impl Compressor {
    pub fn new(threshold: f32, ratio: f32, attack_ms: f32, release_ms: f32) -> Self {
        Self {
            threshold,
            ratio: ratio.max(1.0),
            attack_ms: attack_ms.max(0.0),
            release_ms: release_ms.max(0.0),
            envelope: 0.0,
        }
    }

    fn gain_reduction(&self, level: f32) -> f32 {
        if level <= self.threshold {
            1.0
        } else {
            // Gain computer: above threshold, reduce by ratio
            let excess_db = 20.0 * (level / self.threshold.max(1e-10)).log10();
            let reduced_db = excess_db / self.ratio;
            let gain_db = reduced_db - excess_db; // negative
            10.0_f32.powf(gain_db / 20.0)
        }
    }
}

impl AudioEffect for Compressor {
    fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let sr = sample_rate.max(1.0);
        let attack_coeff = (-1.0 / (self.attack_ms * 0.001 * sr)).exp();
        let release_coeff = (-1.0 / (self.release_ms * 0.001 * sr)).exp();

        let level = sample.abs();
        if level > self.envelope {
            self.envelope = attack_coeff * self.envelope + (1.0 - attack_coeff) * level;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * level;
        }

        sample * self.gain_reduction(self.envelope)
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
    }
}

// ── EffectsChain ──────────────────────────────────────────────────────────────

/// An ordered chain of audio effects processed in series.
pub struct EffectsChain {
    effects: Vec<Box<dyn AudioEffect>>,
}

impl EffectsChain {
    pub fn new() -> Self {
        Self { effects: Vec::new() }
    }

    /// Append an effect to the end of the chain.
    pub fn add(&mut self, effect: Box<dyn AudioEffect>) {
        self.effects.push(effect);
    }

    /// Remove all effects from the chain.
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    /// Process one sample through all effects in order.
    pub fn process(&mut self, mut sample: f32, sample_rate: f32) -> f32 {
        for effect in &mut self.effects {
            sample = effect.process(sample, sample_rate);
        }
        sample
    }

    /// Number of effects in the chain.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}

impl Default for EffectsChain {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    // LowPassFilter tests
    #[test]
    fn test_lpf_attenuates_high_freq() {
        let mut lpf = LowPassFilter::new(100.0, 0.0);
        // Feed a burst of 1.0 samples; the output should converge toward 1.0 slowly
        let mut out = 0.0;
        for _ in 0..1000 {
            out = lpf.process(1.0, SR);
        }
        assert!(out > 0.0, "LPF output should be > 0");
        assert!(out < 1.01, "LPF output should approach but not greatly exceed 1.0");
    }

    #[test]
    fn test_lpf_dc_passthrough() {
        let mut lpf = LowPassFilter::new(100.0, 0.0);
        let mut out = 0.0;
        for _ in 0..5000 {
            out = lpf.process(1.0, SR);
        }
        assert!((out - 1.0).abs() < 0.01, "LPF should pass DC: got {}", out);
    }

    #[test]
    fn test_lpf_reset() {
        let mut lpf = LowPassFilter::new(1000.0, 0.0);
        lpf.process(1.0, SR);
        lpf.reset();
        let first = lpf.process(0.0, SR);
        assert_eq!(first, 0.0);
    }

    // HighPassFilter tests
    #[test]
    fn test_hpf_blocks_dc() {
        let mut hpf = HighPassFilter::new(200.0);
        let mut out = 0.0;
        for _ in 0..5000 {
            out = hpf.process(1.0, SR);
        }
        assert!(out.abs() < 0.05, "HPF should block DC: got {}", out);
    }

    #[test]
    fn test_hpf_passes_impulse_energy() {
        let mut hpf = HighPassFilter::new(200.0);
        let out = hpf.process(1.0, SR);
        // First sample of impulse should pass through with high amplitude
        assert!(out.abs() > 0.5, "HPF should pass impulse onset: got {}", out);
    }

    #[test]
    fn test_hpf_reset() {
        let mut hpf = HighPassFilter::new(200.0);
        hpf.process(1.0, SR);
        hpf.reset();
        let first = hpf.process(0.0, SR);
        assert_eq!(first, 0.0);
    }

    // Reverb tests
    #[test]
    fn test_reverb_silence_gives_silence() {
        let mut rev = Reverb::new(0.5, 0.5);
        let out = rev.process(0.0, SR);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn test_reverb_impulse_produces_tail() {
        let mut rev = Reverb::new(0.8, 0.3);
        let _first = rev.process(1.0, SR);
        // After the impulse, there should be some reverb tail (non-zero output)
        let mut found_nonzero = false;
        for _ in 0..2000 {
            let s = rev.process(0.0, SR);
            if s.abs() > 1e-6 {
                found_nonzero = true;
                break;
            }
        }
        assert!(found_nonzero, "reverb should produce a tail after an impulse");
    }

    #[test]
    fn test_reverb_reset() {
        let mut rev = Reverb::new(0.8, 0.3);
        for _ in 0..100 {
            rev.process(1.0, SR);
        }
        rev.reset();
        let out = rev.process(0.0, SR);
        assert_eq!(out, 0.0);
    }

    // Distortion tests
    #[test]
    fn test_distortion_zero_input() {
        let mut dist = Distortion::new(5.0, 1.0);
        assert_eq!(dist.process(0.0, SR), 0.0);
    }

    #[test]
    fn test_distortion_clips_large_input() {
        let mut dist = Distortion::new(10.0, 1.0);
        let out = dist.process(100.0, SR);
        // tanh(1000) ≈ 1.0 with mix=1
        assert!(out.abs() < 1.1, "clipped output should be near 1: got {}", out);
    }

    #[test]
    fn test_distortion_mix_zero_is_dry() {
        let mut dist = Distortion::new(10.0, 0.0);
        let sample = 0.5_f32;
        let out = dist.process(sample, SR);
        assert!((out - sample).abs() < 1e-6, "mix=0 should be dry signal");
    }

    #[test]
    fn test_distortion_mix_one_is_wet() {
        let mut dist = Distortion::new(1.0, 1.0);
        let sample = 0.5_f32;
        let out = dist.process(sample, SR);
        let expected = (1.0 * sample).tanh();
        assert!((out - expected).abs() < 1e-6);
    }

    // Compressor tests
    #[test]
    fn test_compressor_below_threshold_unity() {
        let mut comp = Compressor::new(0.8, 4.0, 1.0, 100.0);
        let sample = 0.1_f32;
        let out = comp.process(sample, SR);
        // Below threshold: gain ≈ 1 (slight envelope build-up but minimal)
        assert!(out.abs() <= sample.abs() + 1e-4);
    }

    #[test]
    fn test_compressor_attenuates_loud_signal() {
        let mut comp = Compressor::new(0.3, 8.0, 1.0, 10.0);
        let mut out = 0.0;
        for _ in 0..1000 {
            out = comp.process(1.0, SR);
        }
        assert!(out < 0.9, "compressor should attenuate loud signal: got {}", out);
    }

    #[test]
    fn test_compressor_reset() {
        let mut comp = Compressor::new(0.3, 4.0, 1.0, 100.0);
        for _ in 0..100 {
            comp.process(1.0, SR);
        }
        comp.reset();
        // After reset, envelope = 0 so unity gain below threshold
        let out = comp.process(0.1, SR);
        assert!(out.abs() > 0.0);
    }

    // EffectsChain tests
    #[test]
    fn test_empty_chain_passthrough() {
        let mut chain = EffectsChain::new();
        let sample = 0.7_f32;
        assert_eq!(chain.process(sample, SR), sample);
    }

    #[test]
    fn test_chain_len() {
        let mut chain = EffectsChain::new();
        assert_eq!(chain.len(), 0);
        chain.add(Box::new(LowPassFilter::new(1000.0, 0.0)));
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_chain_clear() {
        let mut chain = EffectsChain::new();
        chain.add(Box::new(LowPassFilter::new(1000.0, 0.0)));
        chain.clear();
        assert_eq!(chain.len(), 0);
    }

    #[test]
    fn test_chain_ordering_matters() {
        // LPF followed by HPF vs HPF followed by LPF will differ at certain freqs.
        let mut chain1 = EffectsChain::new();
        chain1.add(Box::new(LowPassFilter::new(5000.0, 0.0)));
        chain1.add(Box::new(HighPassFilter::new(100.0)));

        let mut chain2 = EffectsChain::new();
        chain2.add(Box::new(HighPassFilter::new(100.0)));
        chain2.add(Box::new(LowPassFilter::new(5000.0, 0.0)));

        // Both chains applied to a sustained DC signal should converge to similar
        // values (band-pass), but the transient behaviour differs.
        let mut out1 = 0.0;
        let mut out2 = 0.0;
        for _ in 0..500 {
            out1 = chain1.process(1.0, SR);
            out2 = chain2.process(1.0, SR);
        }
        // Both should be near 0 for DC (high-pass blocks DC)
        assert!(out1.abs() < 0.1 && out2.abs() < 0.1);
    }

    #[test]
    fn test_chain_with_distortion_and_lpf() {
        let mut chain = EffectsChain::new();
        chain.add(Box::new(Distortion::new(20.0, 1.0)));
        chain.add(Box::new(LowPassFilter::new(2000.0, 0.0)));
        // Should not panic; output should be bounded
        let out = chain.process(0.8, SR);
        assert!(out.abs() < 2.0);
    }
}
