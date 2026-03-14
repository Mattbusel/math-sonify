pub mod lorenz;
pub mod rossler;
pub mod double_pendulum;
pub mod geodesic_torus;
pub mod kuramoto;
pub mod three_body;
pub mod duffing;
pub mod van_der_pol;
pub mod halvorsen;
pub mod aizawa;
pub mod chua;

pub use lorenz::Lorenz;
pub use rossler::Rossler;
pub use double_pendulum::DoublePendulum;
pub use geodesic_torus::GeodesicTorus;
pub use kuramoto::Kuramoto;
pub use three_body::ThreeBody;
pub use duffing::Duffing;
pub use van_der_pol::VanDerPol;
pub use halvorsen::Halvorsen;
pub use aizawa::Aizawa;
pub use chua::Chua;

/// A continuous-time dynamical system that can be stepped forward.
pub trait DynamicalSystem: Send {
    fn state(&self) -> &[f64];
    fn step(&mut self, dt: f64);
    fn dimension(&self) -> usize;
    fn name(&self) -> &str;
    /// Approximate speed of the trajectory (|dx/dt|) — used by granular mode.
    fn speed(&self) -> f64 {
        // Default: Euclidean norm of derivative estimate from last step.
        // Systems can override with a direct formula.
        1.0
    }

    /// Compute the derivative (dx/dt) at an arbitrary state point.
    /// Used for vector field visualization.
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        // Default: zero vector
        vec![0.0; state.len()]
    }

    /// Return the current derivative (at self.state())
    fn current_deriv(&self) -> Vec<f64> {
        self.deriv_at(self.state())
    }
}

/// Runge-Kutta 4 helper. Integrates `f(state) -> derivative` by dt.
pub fn rk4<F>(state: &mut Vec<f64>, dt: f64, f: F)
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n = state.len();
    let k1 = f(state);
    let s2: Vec<f64> = (0..n).map(|i| state[i] + 0.5 * dt * k1[i]).collect();
    let k2 = f(&s2);
    let s3: Vec<f64> = (0..n).map(|i| state[i] + 0.5 * dt * k2[i]).collect();
    let k3 = f(&s3);
    let s4: Vec<f64> = (0..n).map(|i| state[i] + dt * k3[i]).collect();
    let k4 = f(&s4);
    for i in 0..n {
        state[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
    }
}

/// Symplectic (leapfrog / Störmer-Verlet) helper for Hamiltonian systems.
/// `q_indices` and `p_indices` are the indices of positions and momenta in state.
/// `dH_dq` computes -∂H/∂q (force), `dH_dp` computes ∂H/∂p (velocity).
pub fn leapfrog<Fv, Fa>(
    state: &mut Vec<f64>,
    q_idx: &[usize],
    p_idx: &[usize],
    dt: f64,
    velocity: Fv,
    force: Fa,
) where
    Fv: Fn(&[f64]) -> Vec<f64>,
    Fa: Fn(&[f64]) -> Vec<f64>,
{
    let n = q_idx.len();
    // half-kick momenta
    let f = force(state);
    for i in 0..n {
        state[p_idx[i]] += 0.5 * dt * f[i];
    }
    // full drift positions
    let v = velocity(state);
    for i in 0..n {
        state[q_idx[i]] += dt * v[i];
    }
    // half-kick momenta again
    let f2 = force(state);
    for i in 0..n {
        state[p_idx[i]] += 0.5 * dt * f2[i];
    }
}
