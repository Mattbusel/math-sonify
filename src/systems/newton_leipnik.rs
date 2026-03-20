use super::{rk4, DynamicalSystem};

/// Newton-Leipnik system — two coupled nonlinear oscillators exhibiting double-scroll chaos.
///
/// Equations:
/// ```text
/// dx/dt = −a·x + y + 10·y·z
/// dy/dt = −x − 0.4·y + 5·x·z
/// dz/dt =  b·z − 5·x·y
/// ```
///
/// Parameters:
/// - `a`: damping of x (default 0.4)
/// - `b`: z growth rate (default 0.175)
///
/// With `a=0.4, b=0.175` the system produces a characteristic double-scroll
/// chaotic attractor.  First described by Newton and Leipnik (1981) while
/// studying rotation of a rigid body.
pub struct NewtonLeipnik {
    state: Vec<f64>,
    speed: f64,
    pub a: f64,
    pub b: f64,
}

impl NewtonLeipnik {
    /// Create a Newton-Leipnik attractor with default parameters (a=0.4, b=0.175).
    pub fn new() -> Self {
        Self {
            state: vec![0.349, 0.0, -0.16],
            speed: 0.0,
            a: 0.4,
            b: 0.175,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64) -> Vec<f64> {
        vec![
            -a * s[0] + s[1] + 10.0 * s[1] * s[2],
            -s[0] - 0.4 * s[1] + 5.0 * s[0] * s[2],
            b * s[2] - 5.0 * s[0] * s[1],
        ]
    }
}

impl Default for NewtonLeipnik {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for NewtonLeipnik {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "newton_leipnik"
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
    fn test_newton_leipnik_initial_state() {
        let sys = NewtonLeipnik::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "newton_leipnik");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_newton_leipnik_step_changes_state() {
        let mut sys = NewtonLeipnik::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_newton_leipnik_state_stays_finite() {
        let mut sys = NewtonLeipnik::new();
        for _ in 0..5000 {
            sys.step(0.01);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_newton_leipnik_deterministic() {
        let mut s1 = NewtonLeipnik::new();
        let mut s2 = NewtonLeipnik::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_newton_leipnik_deriv_at_known_point() {
        // At (1, 0, 0): dx = -0.4*1 + 0 + 0 = -0.4
        //               dy = -1 - 0 + 0 = -1.0
        //               dz = 0.175*0 - 0 = 0.0
        let sys = NewtonLeipnik::new();
        let d = sys.deriv_at(&[1.0, 0.0, 0.0]);
        assert!((d[0] - (-0.4)).abs() < 1e-12, "d[0]={}", d[0]);
        assert!((d[1] - (-1.0)).abs() < 1e-12, "d[1]={}", d[1]);
        assert!(d[2].abs() < 1e-12, "d[2]={}", d[2]);
    }

    #[test]
    fn test_newton_leipnik_speed_positive_after_step() {
        let mut sys = NewtonLeipnik::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0);
    }

    #[test]
    fn test_newton_leipnik_parameter_b_affects_trajectory() {
        let mut s1 = NewtonLeipnik::new();
        let mut s2 = NewtonLeipnik::new();
        s2.b = 0.35; // different b
        for _ in 0..100 {
            s1.step(0.01);
            s2.step(0.01);
        }
        let diff: f64 = s1.state().iter().zip(s2.state().iter()).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff > 1e-6, "Different b should produce different trajectories: diff={}", diff);
    }
}
