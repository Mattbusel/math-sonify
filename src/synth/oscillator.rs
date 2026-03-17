use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum OscShape { #[default] Sine, Triangle, Saw }

pub struct Oscillator {
    phase: f32,
    pub freq: f32,
    pub shape: OscShape,
    sample_rate: f32,
    // Leaky integrator state for band-limited triangle generation
    tri_state: f32,
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
        2.0 * u - u * u - 1.0   // = -(1-u)²
    } else if t > 1.0 - dt {
        // Approaching the discontinuity: ramp correction back to 0
        let u = (t - 1.0) / dt;
        u * u + 2.0 * u + 1.0   // = (1+u)²
    } else {
        0.0
    }
}

impl Oscillator {
    pub fn new(freq: f32, shape: OscShape, sample_rate: f32) -> Self {
        Self { phase: 0.0, freq, shape, sample_rate, tri_state: 0.0 }
    }

    pub fn next_sample(&mut self) -> f32 {
        let t  = self.phase / TAU;
        let dt = (self.freq / self.sample_rate).clamp(0.0, 0.5);

        let out = match self.shape {
            OscShape::Sine => self.phase.sin(),

            OscShape::Saw => {
                // Band-limited sawtooth via PolyBLEP.
                // Naive: 2t - 1, with a step discontinuity at t=0.
                // Correction subtracts the blep residual at the wrap point.
                (2.0 * t - 1.0) - poly_blep(t, dt)
            }

            OscShape::Triangle => {
                // Band-limited triangle via leaky integration of a PolyBLEP square wave.
                // The square has discontinuities at t=0 and t=0.5; blep corrects both.
                // A leaky integrator then shapes the square into a smooth triangle with
                // naturally high-frequency rolloff (−12 dB/oct vs saw's −6 dB/oct).
                let sq_naive = if t < 0.5 { 1.0f32 } else { -1.0f32 };
                let sq = sq_naive
                    + poly_blep(t, dt)
                    - poly_blep((t + 0.5) % 1.0, dt);
                // Integrate: step size = 4*dt to get correct ±1 amplitude
                self.tri_state += 4.0 * dt * sq;
                // Gentle DC-blocking leak (removes integrator drift)
                self.tri_state *= 1.0 - 1e-4;
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
    pub fn new(initial: f32, smoothing_ms: f32, sample_rate: f32) -> Self {
        let samples = smoothing_ms * 0.001 * sample_rate;
        Self { current: initial, target: initial, rate: 1.0 / samples.max(1.0) }
    }

    pub fn set_target(&mut self, t: f32) { self.target = t; }

    pub fn next(&mut self) -> f32 {
        self.current += self.rate * (self.target - self.current);
        self.current
    }

    pub fn current(&self) -> f32 { self.current }
}
