use super::DynamicalSystem;

/// Mackey-Glass delay differential equation approximated with a ring buffer.
/// dx/dt = β*x(t-τ)/(1 + x(t-τ)^n) - γ*x
pub struct MackeyGlass {
    history: Vec<f64>,
    head: usize, // index of oldest value (= x(t-τ))
    buf_len: usize,
    current_x: f64,
    pub beta: f64,
    pub gamma: f64,
    pub tau: f64,
    pub n: f64,
    dt: f64,
    speed: f64,
    /// Previous derivative, used by Adams-Bashforth 2 predictor
    prev_deriv: f64,
    /// 3-element observable: [x(t), x(t-τ/3), x(t-2τ/3)]
    observable: Vec<f64>,
}

impl MackeyGlass {
    /// Creates a Mackey-Glass system with default physiological parameters.
    ///
    /// Defaults: β=0.2, γ=0.1, τ=17.0, n=10.  With τ=17 the system is chaotic;
    /// τ<7 gives a stable limit cycle; τ>17 increases the complexity of the attractor.
    /// The history buffer is pre-filled with x₀=1.5 (near the stable fixed point without delay).
    pub fn new() -> Self {
        let beta = 0.2;
        let gamma = 0.1;
        let tau = 17.0;
        let n_param = 10.0;
        let dt = 0.5;
        let buf_len = ((tau / dt) as f64).ceil() as usize + 1;
        let initial_x = 1.5;
        let observable = vec![initial_x; 3];
        Self {
            history: vec![initial_x; buf_len],
            head: 0,
            buf_len,
            current_x: initial_x,
            beta,
            gamma,
            n: n_param,
            tau,
            dt,
            speed: 0.0,
            prev_deriv: 0.0,
            observable,
        }
    }

    fn delayed_x(&self) -> f64 {
        self.history[self.head]
    }

    fn mg_deriv(&self, x: f64, x_delayed: f64) -> f64 {
        self.beta * x_delayed / (1.0 + x_delayed.powf(self.n)) - self.gamma * x
    }

    fn update_observable(&mut self) {
        let n = self.buf_len;
        // x(t) = current_x
        // x(t - τ/3): ~1/3 of the way back in the ring buffer
        let offset_third = (n / 3).max(1);
        // x(t - 2τ/3): ~2/3 of the way back
        let offset_two_thirds = (2 * n / 3).max(1);
        // The head points to x(t-τ) (oldest). Current value was just written to (head-1+n)%n.
        let cur_idx = (self.head + n - 1) % n;
        let third_idx = (self.head + n - 1 + n - offset_third) % n;
        let two_thirds_idx = (self.head + n - 1 + n - offset_two_thirds) % n;
        self.observable[0] = self.history[cur_idx];
        self.observable[1] = self.history[third_idx];
        self.observable[2] = self.history[two_thirds_idx];
    }
}

impl DynamicalSystem for MackeyGlass {
    fn state(&self) -> &[f64] {
        &self.observable
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "mackey_glass"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, _state: &[f64]) -> Vec<f64> {
        vec![0.0; 3]
    }

    fn step(&mut self, _dt: f64) {
        let x_delayed = self.delayed_x();
        let deriv_curr = self.mg_deriv(self.current_x, x_delayed);

        // Adams-Bashforth 2 predictor: x_pred = x + dt*(1.5*f_curr - 0.5*f_prev)
        let x_pred = self.current_x + self.dt * (1.5 * deriv_curr - 0.5 * self.prev_deriv);

        // Compute derivative at predicted state (use same delayed value as approximation)
        let deriv_pred = self.mg_deriv(x_pred, x_delayed);

        // Adams-Moulton 2 corrector: x_new = x + dt*0.5*(f_curr + f_pred)
        let new_x = self.current_x + self.dt * 0.5 * (deriv_curr + deriv_pred);

        // Update previous derivative before advancing
        self.prev_deriv = deriv_curr;

        // Overwrite the oldest slot with the new value
        self.history[self.head] = new_x;
        self.head = (self.head + 1) % self.buf_len;
        self.speed = deriv_curr.abs();
        self.current_x = new_x;
        self.update_observable();
    }

    fn set_state(&mut self, s: &[f64]) {
        if let Some(&v) = s.first() {
            if v.is_finite() {
                self.current_x = v;
                self.prev_deriv = 0.0;
                for slot in &mut self.history {
                    *slot = v;
                }
                for o in &mut self.observable {
                    *o = v;
                }
            }
        }
    }
}
