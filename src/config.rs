// Config types are used in the binary but appear unused in the lib (plugin) build context.
#![allow(dead_code)]

use crate::sonification::{Scale, SonifMode};
use serde::{Deserialize, Serialize};

/// Top-level application configuration, loaded from `config.toml`.
///
/// All fields have defaults via [`Default`]; call [`Config::validate`] after
/// loading from disk to clamp every value to a physically sensible range.
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
    pub logistic_map: LogisticMapConfig,
    pub standard_map: StandardMapConfig,
    pub stochastic_lorenz: StochasticLorenzConfig,
    pub delayed_map: DelayedMapConfig,
    pub oregonator: OregonatorConfig,
    pub mathieu: MathieuConfig,
    pub kuramoto_driven: KuramotoDrivenConfig,
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
            logistic_map: LogisticMapConfig::default(),
            standard_map: StandardMapConfig::default(),
            stochastic_lorenz: StochasticLorenzConfig::default(),
            delayed_map: DelayedMapConfig::default(),
            oregonator: OregonatorConfig::default(),
            mathieu: MathieuConfig::default(),
            kuramoto_driven: KuramotoDrivenConfig::default(),
        }
    }
}

/// Phase-portrait visualization settings.
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
        Self {
            trail_length: 800,
            projection: "xy".into(),
            glow: true,
            theme: "neon".into(),
        }
    }
}

/// Dynamical system selection and integration parameters.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SystemConfig {
    pub name: String,
    pub dt: f64,
    pub speed: f64,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            name: "lorenz".into(),
            dt: 0.001,
            speed: 1.0,
        }
    }
}

/// Sonification mapping parameters: mode, scale, pitch, and per-voice settings.
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

/// Audio engine and effects chain parameters.
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
    /// Simulation control rate in Hz. Default 120 Hz. Range: [30, 960].
    pub control_rate_hz: f64,
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
            control_rate_hz: 120.0,
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
    fn default() -> Self {
        Self {
            sigma: 10.0,
            rho: 28.0,
            beta: 2.6667,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct RosslerConfig {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}
impl Default for RosslerConfig {
    fn default() -> Self {
        Self {
            a: 0.2,
            b: 0.2,
            c: 5.7,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DoublePendulumConfig {
    pub m1: f64,
    pub m2: f64,
    pub l1: f64,
    pub l2: f64,
}
impl Default for DoublePendulumConfig {
    fn default() -> Self {
        Self {
            m1: 1.0,
            m2: 1.0,
            l1: 1.0,
            l2: 1.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GeodesicTorusConfig {
    #[serde(rename = "R")]
    pub big_r: f64,
    pub r: f64,
}
impl Default for GeodesicTorusConfig {
    fn default() -> Self {
        Self { big_r: 3.0, r: 1.0 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KuramotoConfig {
    pub n_oscillators: usize,
    pub coupling: f64,
}
impl Default for KuramotoConfig {
    fn default() -> Self {
        Self {
            n_oscillators: 8,
            coupling: 1.5,
        }
    }
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
    fn default() -> Self {
        Self {
            delta: 0.3,
            alpha: -1.0,
            beta: 1.0,
            gamma: 0.5,
            omega: 1.2,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct VanDerPolConfig {
    pub mu: f64,
}
impl Default for VanDerPolConfig {
    fn default() -> Self {
        Self { mu: 2.0 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct HalvorsenConfig {
    pub a: f64,
}
impl Default for HalvorsenConfig {
    fn default() -> Self {
        Self { a: 1.89 }
    }
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
    fn default() -> Self {
        Self {
            a: 0.95,
            b: 0.7,
            c: 0.6,
            d: 3.5,
            e: 0.25,
            f: 0.1,
        }
    }
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
    fn default() -> Self {
        Self {
            alpha: 15.6,
            beta: 28.0,
            m0: -1.143,
            m1: -0.714,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct HindmarshRoseConfig {
    pub current_i: f64, // external drive current — main control parameter
    pub r: f64,         // slow adaptation timescale
}
impl Default for HindmarshRoseConfig {
    fn default() -> Self {
        Self {
            current_i: 3.0,
            r: 0.006,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CmlConfig {
    pub r: f64,   // logistic growth rate (3.7–4.0 for chaos)
    pub eps: f64, // coupling strength (0=independent, 1=synchrony)
}
impl Default for CmlConfig {
    fn default() -> Self {
        Self { r: 3.9, eps: 0.35 }
    }
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
    fn default() -> Self {
        Self {
            beta: 0.2,
            gamma: 0.1,
            tau: 17.0,
            n: 10.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct NoseHooverConfig {
    pub a: f64,
}
impl Default for NoseHooverConfig {
    fn default() -> Self {
        Self { a: 3.0 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct HenonMapConfig {
    pub a: f64,
    pub b: f64,
}
impl Default for HenonMapConfig {
    fn default() -> Self {
        Self { a: 1.4, b: 0.3 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Lorenz96Config {
    pub f: f64,
}
impl Default for Lorenz96Config {
    fn default() -> Self {
        Self { f: 8.0 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LogisticMapConfig {
    /// Bifurcation parameter r. Chaotic regime: r > 3.57. Default: 3.9.
    pub r: f64,
}
impl Default for LogisticMapConfig {
    fn default() -> Self {
        Self { r: 3.9 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct StandardMapConfig {
    /// Stochasticity parameter k. Global chaos for k > 0.97. Default: 1.5.
    pub k: f64,
}
impl Default for StandardMapConfig {
    fn default() -> Self {
        Self { k: 1.5 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct StochasticLorenzConfig {
    pub sigma: f64,
    pub rho: f64,
    pub beta: f64,
    pub noise_strength: f64,
}
impl Default for StochasticLorenzConfig {
    fn default() -> Self {
        Self { sigma: 10.0, rho: 28.0, beta: 2.6667, noise_strength: 0.5 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DelayedMapConfig {
    pub r: f64,
    pub tau: usize,
}
impl Default for DelayedMapConfig {
    fn default() -> Self {
        Self { r: 3.9, tau: 5 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct OregonatorConfig {
    pub f: f64,
}
impl Default for OregonatorConfig {
    fn default() -> Self {
        Self { f: 1.0 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct MathieuConfig {
    pub a: f64,
    pub q: f64,
}
impl Default for MathieuConfig {
    fn default() -> Self {
        Self { a: 0.0, q: 0.5 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KuramotoDrivenConfig {
    pub coupling: f64,
    pub drive_amp: f64,
    pub drive_freq: f64,
}
impl Default for KuramotoDrivenConfig {
    fn default() -> Self {
        Self { coupling: 1.0, drive_amp: 0.5, drive_freq: 1.2 }
    }
}

impl Config {
    /// Clamp all parameters to physically sensible bounds.
    /// Call this after deserializing from user-supplied config files.
    pub fn validate(&mut self) {
        // System
        Self::clamp_log_f64(&mut self.system.dt, 0.0001, 0.1, "system.dt");
        Self::clamp_log_f64(&mut self.system.speed, 0.0, 100.0, "system.speed");

        // Lorenz
        Self::clamp_log_f64(&mut self.lorenz.sigma, 0.1, 100.0, "lorenz.sigma");
        Self::clamp_log_f64(&mut self.lorenz.rho, 0.1, 200.0, "lorenz.rho");
        Self::clamp_log_f64(&mut self.lorenz.beta, 0.01, 20.0, "lorenz.beta");

        // Rossler
        Self::clamp_log_f64(&mut self.rossler.a, 0.0, 20.0, "rossler.a");
        Self::clamp_log_f64(&mut self.rossler.b, 0.0, 20.0, "rossler.b");
        Self::clamp_log_f64(&mut self.rossler.c, 0.0, 20.0, "rossler.c");

        // Audio
        Self::clamp_log_f32(&mut self.audio.reverb_wet, 0.0, 1.0, "audio.reverb_wet");
        Self::clamp_log_f32(&mut self.audio.delay_ms, 1.0, 5000.0, "audio.delay_ms");
        Self::clamp_log_f32(
            &mut self.audio.delay_feedback,
            0.0,
            0.99,
            "audio.delay_feedback",
        );
        Self::clamp_log_f32(
            &mut self.audio.master_volume,
            0.0,
            1.0,
            "audio.master_volume",
        );
        if self.audio.sample_rate != 44100 && self.audio.sample_rate != 48000 {
            tracing::warn!(
                field = "audio.sample_rate",
                value = self.audio.sample_rate as f64,
                min = 44100.0_f64,
                max = 48000.0_f64,
                "config value clamped to valid range"
            );
            self.audio.sample_rate = 44100;
        }
        Self::clamp_log_f32(&mut self.audio.chorus_mix, 0.0, 1.0, "audio.chorus_mix");
        Self::clamp_log_f32(&mut self.audio.chorus_rate, 0.01, 20.0, "audio.chorus_rate");
        Self::clamp_log_f32(
            &mut self.audio.chorus_depth,
            0.0,
            50.0,
            "audio.chorus_depth",
        );
        Self::clamp_log_f32(
            &mut self.audio.waveshaper_drive,
            0.0,
            100.0,
            "audio.waveshaper_drive",
        );
        Self::clamp_log_f32(
            &mut self.audio.waveshaper_mix,
            0.0,
            1.0,
            "audio.waveshaper_mix",
        );
        Self::clamp_log_f32(&mut self.audio.rate_crush, 0.0, 1.0, "audio.rate_crush");
        Self::clamp_log_f32(&mut self.audio.bit_depth, 1.0, 32.0, "audio.bit_depth");
        Self::clamp_log_f64(
            &mut self.audio.control_rate_hz,
            30.0,
            960.0,
            "audio.control_rate_hz",
        );

        // Sonification
        Self::clamp_log_f64(
            &mut self.sonification.base_frequency,
            20.0,
            2000.0,
            "sonification.base_frequency",
        );
        Self::clamp_log_f64(
            &mut self.sonification.octave_range,
            0.1,
            8.0,
            "sonification.octave_range",
        );
        Self::clamp_log_f32(
            &mut self.sonification.portamento_ms,
            1.0,
            5000.0,
            "sonification.portamento_ms",
        );
        for v in &mut self.sonification.voice_levels {
            let old = *v;
            *v = v.clamp(0.0, 1.0);
            if (*v - old).abs() > 1e-9 {
                tracing::warn!(
                    field = "sonification.voice_levels",
                    value = old as f64,
                    min = 0.0_f64,
                    max = 1.0_f64,
                    "config value clamped to valid range"
                );
            }
        }

        // Double pendulum – lengths and masses must be positive
        Self::clamp_log_f64(
            &mut self.double_pendulum.m1,
            0.01,
            100.0,
            "double_pendulum.m1",
        );
        Self::clamp_log_f64(
            &mut self.double_pendulum.m2,
            0.01,
            100.0,
            "double_pendulum.m2",
        );
        Self::clamp_log_f64(
            &mut self.double_pendulum.l1,
            0.01,
            100.0,
            "double_pendulum.l1",
        );
        Self::clamp_log_f64(
            &mut self.double_pendulum.l2,
            0.01,
            100.0,
            "double_pendulum.l2",
        );

        // Geodesic torus – radii must be positive
        Self::clamp_log_f64(
            &mut self.geodesic_torus.big_r,
            0.1,
            100.0,
            "geodesic_torus.big_r",
        );
        Self::clamp_log_f64(&mut self.geodesic_torus.r, 0.01, 50.0, "geodesic_torus.r");

        // Kuramoto
        let old_n = self.kuramoto.n_oscillators;
        self.kuramoto.n_oscillators = self.kuramoto.n_oscillators.max(2).min(256);
        if self.kuramoto.n_oscillators != old_n {
            tracing::warn!(
                field = "kuramoto.n_oscillators",
                value = old_n as f64,
                min = 2.0_f64,
                max = 256.0_f64,
                "config value clamped to valid range"
            );
        }
        Self::clamp_log_f64(&mut self.kuramoto.coupling, 0.0, 50.0, "kuramoto.coupling");

        // Duffing
        Self::clamp_log_f64(&mut self.duffing.delta, 0.0, 10.0, "duffing.delta");
        Self::clamp_log_f64(&mut self.duffing.gamma, 0.0, 10.0, "duffing.gamma");
        Self::clamp_log_f64(&mut self.duffing.omega, 0.001, 100.0, "duffing.omega");

        // Van der Pol
        Self::clamp_log_f64(&mut self.van_der_pol.mu, 0.0, 100.0, "van_der_pol.mu");

        // Halvorsen
        Self::clamp_log_f64(&mut self.halvorsen.a, 0.0, 10.0, "halvorsen.a");

        // Aizawa
        Self::clamp_log_f64(&mut self.aizawa.a, 0.0, 5.0, "aizawa.a");
        Self::clamp_log_f64(&mut self.aizawa.b, 0.0, 5.0, "aizawa.b");
        Self::clamp_log_f64(&mut self.aizawa.c, 0.0, 5.0, "aizawa.c");
        Self::clamp_log_f64(&mut self.aizawa.d, 0.0, 10.0, "aizawa.d");
        Self::clamp_log_f64(&mut self.aizawa.e, 0.0, 5.0, "aizawa.e");
        Self::clamp_log_f64(&mut self.aizawa.f, 0.0, 5.0, "aizawa.f");

        // Chua
        Self::clamp_log_f64(&mut self.chua.alpha, 0.0, 100.0, "chua.alpha");
        Self::clamp_log_f64(&mut self.chua.beta, 0.0, 100.0, "chua.beta");

        // Hindmarsh-Rose
        Self::clamp_log_f64(
            &mut self.hindmarsh_rose.current_i,
            -5.0,
            10.0,
            "hindmarsh_rose.current_i",
        );
        Self::clamp_log_f64(&mut self.hindmarsh_rose.r, 1e-6, 1.0, "hindmarsh_rose.r");

        // CML
        Self::clamp_log_f64(
            &mut self.coupled_map_lattice.r,
            0.0,
            4.0,
            "coupled_map_lattice.r",
        );
        Self::clamp_log_f64(
            &mut self.coupled_map_lattice.eps,
            0.0,
            1.0,
            "coupled_map_lattice.eps",
        );

        // Mackey-Glass
        Self::clamp_log_f64(&mut self.mackey_glass.beta, 0.0, 10.0, "mackey_glass.beta");
        Self::clamp_log_f64(
            &mut self.mackey_glass.gamma,
            0.0,
            10.0,
            "mackey_glass.gamma",
        );
        Self::clamp_log_f64(&mut self.mackey_glass.tau, 1.0, 300.0, "mackey_glass.tau");
        Self::clamp_log_f64(&mut self.mackey_glass.n, 1.0, 20.0, "mackey_glass.n");

        // Nose-Hoover
        Self::clamp_log_f64(&mut self.nose_hoover.a, 0.1, 20.0, "nose_hoover.a");

        // Henon map
        Self::clamp_log_f64(&mut self.henon_map.a, 0.0, 2.0, "henon_map.a");
        Self::clamp_log_f64(&mut self.henon_map.b, -1.0, 1.0, "henon_map.b");

        // Lorenz96
        Self::clamp_log_f64(&mut self.lorenz96.f, 0.0, 50.0, "lorenz96.f");

        // New systems added after initial release
        Self::clamp_log_f64(&mut self.logistic_map.r, 0.0, 4.0, "logistic_map.r");
        Self::clamp_log_f64(&mut self.standard_map.k, 0.0, 20.0, "standard_map.k");
        Self::clamp_log_f64(&mut self.stochastic_lorenz.sigma, 0.1, 100.0, "stochastic_lorenz.sigma");
        Self::clamp_log_f64(&mut self.stochastic_lorenz.rho, 0.1, 200.0, "stochastic_lorenz.rho");
        Self::clamp_log_f64(&mut self.stochastic_lorenz.beta, 0.01, 20.0, "stochastic_lorenz.beta");
        Self::clamp_log_f64(&mut self.stochastic_lorenz.noise_strength, 0.0, 10.0, "stochastic_lorenz.noise_strength");
        Self::clamp_log_f64(&mut self.delayed_map.r, 0.0, 4.0, "delayed_map.r");
        self.delayed_map.tau = self.delayed_map.tau.clamp(1, 50);
        Self::clamp_log_f64(&mut self.oregonator.f, 0.1, 10.0, "oregonator.f");
        Self::clamp_log_f64(&mut self.mathieu.a, -5.0, 5.0, "mathieu.a");
        Self::clamp_log_f64(&mut self.mathieu.q, 0.0, 5.0, "mathieu.q");
        Self::clamp_log_f64(&mut self.kuramoto_driven.coupling, 0.0, 20.0, "kuramoto_driven.coupling");
        Self::clamp_log_f64(&mut self.kuramoto_driven.drive_amp, 0.0, 10.0, "kuramoto_driven.drive_amp");
        Self::clamp_log_f64(&mut self.kuramoto_driven.drive_freq, 0.0, 100.0, "kuramoto_driven.drive_freq");
    }

    /// Clamp a `f64` field to `[min, max]`, emitting a tracing warning if clamped.
    fn clamp_log_f64(value: &mut f64, min: f64, max: f64, field: &'static str) {
        let old = *value;
        *value = old.clamp(min, max);
        if (*value - old).abs() > f64::EPSILON {
            tracing::warn!(
                field = field,
                value = old,
                min = min,
                max = max,
                "config value clamped to valid range"
            );
        }
    }

    /// Clamp a `f32` field to `[min, max]`, emitting a tracing warning if clamped.
    fn clamp_log_f32(value: &mut f32, min: f32, max: f32, field: &'static str) {
        let old = *value;
        *value = old.clamp(min, max);
        if (*value - old).abs() > f32::EPSILON {
            tracing::warn!(
                field = field,
                value = old as f64,
                min = min as f64,
                max = max as f64,
                "config value clamped to valid range"
            );
        }
    }
}

// --- Conversions from string config to enums ---

impl From<&str> for SonifMode {
    fn from(s: &str) -> Self {
        match s {
            "orbital" => Self::Orbital,
            "granular" => Self::Granular,
            "spectral" => Self::Spectral,
            "fm" => Self::FM,
            "vocal" => Self::Vocal,
            "waveguide" => Self::Waveguide,
            _ => Self::Direct,
        }
    }
}

impl From<String> for SonifMode {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl From<&str> for Scale {
    fn from(s: &str) -> Self {
        match s {
            "chromatic" => Self::Chromatic,
            "just_intonation" => Self::JustIntonation,
            "microtonal" => Self::Microtonal,
            "edo19" => Self::Edo19,
            "edo31" => Self::Edo31,
            "edo24" => Self::Edo24,
            "whole_tone" => Self::WholeTone,
            "phrygian" => Self::Phrygian,
            "lydian" => Self::Lydian,
            _ => Self::Pentatonic,
        }
    }
}

impl From<String> for Scale {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

/// Loads a `Config` from a TOML file, falling back to defaults on any error.
///
/// # Parameters
/// - `path`: Path to the TOML configuration file.
///
/// # Returns
/// A validated `Config`; defaults are used for any missing or invalid fields.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_valid() {
        // Default config should survive a validate() call with all values unchanged
        // (i.e. all defaults are already within their valid ranges).
        let mut config = Config::default();
        let before = format!("{:?}", config);
        config.validate();
        let after = format!("{:?}", config);
        assert_eq!(before, after, "validate() mutated a default Config value");
    }

    #[test]
    fn test_config_round_trip() {
        let original = Config::default();
        let serialized = toml::to_string(&original).expect("serialization failed");
        let mut deserialized: Config = toml::from_str(&serialized).expect("deserialization failed");
        deserialized.validate();
        // Compare key scalar fields to verify round-trip fidelity
        assert!((original.lorenz.sigma - deserialized.lorenz.sigma).abs() < 1e-9);
        assert!((original.lorenz.rho - deserialized.lorenz.rho).abs() < 1e-9);
        assert!((original.lorenz.beta - deserialized.lorenz.beta).abs() < 1e-9);
        assert!((original.rossler.a - deserialized.rossler.a).abs() < 1e-9);
        assert!((original.rossler.c - deserialized.rossler.c).abs() < 1e-9);
        assert!(
            (original.audio.master_volume as f64 - deserialized.audio.master_volume as f64).abs()
                < 1e-6
        );
        assert_eq!(original.system.name, deserialized.system.name);
    }
}
