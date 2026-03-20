use super::{DynamicalSystem};

/// Lorenz attractor with additive Wiener (Gaussian) noise.
///
/// Same equations as Lorenz (sigma, rho, beta) plus a noise_strength parameter.
/// Each step adds `noise_strength * sqrt(dt) * N(0,1)` to each component.
/// Uses an inline xorshift64 PRNG seeded from the step counter — no external crates.
pub struct StochasticLorenz {
    state: Vec<f64>,
    pub sigma: f64,
    pub rho: f64,
    pub beta: f64,
    pub noise_strength: f64,
    step_count: u64,
    speed: f64,
}

impl StochasticLorenz {
    pub fn new(sigma: f64, rho: f64, beta: f64, noise_strength: f64) -> Self {
        Self {
            state: vec![1.0, 0.0, 0.0],
            sigma,
            rho,
            beta,
            noise_strength,
            step_count: 0,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], sigma: f64, rho: f64, beta: f64) -> Vec<f64> {
        vec![
            sigma * (s[1] - s[0]),
            s[0] * (rho - s[2]) - s[1],
            s[0] * s[1] - beta * s[2],
        ]
    }

    /// Xorshift64 PRNG — returns a value in [0, 1).
    fn xorshift64(state: &mut u64) -> f64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state as f64 / u64::MAX as f64
    }

    /// Box-Muller transform: converts two uniform samples to one N(0,1) sample.
    fn standard_normal(rng: &mut u64) -> f64 {
        use std::f64::consts::PI;
        let u1 = Self::xorshift64(rng).max(1e-15); // avoid log(0)
        let u2 = Self::xorshift64(rng);
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }
}

impl DynamicalSystem for StochasticLorenz {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "Stochastic Lorenz"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.sigma, self.rho, self.beta)
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
        let (sigma, rho, beta) = (self.sigma, self.rho, self.beta);
        let prev = self.state.clone();

        // RK4 deterministic part
        let n = self.state.len();
        let k1 = Self::deriv(&self.state, sigma, rho, beta);
        let s2: Vec<f64> = (0..n).map(|i| self.state[i] + 0.5 * dt * k1[i]).collect();
        let k2 = Self::deriv(&s2, sigma, rho, beta);
        let s3: Vec<f64> = (0..n).map(|i| self.state[i] + 0.5 * dt * k2[i]).collect();
        let k3 = Self::deriv(&s3, sigma, rho, beta);
        let s4: Vec<f64> = (0..n).map(|i| self.state[i] + dt * k3[i]).collect();
        let k4 = Self::deriv(&s4, sigma, rho, beta);
        for i in 0..n {
            self.state[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }

        // Additive Wiener noise: noise_strength * sqrt(dt) * N(0,1) per component
        let noise_scale = self.noise_strength * dt.sqrt();
        // Seed PRNG from step count, one unique seed per component
        for i in 0..n {
            let mut rng = self.step_count.wrapping_mul(2_654_435_761).wrapping_add(i as u64 * 1_234_567_891);
            if rng == 0 { rng = 1; }
            self.state[i] += noise_scale * Self::standard_normal(&mut rng);
        }

        self.step_count = self.step_count.wrapping_add(1);

        let ds: f64 = self
            .state
            .iter()
            .zip(prev.iter())
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
    fn test_stochastic_lorenz_initial_state() {
        let sys = StochasticLorenz::new(10.0, 28.0, 2.667, 0.0);
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 0.0).abs() < 1e-15);
        assert!((s[2] - 0.0).abs() < 1e-15);
        assert_eq!(sys.name(), "Stochastic Lorenz");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_stochastic_lorenz_noise_zero_matches_deterministic() {
        // With noise_strength=0, should match the deterministic Lorenz
        let mut sys = StochasticLorenz::new(10.0, 28.0, 2.667, 0.0);
        for _ in 0..500 {
            sys.step(0.001);
        }
        let s = sys.state();
        assert!(s.iter().all(|v| v.is_finite()), "State has non-finite value");
    }

    #[test]
    fn test_stochastic_lorenz_with_noise_stays_finite() {
        let mut sys = StochasticLorenz::new(10.0, 28.0, 2.667, 0.5);
        for _ in 0..500 {
            sys.step(0.001);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite with noise: {}", v);
        }
    }

    #[test]
    fn test_stochastic_lorenz_step_changes_state() {
        let mut sys = StochasticLorenz::new(10.0, 28.0, 2.667, 0.1);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_stochastic_lorenz_deterministic_zero_noise() {
        // Zero-noise instances with same params should produce identical output
        let mut sys1 = StochasticLorenz::new(10.0, 28.0, 2.667, 0.0);
        let mut sys2 = StochasticLorenz::new(10.0, 28.0, 2.667, 0.0);
        for _ in 0..500 {
            sys1.step(0.001);
            sys2.step(0.001);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Zero-noise should be deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_stochastic_lorenz_speed_positive() {
        let mut sys = StochasticLorenz::new(10.0, 28.0, 2.667, 0.5);
        sys.step(0.001);
        assert!(sys.speed() > 0.0, "speed should be positive: {}", sys.speed());
    }

    #[test]
    fn test_stochastic_lorenz_noise_causes_divergence_from_zero_noise() {
        // A noisy and a noise-free version should diverge over time
        let mut sys_noisy = StochasticLorenz::new(10.0, 28.0, 2.667, 1.0);
        let mut sys_clean = StochasticLorenz::new(10.0, 28.0, 2.667, 0.0);
        for _ in 0..500 {
            sys_noisy.step(0.001);
            sys_clean.step(0.001);
        }
        let d: f64 = sys_noisy.state().iter().zip(sys_clean.state().iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(d > 1e-6, "Noisy and clean should diverge: distance={}", d);
    }
}
