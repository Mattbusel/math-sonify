use super::{DynamicalSystem, rk4};

pub struct Halvorsen {
    state: Vec<f64>,
    pub a: f64,
}

impl Halvorsen {
    pub fn new() -> Self {
        Self { state: vec![-5.0, 0.0, 0.0], a: 1.89 }
    }

    fn deriv(state: &[f64], a: f64) -> Vec<f64> {
        let x = state[0];
        let y = state[1];
        let z = state[2];
        vec![
            -a * x - 4.0 * y - 4.0 * z - y * y,
            -a * y - 4.0 * z - 4.0 * x - z * z,
            -a * z - 4.0 * x - 4.0 * y - x * x,
        ]
    }
}

impl DynamicalSystem for Halvorsen {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 3 }
    fn name(&self) -> &str { "Halvorsen" }

    fn step(&mut self, dt: f64) {
        let a = self.a;
        rk4(&mut self.state, dt, |s| Self::deriv(s, a));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a)
    }

    fn speed(&self) -> f64 {
        let d = self.current_deriv();
        d.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}
