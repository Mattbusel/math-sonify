//! Van der Pol oscillator — standalone module with config/state types.
//!
//! Provides `VanDerPolConfig`, `VanDerPolState`, and `VanDerPolOscillator`
//! with an explicit RK4 integrator, independent of the internal systems module.
//!
//! ## Equations of motion
//! ```text
//! dx/dt = y
//! dy/dt = μ·(1 − x²)·y − x
//! ```
//! For μ > 0 the system has a stable limit cycle. Larger μ gives
//! increasingly relaxation-oscillator-like behaviour.

// ── Configuration ──────────────────────────────────────────────────────────────

/// Configuration for the Van der Pol oscillator.
///
/// The nonlinearity parameter `mu = 1.0` is the classical default. At `mu = 0`
/// the system degenerates to a harmonic oscillator.
#[derive(Debug, Clone, PartialEq)]
pub struct VanDerPolConfig {
    /// Nonlinearity parameter μ. Must be > 0 for a limit cycle.
    pub mu: f64,
}

impl Default for VanDerPolConfig {
    fn default() -> Self {
        Self { mu: 1.0 }
    }
}

// ── State ─────────────────────────────────────────────────────────────────────

/// Phase-space state `(x, y)` of the Van der Pol oscillator.
#[derive(Debug, Clone, PartialEq)]
pub struct VanDerPolState {
    /// Displacement.
    pub x: f64,
    /// Velocity.
    pub y: f64,
}

impl Default for VanDerPolState {
    fn default() -> Self {
        Self { x: 2.0, y: 0.0 }
    }
}

impl VanDerPolState {
    pub fn as_array(&self) -> [f64; 2] {
        [self.x, self.y]
    }

    pub fn from_slice(s: &[f64]) -> Self {
        Self {
            x: s.get(0).copied().unwrap_or(2.0),
            y: s.get(1).copied().unwrap_or(0.0),
        }
    }
}

// ── RK4 helper ────────────────────────────────────────────────────────────────

fn rk4_step(state: [f64; 2], dt: f64, deriv: impl Fn([f64; 2]) -> [f64; 2]) -> [f64; 2] {
    let k1 = deriv(state);
    let s2 = [state[0] + dt/2.0 * k1[0], state[1] + dt/2.0 * k1[1]];
    let k2 = deriv(s2);
    let s3 = [state[0] + dt/2.0 * k2[0], state[1] + dt/2.0 * k2[1]];
    let k3 = deriv(s3);
    let s4 = [state[0] + dt * k3[0], state[1] + dt * k3[1]];
    let k4 = deriv(s4);
    [
        state[0] + dt/6.0 * (k1[0] + 2.0*k2[0] + 2.0*k3[0] + k4[0]),
        state[1] + dt/6.0 * (k1[1] + 2.0*k2[1] + 2.0*k3[1] + k4[1]),
    ]
}

// ── Oscillator ────────────────────────────────────────────────────────────────

/// Van der Pol self-sustaining limit-cycle oscillator with RK4 integration.
///
/// # Example
/// ```
/// use math_sonify::vanderpol::{VanDerPolConfig, VanDerPolOscillator};
///
/// let mut osc = VanDerPolOscillator::new(VanDerPolConfig::default());
/// osc.step(0.01);
/// let s = osc.state();
/// assert!(s.x.is_finite());
/// ```
pub struct VanDerPolOscillator {
    config: VanDerPolConfig,
    state: VanDerPolState,
    speed: f64,
}

impl VanDerPolOscillator {
    /// Create a new oscillator with default initial state `(x=2, y=0)`.
    pub fn new(config: VanDerPolConfig) -> Self {
        Self {
            config,
            state: VanDerPolState::default(),
            speed: 0.0,
        }
    }

    /// Access the current state.
    pub fn state(&self) -> &VanDerPolState {
        &self.state
    }

    /// Access the configuration.
    pub fn config(&self) -> &VanDerPolConfig {
        &self.config
    }

    /// Current trajectory speed ‖(dx/dt, dy/dt)‖.
    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Set state (ignores NaN/Inf).
    pub fn set_state(&mut self, s: VanDerPolState) {
        if s.x.is_finite() { self.state.x = s.x; }
        if s.y.is_finite() { self.state.y = s.y; }
    }

    /// Compute ODE derivatives at a given state.
    pub fn derivatives(&self, s: &VanDerPolState) -> VanDerPolState {
        let mu = self.config.mu;
        VanDerPolState {
            x: s.y,
            y: mu * (1.0 - s.x * s.x) * s.y - s.x,
        }
    }

    /// Advance the oscillator by one RK4 step of size `dt`.
    pub fn step(&mut self, dt: f64) {
        let mu = self.config.mu;
        let prev = self.state.as_array();
        let next = rk4_step(prev, dt, |s| {
            [s[1], mu * (1.0 - s[0] * s[0]) * s[1] - s[0]]
        });
        let ds = ((next[0] - prev[0]).powi(2) + (next[1] - prev[1]).powi(2)).sqrt();
        self.speed = if dt != 0.0 { ds / dt.abs() } else { 0.0 };
        self.state = VanDerPolState::from_slice(&next);
    }

    /// Return state as `Vec<f64>` for interop.
    pub fn state_vec(&self) -> Vec<f64> {
        vec![self.state.x, self.state.y]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let cfg = VanDerPolConfig::default();
        assert!((cfg.mu - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_state_default() {
        let s = VanDerPolState::default();
        assert!((s.x - 2.0).abs() < 1e-12);
        assert!((s.y - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_step_changes_state() {
        let mut osc = VanDerPolOscillator::new(VanDerPolConfig::default());
        let before = osc.state().clone();
        osc.step(0.01);
        let after = osc.state();
        assert!(
            (after.x - before.x).abs() > 1e-15 || (after.y - before.y).abs() > 1e-15,
            "State must change after a step"
        );
    }

    #[test]
    fn test_finite_output_after_many_steps() {
        let mut osc = VanDerPolOscillator::new(VanDerPolConfig::default());
        for _ in 0..1000 {
            osc.step(0.01);
        }
        let s = osc.state();
        assert!(s.x.is_finite(), "x not finite: {}", s.x);
        assert!(s.y.is_finite(), "y not finite: {}", s.y);
    }

    #[test]
    fn test_bounded_limit_cycle() {
        // Van der Pol limit cycle is bounded; x stays within ~[-3, 3], y within ~[-3, 3]
        let mut osc = VanDerPolOscillator::new(VanDerPolConfig::default());
        for _ in 0..2000 {
            osc.step(0.01);
        }
        let s = osc.state();
        assert!(s.x.abs() < 5.0, "x out of expected limit cycle: {}", s.x);
        assert!(s.y.abs() < 10.0, "y out of expected limit cycle: {}", s.y);
    }

    #[test]
    fn test_derivatives_at_initial_state() {
        let osc = VanDerPolOscillator::new(VanDerPolConfig::default());
        // At (2, 0) with mu=1: dx=0, dy=mu*(1-4)*0 - 2 = -2
        let d = osc.derivatives(&VanDerPolState::default());
        assert!((d.x - 0.0).abs() < 1e-12, "dx at (2,0): {}", d.x);
        assert!((d.y - (-2.0)).abs() < 1e-12, "dy at (2,0): {}", d.y);
    }

    #[test]
    fn test_speed_positive_after_step() {
        let mut osc = VanDerPolOscillator::new(VanDerPolConfig::default());
        osc.step(0.01);
        assert!(osc.speed() > 0.0, "speed should be positive: {}", osc.speed());
    }

    #[test]
    fn test_set_state_ignores_nan() {
        let mut osc = VanDerPolOscillator::new(VanDerPolConfig::default());
        osc.set_state(VanDerPolState { x: f64::NAN, y: 3.0 });
        // x unchanged (2.0), y updated to 3.0
        assert!((osc.state().x - 2.0).abs() < 1e-12);
        assert!((osc.state().y - 3.0).abs() < 1e-12);
    }

    #[test]
    fn test_deterministic() {
        let mut osc1 = VanDerPolOscillator::new(VanDerPolConfig::default());
        let mut osc2 = VanDerPolOscillator::new(VanDerPolConfig::default());
        for _ in 0..500 {
            osc1.step(0.01);
            osc2.step(0.01);
        }
        let s1 = osc1.state();
        let s2 = osc2.state();
        assert!((s1.x - s2.x).abs() < 1e-15);
        assert!((s1.y - s2.y).abs() < 1e-15);
    }

    #[test]
    fn test_dt_zero_no_change() {
        let mut osc = VanDerPolOscillator::new(VanDerPolConfig::default());
        let before = osc.state().clone();
        osc.step(0.0);
        let after = osc.state();
        assert!((after.x - before.x).abs() < 1e-15);
        assert!((after.y - before.y).abs() < 1e-15);
    }

    #[test]
    fn test_state_vec_length() {
        let osc = VanDerPolOscillator::new(VanDerPolConfig::default());
        assert_eq!(osc.state_vec().len(), 2);
    }

    #[test]
    fn test_state_from_slice() {
        let s = VanDerPolState::from_slice(&[1.5, -0.5]);
        assert!((s.x - 1.5).abs() < 1e-12);
        assert!((s.y - (-0.5)).abs() < 1e-12);
    }
}
