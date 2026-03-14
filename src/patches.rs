use crate::config::*;
use egui::Color32;

pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub color: egui::Color32,
}

pub const PRESETS: &[Preset] = &[
    Preset { name: "Lorenz Ambience", description: "Slow drift through the butterfly attractor", color: Color32::from_rgb(0, 160, 220) },
    Preset { name: "Pendulum Rhythm", description: "Chaotic pendulum drives granular pulses", color: Color32::from_rgb(220, 120, 0) },
    Preset { name: "Torus Drone", description: "Ergodic geodesic flow, major chord, deep reverb", color: Color32::from_rgb(140, 80, 220) },
    Preset { name: "Kuramoto Sync", description: "Watch synchronization emerge as you raise coupling", color: Color32::from_rgb(0, 200, 100) },
    Preset { name: "Three-Body Jazz", description: "Figure-8 orbit, dom7 chord, spectral mode", color: Color32::from_rgb(200, 170, 0) },
    Preset { name: "Rössler Drift", description: "Gentle spiral attractor, microtonal scale", color: Color32::from_rgb(220, 80, 140) },
    Preset { name: "FM Chaos", description: "Frequency modulation driven by the butterfly attractor", color: Color32::from_rgb(220, 40, 40) },
    Preset { name: "Pendulum Meditation", description: "Slow pendulum drift through pure harmonic ratios", color: Color32::from_rgb(0, 180, 160) },
    Preset { name: "Duffing Rhythm", description: "Period-doubling chaos — rhythmic clicking and pulsing", color: Color32::from_rgb(200, 100, 50) },
    Preset { name: "Chua Grit", description: "Electronic double-scroll — raw gritty harmonic buzz", color: Color32::from_rgb(180, 30, 200) },
    Preset { name: "Halvorsen Spiral", description: "Dense spiral attractor — layered harmonic drifts", color: Color32::from_rgb(50, 150, 220) },
];

pub fn load_preset(name: &str) -> Config {
    match name {
        "Lorenz Ambience" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 1.0 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 110.0, octave_range: 2.5,
                chord_mode: "major".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3], portamento_ms: 300.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
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
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
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
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
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
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
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
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
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
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
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
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
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
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig { reverb_wet: 0.75, delay_ms: 700.0, delay_feedback: 0.3, master_volume: 0.65, ..Default::default() },
            ..Default::default()
        },
        "Duffing Rhythm" => Config {
            system: SystemConfig { name: "duffing".into(), dt: 0.001, speed: 2.0 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 220.0, octave_range: 2.0,
                chord_mode: "power".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4], portamento_ms: 50.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig { reverb_wet: 0.0, delay_ms: 200.0, delay_feedback: 0.3, master_volume: 0.75, ..Default::default() },
            duffing: DuffingConfig { delta: 0.3, alpha: -1.0, beta: 1.0, gamma: 0.5, omega: 1.2 },
            ..Default::default()
        },
        "Chua Grit" => Config {
            system: SystemConfig { name: "chua".into(), dt: 0.0005, speed: 1.0 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "chromatic".into(),
                base_frequency: 110.0, octave_range: 3.0,
                chord_mode: "dom7".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.7, 0.5], portamento_ms: 80.0,
                voice_shapes: ["saw".into(), "saw".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig { reverb_wet: 0.45, delay_ms: 300.0, delay_feedback: 0.4, master_volume: 0.7, ..Default::default() },
            chua: ChuaConfig { alpha: 15.6, beta: 28.0, m0: -1.143, m1: -0.714 },
            ..Default::default()
        },
        "Halvorsen Spiral" => Config {
            system: SystemConfig { name: "halvorsen".into(), dt: 0.001, speed: 0.8 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 110.0, octave_range: 3.5,
                chord_mode: "major".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4], portamento_ms: 200.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig { reverb_wet: 0.6, delay_ms: 400.0, delay_feedback: 0.35, master_volume: 0.7, ..Default::default() },
            halvorsen: HalvorsenConfig { a: 1.89 },
            ..Default::default()
        },
        _ => Config::default(),
    }
}

/// Save a named patch to the patches/ directory.
pub fn save_patch(name: &str, config: &Config) {
    let dir = std::path::PathBuf::from("patches");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    let filename = dir.join(format!("{}.toml", sanitize_name(name)));
    let toml_str = toml::to_string_pretty(config).unwrap_or_default();
    let _ = std::fs::write(&filename, toml_str);
}

/// List all saved patch names (without .toml extension).
pub fn list_patches() -> Vec<String> {
    let dir = std::path::PathBuf::from("patches");
    if !dir.exists() { return Vec::new(); }
    match std::fs::read_dir(&dir) {
        Ok(entries) => {
            let mut names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().and_then(|x| x.to_str()) == Some("toml") {
                        path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();
            names
        }
        Err(_) => Vec::new(),
    }
}

/// Load a patch by name from the patches/ directory.
pub fn load_patch_file(name: &str) -> Option<Config> {
    let filename = std::path::PathBuf::from("patches").join(format!("{}.toml", sanitize_name(name)));
    match std::fs::read_to_string(&filename) {
        Ok(text) => toml::from_str(&text).ok(),
        Err(_) => None,
    }
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
        .collect::<String>()
        .replace(' ', "_")
}
