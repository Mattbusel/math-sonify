use super::{rk4, DynamicalSystem};

/// Duffing oscillator: a driven, damped nonlinear oscillator.
///
/// Equations of motion (extended state includes the driving phase phi):
///
///   dx/dt = v
///   dv/dt = -delta*v - alpha*x - beta*x^3 + gamma*cos(phi)
///   dphi/dt = omega
///
/// With delta=0.3, alpha=-1, beta=1, gamma=0.5, omega=1.2 the system exhibits
/// a strange attractor in (x, v, phi mod 2pi) space.
pub struct Duffing {
    state: Vec<f64>,
    pub delta: f64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub omega: f64,
}

impl Duffing {
    /// Creates a Duffing oscillator with default chaotic parameters.
    ///
    /// Defaults: δ=0.3 (damping), α=-1 (linear stiffness, double-well), β=1 (nonlinear
    /// stiffness), γ=0.5 (driving amplitude), ω=1.2 (driving frequency).
    /// Initial state: x=1.0, v=0.0, φ=0.0 (at rest in the right potential well).
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
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "Duffing"
    }

    fn step(&mut self, dt: f64) {
        let (delta, alpha, beta, gamma, omega) =
            (self.delta, self.alpha, self.beta, self.gamma, self.omega);
        rk4(&mut self.state, dt, |s| {
            Self::deriv(s, delta, alpha, beta, gamma, omega)
        });
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(
            state, self.delta, self.alpha, self.beta, self.gamma, self.omega,
        )
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }

    fn speed(&self) -> f64 {
        let d = self.current_deriv();
        d.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_duffing_initial_state() {
        let sys = Duffing::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 1.0).abs() < 1e-15, "Expected x=1.0, got {}", s[0]);
        assert!((s[1] - 0.0).abs() < 1e-15, "Expected v=0.0, got {}", s[1]);
        assert!((s[2] - 0.0).abs() < 1e-15, "Expected phi=0.0, got {}", s[2]);
        assert_eq!(sys.name(), "Duffing");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_duffing_step_changes_state() {
        let mut sys = Duffing::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_duffing_deterministic() {
        let mut sys1 = Duffing::new();
        let mut sys2 = Duffing::new();
        for _ in 0..500 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_duffing_dt_zero_no_change() {
        let mut sys = Duffing::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0);
        let after = sys.state();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-15, "State changed with dt=0: {} -> {}", a, b);
        }
    }

    #[test]
    fn test_duffing_set_state() {
        let mut sys = Duffing::new();
        sys.set_state(&[0.5, 1.0, 3.14]);
        let s = sys.state();
        assert!((s[0] - 0.5).abs() < 1e-15);
        assert!((s[1] - 1.0).abs() < 1e-15);
        assert!((s[2] - 3.14).abs() < 1e-15);
    }

    #[test]
    fn test_duffing_set_state_ignores_nan() {
        let mut sys = Duffing::new();
        let original: Vec<f64> = sys.state().to_vec();
        sys.set_state(&[f64::NAN, f64::NAN, f64::NAN]);
        for (a, b) in original.iter().zip(sys.state().iter()) {
            assert!((a - b).abs() < 1e-15, "NaN state should be ignored");
        }
    }
}
