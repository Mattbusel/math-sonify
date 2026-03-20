use super::{rk4, DynamicalSystem};

/// Rikitake dynamo — a model of geomagnetic field reversals.
///
/// Equations:
/// ```text
/// dx/dt = −μ·x + z·y
/// dy/dt = −μ·y + x·(z − a)
/// dz/dt = 1 − x·y
/// ```
///
/// With μ=1, a=5 the system exhibits irregular reversals of the x and y
/// variables analogous to reversals of the Earth's magnetic field.  The
/// parameter a controls the coupling strength; larger a gives faster reversals.
pub struct Rikitake {
    state: Vec<f64>,
    /// Dissipation rate. Default 1.0.
    pub mu: f64,
    /// Coupling offset. Default 5.0.
    pub a: f64,
    speed: f64,
}

impl Rikitake {
    /// Create a Rikitake dynamo with default parameters (μ=1, a=5).
    pub fn new() -> Self {
        Self {
            state: vec![0.5, 1.0, 1.0],
            mu: 1.0,
            a: 5.0,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], mu: f64, a: f64) -> Vec<f64> {
        vec![
            -mu * s[0] + s[2] * s[1],
            -mu * s[1] + s[0] * (s[2] - a),
            1.0 - s[0] * s[1],
        ]
    }
}

impl Default for Rikitake {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Rikitake {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "rikitake"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.mu, self.a)
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
        let (mu, a) = (self.mu, self.a);
        rk4(&mut self.state, dt, |s| Self::deriv(s, mu, a));
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
    fn test_rikitake_initial_state() {
        let sys = Rikitake::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "rikitake");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_rikitake_step_changes_state() {
        let mut sys = Rikitake::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_rikitake_state_stays_finite() {
        let mut sys = Rikitake::new();
        for _ in 0..5000 {
            sys.step(0.01);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_rikitake_deterministic() {
        let mut s1 = Rikitake::new();
        let mut s2 = Rikitake::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_rikitake_set_state() {
        let mut sys = Rikitake::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_rikitake_deriv_at_known_point() {
        // At (1, 1, 1) with μ=1, a=5:
        // dx = -1*1 + 1*1 = 0
        // dy = -1*1 + 1*(1-5) = -1 + (-4) = -5
        // dz = 1 - 1*1 = 0
        let sys = Rikitake::new();
        let d = sys.deriv_at(&[1.0, 1.0, 1.0]);
        assert!(d[0].abs() < 1e-12, "d[0]={}", d[0]);
        assert!((d[1] - (-5.0)).abs() < 1e-12, "d[1]={}", d[1]);
        assert!(d[2].abs() < 1e-12, "d[2]={}", d[2]);
    }

    #[test]
    fn test_rikitake_speed_positive_after_step() {
        let mut sys = Rikitake::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0);
    }
}
