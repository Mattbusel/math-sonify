use crate::config::*;

#[derive(Clone)]
pub struct Scene {
    pub name: String,
    pub config: Config,
    pub hold_secs: f32,    // how long to stay at this scene's params
    pub morph_secs: f32,   // how long to morph FROM previous scene TO this one
    pub active: bool,
}

impl Scene {
    pub fn empty(n: usize) -> Self {
        Self {
            name: format!("Scene {}", n + 1),
            config: Config::default(),
            hold_secs: 30.0,
            morph_secs: 8.0,
            active: false,
        }
    }
}

/// Linearly interpolate all numeric fields of Config.
/// String fields (system name, mode, scale, chord_mode) switch at t=0.5.
pub fn lerp_config(a: &Config, b: &Config, t: f32) -> Config {
    let t = t.clamp(0.0, 1.0);
    let lf64 = |a: f64, b: f64| -> f64 { a + (b - a) * t as f64 };
    let lf32 = |a: f32, b: f32| -> f32 { a + (b - a) * t };
    let ls   = |a: &str, b: &str| -> String { if t < 0.5 { a.to_string() } else { b.to_string() } };

    Config {
        system: SystemConfig {
            name: ls(&a.system.name, &b.system.name),
            dt: lf64(a.system.dt, b.system.dt),
            speed: lf64(a.system.speed, b.system.speed),
        },
        sonification: SonificationConfig {
            mode:               ls(&a.sonification.mode, &b.sonification.mode),
            scale:              ls(&a.sonification.scale, &b.sonification.scale),
            base_frequency:     lf64(a.sonification.base_frequency, b.sonification.base_frequency),
            octave_range:       lf64(a.sonification.octave_range, b.sonification.octave_range),
            chord_mode:         ls(&a.sonification.chord_mode, &b.sonification.chord_mode),
            transpose_semitones: lf32(a.sonification.transpose_semitones, b.sonification.transpose_semitones),
            portamento_ms:      lf32(a.sonification.portamento_ms, b.sonification.portamento_ms),
            voice_levels:       std::array::from_fn(|i| lf32(a.sonification.voice_levels[i], b.sonification.voice_levels[i])),
            voice_shapes:       if t < 0.5 { a.sonification.voice_shapes.clone() } else { b.sonification.voice_shapes.clone() },
        },
        audio: AudioConfig {
            sample_rate:      a.audio.sample_rate,
            buffer_size:      a.audio.buffer_size,
            reverb_wet:       lf32(a.audio.reverb_wet,      b.audio.reverb_wet),
            delay_ms:         lf32(a.audio.delay_ms,        b.audio.delay_ms),
            delay_feedback:   lf32(a.audio.delay_feedback,  b.audio.delay_feedback),
            master_volume:    lf32(a.audio.master_volume,   b.audio.master_volume),
            bit_depth:        lf32(a.audio.bit_depth,       b.audio.bit_depth),
            rate_crush:       lf32(a.audio.rate_crush,      b.audio.rate_crush),
            chorus_mix:       lf32(a.audio.chorus_mix,      b.audio.chorus_mix),
            chorus_rate:      lf32(a.audio.chorus_rate,     b.audio.chorus_rate),
            chorus_depth:     lf32(a.audio.chorus_depth,    b.audio.chorus_depth),
            waveshaper_drive: lf32(a.audio.waveshaper_drive, b.audio.waveshaper_drive),
            waveshaper_mix:   lf32(a.audio.waveshaper_mix,  b.audio.waveshaper_mix),
        },
        lorenz:          LorenzConfig { sigma: lf64(a.lorenz.sigma, b.lorenz.sigma), rho: lf64(a.lorenz.rho, b.lorenz.rho), beta: lf64(a.lorenz.beta, b.lorenz.beta) },
        rossler:         RosslerConfig { a: lf64(a.rossler.a, b.rossler.a), b: lf64(a.rossler.b, b.rossler.b), c: lf64(a.rossler.c, b.rossler.c) },
        double_pendulum: DoublePendulumConfig { m1: lf64(a.double_pendulum.m1, b.double_pendulum.m1), m2: lf64(a.double_pendulum.m2, b.double_pendulum.m2), l1: lf64(a.double_pendulum.l1, b.double_pendulum.l1), l2: lf64(a.double_pendulum.l2, b.double_pendulum.l2) },
        geodesic_torus:  GeodesicTorusConfig { big_r: lf64(a.geodesic_torus.big_r, b.geodesic_torus.big_r), r: lf64(a.geodesic_torus.r, b.geodesic_torus.r) },
        kuramoto:        KuramotoConfig { n_oscillators: a.kuramoto.n_oscillators, coupling: lf64(a.kuramoto.coupling, b.kuramoto.coupling) },
        duffing:         DuffingConfig { delta: lf64(a.duffing.delta, b.duffing.delta), alpha: lf64(a.duffing.alpha, b.duffing.alpha), beta: lf64(a.duffing.beta, b.duffing.beta), gamma: lf64(a.duffing.gamma, b.duffing.gamma), omega: lf64(a.duffing.omega, b.duffing.omega) },
        van_der_pol:     VanDerPolConfig { mu: lf64(a.van_der_pol.mu, b.van_der_pol.mu) },
        halvorsen:       HalvorsenConfig { a: lf64(a.halvorsen.a, b.halvorsen.a) },
        aizawa:          AizawaConfig { a: lf64(a.aizawa.a, b.aizawa.a), b: lf64(a.aizawa.b, b.aizawa.b), c: lf64(a.aizawa.c, b.aizawa.c), d: lf64(a.aizawa.d, b.aizawa.d), e: lf64(a.aizawa.e, b.aizawa.e), f: lf64(a.aizawa.f, b.aizawa.f) },
        chua:            ChuaConfig { alpha: lf64(a.chua.alpha, b.chua.alpha), beta: lf64(a.chua.beta, b.chua.beta), m0: lf64(a.chua.m0, b.chua.m0), m1: lf64(a.chua.m1, b.chua.m1) },
        viz:             a.viz.clone(), // don't morph viz settings
    }
}

/// Total arrangement duration in seconds (sum of active scenes' hold + morph times).
pub fn total_duration(scenes: &[Scene]) -> f32 {
    let active: Vec<usize> = (0..scenes.len()).filter(|&i| scenes[i].active).collect();
    active.iter().enumerate().map(|(ord, &idx)| {
        let s = &scenes[idx];
        let morph = if ord > 0 { s.morph_secs } else { 0.0 };
        morph + s.hold_secs
    }).sum()
}

/// Elapsed position in arrangement -> (scene_index, phase, t)
/// phase: true = morphing into scene_index, false = holding at scene_index
pub fn scene_at(scenes: &[Scene], elapsed: f32) -> Option<(usize, bool, f32)> {
    let active: Vec<usize> = (0..scenes.len()).filter(|&i| scenes[i].active).collect();
    if active.is_empty() { return None; }
    let mut t = elapsed;
    for (ord, &idx) in active.iter().enumerate() {
        let scene = &scenes[idx];
        // Morph phase first (transition INTO this scene FROM previous), skip for first scene
        if ord > 0 {
            if t < scene.morph_secs {
                return Some((idx, true, t / scene.morph_secs.max(0.001)));
            }
            t -= scene.morph_secs;
        }
        if t < scene.hold_secs {
            return Some((idx, false, t / scene.hold_secs.max(0.001)));
        }
        t -= scene.hold_secs;
    }
    None // past end
}
