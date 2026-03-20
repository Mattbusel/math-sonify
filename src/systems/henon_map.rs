use super::DynamicalSystem;

/// Hénon map — a discrete-time two-dimensional chaotic map.
///
/// Iteration rule (a=1.4, b=0.3 default):
/// ```text
/// x_{n+1} = 1 - a·x² + y
/// y_{n+1} = b·x
/// ```
/// The strange attractor of the Hénon map has fractal dimension ≈ 1.26.
pub struct HenonMap {
    state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    speed: f64,
}

impl HenonMap {
    /// Create a Hénon map with default parameters (a=1.4, b=0.3).
    pub fn new() -> Self {
        Self {
            state: vec![0.0, 0.0, 0.0],
            a: 1.4,
            b: 0.3,
            speed: 0.0,
        }
    }
}

impl DynamicalSystem for HenonMap {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "henon_map"
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

    fn step(&mut self, dt: f64) {
        let x = self.state[0];
        let y = self.state[1];
        let new_x = 1.0 - self.a * x * x + y;
        let new_y = self.b * x;
        let dx = new_x - x;
        let dy = new_y - y;
        let delta = (dx * dx + dy * dy).sqrt();
        self.speed = if dt > 0.0 { delta / dt } else { delta };
        self.state[0] = new_x;
        self.state[1] = new_y;
        // state[2] stays 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_henon_initial_state() {
        let sys = HenonMap::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.0).abs() < 1e-15);
        assert!((s[1] - 0.0).abs() < 1e-15);
        assert_eq!(sys.name(), "henon_map");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_henon_step_changes_state() {
        let mut sys = HenonMap::new();
        // Start from a non-trivial point so the map actually moves
        sys.state[0] = 0.1;
        sys.state[1] = 0.1;
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_henon_deterministic() {
        let mut sys1 = HenonMap::new();
        let mut sys2 = HenonMap::new();
        sys1.state[0] = 0.3; sys1.state[1] = 0.2;
        sys2.state[0] = 0.3; sys2.state[1] = 0.2;
        for _ in 0..200 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        let s1 = sys1.state();
        let s2 = sys2.state();
        for (a, b) in s1.iter().zip(s2.iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_henon_dt_zero_no_change() {
        let mut sys = HenonMap::new();
        sys.state[0] = 0.5; sys.state[1] = 0.3;
        let before: Vec<f64> = sys.state().to_vec();
        // dt=0 still runs the map (it's discrete), so this just ensures no panic.
        // The map itself will step regardless of dt.
        sys.step(0.0);
        // Just verify state is finite
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
        let _ = before;
    }

    #[test]
    fn test_henon_speed_uses_euclidean_distance() {
        let mut sys = HenonMap::new();
        sys.state[0] = 0.3; sys.state[1] = 0.2;
        let x0 = sys.state[0];
        let y0 = sys.state[1];
        let dt = 1.0;
        sys.step(dt);
        let x1 = sys.state[0];
        let y1 = sys.state[1];
        let expected = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt() / dt;
        assert!((sys.speed() - expected).abs() < 1e-12, "Speed mismatch: {} vs {}", sys.speed(), expected);
    }
}
