/// Granular synthesis engine.
///
/// Each grain is a short windowed burst of a sine oscillator.  Using a
/// raised-cosine (Hann) window instead of a raw ADSR envelope eliminates the
/// spectral splatter and clicking that makes granular clouds sound noisy.
/// Grains also support harmonic stacking (octave / fifth copies at reduced
/// amplitude) so the cloud has natural musical richness.

use std::f32::consts::{TAU, PI};

const MAX_GRAINS: usize = 96; // increased from 64 for denser texture

struct Grain {
    osc_phase: f32,
    freq: f32,
    pan: f32,       // -1..1
    // Window state
    window_phase: f32,  // 0..1 over grain lifetime
    window_inc: f32,    // 1 / duration_samples
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
        if !self.active { return (0.0, 0.0); }

        let env = Self::hann(self.window_phase) * self.amplitude;
        let sig = self.osc_phase.sin() * env;

        self.osc_phase = (self.osc_phase + TAU * self.freq / sample_rate).rem_euclid(TAU);
        self.window_phase += self.window_inc;

        if self.window_phase >= 1.0 { self.active = false; }

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
    pub spawn_rate: f32,    // grains per second
    pub base_freq: f32,
    pub freq_spread: f32,   // semitones of random detune (±)
    /// Grain overlap ratio (0.5 = 50% overlap, i.e., spawn rate relative to grain duration).
    /// Used externally to scale spawn_rate: spawn_rate = overlap * sample_rate / avg_grain_duration.
    pub overlap: f32,
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
            spawn_counter: 0.0,
            rng_state: 12345,
        }
    }

    /// xorshift64 — fast, no stdlib needed in the audio thread.
    fn rand_f32(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        // Map to [0, 1) via top 23 mantissa bits
        let bits = 0x3F800000u32 | ((self.rng_state >> 41) as u32 & 0x007FFFFF);
        f32::from_bits(bits) - 1.0
    }

    fn spawn_grain(&mut self) {
        let sr = self.sample_rate;

        // Random detune in semitones → frequency ratio
        let detune_st = (self.rand_f32() - 0.5) * 2.0 * self.freq_spread;
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
        // Duration 40–220 ms; shorter grains at higher spawn rates → pitched texture
        let dur_ms = 40.0 + self.rand_f32() * 180.0;
        let dur_samples = (dur_ms * 0.001 * sr).max(1.0);
        // Amplitude: compensate for Hann window energy loss.
        // The Hann window averages 0.5 vs a rectangle window's 1.0, so multiply
        // by sqrt(2) ≈ 1.41 to restore perceptual loudness parity with other modes.
        let amplitude = (1.06 + self.rand_f32() * 0.35) * std::f32::consts::SQRT_2;

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
