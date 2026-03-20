use super::{rk4, DynamicalSystem};

/// Halvorsen cyclic-symmetry attractor: a three-dimensional system with
/// rotational symmetry under (x,y,z) -> (y,z,x).
///
/// Equations (a = 1.89 by default):
/// ```text
/// dx/dt = -a·x - 4·y - 4·z - y²
/// dy/dt = -a·y - 4·z - 4·x - z²
/// dz/dt = -a·z - 4·x - 4·y - x²
/// ```
pub struct Halvorsen {
    state: Vec<f64>,
    pub a: f64,
}

impl Halvorsen {
    /// Create a Halvorsen attractor with default parameter a = 1.89.
    pub fn new() -> Self {
        Self {
            state: vec![-5.0, 0.0, 0.0],
            a: 1.89,
        }
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
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "Halvorsen"
    }

    fn step(&mut self, dt: f64) {
        let a = self.a;
        rk4(&mut self.state, dt, |s| Self::deriv(s, a));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a)
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
    fn test_halvorsen_initial_state() {
        let sys = Halvorsen::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - (-5.0)).abs() < 1e-15);
        assert!((s[1] - 0.0).abs() < 1e-15);
        assert!((s[2] - 0.0).abs() < 1e-15);
        assert_eq!(sys.name(), "Halvorsen");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_halvorsen_step_changes_state() {
        let mut sys = Halvorsen::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_halvorsen_deterministic() {
        let mut sys1 = Halvorsen::new();
        let mut sys2 = Halvorsen::new();
        for _ in 0..500 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_halvorsen_dt_zero_no_change() {
        let mut sys = Halvorsen::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0);
        let after = sys.state();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-15, "State changed with dt=0: {} -> {}", a, b);
        }
    }

    #[test]
    fn test_halvorsen_set_state() {
        let mut sys = Halvorsen::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_halvorsen_set_state_ignores_nan() {
        let mut sys = Halvorsen::new();
        let original: Vec<f64> = sys.state().to_vec();
        sys.set_state(&[f64::NAN, f64::NAN, f64::NAN]);
        for (a, b) in original.iter().zip(sys.state().iter()) {
            assert!((a - b).abs() < 1e-15, "NaN state should be ignored");
        }
    }
}
