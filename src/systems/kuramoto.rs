use super::{rk4, DynamicalSystem};

/// Kuramoto model of coupled phase oscillators (Kuramoto 1975).
///
/// Equations of motion for N oscillators with phases theta_i:
///
///   d(theta_i)/dt = omega_i + (K/N) * sum_j sin(theta_j - theta_i)
///
/// Natural frequencies omega_i are drawn from a Lorentzian distribution with
/// center 1.0 and half-width 0.5, sampled via the quantile function.
///
/// The system undergoes a phase transition from incoherence to synchronization
/// at a critical coupling K_c = 2 * gamma = 1.0 (where gamma=0.5 is the
/// half-width of the frequency distribution).  Above K_c the order parameter
/// r = |sum exp(i*theta_j)| / N approaches 1.
///
/// Integration uses fourth-order Runge-Kutta (RK4); phases are wrapped to
/// [0, 2*pi] after each step to prevent floating-point drift.
pub struct Kuramoto {
    state: Vec<f64>,   // phases θᵢ
    omega: Vec<f64>,   // natural frequencies
    pub coupling: f64, // K
    pub n: usize,
    speed: f64,
    order_param: f64, // synchronization order parameter r ∈ [0,1]
}

impl Kuramoto {
    pub fn new(n: usize, coupling: f64) -> Self {
        // Lorentzian natural frequencies centered at 1.0 with width 0.5
        let omega: Vec<f64> = (0..n)
            .map(|i| {
                let u = (i as f64 + 0.5) / n as f64;
                // Lorentzian quantile: tan(π(u-0.5)).
                // Clamp u away from 0 and 1 to avoid tan(±π/2) = ±∞, which would
                // produce infinite natural frequencies and NaN state after one step.
                let u_safe = u.clamp(1e-6, 1.0 - 1e-6);
                1.0 + 0.5 * (std::f64::consts::PI * (u_safe - 0.5)).tan()
            })
            .collect();
        // Distribute initial phases uniformly
        let state: Vec<f64> = (0..n)
            .map(|i| 2.0 * std::f64::consts::PI * i as f64 / n as f64)
            .collect();
        Self {
            state,
            omega,
            coupling,
            n,
            speed: 0.0,
            order_param: 0.0,
        }
    }

    /// Kuramoto order parameter r = |Σ exp(iθⱼ)| / N ∈ [0,1]
    pub fn order_parameter(&self) -> f64 {
        self.order_param
    }

    fn compute_deriv(state: &[f64], omega: &[f64], coupling: f64) -> Vec<f64> {
        let n = state.len();
        let k_over_n = coupling / n as f64;
        (0..n)
            .map(|i| {
                let coupling_sum: f64 = state.iter().map(|&th_j| (th_j - state[i]).sin()).sum();
                omega[i] + k_over_n * coupling_sum
            })
            .collect()
    }
}

impl DynamicalSystem for Kuramoto {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        self.n
    }
    fn name(&self) -> &str {
        "Kuramoto"
    }
    fn speed(&self) -> f64 {
        self.speed
    }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::compute_deriv(state, &self.omega, self.coupling)
    }

    fn step(&mut self, dt: f64) {
        let omega = self.omega.clone();
        let coupling = self.coupling;
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| {
            Self::compute_deriv(s, &omega, coupling)
        });
        // Wrap phases
        for th in &mut self.state {
            *th = th.rem_euclid(std::f64::consts::TAU);
        }
        // Update order parameter
        let (sin_sum, cos_sum): (f64, f64) = self
            .state
            .iter()
            .fold((0.0, 0.0), |(s, c), &th| (s + th.sin(), c + th.cos()));
        self.order_param = (sin_sum.powi(2) + cos_sum.powi(2)).sqrt() / self.n as f64;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_kuramoto_initial_state() {
        let sys = Kuramoto::new(8, 2.0);
        let s = sys.state();
        assert_eq!(s.len(), 8);
        assert_eq!(sys.dimension(), 8);
        assert_eq!(sys.name(), "Kuramoto");
        assert!(s.iter().all(|v| v.is_finite()));
        // Phases should be in [0, 2π)
        for &th in s.iter() {
            assert!(th >= 0.0 && th < std::f64::consts::TAU, "Phase out of range: {}", th);
        }
    }

    #[test]
    fn test_kuramoto_step_changes_state() {
        let mut sys = Kuramoto::new(8, 2.0);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_kuramoto_phases_stay_wrapped() {
        let mut sys = Kuramoto::new(8, 2.0);
        for _ in 0..1000 {
            sys.step(0.01);
        }
        for &th in sys.state().iter() {
            assert!(th >= 0.0 && th < std::f64::consts::TAU, "Phase unwrapped: {}", th);
        }
    }

    #[test]
    fn test_kuramoto_order_parameter_in_range() {
        let mut sys = Kuramoto::new(8, 2.0);
        for _ in 0..500 {
            sys.step(0.01);
        }
        let r = sys.order_parameter();
        assert!(r >= 0.0 && r <= 1.0 + 1e-10, "Order param out of [0,1]: {}", r);
    }

    #[test]
    fn test_kuramoto_high_coupling_synchronizes() {
        // With very high coupling, order parameter should approach 1
        let mut sys = Kuramoto::new(8, 20.0);
        for _ in 0..2000 {
            sys.step(0.01);
        }
        let r = sys.order_parameter();
        assert!(r > 0.8, "High coupling should synchronize: r={}", r);
    }
}
