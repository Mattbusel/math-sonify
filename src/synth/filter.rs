/// Second-order biquad filter (transposed direct form II).
/// Supports low-pass and band-pass configurations.
#[derive(Clone)]
pub struct BiquadFilter {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    z1: f32, z2: f32,
}

impl BiquadFilter {
    pub fn low_pass(cutoff_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = std::f32::consts::TAU * cutoff_hz / sample_rate;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 - cos_w0) / 2.0 / a0,
            b1: (1.0 - cos_w0) / a0,
            b2: (1.0 - cos_w0) / 2.0 / a0,
            a1: -2.0 * cos_w0 / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0, z2: 0.0,
        }
    }

    pub fn band_pass(center_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = std::f32::consts::TAU * center_hz / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b1: 0.0,
            b2: -alpha / a0,
            a1: -2.0 * w0.cos() / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0, z2: 0.0,
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let x = if x.is_finite() { x } else { 0.0 };
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        // On NaN: clear state rather than clamping to ±1.
        // Clamping leaves stored energy that causes a loud transient on recovery;
        // zeroing gives a clean restart with only a brief silence artefact.
        if y.is_finite() { y } else {
            self.z1 = 0.0;
            self.z2 = 0.0;
            0.0
        }
    }

    pub fn update_lp(&mut self, cutoff_hz: f32, q: f32, sample_rate: f32) {
        // Clamp to safe ranges — zero or near-Nyquist cutoff produces NaN coefficients
        let cutoff = cutoff_hz.clamp(20.0, sample_rate * 0.45);
        let q_safe = q.max(0.1);
        let new = Self::low_pass(cutoff, q_safe, sample_rate);
        self.b0 = new.b0; self.b1 = new.b1; self.b2 = new.b2;
        self.a1 = new.a1; self.a2 = new.a2;
        // Reset state if it has gone NaN/inf
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0; self.z2 = 0.0;
        }
    }

    pub fn reset_if_nan(&mut self) {
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0; self.z2 = 0.0;
        }
    }

    /// Update band-pass coefficients in place, preserving filter state.
    /// Use this instead of creating a new filter to avoid resetting z1/z2 state.
    pub fn update_bp(&mut self, center_hz: f32, q: f32, sample_rate: f32) {
        let center = center_hz.clamp(20.0, sample_rate * 0.45);
        let q_safe = q.max(0.1);
        let new = Self::band_pass(center, q_safe, sample_rate);
        self.b0 = new.b0; self.b1 = new.b1; self.b2 = new.b2;
        self.a1 = new.a1; self.a2 = new.a2;
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0; self.z2 = 0.0;
        }
    }
}
