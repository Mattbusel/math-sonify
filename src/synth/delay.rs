/// Stereo delay line with feedback and linear interpolation.
///
/// Linear interpolation matters here because the delay time is BPM-synced and
/// changes when tempo is updated.  Integer snapping would cause a pitched click
/// at each tempo change; interpolation makes the transition inaudible.  It also
/// allows sub-sample delay times for precision in musical timing.
pub struct DelayLine {
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    pub delay_samples: f32, // now fractional
    pub feedback: f32,
    pub mix: f32,
}

impl DelayLine {
    /// Create a new delay line with the specified maximum delay and sample rate.
    ///
    /// # Parameters
    /// - `max_delay_ms`: Maximum delay time in milliseconds; sets the buffer size.
    /// - `sample_rate`: Audio sample rate in Hz.
    pub fn new(max_delay_ms: f32, sample_rate: f32) -> Self {
        let max_samples = (max_delay_ms * 0.001 * sample_rate) as usize + 4;
        Self {
            buf_l: vec![0.0; max_samples],
            buf_r: vec![0.0; max_samples],
            pos: 0,
            delay_samples: 300.0 * 0.001 * sample_rate,
            feedback: 0.3,
            mix: 0.3,
        }
    }

    /// Update the delay time; clamped to `[2 samples, buffer length - 2]`.
    pub fn set_delay_ms(&mut self, ms: f32, sample_rate: f32) {
        let max = self.buf_l.len() as f32 - 2.0;
        self.delay_samples = (ms * 0.001 * sample_rate).clamp(2.0, max);
    }

    #[inline(always)]
    fn read_interp(buf: &[f32], write_pos: usize, delay: f32) -> f32 {
        let len = buf.len();
        let d0 = delay as usize;
        let frac = delay - d0 as f32;
        let i0 = (write_pos + len - d0.min(len - 1)) % len;
        let i1 = (write_pos + len - (d0 + 1).min(len - 1)) % len;
        buf[i0] * (1.0 - frac) + buf[i1] * frac
    }

    /// Process one stereo sample pair and return `(dry + wet_left, dry + wet_right)`.
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let l = if l.is_finite() { l } else { 0.0 };
        let r = if r.is_finite() { r } else { 0.0 };

        let del_l = Self::read_interp(&self.buf_l, self.pos, self.delay_samples);
        let del_r = Self::read_interp(&self.buf_r, self.pos, self.delay_samples);

        let del_l = if del_l.is_finite() { del_l } else { 0.0 };
        let del_r = if del_r.is_finite() { del_r } else { 0.0 };

        // Simple clamp instead of tanh saturation: tanh in the feedback loop
        // causes amplitude compression at moderate levels, heard as squishy pumping
        // on percussion hits and sharp transients.
        self.buf_l[self.pos] = (l + del_l * self.feedback).clamp(-4.0, 4.0);
        self.buf_r[self.pos] = (r + del_r * self.feedback).clamp(-4.0, 4.0);
        self.pos = (self.pos + 1) % self.buf_l.len();

        let dry = 1.0 - self.mix;
        (l * dry + del_l * self.mix, r * dry + del_r * self.mix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_delay_mix_zero_passes_dry() {
        let mut dl = DelayLine::new(1000.0, SR);
        dl.mix = 0.0;
        dl.feedback = 0.0;
        let (l, r) = dl.process(0.5, -0.5);
        // With mix=0, output = l*dry + del*mix = l*1 + del*0 = l
        assert!((l - 0.5).abs() < 1e-6, "mix=0 should pass dry signal: {}", l);
        assert!((r - (-0.5)).abs() < 1e-6, "mix=0 should pass dry signal: {}", r);
    }

    #[test]
    fn test_delay_output_always_finite() {
        let mut dl = DelayLine::new(1000.0, SR);
        dl.mix = 0.3;
        dl.feedback = 0.5;
        for i in 0..2000 {
            let x = (i as f32 * 0.1).sin();
            let (l, r) = dl.process(x, x);
            assert!(l.is_finite(), "Left output non-finite at {}", i);
            assert!(r.is_finite(), "Right output non-finite at {}", i);
        }
    }

    #[test]
    fn test_delay_nan_input_safe() {
        let mut dl = DelayLine::new(1000.0, SR);
        dl.mix = 0.3;
        let (l, r) = dl.process(f32::NAN, f32::NAN);
        assert!(l.is_finite(), "NaN input should produce finite output: {}", l);
        assert!(r.is_finite(), "NaN input should produce finite output: {}", r);
    }

    #[test]
    fn test_delay_produces_echo() {
        // Send an impulse then silence; after delay_samples we should hear the echo
        let mut dl = DelayLine::new(200.0, SR);
        let delay_ms = 100.0_f32;
        dl.set_delay_ms(delay_ms, SR);
        dl.mix = 1.0;
        dl.feedback = 0.0;
        let delay_samples = (delay_ms * 0.001 * SR) as usize;

        // Send impulse then silence
        let mut outputs_l = Vec::new();
        dl.process(1.0, 0.0); // impulse
        for _ in 0..delay_samples + 10 {
            let (l, _) = dl.process(0.0, 0.0);
            outputs_l.push(l);
        }
        // Around delay_samples, there should be a non-zero echo
        let echo_region = &outputs_l[delay_samples.saturating_sub(5)..];
        let has_echo = echo_region.iter().any(|v| v.abs() > 0.01);
        assert!(has_echo, "Delay should produce an echo after delay_samples");
    }

    #[test]
    fn test_delay_feedback_sustains_signal() {
        // With high feedback, the signal should persist longer than without
        let mut dl_no_fb = DelayLine::new(200.0, SR);
        dl_no_fb.set_delay_ms(50.0, SR);
        dl_no_fb.mix = 0.5;
        dl_no_fb.feedback = 0.0;

        let mut dl_with_fb = DelayLine::new(200.0, SR);
        dl_with_fb.set_delay_ms(50.0, SR);
        dl_with_fb.mix = 0.5;
        dl_with_fb.feedback = 0.8;

        // Send one impulse then silence
        dl_no_fb.process(1.0, 1.0);
        dl_with_fb.process(1.0, 1.0);

        let delay_s = (50.0 * 0.001 * SR) as usize;
        let mut energy_no_fb = 0.0_f32;
        let mut energy_with_fb = 0.0_f32;
        for _ in 0..delay_s * 5 {
            let (l, _) = dl_no_fb.process(0.0, 0.0);
            energy_no_fb += l * l;
            let (l, _) = dl_with_fb.process(0.0, 0.0);
            energy_with_fb += l * l;
        }
        assert!(
            energy_with_fb > energy_no_fb,
            "Feedback should sustain signal longer: no_fb={}, with_fb={}",
            energy_no_fb,
            energy_with_fb
        );
    }

    #[test]
    fn test_delay_set_delay_ms_clamps_to_buffer() {
        let mut dl = DelayLine::new(100.0, SR); // 100 ms max
        // Request much larger than buffer — should be clamped
        dl.set_delay_ms(9999.0, SR);
        let max = dl.buf_l.len() as f32 - 2.0;
        assert!(
            dl.delay_samples <= max,
            "delay_samples should be clamped to buffer: {}",
            dl.delay_samples
        );
        // Also test requesting too-small delay
        dl.set_delay_ms(0.0001, SR);
        assert!(
            dl.delay_samples >= 2.0,
            "delay_samples should be clamped to minimum 2: {}",
            dl.delay_samples
        );
    }

    #[test]
    fn test_delay_higher_mix_increases_wet_output() {
        // With a warm-up impulse, higher mix should yield more wet signal in steady state
        let mut dl_low = DelayLine::new(200.0, SR);
        dl_low.set_delay_ms(10.0, SR);
        dl_low.mix = 0.1;
        dl_low.feedback = 0.5;

        let mut dl_high = DelayLine::new(200.0, SR);
        dl_high.set_delay_ms(10.0, SR);
        dl_high.mix = 0.9;
        dl_high.feedback = 0.5;

        // Warm up with a sustained tone
        let warm = (10.0 * 0.001 * SR) as usize * 3;
        let mut rms_low = 0.0_f32;
        let mut rms_high = 0.0_f32;
        for i in 0..warm {
            let x = (i as f32 * 0.1).sin();
            let (l, _) = dl_low.process(x, x);
            let (h, _) = dl_high.process(x, x);
            if i > warm / 2 {
                rms_low += l * l;
                rms_high += h * h;
            }
        }
        assert!(
            rms_high > rms_low,
            "Higher mix should yield more wet signal: low={}, high={}",
            rms_low,
            rms_high
        );
    }
}
