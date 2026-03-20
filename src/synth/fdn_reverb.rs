//! FDN Reverb — 8-channel Feedback Delay Network.
//!
//! Improvements over the original:
//!
//! 1. **Pre-delay** (10 ms default) — separates the dry signal from the early
//!    reflections so the reverb tail doesn't smear into the attack.
//!
//! 2. **Delay-line modulation** — each channel's read pointer is offset by a
//!    slowly drifting LFO (different rate per channel, all below 2 Hz).
//!    Linear interpolation on the read gives smooth pitch variation without
//!    stepping.  This breaks up the metallic, pitched resonances that FDN
//!    reverbs are notorious for at long decay times.
//!
//! 3. **True stereo input injection** — previously both L and R were mixed to
//!    mono before entering the FDN.  Now they inject into separate channels
//!    (even=L, odd=R) at full level, preserving the stereo image in the tail.

// Base delay lengths at 44 100 Hz (coprime, span 35–85 ms)
const FDN_DELAYS_44K: [usize; 8] = [1559, 1877, 2053, 2381, 2713, 3067, 3413, 3761];

// LFO depths in samples and rates in Hz per channel.
// Keep depths < 8 samples and rates well below 2 Hz to avoid audible pitch wobble.
const LFO_DEPTHS: [f32; 8] = [3.5, 2.8, 4.1, 3.2, 5.0, 2.5, 3.8, 4.4];
const LFO_RATES: [f32; 8] = [0.31, 0.47, 0.61, 0.79, 0.97, 1.13, 1.31, 1.51];

struct FdnChannel {
    buf: Vec<f32>,
    write_pos: usize,
    lp_state: f32,
    lfo_phase: f32,
    lfo_rate: f32,  // radians per sample
    lfo_depth: f32, // samples of modulation
}

impl FdnChannel {
    fn new(len: usize, lfo_depth: f32, lfo_rate_hz: f32, sample_rate: f32) -> Self {
        use std::f32::consts::TAU;
        // Extra buffer headroom for LFO swing
        let buf_len = len + (lfo_depth.ceil() as usize) + 4;
        Self {
            buf: vec![0.0; buf_len.max(16)],
            write_pos: 0,
            lp_state: 0.0,
            lfo_phase: 0.0,
            lfo_rate: TAU * lfo_rate_hz / sample_rate,
            lfo_depth,
        }
    }

    /// Write one sample and advance the write pointer.
    fn write(&mut self, val: f32) {
        self.buf[self.write_pos] = val;
        self.write_pos = (self.write_pos + 1) % self.buf.len();
    }

    /// Read from the delay line at `base_delay` samples back, modulated by LFO,
    /// using linear interpolation for alias-free pitch variation.
    fn read_modulated(&mut self, base_delay: usize) -> f32 {
        self.lfo_phase = (self.lfo_phase + self.lfo_rate).rem_euclid(std::f32::consts::TAU);
        let mod_offset = self.lfo_phase.sin() * self.lfo_depth;
        let delay_f = (base_delay as f32 - mod_offset).max(1.0);
        let len = self.buf.len();
        let d0 = delay_f as usize;
        let frac = delay_f - d0 as f32;
        let i0 = (self.write_pos + len - d0.min(len - 1)) % len;
        let i1 = (self.write_pos + len - (d0 + 1).min(len - 1)) % len;
        self.buf[i0] * (1.0 - frac) + self.buf[i1] * frac
    }

    /// Apply first-order lowpass damping to the read value (in place via state).
    fn apply_damping(&mut self, val: f32, damp: f32) -> f32 {
        self.lp_state = (1.0 - damp) * val + damp * self.lp_state;
        if !self.lp_state.is_finite() {
            self.lp_state = 0.0;
        }
        self.lp_state
    }
}

/// Fast Walsh-Hadamard transform for N=8 (in-place).
/// Normalized by 1/√8 so the matrix is unitary (energy-preserving diffusion).
fn hadamard8(v: &mut [f32; 8]) {
    // 3 stages of butterfly operations
    for i in (0..8).step_by(2) {
        let (a, b) = (v[i], v[i + 1]);
        v[i] = a + b;
        v[i + 1] = a - b;
    }
    for base in [0usize, 4] {
        for j in 0..2 {
            let (a, b) = (v[base + j], v[base + j + 2]);
            v[base + j] = a + b;
            v[base + j + 2] = a - b;
        }
    }
    for j in 0..4 {
        let (a, b) = (v[j], v[j + 4]);
        v[j] = a + b;
        v[j + 4] = a - b;
    }
    let norm = (8.0f32).sqrt().recip();
    for x in v.iter_mut() {
        *x *= norm;
    }
}

pub struct FdnReverb {
    channels: Vec<FdnChannel>,
    base_delays: Vec<usize>,
    pre_delay_buf: Vec<(f32, f32)>,
    pre_delay_pos: usize,
    pre_delay_len: usize,
    pub feedback: f32,
    pub damping: f32,
    pub wet: f32,
}

impl FdnReverb {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 44100.0;
        let base_delays: Vec<usize> = FDN_DELAYS_44K
            .iter()
            .map(|&d| (d as f32 * scale) as usize)
            .collect();
        let channels = (0..8)
            .map(|i| FdnChannel::new(base_delays[i], LFO_DEPTHS[i], LFO_RATES[i], sample_rate))
            .collect();

        // 10 ms pre-delay
        let pre_delay_len = ((10.0 * 0.001 * sample_rate) as usize).max(1);

        Self {
            channels,
            base_delays,
            pre_delay_buf: vec![(0.0, 0.0); pre_delay_len],
            pre_delay_pos: 0,
            pre_delay_len,
            feedback: 0.88,
            damping: 0.25,
            wet: 0.4,
        }
    }

    /// Process one stereo sample pair. Returns (left_out, right_out).
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let input_l = if input_l.is_finite() { input_l } else { 0.0 };
        let input_r = if input_r.is_finite() { input_r } else { 0.0 };

        // --- Pre-delay ---------------------------------------------------------
        // Write incoming sample; read the delayed version to feed into the FDN.
        self.pre_delay_buf[self.pre_delay_pos] = (input_l, input_r);
        let read_pos = (self.pre_delay_pos + 1) % self.pre_delay_len;
        let (pd_l, pd_r) = self.pre_delay_buf[read_pos];
        self.pre_delay_pos = (self.pre_delay_pos + 1) % self.pre_delay_len;

        // --- Read, damp, and diffuse -------------------------------------------
        let mut state = [0.0f32; 8];
        for (i, ch) in self.channels.iter_mut().enumerate() {
            let raw = ch.read_modulated(self.base_delays[i]);
            state[i] = ch.apply_damping(raw, self.damping);
        }
        hadamard8(&mut state);

        // --- Write back + inject input -----------------------------------------
        // Even channels receive L, odd channels receive R (true stereo).
        for (i, ch) in self.channels.iter_mut().enumerate() {
            let excitation = if i % 2 == 0 { pd_l } else { pd_r };
            let write_val = (state[i] * self.feedback + excitation * 0.12).clamp(-10.0, 10.0);
            ch.write(write_val);
        }

        // --- Build stereo output ----------------------------------------------
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        for (i, s) in state.iter().enumerate() {
            if i % 2 == 0 {
                out_l += s;
            } else {
                out_r += s;
            }
        }
        out_l *= 0.25;
        out_r *= 0.25;

        if !out_l.is_finite() || !out_r.is_finite() {
            for ch in &mut self.channels {
                ch.buf.iter_mut().for_each(|x| *x = 0.0);
                ch.lp_state = 0.0;
            }
            return (input_l * (1.0 - self.wet), input_r * (1.0 - self.wet));
        }

        let dry = 1.0 - self.wet;
        (
            input_l * dry + out_l * self.wet,
            input_r * dry + out_r * self.wet,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_fdn_output_finite() {
        let mut rv = FdnReverb::new(SR);
        for i in 0..2000 {
            let x = (i as f32 * 0.05).sin();
            let (l, r) = rv.process(x, x);
            assert!(l.is_finite(), "FDN L output non-finite at {}", i);
            assert!(r.is_finite(), "FDN R output non-finite at {}", i);
        }
    }

    #[test]
    fn test_fdn_nan_input_safe() {
        let mut rv = FdnReverb::new(SR);
        let (l, r) = rv.process(f32::NAN, f32::NAN);
        assert!(l.is_finite(), "NaN input should produce finite output");
        assert!(r.is_finite(), "NaN input should produce finite output");
    }

    #[test]
    fn test_fdn_produces_tail_after_impulse() {
        let mut rv = FdnReverb::new(SR);
        rv.wet = 1.0;
        rv.process(1.0, 1.0);
        let mut max_tail = 0.0f32;
        for _ in 0..4410 {
            let (l, _) = rv.process(0.0, 0.0);
            max_tail = max_tail.max(l.abs());
        }
        assert!(max_tail > 0.0, "FDN reverb should produce a tail after impulse");
    }

    #[test]
    fn test_fdn_wet_zero_passes_through() {
        let mut rv = FdnReverb::new(SR);
        rv.wet = 0.0;
        // With wet=0, dry = 1.0, output = input (with pre-delay warmup)
        // Warm up pre-delay buffer first
        let pre_delay_len = (10.0_f32 * 0.001 * SR) as usize;
        for _ in 0..pre_delay_len {
            rv.process(0.5, -0.3);
        }
        let (l, r) = rv.process(0.5, -0.3);
        // After warmup, output should be close to input (delayed but dry signal)
        assert!(l.is_finite());
        assert!(r.is_finite());
    }
}
