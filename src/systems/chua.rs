use super::{rk4, DynamicalSystem};

/// Chua's circuit — canonical example of a chaotic electronic circuit.
///
/// Produces a characteristic double-scroll attractor via a piecewise-linear
/// nonlinear resistor (h(x)).  Parameters: α = 15.6, β = 28, m₀ = -1.143,
/// m₁ = -0.714 (default).
pub struct Chua {
    state: Vec<f64>,
    pub alpha: f64,
    pub beta: f64,
    pub m0: f64,
    pub m1: f64,
}

impl Chua {
    /// Create a Chua circuit with default parameters (α=15.6, β=28.0).
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
        vec![alpha * (y - x - Self::h(x, m0, m1)), x - y + z, -beta * y]
    }
}

impl DynamicalSystem for Chua {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "Chua's Circuit"
    }

    fn step(&mut self, dt: f64) {
        let (alpha, beta, m0, m1) = (self.alpha, self.beta, self.m0, self.m1);
        rk4(&mut self.state, dt, |s| Self::deriv(s, alpha, beta, m0, m1));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.alpha, self.beta, self.m0, self.m1)
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
    fn test_chua_initial_state() {
        let sys = Chua::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.7).abs() < 1e-15);
        assert!((s[1] - 0.0).abs() < 1e-15);
        assert!((s[2] - 0.0).abs() < 1e-15);
        assert_eq!(sys.name(), "Chua's Circuit");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_chua_step_changes_state() {
        let mut sys = Chua::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_chua_deterministic() {
        let mut sys1 = Chua::new();
        let mut sys2 = Chua::new();
        for _ in 0..500 {
            sys1.step(0.001);
            sys2.step(0.001);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_chua_dt_zero_no_change() {
        let mut sys = Chua::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0);
        let after = sys.state();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-15, "State changed with dt=0: {} -> {}", a, b);
        }
    }

    #[test]
    fn test_chua_set_state() {
        let mut sys = Chua::new();
        sys.set_state(&[1.0, -0.5, 2.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - (-0.5)).abs() < 1e-15);
        assert!((s[2] - 2.0).abs() < 1e-15);
    }
}
