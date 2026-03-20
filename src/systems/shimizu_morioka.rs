use super::{rk4, DynamicalSystem};

/// Shimizu-Morioka system: a two-scroll chaotic attractor.
///
/// Equations:
///   x' = y
///   y' = (1 - z) * x - a * y
///   z' = x² - b * z
///
/// Parameters: a=0.75, b=0.45 give a chaotic strange attractor.
/// The system models a nonlinear oscillator with a z-coupling term that
/// drives the characteristic two-scroll topology similar to Lorenz.
pub struct ShimizuMorioka {
    pub state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    speed: f64,
}

impl ShimizuMorioka {
    pub fn new() -> Self {
        Self {
            state: vec![0.1, 0.0, 0.0],
            a: 0.75,
            b: 0.45,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64) -> Vec<f64> {
        let (x, y, z) = (s[0], s[1], s[2]);
        vec![
            y,
            (1.0 - z) * x - a * y,
            x * x - b * z,
        ]
    }
}

impl Default for ShimizuMorioka {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for ShimizuMorioka {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "shimizu_morioka"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b)
    }

    fn step(&mut self, dt: f64) {
        let (a, b) = (self.a, self.b);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b));
        if !self.state.iter().all(|v| v.is_finite()) {
            self.state = prev;
            self.speed = 0.0;
            return;
        }
        self.speed = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
            / dt;
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn shimizu_morioka_initial_state_finite() {
        let sys = ShimizuMorioka::new();
        assert!(sys.state().iter().all(|v| v.is_finite()), "Initial state has non-finite values");
    }

    #[test]
    fn shimizu_morioka_stays_finite() {
        let mut sys = ShimizuMorioka::new();
        for _ in 0..10_000 {
            sys.step(0.01);
        }
        assert!(sys.state().iter().all(|v| v.is_finite()), "State became non-finite: {:?}", sys.state());
    }

    #[test]
    fn shimizu_morioka_state_bounded() {
        let mut sys = ShimizuMorioka::new();
        for _ in 0..10_000 {
            sys.step(0.01);
        }
        let s = sys.state();
        assert!(s[0].abs() < 15.0, "x out of range: {}", s[0]);
        assert!(s[1].abs() < 15.0, "y out of range: {}", s[1]);
        assert!(s[2].abs() < 5.0, "z out of range: {}", s[2]);
    }

    #[test]
    fn shimizu_morioka_step_changes_state() {
        let mut sys = ShimizuMorioka::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(
            before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn shimizu_morioka_deterministic() {
        let mut s1 = ShimizuMorioka::new();
        let mut s2 = ShimizuMorioka::new();
        for _ in 0..500 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn shimizu_morioka_set_state() {
        let mut sys = ShimizuMorioka::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn shimizu_morioka_deriv_at_origin() {
        let sys = ShimizuMorioka::new();
        let d = sys.deriv_at(&[0.0, 0.0, 0.0]);
        // At (0, 0, 0): x'=0, y'=(1-0)*0 - a*0=0, z'=0²-b*0=0
        assert!(d[0].abs() < 1e-15, "x' at origin should be 0: {}", d[0]);
        assert!(d[1].abs() < 1e-15, "y' at origin should be 0: {}", d[1]);
        assert!(d[2].abs() < 1e-15, "z' at origin should be 0: {}", d[2]);
    }
}
