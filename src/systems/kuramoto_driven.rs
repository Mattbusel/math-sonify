use super::{rk4, DynamicalSystem};

/// Kuramoto model with external sinusoidal drive.
///
/// N=6 oscillators with phases θᵢ:
///   dθᵢ/dt = ωᵢ + (K/N)*Σⱼ sin(θⱼ - θᵢ) + A*sin(Ω*t - θᵢ)
///
/// Parameters:
///   K (coupling): mean-field coupling strength
///   A (drive_amp): amplitude of external drive
///   Ω (drive_freq): frequency of external drive
///
/// State: [θ₀, θ₁, θ₂, θ₃, θ₄, θ₅, t_internal] — 7 elements.
/// `dimension()` returns 6 (the useful phase dimensions).
pub struct KuramotoDriven {
    /// [theta_0..theta_5, t_internal]
    state: Vec<f64>,
    omega: Vec<f64>,
    pub coupling: f64,
    pub drive_amp: f64,
    pub drive_freq: f64,
    speed: f64,
}

const N: usize = 6;

impl KuramotoDriven {
    pub fn new(coupling: f64, drive_amp: f64, drive_freq: f64) -> Self {
        // Lorentzian natural frequencies (same spacing as kuramoto.rs)
        let omega: Vec<f64> = (0..N)
            .map(|i| {
                let u = (i as f64 + 0.5) / N as f64;
                let u_safe = u.clamp(1e-6, 1.0 - 1e-6);
                1.0 + 0.5 * (std::f64::consts::PI * (u_safe - 0.5)).tan()
            })
            .collect();
        // Uniform initial phases, then append t=0
        let mut state: Vec<f64> = (0..N)
            .map(|i| 2.0 * std::f64::consts::PI * i as f64 / N as f64)
            .collect();
        state.push(0.0); // t_internal
        Self {
            state,
            omega,
            coupling,
            drive_amp,
            drive_freq,
            speed: 0.0,
        }
    }

    fn compute_deriv(state: &[f64], omega: &[f64], coupling: f64, drive_amp: f64, drive_freq: f64) -> Vec<f64> {
        let t = state[N]; // t_internal
        let k_over_n = coupling / N as f64;
        let mut deriv: Vec<f64> = (0..N)
            .map(|i| {
                let th_i = state[i];
                let coupling_sum: f64 = (0..N).map(|j| (state[j] - th_i).sin()).sum();
                let drive = drive_amp * (drive_freq * t - th_i).sin();
                omega[i] + k_over_n * coupling_sum + drive
            })
            .collect();
        deriv.push(1.0); // dt_internal/dt = 1
        deriv
    }
}

impl DynamicalSystem for KuramotoDriven {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        N
    }

    fn name(&self) -> &str {
        "Kuramoto Driven"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::compute_deriv(state, &self.omega, self.coupling, self.drive_amp, self.drive_freq)
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
        let omega = self.omega.clone();
        let (coupling, drive_amp, drive_freq) = (self.coupling, self.drive_amp, self.drive_freq);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| {
            Self::compute_deriv(s, &omega, coupling, drive_amp, drive_freq)
        });
        // Wrap only the phase components (not t_internal)
        for i in 0..N {
            self.state[i] = self.state[i].rem_euclid(std::f64::consts::TAU);
        }
        let ds: f64 = self.state[0..N]
            .iter()
            .zip(prev[0..N].iter())
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
    fn test_kuramoto_driven_initial_state() {
        let sys = KuramotoDriven::new(1.0, 0.5, 1.0);
        let s = sys.state();
        assert_eq!(s.len(), 7); // 6 phases + t_internal
        assert_eq!(sys.dimension(), 6);
        assert_eq!(sys.name(), "Kuramoto Driven");
        assert!(s.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_kuramoto_driven_step_changes_state() {
        let mut sys = KuramotoDriven::new(1.0, 0.5, 1.0);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_kuramoto_driven_phases_wrapped() {
        let mut sys = KuramotoDriven::new(1.0, 0.5, 1.0);
        for _ in 0..1000 {
            sys.step(0.01);
        }
        for &th in sys.state()[0..N].iter() {
            assert!(th >= 0.0 && th < std::f64::consts::TAU, "Phase unwrapped: {}", th);
        }
    }

    #[test]
    fn test_kuramoto_driven_t_internal_advances() {
        let mut sys = KuramotoDriven::new(1.0, 0.5, 1.0);
        let t_before = sys.state()[N];
        for _ in 0..10 {
            sys.step(0.01);
        }
        let t_after = sys.state()[N];
        assert!(t_after > t_before, "t_internal should advance: {} -> {}", t_before, t_after);
    }

    #[test]
    fn test_kuramoto_driven_deterministic() {
        let mut sys1 = KuramotoDriven::new(1.0, 0.5, 1.0);
        let mut sys2 = KuramotoDriven::new(1.0, 0.5, 1.0);
        for _ in 0..500 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_kuramoto_driven_state_finite_after_many_steps() {
        let mut sys = KuramotoDriven::new(2.0, 1.0, 1.5);
        for _ in 0..2000 {
            sys.step(0.01);
        }
        assert!(
            sys.state().iter().all(|v| v.is_finite()),
            "State has non-finite value after many steps: {:?}", sys.state()
        );
    }

    #[test]
    fn test_kuramoto_driven_speed_positive() {
        let mut sys = KuramotoDriven::new(1.0, 0.5, 1.0);
        sys.step(0.01);
        assert!(sys.speed() > 0.0, "speed should be positive: {}", sys.speed());
    }
}
