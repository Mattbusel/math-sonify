use crate::config::*;

/// A named synthesis preset bundling a human-readable description with a
/// corresponding [`Config`] that can be loaded into the engine.
pub struct Preset {
    /// Display name shown in the UI preset picker.
    pub name: &'static str,
    /// One-line description of the sonic character of this preset.
    pub description: &'static str,
}

/// Built-in preset catalogue.
///
/// Each entry maps a human-readable name to a short description.  Call
/// [`load_preset`] with a name from this slice to obtain the corresponding
/// [`Config`].
pub const PRESETS: &[Preset] = &[
    Preset { name: "Lorenz Ambience", description: "Slow drift through the butterfly attractor" },
    Preset { name: "Pendulum Rhythm", description: "Chaotic pendulum drives granular pulses" },
    Preset { name: "Torus Drone", description: "Ergodic geodesic flow, major chord, deep reverb" },
    Preset { name: "Kuramoto Sync", description: "Watch synchronization emerge as you raise coupling" },
    Preset { name: "Three-Body Jazz", description: "Figure-8 orbit, dom7 chord, spectral mode" },
    Preset { name: "Rössler Drift", description: "Gentle spiral attractor, microtonal scale" },
    Preset { name: "FM Chaos", description: "Frequency modulation driven by the butterfly attractor" },
    Preset { name: "Pendulum Meditation", description: "Slow pendulum drift through pure harmonic ratios" },
];

/// Load the [`Config`] for a named preset.
///
/// Returns the preset configuration for the given `name`, or
/// `Config::default()` if the name does not match any known preset.
pub fn load_preset(name: &str) -> Config {
    match name {
        "Lorenz Ambience" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 1.0 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 110.0, octave_range: 2.5,
                chord_mode: "major".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3], portamento_ms: 300.0,
            },
            audio: AudioConfig { reverb_wet: 0.65, delay_ms: 400.0, delay_feedback: 0.4, master_volume: 0.7, ..Default::default() },
            lorenz: LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 },
            ..Default::default()
        },
        "Pendulum Rhythm" => Config {
            system: SystemConfig { name: "double_pendulum".into(), dt: 0.001, speed: 2.0 },
            sonification: SonificationConfig {
                mode: "granular".into(), scale: "pentatonic".into(),
                base_frequency: 220.0, octave_range: 2.0,
                chord_mode: "power".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4], portamento_ms: 50.0,
            },
            audio: AudioConfig { reverb_wet: 0.3, delay_ms: 200.0, delay_feedback: 0.5, master_volume: 0.75, ..Default::default() },
            ..Default::default()
        },
        "Torus Drone" => Config {
            system: SystemConfig { name: "geodesic_torus".into(), dt: 0.005, speed: 0.5 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 55.0, octave_range: 3.0,
                chord_mode: "major".into(), transpose_semitones: -12.0,
                voice_levels: [1.0, 0.9, 0.7, 0.5], portamento_ms: 400.0,
            },
            audio: AudioConfig { reverb_wet: 0.7, delay_ms: 600.0, delay_feedback: 0.35, master_volume: 0.65, ..Default::default() },
            geodesic_torus: GeodesicTorusConfig { big_r: 3.0, r: 1.0 },
            ..Default::default()
        },
        "Kuramoto Sync" => Config {
            system: SystemConfig { name: "kuramoto".into(), dt: 0.002, speed: 1.0 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "chromatic".into(),
                base_frequency: 440.0, octave_range: 2.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.8, 0.8], portamento_ms: 30.0,
            },
            audio: AudioConfig { reverb_wet: 0.4, delay_ms: 250.0, delay_feedback: 0.3, master_volume: 0.7, ..Default::default() },
            kuramoto: KuramotoConfig { n_oscillators: 8, coupling: 0.5 },
            ..Default::default()
        },
        "Three-Body Jazz" => Config {
            system: SystemConfig { name: "three_body".into(), dt: 0.001, speed: 1.5 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "just_intonation".into(),
                base_frequency: 146.83, octave_range: 3.0,
                chord_mode: "dom7".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.7, 0.5], portamento_ms: 60.0,
            },
            audio: AudioConfig { reverb_wet: 0.5, delay_ms: 330.0, delay_feedback: 0.4, master_volume: 0.7, ..Default::default() },
            ..Default::default()
        },
        "Rössler Drift" => Config {
            system: SystemConfig { name: "rossler".into(), dt: 0.002, speed: 0.8 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "microtonal".into(),
                base_frequency: 180.0, octave_range: 4.0,
                chord_mode: "sus2".into(), transpose_semitones: 7.0,
                voice_levels: [1.0, 0.6, 0.4, 0.3], portamento_ms: 150.0,
            },
            audio: AudioConfig { reverb_wet: 0.55, delay_ms: 450.0, delay_feedback: 0.38, master_volume: 0.65, ..Default::default() },
            rossler: RosslerConfig { a: 0.2, b: 0.2, c: 5.7 },
            ..Default::default()
        },
        "FM Chaos" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 1.8 },
            sonification: SonificationConfig {
                mode: "fm".into(), scale: "chromatic".into(),
                base_frequency: 220.0, octave_range: 2.5,
                chord_mode: "minor".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3], portamento_ms: 40.0,
            },
            audio: AudioConfig { reverb_wet: 0.3, delay_ms: 180.0, delay_feedback: 0.4, master_volume: 0.7, bit_depth: 12.0, rate_crush: 0.0, ..Default::default() },
            lorenz: LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 },
            ..Default::default()
        },
        "Pendulum Meditation" => Config {
            system: SystemConfig { name: "double_pendulum".into(), dt: 0.001, speed: 0.6 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 80.0, octave_range: 2.0,
                chord_mode: "major".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.7, 0.5], portamento_ms: 500.0,
            },
            audio: AudioConfig { reverb_wet: 0.75, delay_ms: 700.0, delay_feedback: 0.3, master_volume: 0.65, ..Default::default() },
            ..Default::default()
        },
        _ => Config::default(),
    }
}
