//! Coupled Map Lattice — 1D lattice of N logistic maps with nearest-neighbor coupling.
//!
//! Spatially-extended chaos. Each site is a logistic map coupled to its neighbors:
//!   x_i(t+1) = (1-ε) * f(x_i) + (ε/2) * (f(x_{i-1}) + f(x_{i+1}))
//!   f(x) = r * x * (1 - x)
//!
//! Periodic boundary conditions. N=16 sites.
//!
//! Character: ghostly, flickering, akin to interference patterns in a physical medium.
//! As coupling ε increases, spatial coherence emerges — "modes" appear in the lattice.
//! In sonification, site index maps to stereo position, so you hear spatial patterns
//! travel through the field left-to-right.
//!
//! Control parameters:
//!   r:   logistic growth rate (3.7–4.0 for chaos; 3.0 for period-doubling edge)
//!   eps: coupling strength (0 = independent maps; 1 = fully coupled = synchrony)

use super::DynamicalSystem;

const N: usize = 16;

pub struct CoupledMapLattice {
    state: Vec<f64>,
    pub r: f64,   // logistic parameter (3.5 to 4.0)
    pub eps: f64, // coupling (0 to 1)
    speed: f64,
}

impl CoupledMapLattice {
    pub fn new(r: f64, eps: f64) -> Self {
        // Seed with varied initial conditions to break symmetry
        let state: Vec<f64> = (0..N)
            .map(|i| {
                let t = i as f64 / N as f64;
                0.2 + 0.6 * (t * std::f64::consts::PI * 3.0).sin().abs()
            })
            .collect();
        Self {
            state,
            r,
            eps,
            speed: 0.0,
        }
    }

    fn logistic(x: f64, r: f64) -> f64 {
        r * x * (1.0 - x)
    }
}

impl DynamicalSystem for CoupledMapLattice {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        N
    }
    fn name(&self) -> &str {
        "Coupled Map Lattice"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, _state: &[f64]) -> Vec<f64> {
        // Discrete map: derivative not meaningful, return zeros
        vec![0.0; N]
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i].clamp(0.001, 0.999);
            }
        }
    }

    fn step(&mut self, _dt: f64) {
        // CML is a discrete-time map — dt is ignored, one call = one iteration.
        // Apply one step of the coupled logistic map.
        let r = self.r;
        let eps = self.eps;
        let prev = self.state.clone();

        for i in 0..N {
            let left = (i + N - 1) % N;
            let right = (i + 1) % N;
            let center = Self::logistic(prev[i], r);
            let fl = Self::logistic(prev[left], r);
            let fr = Self::logistic(prev[right], r);
            let val = (1.0 - eps) * center + (eps / 2.0) * (fl + fr);
            // Clamp to (0,1) — logistic map is only defined there
            self.state[i] = val.clamp(0.0001, 0.9999);
        }

        // Speed = max |Δx_i| across all sites
        self.speed = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f64, f64::max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_cml_initial_state() {
        let sys = CoupledMapLattice::new(3.7, 0.1);
        let s = sys.state();
        assert_eq!(s.len(), 16);
        assert_eq!(sys.dimension(), 16);
        assert_eq!(sys.name(), "Coupled Map Lattice");
        assert!(s.iter().all(|v| v.is_finite()));
        assert!(s.iter().all(|&v| v > 0.0 && v < 1.0), "Initial values should be in (0,1)");
    }

    #[test]
    fn test_cml_step_changes_state() {
        let mut sys = CoupledMapLattice::new(3.7, 0.1);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_cml_values_stay_in_unit_interval() {
        let mut sys = CoupledMapLattice::new(3.9, 0.2);
        for _ in 0..1000 {
            sys.step(0.01);
            for &v in sys.state().iter() {
                assert!(v > 0.0 && v < 1.0, "Value out of (0,1): {}", v);
            }
        }
    }

    #[test]
    fn test_cml_deterministic() {
        let mut s1 = CoupledMapLattice::new(3.7, 0.1);
        let mut s2 = CoupledMapLattice::new(3.7, 0.1);
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_cml_set_state_clamped() {
        let mut sys = CoupledMapLattice::new(3.7, 0.1);
        // set_state clamps values to [0.001, 0.999]
        let new_s: Vec<f64> = (0..16).map(|i| i as f64 * 0.06 + 0.01).collect();
        sys.set_state(&new_s);
        for &v in sys.state().iter() {
            assert!(v > 0.0 && v < 1.0, "set_state value out of (0,1): {}", v);
        }
    }
}
