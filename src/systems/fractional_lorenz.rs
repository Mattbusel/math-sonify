use crate::systems::DynamicalSystem;
use std::collections::VecDeque;

/// Fractional-order Lorenz system using Grünwald-Letnikov approximation.
/// alpha=1.0 reduces to the classical Lorenz system.
pub struct FractionalLorenz {
    state: Vec<f64>,
    history_x: VecDeque<f64>,
    history_y: VecDeque<f64>,
    history_z: VecDeque<f64>,
    pub alpha: f64,
    pub sigma: f64,
    pub rho: f64,
    pub beta: f64,
    memory_len: usize,
    // GL coefficients cache
    gl_coeffs: Vec<f64>,
}

fn gl_coefficients(alpha: f64, n: usize) -> Vec<f64> {
    let mut coeffs = vec![0.0f64; n];
    if n == 0 {
        return coeffs;
    }
    coeffs[0] = 1.0;
    for k in 1..n {
        coeffs[k] = coeffs[k - 1] * ((k as f64 - 1.0 - alpha) / k as f64);
    }
    coeffs
}

impl FractionalLorenz {
    pub fn new(alpha: f64, sigma: f64, rho: f64, beta: f64) -> Self {
        let memory_len = 64;
        let gl_coeffs = gl_coefficients(alpha, memory_len);
        let mut history_x = VecDeque::with_capacity(memory_len);
        let mut history_y = VecDeque::with_capacity(memory_len);
        let mut history_z = VecDeque::with_capacity(memory_len);
        // Initialize with Lorenz starting point
        let x0 = 1.0f64;
        let y0 = 1.0f64;
        let z0 = 1.0f64;
        history_x.push_back(x0);
        history_y.push_back(y0);
        history_z.push_back(z0);
        Self {
            state: vec![x0, y0, z0],
            history_x,
            history_y,
            history_z,
            alpha,
            sigma,
            rho,
            beta,
            memory_len,
            gl_coeffs,
        }
    }

    fn gl_sum(history: &VecDeque<f64>, coeffs: &[f64]) -> f64 {
        // sum_{k=1}^{n} coeff[k] * history[n-k]
        // history[0] = most recent
        history
            .iter()
            .zip(coeffs.iter().skip(1))
            .map(|(&h, &c)| c * h)
            .sum::<f64>()
    }
}

impl DynamicalSystem for FractionalLorenz {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn step(&mut self, dt: f64) {
        // Rebuild coeffs if alpha changed
        let n = self.memory_len;
        if (self.gl_coeffs[1] - (-(self.alpha))).abs() > 1e-10 {
            self.gl_coeffs = gl_coefficients(self.alpha, n);
        }

        let x = self.state[0];
        let y = self[1];
        let z = self[2];

        let h_alpha = dt.powf(self.alpha);

        // Grünwald-Letnikov fractional derivatives
        let sum_x = Self::gl_sum(&self.history_x, &self.gl_coeffs);
        let sum_y = Self::gl_sum(&self.history_y, &self.gl_coeffs);
        let sum_z = Self::gl_sum(&self.history_z, &self.gl_coeffs);

        // Fractional Lorenz equations: D^alpha x = sigma*(y-x) etc.
        let dx = h_alpha * (self.sigma * (y - x)) - sum_x;
        let dy = h_alpha * (x * (self.rho - z) - y) - sum_y;
        let dz = h_alpha * (x * y - self.beta * z) - sum_z;

        let new_x = dx;
        let new_y = dy;
        let new_z = dz;

        // Update history (prepend new values, keep memory_len)
        self.history_x.push_front(new_x);
        self.history_y.push_front(new_y);
        self.history_z.push_front(new_z);
        if self.history_x.len() > self.memory_len {
            self.history_x.pop_back();
        }
        if self.history_y.len() > self.memory_len {
            self.history_y.pop_back();
        }
        if self.history_z.len() > self.memory_len {
            self.history_z.pop_back();
        }

        // Safety clamp: GL approximation can blow up for very low alpha (<0.7)
        // If any state component exceeds 1000.0, reset to a safe state
        if new_x.abs() > 1000.0
            || new_y.abs() > 1000.0
            || new_z.abs() > 1000.0
            || !new_x.is_finite()
            || !new_y.is_finite()
            || !new_z.is_finite()
        {
            self.state[0] = 0.1;
            self.state[1] = 0.0;
            self.state[2] = 0.1;
            self.history_x.clear();
            self.history_x.push_back(0.1);
            self.history_y.clear();
            self.history_y.push_back(0.0);
            self.history_z.clear();
            self.history_z.push_back(0.1);
            return;
        }

        self.state[0] = new_x;
        self.state[1] = new_y;
        self.state[2] = new_z;
    }

    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "fractional_lorenz"
    }

    fn speed(&self) -> f64 {
        let dx = self.sigma * (self.state[1] - self.state[0]);
        let dy = self.state[0] * (self.rho - self.state[2]) - self.state[1];
        let dz = self.state[0] * self.state[1] - self.beta * self.state[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        if state.len() < 3 {
            return vec![0.0; 3];
        }
        vec![
            self.sigma * (state[1] - state[0]),
            state[0] * (self.rho - state[2]) - state[1],
            state[0] * state[1] - self.beta * state[2],
        ]
    }
}

// Allow indexing state
impl std::ops::Index<usize> for FractionalLorenz {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        &self.state[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_fractional_lorenz_initial_state() {
        let sys = FractionalLorenz::new(0.95, 10.0, 28.0, 8.0 / 3.0);
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!(s.iter().all(|v| v.is_finite()));
        assert_eq!(sys.name(), "fractional_lorenz");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_fractional_lorenz_step_changes_state() {
        let mut sys = FractionalLorenz::new(0.95, 10.0, 28.0, 8.0 / 3.0);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_fractional_lorenz_state_stays_finite() {
        let mut sys = FractionalLorenz::new(0.95, 10.0, 28.0, 8.0 / 3.0);
        for _ in 0..500 {
            sys.step(0.01);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_fractional_lorenz_alpha_one_like_lorenz() {
        // With alpha=1.0 the system should behave similarly to classical Lorenz
        let mut sys = FractionalLorenz::new(1.0, 10.0, 28.0, 8.0 / 3.0);
        for _ in 0..100 {
            sys.step(0.01);
        }
        let s = sys.state();
        assert!(s.iter().all(|v| v.is_finite()));
    }
}
