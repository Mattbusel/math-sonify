use super::DynamicalSystem;

/// Geodesic flow on a flat torus T² with major radius R and tube radius r.
/// State: [φ, θ, φ̇, θ̇] — angles and angular velocities.
/// On the flat torus (R→∞ limit), geodesics are straight lines — winding number
/// determines periodicity. On the embedded torus with metric, we solve the
/// geodesic equations from the Christoffel symbols.
///
/// The embedded metric is: ds² = (R + r·cos θ)²dφ² + r²dθ²
/// Geodesic equations (derived from Euler-Lagrange):
///   φ̈ = -2(r·sin θ / (R + r·cos θ)) · φ̇ · θ̇
///   θ̈ = (R + r·cos θ)·sin θ / r · φ̇²
pub struct GeodesicTorus {
    state: Vec<f64>,
    pub big_r: f64,
    pub small_r: f64,
    speed: f64,
    initial_metric_speed_sq: f64,
    metric_speed_error: f64,
}

impl GeodesicTorus {
    /// `big_r`: distance from tube center to torus center.
    /// `small_r`: tube radius.
    /// Initial velocity with components (dphi, dtheta) determines winding number.
    pub fn new(big_r: f64, small_r: f64) -> Self {
        // Winding number ≈ dphi/dtheta — use golden ratio for ergodic flow
        let phi_dot = 1.0;
        let theta_dot = 1.0 / 1.618_033_988_749; // irrational → ergodic
        let state = vec![0.0, 0.0, phi_dot, theta_dot];
        let initial_metric_speed_sq = Self::metric_speed_sq(&state, big_r, small_r);
        Self {
            state,
            big_r,
            small_r,
            speed: 0.0,
            initial_metric_speed_sq,
            metric_speed_error: 0.0,
        }
    }

    /// Metric speed squared: L = (R + r·cosθ)²·φ̇² + r²·θ̇²
    /// Conserved along geodesics (geodesic flow preserves kinetic energy).
    fn metric_speed_sq(s: &[f64], big_r: f64, small_r: f64) -> f64 {
        if s.len() < 4 {
            return 0.0;
        }
        let (theta, dphi, dtheta) = (s[1], s[2], s[3]);
        let factor = big_r + small_r * theta.cos();
        factor * factor * dphi * dphi + small_r * small_r * dtheta * dtheta
    }

    #[allow(clippy::similar_names)]
    fn deriv(s: &[f64], big_r: f64, small_r: f64) -> Vec<f64> {
        let (_phi, theta, dphi, dtheta) = (s[0], s[1], s[2], s[3]);
        let factor = big_r + small_r * theta.cos();
        let ddphi = -2.0 * (small_r * theta.sin() / factor.max(1e-10)) * dphi * dtheta;
        let ddtheta = factor * theta.sin() / small_r.max(1e-10) * dphi * dphi;
        vec![dphi, dtheta, ddphi, ddtheta]
    }
}

impl DynamicalSystem for GeodesicTorus {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        4
    }
    fn name(&self) -> &str {
        "Geodesic Torus"
    }
    fn speed(&self) -> f64 {
        self.speed
    }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.big_r, self.small_r)
    }

    fn energy_error(&self) -> Option<f64> {
        Some(self.metric_speed_error)
    }

    fn step(&mut self, dt: f64) {
        let (big_r, small_r) = (self.big_r, self.small_r);
        let prev = self.state.clone();
        super::rk4(&mut self.state, dt, |s| Self::deriv(s, big_r, small_r));
        // Wrap angles to [0, 2π)
        self.state[0] = self.state[0].rem_euclid(std::f64::consts::TAU);
        self.state[1] = self.state[1].rem_euclid(std::f64::consts::TAU);
        let ds: f64 = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = ds / dt;
        // Track metric speed conservation
        let l_now = Self::metric_speed_sq(&self.state, big_r, small_r);
        if self.initial_metric_speed_sq > 1e-10 {
            self.metric_speed_error =
                ((l_now - self.initial_metric_speed_sq) / self.initial_metric_speed_sq).abs();
        }
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
        // Recompute initial metric speed so energy_error stays meaningful after reset
        self.initial_metric_speed_sq =
            Self::metric_speed_sq(&self.state, self.big_r, self.small_r);
        self.metric_speed_error = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;
    use std::f64::consts::TAU;

    #[test]
    fn test_geodesic_torus_initial_state() {
        let sys = GeodesicTorus::new(3.0, 1.0);
        let s = sys.state();
        assert_eq!(s.len(), 4);
        assert!(s.iter().all(|v| v.is_finite()), "Initial state has non-finite values");
        assert_eq!(sys.name(), "Geodesic Torus");
        assert_eq!(sys.dimension(), 4);
    }

    #[test]
    fn test_geodesic_torus_step_changes_state() {
        let mut sys = GeodesicTorus::new(3.0, 1.0);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_geodesic_torus_angles_wrapped() {
        // After many steps, angles phi and theta should stay in [0, 2π)
        let mut sys = GeodesicTorus::new(3.0, 1.0);
        for _ in 0..1000 {
            sys.step(0.01);
        }
        let phi = sys.state()[0];
        let theta = sys.state()[1];
        assert!(phi >= 0.0 && phi < TAU, "phi out of [0, 2π): {}", phi);
        assert!(theta >= 0.0 && theta < TAU, "theta out of [0, 2π): {}", theta);
    }

    #[test]
    fn test_geodesic_torus_deterministic() {
        let mut sys1 = GeodesicTorus::new(3.0, 1.0);
        let mut sys2 = GeodesicTorus::new(3.0, 1.0);
        for _ in 0..200 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_geodesic_torus_set_state() {
        let mut sys = GeodesicTorus::new(3.0, 1.0);
        sys.set_state(&[1.0, 2.0, 0.5, 0.3]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 0.5).abs() < 1e-15);
        assert!((s[3] - 0.3).abs() < 1e-15);
    }

    #[test]
    fn test_geodesic_torus_metric_speed_conserved() {
        // Geodesic flow on a torus conserves L = (R+r·cosθ)²·φ̇² + r²·θ̇²
        let mut sys = GeodesicTorus::new(3.0, 1.0);
        for _ in 0..2000 {
            sys.step(0.01);
        }
        let drift = sys.energy_error().expect("energy_error should return Some");
        assert!(
            drift < 0.01,
            "Metric speed should be conserved to <1%: drift={}",
            drift
        );
    }

    #[test]
    fn test_geodesic_torus_energy_error_starts_zero() {
        let sys = GeodesicTorus::new(3.0, 1.0);
        assert_eq!(sys.energy_error(), Some(0.0));
    }

    #[test]
    fn test_geodesic_torus_energy_resets_after_set_state() {
        let mut sys = GeodesicTorus::new(3.0, 1.0);
        for _ in 0..500 {
            sys.step(0.01);
        }
        // Reset to a state with different velocity
        sys.set_state(&[0.0, 0.0, 2.0, 1.0]);
        assert_eq!(
            sys.energy_error(),
            Some(0.0),
            "energy_error should reset to 0 after set_state"
        );
    }
}
