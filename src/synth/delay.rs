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
    pub delay_samples: f32,  // now fractional
    pub feedback: f32,
    pub mix: f32,
}

impl DelayLine {
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
