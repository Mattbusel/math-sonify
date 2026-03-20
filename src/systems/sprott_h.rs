use super::{rk4, DynamicalSystem};

/// Sprott-H system — one of Sprott's 19 algebraically simplest chaotic flows.
///
/// Equations:
/// ```text
/// dx/dt = −y + z²
/// dy/dt = x + 0.5·y
/// dz/dt = x − z
/// ```
///
/// This system has no free parameters and exhibits a spiral attractor.
/// It is notable for its unusual mix of quadratic and linear terms.
pub struct SprottH {
    state: Vec<f64>,
    speed: f64,
}

impl SprottH {
    /// Create a Sprott-H attractor with initial state (0.1, 0.0, 0.0).
    pub fn new() -> Self {
        Self {
            state: vec![0.1, 0.0, 0.0],
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64]) -> Vec<f64> {
        vec![
            -s[1] + s[2] * s[2],
            s[0] + 0.5 * s[1],
            s[0] - s[2],
        ]
    }
}

impl Default for SprottH {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for SprottH {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "sprott_h"
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
        self.speed = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
            / dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_sprott_h_initial_state() {
        let sys = SprottH::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "sprott_h");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_sprott_h_step_changes_state() {
        let mut sys = SprottH::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_sprott_h_state_stays_finite() {
        let mut sys = SprottH::new();
        for _ in 0..5000 {
            sys.step(0.01);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_sprott_h_deterministic() {
        let mut s1 = SprottH::new();
        let mut s2 = SprottH::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_sprott_h_deriv_at_known_point() {
        // At (1, 0, 1): dx = -0 + 1 = 1, dy = 1 + 0 = 1, dz = 1 - 1 = 0
        let sys = SprottH::new();
        let d = sys.deriv_at(&[1.0, 0.0, 1.0]);
        assert!((d[0] - 1.0).abs() < 1e-12, "d[0]={}", d[0]);
        assert!((d[1] - 1.0).abs() < 1e-12, "d[1]={}", d[1]);
        assert!(d[2].abs() < 1e-12, "d[2]={}", d[2]);
    }

    #[test]
    fn test_sprott_h_speed_positive_after_step() {
        let mut sys = SprottH::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0);
    }
}
