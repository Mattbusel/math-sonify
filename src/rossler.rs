//! Rössler Attractor — standalone module with config/state types.
//!
//! This module re-exports the Rössler attractor under idiomatic config/state
//! structs and provides a self-contained `RosslerAttractor` that integrates
//! the system with RK4.
//!
//! ## Classic parameters
//! - `a = 0.2`, `b = 0.2`, `c = 5.7` → exhibits near-periodic chaotic orbit
//!
//! ## Equations of motion
//! ```text
//! dx/dt = -y - z
//! dy/dt =  x + a·y
//! dz/dt =  b + z·(x - c)
//! ```

// ── Configuration ──────────────────────────────────────────────────────────────

/// Configuration parameters for the Rössler attractor.
///
/// Classic chaotic parameters: `a=0.2, b=0.2, c=5.7`.
#[derive(Debug, Clone, PartialEq)]
pub struct RosslerConfig {
    /// Controls the y-feedback. Chaos onset near a ≈ 0.398.
    pub a: f64,
    /// Additive constant in the z-equation. Typically small (e.g. 0.2).
    pub b: f64,
    /// Shifts the z-nullcline. Chaos is robust for c ≈ 5.7.
    pub c: f64,
}

impl Default for RosslerConfig {
    /// Returns the classic chaotic Rössler parameters.
    fn default() -> Self {
        Self { a: 0.2, b: 0.2, c: 5.7 }
    }
}

// ── State ─────────────────────────────────────────────────────────────────────

/// The current phase-space state of the Rössler attractor.
#[derive(Debug, Clone, PartialEq)]
pub struct RosslerState {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for RosslerState {
    fn default() -> Self {
        Self { x: 1.0, y: 0.0, z: 0.0 }
    }
}

impl RosslerState {
    /// Returns `[x, y, z]` as a slice-compatible array.
    pub fn as_array(&self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    /// Construct from a slice (must have length ≥ 3).
    pub fn from_slice(s: &[f64]) -> Self {
        Self {
            x: s.get(0).copied().unwrap_or(1.0),
            y: s.get(1).copied().unwrap_or(0.0),
            z: s.get(2).copied().unwrap_or(0.0),
        }
    }
}

// ── RK4 helper ────────────────────────────────────────────────────────────────

fn rk4_step(state: [f64; 3], dt: f64, deriv: impl Fn([f64; 3]) -> [f64; 3]) -> [f64; 3] {
    let k1 = deriv(state);
    let s2 = [state[0] + dt/2.0 * k1[0], state[1] + dt/2.0 * k1[1], state[2] + dt/2.0 * k1[2]];
    let k2 = deriv(s2);
    let s3 = [state[0] + dt/2.0 * k2[0], state[1] + dt/2.0 * k2[1], state[2] + dt/2.0 * k2[2]];
    let k3 = deriv(s3);
    let s4 = [state[0] + dt * k3[0], state[1] + dt * k3[1], state[2] + dt * k3[2]];
    let k4 = deriv(s4);
    [
        state[0] + dt/6.0 * (k1[0] + 2.0*k2[0] + 2.0*k3[0] + k4[0]),
        state[1] + dt/6.0 * (k1[1] + 2.0*k2[1] + 2.0*k3[1] + k4[1]),
        state[2] + dt/6.0 * (k1[2] + 2.0*k2[2] + 2.0*k3[2] + k4[2]),
    ]
}

// ── Attractor ─────────────────────────────────────────────────────────────────

/// Rössler attractor integrator.
///
/// Integrates the Rössler ODE using RK4 at a configurable timestep.
///
/// # Example
/// ```
/// use math_sonify::rossler::{RosslerAttractor, RosslerConfig};
///
/// let mut attractor = RosslerAttractor::new(RosslerConfig::default());
/// attractor.step(0.01);
/// let s = attractor.state();
/// assert!(s.x.is_finite());
/// ```
pub struct RosslerAttractor {
    config: RosslerConfig,
    state: RosslerState,
    /// Trajectory speed: ‖Δstate‖ / dt at the last step.
    speed: f64,
}

impl RosslerAttractor {
    /// Create a new attractor with the given config and default initial state `(1, 0, 0)`.
    pub fn new(config: RosslerConfig) -> Self {
        Self {
            config,
            state: RosslerState::default(),
            speed: 0.0,
        }
    }

    /// Access the current state.
    pub fn state(&self) -> &RosslerState {
        &self.state
    }

    /// Access the configuration.
    pub fn config(&self) -> &RosslerConfig {
        &self.config
    }

    /// Current trajectory speed (‖dx/dt‖ at the previous step).
    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Set a new state directly (ignores NaN/Inf components).
    pub fn set_state(&mut self, s: RosslerState) {
        if s.x.is_finite() { self.state.x = s.x; }
        if s.y.is_finite() { self.state.y = s.y; }
        if s.z.is_finite() { self.state.z = s.z; }
    }

    /// Compute the ODE derivatives at state `s` with the current config.
    pub fn derivatives(&self, s: &RosslerState) -> RosslerState {
        let (a, b, c) = (self.config.a, self.config.b, self.config.c);
        RosslerState {
            x: -s.y - s.z,
            y: s.x + a * s.y,
            z: b + s.z * (s.x - c),
        }
    }

    /// Advance the attractor by one RK4 step of size `dt`.
    pub fn step(&mut self, dt: f64) {
        let (a, b, c) = (self.config.a, self.config.b, self.config.c);
        let prev = self.state.as_array();
        let next = rk4_step(prev, dt, |s| {
            [-s[1] - s[2], s[0] + a * s[1], b + s[2] * (s[0] - c)]
        });
        let ds = (0..3)
            .map(|i| (next[i] - prev[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = if dt != 0.0 { ds / dt.abs() } else { 0.0 };
        self.state = RosslerState::from_slice(&next);
    }

    /// Return the state as a `Vec<f64>` for interop with the systems module.
    pub fn state_vec(&self) -> Vec<f64> {
        vec![self.state.x, self.state.y, self.state.z]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_classic_params() {
        let cfg = RosslerConfig::default();
        assert!((cfg.a - 0.2).abs() < 1e-12);
        assert!((cfg.b - 0.2).abs() < 1e-12);
        assert!((cfg.c - 5.7).abs() < 1e-12);
    }

    #[test]
    fn test_state_default() {
        let s = RosslerState::default();
        assert!((s.x - 1.0).abs() < 1e-12);
        assert!((s.y - 0.0).abs() < 1e-12);
        assert!((s.z - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_step_changes_state() {
        let mut a = RosslerAttractor::new(RosslerConfig::default());
        let before = a.state().clone();
        a.step(0.01);
        let after = a.state();
        assert!(
            (after.x - before.x).abs() > 1e-15
                || (after.y - before.y).abs() > 1e-15
                || (after.z - before.z).abs() > 1e-15,
            "State must change after a step"
        );
    }

    #[test]
    fn test_finite_output_after_many_steps() {
        let mut a = RosslerAttractor::new(RosslerConfig::default());
        for _ in 0..1000 {
            a.step(0.01);
        }
        let s = a.state();
        assert!(s.x.is_finite(), "x not finite: {}", s.x);
        assert!(s.y.is_finite(), "y not finite: {}", s.y);
        assert!(s.z.is_finite(), "z not finite: {}", s.z);
    }

    #[test]
    fn test_bounded_orbit_classic_params() {
        let mut a = RosslerAttractor::new(RosslerConfig::default());
        for _ in 0..2000 {
            a.step(0.01);
        }
        let s = a.state();
        // Classic Rössler stays well within ±30 in x and y
        assert!(s.x.abs() < 30.0, "x out of bounds: {}", s.x);
        assert!(s.y.abs() < 30.0, "y out of bounds: {}", s.y);
        assert!(s.z >= 0.0 && s.z < 40.0, "z out of bounds: {}", s.z);
    }

    #[test]
    fn test_derivatives_at_initial_state() {
        let a = RosslerAttractor::new(RosslerConfig::default());
        let d = a.derivatives(&RosslerState::default());
        // At (1, 0, 0): dx = 0, dy = 1, dz = 0.2
        assert!((d.x - 0.0).abs() < 1e-12, "dx: {}", d.x);
        assert!((d.y - 1.0).abs() < 1e-12, "dy: {}", d.y);
        assert!((d.z - 0.2).abs() < 1e-12, "dz: {}", d.z);
    }

    #[test]
    fn test_speed_positive_after_step() {
        let mut a = RosslerAttractor::new(RosslerConfig::default());
        a.step(0.01);
        assert!(a.speed() > 0.0, "speed should be positive: {}", a.speed());
    }

    #[test]
    fn test_set_state_ignores_nan() {
        let mut a = RosslerAttractor::new(RosslerConfig::default());
        a.set_state(RosslerState { x: f64::NAN, y: 5.0, z: f64::NAN });
        // x and z should be unchanged (1.0 and 0.0)
        assert!((a.state().x - 1.0).abs() < 1e-12);
        assert!((a.state().y - 5.0).abs() < 1e-12);
        assert!((a.state().z - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_deterministic() {
        let mut a1 = RosslerAttractor::new(RosslerConfig::default());
        let mut a2 = RosslerAttractor::new(RosslerConfig::default());
        for _ in 0..500 {
            a1.step(0.01);
            a2.step(0.01);
        }
        let s1 = a1.state();
        let s2 = a2.state();
        assert!((s1.x - s2.x).abs() < 1e-15);
        assert!((s1.y - s2.y).abs() < 1e-15);
        assert!((s1.z - s2.z).abs() < 1e-15);
    }

    #[test]
    fn test_state_vec_length() {
        let a = RosslerAttractor::new(RosslerConfig::default());
        assert_eq!(a.state_vec().len(), 3);
    }

    #[test]
    fn test_state_from_slice() {
        let s = RosslerState::from_slice(&[2.0, 3.0, 4.0]);
        assert!((s.x - 2.0).abs() < 1e-12);
        assert!((s.y - 3.0).abs() < 1e-12);
        assert!((s.z - 4.0).abs() < 1e-12);
    }

    #[test]
    fn test_as_array_roundtrip() {
        let s = RosslerState { x: 1.5, y: 2.5, z: 3.5 };
        let arr = s.as_array();
        assert!((arr[0] - 1.5).abs() < 1e-12);
        assert!((arr[1] - 2.5).abs() < 1e-12);
        assert!((arr[2] - 3.5).abs() < 1e-12);
    }
}
