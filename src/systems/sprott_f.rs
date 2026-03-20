use super::{rk4, DynamicalSystem};

/// Sprott-F system — one of Sprott's 19 algebraically simplest chaotic flows.
///
/// Equations:
/// ```text
/// dx/dt = y + z
/// dy/dt = −x + 0.5·y
/// dz/dt = x² − z
/// ```
///
/// This system has no free parameters.  The `x²` nonlinearity in the z
/// equation drives the chaotic behaviour.
///
/// Reference: Sprott, J.C. (1994). "Some simple chaotic flows."
/// Phys. Rev. E 50, R647.
pub struct SprottF {
    state: Vec<f64>,
    speed: f64,
}

impl SprottF {
    /// Create a Sprott-F attractor with initial state (0.5, 0.5, 0.0).
    pub fn new() -> Self {
        Self {
            state: vec![0.5, 0.5, 0.0],
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64]) -> Vec<f64> {
        vec![
            s[1] + s[2],
            -s[0] + 0.5 * s[1],
            s[0] * s[0] - s[2],
        ]
    }
}

impl Default for SprottF {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for SprottF {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "sprott_f"
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
            .sqrt();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprott_f_initial_state() {
        let sys = SprottF::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.5).abs() < 1e-15);
        assert!((s[1] - 0.5).abs() < 1e-15);
        assert!(s[2].abs() < 1e-15);
        assert_eq!(sys.name(), "sprott_f");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_sprott_f_step_changes_state() {
        let mut sys = SprottF::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_sprott_f_state_stays_finite() {
        let mut sys = SprottF::new();
        for _ in 0..5000 {
            sys.step(0.01);
            assert!(sys.state().iter().all(|v| v.is_finite()), "State went non-finite");
        }
    }

    #[test]
    fn test_sprott_f_deterministic() {
        let mut s1 = SprottF::new();
        let mut s2 = SprottF::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_sprott_f_set_state() {
        let mut sys = SprottF::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_sprott_f_speed_positive_after_step() {
        let mut sys = SprottF::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0, "speed should be positive after step: {}", sys.speed());
    }

    #[test]
    fn test_sprott_f_deriv_at_known_point() {
        // At (1, 0, 0): dx=0+0=0, dy=-1+0=-1, dz=1-0=1
        let sys = SprottF::new();
        let d = sys.deriv_at(&[1.0, 0.0, 0.0]);
        assert!(d[0].abs() < 1e-12, "d[0] expected 0.0, got {}", d[0]);
        assert!((d[1] - (-1.0)).abs() < 1e-12, "d[1] expected -1.0, got {}", d[1]);
        assert!((d[2] - 1.0).abs() < 1e-12, "d[2] expected 1.0, got {}", d[2]);
    }
}
