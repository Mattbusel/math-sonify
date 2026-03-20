use super::DynamicalSystem;

/// Arnold cat map — a hyperbolic toral automorphism on [0,1)².
///
/// Iteration rule:
/// ```text
/// x_{n+1} = (x + y)       mod 1
/// y_{n+1} = (x + 2·y)     mod 1
/// ```
/// This is a linear map on the 2-torus with matrix [[1,1],[1,2]],
/// which has eigenvalues (3 ± √5)/2 (the golden ratio and its inverse).
/// It is uniformly hyperbolic, ergodic, and mixing with Lyapunov
/// exponent λ = ln((3+√5)/2) ≈ 0.962.
///
/// The map is often illustrated with its distortion of a "cat" image,
/// which eventually returns to the original (periodic, but very long period).
pub struct ArnoldCat {
    state: Vec<f64>,
    speed: f64,
}

impl ArnoldCat {
    pub fn new() -> Self {
        // Start at an irrational point to avoid fixed points
        Self {
            state: vec![0.1, 0.4, 0.0],
            speed: 0.0,
        }
    }
}

impl Default for ArnoldCat {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for ArnoldCat {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "arnold_cat"
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
        let x = self.state[0];
        let y = self.state[1];

        let new_x = (x + y).rem_euclid(1.0);
        let new_y = (x + 2.0 * y).rem_euclid(1.0);

        let delta = ((new_x - x).powi(2) + (new_y - y).powi(2)).sqrt();
        self.speed = delta;

        self.state[0] = new_x;
        self.state[1] = new_y;
        // state[2] unused; leave at 0 or accumulate something interesting
        self.state[2] = (self.state[2] + new_x - new_y).rem_euclid(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_arnold_cat_initial_state() {
        let sys = ArnoldCat::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.1).abs() < 1e-15);
        assert!((s[1] - 0.4).abs() < 1e-15);
        assert_eq!(sys.name(), "arnold_cat");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_arnold_cat_step_changes_state() {
        let mut sys = ArnoldCat::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_arnold_cat_stays_in_unit_square() {
        let mut sys = ArnoldCat::new();
        for _ in 0..1000 {
            sys.step(0.01);
            let x = sys.state()[0];
            let y = sys.state()[1];
            assert!(x >= 0.0 && x < 1.0, "x out of [0,1): {}", x);
            assert!(y >= 0.0 && y < 1.0, "y out of [0,1): {}", y);
        }
    }

    #[test]
    fn test_arnold_cat_deterministic() {
        let mut sys1 = ArnoldCat::new();
        let mut sys2 = ArnoldCat::new();
        for _ in 0..200 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_arnold_cat_map_formula() {
        // Verify the map applies [[1,1],[1,2]] mod 1 correctly.
        let mut sys = ArnoldCat::new();
        let x0 = sys.state()[0];
        let y0 = sys.state()[1];
        sys.step(0.0);
        let x1 = sys.state()[0];
        let y1 = sys.state()[1];
        assert!((x1 - (x0 + y0).rem_euclid(1.0)).abs() < 1e-12, "x1 mismatch");
        assert!((y1 - (x0 + 2.0 * y0).rem_euclid(1.0)).abs() < 1e-12, "y1 mismatch");
    }
}
