//! Sonification layer: maps dynamical-system state vectors to [`AudioParams`].
//!
//! Each mode implements the [`Sonification`] trait.  The simulation thread calls
//! [`Sonification::map`] at 120 Hz and sends the resulting [`AudioParams`] to the
//! audio thread via a bounded crossbeam channel.  The audio thread switches on
//! [`AudioParams::mode`] to select the appropriate synthesis path.
//!
//! | Mode | Description |
//! |------|-------------|
//! | [`DirectMapping`] | State variables quantized to a musical scale |
//! | [`OrbitalResonance`] | Angular velocity and Lyapunov exponent drive pitch and inharmonicity |
//! | [`GranularMapping`] | Trajectory speed modulates grain density and pitch |
//! | [`SpectralMapping`] | State vector controls a 32-partial additive envelope |
//! | [`FmMapping`] | Attractor drives FM carrier/modulator ratio and index |
//! | [`VocalMapping`] | State interpolates between vowel formant positions |

// Sonification types are constructed via build_mapper() using dynamic dispatch;
// the compiler can't see through the string-based dispatch, hence these suppressions.
#![allow(dead_code)]

pub mod am;
pub mod direct;
pub mod fm;
pub mod granular;
pub mod orbital;
pub mod spectral;
pub mod vocal;
pub mod waveguide_mapper;

pub use am::AmMapping;
pub use direct::DirectMapping;
pub use fm::FmMapping;
pub use granular::GranularMapping;
pub use orbital::OrbitalResonance;
pub use spectral::SpectralMapping;
pub use vocal::VocalMapping;
pub use waveguide_mapper::WaveguideMapping;

use crate::config::SonificationConfig;
use crate::synth::OscShape;

/// Parameters computed from the dynamical system state, consumed by the audio thread.
#[derive(Clone)]
pub struct AudioParams {
    /// Oscillator frequencies for voices (Hz).
    pub freqs: [f32; 4],
    /// Amplitudes for each voice (0..1).
    pub amps: [f32; 4],
    /// Filter cutoff frequency (Hz).
    pub filter_cutoff: f32,
    /// Filter Q.
    pub filter_q: f32,
    /// Stereo pan for voices (-1..1).
    pub pans: [f32; 4],
    /// Granular: spawn rate and base frequency.
    pub grain_spawn_rate: f32,
    pub grain_base_freq: f32,
    pub grain_freq_spread: f32,
    /// Spectral: amplitude of each harmonic partial (up to 32).
    pub partials: [f32; 32],
    pub partials_base_freq: f32,
    /// Active sonification mode — tells the audio thread which path to render.
    pub mode: SonifMode,
    /// Master gain scalar (already accounts for per-mode normalization).
    pub gain: f32,
    /// Semitone transpose applied to all voices.
    pub transpose_semitones: f32,
    /// Semitone offsets for 3 chord voices from voice[0] (0 = off).
    pub chord_intervals: [f32; 3],
    /// Per-voice amplitude mix 0..1.
    pub voice_levels: [f32; 4],
    /// Frequency glide time in milliseconds.
    pub portamento_ms: f32,
    /// Chaos estimate 0..1 for display.
    pub chaos_level: f32,
    /// Master volume (0..1).
    pub master_volume: f32,
    /// Reverb wet mix (0..1).
    pub reverb_wet: f32,
    /// Delay time in milliseconds.
    pub delay_ms: f32,
    /// Delay feedback (0..1).
    pub delay_feedback: f32,
    /// FM synthesis parameters
    pub fm_carrier_freq: f32,
    pub fm_mod_ratio: f32,
    pub fm_mod_index: f32,
    /// Per-voice waveform shapes
    pub voice_shapes: [OscShape; 4],
    /// Bitcrusher parameters
    pub bit_depth: f32,
    pub rate_crush: f32,
    /// Karplus-Strong trigger
    pub ks_trigger: bool,
    pub ks_freq: f32,
    pub ks_volume: f32,
    /// Chorus parameters
    pub chorus_mix: f32,
    pub chorus_rate: f32,
    pub chorus_depth: f32,
    /// Waveshaper parameters
    pub waveshaper_drive: f32,
    pub waveshaper_mix: f32,
    /// ADSR envelope parameters
    pub adsr_attack_ms: f32,
    pub adsr_decay_ms: f32,
    pub adsr_sustain: f32,
    pub adsr_release_ms: f32,
    /// Per-layer mix level (0..1) and pan (-1..1) for polyphony layers
    pub layer_level: f32,
    pub layer_pan: f32,
    pub layer_id: usize,
    /// Waveguide physical modeling params
    pub waveguide_tension: f32,
    pub waveguide_damping: f32,
    pub waveguide_excite: bool,
    /// Spectral freeze
    pub spectral_freeze_active: bool,
    pub spectral_freeze_freqs: [f32; 16],
    pub spectral_freeze_amps: [f32; 16],
    /// 3-band EQ (dB, ±12)
    pub eq_low_db: f32,
    pub eq_mid_db: f32,
    pub eq_high_db: f32,
    pub eq_mid_freq: f32,
    /// Unison/detune spread in cents (0 = no detune).
    pub voice_detune_cents: f32,
    /// Sub oscillator level (0..1).
    pub sub_osc_level: f32,
    /// Grain density multiplier (0.1..4.0, default 1.0).
    pub grain_density: f32,
}

impl Default for AudioParams {
    fn default() -> Self {
        Self {
            freqs: [0.0; 4],
            amps: [0.0; 4],
            filter_cutoff: 0.0,
            filter_q: 0.0,
            pans: [0.0; 4],
            grain_spawn_rate: 0.0,
            grain_base_freq: 0.0,
            grain_freq_spread: 0.0,
            partials: [0.0; 32],
            partials_base_freq: 0.0,
            mode: SonifMode::Direct,
            gain: 0.0,
            transpose_semitones: 0.0,
            chord_intervals: [0.0; 3],
            voice_levels: [1.0, 0.8, 0.6, 0.4],
            portamento_ms: 80.0,
            chaos_level: 0.0,
            master_volume: 0.7,
            reverb_wet: 0.4,
            delay_ms: 300.0,
            delay_feedback: 0.3,
            fm_carrier_freq: 220.0,
            fm_mod_ratio: 2.0,
            fm_mod_index: 1.0,
            voice_shapes: [OscShape::Sine; 4],
            bit_depth: 16.0,
            rate_crush: 0.0,
            ks_trigger: false,
            ks_freq: 220.0,
            ks_volume: 0.5,
            chorus_mix: 0.0,
            chorus_rate: 0.5,
            chorus_depth: 3.0,
            waveshaper_drive: 1.0,
            waveshaper_mix: 0.0,
            adsr_attack_ms: 10.0,
            adsr_decay_ms: 200.0,
            adsr_sustain: 0.7,
            adsr_release_ms: 400.0,
            layer_level: 1.0,
            layer_pan: 0.0,
            layer_id: 0,
            waveguide_tension: 0.5,
            waveguide_damping: 0.98,
            waveguide_excite: false,
            spectral_freeze_active: false,
            spectral_freeze_freqs: [0.0; 16],
            spectral_freeze_amps: [0.0; 16],
            eq_low_db: 0.0,
            eq_mid_db: 0.0,
            eq_high_db: 0.0,
            eq_mid_freq: 1000.0,
            voice_detune_cents: 0.0,
            sub_osc_level: 0.0,
            grain_density: 1.0,
        }
    }
}

/// Return the semitone intervals above voice[0] for a given chord type.
///
/// Returns `[upper1_semitones, upper2_semitones, 0.0]` (trailing zero means
/// the third chord voice is omitted for two-note chord types).
pub fn chord_intervals_for(mode: &str) -> [f32; 3] {
    match mode {
        "major" => [4.0, 7.0, 0.0],
        "minor" => [3.0, 7.0, 0.0],
        "power" => [7.0, 12.0, 0.0],
        "sus2" => [2.0, 7.0, 0.0],
        "octave" => [12.0, 24.0, 0.0],
        "dom7" => [4.0, 7.0, 10.0],
        "min7" => [3.0, 7.0, 10.0],
        "maj7" => [4.0, 7.0, 11.0],
        "aug" => [4.0, 8.0, 0.0],
        "dim" => [3.0, 6.0, 0.0],
        "sus4" => [5.0, 7.0, 0.0],
        _ => [0.0, 0.0, 0.0],
    }
}

/// Selects which sonification algorithm maps the dynamical system state to audio.
///
/// - `Direct`: each of the first four state variables is mapped linearly to an
///   oscillator frequency and amplitude.
/// - `Orbital`: state variables are interpreted as orbital elements; voices track
///   angular velocity and radial distance.
/// - `Granular`: trajectory speed and position control grain spawn rate and pitch;
///   the grain cloud thickens as chaos increases.
/// - `Spectral`: up to 32 partial amplitudes are filled from the Fourier content
///   of the trajectory, producing continuously evolving additive spectra.
/// - `FM`: two-operator frequency modulation where the modulation index and
///   carrier-to-modulator ratio are driven by the attractor state.
/// - `Vocal`: state space coordinates are mapped to vowel formant positions
///   (F1/F2 pairs), producing evolving vocal-texture synthesis.
/// - `Waveguide`: a Karplus-Strong waveguide string whose tension and damping
///   are modulated by the attractor trajectory in real time.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SonifMode {
    #[default]
    Direct,
    Orbital,
    Granular,
    Spectral,
    FM,
    Vocal,
    Waveguide,
    AM,
    Resonator,
}

impl std::fmt::Display for SonifMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => write!(f, "Direct"),
            Self::Orbital => write!(f, "Orbital"),
            Self::Granular => write!(f, "Granular"),
            Self::Spectral => write!(f, "Spectral"),
            Self::FM => write!(f, "FM"),
            Self::Vocal => write!(f, "Vocal"),
            Self::Waveguide => write!(f, "Waveguide"),
            Self::AM => write!(f, "AM"),
            Self::Resonator => write!(f, "Resonator"),
        }
    }
}

/// Musical scale quantization.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Scale {
    #[default]
    Pentatonic,
    Chromatic,
    JustIntonation,
    Microtonal,
    Edo19,          // 19 equal divisions of the octave
    Edo31,          // 31 equal divisions of the octave
    Edo24,          // 24-EDO: quarter-tones
    WholeTone,      // 6-note whole-tone scale
    Phrygian,       // E Phrygian: 0, 1, 3, 5, 7, 8, 10
    Lydian,         // F Lydian: 0, 2, 4, 6, 7, 9, 11
    HarmonicSeries, // Integer multiples of 110 Hz (A2)
}

/// Semitone intervals for non-computed scales, relative to root.
fn scale_intervals(scale: Scale) -> &'static [f32] {
    match scale {
        Scale::Pentatonic => &[0.0, 2.0, 4.0, 7.0, 9.0],
        Scale::Chromatic => &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0],
        Scale::JustIntonation => &[0.0, 2.039, 3.863, 4.980, 7.020, 8.841, 10.884], // just major
        Scale::Microtonal => &[
            0.0, 0.75, 1.5, 2.25, 3.0, 3.75, 4.5, 5.25, 6.0, 6.75, 7.5, 8.25, 9.0,
        ],
        Scale::WholeTone => &[0.0, 2.0, 4.0, 6.0, 8.0, 10.0],
        Scale::Phrygian => &[0.0, 1.0, 3.0, 5.0, 7.0, 8.0, 10.0],
        Scale::Lydian => &[0.0, 2.0, 4.0, 6.0, 7.0, 9.0, 11.0],
        // EDO and HarmonicSeries scales have computed intervals -- handled in scale_intervals_owned
        Scale::Edo19 | Scale::Edo31 | Scale::Edo24 | Scale::HarmonicSeries => &[0.0],
    }
}

/// Return the full interval set as an owned Vec. EDO scales are computed here.
/// For non-EDO scales, this delegates to scale_intervals to avoid duplication.
fn scale_intervals_owned(scale: Scale) -> Vec<f32> {
    match scale {
        Scale::Edo19 => (0..19).map(|i| i as f32 * 12.0 / 19.0).collect(),
        Scale::Edo31 => (0..31).map(|i| i as f32 * 12.0 / 31.0).collect(),
        Scale::Edo24 => (0..24).map(|i| i as f32 * 0.5).collect(),
        other => scale_intervals(other).to_vec(),
    }
}

/// Harmonic series frequencies: integer multiples of A2 (110 Hz).
const HARMONIC_SERIES: &[f32] = &[
    110.0, 220.0, 330.0, 440.0, 550.0, 660.0, 770.0, 880.0, 990.0, 1100.0, 1320.0, 1540.0,
];

/// Quantize a continuous value [0..1] to a frequency on the given scale.
pub fn quantize_to_scale(t: f32, base_hz: f32, octave_range: f32, scale: Scale) -> f32 {
    // HarmonicSeries: pick the nearest harmonic to the desired frequency
    if scale == Scale::HarmonicSeries {
        let t = t.clamp(0.0, 1.0);
        // Map t to a target frequency across the octave_range
        let target_hz = base_hz * 2.0f32.powf(t * octave_range);
        return HARMONIC_SERIES
            .iter()
            .copied()
            .min_by(|a, b| {
                (a - target_hz)
                    .abs()
                    .partial_cmp(&(b - target_hz).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(110.0);
    }

    let intervals = scale_intervals_owned(scale);
    let n = intervals.len() as f32;
    // Map t to a position in the scale across octave_range octaves
    let total_steps = octave_range * n;
    // Allow t=1.0 to reach the final note (top octave root).
    // Previously `.saturating_sub(1)` prevented the highest note from ever sounding.
    let step_float = ((t.clamp(0.0, 1.0) * total_steps) as usize).min(total_steps as usize);
    let octave = step_float / intervals.len();
    let degree = step_float % intervals.len();
    let semitones = octave as f32 * 12.0 + intervals[degree];
    base_hz * 2.0f32.powf(semitones / 12.0)
}

/// Trait implemented by every sonification algorithm.
///
/// Each call to `map` converts the current dynamical-system state vector into
/// an `AudioParams` value.  The audio thread calls the sonification mapper at
/// the control rate (120 Hz by default) and forwards the result to the DSP
/// synthesis engine.
///
/// Implementors must be `Send` because the mapper runs on a dedicated
/// simulation thread separate from both the UI thread and the audio thread.
pub trait Sonification: Send {
    /// Convert the system state to audio parameters.
    ///
    /// # Arguments
    /// * `state`  - Current dynamical system state vector (length = `dimension()`).
    /// * `speed`  - Euclidean magnitude of dx/dt, used to modulate grain density.
    /// * `config` - Active sonification configuration (scale, base frequency, etc.).
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams;
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // chord_intervals_for
    // -------------------------------------------------------------------------

    #[test]
    fn chord_intervals_major() {
        let iv = chord_intervals_for("major");
        assert!((iv[0] - 4.0).abs() < 1e-6, "major: expected 4 semitones, got {}", iv[0]);
        assert!((iv[1] - 7.0).abs() < 1e-6, "major: expected 7 semitones, got {}", iv[1]);
    }

    #[test]
    fn chord_intervals_dom7_three_voices() {
        let iv = chord_intervals_for("dom7");
        assert!((iv[0] - 4.0).abs() < 1e-6);
        assert!((iv[1] - 7.0).abs() < 1e-6);
        assert!((iv[2] - 10.0).abs() < 1e-6, "dom7 third voice: expected 10, got {}", iv[2]);
    }

    #[test]
    fn chord_intervals_unknown_returns_zeros() {
        let iv = chord_intervals_for("definitely_not_a_chord");
        assert_eq!(iv, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn chord_intervals_all_named_modes_return_finite() {
        for mode in &["major", "minor", "power", "sus2", "octave", "dom7", "min7", "maj7", "aug", "dim", "sus4"] {
            let iv = chord_intervals_for(mode);
            for (i, &v) in iv.iter().enumerate() {
                assert!(v.is_finite(), "chord {} interval[{}] non-finite", mode, i);
            }
        }
    }

    // -------------------------------------------------------------------------
    // quantize_to_scale — new scale variants
    // -------------------------------------------------------------------------

    #[test]
    fn quantize_whole_tone_returns_finite_in_range() {
        let base = 220.0f32;
        let oct = 2.0f32;
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let f = quantize_to_scale(t, base, oct, Scale::WholeTone);
            assert!(f.is_finite() && f > 0.0, "WholeTone: non-positive at t={}: {}", t, f);
            assert!(f >= base * 0.99, "WholeTone: below base at t={}: {}", t, f);
        }
    }

    #[test]
    fn quantize_phrygian_returns_finite() {
        let base = 220.0f32;
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let f = quantize_to_scale(t, base, 2.0, Scale::Phrygian);
            assert!(f.is_finite() && f > 0.0, "Phrygian: invalid at t={}: {}", t, f);
        }
    }

    #[test]
    fn quantize_lydian_returns_finite() {
        let base = 440.0f32;
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let f = quantize_to_scale(t, base, 2.0, Scale::Lydian);
            assert!(f.is_finite() && f > 0.0, "Lydian: invalid at t={}: {}", t, f);
        }
    }

    #[test]
    fn quantize_harmonic_series_snaps_to_harmonics() {
        // HarmonicSeries should return values from the known harmonic series
        let known: &[f32] = &[110.0, 220.0, 330.0, 440.0, 550.0, 660.0, 770.0, 880.0, 990.0, 1100.0, 1320.0, 1540.0];
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let f = quantize_to_scale(t, 110.0, 4.0, Scale::HarmonicSeries);
            assert!(
                known.iter().any(|&h| (f - h).abs() < 1.0),
                "HarmonicSeries: {} not in known harmonics at t={}",
                f,
                t
            );
        }
    }

    #[test]
    fn quantize_edo19_returns_monotone_frequencies() {
        let base = 220.0f32;
        let mut prev = 0.0f32;
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let f = quantize_to_scale(t, base, 2.0, Scale::Edo19);
            assert!(f >= prev, "EDO19 not monotone at t={}: {} < {}", t, f, prev);
            prev = f;
        }
    }

    #[test]
    fn quantize_edo31_frequencies_in_range() {
        let base = 220.0f32;
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let f = quantize_to_scale(t, base, 2.0, Scale::Edo31);
            assert!(f >= base * 0.99 && f.is_finite(), "EDO31 out of range at t={}: {}", t, f);
        }
    }

    #[test]
    fn quantize_just_intonation_returns_valid_frequencies() {
        let base = 220.0f32;
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let f = quantize_to_scale(t, base, 2.0, Scale::JustIntonation);
            assert!(f >= base * 0.99 && f.is_finite(), "JustIntonation at t={}: {}", t, f);
        }
    }
}
