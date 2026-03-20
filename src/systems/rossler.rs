use super::{rk4, DynamicalSystem};

/// Rossler attractor (Rossler 1976) -- a three-dimensional spiral strange attractor.
///
/// Equations of motion:
///
///   dx/dt = -y - z
///   dy/dt = x + a*y
///   dz/dt = b + z*(x - c)
///
/// With a=0.2, b=0.2, c=5.7 the system exhibits a near-periodic orbit with
/// an approximate period of 5.9 time units.  Increasing `c` causes
/// period-doubling bifurcations that lead to full chaos.
/// Integration uses fourth-order Runge-Kutta (RK4).
pub struct Rossler {
    state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    speed: f64,
}

impl Rossler {
    /// Creates a new Rössler attractor with the given parameters and initial state `(1, 0, 0)`.
    ///
    /// # Parameters
    /// - `a`: Controls the y-feedback; increasing `a` toward ~0.398 leads to chaos.
    /// - `b`: Additive constant in the z-equation; typically small (e.g. 0.2).
    /// - `c`: Shifts the z-nullcline; chaos is robust for `c` around 5.7.
    ///
    /// # Returns
    /// A `Rossler` instance ready for integration.
    pub fn new(a: f64, b: f64, c: f64) -> Self {
        Self {
            state: vec![1.0, 0.0, 0.0],
            a,
            b,
            c,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64, c: f64) -> Vec<f64> {
        vec![-s[1] - s[2], s[0] + a * s[1], b + s[2] * (s[0] - c)]
    }
}

impl DynamicalSystem for Rossler {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "Rössler"
    }
    fn speed(&self) -> f64 {
        self.speed
    }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b, self.c)
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }

    /// Advances the attractor state by one RK4 integration step.
    ///
    /// # Parameters
    /// - `dt`: Time step size in simulation units.
    fn step(&mut self, dt: f64) {
        let (a, b, c) = (self.a, self.b, self.c);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, c));
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
    fn test_rossler_step_changes_state() {
        let mut sys = Rossler::new(0.2, 0.2, 5.7);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        let after = sys.state();
        assert!(
            before
                .iter()
                .zip(after.iter())
                .any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step: {:?} -> {:?}",
            before,
            after
        );
    }

    #[test]
    fn test_rossler_set_state() {
        let mut sys = Rossler::new(0.2, 0.2, 5.7);
        sys.set_state(&[2.0, 3.0, 4.0]);
        let s = sys.state();
        assert!((s[0] - 2.0).abs() < 1e-15);
        assert!((s[1] - 3.0).abs() < 1e-15);
        assert!((s[2] - 4.0).abs() < 1e-15);
    }

    #[test]
    fn test_rossler_set_state_ignores_nan() {
        let mut sys = Rossler::new(0.2, 0.2, 5.7);
        sys.set_state(&[f64::NAN, 3.0, 4.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15, "NaN should not change state[0]");
        assert!((s[1] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_rossler_period_positive_a() {
        // With standard chaotic parameters, state stays finite and x oscillates in a
        // bounded range after 1000 steps — the attractor is known to stay near [-15, 15].
        let mut sys = Rossler::new(0.2, 0.2, 5.7);
        for _ in 0..1000 {
            sys.step(0.001);
        }
        let s = sys.state();
        assert!(
            s.iter().all(|v| v.is_finite()),
            "State contains NaN/Inf: {:?}",
            s
        );
        assert!(
            s[0].abs() < 30.0 && s[1].abs() < 30.0,
            "x/y out of expected bounds after 1000 steps: {:?}",
            s
        );
    }
}
