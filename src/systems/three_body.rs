use super::DynamicalSystem;

/// Gravitational three-body problem (2D planar).
/// State: [x1,y1, x2,y2, x3,y3, vx1,vy1, vx2,vy2, vx3,vy3]
/// Uses symplectic leapfrog integration (energy-preserving).
pub struct ThreeBody {
    state: Vec<f64>,
    pub masses: [f64; 3],
    g: f64,
    speed: f64,
}

impl ThreeBody {
    /// Figure-8 initial conditions (Chenciner & Montgomery, 2000).
    pub fn new(masses: [f64; 3]) -> Self {
        // Scaled figure-8 ICs for unit masses
        Self {
            state: vec![
                // positions
                -0.97000436,  0.24308753,
                 0.97000436, -0.24308753,
                 0.0,         0.0,
                // velocities
                 0.93240737 / 2.0,  0.86473146 / 2.0,
                 0.93240737 / 2.0,  0.86473146 / 2.0,
                -0.93240737,       -0.86473146,
            ],
            masses,
            g: 1.0,
            speed: 0.0,
        }
    }

    fn accelerations(state: &[f64], masses: &[f64; 3], g: f64) -> Vec<f64> {
        let pos = |i: usize| (state[2 * i], state[2 * i + 1]);
        let mut ax = [0.0f64; 3];
        let mut ay = [0.0f64; 3];
        for i in 0..3 {
            for j in 0..3 {
                if i == j { continue; }
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
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 12 }
    fn name(&self) -> &str { "Three-Body" }
    fn speed(&self) -> f64 { self.speed }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        let accel = Self::accelerations(state, &self.masses, self.g);
        let mut d = Vec::with_capacity(12);
        // derivative of positions = velocities
        d.extend_from_slice(&state[6..12]);
        // derivative of velocities = accelerations
        d.extend(accel);
        d
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

        let ds: f64 = self.state.iter().zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
        self.speed = ds / dt;
    }
}
