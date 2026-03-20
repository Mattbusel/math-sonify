use super::{rk4, DynamicalSystem};

/// Double pendulum in Hamiltonian form.
/// State: [θ1, θ2, p1, p2] where p_i are conjugate momenta.
/// Uses symplectic (leapfrog) integration.
pub struct DoublePendulum {
    state: Vec<f64>,
    pub m1: f64,
    pub m2: f64,
    pub l1: f64,
    pub l2: f64,
    g: f64,
    speed: f64,
}

impl DoublePendulum {
    /// Create a double pendulum with the given masses and arm lengths.
    ///
    /// # Parameters
    /// - `m1`, `m2`: Masses of the first and second bob (kg).
    /// - `l1`, `l2`: Lengths of the first and second arm (m).
    ///
    /// The initial state is θ₁ = θ₂ = π/2 (horizontal), momenta zero.
    pub fn new(m1: f64, m2: f64, l1: f64, l2: f64) -> Self {
        Self {
            // Start slightly off vertical for interesting dynamics
            state: vec![
                std::f64::consts::PI / 2.0,
                std::f64::consts::PI / 2.0 + 0.1,
                0.0,
                0.0,
            ],
            m1,
            m2,
            l1,
            l2,
            g: 9.81,
            speed: 0.0,
        }
    }

    fn d_theta(&self) -> (f64, f64) {
        let [th1, th2, p1, p2] = [self.state[0], self.state[1], self.state[2], self.state[3]];
        let (m1, m2, l1, l2) = (self.m1, self.m2, self.l1, self.l2);
        let delta = th2 - th1;
        let denom = m1 + m2 - m2 * delta.cos().powi(2);
        let dth1 = (m2 * l2 * p1 - m2 * l1 * p2 * delta.cos())
            / (m1 * m2 * l1.powi(2) * l2 * denom.max(1e-10));
        let dth2 = ((m1 + m2) * l1 * p2 - m2 * l2 * p1 * delta.cos())
            / (m1 * m2 * l1 * l2.powi(2) * denom.max(1e-10));
        (dth1, dth2)
    }

    fn d_p(&self) -> (f64, f64) {
        let [th1, th2, _p1, _p2] = [self.state[0], self.state[1], self.state[2], self.state[3]];
        let (m1, m2, l1, l2, g) = (self.m1, self.m2, self.l1, self.l2, self.g);
        let delta = th2 - th1;
        let (dth1, dth2) = self.d_theta();
        let dp1 = -(m1 + m2) * g * l1 * th1.sin() - m2 * l1 * l2 * dth1 * dth2 * delta.sin();
        let dp2 = -m2 * g * l2 * th2.sin() + m2 * l1 * l2 * dth1 * dth2 * delta.sin();
        (dp1, dp2)
    }
}

impl DynamicalSystem for DoublePendulum {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        4
    }
    fn name(&self) -> &str {
        "Double Pendulum"
    }
    fn speed(&self) -> f64 {
        self.speed
    }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        // Compute derivatives at an *arbitrary* state (needed for vector-field
        // visualisation and Lyapunov spectrum).  The original called self.d_theta /
        // self.d_p which always used self.state, so deriv_at(s) ≠ f(s) for s ≠ self.state.
        if state.len() < 4 {
            return vec![0.0; state.len()];
        }
        let (m1, m2, l1, l2, g) = (self.m1, self.m2, self.l1, self.l2, self.g);
        let (th1, th2, p1, p2) = (state[0], state[1], state[2], state[3]);
        let delta = th2 - th1;
        let denom = (m1 + m2 - m2 * delta.cos().powi(2)).max(1e-10);
        let dth1 =
            (m2 * l2 * p1 - m2 * l1 * p2 * delta.cos()) / (m1 * m2 * l1.powi(2) * l2 * denom);
        let dth2 = ((m1 + m2) * l1 * p2 - m2 * l2 * p1 * delta.cos())
            / (m1 * m2 * l1 * l2.powi(2) * denom);
        let dp1 = -(m1 + m2) * g * l1 * th1.sin() - m2 * l1 * l2 * dth1 * dth2 * delta.sin();
        let dp2 = -m2 * g * l2 * th2.sin() + m2 * l1 * l2 * dth1 * dth2 * delta.sin();
        vec![dth1, dth2, dp1, dp2]
    }

    fn step(&mut self, dt: f64) {
        // RK4 integration using the full canonical equations of motion.
        //
        // The Yoshida symplectic integrator was previously used here, but the
        // double pendulum's Hamiltonian is non-separable (kinetic energy depends
        // on both momenta AND angles through the mass matrix), so Yoshida cannot
        // preserve the symplectic structure and caused large energy drift.
        // RK4 conserves energy to O(dt^4) per step, sufficient for simulation use.
        let prev = self.state.clone();
        let (m1, m2, l1, l2, g) = (self.m1, self.m2, self.l1, self.l2, self.g);

        rk4(&mut self.state, dt, |s| {
            if s.len() < 4 {
                return vec![0.0; s.len()];
            }
            let (th1, th2, p1, p2) = (s[0], s[1], s[2], s[3]);
            let delta = th2 - th1;
            let denom = (m1 + m2 - m2 * delta.cos().powi(2)).max(1e-10);
            let dth1 = (m2 * l2 * p1 - m2 * l1 * p2 * delta.cos())
                / (m1 * m2 * l1.powi(2) * l2 * denom);
            let dth2 = ((m1 + m2) * l1 * p2 - m2 * l2 * p1 * delta.cos())
                / (m1 * m2 * l1 * l2.powi(2) * denom);
            let dp1 = -(m1 + m2) * g * l1 * th1.sin()
                - m2 * l1 * l2 * dth1 * dth2 * delta.sin();
            let dp2 = -m2 * g * l2 * th2.sin()
                + m2 * l1 * l2 * dth1 * dth2 * delta.sin();
            vec![dth1, dth2, dp1, dp2]
        });

        let ds: f64 = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = ds / dt;
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_double_pendulum_initial_state() {
        let sys = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
        let s = sys.state();
        assert_eq!(s.len(), 4);
        assert!(s.iter().all(|v| v.is_finite()), "Initial state has non-finite values");
        assert_eq!(sys.name(), "Double Pendulum");
        assert_eq!(sys.dimension(), 4);
    }

    #[test]
    fn test_double_pendulum_step_changes_state() {
        let mut sys = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_double_pendulum_deterministic() {
        let mut sys1 = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
        let mut sys2 = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
        for _ in 0..200 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_double_pendulum_state_stays_finite() {
        let mut sys = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
        for _ in 0..1000 {
            sys.step(0.01);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_double_pendulum_set_state() {
        let mut sys = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
        sys.set_state(&[0.1, 0.2, 0.3, 0.4]);
        let s = sys.state();
        assert!((s[0] - 0.1).abs() < 1e-15);
        assert!((s[1] - 0.2).abs() < 1e-15);
        assert!((s[2] - 0.3).abs() < 1e-15);
        assert!((s[3] - 0.4).abs() < 1e-15);
    }
}
