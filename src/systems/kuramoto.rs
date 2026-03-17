use super::{DynamicalSystem, rk4};

/// Kuramoto model: N coupled phase oscillators.
/// dθᵢ/dt = ωᵢ + (K/N) Σⱼ sin(θⱼ - θᵢ)
/// Natural frequencies ωᵢ drawn from a Lorentzian distribution.
pub struct Kuramoto {
    state: Vec<f64>,     // phases θᵢ
    omega: Vec<f64>,     // natural frequencies
    pub coupling: f64,   // K
    pub n: usize,
    speed: f64,
    order_param: f64,    // synchronization order parameter r ∈ [0,1]
}

impl Kuramoto {
    pub fn new(n: usize, coupling: f64) -> Self {
        // Lorentzian natural frequencies centered at 1.0 with width 0.5
        let omega: Vec<f64> = (0..n).map(|i| {
            let u = (i as f64 + 0.5) / n as f64;
            // Lorentzian quantile: tan(π(u-0.5)).
            // Clamp u away from 0 and 1 to avoid tan(±π/2) = ±∞, which would
            // produce infinite natural frequencies and NaN state after one step.
            let u_safe = u.clamp(1e-6, 1.0 - 1e-6);
            1.0 + 0.5 * (std::f64::consts::PI * (u_safe - 0.5)).tan()
        }).collect();
        // Distribute initial phases uniformly
        let state: Vec<f64> = (0..n).map(|i| {
            2.0 * std::f64::consts::PI * i as f64 / n as f64
        }).collect();
        Self { state, omega, coupling, n, speed: 0.0, order_param: 0.0 }
    }

    /// Kuramoto order parameter r = |Σ exp(iθⱼ)| / N ∈ [0,1]
    pub fn order_parameter(&self) -> f64 { self.order_param }

    fn compute_deriv(state: &[f64], omega: &[f64], coupling: f64) -> Vec<f64> {
        let n = state.len();
        let k_over_n = coupling / n as f64;
        (0..n).map(|i| {
            let coupling_sum: f64 = state.iter().map(|&th_j| (th_j - state[i]).sin()).sum();
            omega[i] + k_over_n * coupling_sum
        }).collect()
    }
}

impl DynamicalSystem for Kuramoto {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { self.n }
    fn name(&self) -> &str { "Kuramoto" }
    fn speed(&self) -> f64 { self.speed }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> { Self::compute_deriv(state, &self.omega, self.coupling) }

    fn step(&mut self, dt: f64) {
        let omega = self.omega.clone();
        let coupling = self.coupling;
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::compute_deriv(s, &omega, coupling));
        // Wrap phases
        for th in &mut self.state {
            *th = th.rem_euclid(std::f64::consts::TAU);
        }
        // Update order parameter
        let (sin_sum, cos_sum): (f64, f64) = self.state.iter()
            .fold((0.0, 0.0), |(s, c), &th| (s + th.sin(), c + th.cos()));
        self.order_param = (sin_sum.powi(2) + cos_sum.powi(2)).sqrt() / self.n as f64;
        let ds: f64 = self.state.iter().zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
        self.speed = ds / dt;
    }
}
