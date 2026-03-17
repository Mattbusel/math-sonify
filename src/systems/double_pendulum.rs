use super::{DynamicalSystem, yoshida4};

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
    pub fn new(m1: f64, m2: f64, l1: f64, l2: f64) -> Self {
        Self {
            // Start slightly off vertical for interesting dynamics
            state: vec![std::f64::consts::PI / 2.0, std::f64::consts::PI / 2.0 + 0.1, 0.0, 0.0],
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
        let [th1, th2, p1, p2] = [self.state[0], self.state[1], self.state[2], self.state[3]];
        let (m1, m2, l1, l2, g) = (self.m1, self.m2, self.l1, self.l2, self.g);
        let delta = th2 - th1;
        let (dth1, dth2) = self.d_theta();
        let dp1 = -(m1 + m2) * g * l1 * th1.sin()
            - m2 * l1 * l2 * dth1 * dth2 * delta.sin()
            + (p1 * p2 * delta.sin())
                / ((m1 + m2) * l1.powi(2) / (m2 * l2.powi(2)).max(1e-10));
        // Use energy-conserving form via the full Hamiltonian partial derivative
        let dp1_exact = -(m1 + m2) * g * l1 * th1.sin()
            - m2 * l1 * l2 * dth1 * dth2 * delta.sin();
        let dp2_exact = -m2 * g * l2 * th2.sin()
            + m2 * l1 * l2 * dth1 * dth2 * delta.sin();
        let _ = dp1;
        (dp1_exact, dp2_exact)
    }
}

impl DynamicalSystem for DoublePendulum {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 4 }
    fn name(&self) -> &str { "Double Pendulum" }
    fn speed(&self) -> f64 { self.speed }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        // Compute derivatives at an *arbitrary* state (needed for vector-field
        // visualisation and Lyapunov spectrum).  The original called self.d_theta /
        // self.d_p which always used self.state, so deriv_at(s) ≠ f(s) for s ≠ self.state.
        if state.len() < 4 { return vec![0.0; state.len()]; }
        let (m1, m2, l1, l2, g) = (self.m1, self.m2, self.l1, self.l2, self.g);
        let (th1, th2, p1, p2) = (state[0], state[1], state[2], state[3]);
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
    }

    fn step(&mut self, dt: f64) {
        // Yoshida 4th-order symplectic integrator
        let prev = self.state.clone();
        let (m1, m2, l1, l2, g) = (self.m1, self.m2, self.l1, self.l2, self.g);

        let velocity = |s: &[f64]| -> Vec<f64> {
            let (th1, th2, p1, p2) = (s[0], s[1], s[2], s[3]);
            let delta = th2 - th1;
            let denom = (m1 + m2 - m2 * delta.cos().powi(2)).max(1e-10);
            let dth1 = (m2 * l2 * p1 - m2 * l1 * p2 * delta.cos())
                / (m1 * m2 * l1.powi(2) * l2 * denom);
            let dth2 = ((m1 + m2) * l1 * p2 - m2 * l2 * p1 * delta.cos())
                / (m1 * m2 * l1 * l2.powi(2) * denom);
            vec![dth1, dth2]
        };

        let force = |s: &[f64]| -> Vec<f64> {
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
            vec![dp1, dp2]
        };

        yoshida4(&mut self.state, &[0, 1], &[2, 3], dt, velocity, force);

        let ds: f64 = self.state.iter().zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
        self.speed = ds / dt;
    }
}
