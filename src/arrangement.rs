// Functions here are used by the binary but may appear unused in the plugin lib context.
#![allow(dead_code)]

use crate::config::*;
use crate::patches::load_preset;

/// Controls how the arranger transitions from one scene to the next.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionType {
    /// Current behaviour: interpolate all parameters over `morph_secs`.
    Morph,
    /// Instant parameter jump — `morph_secs` is ignored and treated as 0.
    Snap,
    /// Parameters jump immediately (like Snap) but the audio layer cross-fades
    /// via its existing `mode_morph` mechanism.  Config-side `t` is always 1.0.
    Fade,
}

impl Default for TransitionType {
    fn default() -> Self {
        Self::Morph
    }
}

/// A single named snapshot of synthesis configuration used by the scene arranger.
///
/// Scenes are arranged in a linear timeline.  The arranger holds at each scene
/// for `hold_secs` seconds and then morphs to the next active scene over
/// `morph_secs` seconds.  During a morph all numeric fields of `config` are
/// linearly interpolated; string fields switch at the midpoint.
#[derive(Clone)]
pub struct Scene {
    /// Human-readable label shown in the Timeline tab.
    pub name: String,
    /// Synthesis parameters for this scene.
    pub config: Config,
    /// Duration in seconds to hold at this scene's parameters before morphing.
    pub hold_secs: f32,
    /// Duration in seconds to morph from the previous scene into this one.
    /// For the first active scene this value is ignored.
    pub morph_secs: f32,
    /// Whether this scene participates in the timeline.  Inactive scenes are
    /// skipped during playback and do not contribute to `total_duration`.
    pub active: bool,
    /// Relative probability weight used when `transition_mode = "random"`.
    /// Higher values increase the chance of the arranger jumping to this scene.
    pub transition_prob: f32,
    /// How this scene transitions in from the previous one.
    pub transition_type: TransitionType,
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
            transition_type: TransitionType::default(),
        }
    }
}

/// Linearly interpolate all numeric fields of Config.
/// String fields (system name, mode, scale, chord_mode) switch at t=0.5.
pub fn lerp_config(a: &Config, b: &Config, t: f32) -> Config {
    let t = t.clamp(0.0, 1.0);
    let lf64 = |a: f64, b: f64| -> f64 { a + (b - a) * t as f64 };
    let lf32 = |a: f32, b: f32| -> f32 { a + (b - a) * t };
    let ls = |a: &str, b: &str| -> String {
        if t < 0.5 {
            a.to_string()
        } else {
            b.to_string()
        }
    };

    Config {
        system: SystemConfig {
            name: ls(&a.system.name, &b.system.name),
            dt: lf64(a.system.dt, b.system.dt),
            speed: lf64(a.system.speed, b.system.speed),
        },
        sonification: SonificationConfig {
            mode: ls(&a.sonification.mode, &b.sonification.mode),
            scale: ls(&a.sonification.scale, &b.sonification.scale),
            base_frequency: lf64(a.sonification.base_frequency, b.sonification.base_frequency),
            octave_range: lf64(a.sonification.octave_range, b.sonification.octave_range),
            chord_mode: ls(&a.sonification.chord_mode, &b.sonification.chord_mode),
            transpose_semitones: lf32(
                a.sonification.transpose_semitones,
                b.sonification.transpose_semitones,
            ),
            portamento_ms: lf32(a.sonification.portamento_ms, b.sonification.portamento_ms),
            voice_levels: std::array::from_fn(|i| {
                lf32(
                    a.sonification.voice_levels[i],
                    b.sonification.voice_levels[i],
                )
            }),
            voice_shapes: if t < 0.5 {
                a.sonification.voice_shapes.clone()
            } else {
                b.sonification.voice_shapes.clone()
            },
        },
        audio: AudioConfig {
            sample_rate: a.audio.sample_rate,
            buffer_size: a.audio.buffer_size,
            reverb_wet: lf32(a.audio.reverb_wet, b.audio.reverb_wet),
            delay_ms: lf32(a.audio.delay_ms, b.audio.delay_ms),
            delay_feedback: lf32(a.audio.delay_feedback, b.audio.delay_feedback),
            // Clamp lerped volume so morph valleys never drop below audible threshold
            master_volume: lf32(a.audio.master_volume, b.audio.master_volume).max(0.45),
            bit_depth: lf32(a.audio.bit_depth, b.audio.bit_depth),
            rate_crush: lf32(a.audio.rate_crush, b.audio.rate_crush),
            chorus_mix: lf32(a.audio.chorus_mix, b.audio.chorus_mix),
            chorus_rate: lf32(a.audio.chorus_rate, b.audio.chorus_rate),
            chorus_depth: lf32(a.audio.chorus_depth, b.audio.chorus_depth),
            waveshaper_drive: lf32(a.audio.waveshaper_drive, b.audio.waveshaper_drive),
            waveshaper_mix: lf32(a.audio.waveshaper_mix, b.audio.waveshaper_mix),
            ..a.audio.clone()
        },
        lorenz: LorenzConfig {
            sigma: lf64(a.lorenz.sigma, b.lorenz.sigma),
            rho: lf64(a.lorenz.rho, b.lorenz.rho),
            beta: lf64(a.lorenz.beta, b.lorenz.beta),
        },
        rossler: RosslerConfig {
            a: lf64(a.rossler.a, b.rossler.a),
            b: lf64(a.rossler.b, b.rossler.b),
            c: lf64(a.rossler.c, b.rossler.c),
        },
        double_pendulum: DoublePendulumConfig {
            m1: lf64(a.double_pendulum.m1, b.double_pendulum.m1),
            m2: lf64(a.double_pendulum.m2, b.double_pendulum.m2),
            l1: lf64(a.double_pendulum.l1, b.double_pendulum.l1),
            l2: lf64(a.double_pendulum.l2, b.double_pendulum.l2),
        },
        geodesic_torus: GeodesicTorusConfig {
            big_r: lf64(a.geodesic_torus.big_r, b.geodesic_torus.big_r),
            r: lf64(a.geodesic_torus.r, b.geodesic_torus.r),
        },
        kuramoto: KuramotoConfig {
            n_oscillators: a.kuramoto.n_oscillators,
            coupling: lf64(a.kuramoto.coupling, b.kuramoto.coupling),
        },
        duffing: DuffingConfig {
            delta: lf64(a.duffing.delta, b.duffing.delta),
            alpha: lf64(a.duffing.alpha, b.duffing.alpha),
            beta: lf64(a.duffing.beta, b.duffing.beta),
            gamma: lf64(a.duffing.gamma, b.duffing.gamma),
            omega: lf64(a.duffing.omega, b.duffing.omega),
        },
        van_der_pol: VanDerPolConfig {
            mu: lf64(a.van_der_pol.mu, b.van_der_pol.mu),
        },
        halvorsen: HalvorsenConfig {
            a: lf64(a.halvorsen.a, b.halvorsen.a),
        },
        aizawa: AizawaConfig {
            a: lf64(a.aizawa.a, b.aizawa.a),
            b: lf64(a.aizawa.b, b.aizawa.b),
            c: lf64(a.aizawa.c, b.aizawa.c),
            d: lf64(a.aizawa.d, b.aizawa.d),
            e: lf64(a.aizawa.e, b.aizawa.e),
            f: lf64(a.aizawa.f, b.aizawa.f),
        },
        chua: ChuaConfig {
            alpha: lf64(a.chua.alpha, b.chua.alpha),
            beta: lf64(a.chua.beta, b.chua.beta),
            m0: lf64(a.chua.m0, b.chua.m0),
            m1: lf64(a.chua.m1, b.chua.m1),
        },
        hindmarsh_rose: HindmarshRoseConfig {
            current_i: lf64(a.hindmarsh_rose.current_i, b.hindmarsh_rose.current_i),
            r: lf64(a.hindmarsh_rose.r, b.hindmarsh_rose.r),
        },
        coupled_map_lattice: CmlConfig {
            r: lf64(a.coupled_map_lattice.r, b.coupled_map_lattice.r),
            eps: lf64(a.coupled_map_lattice.eps, b.coupled_map_lattice.eps),
        },
        mackey_glass: MackeyGlassConfig {
            beta: lf64(a.mackey_glass.beta, b.mackey_glass.beta),
            gamma: lf64(a.mackey_glass.gamma, b.mackey_glass.gamma),
            tau: lf64(a.mackey_glass.tau, b.mackey_glass.tau),
            n: lf64(a.mackey_glass.n, b.mackey_glass.n),
        },
        nose_hoover: NoseHooverConfig {
            a: lf64(a.nose_hoover.a, b.nose_hoover.a),
        },
        henon_map: HenonMapConfig {
            a: lf64(a.henon_map.a, b.henon_map.a),
            b: lf64(a.henon_map.b, b.henon_map.b),
        },
        lorenz96: Lorenz96Config {
            f: lf64(a.lorenz96.f, b.lorenz96.f),
        },
        thomas: ThomasConfig {
            b: lf64(a.thomas.b, b.thomas.b),
        },
        dadras: DadrasConfig {
            a: lf64(a.dadras.a, b.dadras.a),
            b: lf64(a.dadras.b, b.dadras.b),
            c: lf64(a.dadras.c, b.dadras.c),
            d: lf64(a.dadras.d, b.dadras.d),
            e: lf64(a.dadras.e, b.dadras.e),
        },
        rucklidge: RucklidgeConfig {
            kappa: lf64(a.rucklidge.kappa, b.rucklidge.kappa),
            lambda: lf64(a.rucklidge.lambda, b.rucklidge.lambda),
        },
        chen: ChenConfig {
            a: lf64(a.chen.a, b.chen.a),
            b: lf64(a.chen.b, b.chen.b),
            c: lf64(a.chen.c, b.chen.c),
        },
        burke_shaw: BurkeShawConfig {
            sigma: lf64(a.burke_shaw.sigma, b.burke_shaw.sigma),
            rho: lf64(a.burke_shaw.rho, b.burke_shaw.rho),
        },
        lorenz84: crate::config::Lorenz84Config {
            a: lf64(a.lorenz84.a, b.lorenz84.a),
            b: lf64(a.lorenz84.b, b.lorenz84.b),
            f: lf64(a.lorenz84.f, b.lorenz84.f),
            g: lf64(a.lorenz84.g, b.lorenz84.g),
        },
        rabinovich_fabrikant: crate::config::RabinovichFabrikantConfig {
            alpha: lf64(a.rabinovich_fabrikant.alpha, b.rabinovich_fabrikant.alpha),
            gamma: lf64(a.rabinovich_fabrikant.gamma, b.rabinovich_fabrikant.gamma),
        },
        rikitake: crate::config::RikitakeConfig {
            mu: lf64(a.rikitake.mu, b.rikitake.mu),
            a: lf64(a.rikitake.a, b.rikitake.a),
        },
        logistic_map: crate::config::LogisticMapConfig {
            r: lf64(a.logistic_map.r, b.logistic_map.r),
        },
        standard_map: crate::config::StandardMapConfig {
            k: lf64(a.standard_map.k, b.standard_map.k),
        },
        stochastic_lorenz: crate::config::StochasticLorenzConfig {
            sigma: lf64(a.stochastic_lorenz.sigma, b.stochastic_lorenz.sigma),
            rho: lf64(a.stochastic_lorenz.rho, b.stochastic_lorenz.rho),
            beta: lf64(a.stochastic_lorenz.beta, b.stochastic_lorenz.beta),
            noise_strength: lf64(a.stochastic_lorenz.noise_strength, b.stochastic_lorenz.noise_strength),
        },
        delayed_map: crate::config::DelayedMapConfig {
            r: lf64(a.delayed_map.r, b.delayed_map.r),
            // tau is discrete — switch at midpoint like string fields
            tau: if t < 0.5 { a.delayed_map.tau } else { b.delayed_map.tau },
        },
        oregonator: crate::config::OregonatorConfig {
            f: lf64(a.oregonator.f, b.oregonator.f),
        },
        mathieu: crate::config::MathieuConfig {
            a: lf64(a.mathieu.a, b.mathieu.a),
            q: lf64(a.mathieu.q, b.mathieu.q),
        },
        kuramoto_driven: crate::config::KuramotoDrivenConfig {
            coupling: lf64(a.kuramoto_driven.coupling, b.kuramoto_driven.coupling),
            drive_amp: lf64(a.kuramoto_driven.drive_amp, b.kuramoto_driven.drive_amp),
            drive_freq: lf64(a.kuramoto_driven.drive_freq, b.kuramoto_driven.drive_freq),
        },
        fractional_lorenz: crate::config::FractionalLorenzConfig {
            alpha: lf64(a.fractional_lorenz.alpha, b.fractional_lorenz.alpha),
            sigma: lf64(a.fractional_lorenz.sigma, b.fractional_lorenz.sigma),
            rho: lf64(a.fractional_lorenz.rho, b.fractional_lorenz.rho),
            beta: lf64(a.fractional_lorenz.beta, b.fractional_lorenz.beta),
        },
        viz: a.viz.clone(), // don't morph viz settings
        ..a.clone()
    }
}

/// Total arrangement duration in seconds (sum of active scenes' hold + morph times).
pub fn total_duration(scenes: &[Scene]) -> f32 {
    let active: Vec<usize> = (0..scenes.len()).filter(|&i| scenes[i].active).collect();
    active
        .iter()
        .enumerate()
        .map(|(ord, &idx)| {
            let s = &scenes[idx];
            let morph = if ord > 0 && s.transition_type != TransitionType::Snap {
                s.morph_secs
            } else {
                0.0
            };
            morph + s.hold_secs
        })
        .sum()
}

/// Elapsed position in arrangement -> (scene_index, phase, t)
/// phase: true = morphing into scene_index, false = holding at scene_index
pub fn scene_at(scenes: &[Scene], elapsed: f32) -> Option<(usize, bool, f32)> {
    let active: Vec<usize> = (0..scenes.len()).filter(|&i| scenes[i].active).collect();
    if active.is_empty() {
        return None;
    }
    let mut t = elapsed;
    for (ord, &idx) in active.iter().enumerate() {
        let scene = &scenes[idx];
        // Morph phase first (transition INTO this scene FROM previous), skip for first scene.
        // Snap transitions skip the morph phase entirely (treated as morph_secs = 0).
        if ord > 0 && scene.transition_type != TransitionType::Snap {
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
    #[allow(clippy::unreadable_literal)]
    {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
    }
    // >> 33 produces 31 bits of output; divide by 2^31 to get [0, 1).
    // Previously divided by u32::MAX (≈ 2^32), which capped output at ~0.5.
    (*seed >> 33) as f64 / (1u64 << 31) as f64
}

fn make_scene(
    name: &str,
    preset: &str,
    hold: f32,
    morph: f32,
    tweaks: impl FnOnce(&mut Config),
) -> Scene {
    let mut cfg = load_preset(preset);
    tweaks(&mut cfg);
    Scene {
        name: name.to_string(),
        config: cfg,
        hold_secs: hold,
        morph_secs: morph,
        active: true,
        transition_prob: 1.0,
        transition_type: TransitionType::default(),
    }
}

/// A hardcoded 3-minute demo piece showcasing the best sounds.
pub fn demo_arrangement() -> Vec<Scene> {
    let mut scenes: Vec<Scene> = vec![
        make_scene("Opening", "Midnight Approach", 25.0, 0.0, |_| {}),
        make_scene("The Emergence", "The Phase Transition", 20.0, 30.0, |c| {
            c.kuramoto.coupling = 1.0;
        }),
        make_scene("Turbulence", "Frozen Machinery", 15.0, 25.0, |_| {}),
        make_scene("The Turn", "Glass Harp", 20.0, 28.0, |_| {}),
        make_scene("Dissolution", "Collapsing Cathedral", 25.0, 30.0, |_| {}),
        make_scene("Return", "Midnight Approach", 20.0, 25.0, |c| {
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
    #[allow(clippy::unreadable_literal)]
    let mut rng = seed ^ 0xDEAD_BEEF_CAFE_BABE;

    // LCG helpers
    let rf = |rng: &mut u64| -> f32 { lcg(rng) as f32 }; // 0..1
    let ri = |rng: &mut u64, n: usize| -> usize {
        // 0..n
        (lcg(rng) * n as f64) as usize % n
    };
    let rrange = |rng: &mut u64, lo: f32, hi: f32| -> f32 {
        // lo..hi
        lo + rf(rng) * (hi - lo)
    };

    // Pick from a slice randomly
    let pick =
        |rng: &mut u64, choices: &[&str]| -> String { choices[ri(rng, choices.len())].to_string() };

    // Preset pools per mood (large enough that repeats are rare across 6 scenes)
    let ambient_pool: &[&str] = &[
        "Midnight Approach",
        "The Irrational Winding",
        "Breathing Galaxy",
        "Memory of Water",
        "Monk's Bell",
        "Deep Hypnosis",
        "Aurora Borealis",
        "The Synchronization",
        "Last Light",
        "Cathedral Organ",
        "Collapsing Cathedral",
        "The Butterfly's Aria",
        "Siren Call",
        "Throat of the Storm",
    ];
    let rhythmic_pool: &[&str] = &[
        "Frozen Machinery",
        "The Phase Transition",
        "Clockwork Insect",
        "Planetary Clockwork",
        "Industrial Heartbeat",
        "Bone Structure",
        "Dissociation",
        "Neon Labyrinth",
        "Tungsten Filament",
        "The Double Scroll",
        "Solar Wind",
        "Electric Kelp",
        "Möbius Lead",
    ];
    let experimental_pool: &[&str] = &[
        "Neon Labyrinth",
        "Dissociation",
        "Tungsten Filament",
        "The Double Scroll",
        "Bone Structure",
        "Clockwork Insect",
        "Ancient Algorithm",
        "Seismic Event",
        "Frozen Machinery",
        "The Phase Transition",
        "Electric Kelp",
        "Solar Wind",
        "Collapsing Cathedral",
    ];

    let pool = match mood {
        "rhythmic" => rhythmic_pool,
        "experimental" => experimental_pool,
        _ => ambient_pool,
    };

    // Sonification modes — full pool, weighted toward mood character
    let modes_ambient: &[&str] = &[
        "direct",
        "direct",
        "orbital",
        "spectral",
        "granular",
        "vocal",
        "waveguide",
    ];
    let modes_rhythmic: &[&str] = &[
        "direct", "direct", "granular", "fm", "orbital", "granular", "spectral",
    ];
    let modes_experimental: &[&str] = &[
        "spectral",
        "fm",
        "granular",
        "orbital",
        "vocal",
        "waveguide",
        "fm",
    ];
    let mode_pool = match mood {
        "rhythmic" => modes_rhythmic,
        "experimental" => modes_experimental,
        _ => modes_ambient,
    };

    // Scale pools per mood
    let scales_ambient: &[&str] = &[
        "pentatonic",
        "pentatonic",
        "just_intonation",
        "chromatic",
        "pentatonic",
    ];
    let scales_rhythmic: &[&str] = &[
        "pentatonic",
        "chromatic",
        "chromatic",
        "just_intonation",
        "pentatonic",
    ];
    let scales_experimental: &[&str] = &[
        "microtonal",
        "chromatic",
        "just_intonation",
        "pentatonic",
        "microtonal",
    ];
    let scale_pool = match mood {
        "rhythmic" => scales_rhythmic,
        "experimental" => scales_experimental,
        _ => scales_ambient,
    };

    // Voice shape pool
    let voice_shapes: &[&str] = &[
        "sine", "sine", "sine", "triangle", "saw", "triangle", "triangle",
    ];

    // Chord mode pool
    let chord_modes: &[&str] = &["none", "none", "none", "power", "octave", "power"];

    // Musical root frequencies (A2=110 through A4=440, hitting each octave position)
    let root_freqs: &[f64] = &[
        82.4, 110.0, 130.8, 164.8, 196.0, 220.0, 261.6, 293.7, 329.6, 392.0, 440.0, 523.3,
    ];

    // Transpose pool in semitones (musically sensible intervals)
    let transpose_opts: &[f32] = &[-12.0, -7.0, -5.0, 0.0, 0.0, 0.0, 5.0, 7.0, 12.0];

    // item 17: Coherence budget — anchor musical DNA at song level so scenes form an arc.
    let anchor_root_freq = root_freqs[ri(&mut rng, root_freqs.len())];
    let anchor_scale = pick(&mut rng, scale_pool);
    let anchor_speed_mult = rrange(&mut rng, 0.6, 2.0);
    let reverb_budget_total = rrange(&mut rng, 2.0, 3.5);
    let mut reverb_spent = 0.0f32;

    // Hold / morph time ranges per mood (morphs intentionally longer)
    let (hold_lo, hold_hi, morph_lo, morph_hi) = match mood {
        "rhythmic" => (10.0f32, 20.0, 18.0f32, 32.0),
        "experimental" => (8.0f32, 18.0, 22.0f32, 42.0),
        _ => (12.0f32, 26.0, 22.0f32, 40.0),
    };

    // 6 scenes, no two adjacent presets the same
    let n_scenes = 6;
    let mut chosen_presets: Vec<String> = Vec::with_capacity(n_scenes);
    for _ in 0..n_scenes {
        let mut attempts = 0;
        loop {
            let candidate = pick(&mut rng, pool);
            let last = chosen_presets.last().map(|s| s.as_str()).unwrap_or("");
            if candidate != last || attempts > 8 {
                chosen_presets.push(candidate);
                break;
            }
            attempts += 1;
        }
    }

    // Scene name banks
    let names_ambient: &[&str] = &[
        "Drift", "Emerge", "Float", "Breathe", "Dissolve", "Return", "Expand", "Recede",
    ];
    let names_rhythmic: &[&str] = &[
        "Pulse", "Build", "Lock", "Scatter", "Drive", "Release", "Surge", "Drop",
    ];
    let names_experimental: &[&str] = &[
        "Fracture", "Warp", "Corrupt", "Scatter", "Void", "Strange", "Collapse", "Mutate",
    ];
    let name_pool = match mood {
        "rhythmic" => names_rhythmic,
        "experimental" => names_experimental,
        _ => names_ambient,
    };

    let mut scenes: Vec<Scene> = Vec::with_capacity(n_scenes);
    for (i, preset) in chosen_presets.iter().enumerate() {
        let hold = rrange(&mut rng, hold_lo, hold_hi);
        let morph = if i == 0 {
            0.0
        } else {
            rrange(&mut rng, morph_lo, morph_hi)
        };
        let mode = pick(&mut rng, mode_pool);
        let scale = if rf(&mut rng) < 0.65 {
            anchor_scale.clone()
        } else {
            pick(&mut rng, scale_pool)
        };
        let chord = pick(&mut rng, chord_modes);

        // Effects — wide independent ranges per scene
        let remaining_scenes = (n_scenes - i).max(1) as f32;
        let reverb_max =
            ((reverb_budget_total - reverb_spent) / remaining_scenes).clamp(0.15, 0.80);
        let reverb = rrange(&mut rng, 0.15, reverb_max);
        reverb_spent += reverb;
        let chorus = rrange(&mut rng, 0.0, 0.65);
        let delay_fb = rrange(&mut rng, 0.0, 0.65);
        let delay_ms = rrange(&mut rng, 60.0, 800.0);
        let porta = rrange(&mut rng, 20.0, 800.0);
        let waveshaper_drive = rrange(&mut rng, 1.0, 6.0);
        let waveshaper_mix = if rf(&mut rng) > 0.5 {
            rrange(&mut rng, 0.0, 0.5)
        } else {
            0.0
        };

        // Pitch — anchor-coherent root frequency and transposition
        let base_freq = if rf(&mut rng) < 0.70 {
            anchor_root_freq
        } else {
            let octave_shift = if rf(&mut rng) < 0.5 { 0.5 } else { 2.0 };
            (anchor_root_freq * octave_shift).clamp(82.0, 880.0)
        };
        let transpose = transpose_opts[ri(&mut rng, transpose_opts.len())];
        let octave_range = rrange(&mut rng, 0.8, 3.5) as f64;

        // Speed — scatter around the anchor speed
        let speed_scatter = rrange(&mut rng, 0.6, 1.4);
        let speed_mult = anchor_speed_mult * speed_scatter;

        // Voice shapes — each voice independently randomized
        let vs0 = pick(&mut rng, voice_shapes);
        let vs1 = pick(&mut rng, voice_shapes);
        let vs2 = pick(&mut rng, voice_shapes);
        let vs3 = pick(&mut rng, voice_shapes);

        // Voice levels — random mix of 4 oscillators (always sum ≥ 1.0 so there's always sound)
        let vl0 = rrange(&mut rng, 0.3, 1.0);
        let vl1 = rrange(&mut rng, 0.0, 1.0);
        let vl2 = rrange(&mut rng, 0.0, 0.9);
        let vl3 = rrange(&mut rng, 0.0, 0.7);

        // System parameter scatter — push parameters into different dynamical regimes
        let lorenz_sigma = rrange(&mut rng, 7.0, 14.0) as f64;
        let lorenz_rho = rrange(&mut rng, 20.0, 40.0) as f64;
        let lorenz_beta = rrange(&mut rng, 1.5, 4.0) as f64;
        let rossler_a = rrange(&mut rng, 0.05, 0.35) as f64;
        let rossler_c = rrange(&mut rng, 3.0, 13.0) as f64;
        let kuramoto_k = rrange(&mut rng, 0.3, 4.5) as f64;
        let duffing_om = rrange(&mut rng, 0.6, 1.4) as f64;
        let halvorsen_a = rrange(&mut rng, 1.2, 1.9) as f64;

        let name_idx = (i + ri(&mut rng, 3)) % name_pool.len();
        let name = name_pool[name_idx];

        scenes.push(make_scene(name, preset, hold, morph, move |c| {
            c.sonification.mode = mode;
            c.sonification.scale = scale;
            c.sonification.chord_mode = chord;
            c.sonification.base_frequency = base_freq;
            c.sonification.octave_range = octave_range;
            c.sonification.transpose_semitones = transpose;
            c.sonification.portamento_ms = porta;
            c.sonification.voice_shapes = [vs0, vs1, vs2, vs3];
            c.sonification.voice_levels = [vl0, vl1, vl2, vl3];

            // Speed
            c.system.speed = (c.system.speed * speed_mult as f64).clamp(0.5, 6.0);

            // Effects
            c.audio.reverb_wet = reverb.min(0.82);
            c.audio.chorus_mix = chorus;
            c.audio.delay_feedback = delay_fb;
            c.audio.delay_ms = delay_ms;
            c.audio.waveshaper_drive = waveshaper_drive;
            c.audio.waveshaper_mix = waveshaper_mix;

            // System-specific parameter scatter (independent of preset values)
            c.lorenz.sigma = lorenz_sigma;
            c.lorenz.rho = lorenz_rho;
            c.lorenz.beta = lorenz_beta;
            c.rossler.a = rossler_a;
            c.rossler.c = rossler_c;
            c.kuramoto.coupling = kuramoto_k;
            c.duffing.omega = duffing_om;
            c.halvorsen.a = halvorsen_a;

            c.audio.master_volume = c.audio.master_volume.max(0.62);
        }));
    }

    // Bitcrusher texture for rhythmic/experimental — independent per scene
    if mood == "rhythmic" || mood == "experimental" {
        for scene in scenes.iter_mut() {
            if rf(&mut rng) > 0.55 {
                scene.config.audio.bit_depth = rrange(&mut rng, 5.0, 15.0);
                scene.config.audio.rate_crush = rrange(&mut rng, 0.0, 0.45);
            }
        }
    }

    // Pad to 8 slots
    while scenes.len() < 8 {
        scenes.push(Scene::empty(scenes.len()));
    }
    scenes
}
