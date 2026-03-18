// Config types are used in the binary but appear unused in the lib (plugin) build context.
#![allow(dead_code)]

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
    pub duffing: DuffingConfig,
    pub van_der_pol: VanDerPolConfig,
    pub halvorsen: HalvorsenConfig,
    pub aizawa: AizawaConfig,
    pub chua: ChuaConfig,
    pub hindmarsh_rose: HindmarshRoseConfig,
    pub coupled_map_lattice: CmlConfig,
    pub mackey_glass: MackeyGlassConfig,
    pub nose_hoover: NoseHooverConfig,
    pub henon_map: HenonMapConfig,
    pub lorenz96: Lorenz96Config,
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
            duffing: DuffingConfig::default(),
            van_der_pol: VanDerPolConfig::default(),
            halvorsen: HalvorsenConfig::default(),
            aizawa: AizawaConfig::default(),
            chua: ChuaConfig::default(),
            hindmarsh_rose: HindmarshRoseConfig::default(),
            coupled_map_lattice: CmlConfig::default(),
            mackey_glass: MackeyGlassConfig::default(),
            nose_hoover: NoseHooverConfig::default(),
            henon_map: HenonMapConfig::default(),
            lorenz96: Lorenz96Config::default(),
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
    pub chorus_mix: f32,
    pub chorus_rate: f32,
    pub chorus_depth: f32,
    pub waveshaper_drive: f32,
    pub waveshaper_mix: f32,
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
            chorus_mix: 0.0,
            chorus_rate: 0.5,
            chorus_depth: 3.0,
            waveshaper_drive: 1.0,
            waveshaper_mix: 0.0,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DuffingConfig {
    pub delta: f64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub omega: f64,
}
impl Default for DuffingConfig {
    fn default() -> Self { Self { delta: 0.3, alpha: -1.0, beta: 1.0, gamma: 0.5, omega: 1.2 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct VanDerPolConfig {
    pub mu: f64,
}
impl Default for VanDerPolConfig {
    fn default() -> Self { Self { mu: 2.0 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct HalvorsenConfig {
    pub a: f64,
}
impl Default for HalvorsenConfig {
    fn default() -> Self { Self { a: 1.89 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AizawaConfig {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}
impl Default for AizawaConfig {
    fn default() -> Self { Self { a: 0.95, b: 0.7, c: 0.6, d: 3.5, e: 0.25, f: 0.1 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ChuaConfig {
    pub alpha: f64,
    pub beta: f64,
    pub m0: f64,
    pub m1: f64,
}
impl Default for ChuaConfig {
    fn default() -> Self { Self { alpha: 15.6, beta: 28.0, m0: -1.143, m1: -0.714 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct HindmarshRoseConfig {
    pub current_i: f64, // external drive current — main control parameter
    pub r: f64,         // slow adaptation timescale
}
impl Default for HindmarshRoseConfig {
    fn default() -> Self { Self { current_i: 3.0, r: 0.006 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CmlConfig {
    pub r: f64,   // logistic growth rate (3.7–4.0 for chaos)
    pub eps: f64, // coupling strength (0=independent, 1=synchrony)
}
impl Default for CmlConfig {
    fn default() -> Self { Self { r: 3.9, eps: 0.35 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct MackeyGlassConfig {
    pub beta: f64,
    pub gamma: f64,
    pub tau: f64,
    pub n: f64,
}
impl Default for MackeyGlassConfig {
    fn default() -> Self { Self { beta: 0.2, gamma: 0.1, tau: 17.0, n: 10.0 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct NoseHooverConfig {
    pub a: f64,
}
impl Default for NoseHooverConfig {
    fn default() -> Self { Self { a: 3.0 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct HenonMapConfig {
    pub a: f64,
    pub b: f64,
}
impl Default for HenonMapConfig {
    fn default() -> Self { Self { a: 1.4, b: 0.3 } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Lorenz96Config {
    pub f: f64,
}
impl Default for Lorenz96Config {
    fn default() -> Self { Self { f: 8.0 } }
}

impl Config {
    /// Clamp all parameters to physically sensible bounds.
    /// Call this after deserializing from user-supplied config files.
    pub fn validate(&mut self) {
        // System
        self.system.dt = self.system.dt.clamp(0.0001, 0.1);
        self.system.speed = self.system.speed.clamp(0.0, 100.0);

        // Lorenz
        self.lorenz.sigma = self.lorenz.sigma.clamp(0.1, 100.0);
        self.lorenz.rho   = self.lorenz.rho.clamp(0.1, 200.0);
        self.lorenz.beta  = self.lorenz.beta.clamp(0.01, 20.0);

        // Rossler
        self.rossler.a = self.rossler.a.clamp(0.0, 20.0);
        self.rossler.b = self.rossler.b.clamp(0.0, 20.0);
        self.rossler.c = self.rossler.c.clamp(0.0, 20.0);

        // Audio
        self.audio.reverb_wet     = self.audio.reverb_wet.clamp(0.0, 1.0);
        self.audio.delay_ms       = self.audio.delay_ms.clamp(1.0, 5000.0);
        self.audio.delay_feedback = self.audio.delay_feedback.clamp(0.0, 0.99);
        self.audio.master_volume  = self.audio.master_volume.clamp(0.0, 1.0);
        if self.audio.sample_rate != 44100 && self.audio.sample_rate != 48000 {
            self.audio.sample_rate = 44100;
        }
        self.audio.chorus_mix         = self.audio.chorus_mix.clamp(0.0, 1.0);
        self.audio.chorus_rate        = self.audio.chorus_rate.clamp(0.01, 20.0);
        self.audio.chorus_depth       = self.audio.chorus_depth.clamp(0.0, 50.0);
        self.audio.waveshaper_drive   = self.audio.waveshaper_drive.clamp(0.0, 100.0);
        self.audio.waveshaper_mix     = self.audio.waveshaper_mix.clamp(0.0, 1.0);
        self.audio.rate_crush         = self.audio.rate_crush.clamp(0.0, 1.0);
        self.audio.bit_depth          = self.audio.bit_depth.clamp(1.0, 32.0);

        // Sonification
        self.sonification.base_frequency  = self.sonification.base_frequency.clamp(20.0, 2000.0);
        self.sonification.octave_range    = self.sonification.octave_range.clamp(0.1, 8.0);
        self.sonification.portamento_ms   = self.sonification.portamento_ms.clamp(1.0, 5000.0);
        for v in &mut self.sonification.voice_levels {
            *v = v.clamp(0.0, 1.0);
        }

        // Double pendulum – lengths and masses must be positive
        self.double_pendulum.m1 = self.double_pendulum.m1.clamp(0.01, 100.0);
        self.double_pendulum.m2 = self.double_pendulum.m2.clamp(0.01, 100.0);
        self.double_pendulum.l1 = self.double_pendulum.l1.clamp(0.01, 100.0);
        self.double_pendulum.l2 = self.double_pendulum.l2.clamp(0.01, 100.0);

        // Geodesic torus – radii must be positive
        self.geodesic_torus.big_r = self.geodesic_torus.big_r.clamp(0.1, 100.0);
        self.geodesic_torus.r     = self.geodesic_torus.r.clamp(0.01, 50.0);

        // Kuramoto
        self.kuramoto.n_oscillators = self.kuramoto.n_oscillators.max(2).min(256);
        self.kuramoto.coupling      = self.kuramoto.coupling.clamp(0.0, 50.0);

        // Duffing
        self.duffing.delta = self.duffing.delta.clamp(0.0, 10.0);
        self.duffing.gamma = self.duffing.gamma.clamp(0.0, 10.0);
        self.duffing.omega = self.duffing.omega.clamp(0.001, 100.0);

        // Van der Pol
        self.van_der_pol.mu = self.van_der_pol.mu.clamp(0.0, 100.0);

        // Halvorsen
        self.halvorsen.a = self.halvorsen.a.clamp(0.0, 10.0);

        // Aizawa
        self.aizawa.a = self.aizawa.a.clamp(0.0, 5.0);
        self.aizawa.b = self.aizawa.b.clamp(0.0, 5.0);
        self.aizawa.c = self.aizawa.c.clamp(0.0, 5.0);
        self.aizawa.d = self.aizawa.d.clamp(0.0, 10.0);
        self.aizawa.e = self.aizawa.e.clamp(0.0, 5.0);
        self.aizawa.f = self.aizawa.f.clamp(0.0, 5.0);

        // Chua
        self.chua.alpha = self.chua.alpha.clamp(0.0, 100.0);
        self.chua.beta  = self.chua.beta.clamp(0.0, 100.0);

        // Hindmarsh-Rose
        self.hindmarsh_rose.current_i = self.hindmarsh_rose.current_i.clamp(-5.0, 10.0);
        self.hindmarsh_rose.r         = self.hindmarsh_rose.r.clamp(1e-6, 1.0);

        // CML
        self.coupled_map_lattice.r   = self.coupled_map_lattice.r.clamp(0.0, 4.0);
        self.coupled_map_lattice.eps = self.coupled_map_lattice.eps.clamp(0.0, 1.0);

        // Mackey-Glass
        self.mackey_glass.beta  = self.mackey_glass.beta.clamp(0.0, 10.0);
        self.mackey_glass.gamma = self.mackey_glass.gamma.clamp(0.0, 10.0);
        self.mackey_glass.tau   = self.mackey_glass.tau.clamp(1.0, 300.0);
        self.mackey_glass.n     = self.mackey_glass.n.clamp(1.0, 20.0);

        // Nose-Hoover
        self.nose_hoover.a = self.nose_hoover.a.clamp(0.1, 20.0);

        // Henon map
        self.henon_map.a = self.henon_map.a.clamp(0.0, 2.0);
        self.henon_map.b = self.henon_map.b.clamp(-1.0, 1.0);

        // Lorenz96
        self.lorenz96.f = self.lorenz96.f.clamp(0.0, 50.0);
    }
}

// --- Conversions from string config to enums ---

impl From<&str> for SonifMode {
    fn from(s: &str) -> Self {
        match s { "orbital" => Self::Orbital, "granular" => Self::Granular,
                  "spectral" => Self::Spectral, "fm" => Self::FM,
                  "vocal" => Self::Vocal, "waveguide" => Self::Waveguide,
                  _ => Self::Direct }
    }
}

impl From<String> for SonifMode {
    fn from(s: String) -> Self { Self::from(s.as_str()) }
}

impl From<&str> for Scale {
    fn from(s: &str) -> Self {
        match s {
            "chromatic"       => Self::Chromatic,
            "just_intonation" => Self::JustIntonation,
            "microtonal"      => Self::Microtonal,
            "edo19"           => Self::Edo19,
            "edo31"           => Self::Edo31,
            "edo24"           => Self::Edo24,
            "whole_tone"      => Self::WholeTone,
            "phrygian"        => Self::Phrygian,
            "lydian"          => Self::Lydian,
            _                 => Self::Pentatonic,
        }
    }
}

impl From<String> for Scale {
    fn from(s: String) -> Self { Self::from(s.as_str()) }
}

pub fn load_config(path: &std::path::Path) -> Config {
    let mut config = match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Config parse error: {e}. Using defaults.");
            Config::default()
        }),
        Err(_) => {
            log::info!("No config.toml found, using defaults.");
            Config::default()
        }
    };
    config.validate();
    config
}
