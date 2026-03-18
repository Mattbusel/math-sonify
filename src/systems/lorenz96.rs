use super::{rk4, DynamicalSystem};
use std::f64::consts::PI;

/// Lorenz-96: dx_i/dt = (x_{i+1} - x_{i-2})*x_{i-1} - x_i + F_i
/// N=8 oscillators, periodic boundary.
/// Supports homogeneous (scalar F) or heterogeneous (per-oscillator forcing) modes.
pub struct Lorenz96 {
    state: Vec<f64>,
    pub f: f64,
    pub n: usize,
    speed: f64,
    /// Per-oscillator forcing values. When homogeneous, all entries equal `f`.
    forcing: Vec<f64>,
}

impl Lorenz96 {
    /// Creates a Lorenz-96 instance with N=8 oscillators and forcing F=8.0.
    ///
    /// The perturbation x[0]=0.01 breaks perfect symmetry to seed interesting dynamics.
    /// F=8.0 is the classical chaotic regime; F<5 gives periodic behaviour.
    pub fn new() -> Self {
        let n = 8;
        let f = 8.0;
        let mut state = vec![0.0f64; n];
        state[0] = 0.01;
        let forcing = vec![f; n];
        Self {
            state,
            f,
            n,
            speed: 0.0,
            forcing,
        }
    }

    /// Construct with heterogeneous forcing: F_i = f_mean + f_spread * sin(2π·i/n).
    pub fn with_forcing(n: usize, f_mean: f64, f_spread: f64) -> Self {
        let n = n.max(4);
        let mut state = vec![0.0f64; n];
        state[0] = 0.01;
        let forcing: Vec<f64> = (0..n)
            .map(|i| f_mean + f_spread * (2.0 * PI * i as f64 / n as f64).sin())
            .collect();
        Self {
            state,
            f: f_mean,
            n,
            speed: 0.0,
            forcing,
        }
    }

    fn deriv_het(s: &[f64], forcing: &[f64]) -> Vec<f64> {
        let n = s.len();
        (0..n)
            .map(|i| {
                let xm2 = s[(i + n - 2) % n];
                let xm1 = s[(i + n - 1) % n];
                let xp1 = s[(i + 1) % n];
                (xp1 - xm2) * xm1 - s[i] + forcing[i]
            })
            .collect()
    }

    fn deriv(s: &[f64], f_forcing: f64) -> Vec<f64> {
        let n = s.len();
        (0..n)
            .map(|i| {
                let xm2 = s[(i + n - 2) % n];
                let xm1 = s[(i + n - 1) % n];
                let xp1 = s[(i + 1) % n];
                (xp1 - xm2) * xm1 - s[i] + f_forcing
            })
            .collect()
    }
}

impl DynamicalSystem for Lorenz96 {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        self.n
    }
    fn name(&self) -> &str {
        "lorenz96"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv_het(state, &self.forcing)
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
        let forcing = self.forcing.clone();
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv_het(s, &forcing));
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
