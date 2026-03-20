use super::DynamicalSystem;

/// Discrete-time delay map: x[n+1] = r * x[n] * (1 - x[n-tau])
///
/// Uses a ring buffer of length tau+1 to store history.
/// The `step(dt)` method advances one discrete step regardless of dt.
pub struct DelayedMap {
    /// Ring buffer holding x history; length = tau + 1
    history: Vec<f64>,
    /// Current write position in the ring buffer
    head: usize,
    pub r: f64,
    pub tau: usize,
    /// Exposed state: [x_current, x_delayed]
    state: Vec<f64>,
    speed: f64,
}

impl DelayedMap {
    pub fn new(r: f64, tau: usize) -> Self {
        let buf_len = tau + 1;
        // Initialize all history to 0.5 (interior of [0,1])
        let history = vec![0.5; buf_len];
        let state = vec![0.5, 0.5];
        Self {
            history,
            head: 0,
            r,
            tau,
            state,
            speed: 0.0,
        }
    }

    fn current(&self) -> f64 {
        self.history[self.head]
    }

    fn delayed(&self) -> f64 {
        let buf_len = self.history.len();
        let delayed_idx = (self.head + buf_len - self.tau) % buf_len;
        self.history[delayed_idx]
    }
}

impl DynamicalSystem for DelayedMap {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        2
    }

    fn name(&self) -> &str {
        "Delayed Map"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn set_state(&mut self, s: &[f64]) {
        // Reset entire history to the provided x value (or 0.5 if not given)
        let x = if !s.is_empty() && s[0].is_finite() { s[0] } else { 0.5 };
        for v in &mut self.history {
            *v = x;
        }
        self.head = 0;
        self.state[0] = x;
        self.state[1] = x;
    }

    fn step(&mut self, _dt: f64) {
        let x_cur = self.current();
        let x_del = self.delayed();
        let x_next = self.r * x_cur * (1.0 - x_del);

        // Advance head
        let buf_len = self.history.len();
        self.head = (self.head + 1) % buf_len;
        self.history[self.head] = x_next;

        let prev_x = self.state[0];
        self.state[0] = x_next;
        self.state[1] = self.delayed();
        self.speed = (x_next - prev_x).abs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_delayed_map_initial_state() {
        let sys = DelayedMap::new(3.9, 3);
        let s = sys.state();
        assert_eq!(s.len(), 2);
        assert!((s[0] - 0.5).abs() < 1e-15, "Expected x=0.5");
        assert!((s[1] - 0.5).abs() < 1e-15, "Expected x_delayed=0.5");
        assert_eq!(sys.name(), "Delayed Map");
        assert_eq!(sys.dimension(), 2);
    }

    #[test]
    fn test_delayed_map_step_changes_state() {
        let mut sys = DelayedMap::new(3.9, 3);
        let before: Vec<f64> = sys.state().to_vec();
        // Need a few steps to get past the constant history warm-up
        for _ in 0..5 {
            sys.step(0.01);
        }
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_delayed_map_step_runs_without_panic() {
        // Verify that stepping the delayed map does not panic.
        // Note: unlike the local logistic map, the delayed version can diverge
        // because the stabilizing (1-x) feedback uses old state, not current.
        let mut sys = DelayedMap::new(3.9, 3);
        for _ in 0..100 {
            sys.step(0.01);
        }
        // At minimum, state should be accessible (not panic)
        let _ = sys.state();
        let _ = sys.speed();
    }

    #[test]
    fn test_delayed_map_set_state() {
        let mut sys = DelayedMap::new(3.9, 3);
        sys.set_state(&[0.7]);
        // After set_state, state[0] and state[1] should both be 0.7
        let s = sys.state();
        assert!((s[0] - 0.7).abs() < 1e-15, "state[0] should be 0.7");
        assert!((s[1] - 0.7).abs() < 1e-15, "state[1] should be 0.7");
    }

    #[test]
    fn test_delayed_map_r_affects_dynamics() {
        // Different r values should produce different trajectories after warmup
        let mut sys_low = DelayedMap::new(2.5, 3);
        let mut sys_high = DelayedMap::new(3.9, 3);
        for _ in 0..20 {
            sys_low.step(0.01);
            sys_high.step(0.01);
        }
        let s_low = sys_low.state()[0];
        let s_high = sys_high.state()[0];
        assert!(
            (s_low - s_high).abs() > 1e-6,
            "Different r should give different trajectories: {} vs {}", s_low, s_high
        );
    }

    #[test]
    fn test_delayed_map_longer_tau_different_dynamics() {
        // tau=1 and tau=5 should diverge quickly
        let mut sys_short = DelayedMap::new(3.9, 1);
        let mut sys_long = DelayedMap::new(3.9, 5);
        for _ in 0..10 {
            sys_short.step(0.01);
            sys_long.step(0.01);
        }
        let s_short = sys_short.state()[0];
        let s_long = sys_long.state()[0];
        assert!(
            (s_short - s_long).abs() > 1e-6,
            "tau=1 and tau=5 should diverge: {} vs {}", s_short, s_long
        );
    }

    #[test]
    fn test_delayed_map_state_finite() {
        // With moderate r, state should remain finite
        let mut sys = DelayedMap::new(2.0, 3);
        for _ in 0..1000 {
            sys.step(0.01);
        }
        assert!(sys.state().iter().all(|v| v.is_finite()),
            "State should stay finite: {:?}", sys.state());
    }
}
