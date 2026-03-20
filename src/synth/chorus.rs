/// Stereo chorus — 3 voices with LFO-modulated delay lines.
///
/// Linear interpolation on the delay-line read pointer is essential here:
/// integer-sample snapping causes audible pitch quantisation that makes
/// the modulation sound stepped and digital rather than smooth and liquid.
pub struct Chorus {
    pub mix: f32,
    pub rate: f32,
    pub depth: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    lfo_phases: [f32; 3],
    sample_rate: f32,
}

impl Chorus {
    /// Create a new chorus with default settings (off, slow rate, moderate depth).
    pub fn new(sample_rate: f32) -> Self {
        // Max delay 50 ms (a little headroom above the deepest modulation)
        let max_delay_samples = (50.0 * 0.001 * sample_rate) as usize + 2;
        Self {
            mix: 0.0,
            rate: 0.5,
            depth: 3.0,
            buf_l: vec![0.0; max_delay_samples],
            buf_r: vec![0.0; max_delay_samples],
            pos: 0,
            // Three LFOs 120° apart in phase for uniform stereo spreading
            lfo_phases: [0.0, 2.094_395, 4.188_790],
            sample_rate,
        }
    }

    /// Linearly interpolate between two consecutive delay-buffer samples.
    /// This removes the harmonic distortion caused by integer-index snapping.
    #[inline(always)]
    fn read_interp(buf: &[f32], write_pos: usize, delay_samples: f32) -> f32 {
        let len = buf.len();
        let delay_floor = delay_samples as usize;
        let frac = delay_samples - delay_floor as f32;
        let i0 = (write_pos + len - delay_floor.min(len - 1)) % len;
        let i1 = (write_pos + len - (delay_floor + 1).min(len - 1)) % len;
        buf[i0] * (1.0 - frac) + buf[i1] * frac
    }

    /// Process one stereo sample pair and return the chorused output.
    ///
    /// Returns the input unchanged when `self.mix < 0.001`.
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        use std::f32::consts::TAU;
        if self.mix < 0.001 {
            return (l, r);
        }

        self.buf_l[self.pos] = l;
        self.buf_r[self.pos] = r;

        let omega = TAU * self.rate / self.sample_rate;
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;

        for (i, phase) in self.lfo_phases.iter_mut().enumerate() {
            *phase = (*phase + omega).rem_euclid(TAU);
            // Centre delay 7 ms, modulated ± depth ms
            let delay_ms = 7.0 + phase.sin() * self.depth;
            let delay_samples = (delay_ms * 0.001 * self.sample_rate).max(0.0);

            let dl = Self::read_interp(&self.buf_l, self.pos, delay_samples);
            let dr = Self::read_interp(&self.buf_r, self.pos, delay_samples);

            // Distribute voices symmetrically: L-dominant, R-dominant, R-dominant.
            // Weight sums: L = 1+0.5+0.5 = 2.0, R = 0.5+1+1 = 2.5 — equal after
            // the per-channel normalisation below.  (Previously voices 0 and 2
            // were both L-dominant giving L=2.5, R=2.0 → 1.94 dB stereo imbalance.)
            if i == 0 {
                out_l += dl;
                out_r += dr * 0.5;
            } else {
                out_l += dl * 0.5;
                out_r += dr;
            }
        }
        // Normalise each channel by its actual total voice weight so the wet
        // level is equal on L and R regardless of the panning distribution.
        out_l /= 2.0; // L weights: 1 + 0.5 + 0.5 = 2.0
        out_r /= 2.5; // R weights: 0.5 + 1   + 1   = 2.5

        self.pos = (self.pos + 1) % self.buf_l.len();

        let dry = 1.0 - self.mix;
        (l * dry + out_l * self.mix, r * dry + out_r * self.mix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_chorus_bypass_when_mix_zero() {
        let mut ch = Chorus::new(SR);
        ch.mix = 0.0;
        let (l, r) = ch.process(0.7, -0.3);
        assert!((l - 0.7).abs() < 1e-6, "mix=0 should be bypass: {}", l);
        assert!((r - (-0.3)).abs() < 1e-6, "mix=0 should be bypass: {}", r);
    }

    #[test]
    fn test_chorus_output_finite() {
        let mut ch = Chorus::new(SR);
        ch.mix = 0.5;
        ch.rate = 1.0;
        ch.depth = 5.0;
        for i in 0..2000 {
            let x = (i as f32 * 0.05).sin();
            let (l, r) = ch.process(x, -x);
            assert!(l.is_finite(), "Left output non-finite at {}", i);
            assert!(r.is_finite(), "Right output non-finite at {}", i);
        }
    }

    #[test]
    fn test_chorus_output_bounded() {
        // With mix=1 and unity input, output should stay in a reasonable range
        let mut ch = Chorus::new(SR);
        ch.mix = 1.0;
        for i in 0..1000 {
            let x = (i as f32 * 0.05).sin();
            let (l, r) = ch.process(x, x);
            assert!(l.abs() < 2.0, "Left output too large: {}", l);
            assert!(r.abs() < 2.0, "Right output too large: {}", r);
        }
    }

    #[test]
    fn test_chorus_both_channels_produce_output() {
        // Both L and R channels should produce non-zero output when fully wet.
        // Uses noise-like input to avoid cancellation artifacts.
        let mut ch = Chorus::new(SR);
        ch.mix = 1.0;
        ch.rate = 0.5;
        ch.depth = 3.0;
        let mut max_l = 0.0f32;
        let mut max_r = 0.0f32;
        for i in 0..4000 {
            // Use coprime-frequency sum to avoid perfect cancellation
            let x = (i as f32 * 0.07).sin() + (i as f32 * 0.13).sin();
            let (l, r) = ch.process(x, x);
            max_l = max_l.max(l.abs());
            max_r = max_r.max(r.abs());
        }
        assert!(max_l > 0.01, "Left channel has no output: max={}", max_l);
        assert!(max_r > 0.01, "Right channel has no output: max={}", max_r);
    }
}
