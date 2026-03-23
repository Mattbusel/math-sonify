//! Granular synthesis engine.
//!
//! Implements a grain-based audio processing pipeline capable of time-stretching
//! and pitch-shifting via independent control of playback position and pitch ratio.

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Grain envelope
// ---------------------------------------------------------------------------

/// Window applied to each grain to avoid clicks.
#[derive(Debug, Clone)]
pub enum GrainEnvelope {
    Hann,
    Gaussian,
    Trapezoidal { attack_pct: f64, release_pct: f64 },
    Rectangular,
}

impl GrainEnvelope {
    /// Compute the amplitude at normalised position `t` ∈ [0, 1].
    pub fn apply(t: f64, envelope: &GrainEnvelope) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match envelope {
            GrainEnvelope::Hann => {
                0.5 * (1.0 - (2.0 * PI * t).cos())
            }
            GrainEnvelope::Gaussian => {
                let sigma = 0.4;
                let x = (t - 0.5) / sigma;
                (-0.5 * x * x).exp()
            }
            GrainEnvelope::Trapezoidal { attack_pct, release_pct } => {
                let attack  = attack_pct.clamp(0.0, 1.0);
                let release = release_pct.clamp(0.0, 1.0);
                if t < attack {
                    t / attack
                } else if t > 1.0 - release {
                    (1.0 - t) / release
                } else {
                    1.0
                }
            }
            GrainEnvelope::Rectangular => 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Grain
// ---------------------------------------------------------------------------

/// A single active grain reading from the source buffer.
#[derive(Debug, Clone)]
pub struct Grain {
    /// Offset in the source buffer where this grain starts.
    pub start_sample: usize,
    /// Length of the grain in samples.
    pub grain_size: usize,
    /// Pitch ratio (1.0 = original pitch).
    pub pitch_ratio: f64,
    /// Linear amplitude scalar.
    pub amplitude: f64,
    /// Pan position: -1 = full left, +1 = full right.
    pub pan: f64,
    pub envelope: GrainEnvelope,
    /// Current read position within the grain (fractional samples).
    pub current_pos: f64,
}

impl Grain {
    /// Returns true while the grain still has samples to produce.
    pub fn is_active(&self) -> bool {
        self.current_pos < self.grain_size as f64
    }

    /// Advance by one output sample.  Returns false when the grain is done.
    pub fn advance(&mut self) -> bool {
        if !self.is_active() {
            return false;
        }
        self.current_pos += self.pitch_ratio;
        self.is_active()
    }
}

// ---------------------------------------------------------------------------
// GranularParams
// ---------------------------------------------------------------------------

/// Runtime-configurable parameters for the granular engine.
#[derive(Debug, Clone)]
pub struct GranularParams {
    /// Source read position as a fraction of the buffer (0–1).
    pub source_position: f64,
    pub grain_size_ms: f64,
    /// Number of new grains spawned per second.
    pub grain_density: f64,
    pub pitch_ratio: f64,
    /// Scatter applied to the source position (fraction of buffer length).
    pub position_scatter: f64,
    /// Scatter applied to grain size (fraction of `grain_size_ms`).
    pub size_scatter: f64,
    /// Scatter applied to pitch ratio.
    pub pitch_scatter: f64,
    pub amplitude: f64,
    pub envelope: GrainEnvelope,
}

impl Default for GranularParams {
    fn default() -> Self {
        GranularParams {
            source_position: 0.0,
            grain_size_ms: 80.0,
            grain_density: 20.0,
            pitch_ratio: 1.0,
            position_scatter: 0.01,
            size_scatter: 0.1,
            pitch_scatter: 0.0,
            amplitude: 0.8,
            envelope: GrainEnvelope::Hann,
        }
    }
}

// ---------------------------------------------------------------------------
// LCG helpers
// ---------------------------------------------------------------------------

/// Simple linear congruential generator for deterministic randomness.
fn lcg_next(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*seed >> 33) as f64 / (u32::MAX as f64)
}

// ---------------------------------------------------------------------------
// GranularSynth
// ---------------------------------------------------------------------------

/// The main granular synthesis engine.
pub struct GranularSynth {
    pub source_buffer: Vec<f64>,
    pub active_grains: Vec<Grain>,
    pub params: GranularParams,
    pub sample_rate: f64,
    /// Running sample counter for density scheduling.
    pub time_samples: u64,
}

impl GranularSynth {
    pub fn new(source: Vec<f64>, sample_rate: f64, params: GranularParams) -> Self {
        GranularSynth {
            source_buffer: source,
            active_grains: Vec::new(),
            params,
            sample_rate,
            time_samples: 0,
        }
    }

    pub fn set_params(&mut self, params: GranularParams) {
        self.params = params;
    }

    /// Spawn a new grain with scatter applied via LCG.
    pub fn spawn_grain(&mut self, seed: u64) {
        if self.source_buffer.is_empty() {
            return;
        }
        let mut rng = seed;

        // Position scatter
        let pos_jitter = (lcg_next(&mut rng) - 0.5) * 2.0 * self.params.position_scatter;
        let src_pos = (self.params.source_position + pos_jitter).clamp(0.0, 1.0);
        let start_sample = ((src_pos * self.source_buffer.len() as f64) as usize)
            .min(self.source_buffer.len().saturating_sub(1));

        // Size scatter
        let size_jitter = 1.0 + (lcg_next(&mut rng) - 0.5) * 2.0 * self.params.size_scatter;
        let grain_size_ms = (self.params.grain_size_ms * size_jitter).max(1.0);
        let grain_size = ((grain_size_ms * 0.001 * self.sample_rate) as usize).max(1);

        // Pitch scatter
        let pitch_jitter = 1.0 + (lcg_next(&mut rng) - 0.5) * 2.0 * self.params.pitch_scatter;
        let pitch_ratio = (self.params.pitch_ratio * pitch_jitter).max(0.1);

        // Pan
        let pan = (lcg_next(&mut rng) - 0.5) * 0.4; // mild pan spread

        self.active_grains.push(Grain {
            start_sample,
            grain_size,
            pitch_ratio,
            amplitude: self.params.amplitude,
            pan,
            envelope: self.params.envelope.clone(),
            current_pos: 0.0,
        });
    }

    /// Read a single sample from the source buffer for the given grain
    /// using linear interpolation and the grain's pitch ratio.
    pub fn read_grain_sample(grain: &Grain, source: &[f64]) -> f64 {
        if source.is_empty() {
            return 0.0;
        }
        let abs_pos = grain.start_sample as f64 + grain.current_pos;
        let idx = abs_pos as usize;
        let frac = abs_pos - idx as f64;
        let s0 = source.get(idx).copied().unwrap_or(0.0);
        let s1 = source.get(idx + 1).copied().unwrap_or(0.0);
        s0 + frac * (s1 - s0)
    }

    /// Process `num_samples` output samples.  Returns interleaved `(left, right)`.
    pub fn process(&mut self, num_samples: usize, seed: u64) -> Vec<(f64, f64)> {
        let mut out = vec![(0.0_f64, 0.0_f64); num_samples];
        let samples_per_grain = if self.params.grain_density > 0.0 {
            (self.sample_rate / self.params.grain_density) as u64
        } else {
            u64::MAX
        };

        let mut rng_seed = seed;

        for i in 0..num_samples {
            // Spawn grains on schedule
            if samples_per_grain > 0 && self.time_samples % samples_per_grain == 0 {
                rng_seed = rng_seed.wrapping_add(self.time_samples).wrapping_mul(6364136223846793005);
                self.spawn_grain(rng_seed);
            }

            // Mix active grains
            let mut left = 0.0_f64;
            let mut right = 0.0_f64;

            for grain in self.active_grains.iter_mut() {
                if !grain.is_active() {
                    continue;
                }
                let t = grain.current_pos / grain.grain_size as f64;
                let env = GrainEnvelope::apply(t, &grain.envelope);
                let sample = Self::read_grain_sample(grain, &self.source_buffer);
                let amp = sample * env * grain.amplitude;

                // Equal-power panning
                let pan_l = ((1.0 - grain.pan) * 0.5 * std::f64::consts::FRAC_PI_2).cos();
                let pan_r = ((1.0 + grain.pan) * 0.5 * std::f64::consts::FRAC_PI_2).cos();
                left  += amp * pan_l;
                right += amp * pan_r;

                grain.advance();
            }

            out[i] = (left, right);
            self.active_grains.retain(|g| g.is_active());
            self.time_samples += 1;
        }
        out
    }

    /// Granular time stretch: same pitch, different playback speed.
    pub fn time_stretch(source: &[f64], ratio: f64, sample_rate: f64, seed: u64) -> Vec<f64> {
        let output_len = ((source.len() as f64) * ratio) as usize;
        let params = GranularParams {
            source_position: 0.0,
            grain_size_ms: 80.0,
            grain_density: 20.0,
            pitch_ratio: 1.0,
            position_scatter: 0.02,
            size_scatter: 0.1,
            pitch_scatter: 0.0,
            amplitude: 0.8,
            envelope: GrainEnvelope::Hann,
        };
        let mut synth = GranularSynth::new(source.to_vec(), sample_rate, params);

        // Advance source position linearly across the stretch
        let mut out = Vec::with_capacity(output_len);
        let block = 512usize;
        let mut written = 0usize;
        while written < output_len {
            let n = block.min(output_len - written);
            // Update position proportionally
            synth.params.source_position = (written as f64 / output_len as f64) / ratio;
            let stereo = synth.process(n, seed.wrapping_add(written as u64));
            out.extend(stereo.iter().map(|(l, r)| (l + r) * 0.5));
            written += n;
        }
        out
    }

    /// Granular pitch shift: different pitch, same duration.
    pub fn pitch_shift(source: &[f64], semitones: f64, sample_rate: f64, seed: u64) -> Vec<f64> {
        let pitch_ratio = 2.0_f64.powf(semitones / 12.0);
        let output_len = source.len();
        let params = GranularParams {
            source_position: 0.0,
            grain_size_ms: 60.0,
            grain_density: 25.0,
            pitch_ratio,
            position_scatter: 0.01,
            size_scatter: 0.05,
            pitch_scatter: 0.0,
            amplitude: 0.8,
            envelope: GrainEnvelope::Hann,
        };
        let mut synth = GranularSynth::new(source.to_vec(), sample_rate, params);

        let mut out = Vec::with_capacity(output_len);
        let block = 512usize;
        let mut written = 0usize;
        while written < output_len {
            let n = block.min(output_len - written);
            synth.params.source_position = written as f64 / output_len as f64;
            let stereo = synth.process(n, seed.wrapping_add(written as u64));
            out.extend(stereo.iter().map(|(l, r)| (l + r) * 0.5));
            written += n;
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_buf(len: usize, sample_rate: f64) -> Vec<f64> {
        (0..len)
            .map(|i| (2.0 * PI * 440.0 * i as f64 / sample_rate).sin() * 0.5)
            .collect()
    }

    #[test]
    fn test_hann_envelope_endpoints() {
        assert!(GrainEnvelope::apply(0.0, &GrainEnvelope::Hann).abs() < 1e-9);
        assert!(GrainEnvelope::apply(1.0, &GrainEnvelope::Hann).abs() < 1e-9);
    }

    #[test]
    fn test_grain_advance() {
        let mut g = Grain {
            start_sample: 0,
            grain_size: 4,
            pitch_ratio: 1.0,
            amplitude: 1.0,
            pan: 0.0,
            envelope: GrainEnvelope::Rectangular,
            current_pos: 0.0,
        };
        assert!(g.is_active());
        g.advance(); g.advance(); g.advance(); g.advance();
        assert!(!g.is_active());
    }

    #[test]
    fn test_process_returns_correct_length() {
        let buf = sine_buf(44100, 44100.0);
        let params = GranularParams::default();
        let mut synth = GranularSynth::new(buf, 44100.0, params);
        let out = synth.process(512, 42);
        assert_eq!(out.len(), 512);
    }

    #[test]
    fn test_time_stretch_output_length() {
        let buf = sine_buf(1000, 44100.0);
        let stretched = GranularSynth::time_stretch(&buf, 2.0, 44100.0, 1);
        assert_eq!(stretched.len(), 2000);
    }

    #[test]
    fn test_pitch_shift_preserves_length() {
        let buf = sine_buf(1000, 44100.0);
        let shifted = GranularSynth::pitch_shift(&buf, 12.0, 44100.0, 99);
        assert_eq!(shifted.len(), 1000);
    }
}
