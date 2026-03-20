use super::DynamicalSystem;
use std::f64::consts::TAU;

/// Chirikov–Taylor standard map — an area-preserving 2D discrete map.
///
/// Iteration rule (all angles in [0, 2π)):
/// ```text
/// p_{n+1} = (p + k · sin θ) mod 2π
/// θ_{n+1} = (θ + p_{n+1}) mod 2π
/// ```
/// For k = 0 the motion is integrable (circles in phase space).
/// For k ≈ 0.97 the last KAM torus breaks, yielding global chaos.
/// For large k the phase space is almost entirely chaotic.
///
/// The state is (θ, p, k) so the sonification pipeline gets both
/// the angle and the momentum as independent signal dimensions.
pub struct StandardMap {
    state: Vec<f64>,
    pub k: f64,
    speed: f64,
}

impl StandardMap {
    /// Create a standard map with the given stochasticity parameter k.
    /// Default k = 1.5 gives strong but not complete chaos.
    pub fn new(k: f64) -> Self {
        Self {
            state: vec![0.5, 0.5, k],
            k,
            speed: 0.0,
        }
    }
}

impl DynamicalSystem for StandardMap {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "standard_map"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, _state: &[f64]) -> Vec<f64> {
        vec![0.0; 3]
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }

    fn step(&mut self, _dt: f64) {
        let theta = self.state[0];
        let p = self.state[1];

        let new_p = (p + self.k * theta.sin()).rem_euclid(TAU);
        let new_theta = (theta + new_p).rem_euclid(TAU);

        let delta = ((new_theta - theta).powi(2) + (new_p - p).powi(2)).sqrt();
        self.speed = delta;

        self.state[0] = new_theta;
        self.state[1] = new_p;
        self.state[2] = self.k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;
    use std::f64::consts::TAU;

    #[test]
    fn test_standard_map_initial_state() {
        let sys = StandardMap::new(1.5);
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.5).abs() < 1e-15);
        assert!((s[1] - 0.5).abs() < 1e-15);
        assert!((s[2] - 1.5).abs() < 1e-15);
        assert_eq!(sys.name(), "standard_map");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_standard_map_step_changes_state() {
        let mut sys = StandardMap::new(1.5);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_standard_map_stays_in_torus() {
        let mut sys = StandardMap::new(1.5);
        for _ in 0..1000 {
            sys.step(0.01);
            let theta = sys.state()[0];
            let p = sys.state()[1];
            assert!(theta >= 0.0 && theta < TAU, "theta out of [0, 2π): {}", theta);
            assert!(p >= 0.0 && p < TAU, "p out of [0, 2π): {}", p);
        }
    }

    #[test]
    fn test_standard_map_deterministic() {
        let mut sys1 = StandardMap::new(1.5);
        let mut sys2 = StandardMap::new(1.5);
        for _ in 0..200 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_standard_map_k_zero_integrable() {
        // With k=0 the map becomes p_{n+1} = p, θ_{n+1} = θ + p (mod 2π).
        let mut sys = StandardMap::new(0.0);
        let p0 = sys.state()[1];
        let theta0 = sys.state()[0];
        sys.step(0.0);
        let theta1 = sys.state()[0];
        let expected = (theta0 + p0).rem_euclid(TAU);
        assert!((theta1 - expected).abs() < 1e-12, "k=0 should give theta += p");
    }
}
