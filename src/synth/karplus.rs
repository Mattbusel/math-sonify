/// Karplus-Strong plucked string synthesis.
/// Triggered by Poincaré section crossings from the dynamical system.
pub struct KarplusStrong {
    buf: Vec<f32>,
    pos: usize,
    pub decay: f32,
    pub active: bool,
    pub volume: f32,
}

impl KarplusStrong {
    pub fn new(max_freq_hz: f32, sample_rate: f32) -> Self {
        let max_len = (sample_rate / max_freq_hz) as usize + 2;
        Self { buf: vec![0.0; max_len], pos: 0, decay: 0.996, active: false, volume: 0.5 }
    }

    /// Trigger a new note at the given frequency.
    pub fn trigger(&mut self, freq: f32, sample_rate: f32) {
        let len = ((sample_rate / freq.max(20.0)) as usize).max(2).min(self.buf.len());
        let mut rng = self.pos as u64 * 6364136223846793005 + 1;
        for i in 0..len {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.buf[i] = (rng >> 33) as f32 / u32::MAX as f32 * 2.0 - 1.0;
        }
        for i in len..self.buf.len() { self.buf[i] = 0.0; }
        self.pos = 0;
        self.active = true;
    }

    pub fn next_sample(&mut self) -> f32 {
        if !self.active { return 0.0; }
        let len = self.buf.len();
        let next = (self.pos + 1) % len;
        let out = self.buf[self.pos];
        self.buf[self.pos] = (self.buf[self.pos] + self.buf[next]) * 0.5 * self.decay;
        self.pos = next;
        out * self.volume
    }
}
