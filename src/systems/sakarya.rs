use super::{rk4, DynamicalSystem};

/// Sakarya attractor — three-variable chaotic system with a distinctive topology.
///
/// Equations:
/// ```text
/// dx/dt = −x − y
/// dy/dt =  y − x·z + a
/// dz/dt =  x·y − b·z
/// ```
///
/// Parameters:
/// - `a`: additive constant in dy/dt (default 0.4)
/// - `b`: z damping coefficient in dz/dt (default 0.3); must be > 0 for bounded attractor
///
/// With `a=0.4, b=0.3` the system exhibits a bounded chaotic attractor.  The
/// `−b·z` term provides global damping in z, ensuring trajectories remain bounded.
/// Named after the Sakarya attractor family described in the nonlinear dynamics
/// literature.
pub struct Sakarya {
    state: Vec<f64>,
    speed: f64,
    pub a: f64,
    pub b: f64,
}

impl Sakarya {
    /// Create a Sakarya attractor with default parameters (a=0.4, b=0.3).
    pub fn new() -> Self {
        Self {
            state: vec![1.0, 1.0, 0.5],
            speed: 0.0,
            a: 0.4,
            b: 0.3,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64) -> Vec<f64> {
        vec![
            -s[0] - s[1],
            s[1] - s[0] * s[2] + a,
            s[0] * s[1] - b * s[2],
        ]
    }
}

impl Default for Sakarya {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Sakarya {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "sakarya"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b)
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
        let b = self.b;
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b));
        // Guard: revert to previous state if RK4 produced non-finite values
        if !self.state.iter().all(|v| v.is_finite()) {
            self.state = prev.clone();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_sakarya_initial_state() {
        let sys = Sakarya::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "sakarya");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_sakarya_step_changes_state() {
        let mut sys = Sakarya::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_sakarya_state_stays_finite() {
        let mut sys = Sakarya::new();
        for _ in 0..5000 {
            sys.step(0.01);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_sakarya_deterministic() {
        let mut s1 = Sakarya::new();
        let mut s2 = Sakarya::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_sakarya_deriv_at_known_point() {
        // At (1, 1, 0): dx = -1-1 = -2, dy = 1-1*0+0.4 = 1.4, dz = 1*1-0.3*0 = 1
        let sys = Sakarya::new();
        let d = sys.deriv_at(&[1.0, 1.0, 0.0]);
        assert!((d[0] - (-2.0)).abs() < 1e-12, "d[0]={}", d[0]);
        assert!((d[1] - 1.4).abs() < 1e-12, "d[1]={}", d[1]);
        assert!((d[2] - 1.0).abs() < 1e-12, "d[2]={}", d[2]);
    }

    #[test]
    fn test_sakarya_speed_positive_after_step() {
        let mut sys = Sakarya::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0);
    }

    #[test]
    fn test_sakarya_parameter_a_affects_trajectory() {
        let mut s1 = Sakarya::new();
        let mut s2 = Sakarya::new();
        s2.a = 0.8;
        for _ in 0..100 {
            s1.step(0.01);
            s2.step(0.01);
        }
        let diff: f64 = s1.state().iter().zip(s2.state().iter()).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff > 1e-6, "Different a should produce different trajectories: diff={}", diff);
    }
}
