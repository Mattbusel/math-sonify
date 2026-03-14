/// Stereo delay line with feedback.
pub struct DelayLine {
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    pub delay_samples: usize,
    pub feedback: f32,
    pub mix: f32,
}

impl DelayLine {
    pub fn new(max_delay_ms: f32, sample_rate: f32) -> Self {
        let max_samples = (max_delay_ms * 0.001 * sample_rate) as usize + 1;
        Self {
            buf_l: vec![0.0; max_samples],
            buf_r: vec![0.0; max_samples],
            pos: 0,
            delay_samples: (300.0 * 0.001 * sample_rate) as usize,
            feedback: 0.3,
            mix: 0.3,
        }
    }

    pub fn set_delay_ms(&mut self, ms: f32, sample_rate: f32) {
        self.delay_samples = ((ms * 0.001 * sample_rate) as usize)
            .min(self.buf_l.len() - 1);
    }

    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let l = if l.is_finite() { l } else { 0.0 };
        let r = if r.is_finite() { r } else { 0.0 };
        let read_pos = (self.pos + self.buf_l.len() - self.delay_samples.max(1)) % self.buf_l.len();
        let del_l = if self.buf_l[read_pos].is_finite() { self.buf_l[read_pos] } else { 0.0 };
        let del_r = if self.buf_r[read_pos].is_finite() { self.buf_r[read_pos] } else { 0.0 };
        self.buf_l[self.pos] = l + del_l * self.feedback;
        self.buf_r[self.pos] = r + del_r * self.feedback;
        self.pos = (self.pos + 1) % self.buf_l.len();
        let dry = 1.0 - self.mix;
        (l * dry + del_l * self.mix, r * dry + del_r * self.mix)
    }
}
