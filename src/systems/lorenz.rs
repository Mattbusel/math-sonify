use super::{DynamicalSystem, rk4};

/// Lorenz attractor: dx/dt = σ(y-x), dy/dt = x(ρ-z)-y, dz/dt = xy-βz
pub struct Lorenz {
    state: Vec<f64>,
    pub sigma: f64,
    pub rho: f64,
    pub beta: f64,
    speed: f64,
}

impl Lorenz {
    pub fn new(sigma: f64, rho: f64, beta: f64) -> Self {
        Self {
            state: vec![1.0, 0.0, 0.0],
            sigma,
            rho,
            beta,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], sigma: f64, rho: f64, beta: f64) -> Vec<f64> {
        vec![
            sigma * (s[1] - s[0]),
            s[0] * (rho - s[2]) - s[1],
            s[0] * s[1] - beta * s[2],
        ]
    }
}

impl DynamicalSystem for Lorenz {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 3 }
    fn name(&self) -> &str { "Lorenz" }
    fn speed(&self) -> f64 { self.speed }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> { Self::deriv(state, self.sigma, self.rho, self.beta) }
    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() { self.state[i] = s[i]; }
        }
    }

    fn step(&mut self, dt: f64) {
        let (sigma, rho, beta) = (self.sigma, self.rho, self.beta);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, sigma, rho, beta));
        // Estimate speed as |Δstate|/dt
        let ds: f64 = self.state.iter().zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
        self.speed = ds / dt;
    }
}
