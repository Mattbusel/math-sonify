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
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
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
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Construct a new high-pass biquad (RBJ audio cookbook §"HPF").
    ///
    /// Attenuates frequencies below `cutoff_hz` at −12 dB/oct.
    ///
    /// # Parameters
    /// - `cutoff_hz`: −3 dB cutoff frequency in Hz.
    /// - `q`: Quality factor; 0.707 gives a maximally-flat (Butterworth) response.
    /// - `sample_rate`: Audio sample rate in Hz.
    pub fn high_pass(cutoff_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = std::f32::consts::TAU * cutoff_hz / sample_rate;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 + cos_w0) / 2.0 / a0,
            b1: -(1.0 + cos_w0) / a0,
            b2: (1.0 + cos_w0) / 2.0 / a0,
            a1: -2.0 * cos_w0 / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Update high-pass coefficients in place, preserving the filter delay state.
    pub fn update_hp(&mut self, cutoff_hz: f32, q: f32, sample_rate: f32) {
        let cutoff = cutoff_hz.clamp(20.0, sample_rate * 0.45);
        let q_safe = q.max(0.1);
        let new = Self::high_pass(cutoff, q_safe, sample_rate);
        self.b0 = new.b0;
        self.b1 = new.b1;
        self.b2 = new.b2;
        self.a1 = new.a1;
        self.a2 = new.a2;
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
    }

    /// Construct a notch (band-reject) biquad (RBJ audio cookbook §"notch filter").
    ///
    /// Attenuates a narrow band around `center_hz` while passing all other frequencies.
    ///
    /// # Parameters
    /// - `center_hz`: Notch center frequency in Hz.
    /// - `q`: Quality factor; higher Q = narrower notch.
    /// - `sample_rate`: Audio sample rate in Hz.
    pub fn notch(center_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = std::f32::consts::TAU * center_hz / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: 1.0 / a0,
            b1: -2.0 * cos_w0 / a0,
            b2: 1.0 / a0,
            a1: -2.0 * cos_w0 / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Update notch coefficients in place, preserving filter state.
    pub fn update_notch(&mut self, center_hz: f32, q: f32, sample_rate: f32) {
        let center = center_hz.clamp(20.0, sample_rate * 0.45);
        let q_safe = q.max(0.1);
        let new = Self::notch(center, q_safe, sample_rate);
        self.b0 = new.b0;
        self.b1 = new.b1;
        self.b2 = new.b2;
        self.a1 = new.a1;
        self.a2 = new.a2;
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0;
            self.z2 = 0.0;
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
            z1: 0.0,
            z2: 0.0,
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
        if y.is_finite() {
            y
        } else {
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
        self.b0 = new.b0;
        self.b1 = new.b1;
        self.b2 = new.b2;
        self.a1 = new.a1;
        self.a2 = new.a2;
        // Reset state if it has gone NaN/inf
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
    }

    /// Construct a low-shelf biquad filter (RBJ audio cookbook, §"Low Shelf EQ filter").
    ///
    /// Boosts or cuts frequencies below `shelf_hz` by `gain_db` dB.
    /// Q controls the shelf transition slope; 0.707 gives a maximally-flat shelf.
    pub fn low_shelf(shelf_hz: f32, gain_db: f32, q: f32, sample_rate: f32) -> Self {
        let a = 10.0f32.powf(gain_db / 40.0); // sqrt(10^(dB/20))
        let w0 = std::f32::consts::TAU * shelf_hz / sample_rate;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / 2.0 * (a + 1.0 / a).sqrt() / q.max(0.1);
        let sq = 2.0 * a.sqrt() * alpha;
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + sq;
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + sq) / a0;
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0) / a0;
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - sq) / a0;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0) / a0;
        let a2 = ((a + 1.0) + (a - 1.0) * cos_w0 - sq) / a0;
        Self { b0, b1, b2, a1, a2, z1: 0.0, z2: 0.0 }
    }

    /// Construct a high-shelf biquad filter (RBJ audio cookbook, §"High Shelf EQ filter").
    ///
    /// Boosts or cuts frequencies above `shelf_hz` by `gain_db` dB.
    pub fn high_shelf(shelf_hz: f32, gain_db: f32, q: f32, sample_rate: f32) -> Self {
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = std::f32::consts::TAU * shelf_hz / sample_rate;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / 2.0 * (a + 1.0 / a).sqrt() / q.max(0.1);
        let sq = 2.0 * a.sqrt() * alpha;
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + sq;
        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + sq) / a0;
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0) / a0;
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - sq) / a0;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0) / a0;
        let a2 = ((a + 1.0) - (a - 1.0) * cos_w0 - sq) / a0;
        Self { b0, b1, b2, a1, a2, z1: 0.0, z2: 0.0 }
    }

    /// Update low-shelf coefficients in-place, preserving delay state.
    pub fn update_low_shelf(&mut self, shelf_hz: f32, gain_db: f32, q: f32, sample_rate: f32) {
        let new = Self::low_shelf(shelf_hz, gain_db, q, sample_rate);
        self.b0 = new.b0; self.b1 = new.b1; self.b2 = new.b2;
        self.a1 = new.a1; self.a2 = new.a2;
        if !self.z1.is_finite() || !self.z2.is_finite() { self.z1 = 0.0; self.z2 = 0.0; }
    }

    /// Update high-shelf coefficients in-place, preserving delay state.
    pub fn update_high_shelf(&mut self, shelf_hz: f32, gain_db: f32, q: f32, sample_rate: f32) {
        let new = Self::high_shelf(shelf_hz, gain_db, q, sample_rate);
        self.b0 = new.b0; self.b1 = new.b1; self.b2 = new.b2;
        self.a1 = new.a1; self.a2 = new.a2;
        if !self.z1.is_finite() || !self.z2.is_finite() { self.z1 = 0.0; self.z2 = 0.0; }
    }

    /// Reset the delay-line state to zero if it has gone non-finite.
    pub fn reset_if_nan(&mut self) {
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
    }

    /// Update band-pass coefficients in place, preserving filter state.
    /// Use this instead of creating a new filter to avoid resetting z1/z2 state.
    pub fn update_bp(&mut self, center_hz: f32, q: f32, sample_rate: f32) {
        let center = center_hz.clamp(20.0, sample_rate * 0.45);
        let q_safe = q.max(0.1);
        let new = Self::band_pass(center, q_safe, sample_rate);
        self.b0 = new.b0;
        self.b1 = new.b1;
        self.b2 = new.b2;
        self.a1 = new.a1;
        self.a2 = new.a2;
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    /// Feed `n` samples of DC (value 1.0) through the filter and return the last output.
    fn feed_dc(filt: &mut BiquadFilter, n: usize) -> f32 {
        let mut out = 0.0;
        for _ in 0..n {
            out = filt.process(1.0);
        }
        out
    }

    /// Compute RMS of filter output for a pure sine at `freq_hz`.
    fn sine_rms(filt: &mut BiquadFilter, freq_hz: f32, n: usize) -> f32 {
        let mut sum_sq = 0.0f32;
        let dt = std::f32::consts::TAU * freq_hz / SR;
        for i in 0..n {
            let x = (dt * i as f32).sin();
            let y = filt.process(x);
            sum_sq += y * y;
        }
        (sum_sq / n as f32).sqrt()
    }

    #[test]
    fn test_low_pass_passes_dc() {
        // A low-pass filter with a high cutoff should let DC through.
        let mut filt = BiquadFilter::low_pass(10000.0, 0.707, SR);
        let out = feed_dc(&mut filt, 8000);
        assert!(
            out > 0.9,
            "Low-pass should pass DC (output near 1.0), got {}",
            out
        );
    }

    #[test]
    fn test_low_pass_attenuates_high_freq() {
        // A low-pass at 500 Hz should heavily attenuate a 10 kHz sine.
        let mut filt = BiquadFilter::low_pass(500.0, 0.707, SR);
        let rms = sine_rms(&mut filt, 10000.0, 8000);
        assert!(
            rms < 0.1,
            "Low-pass at 500 Hz should attenuate 10 kHz, RMS={}",
            rms
        );
    }

    #[test]
    fn test_band_pass_has_peak_at_center() {
        // A band-pass filter should pass the center frequency better than far-off frequencies.
        let center = 1000.0_f32;
        let mut filt_center = BiquadFilter::band_pass(center, 2.0, SR);
        let rms_center = sine_rms(&mut filt_center, center, 4000);

        let mut filt_high = BiquadFilter::band_pass(center, 2.0, SR);
        let rms_high = sine_rms(&mut filt_high, 10000.0, 4000);

        assert!(
            rms_center > rms_high,
            "Band-pass should pass center freq ({}) better than 10 kHz ({} vs {})",
            center,
            rms_center,
            rms_high
        );
    }

    #[test]
    fn test_filter_outputs_are_finite() {
        // No input should ever produce NaN or Inf from a biquad filter.
        let mut lp = BiquadFilter::low_pass(1000.0, 0.707, SR);
        let mut bp = BiquadFilter::band_pass(1000.0, 2.0, SR);
        for i in 0..8000 {
            let x = (i as f32 * 0.1).sin() * 10.0; // intentionally large signal
            assert!(
                lp.process(x).is_finite(),
                "LP output non-finite at sample {}",
                i
            );
            assert!(
                bp.process(x).is_finite(),
                "BP output non-finite at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_filter_nan_input_cleared() {
        // A NaN input sample should not corrupt the filter permanently.
        let mut filt = BiquadFilter::low_pass(1000.0, 0.707, SR);
        let _ = filt.process(f32::NAN);
        // After the NaN, normal input should produce finite output.
        let out = filt.process(1.0);
        assert!(
            out.is_finite(),
            "Filter should recover from NaN input, got {}",
            out
        );
    }
}
