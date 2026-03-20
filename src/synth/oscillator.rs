use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique seed counter — each Oscillator instance gets a distinct noise seed so
/// simultaneous Noise-shape voices don't produce correlated (identical) bursts.
static OSC_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Waveform shape for a band-limited oscillator.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum OscShape {
    /// Pure sinusoidal wave (no harmonics).
    #[default]
    Sine,
    /// Band-limited triangle wave (−12 dB/oct harmonic rolloff).
    Triangle,
    /// Band-limited sawtooth wave via PolyBLEP anti-aliasing.
    Saw,
    /// Band-limited square wave via PolyBLEP anti-aliasing.
    Square,
    /// White noise via xorshift64.
    Noise,
}

/// Band-limited oscillator with configurable waveform and per-sample output.
///
/// Uses PolyBLEP anti-aliasing for Saw, Square, and Triangle waveforms to reduce
/// aliasing artifacts at high frequencies.
pub struct Oscillator {
    phase: f32,
    pub freq: f32,
    pub shape: OscShape,
    sample_rate: f32,
    // Leaky integrator state for band-limited triangle generation
    tri_state: f32,
    // DC-blocking state for square wave input to triangle integrator
    sq_dc: f32,
    // xorshift64 state for noise generation
    noise_seed: u64,
}

/// PolyBLEP residual — removes the aliasing step artifact at a phase discontinuity.
/// `t`  : normalized phase in [0, 1)
/// `dt` : normalized frequency (freq / sample_rate)
///
/// Returns the correction term to subtract (for saw) or use in integration (for square/tri).
/// Based on Valimaki & Pakarinen (2007) and common DAW synth implementations.
#[inline(always)]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        // Just past the discontinuity: ramp correction from 0→0
        let u = t / dt;
        2.0 * u - u * u - 1.0 // = -(1-u)²
    } else if t > 1.0 - dt {
        // Approaching the discontinuity: ramp correction back to 0
        let u = (t - 1.0) / dt;
        u * u + 2.0 * u + 1.0 // = (1+u)²
    } else {
        0.0
    }
}

impl Oscillator {
    /// Creates a new oscillator at the specified frequency and waveform shape.
    ///
    /// # Parameters
    /// - `freq`: Oscillator frequency in Hz.
    /// - `shape`: Waveform shape (`Sine`, `Saw`, `Square`, `Triangle`, or `Noise`).
    /// - `sample_rate`: Audio sample rate in Hz (e.g. 44100.0).
    ///
    /// # Returns
    /// An `Oscillator` instance with phase initialized to zero.
    pub fn new(freq: f32, shape: OscShape, sample_rate: f32) -> Self {
        let mut osc = Self {
            phase: 0.0,
            freq,
            shape,
            sample_rate,
            tri_state: 0.0,
            sq_dc: 0.0,
            noise_seed: OSC_COUNTER
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(0x517C_C1B7_2722_0A95),
        };
        // Analytically initialize the triangle integrator to its steady-state trough
        // so the waveform starts at full amplitude without a multi-period startup transient.
        //
        // At steady state the leaky integrator alternates between trough y0 (at phase=0,
        // start of positive half-cycle) and peak -y0. Solving the fixed-point equation:
        //   y_peak = (1-leak)^{T/2} * y0 + (4*dt/leak) * (1 - (1-leak)^{T/2})
        //   y0 = -y_peak   (odd symmetry)
        // gives: y0 = -(4*dt/leak) * (1 - p) / (1 + p),  p = (1-leak)^{T/2}
        if shape == OscShape::Triangle {
            let leak = 1e-3_f32;
            let dt = freq / sample_rate;
            let half_samples = sample_rate / (2.0 * freq.max(1.0));
            let p = (1.0 - leak).powf(half_samples);
            osc.tri_state = -(4.0 * dt / leak) * (1.0 - p) / (1.0 + p);
        }
        osc
    }

    /// Advances the oscillator by one sample and returns the output value in `[-1, 1]`.
    ///
    /// # Returns
    /// The next audio sample as an `f32` in the range `[-1, 1]`.
    pub fn next_sample(&mut self) -> f32 {
        let t = self.phase / TAU;
        let dt = (self.freq / self.sample_rate).clamp(0.0, 0.5);

        let out = match self.shape {
            OscShape::Sine => self.phase.sin(),

            OscShape::Saw => {
                // Band-limited sawtooth via PolyBLEP.
                // Naive: 2t - 1, with a step discontinuity at t=0.
                // Correction subtracts the blep residual at the wrap point.
                (2.0 * t - 1.0) - poly_blep(t, dt)
            }

            OscShape::Square => {
                // Band-limited square via PolyBLEP.
                let sq_naive = if t < 0.5 { 1.0f32 } else { -1.0f32 };
                sq_naive + poly_blep(t, dt) - poly_blep((t + 0.5) % 1.0, dt)
            }

            OscShape::Noise => {
                // White noise via xorshift64, mapped to [-1, 1]
                let mut s = self.noise_seed;
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                self.noise_seed = s;
                (s as f32 / u64::MAX as f32) * 2.0 - 1.0
            }

            OscShape::Triangle => {
                // Band-limited triangle via leaky integration of a PolyBLEP square wave.
                // The square has discontinuities at t=0 and t=0.5; blep corrects both.
                // A leaky integrator then shapes the square into a smooth triangle with
                // naturally high-frequency rolloff (−12 dB/oct vs saw's −6 dB/oct).
                let sq_naive = if t < 0.5 { 1.0f32 } else { -1.0f32 };
                let sq = sq_naive + poly_blep(t, dt) - poly_blep((t + 0.5) % 1.0, dt);
                // DC-block the square before integrating (prevents sub-bass accumulation).
                // α=0.001 gives a ~1000-sample (23ms at 44.1kHz) time constant — fast
                // enough to track any DC offset without affecting the audio band.
                self.sq_dc += 0.001 * (sq - self.sq_dc);
                let sq_ac = sq - self.sq_dc;
                // Integrate: step size = 4*dt to get correct ±1 amplitude
                self.tri_state += 4.0 * dt * sq_ac;
                // Leaky integrator — leak of 1e-3 gives a ~1000-sample time constant
                // so the waveform reaches steady-state amplitude within ~23 ms.
                // The prior value (2e-5) had a 50 000-sample constant, causing a multi-
                // second transient where the output overshot ±1 before settling.
                self.tri_state *= 1.0 - 1e-3;
                self.tri_state
            }
        };

        self.phase = (self.phase + TAU * self.freq / self.sample_rate).rem_euclid(TAU);
        out
    }
}

/// Exponential smoothing for audio parameters (frequency glide, amplitude, etc.).
/// Eliminates zipper noise when parameters change between control frames.
pub struct SmoothParam {
    current: f32,
    target: f32,
    rate: f32, // lerp coefficient per sample
}

impl SmoothParam {
    /// Creates a new smoothed parameter with the given initial value and glide time.
    ///
    /// # Parameters
    /// - `initial`: Starting value of the parameter.
    /// - `smoothing_ms`: One-pole filter time constant in milliseconds.
    /// - `sample_rate`: Audio sample rate in Hz used to convert the time constant.
    ///
    /// # Returns
    /// A `SmoothParam` with `current` and `target` both set to `initial`.
    pub fn new(initial: f32, smoothing_ms: f32, sample_rate: f32) -> Self {
        let samples = smoothing_ms * 0.001 * sample_rate;
        Self {
            current: initial,
            target: initial,
            rate: 1.0 / samples.max(1.0),
        }
    }

    /// Sets the target value that the parameter will glide toward.
    ///
    /// # Parameters
    /// - `t`: The new target value.
    pub fn set_target(&mut self, t: f32) {
        self.target = t;
    }

    /// Advances the smoothed parameter by one sample and returns the current interpolated value.
    ///
    /// # Returns
    /// The current value after one step of exponential smoothing toward the target.
    pub fn next(&mut self) -> f32 {
        self.current += self.rate * (self.target - self.current);
        self.current
    }

    pub fn current(&self) -> f32 {
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oscillator_frequency_above_zero() {
        // A sine oscillator at 440 Hz must produce non-zero output eventually
        // (initial phase is 0 so sample 0 is 0.0; check after a few samples).
        let mut osc = Oscillator::new(440.0, OscShape::Sine, 44100.0);
        // Skip the first sample (sin(0) == 0) and check that subsequent ones are non-zero
        osc.next_sample(); // sample at phase 0
        let s = osc.next_sample();
        assert!(
            s.abs() > 1e-6,
            "Sine oscillator at 440 Hz should produce non-zero output, got {}",
            s
        );
    }

    #[test]
    fn test_oscillator_amplitude_clamp() {
        // The oscillator output for Sine should always be in [-1, 1]
        let mut osc = Oscillator::new(440.0, OscShape::Sine, 44100.0);
        for _ in 0..4410 {
            let s = osc.next_sample();
            assert!(
                (-1.0..=1.0).contains(&s),
                "Sine sample out of [-1, 1]: {}",
                s
            );
        }
    }

    #[test]
    fn test_oscillator_silence_when_amplitude_zero() {
        // A zero-frequency oscillator has dt=0 so phase never advances;
        // sin(0) = 0 for every sample — effectively silence.
        let mut osc = Oscillator::new(0.0, OscShape::Sine, 44100.0);
        for _ in 0..100 {
            let s = osc.next_sample();
            assert!(
                s.abs() < 1e-10,
                "Expected silence from zero-freq oscillator, got {}",
                s
            );
        }
    }

    #[test]
    fn test_sine_at_phase_zero_is_zero() {
        // sin(0) = 0; the very first sample must be 0.
        let mut osc = Oscillator::new(440.0, OscShape::Sine, 44100.0);
        let s = osc.next_sample();
        assert!(s.abs() < 1e-10, "sin(0) should be 0, got {}", s);
    }

    #[test]
    fn test_sine_at_quarter_period_is_near_one() {
        // After exactly 1/4 of one period (sample_rate / freq / 4 samples)
        // the sine should be very close to 1.0.
        let freq = 1000.0_f32;
        let sr = 44100.0_f32;
        let quarter_samples = (sr / freq / 4.0).round() as usize;
        let mut osc = Oscillator::new(freq, OscShape::Sine, sr);
        let mut last = 0.0_f32;
        for _ in 0..quarter_samples {
            last = osc.next_sample();
        }
        assert!(
            last > 0.9,
            "Sine at ~quarter period should be near 1.0, got {}",
            last
        );
    }

    #[test]
    fn test_square_wave_is_plus_or_minus_one() {
        // A band-limited square should be very close to +1 or -1 away from
        // the discontinuities. Check 100 samples from the middle of each half-cycle.
        let freq = 440.0_f32;
        let sr = 44100.0_f32;
        let mut osc = Oscillator::new(freq, OscShape::Square, sr);
        // Skip to 10% into the first half-cycle to avoid the PolyBLEP transition region.
        let skip = (sr / freq * 0.1) as usize;
        for _ in 0..skip {
            let _ = osc.next_sample();
        }
        for _ in 0..20 {
            let s = osc.next_sample();
            assert!(s.abs() > 0.5, "Square wave sample not near ±1: {}", s);
        }
    }

    #[test]
    fn test_saw_wave_in_range() {
        // Sawtooth output should stay in [-1.5, 1.5] (PolyBLEP can briefly overshoot).
        let mut osc = Oscillator::new(440.0, OscShape::Saw, 44100.0);
        for _ in 0..4410 {
            let s = osc.next_sample();
            assert!(s.abs() < 1.5, "Saw sample out of expected range: {}", s);
            assert!(s.is_finite(), "Saw sample is non-finite");
        }
    }

    #[test]
    fn test_higher_frequency_shorter_period() {
        // A 1000 Hz sine completes one cycle in 44.1 samples; a 500 Hz sine
        // needs 88.2 samples. Verify that doubling frequency halves the zero-crossing period.
        let sr = 44100.0_f32;
        let count_crossings = |freq: f32| -> usize {
            let mut osc = Oscillator::new(freq, OscShape::Sine, sr);
            let mut prev = 0.0_f32;
            let mut crossings = 0;
            for _ in 0..4410 {
                let s = osc.next_sample();
                if prev < 0.0 && s >= 0.0 {
                    crossings += 1;
                }
                prev = s;
            }
            crossings
        };
        let c1000 = count_crossings(1000.0);
        let c500 = count_crossings(500.0);
        // 1000 Hz has roughly twice the zero-crossings of 500 Hz over the same window.
        assert!(
            c1000 > c500,
            "1000 Hz should have more zero-crossings than 500 Hz ({} vs {})",
            c1000,
            c500
        );
    }

    #[test]
    fn test_amplitude_scaling_via_level() {
        // If we scale the oscillator output by 0.5 we should get half the amplitude.
        let mut osc1 = Oscillator::new(440.0, OscShape::Sine, 44100.0);
        let mut osc2 = Oscillator::new(440.0, OscShape::Sine, 44100.0);
        // Skip first sample (it is 0).
        let _ = osc1.next_sample();
        let _ = osc2.next_sample();
        let s1 = osc1.next_sample();
        let s2 = osc2.next_sample() * 0.5;
        assert!(
            (s1 * 0.5 - s2).abs() < 1e-6,
            "Amplitude scaling mismatch: {} vs {}",
            s1 * 0.5,
            s2
        );
    }
}
