/// Simple stereo chorus: 3 voices with LFO-modulated delay lines.
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
    pub fn new(sample_rate: f32) -> Self {
        let max_delay_samples = (20.0 * 0.001 * sample_rate) as usize + 1;
        Self {
            mix: 0.0,
            rate: 0.5,
            depth: 3.0,
            buf_l: vec![0.0; max_delay_samples],
            buf_r: vec![0.0; max_delay_samples],
            pos: 0,
            lfo_phases: [0.0, 2.094, 4.189],
            sample_rate,
        }
    }

    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        use std::f32::consts::TAU;
        if self.mix < 0.001 { return (l, r); }
        let len = self.buf_l.len();
        self.buf_l[self.pos] = l;
        self.buf_r[self.pos] = r;

        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        let omega = TAU * self.rate / self.sample_rate;

        for (i, phase) in self.lfo_phases.iter_mut().enumerate() {
            *phase = (*phase + omega).rem_euclid(TAU);
            let delay_ms = 7.0 + phase.sin() * self.depth;
            let delay_samples = (delay_ms * 0.001 * self.sample_rate) as usize;
            let read = (self.pos + len - delay_samples.min(len - 1)) % len;
            if i % 2 == 0 { out_l += self.buf_l[read]; } else { out_l += self.buf_l[read] * 0.5; }
            if i % 2 == 1 { out_r += self.buf_r[read]; } else { out_r += self.buf_r[read] * 0.5; }
        }
        out_l /= 2.0;
        out_r /= 2.0;

        self.pos = (self.pos + 1) % len;
        let dry = 1.0 - self.mix;
        (l * dry + out_l * self.mix, r * dry + out_r * self.mix)
    }
}
