use super::{rk4, DynamicalSystem};

/// Dadras attractor — a five-parameter three-dimensional chaotic system.
///
/// Equations:
/// ```text
/// dx/dt = y − a·x + b·y·z
/// dy/dt = c·y − x·z + z
/// dz/dt = d·x·y − e·z
/// ```
///
/// Default parameters (a=3, b=2.7, c=1.7, d=2, e=9) produce a robust
/// strange attractor with a distinctive multi-lobed topology distinct from
/// the Lorenz butterfly.  The Dadras system is notable for its very large
/// basin of attraction and predictable chaotic behaviour across a wide
/// parameter range.
pub struct Dadras {
    state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    speed: f64,
}

impl Dadras {
    /// Create a Dadras attractor with default parameters (a=3, b=2.7, c=1.7, d=2, e=9).
    pub fn new() -> Self {
        Self {
            state: vec![1.0, 1.0, 0.0],
            a: 3.0,
            b: 2.7,
            c: 1.7,
            d: 2.0,
            e: 9.0,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64, c: f64, d: f64, e: f64) -> Vec<f64> {
        vec![
            s[1] - a * s[0] + b * s[1] * s[2],
            c * s[1] - s[0] * s[2] + s[2],
            d * s[0] * s[1] - e * s[2],
        ]
    }
}

impl Default for Dadras {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Dadras {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "dadras"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b, self.c, self.d, self.e)
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
        let prev = self.state.clone();
        let (a, b, c, d, e) = (self.a, self.b, self.c, self.d, self.e);
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, c, d, e));
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
    fn test_dadras_initial_state() {
        let sys = Dadras::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "dadras");
        let s = sys.state();
        assert!(s.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_dadras_step_changes_state() {
        let mut sys = Dadras::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_dadras_state_stays_finite() {
        let mut sys = Dadras::new();
        for _ in 0..1000 {
            sys.step(0.01);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_dadras_deterministic() {
        let mut s1 = Dadras::new();
        let mut s2 = Dadras::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_dadras_set_state() {
        let mut sys = Dadras::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_dadras_deriv_at() {
        // At (1, 1, 0): dx = 1 - 3 + 0 = -2, dy = 1.7 - 0 + 0 = 1.7, dz = 2 - 0 = 2
        let sys = Dadras::new();
        let d = sys.deriv_at(&[1.0, 1.0, 0.0]);
        assert!((d[0] - (-2.0)).abs() < 1e-12, "d[0] expected -2.0, got {}", d[0]);
        assert!((d[1] - 1.7).abs() < 1e-12, "d[1] expected 1.7, got {}", d[1]);
        assert!((d[2] - 2.0).abs() < 1e-12, "d[2] expected 2.0, got {}", d[2]);
    }
}
