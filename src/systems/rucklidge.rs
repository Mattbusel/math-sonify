use super::{rk4, DynamicalSystem};

/// Rucklidge attractor — a two-parameter three-dimensional chaotic system.
///
/// Equations:
/// ```text
/// dx/dt = −κ·x + λ·y − y·z
/// dy/dt = x
/// dz/dt = −z + y²
/// ```
///
/// With κ=2, λ=6.7 the system exhibits robust chaos with a characteristic
/// folded-band structure.  The Rucklidge system was originally derived from
/// a double-convection model in fluid dynamics.  It is notable for having
/// only two free parameters and a particularly simple nonlinear structure
/// (a single bilinear term in each of dx/dt and dz/dt).
pub struct Rucklidge {
    state: Vec<f64>,
    pub kappa: f64,
    pub lambda: f64,
    speed: f64,
}

impl Rucklidge {
    /// Create a Rucklidge attractor with default parameters (κ=2, λ=6.7).
    pub fn new() -> Self {
        Self {
            state: vec![1.0, 0.0, 4.5],
            kappa: 2.0,
            lambda: 6.7,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], kappa: f64, lambda: f64) -> Vec<f64> {
        vec![
            -kappa * s[0] + lambda * s[1] - s[1] * s[2],
            s[0],
            -s[2] + s[1] * s[1],
        ]
    }
}

impl Default for Rucklidge {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Rucklidge {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "rucklidge"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.kappa, self.lambda)
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
        let (kappa, lambda) = (self.kappa, self.lambda);
        rk4(&mut self.state, dt, |s| Self::deriv(s, kappa, lambda));
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
    fn test_rucklidge_initial_state() {
        let sys = Rucklidge::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "rucklidge");
        let s = sys.state();
        assert!(s.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_rucklidge_step_changes_state() {
        let mut sys = Rucklidge::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_rucklidge_state_stays_finite() {
        let mut sys = Rucklidge::new();
        for _ in 0..1000 {
            sys.step(0.01);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_rucklidge_deterministic() {
        let mut s1 = Rucklidge::new();
        let mut s2 = Rucklidge::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_rucklidge_set_state() {
        let mut sys = Rucklidge::new();
        sys.set_state(&[2.0, 1.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 2.0).abs() < 1e-15);
        assert!((s[1] - 1.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_rucklidge_deriv_at() {
        // At (1, 0, 4.5): dx = -2 + 0 - 0 = -2, dy = 1, dz = -4.5 + 0 = -4.5
        let sys = Rucklidge::new();
        let d = sys.deriv_at(&[1.0, 0.0, 4.5]);
        assert!((d[0] - (-2.0)).abs() < 1e-12, "d[0] expected -2.0, got {}", d[0]);
        assert!((d[1] - 1.0).abs() < 1e-12, "d[1] expected 1.0, got {}", d[1]);
        assert!((d[2] - (-4.5)).abs() < 1e-12, "d[2] expected -4.5, got {}", d[2]);
    }
}
