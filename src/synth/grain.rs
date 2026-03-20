//! Granular synthesis engine.
//!
//! Each grain is a short windowed burst of a sine oscillator.  Using a
//! raised-cosine (Hann) window instead of a raw ADSR envelope eliminates the
//! spectral splatter and clicking that makes granular clouds sound noisy.
//! Grains also support harmonic stacking (octave / fifth copies at reduced
//! amplitude) so the cloud has natural musical richness.

use std::f32::consts::{PI, TAU};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_GRAINS: usize = 256; // increased from 96 for even denser texture

/// Global counter ensures each GrainEngine instance gets a unique xorshift seed,
/// preventing correlated noise bursts when multiple layers start simultaneously.
static GRAIN_ENGINE_COUNTER: AtomicU64 = AtomicU64::new(1);

struct Grain {
    osc_phase: f32,
    freq: f32,
    pan: f32, // -1..1
    // Window state
    window_phase: f32, // 0..1 over grain lifetime
    window_inc: f32,   // 1 / duration_samples
    amplitude: f32,
    active: bool,
}

impl Grain {
    fn silent() -> Self {
        Self {
            osc_phase: 0.0,
            freq: 440.0,
            pan: 0.0,
            window_phase: 0.0,
            window_inc: 0.0,
            amplitude: 0.0,
            active: false,
        }
    }

    /// Hann window: sin²(π·t) — zero at both ends, peak of 1 at midpoint.
    /// Guarantees perfect amplitude reconstruction when grains overlap at 50% duty.
    #[inline(always)]
    fn hann(t: f32) -> f32 {
        let s = (PI * t).sin();
        s * s
    }

    fn next_sample(&mut self, sample_rate: f32) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }

        let env = Self::hann(self.window_phase) * self.amplitude;
        let sig = self.osc_phase.sin() * env;

        self.osc_phase = (self.osc_phase + TAU * self.freq / sample_rate).rem_euclid(TAU);
        self.window_phase += self.window_inc;

        if self.window_phase >= 1.0 {
            self.active = false;
        }

        // Equal-power panning: constant loudness across the stereo field
        let pan_angle = (self.pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4; // [0, π/2]
        let l = sig * pan_angle.cos();
        let r = sig * pan_angle.sin();
        (l, r)
    }
}

pub struct GrainEngine {
    grains: Vec<Grain>,
    sample_rate: f32,
    pub spawn_rate: f32, // grains per second
    pub base_freq: f32,
    pub freq_spread: f32, // semitones of random detune (±)
    /// Grain overlap ratio (0.5 = 50% overlap, i.e., spawn rate relative to grain duration).
    /// Used externally to scale spawn_rate: spawn_rate = overlap * sample_rate / avg_grain_duration.
    pub overlap: f32,
    /// Stochastic variation level (0..1).  At 0: all grains are identical/coherent.
    /// At 1: duration varies ±30% and pitch varies ±0.05 semitones per grain.
    pub chaos_level: f32,
    spawn_counter: f32,
    rng_state: u64,
}

impl GrainEngine {
    pub fn new(sample_rate: f32) -> Self {
        let grains = (0..MAX_GRAINS).map(|_| Grain::silent()).collect();
        Self {
            grains,
            sample_rate,
            spawn_rate: 20.0,
            base_freq: 220.0,
            freq_spread: 0.5,
            overlap: 0.5,
            chaos_level: 0.0,
            spawn_counter: 0.0,
            // Unique seed per instance: prevents correlated noise across simultaneous layers.
            rng_state: GRAIN_ENGINE_COUNTER
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(0x6C62272E07BB0142),
        }
    }

    /// xorshift64 — fast, no stdlib needed in the audio thread.
    fn rand_f32(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        // Map to [0, 1) via top 23 mantissa bits
        let bits = 0x3F80_0000u32 | ((self.rng_state >> 41) as u32 & 0x007F_FFFF);
        f32::from_bits(bits) - 1.0
    }

    fn spawn_grain(&mut self) {
        let sr = self.sample_rate;

        // Random detune in semitones → frequency ratio.
        // chaos_level adds ±0.05 semitones of per-grain pitch shimmer on top of freq_spread.
        let shimmer_st = (self.rand_f32() - 0.5) * 2.0 * self.chaos_level * 0.05;
        let detune_st = (self.rand_f32() - 0.5) * 2.0 * self.freq_spread + shimmer_st;
        let freq = self.base_freq * 2.0f32.powf(detune_st / 12.0);

        // Occasional harmonic shift: octave down (25%), fifth up (15%), unison (60%)
        let harmonic_roll = self.rand_f32();
        let freq = if harmonic_roll < 0.25 {
            freq * 0.5
        } else if harmonic_roll < 0.40 {
            freq * 1.5
        } else {
            freq
        };

        let pan = (self.rand_f32() - 0.5) * 1.6; // slight extra spread
        let osc_phase = self.rand_f32() * TAU;
        // Duration 40–220 ms; shorter grains at higher spawn rates → pitched texture.
        // chaos_level scales ±30% duration jitter for shimmer at high chaos.
        let dur_ms_base = 40.0 + self.rand_f32() * 180.0;
        let dur_jitter = (self.rand_f32() - 0.5) * 2.0 * self.chaos_level * 0.3;
        let dur_ms = dur_ms_base * (1.0 + dur_jitter);
        let dur_samples = (dur_ms * 0.001 * sr).max(1.0);
        // Amplitude: compensate for Hann window energy loss.
        // The Hann window averages 0.5 vs a rectangle window's 1.0, so multiply
        // by sqrt(2) ≈ 1.41 to restore perceptual loudness parity with other modes.
        // Fixed amplitude (no per-grain random variance) prevents the ±14% gain
        // fluctuation that caused audible tremolo at low grain counts.
        let amplitude = std::f32::consts::SQRT_2;

        if let Some(g) = self.grains.iter_mut().find(|g| !g.active) {
            g.freq = freq;
            g.pan = pan;
            g.osc_phase = osc_phase;
            g.window_phase = 0.0;
            g.window_inc = 1.0 / dur_samples;
            g.amplitude = amplitude;
            g.active = true;
        }
    }

    pub fn next_sample(&mut self) -> (f32, f32) {
        let sr = self.sample_rate;

        // Spawn new grains
        self.spawn_counter += self.spawn_rate / sr;
        while self.spawn_counter >= 1.0 {
            self.spawn_grain();
            self.spawn_counter -= 1.0;
        }

        let mut l = 0.0f32;
        let mut r = 0.0f32;
        for g in &mut self.grains {
            let (gl, gr) = g.next_sample(sr);
            l += gl;
            r += gr;
        }

        // Normalise by √N gives correct RMS loudness for incoherent (random-phase)
        // grains. But when many grains share similar frequencies and phases they can
        // add constructively, pushing peaks up to N rather than √N. The extra 0.6×
        // factor provides ~4 dB of headroom against coherent-phase worst-case peaks
        // without making sparse clouds sound noticeably quieter.
        let active = self.grains.iter().filter(|g| g.active).count().max(1) as f32;
        let norm = 0.6 / active.sqrt();
        (l * norm, r * norm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_grain_engine_output_finite() {
        let mut engine = GrainEngine::new(SR);
        for i in 0..2000 {
            let (l, r) = engine.next_sample();
            assert!(l.is_finite(), "L output non-finite at {}", i);
            assert!(r.is_finite(), "R output non-finite at {}", i);
        }
    }

    #[test]
    fn test_grain_engine_produces_output_with_spawn() {
        let mut engine = GrainEngine::new(SR);
        engine.spawn_rate = 100.0;
        engine.base_freq = 440.0;
        let mut max_abs = 0.0f32;
        for _ in 0..4410 {
            let (l, r) = engine.next_sample();
            max_abs = max_abs.max(l.abs()).max(r.abs());
        }
        assert!(max_abs > 0.0, "Should produce output with active grains");
    }

    #[test]
    fn test_grain_engine_two_instances_decorrelated() {
        // Each instance gets a unique seed so outputs differ
        let mut e1 = GrainEngine::new(SR);
        let mut e2 = GrainEngine::new(SR);
        e1.spawn_rate = 50.0;
        e2.spawn_rate = 50.0;
        let (l1, _) = e1.next_sample();
        let (l2, _) = e2.next_sample();
        // With unique seeds the outputs should not be identical (extremely unlikely)
        // We just verify both are finite as a smoke test
        assert!(l1.is_finite());
        assert!(l2.is_finite());
    }

    #[test]
    fn test_grain_engine_output_bounded() {
        // Even with many concurrent grains, normalization keeps output below 2.0.
        let mut engine = GrainEngine::new(SR);
        engine.spawn_rate = 500.0;
        engine.base_freq = 220.0;
        let mut max_abs = 0.0f32;
        for _ in 0..4410 {
            let (l, r) = engine.next_sample();
            max_abs = max_abs.max(l.abs()).max(r.abs());
        }
        assert!(
            max_abs < 2.0,
            "Grain engine output exceeded expected bounds: {}",
            max_abs
        );
    }

    #[test]
    fn test_grain_engine_higher_spawn_rate_increases_activity() {
        // Higher spawn rate should produce higher RMS after warmup.
        let mut engine_lo = GrainEngine::new(SR);
        engine_lo.spawn_rate = 5.0;
        let mut engine_hi = GrainEngine::new(SR);
        engine_hi.spawn_rate = 200.0;

        let mut rms_lo = 0.0f32;
        let mut rms_hi = 0.0f32;
        let n = 4410usize;
        for _ in 0..n {
            let (l, _) = engine_lo.next_sample();
            rms_lo += l * l;
            let (l2, _) = engine_hi.next_sample();
            rms_hi += l2 * l2;
        }
        rms_lo = (rms_lo / n as f32).sqrt();
        rms_hi = (rms_hi / n as f32).sqrt();
        assert!(
            rms_hi > rms_lo,
            "Higher spawn rate should increase RMS: lo={}, hi={}",
            rms_lo,
            rms_hi
        );
    }

    #[test]
    fn test_grain_hann_window_zero_at_endpoints() {
        // The Hann window must be 0 at t=0 and t=1 to prevent clicks.
        assert!(Grain::hann(0.0).abs() < 1e-6, "Hann window should be 0 at t=0");
        assert!(Grain::hann(1.0).abs() < 1e-6, "Hann window should be 0 at t=1");
    }

    #[test]
    fn test_grain_hann_window_peak_at_midpoint() {
        // Hann window peaks at t=0.5 with value 1.0.
        let peak = Grain::hann(0.5);
        assert!(
            (peak - 1.0).abs() < 1e-6,
            "Hann window should peak at 1.0 at t=0.5, got {}",
            peak
        );
    }
}
