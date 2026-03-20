use super::{rk4, DynamicalSystem};

/// Thomas' cyclically symmetric attractor.
///
/// dx/dt = sin(y) − b·x
/// dy/dt = sin(z) − b·y
/// dz/dt = sin(x) − b·z
///
/// With b ≈ 0.208186 the system is chaotic and exhibits cyclic (x→y→z→x)
/// symmetry.  The attractor has a distinctive three-armed scroll structure.
/// Reducing b below ~0.209 gives a fully-developed strange attractor; values
/// above ~0.32 produce limit-cycle behaviour.
pub struct Thomas {
    state: Vec<f64>,
    pub b: f64,
    speed: f64,
}

impl Thomas {
    /// Create a Thomas attractor with the given dissipation parameter `b`.
    ///
    /// `b = 0.208186` produces the canonical strange attractor.
    pub fn new(b: f64) -> Self {
        Self {
            state: vec![0.1, 0.0, 0.0],
            b,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], b: f64) -> Vec<f64> {
        vec![
            s[1].sin() - b * s[0],
            s[2].sin() - b * s[1],
            s[0].sin() - b * s[2],
        ]
    }
}

impl Default for Thomas {
    fn default() -> Self {
        Self::new(0.208186)
    }
}

impl DynamicalSystem for Thomas {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "thomas"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.b)
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
        let b = self.b;
        rk4(&mut self.state, dt, |s| Self::deriv(s, b));
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
    fn test_thomas_initial_state() {
        let sys = Thomas::default();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "thomas");
        let s = sys.state();
        assert!(s.iter().all(|v| v.is_finite()));
        assert!((s[0] - 0.1).abs() < 1e-15);
    }

    #[test]
    fn test_thomas_step_changes_state() {
        let mut sys = Thomas::default();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_thomas_state_stays_finite() {
        let mut sys = Thomas::default();
        for _ in 0..1000 {
            sys.step(0.01);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_thomas_set_state() {
        let mut sys = Thomas::default();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_thomas_set_state_ignores_nan() {
        let mut sys = Thomas::default();
        sys.set_state(&[f64::NAN, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 0.1).abs() < 1e-15, "NaN should not change state[0]");
        assert!((s[1] - 2.0).abs() < 1e-15);
    }

    #[test]
    fn test_thomas_cyclic_symmetry_deriv() {
        // Thomas: dx/dt = sin(y) - b*x, dy/dt = sin(z) - b*y, dz/dt = sin(x) - b*z
        let b = 0.208186;
        let sys = Thomas::new(b);
        let pi_2 = std::f64::consts::PI / 2.0;
        let d = sys.deriv_at(&[0.0, pi_2, 0.0]);
        // d[0] = sin(π/2) - b*0 = 1.0
        assert!((d[0] - 1.0).abs() < 1e-10, "d[0] expected 1.0, got {}", d[0]);
        // d[1] = sin(0) - b*(π/2) = -b*π/2
        let expected_d1 = -b * pi_2;
        assert!((d[1] - expected_d1).abs() < 1e-10, "d[1] expected {}, got {}", expected_d1, d[1]);
        // d[2] = sin(0) - b*0 = 0
        assert!(d[2].abs() < 1e-10, "d[2] expected 0.0, got {}", d[2]);
    }
}
