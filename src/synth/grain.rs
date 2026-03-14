/// Granular synthesis engine.
/// Maintains a pool of active grains; new grains are spawned by the audio thread
/// based on control parameters set by the simulation thread.

use super::envelope::Adsr;
use std::f32::consts::TAU;

const MAX_GRAINS: usize = 64;

struct Grain {
    osc_phase: f32,
    freq: f32,
    pan: f32, // -1..1
    envelope: Adsr,
    active: bool,
}

impl Grain {
    fn silent() -> Self {
        Self {
            osc_phase: 0.0,
            freq: 440.0,
            pan: 0.0,
            envelope: Adsr::new(10.0, 50.0, 0.7, 80.0, 44100.0),
            active: false,
        }
    }

    fn next_sample(&mut self) -> (f32, f32) {
        if !self.active { return (0.0, 0.0); }
        let env = self.envelope.next_sample();
        let sig = self.osc_phase.sin() * env;
        self.osc_phase = (self.osc_phase + TAU * self.freq / 44100.0).rem_euclid(TAU);
        if self.envelope.is_idle() { self.active = false; }
        let l = sig * (1.0 - self.pan.max(0.0));
        let r = sig * (1.0 + self.pan.min(0.0));
        (l, r)
    }
}

pub struct GrainEngine {
    grains: Vec<Grain>,
    sample_rate: f32,
    // Control parameters (written from sim thread via shared state)
    pub spawn_rate: f32,   // grains per second
    pub base_freq: f32,
    pub freq_spread: f32,
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
            spawn_counter: 0.0,
            rng_state: 12345,
        }
    }

    /// Simple xorshift64 RNG — no stdlib needed in audio thread.
    fn rand_f32(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        (self.rng_state as f32) / (u64::MAX as f32)
    }

    fn spawn_grain(&mut self) {
        // Precompute all random values before borrowing self.grains
        let detune = (self.rand_f32() - 0.5) * 2.0 * self.freq_spread;
        let freq = self.base_freq * 2.0f32.powf(detune);
        let pan = (self.rand_f32() - 0.5) * 1.4;
        let osc_phase = self.rand_f32() * TAU;
        let dur_ms = 50.0 + self.rand_f32() * 150.0;
        let sample_rate = self.sample_rate;

        if let Some(g) = self.grains.iter_mut().find(|g| !g.active) {
            g.freq = freq;
            g.pan = pan;
            g.osc_phase = osc_phase;
            g.envelope = Adsr::new(
                dur_ms * 0.1,
                dur_ms * 0.3,
                0.6,
                dur_ms * 0.6,
                sample_rate,
            );
            g.envelope.trigger();
            g.active = true;
        }
    }

    pub fn next_sample(&mut self) -> (f32, f32) {
        // Spawn grains
        self.spawn_counter += self.spawn_rate / self.sample_rate;
        while self.spawn_counter >= 1.0 {
            self.spawn_grain();
            self.spawn_counter -= 1.0;
        }

        let mut l = 0.0f32;
        let mut r = 0.0f32;
        for g in &mut self.grains {
            let (gl, gr) = g.next_sample();
            l += gl;
            r += gr;
        }
        // Normalize by sqrt(active) to maintain roughly constant loudness
        let active = self.grains.iter().filter(|g| g.active).count().max(1) as f32;
        let norm = 1.0 / active.sqrt();
        (l * norm, r * norm)
    }
}
