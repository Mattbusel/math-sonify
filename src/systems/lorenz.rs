use super::{rk4, DynamicalSystem};

/// Lorenz attractor (Lorenz 1963) -- the classic strange attractor.
///
/// Equations of motion:
///
///   dx/dt = sigma * (y - x)
///   dy/dt = x * (rho - z) - y
///   dz/dt = x * y - beta * z
///
/// With the canonical parameters sigma=10, rho=28, beta=8/3 the trajectory
/// is confined to a bounded strange attractor (|x|, |y| < 30, 0 < z < 60).
/// Integration uses fourth-order Runge-Kutta (RK4) at the configured dt.
pub struct Lorenz {
    state: Vec<f64>,
    pub sigma: f64,
    pub rho: f64,
    pub beta: f64,
    speed: f64,
}

impl Lorenz {
    /// Creates a new Lorenz attractor with the given parameters and initial state `(1, 0, 0)`.
    ///
    /// # Parameters
    /// - `sigma`: Prandtl number; controls the rate of rotation around the z-axis.
    /// - `rho`: Rayleigh number; primary bifurcation parameter — chaos onset near 24.74.
    /// - `beta`: Geometric factor; typically set to `8/3 ≈ 2.6667`.
    ///
    /// # Returns
    /// A `Lorenz` instance ready for integration.
    pub fn new(sigma: f64, rho: f64, beta: f64) -> Self {
        Self {
            state: vec![1.0, 0.0, 0.0],
            sigma,
            rho,
            beta,
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
}

impl DynamicalSystem for Lorenz {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "Lorenz"
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

    /// Advances the attractor state by one RK4 integration step.
    ///
    /// # Parameters
    /// - `dt`: Time step size in simulation units.
    fn step(&mut self, dt: f64) {
        let (sigma, rho, beta) = (self.sigma, self.rho, self.beta);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, sigma, rho, beta));
        // Estimate speed as |Δstate|/dt
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
    fn test_lorenz_step_changes_state() {
        let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        let after = sys.state();
        // At least one component must change after a non-zero step
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
    fn test_lorenz_deterministic() {
        let mut sys1 = Lorenz::new(10.0, 28.0, 2.6667);
        let mut sys2 = Lorenz::new(10.0, 28.0, 2.6667);
        for _ in 0..500 {
            sys1.step(0.001);
            sys2.step(0.001);
        }
        let s1 = sys1.state();
        let s2 = sys2.state();
        assert_eq!(s1.len(), s2.len());
        for (a, b) in s1.iter().zip(s2.iter()) {
            assert!(
                (a - b).abs() < 1e-15,
                "Non-deterministic output: {} vs {}",
                a,
                b
            );
        }
    }

    #[test]
    fn test_lorenz_dt_zero_no_change() {
        let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0);
        let after = sys.state();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!(
                (a - b).abs() < 1e-15,
                "State changed with dt=0: {} -> {}",
                a,
                b
            );
        }
    }

    #[test]
    fn test_lorenz_initial_state() {
        let sys = Lorenz::new(10.0, 28.0, 2.6667);
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 1.0).abs() < 1e-15, "Expected x=1.0, got {}", s[0]);
        assert!((s[1] - 0.0).abs() < 1e-15, "Expected y=0.0, got {}", s[1]);
        assert!((s[2] - 0.0).abs() < 1e-15, "Expected z=0.0, got {}", s[2]);
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "Lorenz");
    }
}
