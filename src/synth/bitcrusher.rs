pub struct Bitcrusher {
    pub bit_depth: f32,   // 1..16, 16=bypass
    pub rate_crush: f32,  // 0..1, 0=bypass (sample rate reduction)
    sample_hold: f32,
    sample_counter: u32,  // integer counter for even timing
    rate_period: u32,     // how many input samples per held sample
    rng_state: u64,
}

impl Bitcrusher {
    pub fn new() -> Self {
        Self::with_seed(0xDEADBEEFCAFEBABE)
    }

    /// Construct with a caller-supplied seed so per-layer dither is decorrelated.
    pub fn with_seed(seed: u64) -> Self {
        Self {
            bit_depth: 16.0,
            rate_crush: 0.0,
            sample_hold: 0.0,
            sample_counter: 0,
            rate_period: 1,
            rng_state: seed,
        }
    }

    /// xorshift64 — fast PRNG, returns [0, 1)
    fn rng(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        let bits = 0x3F800000u32 | ((self.rng_state >> 41) as u32 & 0x007FFFFF);
        f32::from_bits(bits) - 1.0
    }

    pub fn process(&mut self, x: f32) -> f32 {
        // Bit crush (bypass at 16 bits)
        let crushed = if self.bit_depth < 15.9 {
            let levels = 2.0f32.powf(self.bit_depth.clamp(1.0, 16.0));
            // Triangular dither: two uniform samples summed → triangular distribution
            let d1 = (self.rng() - 0.5) / levels;
            let d2 = (self.rng() - 0.5) / levels;
            ((x * levels + d1 + d2).round()) / levels
        } else {
            x
        };

        // Rate crush (bypass at 0) — integer modulo for even timing
        if self.rate_crush < 0.001 {
            return crushed;
        }
        // Convert rate_crush [0,1] to a period: 1 = no crush, higher = more crush
        let new_period = (1.0 / self.rate_crush.clamp(0.001, 1.0)).round() as u32;
        if new_period != self.rate_period {
            self.rate_period = new_period;
            self.sample_counter = 0;
        }
        self.sample_counter += 1;
        if self.sample_counter >= self.rate_period {
            self.sample_counter = 0;
            self.sample_hold = crushed;
        }
        self.sample_hold
    }
}
