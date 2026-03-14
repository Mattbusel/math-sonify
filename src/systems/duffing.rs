use super::{DynamicalSystem, rk4};

pub struct Duffing {
    state: Vec<f64>,
    pub delta: f64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub omega: f64,
}

impl Duffing {
    pub fn new() -> Self {
        Self {
            state: vec![1.0, 0.0, 0.0],
            delta: 0.3,
            alpha: -1.0,
            beta: 1.0,
            gamma: 0.5,
            omega: 1.2,
        }
    }

    fn deriv(state: &[f64], delta: f64, alpha: f64, beta: f64, gamma: f64, omega: f64) -> Vec<f64> {
        let x = state[0];
        let v = state[1];
        let phase = state[2];
        vec![
            v,
            -delta * v - alpha * x - beta * x * x * x + gamma * phase.cos(),
            omega,
        ]
    }
}

impl DynamicalSystem for Duffing {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 3 }
    fn name(&self) -> &str { "Duffing" }

    fn step(&mut self, dt: f64) {
        let (delta, alpha, beta, gamma, omega) = (self.delta, self.alpha, self.beta, self.gamma, self.omega);
        rk4(&mut self.state, dt, |s| Self::deriv(s, delta, alpha, beta, gamma, omega));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.delta, self.alpha, self.beta, self.gamma, self.omega)
    }

    fn speed(&self) -> f64 {
        let d = self.current_deriv();
        d.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}
