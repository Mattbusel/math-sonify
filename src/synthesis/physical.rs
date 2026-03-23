//! Physical modeling synthesis modes.
//!
//! This module wraps the lower-level [`KarplusStrong`] and [`WaveguideString`]
//! DSP primitives (in `src/synth/`) in higher-level "instrument" abstractions
//! that accept normalised state vectors from the dynamical system and produce
//! audio frames.
//!
//! # Synthesis modes
//!
//! | Mode | DSP core | Character |
//! |------|----------|-----------|
//! | [`PluckedString`] | Karplus-Strong | Bright plucked string, guitar/harp |
//! | [`TubeResonator`] | Waveguide (two-delay) + resonator | Wind instrument, tube resonance |
//!
//! Both instruments expose a common [`PhysicalSynth`] trait so the audio
//! engine can dispatch dynamically.

#![allow(dead_code)]

use crate::synth::{KarplusStrong, ResonatorBank, WaveguideString};

// ── Trait ─────────────────────────────────────────────────────────────────────

/// Common interface for physical modeling synthesis modes.
pub trait PhysicalSynth: Send {
    /// Map a normalised state vector (values in roughly `[-1, 1]`) to synthesis
    /// parameters and produce the next audio sample.
    ///
    /// - `state[0]` → pitch / frequency
    /// - `state[1]` → brightness / timbre
    /// - `state[2]` → excitation / pluck strength (if present)
    fn next_sample(&mut self, state: &[f64], sample_rate: f32) -> f32;

    /// True if this synth is currently active (producing non-zero output).
    fn is_active(&self) -> bool;

    /// Force re-excitation (e.g. when the attractor crosses a threshold).
    fn excite(&mut self, freq_hz: f32, sample_rate: f32);

    /// Set master output volume (0–1).
    fn set_volume(&mut self, vol: f32);
}

// ── Plucked string ────────────────────────────────────────────────────────────

/// Karplus-Strong plucked string instrument driven by ODE state.
///
/// The dynamical system state modulates:
/// - `state[0]` → pitch (mapped to a frequency range)
/// - `state[1]` → brightness (IIR coefficient)
/// - Automatic re-excitation when the pitch changes by more than a threshold
pub struct PluckedString {
    ks: KarplusStrong,
    /// Minimum frequency in Hz.
    freq_min: f32,
    /// Maximum frequency in Hz.
    freq_max: f32,
    last_freq: f32,
    /// Frequency change that triggers a re-excitation (cents).
    retrigger_cents: f32,
    /// Excitation cooldown in samples to avoid clicking.
    cooldown: u32,
    cooldown_remaining: u32,
    volume: f32,
}

impl PluckedString {
    /// Create a plucked string with the given frequency range.
    pub fn new(freq_min: f32, freq_max: f32, sample_rate: f32) -> Self {
        Self {
            ks: KarplusStrong::new(freq_min.max(10.0), sample_rate),
            freq_min: freq_min.max(10.0),
            freq_max: freq_max.max(freq_min + 1.0),
            last_freq: 0.0,
            retrigger_cents: 200.0, // 2 semitones
            cooldown: (sample_rate * 0.05) as u32, // 50 ms
            cooldown_remaining: 0,
            volume: 0.7,
        }
    }

    /// Map a normalised value in `[-1, 1]` to a frequency in Hz (log scale).
    fn map_freq(&self, x: f64) -> f32 {
        let t = ((x + 1.0) * 0.5).clamp(0.0, 1.0) as f32;
        let log_min = self.freq_min.ln();
        let log_max = self.freq_max.ln();
        (log_min + t * (log_max - log_min)).exp()
    }

    /// Cents distance between two frequencies.
    fn cents_diff(f1: f32, f2: f32) -> f32 {
        if f1 <= 0.0 || f2 <= 0.0 {
            return f32::MAX;
        }
        (1200.0 * (f2 / f1).log2()).abs()
    }
}

impl PhysicalSynth for PluckedString {
    fn next_sample(&mut self, state: &[f64], sample_rate: f32) -> f32 {
        if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
        }

        let freq = self
            .map_freq(state.first().copied().unwrap_or(0.0));

        // Re-excite if pitch has moved significantly.
        if self.cooldown_remaining == 0
            && Self::cents_diff(self.last_freq, freq) > self.retrigger_cents
        {
            self.ks.trigger(freq, sample_rate);
            self.last_freq = freq;
            self.cooldown_remaining = self.cooldown;
        }

        // Modulate brightness from state[1].
        if let Some(&s1) = state.get(1) {
            let b = ((s1 + 1.0) * 0.5).clamp(0.0, 1.0) as f32 * 0.8;
            self.ks.brightness = b;
        }

        self.ks.volume = self.volume;
        self.ks.next_sample()
    }

    fn is_active(&self) -> bool {
        self.ks.active
    }

    fn excite(&mut self, freq_hz: f32, sample_rate: f32) {
        self.ks.trigger(freq_hz, sample_rate);
        self.last_freq = freq_hz;
        self.cooldown_remaining = self.cooldown;
    }

    fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
    }
}

// ── Tube resonator ────────────────────────────────────────────────────────────

/// Cylindrical/conical tube resonator using a bidirectional waveguide model.
///
/// Models a wind-instrument resonating column.  The ODE state modulates:
/// - `state[0]` → resonant frequency (embouchure / fingering)
/// - `state[1]` → damping (open vs. stopped)
/// - `state[2]` → excitation pressure (breathiness)
///
/// A [`ResonatorBank`] adds body resonance on top of the waveguide output.
pub struct TubeResonator {
    wg: WaveguideString,
    resonators: ResonatorBank,
    /// Frequency range.
    freq_min: f32,
    freq_max: f32,
    /// Continuous excitation level derived from `state[2]`.
    excite_level: f32,
    volume: f32,
    sample_rate: f32,
    /// Noise state for breath excitation.
    noise_seed: u64,
}

impl TubeResonator {
    /// Create a tube resonator for the given frequency range.
    pub fn new(freq_min: f32, freq_max: f32, sample_rate: f32) -> Self {
        let mut wg = WaveguideString::new(sample_rate);
        wg.set_freq(freq_min);
        wg.damping = 0.998;
        wg.brightness = 0.2; // bright tube
        wg.dispersion = 0.0; // ideal tube (no stiffness)

        // Three body resonance peaks typical of a clarinet/oboe body.
        let resonators = ResonatorBank::new(sample_rate, &[
            (freq_min * 1.5, 20.0, 0.4),   // first mode
            (freq_min * 2.5, 15.0, 0.25),  // second mode
            (freq_min * 4.0, 10.0, 0.15),  // third mode
        ]);

        Self {
            wg,
            resonators,
            freq_min: freq_min.max(20.0),
            freq_max: freq_max.max(freq_min + 1.0),
            excite_level: 0.0,
            volume: 0.6,
            sample_rate,
            noise_seed: 0xDEAD_CAFE_1234_5678,
        }
    }

    fn map_freq(&self, x: f64) -> f32 {
        let t = ((x + 1.0) * 0.5).clamp(0.0, 1.0) as f32;
        let log_min = self.freq_min.ln();
        let log_max = self.freq_max.ln();
        (log_min + t * (log_max - log_min)).exp()
    }

    /// Produce a breath noise sample scaled by `level`.
    fn breath_noise(&mut self, level: f32) -> f32 {
        self.noise_seed = self
            .noise_seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let n = (self.noise_seed >> 33) as f32 / (1u64 << 31) as f32 * 2.0 - 1.0;
        n * level * 0.1
    }
}

impl PhysicalSynth for TubeResonator {
    fn next_sample(&mut self, state: &[f64], _sample_rate: f32) -> f32 {
        // Map frequency.
        let freq = self.map_freq(state.first().copied().unwrap_or(0.0));
        self.wg.set_freq(freq);

        // Map damping from state[1].
        if let Some(&s1) = state.get(1) {
            let d = 0.990 + ((s1 + 1.0) * 0.5).clamp(0.0, 1.0) as f32 * 0.008;
            self.wg.damping = d;
        }

        // Breath pressure from state[2] — continuous excitation.
        let pressure = state.get(2).copied().unwrap_or(0.5);
        self.excite_level = ((pressure + 1.0) * 0.5).clamp(0.0, 1.0) as f32;

        // Inject breath noise into the waveguide.
        if self.excite_level > 0.01 {
            let noise = self.breath_noise(self.excite_level);
            // Pulse the excite flag when noise is large enough.
            if noise.abs() > 0.05 {
                self.wg.excite = true;
                self.wg.excite_pos = 0.1; // near the mouthpiece
            }
        }

        let wg_out = self.wg.next_sample();
        let resonated = self.resonators.process(wg_out);

        (wg_out * 0.7 + resonated * 0.3) * self.volume
    }

    fn is_active(&self) -> bool {
        // The tube resonator is continuously active when driven by breath.
        self.excite_level > 0.001
    }

    fn excite(&mut self, freq_hz: f32, _sample_rate: f32) {
        self.wg.set_freq(freq_hz);
        self.wg.excite = true;
        self.excite_level = 0.5;
    }

    fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Available physical synthesis modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalMode {
    PluckedString,
    TubeResonator,
}

/// Construct the appropriate physical synth.
pub fn build_physical_synth(
    mode: PhysicalMode,
    freq_min: f32,
    freq_max: f32,
    sample_rate: f32,
) -> Box<dyn PhysicalSynth> {
    match mode {
        PhysicalMode::PluckedString => {
            Box::new(PluckedString::new(freq_min, freq_max, sample_rate))
        }
        PhysicalMode::TubeResonator => {
            Box::new(TubeResonator::new(freq_min, freq_max, sample_rate))
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_plucked_string_produces_output_after_excite() {
        let mut ps = PluckedString::new(80.0, 1200.0, SR);
        ps.excite(440.0, SR);
        let mut max = 0.0_f32;
        let state = [0.0f64, 0.0, 0.0];
        for _ in 0..4410 {
            let s = ps.next_sample(&state, SR);
            max = max.max(s.abs());
        }
        assert!(max > 0.0, "PluckedString should produce output after excite");
    }

    #[test]
    fn test_plucked_string_output_finite() {
        let mut ps = PluckedString::new(80.0, 1200.0, SR);
        ps.excite(220.0, SR);
        let state = [0.0f64, 0.5, -0.5];
        for i in 0..22050 {
            let s = ps.next_sample(&state, SR);
            assert!(s.is_finite(), "non-finite at sample {i}");
        }
    }

    #[test]
    fn test_tube_resonator_produces_output() {
        let mut tr = TubeResonator::new(60.0, 800.0, SR);
        tr.excite(220.0, SR);
        let state = [0.0f64, 0.0, 0.5];
        let mut max = 0.0_f32;
        for _ in 0..4410 {
            max = max.max(tr.next_sample(&state, SR).abs());
        }
        assert!(max > 0.0, "TubeResonator should produce output after excite");
    }

    #[test]
    fn test_tube_resonator_output_finite() {
        let mut tr = TubeResonator::new(60.0, 800.0, SR);
        tr.excite(110.0, SR);
        let state = [0.3f64, -0.2, 0.8];
        for i in 0..22050 {
            let s = tr.next_sample(&state, SR);
            assert!(s.is_finite(), "non-finite at sample {i}");
        }
    }

    #[test]
    fn test_build_physical_synth_factory() {
        let mut ps = build_physical_synth(PhysicalMode::PluckedString, 80.0, 1200.0, SR);
        ps.excite(440.0, SR);
        let state = [0.0f64];
        let _ = ps.next_sample(&state, SR);

        let mut tr = build_physical_synth(PhysicalMode::TubeResonator, 60.0, 800.0, SR);
        tr.excite(220.0, SR);
        let _ = tr.next_sample(&state, SR);
    }
}
