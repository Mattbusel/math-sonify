use super::{rk4, DynamicalSystem};

/// Oregonator model of the Belousov-Zhabotinsky oscillating reaction.
///
/// 3D system:
///   dx/dt = s * (y - x*y + x - q*x²)
///   dy/dt = (-y - x*y + f*z) / s
///   dz/dt = w * (x - z)
///
/// Classic parameters: s=77.27, q=8.375e-6, w=0.161, f=1.0 (configurable).
/// State is clamped to [1e-12, 1e6] after each step to prevent blowup.
pub struct Oregonator {
    state: Vec<f64>,
    pub s: f64,
    pub q: f64,
    pub w: f64,
    pub f: f64,
    speed: f64,
}

impl Oregonator {
    pub fn new(f: f64) -> Self {
        Self {
            state: vec![1.0, 2.0, 3.0],
            s: 77.27,
            q: 8.375e-6,
            w: 0.161,
            f,
            speed: 0.0,
        }
    }

    fn deriv(state: &[f64], s: f64, q: f64, w: f64, f: f64) -> Vec<f64> {
        let x = state[0];
        let y = state[1];
        let z = state[2];
        vec![
            s * (y - x * y + x - q * x * x),
            (-y - x * y + f * z) / s,
            w * (x - z),
        ]
    }
}

impl DynamicalSystem for Oregonator {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "Oregonator"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.s, self.q, self.w, self.f)
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
        let (s, q, w, f) = (self.s, self.q, self.w, self.f);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |st| Self::deriv(st, s, q, w, f));
        // Clamp to avoid blowup
        for v in &mut self.state {
            *v = v.clamp(1e-12, 1e6);
        }
        let ds: f64 = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = ds / dt.max(1e-15);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_oregonator_initial_state() {
        let sys = Oregonator::new(1.0);
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
        assert_eq!(sys.name(), "Oregonator");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_oregonator_step_changes_state() {
        let mut sys = Oregonator::new(1.0);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.0001);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_oregonator_state_stays_positive() {
        // The Oregonator models chemical concentrations which must stay positive.
        let mut sys = Oregonator::new(1.0);
        for _ in 0..1000 {
            sys.step(0.0001);
        }
        for v in sys.state().iter() {
            assert!(*v >= 0.0, "State became negative: {}", v);
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_oregonator_deterministic() {
        let mut sys1 = Oregonator::new(1.0);
        let mut sys2 = Oregonator::new(1.0);
        for _ in 0..200 {
            sys1.step(0.0001);
            sys2.step(0.0001);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_oregonator_set_state() {
        let mut sys = Oregonator::new(1.0);
        sys.set_state(&[0.5, 1.5, 2.5]);
        let s = sys.state();
        assert!((s[0] - 0.5).abs() < 1e-15, "x should be 0.5: {}", s[0]);
        assert!((s[1] - 1.5).abs() < 1e-15, "y should be 1.5: {}", s[1]);
        assert!((s[2] - 2.5).abs() < 1e-15, "z should be 2.5: {}", s[2]);
    }

    #[test]
    fn test_oregonator_speed_positive() {
        let mut sys = Oregonator::new(1.0);
        sys.step(0.0001);
        assert!(sys.speed() > 0.0, "speed should be positive: {}", sys.speed());
    }

    #[test]
    fn test_oregonator_f_param_affects_dynamics() {
        // Different f values should produce different oscillation behavior
        let mut sys1 = Oregonator::new(0.5);
        let mut sys2 = Oregonator::new(2.0);
        for _ in 0..1000 {
            sys1.step(0.0001);
            sys2.step(0.0001);
        }
        // After enough steps the two trajectories should diverge
        let d: f64 = sys1.state().iter().zip(sys2.state().iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(d > 1e-6, "Different f should produce different dynamics: diff={}", d);
    }
}
