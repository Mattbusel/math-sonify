use super::{rk4, DynamicalSystem};

/// Mathieu equation — parametric resonance.
///
/// 2D oscillator with time-varying stiffness:
///   dx/dt = v
///   dv/dt = -(a + 2*q*cos(2*t)) * x
///
/// The internal time t is carried as state[2] and advances each step.
/// `dimension()` returns 3 (including internal time) but the useful dimensions
/// for sonification are state[0] (displacement) and state[1] (velocity).
pub struct Mathieu {
    /// [x, v, t_internal]
    state: Vec<f64>,
    pub a: f64,
    pub q: f64,
    speed: f64,
}

impl Mathieu {
    pub fn new(a: f64, q: f64) -> Self {
        Self {
            state: vec![1.0, 0.0, 0.0],
            a,
            q,
            speed: 0.0,
        }
    }

    fn deriv(state: &[f64], a: f64, q: f64) -> Vec<f64> {
        let x = state[0];
        let v = state[1];
        let t = state[2];
        vec![
            v,
            -(a + 2.0 * q * (2.0 * t).cos()) * x,
            1.0, // dt/dt = 1
        ]
    }
}

impl DynamicalSystem for Mathieu {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "Mathieu"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.q)
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
        let (a, q) = (self.a, self.q);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |st| Self::deriv(st, a, q));
        let ds: f64 = self.state[0..2]
            .iter()
            .zip(prev[0..2].iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = ds / dt.max(1e-15);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_mathieu_initial_state() {
        let sys = Mathieu::new(0.0, 0.5);
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 1.0).abs() < 1e-15, "Expected x=1.0");
        assert!((s[1] - 0.0).abs() < 1e-15, "Expected v=0.0");
        assert!((s[2] - 0.0).abs() < 1e-15, "Expected t=0.0");
        assert_eq!(sys.name(), "Mathieu");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_mathieu_step_changes_state() {
        let mut sys = Mathieu::new(0.0, 0.5);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_mathieu_internal_time_advances() {
        let mut sys = Mathieu::new(0.0, 0.5);
        let dt = 0.01;
        sys.step(dt);
        // state[2] = internal time, should increase by approximately dt
        assert!(sys.state()[2] > 0.0, "Internal time should advance");
    }

    #[test]
    fn test_mathieu_deterministic() {
        let mut sys1 = Mathieu::new(0.0, 0.5);
        let mut sys2 = Mathieu::new(0.0, 0.5);
        for _ in 0..500 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_mathieu_set_state() {
        let mut sys = Mathieu::new(0.0, 0.5);
        sys.set_state(&[2.0, 1.0, 0.5]);
        let s = sys.state();
        assert!((s[0] - 2.0).abs() < 1e-15, "x should be 2: {}", s[0]);
        assert!((s[1] - 1.0).abs() < 1e-15, "v should be 1: {}", s[1]);
        assert!((s[2] - 0.5).abs() < 1e-15, "t should be 0.5: {}", s[2]);
    }

    #[test]
    fn test_mathieu_speed_positive_after_step() {
        let mut sys = Mathieu::new(0.0, 0.5);
        sys.step(0.01);
        assert!(sys.speed() > 0.0, "speed should be positive: {}", sys.speed());
    }

    #[test]
    fn test_mathieu_state_finite_long_run() {
        // In the stable zone (small a, q), the system should stay finite
        let mut sys = Mathieu::new(0.0, 0.3);
        for _ in 0..5000 {
            sys.step(0.01);
        }
        assert!(
            sys.state().iter().all(|v| v.is_finite()),
            "State should stay finite in stable zone: {:?}", sys.state()
        );
    }
}
