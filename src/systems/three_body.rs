use super::DynamicalSystem;

/// Gravitational three-body problem in the 2D plane.
///
/// State vector (12 elements): [x1, y1, x2, y2, x3, y3, vx1, vy1, vx2, vy2, vx3, vy3].
///
/// The Newtonian equations of motion are:
///
///   d^2 r_i / dt^2 = G * sum_{j != i} m_j * (r_j - r_i) / |r_j - r_i|^3
///
/// A gravitational softening floor of 1e-3 prevents the force from diverging
/// during close encounters while preserving qualitatively correct dynamics.
///
/// Integration uses velocity Verlet (leapfrog), a second-order symplectic
/// method that conserves the Hamiltonian to O(dt^2) per step.  The figure-8
/// periodic orbit (Chenciner and Montgomery 2000) is used as the default
/// initial condition for equal masses.
pub struct ThreeBody {
    state: Vec<f64>,
    pub masses: [f64; 3],
    g: f64,
    speed: f64,
    initial_energy: f64,
    pub energy_error: f64,
}

impl ThreeBody {
    /// Figure-8 initial conditions (Chenciner & Montgomery, 2000).
    pub fn new(masses: [f64; 3]) -> Self {
        // Scaled figure-8 ICs for unit masses
        let state = vec![
            // positions
            -0.970_004_36,
            0.243_087_53,
            0.970_004_36,
            -0.243_087_53,
            0.0,
            0.0,
            // velocities
            0.932_407_37 / 2.0,
            0.864_731_46 / 2.0,
            0.932_407_37 / 2.0,
            0.864_731_46 / 2.0,
            -0.932_407_37,
            -0.864_731_46,
        ];
        let g = 1.0;
        let initial_energy = Self::compute_hamiltonian(&state, &masses, g);
        Self {
            state,
            masses,
            g,
            speed: 0.0,
            initial_energy,
            energy_error: 0.0,
        }
    }

    fn compute_hamiltonian(state: &[f64], masses: &[f64; 3], g: f64) -> f64 {
        // T = (1/2) * sum (vx_i^2 + vy_i^2) / m_i
        let mut t = 0.0f64;
        for i in 0..3 {
            let vx = state[6 + 2 * i];
            let vy = state[6 + 2 * i + 1];
            t += 0.5 * (vx * vx + vy * vy) / masses[i];
        }
        // V = -G * sum_{i<j} m_i * m_j / |r_i - r_j|
        let mut v = 0.0f64;
        for i in 0..3 {
            for j in (i + 1)..3 {
                let dx = state[2 * j] - state[2 * i];
                let dy = state[2 * j + 1] - state[2 * i + 1];
                let r = (dx * dx + dy * dy).sqrt().max(1e-10);
                v -= g * masses[i] * masses[j] / r;
            }
        }
        t + v
    }

    pub fn hamiltonian(&self) -> f64 {
        Self::compute_hamiltonian(&self.state, &self.masses, self.g)
    }

    fn accelerations(state: &[f64], masses: &[f64; 3], g: f64) -> Vec<f64> {
        let pos = |i: usize| (state[2 * i], state[2 * i + 1]);
        let mut ax = [0.0f64; 3];
        let mut ay = [0.0f64; 3];
        for i in 0..3 {
            for j in 0..3 {
                if i == j {
                    continue;
                }
                let (xi, yi) = pos(i);
                let (xj, yj) = pos(j);
                let dx = xj - xi;
                let dy = yj - yi;
                // Softening floor: 1e-3 prevents the 1/r³ term from blowing up
                // to ~1e18 when two bodies pass within 1e-6 of each other, which
                // causes the leapfrog integrator to diverge in a single step.
                let r = (dx * dx + dy * dy).sqrt().max(1e-3);
                let r3 = r * r * r;
                ax[i] += g * masses[j] * dx / r3;
                ay[i] += g * masses[j] * dy / r3;
            }
        }
        // Return as [ax1,ay1, ax2,ay2, ax3,ay3]
        vec![ax[0], ay[0], ax[1], ay[1], ax[2], ay[2]]
    }
}

impl DynamicalSystem for ThreeBody {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        12
    }
    fn name(&self) -> &str {
        "Three-Body"
    }
    fn speed(&self) -> f64 {
        self.speed
    }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        let accel = Self::accelerations(state, &self.masses, self.g);
        let mut d = Vec::with_capacity(12);
        // derivative of positions = velocities
        d.extend_from_slice(&state[6..12]);
        // derivative of velocities = accelerations
        d.extend(accel);
        d
    }

    fn energy_error(&self) -> Option<f64> {
        Some(self.energy_error)
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
        // Recompute initial energy after state reset so energy_error is meaningful
        self.initial_energy = Self::compute_hamiltonian(&self.state, &self.masses, self.g);
        self.energy_error = 0.0;
    }

    fn step(&mut self, dt: f64) {
        let prev = self.state.clone();
        let (masses, g) = (self.masses, self.g);

        // Leapfrog: positions in [0..6], velocities in [6..12]
        // Half-kick velocities
        let accel = Self::accelerations(&self.state, &masses, g);
        for i in 0..6 {
            self.state[6 + i] += 0.5 * dt * accel[i];
        }
        // Drift positions
        for i in 0..6 {
            self.state[i] += dt * self.state[6 + i];
        }
        // Half-kick velocities again
        let accel2 = Self::accelerations(&self.state, &masses, g);
        for i in 0..6 {
            self.state[6 + i] += 0.5 * dt * accel2[i];
        }

        let ds: f64 = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = ds / dt;

        // Update energy conservation error
        let h_now = self.hamiltonian();
        if self.initial_energy.abs() > 1e-15 {
            self.energy_error = ((h_now - self.initial_energy) / self.initial_energy).abs();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_three_body_initial_state() {
        let sys = ThreeBody::new([1.0, 1.0, 1.0]);
        let s = sys.state();
        assert_eq!(s.len(), 12);
        assert_eq!(sys.dimension(), 12);
        assert_eq!(sys.name(), "Three-Body");
        assert!(s.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_three_body_step_changes_state() {
        let mut sys = ThreeBody::new([1.0, 1.0, 1.0]);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_three_body_state_stays_finite() {
        let mut sys = ThreeBody::new([1.0, 1.0, 1.0]);
        for _ in 0..500 {
            sys.step(0.001);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_three_body_energy_conserved() {
        // Leapfrog should conserve energy to within a small relative error
        let mut sys = ThreeBody::new([1.0, 1.0, 1.0]);
        for _ in 0..1000 {
            sys.step(0.001);
        }
        assert!(
            sys.energy_error < 0.01,
            "Energy error too large: {}",
            sys.energy_error
        );
    }

    #[test]
    fn test_three_body_set_state_resets_energy() {
        let mut sys = ThreeBody::new([1.0, 1.0, 1.0]);
        // Run to build up some energy error
        for _ in 0..100 {
            sys.step(0.01);
        }
        // Reset state to initial
        let new_state: Vec<f64> = (0..12).map(|i| i as f64 * 0.1).collect();
        sys.set_state(&new_state);
        // energy_error should have been reset to 0
        assert_eq!(sys.energy_error, 0.0, "energy_error should reset after set_state");
    }

    #[test]
    fn test_three_body_deterministic() {
        let mut s1 = ThreeBody::new([1.0, 1.0, 1.0]);
        let mut s2 = ThreeBody::new([1.0, 1.0, 1.0]);
        for _ in 0..200 {
            s1.step(0.001);
            s2.step(0.001);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }
}
