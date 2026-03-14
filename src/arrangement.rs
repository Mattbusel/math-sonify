use crate::config::*;
use crate::patches::load_preset;

#[derive(Clone)]
pub struct Scene {
    pub name: String,
    pub config: Config,
    pub hold_secs: f32,    // how long to stay at this scene's params
    pub morph_secs: f32,   // how long to morph FROM previous scene TO this one
    pub active: bool,
    pub transition_prob: f32,  // relative probability weight for transitioning TO this scene
}

impl Scene {
    pub fn empty(n: usize) -> Self {
        Self {
            name: format!("Scene {}", n + 1),
            config: Config::default(),
            hold_secs: 30.0,
            morph_secs: 8.0,
            active: false,
            transition_prob: 1.0,
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
            // Clamp lerped volume so morph valleys never drop below audible threshold
            master_volume:    lf32(a.audio.master_volume,   b.audio.master_volume).max(0.45),
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

// ---------------------------------------------------------------------------
// Song Generator
// ---------------------------------------------------------------------------
// Philosophy: the MORPH is the music. Short holds (15-20s) establish each
// scene, then long morphs (25-35s) are the actual musical events — two
// attractors simultaneously deforming into each other. The generated
// arrangements are shaped so you spend most of the time in transition.

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*seed >> 33) as f64 / u32::MAX as f64
}

fn make_scene(name: &str, preset: &str, hold: f32, morph: f32, tweaks: impl FnOnce(&mut Config)) -> Scene {
    let mut cfg = load_preset(preset);
    tweaks(&mut cfg);
    Scene { name: name.to_string(), config: cfg, hold_secs: hold, morph_secs: morph, active: true, transition_prob: 1.0 }
}

/// A hardcoded 3-minute demo piece showcasing the best sounds.
pub fn demo_arrangement() -> Vec<Scene> {
    let mut scenes: Vec<Scene> = vec![
        make_scene("Opening",         "Midnight Approach",    25.0, 0.0,  |_| {}),
        make_scene("The Emergence",   "The Phase Transition", 20.0, 30.0, |c| {
            c.kuramoto.coupling = 1.0;
        }),
        make_scene("Turbulence",      "Frozen Machinery",     15.0, 25.0, |_| {}),
        make_scene("The Turn",        "Glass Harp",           20.0, 28.0, |_| {}),
        make_scene("Dissolution",     "Collapsing Cathedral", 25.0, 30.0, |_| {}),
        make_scene("Return",          "Midnight Approach",    20.0, 25.0, |c| {
            c.audio.reverb_wet = (c.audio.reverb_wet + 0.08).min(0.95);
        }),
    ];
    while scenes.len() < 8 {
        scenes.push(Scene::empty(scenes.len()));
    }
    scenes
}

/// Generate a full arrangement for a given mood.
/// Every call produces a unique combination of presets, modes, scales, and effects.
/// Morphs are the feature — time is spent in transition, not in stasis.
pub fn generate_song(mood: &str, seed: u64) -> Vec<Scene> {
    let mut rng = seed ^ 0xdeadbeef_cafebabe;

    // LCG helpers
    let rf = |rng: &mut u64| -> f32 { lcg(rng) as f32 };              // 0..1
    let ri = |rng: &mut u64, n: usize| -> usize {                      // 0..n
        (lcg(rng) * n as f64) as usize % n
    };
    let rrange = |rng: &mut u64, lo: f32, hi: f32| -> f32 {           // lo..hi
        lo + rf(rng) * (hi - lo)
    };

    // Pick from a slice randomly
    let pick = |rng: &mut u64, choices: &[&str]| -> String {
        choices[ri(rng, choices.len())].to_string()
    };

    // Preset pools per mood (large enough that repeats are rare across 6 scenes)
    let ambient_pool: &[&str] = &[
        "Midnight Approach", "The Irrational Winding", "Breathing Galaxy",
        "Memory of Water", "Monk's Bell", "Deep Hypnosis",
        "Aurora Borealis", "The Synchronization", "Last Light",
        "Cathedral Organ", "Collapsing Cathedral", "The Butterfly's Aria",
        "Siren Call", "Throat of the Storm",
    ];
    let rhythmic_pool: &[&str] = &[
        "Frozen Machinery", "The Phase Transition", "Clockwork Insect",
        "Planetary Clockwork", "Industrial Heartbeat", "Bone Structure",
        "Dissociation", "Neon Labyrinth", "Tungsten Filament", "The Double Scroll",
        "Solar Wind", "Electric Kelp", "Möbius Lead",
    ];
    let experimental_pool: &[&str] = &[
        "Neon Labyrinth", "Dissociation", "Tungsten Filament", "The Double Scroll",
        "Bone Structure", "Clockwork Insect", "Ancient Algorithm",
        "Seismic Event", "Frozen Machinery", "The Phase Transition",
        "Electric Kelp", "Solar Wind", "Collapsing Cathedral",
    ];

    let pool = match mood {
        "rhythmic"     => rhythmic_pool,
        "experimental" => experimental_pool,
        _              => ambient_pool,
    };

    // Sonification modes weighted by mood
    let modes_ambient:       &[&str] = &["direct", "direct", "orbital", "spectral", "granular"];
    let modes_rhythmic:      &[&str] = &["direct", "direct", "granular", "fm", "orbital"];
    let modes_experimental:  &[&str] = &["spectral", "fm", "granular", "orbital", "direct"];
    let mode_pool = match mood {
        "rhythmic"     => modes_rhythmic,
        "experimental" => modes_experimental,
        _              => modes_ambient,
    };

    // Scale pools per mood
    let scales_ambient:      &[&str] = &["pentatonic", "pentatonic", "just_intonation", "chromatic"];
    let scales_rhythmic:     &[&str] = &["pentatonic", "chromatic", "chromatic", "just_intonation"];
    let scales_experimental: &[&str] = &["microtonal", "chromatic", "just_intonation", "pentatonic"];
    let scale_pool = match mood {
        "rhythmic"     => scales_rhythmic,
        "experimental" => scales_experimental,
        _              => scales_ambient,
    };

    // Hold / morph time ranges per mood (morphs intentionally longer)
    let (hold_lo, hold_hi, morph_lo, morph_hi) = match mood {
        "rhythmic"     => (12.0f32, 20.0, 20.0f32, 32.0),
        "experimental" => (10.0f32, 18.0, 25.0f32, 40.0),
        _              => (15.0f32, 25.0, 25.0f32, 38.0),
    };

    // Pick 6 scenes with no two adjacent presets the same
    let n_scenes = 6;
    let mut chosen_presets: Vec<String> = Vec::with_capacity(n_scenes);
    for _ in 0..n_scenes {
        let mut attempts = 0;
        loop {
            let candidate = pick(&mut rng, pool);
            let last = chosen_presets.last().map(|s| s.as_str()).unwrap_or("");
            if candidate != last || attempts > 8 { chosen_presets.push(candidate); break; }
            attempts += 1;
        }
    }

    // Scene name banks
    let names_ambient:      &[&str] = &["Drift", "Emerge", "Float", "Breathe", "Dissolve", "Return", "Expand", "Recede"];
    let names_rhythmic:     &[&str] = &["Pulse", "Build", "Lock", "Scatter", "Drive", "Release", "Surge", "Drop"];
    let names_experimental: &[&str] = &["Fracture", "Warp", "Corrupt", "Scatter", "Void", "Strange", "Collapse", "Mutate"];
    let name_pool = match mood {
        "rhythmic"     => names_rhythmic,
        "experimental" => names_experimental,
        _              => names_ambient,
    };

    let mut scenes: Vec<Scene> = chosen_presets.iter().enumerate().map(|(i, preset)| {
        let hold  = rrange(&mut rng, hold_lo, hold_hi);
        let morph = if i == 0 { 0.0 } else { rrange(&mut rng, morph_lo, morph_hi) };
        let mode  = pick(&mut rng, mode_pool);
        let scale = pick(&mut rng, scale_pool);
        // Randomize effects per scene
        let reverb   = rrange(&mut rng, 0.35, 0.85);
        let chorus   = rrange(&mut rng, 0.0, 0.55);
        let delay_fb = rrange(&mut rng, 0.0, 0.6);
        let delay_ms = rrange(&mut rng, 80.0, 700.0);
        let porta    = rrange(&mut rng, 30.0, 600.0);
        let speed_mult = rrange(&mut rng, 0.6, 1.8);
        let name_idx = (i + ri(&mut rng, 3)) % name_pool.len();
        let name = name_pool[name_idx];

        make_scene(name, preset, hold, morph, move |c| {
            c.sonification.mode  = mode;
            c.sonification.scale = scale;
            c.system.speed      *= speed_mult as f64;
            // Clamp speed to audible range
            c.system.speed       = c.system.speed.clamp(0.5, 6.0);
            c.audio.reverb_wet   = reverb.min(0.82); // cap reverb to avoid washout
            c.audio.chorus_mix   = chorus;
            c.audio.delay_feedback = delay_fb;
            c.audio.delay_ms     = delay_ms;
            c.sonification.portamento_ms = porta;
            // Ensure base_frequency is audible
            c.sonification.base_frequency = c.sonification.base_frequency.clamp(60.0, 600.0);
            // Guarantee audible master_volume — never let arrangement scenes go silent
            c.audio.master_volume = c.audio.master_volume.max(0.62);
        })
    }).collect();

    // For rhythmic mood, randomly enable bitcrusher on 1-2 scenes for texture
    if mood == "rhythmic" || mood == "experimental" {
        for scene in scenes.iter_mut() {
            if rf(&mut rng) > 0.65 {
                scene.config.audio.bit_depth  = rrange(&mut rng, 6.0, 14.0);
                scene.config.audio.rate_crush = rrange(&mut rng, 0.0, 0.4);
            }
        }
    }

    // Pad to 8 slots
    while scenes.len() < 8 {
        scenes.push(Scene::empty(scenes.len()));
    }
    scenes
}
