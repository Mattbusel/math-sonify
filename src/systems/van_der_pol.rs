use super::{rk4, DynamicalSystem};

/// Van der Pol self-sustaining limit-cycle oscillator.
///
/// Equations (μ = 2.0 by default):
/// ```text
/// dx/dt = y
/// dy/dt = μ·(1 - x²)·y - x
/// ```
/// For μ > 0 the system has a stable limit cycle; larger μ gives increasingly
/// relaxation-oscillator-like behavior (sharp transitions).
pub struct VanDerPol {
    state: Vec<f64>,
    pub mu: f64,
}

impl VanDerPol {
    /// Create a Van der Pol oscillator with default nonlinearity μ = 2.0.
    pub fn new() -> Self {
        Self {
            state: vec![2.0, 0.0],
            mu: 2.0,
        }
    }

    fn deriv(state: &[f64], mu: f64) -> Vec<f64> {
        let x = state[0];
        let y = state[1];
        vec![y, mu * (1.0 - x * x) * y - x]
    }
}

impl DynamicalSystem for VanDerPol {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        2
    }
    fn name(&self) -> &str {
        "Van der Pol"
    }

    fn step(&mut self, dt: f64) {
        let mu = self.mu;
        rk4(&mut self.state, dt, |s| Self::deriv(s, mu));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.mu)
    }

    fn speed(&self) -> f64 {
        let d = self.current_deriv();
        d.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}
