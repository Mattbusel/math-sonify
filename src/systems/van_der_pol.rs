use super::{rk4, DynamicalSystem};

/// Van der Pol self-sustaining limit-cycle oscillator.
///
/// Equations (μ = 2.0 by default):
/// ```text
/// dx/dt = y
/// dy/dt = μ·(1 - x²)·y - x
/// ```
/// For μ > 0 the system has a stable limit cycle; larger μ gives increasingly
/// relaxation-oscillator-like behavior (sharp transitions).
pub struct VanDerPol {
    state: Vec<f64>,
    pub mu: f64,
}

impl VanDerPol {
    /// Create a Van der Pol oscillator with default nonlinearity μ = 2.0.
    pub fn new() -> Self {
        Self {
            state: vec![2.0, 0.0],
            mu: 2.0,
        }
    }

    fn deriv(state: &[f64], mu: f64) -> Vec<f64> {
        let x = state[0];
        let y = state[1];
        vec![y, mu * (1.0 - x * x) * y - x]
    }
}

impl DynamicalSystem for VanDerPol {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        2
    }
    fn name(&self) -> &str {
        "Van der Pol"
    }

    fn step(&mut self, dt: f64) {
        let mu = self.mu;
        rk4(&mut self.state, dt, |s| Self::deriv(s, mu));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.mu)
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
    fn test_van_der_pol_initial_state() {
        let sys = VanDerPol::new();
        let s = sys.state();
        assert_eq!(s.len(), 2);
        assert!((s[0] - 2.0).abs() < 1e-15);
        assert!((s[1] - 0.0).abs() < 1e-15);
        assert_eq!(sys.name(), "Van der Pol");
        assert_eq!(sys.dimension(), 2);
    }

    #[test]
    fn test_van_der_pol_step_changes_state() {
        let mut sys = VanDerPol::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_van_der_pol_deterministic() {
        let mut sys1 = VanDerPol::new();
        let mut sys2 = VanDerPol::new();
        for _ in 0..500 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_van_der_pol_dt_zero_no_change() {
        let mut sys = VanDerPol::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0);
        let after = sys.state();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-15, "State changed with dt=0: {} -> {}", a, b);
        }
    }

    #[test]
    fn test_van_der_pol_set_state() {
        let mut sys = VanDerPol::new();
        sys.set_state(&[1.5, -0.5]);
        let s = sys.state();
        assert!((s[0] - 1.5).abs() < 1e-15);
        assert!((s[1] - (-0.5)).abs() < 1e-15);
    }

    #[test]
    fn test_van_der_pol_set_state_ignores_nan() {
        let mut sys = VanDerPol::new();
        let original: Vec<f64> = sys.state().to_vec();
        sys.set_state(&[f64::NAN, f64::NAN]);
        for (a, b) in original.iter().zip(sys.state().iter()) {
            assert!((a - b).abs() < 1e-15, "NaN state should be ignored");
        }
    }
}
