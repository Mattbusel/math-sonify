use super::{rk4, DynamicalSystem};

/// Sprott-G system — one of Sprott's 19 algebraically simplest chaotic flows.
///
/// Equations:
/// ```text
/// dx/dt = 0.4·x + z
/// dy/dt = x·z − y
/// dz/dt = −x + y
/// ```
///
/// This system has no free parameters and exhibits a strange attractor with a
/// single scroll structure.  Like all Sprott flows it contains only quadratic
/// and linear terms, making it one of the most efficient chaotic generators.
pub struct SprottG {
    state: Vec<f64>,
    speed: f64,
}

impl SprottG {
    /// Create a Sprott-G attractor with initial state (0.1, 0.1, 0.1).
    pub fn new() -> Self {
        Self {
            state: vec![0.1, 0.1, 0.1],
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64]) -> Vec<f64> {
        vec![
            0.4 * s[0] + s[2],
            s[0] * s[2] - s[1],
            -s[0] + s[1],
        ]
    }
}

impl Default for SprottG {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for SprottG {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "sprott_g"
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
    fn test_sprott_g_initial_state() {
        let sys = SprottG::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "sprott_g");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_sprott_g_step_changes_state() {
        let mut sys = SprottG::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_sprott_g_state_stays_finite() {
        let mut sys = SprottG::new();
        for _ in 0..5000 {
            sys.step(0.01);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_sprott_g_deterministic() {
        let mut s1 = SprottG::new();
        let mut s2 = SprottG::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_sprott_g_set_state() {
        let mut sys = SprottG::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_sprott_g_deriv_at_known_point() {
        // At (1, 0, 0): dx = 0.4*1 + 0 = 0.4, dy = 1*0 - 0 = 0, dz = -1 + 0 = -1
        let sys = SprottG::new();
        let d = sys.deriv_at(&[1.0, 0.0, 0.0]);
        assert!((d[0] - 0.4).abs() < 1e-12, "d[0]={}", d[0]);
        assert!(d[1].abs() < 1e-12, "d[1]={}", d[1]);
        assert!((d[2] - (-1.0)).abs() < 1e-12, "d[2]={}", d[2]);
    }

    #[test]
    fn test_sprott_g_speed_positive_after_step() {
        let mut sys = SprottG::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0);
    }
}
