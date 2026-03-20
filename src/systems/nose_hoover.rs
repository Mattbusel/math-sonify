use super::{rk4, DynamicalSystem};

/// Nose-Hoover thermostat: dx/dt = y, dy/dt = -x + y*z, dz/dt = a - y²
pub struct NoseHoover {
    state: Vec<f64>,
    pub a: f64,
    speed: f64,
}

impl NoseHoover {
    /// Creates a Nosé-Hoover thermostat with default parameter a=3.0.
    ///
    /// The initial state (x=0, y=5, z=0) places the trajectory far from the
    /// fixed point so that the attractor is immediately engaged.
    /// The `a` parameter controls the coupling strength to the thermal reservoir;
    /// a=3.0 gives a persistent chaotic torus-like attractor.
    pub fn new() -> Self {
        Self {
            state: vec![0.0, 5.0, 0.0],
            a: 3.0,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64) -> Vec<f64> {
        vec![s[1], -s[0] + s[1] * s[2], a - s[1] * s[1]]
    }
}

impl DynamicalSystem for NoseHoover {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "nose_hoover"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a)
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
        let a = self.a;
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, a));
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
    fn test_nose_hoover_initial_state() {
        let sys = NoseHoover::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.0).abs() < 1e-15);
        assert!((s[1] - 5.0).abs() < 1e-15);
        assert!((s[2] - 0.0).abs() < 1e-15);
        assert_eq!(sys.name(), "nose_hoover");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_nose_hoover_step_changes_state() {
        let mut sys = NoseHoover::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_nose_hoover_deterministic() {
        let mut sys1 = NoseHoover::new();
        let mut sys2 = NoseHoover::new();
        for _ in 0..500 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_nose_hoover_dt_zero_no_change() {
        let mut sys = NoseHoover::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0);
        let after = sys.state();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-15, "State changed with dt=0: {} -> {}", a, b);
        }
    }

    #[test]
    fn test_nose_hoover_set_state() {
        let mut sys = NoseHoover::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }
}
