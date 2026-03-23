//! Hindmarsh-Rose neuron model with config/state types and RK4 integrator.
//!
//! This top-level module provides a clean public API for the Hindmarsh-Rose
//! bursting neuron model. The core numerical implementation lives in
//! `systems::hindmarsh_rose`; this module adds configuration structs, a
//! typed state struct, and a spike-detection helper so callers need not
//! interact with the generic `DynamicalSystem` trait.
//!
//! # Equations
//!
//! ```text
//! dx/dt = y - a*x^3 + b*x^2 - z + I_ext
//! dy/dt = c - d*x^2 - y
//! dz/dt = r * (s*(x - x_rest) - z)
//! ```
//!
//! # Classic (chaotic) parameters
//!
//! `a=1, b=3, c=1, d=5, r=0.001, s=4, x_rest=-1.6, i_ext=1.5`
//!
//! # Example
//!
//! ```rust
//! use math_sonify_plugin::hindmarsh_rose::{HindmarshRoseConfig, HindmarshRoseNeuron};
//!
//! let cfg = HindmarshRoseConfig::default();
//! let mut neuron = HindmarshRoseNeuron::new(cfg);
//! for _ in 0..1000 {
//!     neuron.step(0.01);
//! }
//! println!("spiking: {}", neuron.is_spiking());
//! ```

// ── Config ────────────────────────────────────────────────────────────────────

/// Parameters for the Hindmarsh-Rose neuron model.
#[derive(Debug, Clone, PartialEq)]
pub struct HindmarshRoseConfig {
    /// Fast-current parameter (default 1.0).
    pub a: f64,
    /// Fast-current parameter (default 3.0).
    pub b: f64,
    /// Recovery current parameter (default 1.0).
    pub c: f64,
    /// Recovery current parameter (default 5.0).
    pub d: f64,
    /// Slow adaptation time-scale (default 0.001).
    pub r: f64,
    /// Adaptation sensitivity (default 4.0).
    pub s: f64,
    /// Resting potential of the slow variable (default -1.6).
    pub x_rest: f64,
    /// External drive current (default 1.5 — chaotic bursting regime).
    pub i_ext: f64,
}

impl Default for HindmarshRoseConfig {
    fn default() -> Self {
        Self {
            a: 1.0,
            b: 3.0,
            c: 1.0,
            d: 5.0,
            r: 0.001,
            s: 4.0,
            x_rest: -1.6,
            i_ext: 1.5,
        }
    }
}

// ── State ─────────────────────────────────────────────────────────────────────

/// State of the Hindmarsh-Rose neuron.
///
/// * `x` — membrane potential (voltage-like variable)
/// * `y` — spiking variable (fast recovery)
/// * `z` — bursting variable (slow adaptation)
#[derive(Debug, Clone, PartialEq)]
pub struct HindmarshRoseState {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for HindmarshRoseState {
    fn default() -> Self {
        // Standard initial conditions from literature
        Self { x: 0.0, y: -8.0, z: -1.6 }
    }
}

// ── Neuron ────────────────────────────────────────────────────────────────────

/// Hindmarsh-Rose bursting neuron with RK4 integrator.
pub struct HindmarshRoseNeuron {
    config: HindmarshRoseConfig,
    state: HindmarshRoseState,
    /// Whether the neuron crossed the spike threshold on the most recent step.
    spiking: bool,
    /// Previous x value, used for threshold crossing detection.
    prev_x: f64,
}

impl HindmarshRoseNeuron {
    /// Threshold for spike detection (x above this = spiking).
    pub const SPIKE_THRESHOLD: f64 = 0.0;

    /// Create a new neuron with the given configuration and default initial state.
    pub fn new(config: HindmarshRoseConfig) -> Self {
        let state = HindmarshRoseState::default();
        let prev_x = state.x;
        Self { config, state, spiking: false, prev_x }
    }

    /// Create with explicit initial state.
    pub fn with_state(config: HindmarshRoseConfig, state: HindmarshRoseState) -> Self {
        let prev_x = state.x;
        Self { config, state, spiking: false, prev_x }
    }

    /// Read-only reference to current state.
    pub fn state(&self) -> &HindmarshRoseState {
        &self.state
    }

    /// Read-only reference to the configuration.
    pub fn config(&self) -> &HindmarshRoseConfig {
        &self.config
    }

    /// Returns true if the neuron crossed the spike threshold on the last `step()`.
    ///
    /// Spike = x crossed from below threshold to above threshold.
    pub fn is_spiking(&self) -> bool {
        self.spiking
    }

    /// Compute the derivative vector `[dx/dt, dy/dt, dz/dt]` at the given state.
    fn deriv(state: &HindmarshRoseState, cfg: &HindmarshRoseConfig) -> [f64; 3] {
        let (x, y, z) = (state.x, state.y, state.z);
        let dx = y - cfg.a * x * x * x + cfg.b * x * x - z + cfg.i_ext;
        let dy = cfg.c - cfg.d * x * x - y;
        let dz = cfg.r * (cfg.s * (x - cfg.x_rest) - z);
        [dx, dy, dz]
    }

    /// Advance the neuron state by one RK4 step of size `dt`.
    pub fn step(&mut self, dt: f64) {
        self.prev_x = self.state.x;

        // RK4 integration
        let k1 = Self::deriv(&self.state, &self.config);

        let s2 = HindmarshRoseState {
            x: self.state.x + 0.5 * dt * k1[0],
            y: self.state.y + 0.5 * dt * k1[1],
            z: self.state.z + 0.5 * dt * k1[2],
        };
        let k2 = Self::deriv(&s2, &self.config);

        let s3 = HindmarshRoseState {
            x: self.state.x + 0.5 * dt * k2[0],
            y: self.state.y + 0.5 * dt * k2[1],
            z: self.state.z + 0.5 * dt * k2[2],
        };
        let k3 = Self::deriv(&s3, &self.config);

        let s4 = HindmarshRoseState {
            x: self.state.x + dt * k3[0],
            y: self.state.y + dt * k3[1],
            z: self.state.z + dt * k3[2],
        };
        let k4 = Self::deriv(&s4, &self.config);

        self.state.x += dt / 6.0 * (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]);
        self.state.y += dt / 6.0 * (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]);
        self.state.z += dt / 6.0 * (k1[2] + 2.0 * k2[2] + 2.0 * k3[2] + k4[2]);

        // Clamp to prevent divergence
        self.state.x = self.state.x.clamp(-5.0, 5.0);
        self.state.y = self.state.y.clamp(-20.0, 20.0);
        self.state.z = self.state.z.clamp(-5.0, 5.0);

        // Spike detection: upward threshold crossing
        self.spiking =
            self.prev_x < Self::SPIKE_THRESHOLD && self.state.x >= Self::SPIKE_THRESHOLD;
    }

    /// Generate a trajectory of `n_steps` x-values (membrane potential) starting
    /// from the current state.
    pub fn trajectory_x(&mut self, n_steps: usize, dt: f64) -> Vec<f64> {
        let mut out = Vec::with_capacity(n_steps);
        for _ in 0..n_steps {
            self.step(dt);
            out.push(self.state.x);
        }
        out
    }

    /// Count spikes over `n_steps` steps. Useful for quantifying firing rate.
    pub fn count_spikes(&mut self, n_steps: usize, dt: f64) -> usize {
        let mut count = 0;
        for _ in 0..n_steps {
            self.step(dt);
            if self.is_spiking() {
                count += 1;
            }
        }
        count
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_neuron() -> HindmarshRoseNeuron {
        HindmarshRoseNeuron::new(HindmarshRoseConfig::default())
    }

    #[test]
    fn test_default_config() {
        let cfg = HindmarshRoseConfig::default();
        assert_eq!(cfg.a, 1.0);
        assert_eq!(cfg.b, 3.0);
        assert_eq!(cfg.c, 1.0);
        assert_eq!(cfg.d, 5.0);
        assert!((cfg.r - 0.001).abs() < 1e-15);
        assert_eq!(cfg.s, 4.0);
        assert_eq!(cfg.x_rest, -1.6);
        assert_eq!(cfg.i_ext, 1.5);
    }

    #[test]
    fn test_default_state() {
        let st = HindmarshRoseState::default();
        assert_eq!(st.x, 0.0);
        assert_eq!(st.y, -8.0);
        assert_eq!(st.z, -1.6);
    }

    #[test]
    fn test_initial_state_matches_default() {
        let neuron = default_neuron();
        let st = neuron.state();
        assert_eq!(st.x, 0.0);
        assert_eq!(st.y, -8.0);
        assert_eq!(st.z, -1.6);
    }

    #[test]
    fn test_step_changes_state() {
        let mut neuron = default_neuron();
        let before = neuron.state().clone();
        neuron.step(0.01);
        let after = neuron.state();
        assert!(
            (before.x - after.x).abs() > 1e-15
                || (before.y - after.y).abs() > 1e-15
                || (before.z - after.z).abs() > 1e-15,
            "state must change after step"
        );
    }

    #[test]
    fn test_state_stays_finite() {
        let mut neuron = default_neuron();
        for _ in 0..10_000 {
            neuron.step(0.01);
            let st = neuron.state();
            assert!(st.x.is_finite(), "x diverged: {}", st.x);
            assert!(st.y.is_finite(), "y diverged: {}", st.y);
            assert!(st.z.is_finite(), "z diverged: {}", st.z);
        }
    }

    #[test]
    fn test_spike_detected() {
        let mut neuron = default_neuron();
        let mut spiked = false;
        for _ in 0..10_000 {
            neuron.step(0.01);
            if neuron.is_spiking() {
                spiked = true;
                break;
            }
        }
        assert!(spiked, "neuron should spike within 10000 steps with i_ext=1.5");
    }

    #[test]
    fn test_trajectory_x_length() {
        let mut neuron = default_neuron();
        let traj = neuron.trajectory_x(500, 0.01);
        assert_eq!(traj.len(), 500);
    }

    #[test]
    fn test_trajectory_values_finite() {
        let mut neuron = default_neuron();
        let traj = neuron.trajectory_x(1000, 0.01);
        for &v in &traj {
            assert!(v.is_finite(), "trajectory value is not finite: {}", v);
        }
    }

    #[test]
    fn test_count_spikes_positive() {
        let mut neuron = default_neuron();
        let n = neuron.count_spikes(10_000, 0.01);
        assert!(n > 0, "should detect at least one spike in 10000 steps");
    }

    #[test]
    fn test_deterministic() {
        let mut n1 = default_neuron();
        let mut n2 = default_neuron();
        for _ in 0..500 {
            n1.step(0.01);
            n2.step(0.01);
        }
        assert!((n1.state().x - n2.state().x).abs() < 1e-14);
        assert!((n1.state().y - n2.state().y).abs() < 1e-14);
        assert!((n1.state().z - n2.state().z).abs() < 1e-14);
    }

    #[test]
    fn test_different_i_ext_gives_different_trajectory() {
        let mut n_low = HindmarshRoseNeuron::new(HindmarshRoseConfig { i_ext: 1.0, ..Default::default() });
        let mut n_high = HindmarshRoseNeuron::new(HindmarshRoseConfig { i_ext: 3.5, ..Default::default() });
        for _ in 0..1000 {
            n_low.step(0.01);
            n_high.step(0.01);
        }
        assert!(
            (n_low.state().x - n_high.state().x).abs() > 0.01,
            "different i_ext should diverge: x_low={}, x_high={}",
            n_low.state().x, n_high.state().x
        );
    }

    #[test]
    fn test_with_state() {
        let cfg = HindmarshRoseConfig::default();
        let init = HindmarshRoseState { x: 1.0, y: -2.0, z: 0.5 };
        let mut neuron = HindmarshRoseNeuron::with_state(cfg, init.clone());
        assert_eq!(neuron.state().x, init.x);
        neuron.step(0.01);
        assert!(neuron.state().x.is_finite());
    }

    #[test]
    fn test_spiking_off_initially() {
        let neuron = default_neuron();
        // Before any step, spiking should be false
        assert!(!neuron.is_spiking());
    }
}
