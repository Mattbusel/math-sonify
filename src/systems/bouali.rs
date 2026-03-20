use super::{rk4, DynamicalSystem};

/// Bouali attractor — three-parameter chaotic system exhibiting complex spiral geometry.
///
/// Equations:
/// ```text
/// dx/dt = x·(4 − y) + a·z
/// dy/dt = −y·(1 − x²)
/// dz/dt = −x·(1.5 − s·z) − 0.05·z
/// ```
///
/// Parameters:
/// - `a`: coupling from z to dx/dt (default 0.3)
/// - `s`: z-feedback in dz/dt (default 1.0)
///
/// The system displays a double-scroll attractor with meandering trajectories
/// between two lobes.  Named after Safieddine Bouali (1994).
pub struct Bouali {
    state: Vec<f64>,
    speed: f64,
    pub a: f64,
    pub s: f64,
}

impl Bouali {
    /// Create a Bouali attractor with default parameters (a=0.3, s=1.0).
    pub fn new() -> Self {
        Self {
            state: vec![1.0, 1.0, 0.0],
            speed: 0.0,
            a: 0.3,
            s: 1.0,
        }
    }

    fn deriv(s: &[f64], a: f64, sf: f64) -> Vec<f64> {
        vec![
            s[0] * (4.0 - s[1]) + a * s[2],
            -s[1] * (1.0 - s[0] * s[0]),
            -s[0] * (1.5 - sf * s[2]) - 0.05 * s[2],
        ]
    }
}

impl Default for Bouali {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Bouali {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "bouali"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.s)
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
        let a = self.a;
        let sf = self.s;
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, sf));
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
    fn test_bouali_initial_state() {
        let sys = Bouali::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "bouali");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_bouali_step_changes_state() {
        let mut sys = Bouali::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_bouali_state_stays_finite() {
        let mut sys = Bouali::new();
        for _ in 0..5000 {
            sys.step(0.01);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_bouali_deterministic() {
        let mut s1 = Bouali::new();
        let mut s2 = Bouali::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_bouali_deriv_at_known_point() {
        // At (1, 1, 0): dx = 1*(4-1)+0.3*0 = 3, dy = -1*(1-1) = 0, dz = -1*(1.5-0)-0 = -1.5
        let sys = Bouali::new();
        let d = sys.deriv_at(&[1.0, 1.0, 0.0]);
        assert!((d[0] - 3.0).abs() < 1e-12, "d[0]={}", d[0]);
        assert!(d[1].abs() < 1e-12, "d[1]={}", d[1]);
        assert!((d[2] - (-1.5)).abs() < 1e-12, "d[2]={}", d[2]);
    }

    #[test]
    fn test_bouali_speed_positive_after_step() {
        let mut sys = Bouali::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0);
    }

    #[test]
    fn test_bouali_parameter_a_affects_trajectory() {
        let mut s1 = Bouali::new();
        let mut s2 = Bouali::new();
        s2.a = 0.6;
        for _ in 0..100 {
            s1.step(0.01);
            s2.step(0.01);
        }
        // Different a should diverge trajectories
        let diff: f64 = s1.state().iter().zip(s2.state().iter()).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff > 1e-6, "Different a should produce different trajectories: diff={}", diff);
    }
}
