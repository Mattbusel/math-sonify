use super::{rk4, DynamicalSystem};

/// Chen-Li hyperchaotic system (4D extension of the Chen attractor).
///
/// Equations:
/// ```text
/// x' =  a·(y - x) + w
/// y' = (c - a)·x - x·z + c·y
/// z' =  x·y - b·z
/// w' = -y·z + d·w
/// ```
///
/// Parameters a=35, b=3, c=28, d=-7 produce hyperchaos — two positive
/// Lyapunov exponents. The extra variable w injects energy into x while
/// the yz product in the w-equation creates additional instability.
///
/// Reference: Li, Y., Tang, W. & Chen, G. (2005). "Generating hyperchaos
/// via state feedback control." Int. J. Bifurc. Chaos 15(10), 3367–3375.
pub struct Hyperchaos {
    pub state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    speed: f64,
}

impl Hyperchaos {
    pub fn new() -> Self {
        Self {
            state: vec![1.0, 1.0, 0.0, 0.0],
            a: 35.0,
            b: 3.0,
            c: 28.0,
            d: -7.0,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
        vec![
            a * (s[1] - s[0]) + s[3],
            (c - a) * s[0] - s[0] * s[2] + c * s[1],
            s[0] * s[1] - b * s[2],
            -s[1] * s[2] + d * s[3],
        ]
    }
}

impl Default for Hyperchaos {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Hyperchaos {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        4
    }
    fn name(&self) -> &str {
        "hyperchaos"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b, self.c, self.d)
    }

    fn step(&mut self, dt: f64) {
        let (a, b, c, d) = (self.a, self.b, self.c, self.d);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, c, d));
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
    fn hyperchaos_initial_state_finite() {
        let sys = Hyperchaos::new();
        assert!(sys.state().iter().all(|v| v.is_finite()), "Initial state has non-finite values");
        assert_eq!(sys.dimension(), 4);
    }

    #[test]
    fn hyperchaos_stays_finite() {
        let mut sys = Hyperchaos::new();
        for _ in 0..5_000 {
            sys.step(0.001);
        }
        assert!(sys.state().iter().all(|v| v.is_finite()), "State became non-finite: {:?}", sys.state());
    }

    #[test]
    fn hyperchaos_state_bounded() {
        let mut sys = Hyperchaos::new();
        for _ in 0..5_000 {
            sys.step(0.001);
        }
        let s = sys.state();
        assert!(s[0].abs() < 50.0, "x out of range: {}", s[0]);
        assert!(s[1].abs() < 50.0, "y out of range: {}", s[1]);
        assert!(s[2].abs() < 100.0, "z out of range: {}", s[2]);
        assert!(s[3].abs() < 200.0, "w out of range: {}", s[3]);
    }

    #[test]
    fn hyperchaos_step_changes_state() {
        let mut sys = Hyperchaos::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        assert!(
            before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn hyperchaos_deterministic() {
        let mut s1 = Hyperchaos::new();
        let mut s2 = Hyperchaos::new();
        for _ in 0..500 {
            s1.step(0.001);
            s2.step(0.001);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn hyperchaos_set_state() {
        let mut sys = Hyperchaos::new();
        sys.set_state(&[1.0, 2.0, 3.0, 4.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
        assert!((s[3] - 4.0).abs() < 1e-15);
    }

    #[test]
    fn hyperchaos_deriv_at_origin() {
        let sys = Hyperchaos::new();
        // At (0,0,0,0): all derivatives should be 0
        let d = sys.deriv_at(&[0.0, 0.0, 0.0, 0.0]);
        for (i, di) in d.iter().enumerate() {
            assert!(di.abs() < 1e-14, "d[{}] should be 0 at origin: {}", i, di);
        }
    }
}
