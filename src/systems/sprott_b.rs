use super::{rk4, DynamicalSystem};

/// Sprott B system: dx/dt = y*z, dy/dt = x - y, dz/dt = 1 - x*y
pub struct SprottB {
    state: Vec<f64>,
    speed: f64,
}

impl SprottB {
    /// Creates a Sprott B attractor with initial state (0, 3, 0).
    ///
    /// The Sprott B system is one of the simplest three-dimensional chaotic
    /// flows with only quadratic nonlinearities.  It has no free parameters;
    /// the strange attractor is an invariant set of the fixed equations.
    pub fn new() -> Self {
        Self {
            state: vec![0.0, 3.0, 0.0],
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64]) -> Vec<f64> {
        vec![s[1] * s[2], s[0] - s[1], 1.0 - s[0] * s[1]]
    }
}

impl Default for SprottB {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for SprottB {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "sprott_b"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state)
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
        let prev = self.state.clone();
        rk4(&mut self.state, dt, Self::deriv);
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
    fn test_sprott_b_initial_state() {
        let sys = SprottB::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.0).abs() < 1e-15);
        assert!((s[1] - 3.0).abs() < 1e-15);
        assert!((s[2] - 0.0).abs() < 1e-15);
        assert_eq!(sys.name(), "sprott_b");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_sprott_b_step_changes_state() {
        let mut sys = SprottB::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_sprott_b_deterministic() {
        let mut sys1 = SprottB::new();
        let mut sys2 = SprottB::new();
        for _ in 0..500 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_sprott_b_dt_zero_no_change() {
        let mut sys = SprottB::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0);
        let after = sys.state();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-15, "State changed with dt=0: {} -> {}", a, b);
        }
    }

    #[test]
    fn test_sprott_b_set_state() {
        let mut sys = SprottB::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }
}
