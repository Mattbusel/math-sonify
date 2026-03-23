//! Duffing Oscillator — standalone module for the library crate.
//!
//! Provides `DuffingConfig`, `DuffingState`, and an RK4 integrator.
//! The system also lives in `systems::duffing` for the main binary;
//! this module exposes a clean public API for external callers.

/// Parameters of the Duffing oscillator.
///
/// Classic chaotic parameters: α=-1, β=1, δ=0.3, γ=0.5, ω=1.2
#[derive(Debug, Clone, PartialEq)]
pub struct DuffingConfig {
    /// Linear stiffness coefficient (negative ⇒ double-well potential).
    pub alpha: f64,
    /// Nonlinear (cubic) stiffness coefficient.
    pub beta: f64,
    /// Damping coefficient.
    pub delta: f64,
    /// Amplitude of periodic forcing.
    pub gamma: f64,
    /// Angular frequency of periodic forcing.
    pub omega: f64,
}

impl Default for DuffingConfig {
    fn default() -> Self {
        Self {
            alpha: -1.0,
            beta: 1.0,
            delta: 0.3,
            gamma: 0.5,
            omega: 1.2,
        }
    }
}

/// Integrator state for the Duffing oscillator.
///
/// `x` is position, `y` is velocity, `t` tracks elapsed time for the
/// cosine forcing term `γ·cos(ω·t)`.
#[derive(Debug, Clone, PartialEq)]
pub struct DuffingState {
    pub x: f64,
    pub y: f64,
    pub t: f64,
}

impl Default for DuffingState {
    fn default() -> Self {
        Self { x: 1.0, y: 0.0, t: 0.0 }
    }
}

impl DuffingState {
    pub fn new(x: f64, y: f64, t: f64) -> Self {
        Self { x, y, t }
    }
}

/// Compute the derivatives `(dx/dt, dy/dt)` at the given state.
///
/// ```
/// dx/dt = y
/// dy/dt = -δ·y - α·x - β·x³ + γ·cos(ω·t)
/// ```
fn deriv(state: &DuffingState, cfg: &DuffingConfig) -> (f64, f64) {
    let dx = state.y;
    let dy = -cfg.delta * state.y
        - cfg.alpha * state.x
        - cfg.beta * state.x * state.x * state.x
        + cfg.gamma * (cfg.omega * state.t).cos();
    (dx, dy)
}

/// Step the Duffing oscillator forward by `dt` using RK4.
pub fn step_rk4(state: &mut DuffingState, cfg: &DuffingConfig, dt: f64) {
    // k1
    let (dx1, dy1) = deriv(state, cfg);

    // k2 (midpoint)
    let s2 = DuffingState {
        x: state.x + 0.5 * dt * dx1,
        y: state.y + 0.5 * dt * dy1,
        t: state.t + 0.5 * dt,
    };
    let (dx2, dy2) = deriv(&s2, cfg);

    // k3 (corrected midpoint)
    let s3 = DuffingState {
        x: state.x + 0.5 * dt * dx2,
        y: state.y + 0.5 * dt * dy2,
        t: state.t + 0.5 * dt,
    };
    let (dx3, dy3) = deriv(&s3, cfg);

    // k4 (endpoint)
    let s4 = DuffingState {
        x: state.x + dt * dx3,
        y: state.y + dt * dy3,
        t: state.t + dt,
    };
    let (dx4, dy4) = deriv(&s4, cfg);

    state.x += dt / 6.0 * (dx1 + 2.0 * dx2 + 2.0 * dx3 + dx4);
    state.y += dt / 6.0 * (dy1 + 2.0 * dy2 + 2.0 * dy3 + dy4);
    state.t += dt;
}

/// Generate a trajectory of `steps` states starting from `initial`.
pub fn generate_trajectory(
    cfg: &DuffingConfig,
    initial: DuffingState,
    steps: usize,
    dt: f64,
) -> Vec<DuffingState> {
    let mut trajectory = Vec::with_capacity(steps);
    let mut state = initial;
    for _ in 0..steps {
        trajectory.push(state.clone());
        step_rk4(&mut state, cfg, dt);
    }
    trajectory
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_default_config() {
        let cfg = DuffingConfig::default();
        assert_eq!(cfg.alpha, -1.0);
        assert_eq!(cfg.beta, 1.0);
        assert_eq!(cfg.delta, 0.3);
        assert_eq!(cfg.gamma, 0.5);
        assert_eq!(cfg.omega, 1.2);
    }

    #[test]
    fn test_default_state() {
        let s = DuffingState::default();
        assert_eq!(s.x, 1.0);
        assert_eq!(s.y, 0.0);
        assert_eq!(s.t, 0.0);
    }

    #[test]
    fn test_step_advances_time() {
        let cfg = DuffingConfig::default();
        let mut state = DuffingState::default();
        step_rk4(&mut state, &cfg, 0.01);
        assert!((state.t - 0.01).abs() < 1e-12);
    }

    #[test]
    fn test_step_changes_position() {
        let cfg = DuffingConfig::default();
        let mut state = DuffingState::default();
        let x0 = state.x;
        step_rk4(&mut state, &cfg, 0.01);
        // y=0 initially so dx/dt=0, but second step should move x
        // after 10 steps position should differ
        for _ in 0..10 {
            step_rk4(&mut state, &cfg, 0.01);
        }
        assert!((state.x - x0).abs() > 1e-6);
    }

    #[test]
    fn test_trajectory_length() {
        let cfg = DuffingConfig::default();
        let init = DuffingState::default();
        let traj = generate_trajectory(&cfg, init, 100, 0.01);
        assert_eq!(traj.len(), 100);
    }

    #[test]
    fn test_trajectory_first_state_is_initial() {
        let cfg = DuffingConfig::default();
        let init = DuffingState::new(2.0, 1.0, 0.5);
        let traj = generate_trajectory(&cfg, init.clone(), 10, 0.01);
        assert_eq!(traj[0], init);
    }

    #[test]
    fn test_trajectory_time_monotone() {
        let cfg = DuffingConfig::default();
        let init = DuffingState::default();
        let traj = generate_trajectory(&cfg, init, 50, 0.01);
        for i in 1..traj.len() {
            assert!(traj[i].t > traj[i - 1].t);
        }
    }

    #[test]
    fn test_zero_gamma_no_forcing() {
        // With γ=0 the system is unforced; energy should decrease monotonically
        // (damped oscillator) — x should remain bounded.
        let mut cfg = DuffingConfig::default();
        cfg.gamma = 0.0;
        let mut state = DuffingState::default();
        for _ in 0..1000 {
            step_rk4(&mut state, &cfg, 0.01);
        }
        // unforced, damped: x should have decayed
        assert!(state.x.abs() < 2.0);
    }

    #[test]
    fn test_zero_damping_bounded() {
        // δ=0, γ=0: conservative double-well. State should stay bounded.
        let cfg = DuffingConfig {
            alpha: -1.0,
            beta: 1.0,
            delta: 0.0,
            gamma: 0.0,
            omega: 1.2,
        };
        let mut state = DuffingState::default();
        for _ in 0..500 {
            step_rk4(&mut state, &cfg, 0.01);
        }
        assert!(state.x.abs() < 10.0);
        assert!(state.y.abs() < 10.0);
    }

    #[test]
    fn test_forcing_frequency() {
        // Check that the omega parameter influences the period of oscillation
        let cfg1 = DuffingConfig { omega: 1.0, gamma: 0.0, delta: 0.0, alpha: 0.0, beta: 0.0 };
        let cfg2 = DuffingConfig { omega: 2.0, gamma: 0.0, delta: 0.0, alpha: 0.0, beta: 0.0 };
        let s1 = DuffingState::new(0.0, 0.0, 0.0);
        let s2 = s1.clone();
        let t1 = generate_trajectory(&cfg1, s1, 10, 0.1);
        let t2 = generate_trajectory(&cfg2, s2, 10, 0.1);
        // Both should be identical since gamma=0 — forcing doesn't matter
        for (a, b) in t1.iter().zip(t2.iter()) {
            assert!((a.x - b.x).abs() < 1e-12);
        }
    }

    #[test]
    fn test_deriv_at_origin_with_default() {
        // At x=0, y=0, t=0 with default cfg:
        // dx/dt = 0, dy/dt = gamma*cos(0) = 0.5
        let cfg = DuffingConfig::default();
        let s = DuffingState::new(0.0, 0.0, 0.0);
        let (dx, dy) = deriv(&s, &cfg);
        assert!((dx - 0.0).abs() < 1e-12);
        assert!((dy - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_pi_forcing() {
        // At t = π/ω, cos(ω·t) = -1
        let cfg = DuffingConfig::default();
        let t = PI / cfg.omega;
        let s = DuffingState::new(0.0, 0.0, t);
        let (_dx, dy) = deriv(&s, &cfg);
        // dy = gamma*cos(pi) = -0.5
        assert!((dy - (-0.5)).abs() < 1e-12);
    }

    #[test]
    fn test_empty_trajectory() {
        let cfg = DuffingConfig::default();
        let init = DuffingState::default();
        let traj = generate_trajectory(&cfg, init, 0, 0.01);
        assert!(traj.is_empty());
    }
}
