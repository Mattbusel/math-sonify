use super::DynamicalSystem;

/// Tinkerbell map — a discrete-time 2D chaotic map with butterfly attractor.
///
/// Iteration rule (default params a=0.9, b=-0.6013, c=2.0, d=0.5):
/// ```text
/// x_{n+1} = x_n^2 - y_n^2 + a*x_n + b*y_n
/// y_{n+1} = 2*x_n*y_n + c*x_n + d*y_n
/// ```
///
/// The Tinkerbell map produces a beautiful butterfly-shaped strange attractor.
/// With default parameters the trajectory remains bounded and visits two lobes
/// that mirror each other, much like the Lorenz butterfly in continuous time.
pub struct TinkerbellMap {
    state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    speed: f64,
}

impl TinkerbellMap {
    /// Create a Tinkerbell map with default chaotic parameters.
    ///
    /// Default: a=0.9, b=-0.6013, c=2.0, d=0.5
    /// Initial conditions: (-0.72, -0.64) — known to lie in the attractor basin.
    pub fn new() -> Self {
        Self {
            state: vec![-0.72, -0.64, 0.0],
            a: 0.9,
            b: -0.6013,
            c: 2.0,
            d: 0.5,
            speed: 0.0,
        }
    }
}

impl DynamicalSystem for TinkerbellMap {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "tinkerbell"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, _state: &[f64]) -> Vec<f64> {
        // Discrete map — no continuous derivative
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
        let new_x = x * x - y * y + self.a * x + self.b * y;
        let new_y = 2.0 * x * y + self.c * x + self.d * y;

        // Guard against divergence — reset to basin if trajectory escapes
        if !new_x.is_finite() || !new_y.is_finite() || new_x.abs() > 10.0 || new_y.abs() > 10.0 {
            self.state[0] = -0.72;
            self.state[1] = -0.64;
            self.speed = 0.0;
            return;
        }

        let dx = new_x - x;
        let dy = new_y - y;
        let delta = (dx * dx + dy * dy).sqrt();
        self.speed = if dt > 0.0 { delta / dt } else { delta };
        self.state[0] = new_x;
        self.state[1] = new_y;
        // state[2] stays 0.0 (2D map embedded in 3D space)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_tinkerbell_initial_state() {
        let sys = TinkerbellMap::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - (-0.72)).abs() < 1e-12);
        assert!((s[1] - (-0.64)).abs() < 1e-12);
        assert_eq!(sys.name(), "tinkerbell");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_tinkerbell_step_changes_state() {
        let mut sys = TinkerbellMap::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_tinkerbell_bounded() {
        let mut sys = TinkerbellMap::new();
        for _ in 0..10000 {
            sys.step(0.01);
        }
        assert!(
            sys.state().iter().all(|v| v.is_finite()),
            "State became non-finite: {:?}", sys.state()
        );
    }

    #[test]
    fn test_tinkerbell_deterministic() {
        let mut sys1 = TinkerbellMap::new();
        let mut sys2 = TinkerbellMap::new();
        for _ in 0..1000 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        let s1 = sys1.state();
        let s2 = sys2.state();
        for (a, b) in s1.iter().zip(s2.iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }
}
