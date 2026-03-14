use serde::{Deserialize, Serialize};
use crate::sonification::{SonifMode, Scale};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub system: SystemConfig,
    pub sonification: SonificationConfig,
    pub audio: AudioConfig,
    pub lorenz: LorenzConfig,
    pub rossler: RosslerConfig,
    pub double_pendulum: DoublePendulumConfig,
    pub geodesic_torus: GeodesicTorusConfig,
    pub kuramoto: KuramotoConfig,
    pub viz: VizConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            system: SystemConfig::default(),
            sonification: SonificationConfig::default(),
            audio: AudioConfig::default(),
            lorenz: LorenzConfig::default(),
            rossler: RosslerConfig::default(),
            double_pendulum: DoublePendulumConfig::default(),
            geodesic_torus: GeodesicTorusConfig::default(),
            kuramoto: KuramotoConfig::default(),
            viz: VizConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct VizConfig {
    pub trail_length: usize,
    pub projection: String,
    pub glow: bool,
    pub theme: String,
}

impl Default for VizConfig {
    fn default() -> Self {
        Self { trail_length: 800, projection: "xy".into(), glow: true, theme: "neon".into() }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SystemConfig {
    pub name: String,
    pub dt: f64,
    pub speed: f64,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self { name: "lorenz".into(), dt: 0.001, speed: 1.0 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SonificationConfig {
    pub mode: String,
    pub scale: String,
    pub base_frequency: f64,
    pub octave_range: f64,
    pub transpose_semitones: f32,
    pub chord_mode: String,
    pub voice_levels: [f32; 4],
    pub portamento_ms: f32,
    pub voice_shapes: [String; 4],
}

impl Default for SonificationConfig {
    fn default() -> Self {
        Self {
            mode: "direct".into(),
            scale: "pentatonic".into(),
            base_frequency: 220.0,
            octave_range: 3.0,
            transpose_semitones: 0.0,
            chord_mode: "none".into(),
            voice_levels: [1.0, 0.8, 0.6, 0.4],
            portamento_ms: 80.0,
            voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub reverb_wet: f32,
    pub delay_ms: f32,
    pub delay_feedback: f32,
    pub master_volume: f32,
    pub bit_depth: f32,
    pub rate_crush: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            buffer_size: 512,
            reverb_wet: 0.4,
            delay_ms: 300.0,
            delay_feedback: 0.3,
            master_volume: 0.7,
            bit_depth: 16.0,
            rate_crush: 0.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LorenzConfig {
    pub sigma: f64,
    pub rho: f64,
    pub beta: f64,
}
impl Default for LorenzConfig {
    fn default() -> Self { Self { sigma: 10.0, rho: 28.0, beta: 2.6667 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct RosslerConfig {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}
impl Default for RosslerConfig {
    fn default() -> Self { Self { a: 0.2, b: 0.2, c: 5.7 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DoublePendulumConfig {
    pub m1: f64, pub m2: f64,
    pub l1: f64, pub l2: f64,
}
impl Default for DoublePendulumConfig {
    fn default() -> Self { Self { m1: 1.0, m2: 1.0, l1: 1.0, l2: 1.0 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GeodesicTorusConfig {
    #[serde(rename = "R")]
    pub big_r: f64,
    pub r: f64,
}
impl Default for GeodesicTorusConfig {
    fn default() -> Self { Self { big_r: 3.0, r: 1.0 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KuramotoConfig {
    pub n_oscillators: usize,
    pub coupling: f64,
}
impl Default for KuramotoConfig {
    fn default() -> Self { Self { n_oscillators: 8, coupling: 1.5 } }
}

// --- Conversions from string config to enums ---

impl From<&str> for SonifMode {
    fn from(s: &str) -> Self {
        match s { "orbital" => Self::Orbital, "granular" => Self::Granular,
                  "spectral" => Self::Spectral, "fm" => Self::FM, _ => Self::Direct }
    }
}

impl From<String> for SonifMode {
    fn from(s: String) -> Self { Self::from(s.as_str()) }
}

impl From<&str> for Scale {
    fn from(s: &str) -> Self {
        match s { "chromatic" => Self::Chromatic, "just_intonation" => Self::JustIntonation,
                  "microtonal" => Self::Microtonal, _ => Self::Pentatonic }
    }
}

impl From<String> for Scale {
    fn from(s: String) -> Self { Self::from(s.as_str()) }
}

pub fn load_config(path: &std::path::Path) -> Config {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Config parse error: {e}. Using defaults.");
            Config::default()
        }),
        Err(_) => {
            log::info!("No config.toml found, using defaults.");
            Config::default()
        }
    }
}
