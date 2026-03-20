use super::DynamicalSystem;

/// Logistic map — a classic 1D discrete chaotic map.
///
/// Iteration rule:
/// ```text
/// x_{n+1} = r · x · (1 − x)
/// ```
/// For r > 3.57 the map exhibits fully developed chaos.
/// Classic chaotic regime: r ≈ 3.9.
///
/// The state vector is 3D (x, r, 0) so the existing sonification
/// pipeline can use both the value and the bifurcation parameter.
pub struct LogisticMap {
    state: Vec<f64>,
    pub r: f64,
    speed: f64,
}

impl LogisticMap {
    /// Create a logistic map with the given r parameter.
    /// Default r=3.9 places it in the chaotic regime.
    pub fn new(r: f64) -> Self {
        Self {
            state: vec![0.5, r, 0.0],
            r,
            speed: 0.0,
        }
    }
}

impl DynamicalSystem for LogisticMap {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "logistic_map"
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
        let x = self.state[0].clamp(0.0, 1.0);
        let new_x = self.r * x * (1.0 - x);
        let new_x = if new_x.is_finite() {
            new_x.clamp(0.0, 1.0)
        } else {
            0.5
        };
        let delta = (new_x - x).abs();
        self.speed = delta;
        self.state[0] = new_x;
        self.state[1] = self.r;
        // state[2] stays 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_logistic_initial_state() {
        let sys = LogisticMap::new(3.9);
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.5).abs() < 1e-15, "Expected x=0.5, got {}", s[0]);
        assert!((s[1] - 3.9).abs() < 1e-15, "Expected r=3.9, got {}", s[1]);
        assert_eq!(sys.name(), "logistic_map");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_logistic_step_changes_state() {
        let mut sys = LogisticMap::new(3.9);
        let before = sys.state[0];
        sys.step(0.01);
        let after = sys.state[0];
        assert!(
            (before - after).abs() > 1e-15,
            "State[0] did not change after step"
        );
    }

    #[test]
    fn test_logistic_output_stays_in_unit_interval() {
        let mut sys = LogisticMap::new(3.9);
        for _ in 0..1000 {
            sys.step(0.01);
            let x = sys.state[0];
            assert!(
                (0.0..=1.0).contains(&x),
                "x escaped [0,1]: {}",
                x
            );
        }
    }

    #[test]
    fn test_logistic_deterministic() {
        let mut sys1 = LogisticMap::new(3.9);
        let mut sys2 = LogisticMap::new(3.9);
        for _ in 0..200 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        assert!(
            (sys1.state[0] - sys2.state[0]).abs() < 1e-15,
            "Non-deterministic output"
        );
    }

    #[test]
    fn test_logistic_r_stored_in_state() {
        let r = 3.7;
        let mut sys = LogisticMap::new(r);
        sys.step(0.01);
        assert!((sys.state[1] - r).abs() < 1e-15, "r not preserved in state[1]");
    }
}
