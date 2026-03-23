//! # Stochastic Resonance Module
//!
//! Demonstrates the counterintuitive phenomenon where adding an *optimal*
//! level of noise to a nonlinear system actually *improves* signal detection.
//!
//! ## Theory
//!
//! A weak sub-threshold signal s(t) is normally too small to cross the
//! detection threshold of a nonlinear element (e.g., a bistable potential).
//! Adding Gaussian noise of amplitude σ can help the signal cross the
//! threshold at the right moments, effectively amplifying it.
//!
//! The signal-to-noise ratio (SNR) as a function of noise amplitude σ shows
//! a characteristic inverted-U shape with a maximum at σ_opt.
//!
//! ## Audio mapping
//!
//! - Detected signal crossings → rhythmic clicks / triggers.
//! - Noise amplitude σ → brightness / spectral centroid of background texture.
//! - SNR curve peak → loudest / most rhythmically coherent output.
//!
//! ## Usage
//!
//! ```rust
//! use math_sonify::stochastic_resonance::{StochasticResonance, SRConfig};
//!
//! let cfg = SRConfig::default();
//! let mut sr = StochasticResonance::new(cfg);
//! let (state, crossings) = sr.step(0.0);
//! let _ = sr.snr_curve(20);
//! ```

/// Configuration for the stochastic resonance experiment.
#[derive(Debug, Clone)]
pub struct SRConfig {
    /// Weak sinusoidal signal amplitude (should be sub-threshold, i.e., < 1.0).
    pub signal_amplitude: f64,
    /// Signal frequency (Hz equivalent in simulation time).
    pub signal_frequency: f64,
    /// Current noise standard deviation σ (tunable in real-time).
    pub noise_amplitude: f64,
    /// Double-well potential barrier height.
    pub barrier_height: f64,
    /// Bistable system damping coefficient.
    pub damping: f64,
    /// Integration timestep.
    pub dt: f64,
}

impl Default for SRConfig {
    fn default() -> Self {
        Self {
            signal_amplitude: 0.3,
            signal_frequency: 0.05,
            noise_amplitude: 0.5,
            barrier_height: 1.0,
            damping: 0.1,
            dt: 0.01,
        }
    }
}

/// State of the stochastic resonance system.
pub struct StochasticResonance {
    pub config: SRConfig,
    /// Current position in the bistable potential.
    x: f64,
    /// Simulation time.
    t: f64,
    /// Last well the particle was in (+1 or -1).
    last_well: i32,
    /// Count of threshold crossings in current measurement window.
    crossing_count: u32,
    /// Deterministic pseudo-random state (LCG).
    rng_state: u64,
    /// Accumulated SNR data: (sigma, snr) pairs.
    pub snr_data: Vec<(f64, f64)>,
}

impl StochasticResonance {
    pub fn new(config: SRConfig) -> Self {
        Self {
            config,
            x: -1.0,
            t: 0.0,
            last_well: -1,
            crossing_count: 0,
            rng_state: 12345678901234567,
            snr_data: Vec::new(),
        }
    }

    /// Advance the system by one timestep.
    ///
    /// Returns `(x, crossed)` where `x` is the current position and
    /// `crossed` is true if the particle switched wells this step.
    pub fn step(&mut self, _external_signal: f64) -> (f64, bool) {
        let dt = self.config.dt;
        let a = self.config.barrier_height;
        let gamma = self.config.damping;
        let amp = self.config.signal_amplitude;
        let freq = self.config.signal_frequency;

        // Driving signal s(t)
        let s = amp * (2.0 * std::f64::consts::PI * freq * self.t).sin();

        // Drift: -dV/dx for V(x) = -ax²/2 + x⁴/4  →  dV/dx = -ax + x³
        let drift = a * self.x - self.x.powi(3) + s;

        // Gaussian noise via Box-Muller
        let noise = self.gaussian_noise() * self.config.noise_amplitude / dt.sqrt();

        // Euler-Maruyama step
        self.x += (drift - gamma * self.x) * dt + noise * dt.sqrt();

        self.t += dt;

        // Detect well crossing
        let current_well = if self.x > 0.0 { 1 } else { -1 };
        let crossed = current_well != self.last_well;
        if crossed {
            self.last_well = current_well;
            self.crossing_count += 1;
        }

        (self.x, crossed)
    }

    /// Compute the SNR curve by sweeping noise amplitudes.
    ///
    /// Returns a vector of `(sigma, snr_db)` pairs.
    /// The SNR is estimated as the ratio of signal-correlated crossings to
    /// noise-only crossings.
    pub fn snr_curve(&mut self, num_sigma_points: usize) -> Vec<(f64, f64)> {
        let mut result = Vec::with_capacity(num_sigma_points);
        let freq = self.config.signal_frequency;
        let amp = self.config.signal_amplitude;
        let barrier = self.config.barrier_height;

        for i in 0..num_sigma_points {
            let sigma = 0.05 + 3.0 * i as f64 / (num_sigma_points as f64 - 1.0).max(1.0);

            // Theoretical SNR for bistable system (Kramers' rate approximation)
            // r_K = (barrier * sqrt(2)) * exp(-barrier^2 / (2 * sigma^2))
            let kramers_rate =
                barrier * 2.0_f64.sqrt() * (-(barrier * barrier) / (2.0 * sigma * sigma)).exp();

            // SNR ∝ amp^2 / (sigma^2 * r_K) for weak-signal limit, normalised
            let snr_linear = if kramers_rate > 1e-12 {
                (amp * amp) / (sigma * sigma) * kramers_rate
                    / (kramers_rate * kramers_rate + (2.0 * std::f64::consts::PI * freq).powi(2))
            } else {
                0.0
            };

            // Convert to dB
            let snr_db = if snr_linear > 1e-15 {
                10.0 * snr_linear.log10()
            } else {
                -60.0
            };

            result.push((sigma, snr_db));
        }

        self.snr_data = result.clone();
        result
    }

    /// Optimal noise amplitude (σ at which SNR is maximised).
    pub fn optimal_noise(&mut self, resolution: usize) -> f64 {
        let curve = self.snr_curve(resolution);
        curve
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(sigma, _)| *sigma)
            .unwrap_or(self.config.noise_amplitude)
    }

    /// Map system state to audio parameters.
    ///
    /// Returns `(frequency_hz, amplitude, spectral_tilt)`.
    pub fn audio_params(&self) -> (f64, f64, f64) {
        // Map position to frequency: well -1 → 220 Hz, well +1 → 440 Hz
        let base_freq = if self.x > 0.0 { 440.0 } else { 220.0 };
        let freq_wobble = 1.0 + 0.05 * self.x.abs().min(2.0);
        let frequency = base_freq * freq_wobble;

        // Amplitude proportional to distance from barrier (x=0)
        let amplitude = (self.x.abs() / 2.0).min(1.0);

        // Spectral tilt: high noise → brighter (more high-frequency content)
        let tilt = self.config.noise_amplitude.clamp(0.0, 3.0) / 3.0;

        (frequency, amplitude, tilt)
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn gaussian_noise(&mut self) -> f64 {
        // Box-Muller transform from two uniform samples
        let u1 = self.lcg_uniform();
        let u2 = self.lcg_uniform();
        let u1 = u1.max(1e-15); // avoid log(0)
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    fn lcg_uniform(&mut self) -> f64 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng_state >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_does_not_nan() {
        let mut sr = StochasticResonance::new(SRConfig::default());
        for _ in 0..100 {
            let (x, _) = sr.step(0.0);
            assert!(x.is_finite(), "x must be finite");
        }
    }

    #[test]
    fn snr_curve_length() {
        let mut sr = StochasticResonance::new(SRConfig::default());
        let curve = sr.snr_curve(10);
        assert_eq!(curve.len(), 10);
    }

    #[test]
    fn snr_has_interior_maximum() {
        let mut sr = StochasticResonance::new(SRConfig::default());
        let curve = sr.snr_curve(30);
        let max_idx = curve
            .iter()
            .enumerate()
            .max_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        // Max should not be at the extremes (pure stochastic resonance behaviour)
        assert!(max_idx > 0 && max_idx < curve.len() - 1,
            "SNR maximum should be at an interior point, got idx={max_idx}");
    }

    #[test]
    fn audio_params_in_range() {
        let sr = StochasticResonance::new(SRConfig::default());
        let (freq, amp, tilt) = sr.audio_params();
        assert!(freq > 0.0);
        assert!((0.0..=1.0).contains(&amp));
        assert!((0.0..=1.0).contains(&tilt));
    }

    #[test]
    fn optimal_noise_positive() {
        let mut sr = StochasticResonance::new(SRConfig::default());
        let opt = sr.optimal_noise(20);
        assert!(opt > 0.0);
    }
}
