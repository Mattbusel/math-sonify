use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum OscShape { #[default] Sine, Triangle, Saw }

pub struct Oscillator {
    phase: f32,
    pub freq: f32,
    pub shape: OscShape,
    sample_rate: f32,
}

impl Oscillator {
    pub fn new(freq: f32, shape: OscShape, sample_rate: f32) -> Self {
        Self { phase: 0.0, freq, shape, sample_rate }
    }

    pub fn next_sample(&mut self) -> f32 {
        let out = match self.shape {
            OscShape::Sine => self.phase.sin(),
            OscShape::Triangle => {
                let t = self.phase / TAU;
                1.0 - 4.0 * (t - (t + 0.5).floor() + 0.5).abs()
            }
            OscShape::Saw => {
                let t = self.phase / TAU;
                2.0 * (t - (t + 0.5).floor())
            }
        };
        self.phase = (self.phase + TAU * self.freq / self.sample_rate).rem_euclid(TAU);
        out
    }
}

/// Linear interpolation between two frequencies, updated each control frame.
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
