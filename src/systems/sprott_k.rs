use super::{rk4, DynamicalSystem};

/// Sprott Case K chaotic system.
///
/// Equations:
/// ```text
/// x' = x·y - z
/// y' = x - y
/// z' = x + 0.3·z
/// ```
///
/// One of Sprott's (1994) simplest three-dimensional chaotic flows.
/// The xy product creates the nonlinear feedback needed for chaos while
/// all equations are at most degree-2 polynomial.
///
/// Reference: Sprott, J. C. (1994). "Some simple chaotic flows."
/// Physical Review E, 50(2), R647–R650.
pub struct SprottK {
    pub state: Vec<f64>,
    speed: f64,
}

impl SprottK {
    pub fn new() -> Self {
        Self {
            state: vec![0.1, 0.0, 0.5],
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64]) -> Vec<f64> {
        vec![
            s[0] * s[1] - s[2],
            s[0] - s[1],
            s[0] + 0.3 * s[2],
        ]
    }
}

impl Default for SprottK {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for SprottK {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "sprott_k"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state)
    }

    fn step(&mut self, dt: f64) {
        let prev = self.state.clone();
        rk4(&mut self.state, dt, Self::deriv);
        if !self.state.iter().all(|v| v.is_finite()) {
            self.state = prev;
            self.speed = 0.0;
            return;
        }
        self.speed = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
            / dt;
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn sprott_k_initial_state_finite() {
        let sys = SprottK::new();
        assert!(sys.state().iter().all(|v| v.is_finite()), "Initial state has non-finite values");
    }

    #[test]
    fn sprott_k_stays_finite() {
        let mut sys = SprottK::new();
        for _ in 0..5_000 {
            sys.step(0.01);
        }
        assert!(sys.state().iter().all(|v| v.is_finite()), "State became non-finite: {:?}", sys.state());
    }

    #[test]
    fn sprott_k_state_bounded() {
        let mut sys = SprottK::new();
        for _ in 0..5_000 {
            sys.step(0.01);
        }
        let s = sys.state();
        assert!(s[0].abs() < 15.0, "x out of range: {}", s[0]);
        assert!(s[1].abs() < 15.0, "y out of range: {}", s[1]);
        assert!(s[2].abs() < 15.0, "z out of range: {}", s[2]);
    }

    #[test]
    fn sprott_k_step_changes_state() {
        let mut sys = SprottK::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(
            before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn sprott_k_deterministic() {
        let mut s1 = SprottK::new();
        let mut s2 = SprottK::new();
        for _ in 0..500 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn sprott_k_set_state() {
        let mut sys = SprottK::new();
        sys.set_state(&[1.0, -0.5, 0.2]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] + 0.5).abs() < 1e-15);
        assert!((s[2] - 0.2).abs() < 1e-15);
    }

    #[test]
    fn sprott_k_deriv_at_known_point() {
        let sys = SprottK::new();
        // At (1, 1, 0): x' = 1*1 - 0 = 1, y' = 1 - 1 = 0, z' = 1 + 0 = 1
        let d = sys.deriv_at(&[1.0, 1.0, 0.0]);
        assert!((d[0] - 1.0).abs() < 1e-14, "x' expected 1: {}", d[0]);
        assert!(d[1].abs() < 1e-14, "y' expected 0: {}", d[1]);
        assert!((d[2] - 1.0).abs() < 1e-14, "z' expected 1: {}", d[2]);
    }

    #[test]
    fn sprott_k_speed_positive_after_step() {
        let mut sys = SprottK::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0, "speed should be positive after a step, got {}", sys.speed());
    }
}
