use super::{AudioParams, SonifMode, Sonification};
use crate::config::SonificationConfig;

/// Orbital resonance: angular velocity in the projected 2D plane → fundamental.
/// Harmonics are weighted by a profile that shifts with chaos level,
/// so ordered attractors sound string-like and chaotic ones sound metallic/bell-like.
pub struct OrbitalResonance {
    prev_state: Vec<f64>,
    prev_angle: Option<f64>,
    lyap_estimate: f64,
    // Smoothed fundamental — prevents jarring jumps as the attractor moves
    smooth_fund: f32,
}

impl OrbitalResonance {
    pub fn new() -> Self {
        Self {
            prev_state: Vec::new(),
            prev_angle: None,
            lyap_estimate: 0.0,
            smooth_fund: 0.0,
        }
    }
}

/// Harmonic amplitude profiles.
/// Each entry is the amplitude of partial n (0-indexed, n=0 is the fundamental).
///
/// - `string`: 1/n falloff  — rich but fundamental-dominated (cello, violin)
/// - `woodwind`: 1/n² falloff — mellow, odd partials slightly louder (clarinet)
/// - `bell`: inharmonic; only partials 1, 2, 5, 7 are prominent (bell/marimba)
///
/// The actual weights are crossfaded based on `stretch` (chaos estimate):
///   stretch ≈ 0 → string-like (ordered attractor → musical intervals)
///   stretch ≈ 1 → bell-like  (chaotic attractor → inharmonic cluster)
fn harmonic_amp(partial: usize, stretch: f32) -> f32 {
    let n = (partial + 1) as f32;

    // String profile: classic 1/n, warm and rich
    let string_amp = 1.0 / n;

    // Bell profile: emphasises 2nd, 5th, 7th partials like a real bell
    // (based on Helmholtz's analysis of struck bells)
    let bell_amp = match partial {
        0 => 1.0,
        1 => 0.6,
        4 => 0.4,
        6 => 0.3,
        _ => 0.05 / n,
    };

    // Crossfade between profiles
    string_amp * (1.0 - stretch) + bell_amp * stretch
}

impl Sonification for OrbitalResonance {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let mut params = AudioParams {
            mode: SonifMode::Orbital,
            gain: 0.2,
            filter_cutoff: 1500.0,
            filter_q: 1.2,
            ..Default::default()
        };

        // --- Angular velocity → fundamental -----------------------------------
        let (x, y) = (state[0], state[1]);
        let angle = y.atan2(x);

        let angular_vel = if let Some(prev_a) = self.prev_angle {
            let da = angle - prev_a;
            let da_unwrapped = if da > std::f64::consts::PI {
                da - std::f64::consts::TAU
            } else if da < -std::f64::consts::PI {
                da + std::f64::consts::TAU
            } else {
                da
            };
            da_unwrapped.abs() * 60.0
        } else {
            1.0
        };
        self.prev_angle = Some(angle);

        // --- Lyapunov-based chaos estimate ------------------------------------
        if !self.prev_state.is_empty() {
            let divergence: f64 = state
                .iter()
                .zip(self.prev_state.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            let log_div = divergence.ln().clamp(-5.0, 5.0);
            self.lyap_estimate = self.lyap_estimate * 0.99 + log_div * 0.01;
        }
        self.prev_state = state.to_vec();

        let base = config.base_frequency as f32;
        let stretch = (self.lyap_estimate.tanh() * 0.5 + 0.5) as f32; // 0 = ordered, 1 = chaotic

        // Raw fundamental from angular velocity
        let raw_fund = (angular_vel.abs() as f32 * base * 0.05).clamp(base * 0.0625, base * 4.0);

        // Smooth the fundamental to avoid jarring register jumps as the
        // attractor crosses its own path.  Coefficient 0.02 → ~50 control frames
        // (≈ 400 ms at 120 Hz) to reach a new pitch — legato character.
        if self.smooth_fund < 10.0 {
            self.smooth_fund = raw_fund;
        }
        self.smooth_fund += 0.02 * (raw_fund - self.smooth_fund);
        let fundamental = self.smooth_fund;

        // --- Inharmonic partial series ----------------------------------------
        // Ordered attractor (stretch≈0): harmonic series, string amplitudes
        // Chaotic attractor (stretch≈1): stretched partials, bell amplitudes
        for i in 0..4 {
            let n = (i + 1) as f32;
            // Inharmonic stretch: f_n = f₁ · n^(1 + stretch·0.35)
            params.freqs[i] = fundamental * n.powf(1.0 + stretch * 0.35);
            params.amps[i] = harmonic_amp(i, stretch);
            // Stereo: even partials slightly left, odd slightly right — matches
            // how real instruments project different harmonics into the room.
            params.pans[i] = if i % 2 == 0 { -0.35 } else { 0.35 };
        }

        // If the state has a z-dimension, use it to drive the sub-octave voice
        // that grounds the texture with low-frequency energy.
        // sub_osc_level drives a sine at half voice[0]'s frequency — do NOT
        // overwrite freqs[0]/amps[0] here, which would destroy the harmonic series.
        if state.len() >= 3 {
            let z_norm = (state[2].tanh() * 0.5 + 0.5) as f32;
            params.sub_osc_level = harmonic_amp(0, stretch) * (0.5 + 0.5 * z_norm);
        }

        // Filter: ordered → warm & dark, chaotic → bright & resonant
        params.filter_cutoff = 400.0 + 4000.0 * stretch;
        params.filter_q = 0.5 + 2.0 * stretch; // resonant filter peak at chaos
        params.gain = (0.15 + 0.08 * speed.tanh() as f32).min(0.35);
        params.chaos_level = stretch;
        params
    }
}
