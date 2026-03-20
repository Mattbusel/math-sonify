use super::{rk4, DynamicalSystem};

/// Aizawa toroidal attractor — exhibits a slow polar wobble around a torus-like surface.
///
/// Parameters: a, b, c, d, e, f (defaults: 0.95, 0.7, 0.6, 3.5, 0.25, 0.1).
pub struct Aizawa {
    state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Aizawa {
    /// Create an Aizawa attractor with default parameters.
    pub fn new() -> Self {
        Self {
            state: vec![0.1, 0.0, 0.0],
            a: 0.95,
            b: 0.7,
            c: 0.6,
            d: 3.5,
            e: 0.25,
            f: 0.1,
        }
    }

    #[allow(clippy::many_single_char_names)]
    fn deriv(state: &[f64], a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Vec<f64> {
        let x = state[0];
        let y = state[1];
        let z = state[2];
        vec![
            (z - b) * x - d * y,
            d * x + (z - b) * y,
            c + a * z - z * z * z / 3.0 - (x * x + y * y) * (1.0 + e * z) + f * z * x * x * x,
        ]
    }
}

impl DynamicalSystem for Aizawa {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "Aizawa"
    }

    #[allow(clippy::many_single_char_names)]
    fn step(&mut self, dt: f64) {
        let (a, b, c, d, e, f) = (self.a, self.b, self.c, self.d, self.e, self.f);
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, c, d, e, f));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b, self.c, self.d, self.e, self.f)
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }

    fn speed(&self) -> f64 {
        let d = self.current_deriv();
        d.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_aizawa_initial_state() {
        let sys = Aizawa::new();
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.1).abs() < 1e-15);
        assert!((s[1] - 0.0).abs() < 1e-15);
        assert!((s[2] - 0.0).abs() < 1e-15);
        assert_eq!(sys.name(), "Aizawa");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_aizawa_step_changes_state() {
        let mut sys = Aizawa::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_aizawa_deterministic() {
        let mut sys1 = Aizawa::new();
        let mut sys2 = Aizawa::new();
        for _ in 0..500 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_aizawa_dt_zero_no_change() {
        let mut sys = Aizawa::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0);
        let after = sys.state();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-15, "State changed with dt=0: {} -> {}", a, b);
        }
    }

    #[test]
    fn test_aizawa_set_state() {
        let mut sys = Aizawa::new();
        sys.set_state(&[0.5, 0.5, 0.5]);
        let s = sys.state();
        assert!((s[0] - 0.5).abs() < 1e-15);
        assert!((s[1] - 0.5).abs() < 1e-15);
        assert!((s[2] - 0.5).abs() < 1e-15);
    }
}
