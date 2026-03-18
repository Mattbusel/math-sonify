use super::{rk4, DynamicalSystem};

/// Nose-Hoover thermostat: dx/dt = y, dy/dt = -x + y*z, dz/dt = a - y²
pub struct NoseHoover {
    state: Vec<f64>,
    pub a: f64,
    speed: f64,
}

impl NoseHoover {
    /// Creates a Nosé-Hoover thermostat with default parameter a=3.0.
    ///
    /// The initial state (x=0, y=5, z=0) places the trajectory far from the
    /// fixed point so that the attractor is immediately engaged.
    /// The `a` parameter controls the coupling strength to the thermal reservoir;
    /// a=3.0 gives a persistent chaotic torus-like attractor.
    pub fn new() -> Self {
        Self {
            state: vec![0.0, 5.0, 0.0],
            a: 3.0,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64) -> Vec<f64> {
        vec![s[1], -s[0] + s[1] * s[2], a - s[1] * s[1]]
    }
}

impl DynamicalSystem for NoseHoover {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "nose_hoover"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a)
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }

    fn step(&mut self, dt: f64) {
        let a = self.a;
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, a));
        let ds: f64 = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = ds / dt;
    }
}
