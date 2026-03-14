use super::{DynamicalSystem, rk4};

pub struct Chua {
    state: Vec<f64>,
    pub alpha: f64,
    pub beta: f64,
    pub m0: f64,
    pub m1: f64,
}

impl Chua {
    pub fn new() -> Self {
        Self {
            state: vec![0.7, 0.0, 0.0],
            alpha: 15.6,
            beta: 28.0,
            m0: -1.143,
            m1: -0.714,
        }
    }

    fn h(x: f64, m0: f64, m1: f64) -> f64 {
        m1 * x + 0.5 * (m0 - m1) * ((x + 1.0).abs() - (x - 1.0).abs())
    }

    fn deriv(state: &[f64], alpha: f64, beta: f64, m0: f64, m1: f64) -> Vec<f64> {
        let x = state[0];
        let y = state[1];
        let z = state[2];
        vec![
            alpha * (y - x - Self::h(x, m0, m1)),
            x - y + z,
            -beta * y,
        ]
    }
}

impl DynamicalSystem for Chua {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 3 }
    fn name(&self) -> &str { "Chua's Circuit" }

    fn step(&mut self, dt: f64) {
        let (alpha, beta, m0, m1) = (self.alpha, self.beta, self.m0, self.m1);
        rk4(&mut self.state, dt, |s| Self::deriv(s, alpha, beta, m0, m1));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.alpha, self.beta, self.m0, self.m1)
    }

    fn speed(&self) -> f64 {
        let d = self.current_deriv();
        d.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}
