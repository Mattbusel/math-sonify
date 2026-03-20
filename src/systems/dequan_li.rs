use super::{rk4, DynamicalSystem};

/// Dequan Li attractor — six-parameter chaotic system with complex folded structure.
///
/// Equations:
/// ```text
/// dx/dt = a·(y − x) + d·x·z
/// dy/dt = k·x + f·y − x·z
/// dz/dt = c·z + x·y − e·x²
/// ```
///
/// Parameters (defaults are the classical chaotic values):
/// - `a`: 40.0 — cross-coupling
/// - `c`: −11/6 ≈ −1.8333 — z damping
/// - `d`: 0.16 — xz coupling to dx
/// - `k`: 55.0 — x term in dy
/// - `f`: 20.0 — y damping in dy
/// - `e`: 4.0 — x² term in dz
///
/// First described by Dequan Li (2008); exhibits a complex multi-lobe attractor
/// with significant parameter sensitivity.
pub struct DequanLi {
    state: Vec<f64>,
    speed: f64,
    pub a: f64,
    pub c: f64,
    pub d: f64,
    pub k: f64,
    pub f: f64,
    pub e: f64,
}

impl DequanLi {
    /// Create a Dequan Li attractor with default parameters.
    pub fn new() -> Self {
        Self {
            state: vec![0.349, 0.0, -0.16],
            speed: 0.0,
            a: 40.0,
            c: -11.0 / 6.0,
            d: 0.16,
            k: 55.0,
            f: 20.0,
            e: 4.0,
        }
    }

    fn deriv(s: &[f64], a: f64, c: f64, d: f64, k: f64, f: f64, e: f64) -> Vec<f64> {
        vec![
            a * (s[1] - s[0]) + d * s[0] * s[2],
            k * s[0] + f * s[1] - s[0] * s[2],
            c * s[2] + s[0] * s[1] - e * s[0] * s[0],
        ]
    }
}

impl Default for DequanLi {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for DequanLi {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "dequan_li"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.c, self.d, self.k, self.f, self.e)
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
        let (a, c, d, k, f, e) = (self.a, self.c, self.d, self.k, self.f, self.e);
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, c, d, k, f, e));
        // Guard: revert to previous state if RK4 produced non-finite values
        if !self.state.iter().all(|v| v.is_finite()) {
            self.state = prev.clone();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_dequan_li_initial_state() {
        let sys = DequanLi::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "dequan_li");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_dequan_li_step_changes_state() {
        let mut sys = DequanLi::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0001);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-20));
    }

    #[test]
    fn test_dequan_li_state_stays_finite() {
        // Dequan Li has large derivatives; use dt=0.0001 to keep it stable.
        let mut sys = DequanLi::new();
        for _ in 0..5000 {
            sys.step(0.0001);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_dequan_li_deterministic() {
        let mut s1 = DequanLi::new();
        let mut s2 = DequanLi::new();
        for _ in 0..200 {
            s1.step(0.0001);
            s2.step(0.0001);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_dequan_li_deriv_at_known_point() {
        // At (1, 0, 0):
        // dx = 40*(0-1) + 0.16*1*0 = -40
        // dy = 55*1 + 20*0 - 1*0 = 55
        // dz = (-11/6)*0 + 1*0 - 4*1 = -4
        let sys = DequanLi::new();
        let d = sys.deriv_at(&[1.0, 0.0, 0.0]);
        assert!((d[0] - (-40.0)).abs() < 1e-10, "d[0]={}", d[0]);
        assert!((d[1] - 55.0).abs() < 1e-10, "d[1]={}", d[1]);
        assert!((d[2] - (-4.0)).abs() < 1e-10, "d[2]={}", d[2]);
    }

    #[test]
    fn test_dequan_li_speed_positive_after_step() {
        let mut sys = DequanLi::new();
        sys.step(0.0001);
        assert!(sys.speed() > 0.0);
    }
}
