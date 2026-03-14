use super::{DynamicalSystem, rk4};

pub struct Aizawa {
    state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Aizawa {
    pub fn new() -> Self {
        Self {
            state: vec![0.1, 0.0, 0.0],
            a: 0.95,
            b: 0.7,
            c: 0.6,
            d: 3.5,
            e: 0.25,
            f: 0.1,
        }
    }

    fn deriv(state: &[f64], a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Vec<f64> {
        let x = state[0];
        let y = state[1];
        let z = state[2];
        vec![
            (z - b) * x - d * y,
            d * x + (z - b) * y,
            c + a * z - z * z * z / 3.0 - (x * x + y * y) * (1.0 + e * z) + f * z * x * x * x,
        ]
    }
}

impl DynamicalSystem for Aizawa {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 3 }
    fn name(&self) -> &str { "Aizawa" }

    fn step(&mut self, dt: f64) {
        let (a, b, c, d, e, f) = (self.a, self.b, self.c, self.d, self.e, self.f);
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, c, d, e, f));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b, self.c, self.d, self.e, self.f)
    }

    fn speed(&self) -> f64 {
        let d = self.current_deriv();
        d.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}
