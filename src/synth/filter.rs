/// Second-order biquad filter using transposed direct form II.
///
/// Supports low-pass and band-pass configurations.  All internal state is
/// sanitized after each sample so that non-finite values (NaN / Inf) in the
/// input or coefficient calculation cannot corrupt the filter permanently.
///
/// Use [`BiquadFilter::update_lp`] / [`BiquadFilter::update_bp`] to change
/// the filter parameters at run-time without resetting the delay state.
#[derive(Clone)]
pub struct BiquadFilter {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    z1: f32, z2: f32,
}

impl BiquadFilter {
    /// Construct a new low-pass biquad at the given cutoff and resonance.
    ///
    /// # Parameters
    /// - `cutoff_hz`: -3 dB cutoff frequency in Hz.
    /// - `q`: Filter quality factor; 0.707 gives a maximally-flat (Butterworth) response.
    /// - `sample_rate`: Audio sample rate in Hz.
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

    /// Construct a new band-pass biquad (constant skirt gain, unity peak gain).
    ///
    /// # Parameters
    /// - `center_hz`: Center frequency in Hz.
    /// - `q`: Quality factor (bandwidth = center_hz / q).
    /// - `sample_rate`: Audio sample rate in Hz.
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

    /// Process one audio sample through the filter and return the filtered output.
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

    /// Update low-pass coefficients in place, preserving the filter delay state.
    ///
    /// The cutoff is clamped to `[20 Hz, sample_rate * 0.45]` and Q to `[0.1, ∞)` to
    /// prevent coefficient computation from producing NaN values.
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

    /// Reset the delay-line state to zero if it has gone non-finite.
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
