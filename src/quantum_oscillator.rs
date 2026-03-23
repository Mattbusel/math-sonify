//! # Quantum Harmonic Oscillator Sonification
//!
//! Sonify the quantum harmonic oscillator's wavefunction ψ(x,t) as it evolves
//! in time under the Schrödinger equation.
//!
//! ## Physics
//!
//! The energy eigenstates of the harmonic oscillator are:
//!
//!   ψₙ(x) = Aₙ · Hₙ(x) · exp(-x²/2)
//!
//! where Hₙ is the n-th Hermite polynomial and Aₙ is a normalisation constant.
//!
//! Time evolution: ψₙ(x,t) = ψₙ(x) · exp(-iEₙt/ℏ), with Eₙ = (n+1/2)ℏω.
//!
//! A superposition of energy eigenstates:
//!
//!   Ψ(x,t) = Σₙ cₙ · ψₙ(x) · exp(-iωₙt)
//!
//! produces an oscillating wavepacket.  The probability density |Ψ(x,t)|²
//! peaks at different x positions as the packet oscillates.
//!
//! ## Audio mapping
//!
//! - Spatial probability peaks → oscillator frequencies (peaks → pitches).
//! - Peak sharpness → amplitude of each partial.
//! - Superposition coefficients |cₙ|² → chord structure.
//! - Wavefunction collapse (simulated reset to eigenstate) → percussion hit.
//!
//! ## Usage
//!
//! ```rust
//! use math_sonify::quantum_oscillator::{QuantumOscillator, QuantumConfig};
//!
//! let cfg = QuantumConfig { max_n: 5, ..Default::default() };
//! let mut qo = QuantumOscillator::new(cfg);
//! qo.set_superposition(&[(0, 1.0), (1, 1.0), (2, 0.5)]);
//! let audio = qo.synthesise(44100, 256);
//! assert_eq!(audio.len(), 256);
//! ```

/// Configuration for the quantum harmonic oscillator sonification.
#[derive(Debug, Clone)]
pub struct QuantumConfig {
    /// Maximum energy level (n) to include in superpositions.
    pub max_n: usize,
    /// Spatial grid size (number of x points).
    pub grid_points: usize,
    /// Spatial extent: x ∈ [-x_max, +x_max].
    pub x_max: f64,
    /// Angular frequency ω (sets the energy spacing: Eₙ = (n+½)ω).
    pub omega: f64,
    /// Base audio frequency (Hz) — maps the lowest peak position.
    pub base_frequency: f64,
    /// Audio frequency range (Hz) — maps the full spatial extent.
    pub frequency_range: f64,
    /// Probability of a spontaneous "collapse" event per step.
    pub collapse_probability: f64,
    /// Amplitude of the collapse percussion hit.
    pub collapse_amplitude: f64,
}

impl Default for QuantumConfig {
    fn default() -> Self {
        Self {
            max_n: 8,
            grid_points: 128,
            x_max: 4.0,
            omega: 1.0,
            base_frequency: 110.0,
            frequency_range: 880.0,
            collapse_probability: 0.001,
            collapse_amplitude: 0.5,
        }
    }
}

/// A single audio partial derived from a probability density peak.
#[derive(Debug, Clone)]
pub struct QuantumPartial {
    /// Frequency (Hz).
    pub frequency: f64,
    /// Amplitude.
    pub amplitude: f64,
    /// Current phase.
    pub phase: f64,
}

impl QuantumPartial {
    fn tick(&mut self, sample_rate: f64) -> f64 {
        let out = self.amplitude * self.phase.sin();
        self.phase += 2.0 * std::f64::consts::PI * self.frequency / sample_rate;
        if self.phase > std::f64::consts::TAU {
            self.phase -= std::f64::consts::TAU;
        }
        out
    }
}

/// Quantum harmonic oscillator sonifier.
pub struct QuantumOscillator {
    pub config: QuantumConfig,
    /// Superposition coefficients cₙ (complex: re, im).
    coefficients: Vec<(f64, f64)>,
    /// Simulation time.
    t: f64,
    /// Time step per `step()` call.
    pub dt: f64,
    /// Active audio partials.
    partials: Vec<QuantumPartial>,
    /// Pending collapse hit amplitude (set to >0 on collapse).
    collapse_hit: f64,
    /// LCG random state.
    rng: u64,
    /// Cached probability density on the spatial grid.
    pub prob_density: Vec<f64>,
}

impl QuantumOscillator {
    pub fn new(config: QuantumConfig) -> Self {
        let max_n = config.max_n;
        let grid = config.grid_points;
        let mut qo = Self {
            coefficients: vec![(0.0, 0.0); max_n + 1],
            t: 0.0,
            dt: 0.02,
            partials: Vec::new(),
            collapse_hit: 0.0,
            rng: 314159265358979,
            prob_density: vec![0.0; grid],
            config,
        };
        // Default: ground state
        qo.coefficients[0] = (1.0, 0.0);
        qo.update_prob_density();
        qo
    }

    /// Set the superposition state from (n, amplitude) pairs.
    ///
    /// Coefficients are automatically normalised so Σ|cₙ|² = 1.
    pub fn set_superposition(&mut self, levels: &[(usize, f64)]) {
        self.coefficients = vec![(0.0, 0.0); self.config.max_n + 1];
        let sum_sq: f64 = levels.iter().map(|(_, a)| a * a).sum::<f64>().max(1e-12);
        let norm = sum_sq.sqrt();
        for &(n, amp) in levels {
            if n <= self.config.max_n {
                self.coefficients[n] = (amp / norm, 0.0);
            }
        }
        self.update_prob_density();
    }

    /// Advance the wavefunction by `dt` seconds.
    ///
    /// Returns true if a collapse event occurred this step.
    pub fn step(&mut self) -> bool {
        // Time-evolve: cₙ(t) = cₙ(0) · exp(-iEₙt)  →  rotate each coefficient
        let omega = self.config.omega;
        for (n, (re, im)) in self.coefficients.iter_mut().enumerate() {
            let energy = omega * (n as f64 + 0.5);
            let phase = -energy * self.dt; // ℏ = 1 units
            let cos_p = phase.cos();
            let sin_p = phase.sin();
            let new_re = *re * cos_p - *im * sin_p;
            let new_im = *re * sin_p + *im * cos_p;
            *re = new_re;
            *im = new_im;
        }
        self.t += self.dt;

        // Stochastic collapse
        let collapsed = self.lcg_float() < self.config.collapse_probability;
        if collapsed {
            self.collapse();
        }

        self.update_prob_density();
        self.update_partials();
        collapsed
    }

    /// Collapse the wavefunction to the most probable eigenstate.
    pub fn collapse(&mut self) {
        // Find eigenstate with largest |cₙ|²
        let best_n = self
            .coefficients
            .iter()
            .enumerate()
            .max_by(|(_, (ra, ia)), (_, (rb, ib))| {
                let pa = ra * ra + ia * ia;
                let pb = rb * rb + ib * ib;
                pa.partial_cmp(&pb).unwrap()
            })
            .map(|(n, _)| n)
            .unwrap_or(0);

        // Reset to that eigenstate
        self.coefficients = vec![(0.0, 0.0); self.config.max_n + 1];
        self.coefficients[best_n] = (1.0, 0.0);
        self.collapse_hit = self.config.collapse_amplitude;
    }

    /// Synthesise `num_samples` audio samples.
    pub fn synthesise(&mut self, sample_rate: u32, num_samples: usize) -> Vec<f32> {
        let sr = sample_rate as f64;
        (0..num_samples)
            .map(|_| {
                let partial_sum: f64 =
                    self.partials.iter_mut().map(|p| p.tick(sr)).sum();

                // Add collapse percussion (exponentially decaying click)
                let hit = self.collapse_hit;
                self.collapse_hit *= 0.99;

                let out = partial_sum + hit;
                out.tanh() as f32
            })
            .collect()
    }

    /// Current probability density |Ψ(x,t)|² on the spatial grid.
    pub fn probability_density(&self) -> &[f64] {
        &self.prob_density
    }

    /// Peak positions in the probability density (x values where |Ψ|² is local max).
    pub fn probability_peaks(&self) -> Vec<f64> {
        let n = self.config.grid_points;
        let dx = 2.0 * self.config.x_max / n as f64;
        let pd = &self.prob_density;
        (1..n - 1)
            .filter(|&i| pd[i] > pd[i - 1] && pd[i] > pd[i + 1] && pd[i] > 1e-4)
            .map(|i| -self.config.x_max + i as f64 * dx)
            .collect()
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    /// Evaluate the n-th eigenstate ψₙ(x) = Aₙ · Hₙ(x) · exp(-x²/2).
    fn eigenstate_at(n: usize, x: f64) -> f64 {
        let hn = hermite(n, x);
        let norm = (2.0_f64.powi(n as i32)
            * factorial(n) as f64
            * std::f64::consts::PI.sqrt())
        .sqrt();
        let norm = norm.max(1e-300);
        hn / norm * (-x * x / 2.0).exp()
    }

    fn update_prob_density(&mut self) {
        let n_pts = self.config.grid_points;
        let x_max = self.config.x_max;
        let dx = 2.0 * x_max / n_pts as f64;

        for i in 0..n_pts {
            let x = -x_max + i as f64 * dx;
            // Ψ(x,t) = Σₙ cₙ · ψₙ(x)  (coefficients already time-rotated)
            let mut re_sum = 0.0_f64;
            let mut im_sum = 0.0_f64;
            for (n, &(cn_re, cn_im)) in self.coefficients.iter().enumerate() {
                let psi_n = Self::eigenstate_at(n, x);
                re_sum += cn_re * psi_n;
                im_sum += cn_im * psi_n;
            }
            self.prob_density[i] = re_sum * re_sum + im_sum * im_sum;
        }
    }

    fn update_partials(&mut self) {
        let peaks = self.probability_peaks();
        let x_max = self.config.x_max;
        let base = self.config.base_frequency;
        let range = self.config.frequency_range;

        // Map each peak position to a frequency
        let max_pd = self.prob_density.iter().cloned().fold(0.0_f64, f64::max).max(1e-12);

        self.partials = peaks
            .iter()
            .map(|&x| {
                let t = (x + x_max) / (2.0 * x_max); // normalise to [0,1]
                let frequency = base + t * range;
                // Amplitude proportional to |Ψ(x)|² at this peak
                let grid_idx = (((x + x_max) / (2.0 * x_max) * self.config.grid_points as f64)
                    as usize)
                    .min(self.config.grid_points - 1);
                let amplitude = (self.prob_density[grid_idx] / max_pd).min(1.0);
                // Preserve phase from matching partial if exists
                let phase = self
                    .partials
                    .iter()
                    .find(|p| (p.frequency - frequency).abs() < range / 10.0)
                    .map(|p| p.phase)
                    .unwrap_or(0.0);
                QuantumPartial {
                    frequency,
                    amplitude,
                    phase,
                }
            })
            .collect();
    }

    fn lcg_float(&mut self) -> f64 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ── Hermite polynomials (physicists') ─────────────────────────────────────────

/// Compute the n-th physicists' Hermite polynomial H_n(x).
///
/// Recurrence: H₀=1, H₁=2x, Hₙ = 2x·Hₙ₋₁ - 2(n-1)·Hₙ₋₂
fn hermite(n: usize, x: f64) -> f64 {
    match n {
        0 => 1.0,
        1 => 2.0 * x,
        _ => {
            let mut h_prev2 = 1.0_f64;
            let mut h_prev1 = 2.0 * x;
            for k in 2..=n {
                let h = 2.0 * x * h_prev1 - 2.0 * (k - 1) as f64 * h_prev2;
                h_prev2 = h_prev1;
                h_prev1 = h;
            }
            h_prev1
        }
    }
}

fn factorial(n: usize) -> u64 {
    (1..=n as u64).product::<u64>().max(1)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_state_probability_normalised() {
        let mut qo = QuantumOscillator::new(QuantumConfig {
            grid_points: 256,
            x_max: 5.0,
            ..Default::default()
        });
        qo.set_superposition(&[(0, 1.0)]);
        let dx = 2.0 * qo.config.x_max / qo.config.grid_points as f64;
        let integral: f64 = qo.prob_density.iter().sum::<f64>() * dx;
        assert!((integral - 1.0).abs() < 0.05, "normalisation: {integral}");
    }

    #[test]
    fn step_advances_time() {
        let mut qo = QuantumOscillator::new(QuantumConfig::default());
        let t0 = qo.t;
        qo.step();
        assert!(qo.t > t0);
    }

    #[test]
    fn synthesise_length() {
        let mut qo = QuantumOscillator::new(QuantumConfig::default());
        qo.set_superposition(&[(0, 1.0), (1, 1.0), (2, 0.5)]);
        let audio = qo.synthesise(44100, 256);
        assert_eq!(audio.len(), 256);
        assert!(audio.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn synthesise_clipped() {
        let mut qo = QuantumOscillator::new(QuantumConfig::default());
        for _ in 0..10 {
            qo.step();
        }
        let audio = qo.synthesise(44100, 128);
        for s in &audio {
            assert!(s.abs() <= 1.0 + 1e-6, "out of tanh range: {s}");
        }
    }

    #[test]
    fn collapse_resets_to_eigenstate() {
        let mut qo = QuantumOscillator::new(QuantumConfig::default());
        qo.set_superposition(&[(0, 1.0), (2, 1.0), (4, 1.0)]);
        qo.collapse();
        // After collapse, exactly one coefficient should be non-zero
        let nonzero: usize = qo
            .coefficients
            .iter()
            .filter(|(re, im)| re * re + im * im > 1e-10)
            .count();
        assert_eq!(nonzero, 1, "after collapse, exactly one eigenstate should remain");
    }

    #[test]
    fn hermite_polynomials() {
        // H₀(1) = 1
        assert!((hermite(0, 1.0) - 1.0).abs() < 1e-10);
        // H₁(1) = 2
        assert!((hermite(1, 1.0) - 2.0).abs() < 1e-10);
        // H₂(1) = 4(1)² - 2 = 2
        assert!((hermite(2, 1.0) - 2.0).abs() < 1e-10);
        // H₃(1) = 8(1)³ - 12(1) = -4
        assert!((hermite(3, 1.0) - (-4.0)).abs() < 1e-10);
    }

    #[test]
    fn probability_density_non_negative() {
        let mut qo = QuantumOscillator::new(QuantumConfig::default());
        qo.set_superposition(&[(0, 0.7), (1, 0.3), (3, 0.5)]);
        for &pd in &qo.prob_density {
            assert!(pd >= 0.0, "probability density must be non-negative: {pd}");
        }
    }
}
