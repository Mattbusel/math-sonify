pub mod direct;
pub mod orbital;
pub mod granular;
pub mod spectral;
pub mod fm;
pub mod vocal;

pub use direct::DirectMapping;
pub use orbital::OrbitalResonance;
pub use granular::GranularMapping;
pub use spectral::SpectralMapping;
pub use fm::FmMapping;
pub use vocal::VocalMapping;

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
        }
    }
}

pub fn chord_intervals_for(mode: &str) -> [f32; 3] {
    match mode {
        "major"  => [4.0, 7.0, 0.0],
        "minor"  => [3.0, 7.0, 0.0],
        "power"  => [7.0, 12.0, 0.0],
        "sus2"   => [2.0, 7.0, 0.0],
        "octave" => [12.0, 24.0, 0.0],
        "dom7"   => [4.0, 7.0, 10.0],
        _        => [0.0, 0.0, 0.0],
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SonifMode { #[default] Direct, Orbital, Granular, Spectral, FM, Vocal, Waveguide }

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
        }
    }
}

/// Musical scale quantization.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Scale { #[default] Pentatonic, Chromatic, JustIntonation, Microtonal }

/// Semitone intervals for each scale, relative to root.
fn scale_intervals(scale: Scale) -> &'static [f32] {
    match scale {
        Scale::Pentatonic =>      &[0.0, 2.0, 4.0, 7.0, 9.0],
        Scale::Chromatic =>       &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0],
        Scale::JustIntonation =>  &[0.0, 2.039, 3.863, 4.980, 7.020, 8.841, 10.884], // just major
        Scale::Microtonal =>      &[0.0, 0.75, 1.5, 2.25, 3.0, 3.75, 4.5, 5.25, 6.0, 6.75, 7.5, 8.25, 9.0],
    }
}

/// Quantize a continuous value [0..1] to a frequency on the given scale.
pub fn quantize_to_scale(t: f32, base_hz: f32, octave_range: f32, scale: Scale) -> f32 {
    let intervals = scale_intervals(scale);
    let n = intervals.len() as f32;
    // Map t to a position in the scale across octave_range octaves
    let total_steps = octave_range * n;
    let step_float = (t.clamp(0.0, 1.0) * total_steps) as usize;
    let octave = step_float / intervals.len();
    let degree = step_float % intervals.len();
    let semitones = octave as f32 * 12.0 + intervals[degree];
    base_hz * 2.0f32.powf(semitones / 12.0)
}

pub trait Sonification: Send {
    /// Map the system state to audio parameters.
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams;
}
