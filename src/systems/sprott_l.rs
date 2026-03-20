use super::{rk4, DynamicalSystem};

/// Sprott-L system — one of Sprott's 19 algebraically simplest chaotic flows.
///
/// Equations:
/// ```text
/// dx/dt = y + 3.9·z
/// dy/dt = 0.9·x² − y
/// dz/dt = 1 − x
/// ```
///
/// This system has no free parameters and exhibits a scroll-type strange attractor.
/// The quadratic x² term in dy/dt provides the necessary nonlinearity for chaos.
pub struct SprottL {
    state: Vec<f64>,
    speed: f64,
}

impl SprottL {
    /// Create a Sprott-L attractor with initial state (0.1, 0.0, 0.0).
    pub fn new() -> Self {
        Self {
            state: vec![0.1, 0.0, 0.0],
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64]) -> Vec<f64> {
        vec![
            s[1] + 3.9 * s[2],
            0.9 * s[0] * s[0] - s[1],
            1.0 - s[0],
        ]
    }
}

impl Default for SprottL {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for SprottL {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "sprott_l"
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
    fn test_sprott_l_initial_state() {
        let sys = SprottL::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "sprott_l");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_sprott_l_step_changes_state() {
        let mut sys = SprottL::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_sprott_l_state_stays_finite() {
        let mut sys = SprottL::new();
        for _ in 0..5000 {
            sys.step(0.01);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_sprott_l_deterministic() {
        let mut s1 = SprottL::new();
        let mut s2 = SprottL::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_sprott_l_deriv_at_known_point() {
        // At (1, 0, 0): dx = 0 + 0 = 0, dy = 0.9*1 - 0 = 0.9, dz = 1 - 1 = 0
        let sys = SprottL::new();
        let d = sys.deriv_at(&[1.0, 0.0, 0.0]);
        assert!(d[0].abs() < 1e-12, "d[0]={}", d[0]);
        assert!((d[1] - 0.9).abs() < 1e-12, "d[1]={}", d[1]);
        assert!(d[2].abs() < 1e-12, "d[2]={}", d[2]);
    }

    #[test]
    fn test_sprott_l_speed_positive_after_step() {
        let mut sys = SprottL::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0);
    }
}
