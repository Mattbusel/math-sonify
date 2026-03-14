pub struct Bitcrusher {
    pub bit_depth: f32,   // 1..16, 16=bypass
    pub rate_crush: f32,  // 0..1, 0=bypass (sample rate reduction)
    sample_hold: f32,
    sample_counter: f32,
}

impl Bitcrusher {
    pub fn new() -> Self {
        Self { bit_depth: 16.0, rate_crush: 0.0, sample_hold: 0.0, sample_counter: 0.0 }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        // Rate crusher: hold sample
        self.sample_counter += self.rate_crush + 0.01;
        if self.sample_counter >= 1.0 {
            self.sample_counter = 0.0;
            // Bit crusher: quantize
            let levels = 2.0f32.powf(self.bit_depth.clamp(1.0, 16.0));
            self.sample_hold = (x * levels).round() / levels;
        }
        self.sample_hold
    }
}
