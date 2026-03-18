use super::{rk4, DynamicalSystem};

/// Sprott B system: dx/dt = y*z, dy/dt = x - y, dz/dt = 1 - x*y
pub struct SprottB {
    state: Vec<f64>,
    speed: f64,
}

impl SprottB {
    /// Creates a Sprott B attractor with initial state (0, 3, 0).
    ///
    /// The Sprott B system is one of the simplest three-dimensional chaotic
    /// flows with only quadratic nonlinearities.  It has no free parameters;
    /// the strange attractor is an invariant set of the fixed equations.
    pub fn new() -> Self {
        Self {
            state: vec![0.0, 3.0, 0.0],
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64]) -> Vec<f64> {
        vec![s[1] * s[2], s[0] - s[1], 1.0 - s[0] * s[1]]
    }
}

impl DynamicalSystem for SprottB {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "sprott_b"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state)
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
        let prev = self.state.clone();
        rk4(&mut self.state, dt, Self::deriv);
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
