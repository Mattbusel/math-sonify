// Functions here are used by the binary but may appear unused in the plugin lib context.
#![allow(dead_code)]

use crate::config::*;
use egui::Color32;

pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub color: egui::Color32,
    pub category: &'static str,
}

pub const PRESETS: &[Preset] = &[
    // --- ATMOSPHERIC ---
    Preset {
        name: "Midnight Approach",
        description: "The butterfly attractor at glacial speed. A harmonic drone that never quite repeats.",
        color: Color32::from_rgb(0, 120, 200),
        category: "Atmospheric",
    },
    Preset {
        name: "Collapsing Cathedral",
        description: "Vast reverberant chaos. The attractor orbits through minor chord space like a bell that forgot to stop ringing.",
        color: Color32::from_rgb(60, 0, 120),
        category: "Atmospheric",
    },
    Preset {
        name: "The Irrational Winding",
        description: "A geodesic torus path that is provably ergodic — it will visit every point on the surface, eventually. A drone that mathematically cannot repeat.",
        color: Color32::from_rgb(100, 60, 200),
        category: "Atmospheric",
    },
    Preset {
        name: "Breathing Galaxy",
        description: "Each harmonic partial drifting independently. A slowly rotating chord cluster that expands and contracts like something alive.",
        color: Color32::from_rgb(20, 80, 180),
        category: "Atmospheric",
    },
    Preset {
        name: "Throat of the Storm",
        description: "Lorenz at high sigma with vocal formant synthesis. The attractor speaks in vowels.",
        color: Color32::from_rgb(180, 60, 0),
        category: "Atmospheric",
    },
    Preset {
        name: "Siren Call",
        description: "Rössler spiral wandering through vowel space. Uncanny, almost human, but not.",
        color: Color32::from_rgb(200, 40, 120),
        category: "Atmospheric",
    },

    // --- RHYTHMIC ---
    Preset {
        name: "Frozen Machinery",
        description: "Duffing period-doubling through a bitcrusher. The mathematical route to chaos sounds like a machine breaking in slow motion.",
        color: Color32::from_rgb(80, 80, 80),
        category: "Rhythmic",
    },
    Preset {
        name: "The Phase Transition",
        description: "Eight oscillators between noise and harmony — drag K to cross the synchronization boundary.",
        color: Color32::from_rgb(0, 180, 100),
        category: "Rhythmic",
    },
    Preset {
        name: "Clockwork Insect",
        description: "Van der Pol limit cycle at high mu — a self-sustaining oscillation that clicks and grinds with mechanical regularity.",
        color: Color32::from_rgb(120, 80, 0),
        category: "Rhythmic",
    },
    Preset {
        name: "Planetary Clockwork",
        description: "Three gravitational bodies locked in figure-eight orbit. The rhythm is the orbital period. The pitch is the angular velocity.",
        color: Color32::from_rgb(200, 160, 0),
        category: "Rhythmic",
    },
    Preset {
        name: "Industrial Heartbeat",
        description: "The double pendulum's chaotic swings trigger a physical string model. Irregular tempo, real physics.",
        color: Color32::from_rgb(140, 40, 0),
        category: "Rhythmic",
    },
    Preset {
        name: "Bone Structure",
        description: "Chua double-scroll mapped to waveguide strings. The attractor plucks — you hear the resonance of a material that doesn't exist.",
        color: Color32::from_rgb(160, 20, 180),
        category: "Rhythmic",
    },

    // --- MELODIC ---
    Preset {
        name: "Glass Harp",
        description: "Aizawa attractor tracing a delicate toroidal path. Each loop is a note. Each orbit is a phrase.",
        color: Color32::from_rgb(140, 220, 255),
        category: "Melodic",
    },
    Preset {
        name: "Electric Kelp",
        description: "Rössler spiral in FM mode. The modulation index follows the chaos level — the more chaotic, the richer the harmonic content.",
        color: Color32::from_rgb(0, 200, 120),
        category: "Melodic",
    },
    Preset {
        name: "The Butterfly's Aria",
        description: "Lorenz at exactly the chaos boundary (rho=24.5). At this value the system is right on the edge — every trajectory is a different song.",
        color: Color32::from_rgb(0, 160, 220),
        category: "Melodic",
    },
    Preset {
        name: "Solar Wind",
        description: "Halvorsen attractor in FM synthesis. The dense spiral trajectory drives the modulation index, producing shimmering harmonic clouds.",
        color: Color32::from_rgb(220, 160, 20),
        category: "Melodic",
    },
    Preset {
        name: "Möbius Lead",
        description: "Aizawa in orbital mode. The attractor traces a path with half-twist topology — the melody has no beginning.",
        color: Color32::from_rgb(180, 100, 220),
        category: "Melodic",
    },

    // --- CINEMATIC ---
    Preset {
        name: "Last Light",
        description: "Rössler c-parameter near the bifurcation point. The spiral tightens. The chord opens. Something ends.",
        color: Color32::from_rgb(180, 80, 40),
        category: "Cinematic",
    },
    Preset {
        name: "Seismic Event",
        description: "Three-body gravitational chaos at sub-bass frequencies. The figure-eight orbit produces a rhythm no human could notate.",
        color: Color32::from_rgb(60, 40, 120),
        category: "Cinematic",
    },
    Preset {
        name: "Ancient Algorithm",
        description: "Three-body system in spectral and microtonal mode. Gravitational mathematics converted to quarter-tones. Nothing is resolved.",
        color: Color32::from_rgb(80, 120, 60),
        category: "Cinematic",
    },
    Preset {
        name: "Cathedral Organ",
        description: "Lorenz attractor voiced through a 32-partial spectral additive synthesizer. The chaos drives the harmonic balance.",
        color: Color32::from_rgb(140, 60, 160),
        category: "Cinematic",
    },

    // --- EXPERIMENTAL ---
    Preset {
        name: "Neon Labyrinth",
        description: "Chua double-scroll at high speed in spectral mode. Dense chromatic content that never resolves, never repeats.",
        color: Color32::from_rgb(0, 255, 120),
        category: "Experimental",
    },
    Preset {
        name: "Dissociation",
        description: "Double pendulum in granular mode at high speed. The grain density tracks the trajectory — when the pendulum is most chaotic, the texture is densest.",
        color: Color32::from_rgb(255, 0, 160),
        category: "Experimental",
    },
    Preset {
        name: "Tungsten Filament",
        description: "Chua circuit through heavy waveshaper saturation. The electronic buzz of something at its operating limit.",
        color: Color32::from_rgb(255, 100, 0),
        category: "Experimental",
    },
    Preset {
        name: "The Double Scroll",
        description: "Chua circuit raw — the original electronic chaos. Two lobes, infinite complexity.",
        color: Color32::from_rgb(200, 0, 200),
        category: "Experimental",
    },

    // --- MEDITATIVE ---
    Preset {
        name: "Memory of Water",
        description: "The system depends on its history — this drone remembers where it has been. A harmonic field of extraordinary patience.",
        color: Color32::from_rgb(0, 80, 160),
        category: "Meditative",
    },
    Preset {
        name: "Monk's Bell",
        description: "Double pendulum with long delay triggering on every zero-crossing. Irregular intervals, infinite sustain.",
        color: Color32::from_rgb(180, 140, 60),
        category: "Meditative",
    },
    Preset {
        name: "Deep Hypnosis",
        description: "Geodesic torus at near-zero speed in microtonal scale. A drone so slow it's nearly DC. It drifts through quarter-tones glacially.",
        color: Color32::from_rgb(40, 20, 100),
        category: "Meditative",
    },
    Preset {
        name: "Aurora Borealis",
        description: "Lorenz in FM mode with heavy chorus. The butterfly trajectory drives the modulation index — high chaos means richer sideband content.",
        color: Color32::from_rgb(0, 200, 180),
        category: "Meditative",
    },
    Preset {
        name: "The Synchronization",
        description: "Kuramoto oscillators at the exact coupling strength where order emerges from chaos. This is a mathematical phase transition, audible.",
        color: Color32::from_rgb(0, 220, 80),
        category: "Meditative",
    },

    // --- EXPERIMENTAL (new systems) ---
    Preset {
        name: "Cyclic Tangle",
        description: "Thomas attractor — three variables each driving the next in a cyclic loop. The dissipation b≈0.208 is the exact threshold between order and chaos.",
        color: Color32::from_rgb(80, 200, 120),
        category: "Experimental",
    },
    Preset {
        name: "Polarity Reversal",
        description: "Rikitake two-disk dynamo — the mathematical model of geomagnetic polarity flips. The irregular intervals between reversals are the pitch.",
        color: Color32::from_rgb(200, 80, 40),
        category: "Cinematic",
    },
    Preset {
        name: "Anti-Lorenz",
        description: "Chen attractor — derived from Lorenz by anti-control. The same folded band topology, but synthesized to be chaotic. A strange attractor by design.",
        color: Color32::from_rgb(255, 160, 0),
        category: "Experimental",
    },
    Preset {
        name: "Double Convection",
        description: "Rucklidge double-scroll — chaos from a model of convection in a heated fluid layer. The two lobes correspond to left and right circulation.",
        color: Color32::from_rgb(40, 160, 220),
        category: "Atmospheric",
    },
    Preset {
        name: "Mirror Attractor",
        description: "Shimizu-Morioka two-scroll — a left-right symmetric pair of lobes connected by unstable manifolds. The x²-bz coupling creates the scroll pinch.",
        color: Color32::from_rgb(160, 80, 220),
        category: "Experimental",
    },
    Preset {
        name: "Xz Knot",
        description: "Sprott-D: parameter-free chaos from xz coupling and 3y² forcing. One of Sprott's 19 algebraically simplest chaotic flows.",
        color: Color32::from_rgb(220, 120, 60),
        category: "Experimental",
    },
    Preset {
        name: "Equilibrium Fugue",
        description: "Sprott-E: chaos near the fixed point (¼, 1/16, 0). The yz product and x² feedback sustain the attractor with minimal algebra.",
        color: Color32::from_rgb(60, 200, 140),
        category: "Melodic",
    },
    Preset {
        name: "Half-Speed Spiral",
        description: "Sprott-F: the 0.5y damping creates a slow inward spiral that the x² term periodically ruptures into chaos.",
        color: Color32::from_rgb(100, 160, 255),
        category: "Atmospheric",
    },
    Preset {
        name: "Jerk Circuit",
        description: "Genesio-Tesi: a single x² term makes a 3rd-order ODE chaotic. Electronically realizable as a Jerk circuit.",
        color: Color32::from_rgb(200, 200, 60),
        category: "Experimental",
    },
    Preset {
        name: "Velocity Band",
        description: "Liu attractor: the y² coupling to x creates a tight single-band scroll unlike Lorenz's double butterfly.",
        color: Color32::from_rgb(60, 220, 200),
        category: "Rhythmic",
    },
    Preset {
        name: "Substorm",
        description: "WINDMI ionospheric substorm model: exponential feedback in a jerk system evokes sudden electromagnetic bursts.",
        color: Color32::from_rgb(80, 130, 255),
        category: "Atmospheric",
    },
    Preset {
        name: "Invisible Hand",
        description: "Finance attractor: chaotic interest rate, investment, and price-index dynamics in a minimal 3D macroeconomic model.",
        color: Color32::from_rgb(200, 180, 80),
        category: "Experimental",
    },
    Preset {
        name: "Hyperchaos Engine",
        description: "Chen-Li 4D hyperchaotic system: two positive Lyapunov exponents produce maximally unpredictable trajectories.",
        color: Color32::from_rgb(255, 40, 160),
        category: "Experimental",
    },
];

pub fn load_preset(name: &str) -> Config {
    match name {
        // ------------------------------------------------------------------ ATMOSPHERIC
        "Midnight Approach" => Config {
            system: SystemConfig {
                name: "lorenz".into(),
                dt: 0.001,
                speed: 0.3,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "pentatonic".into(),
                base_frequency: 55.0,
                octave_range: 2.0,
                chord_mode: "major".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 600.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.82,
                delay_ms: 500.0,
                delay_feedback: 0.4,
                master_volume: 0.70,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.3,
                chorus_rate: 0.12,
                chorus_depth: 0.5,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            lorenz: LorenzConfig {
                sigma: 10.0,
                rho: 28.0,
                beta: 2.6667,
            },
            ..Default::default()
        },

        "Collapsing Cathedral" => Config {
            system: SystemConfig {
                name: "lorenz".into(),
                dt: 0.001,
                speed: 0.6,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "chromatic".into(),
                base_frequency: 65.0,
                octave_range: 3.5,
                chord_mode: "minor".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4],
                portamento_ms: 400.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.88,
                delay_ms: 800.0,
                delay_feedback: 0.5,
                master_volume: 0.72,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.2,
                chorus_rate: 0.1,
                chorus_depth: 0.4,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            lorenz: LorenzConfig {
                sigma: 14.0,
                rho: 35.0,
                beta: 2.6667,
            },
            ..Default::default()
        },

        "The Irrational Winding" => Config {
            system: SystemConfig {
                name: "geodesic_torus".into(),
                dt: 0.005,
                speed: 0.4,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "just_intonation".into(),
                base_frequency: 80.0,
                octave_range: 2.5,
                chord_mode: "major".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4],
                portamento_ms: 800.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.78,
                delay_ms: 600.0,
                delay_feedback: 0.4,
                master_volume: 0.68,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.4,
                chorus_rate: 0.1,
                chorus_depth: 0.5,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            geodesic_torus: GeodesicTorusConfig { big_r: 3.0, r: 1.0 },
            ..Default::default()
        },

        "Breathing Galaxy" => Config {
            system: SystemConfig {
                name: "halvorsen".into(),
                dt: 0.001,
                speed: 0.5,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "pentatonic".into(),
                base_frequency: 110.0,
                octave_range: 3.0,
                chord_mode: "sus2".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4],
                portamento_ms: 500.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.75,
                delay_ms: 600.0,
                delay_feedback: 0.35,
                master_volume: 0.70,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.5,
                chorus_rate: 0.12,
                chorus_depth: 0.6,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            halvorsen: HalvorsenConfig { a: 1.4 },
            ..Default::default()
        },

        "Throat of the Storm" => Config {
            system: SystemConfig {
                name: "lorenz".into(),
                dt: 0.001,
                speed: 1.2,
            },
            sonification: SonificationConfig {
                mode: "vocal".into(),
                scale: "chromatic".into(),
                base_frequency: 120.0,
                octave_range: 2.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.6, 0.4, 0.2],
                portamento_ms: 200.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.65,
                delay_ms: 300.0,
                delay_feedback: 0.3,
                master_volume: 0.72,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.4,
                chorus_rate: 0.2,
                chorus_depth: 0.5,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            lorenz: LorenzConfig {
                sigma: 18.0,
                rho: 40.0,
                beta: 2.6667,
            },
            ..Default::default()
        },

        "Siren Call" => Config {
            system: SystemConfig {
                name: "rossler".into(),
                dt: 0.002,
                speed: 0.7,
            },
            sonification: SonificationConfig {
                mode: "vocal".into(),
                scale: "pentatonic".into(),
                base_frequency: 200.0,
                octave_range: 2.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.5, 0.3, 0.1],
                portamento_ms: 350.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.70,
                delay_ms: 400.0,
                delay_feedback: 0.35,
                master_volume: 0.70,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.5,
                chorus_rate: 0.18,
                chorus_depth: 0.6,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            rossler: RosslerConfig {
                a: 0.2,
                b: 0.2,
                c: 5.7,
            },
            ..Default::default()
        },

        // ------------------------------------------------------------------ RHYTHMIC
        "Frozen Machinery" => Config {
            system: SystemConfig {
                name: "duffing".into(),
                dt: 0.001,
                speed: 2.0,
            },
            sonification: SonificationConfig {
                mode: "granular".into(),
                scale: "chromatic".into(),
                base_frequency: 110.0,
                octave_range: 2.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 50.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.20,
                delay_ms: 200.0,
                delay_feedback: 0.35,
                master_volume: 0.78,
                bit_depth: 8.0,
                rate_crush: 0.0,
                chorus_mix: 0.1,
                chorus_rate: 0.5,
                chorus_depth: 0.3,
                waveshaper_drive: 3.0,
                waveshaper_mix: 0.6,
                ..Default::default()
            },
            duffing: DuffingConfig {
                delta: 0.3,
                alpha: -1.0,
                beta: 1.0,
                gamma: 0.5,
                omega: 1.2,
            },
            ..Default::default()
        },

        "The Phase Transition" => Config {
            system: SystemConfig {
                name: "kuramoto".into(),
                dt: 0.002,
                speed: 1.0,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "pentatonic".into(),
                base_frequency: 220.0,
                octave_range: 3.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.8, 0.8],
                portamento_ms: 150.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.40,
                delay_ms: 250.0,
                delay_feedback: 0.3,
                master_volume: 0.72,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.3,
                chorus_rate: 0.3,
                chorus_depth: 0.4,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            kuramoto: KuramotoConfig {
                n_oscillators: 8,
                coupling: 1.3,
            },
            ..Default::default()
        },

        "Clockwork Insect" => Config {
            system: SystemConfig {
                name: "van_der_pol".into(),
                dt: 0.001,
                speed: 2.5,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "chromatic".into(),
                base_frequency: 180.0,
                octave_range: 2.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.5, 0.3, 0.1],
                portamento_ms: 20.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.15,
                delay_ms: 120.0,
                delay_feedback: 0.3,
                master_volume: 0.75,
                bit_depth: 10.0,
                rate_crush: 0.2,
                chorus_mix: 0.1,
                chorus_rate: 1.0,
                chorus_depth: 0.2,
                waveshaper_drive: 4.0,
                waveshaper_mix: 0.5,
                ..Default::default()
            },
            van_der_pol: VanDerPolConfig { mu: 3.0 },
            ..Default::default()
        },

        "Planetary Clockwork" => Config {
            system: SystemConfig {
                name: "three_body".into(),
                dt: 0.001,
                speed: 0.8,
            },
            sonification: SonificationConfig {
                mode: "orbital".into(),
                scale: "just_intonation".into(),
                base_frequency: 110.0,
                octave_range: 3.0,
                chord_mode: "dom7".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.7, 0.5],
                portamento_ms: 200.0,
                voice_shapes: [
                    "sine".into(),
                    "sine".into(),
                    "triangle".into(),
                    "sine".into(),
                ],
            },
            audio: AudioConfig {
                reverb_wet: 0.55,
                delay_ms: 400.0,
                delay_feedback: 0.4,
                master_volume: 0.72,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.2,
                chorus_rate: 0.2,
                chorus_depth: 0.4,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            ..Default::default()
        },

        "Industrial Heartbeat" => Config {
            system: SystemConfig {
                name: "double_pendulum".into(),
                dt: 0.001,
                speed: 1.5,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "pentatonic".into(),
                base_frequency: 80.0,
                octave_range: 2.5,
                chord_mode: "power".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 30.0,
                voice_shapes: [
                    "saw".into(),
                    "triangle".into(),
                    "sine".into(),
                    "sine".into(),
                ],
            },
            audio: AudioConfig {
                reverb_wet: 0.30,
                delay_ms: 180.0,
                delay_feedback: 0.35,
                master_volume: 0.76,
                bit_depth: 10.0,
                rate_crush: 0.0,
                chorus_mix: 0.1,
                chorus_rate: 0.5,
                chorus_depth: 0.3,
                waveshaper_drive: 3.5,
                waveshaper_mix: 0.7,
                ..Default::default()
            },
            ..Default::default()
        },

        "Bone Structure" => Config {
            system: SystemConfig {
                name: "chua".into(),
                dt: 0.0005,
                speed: 1.8,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "pentatonic".into(),
                base_frequency: 110.0,
                octave_range: 2.5,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.5, 0.3],
                portamento_ms: 40.0,
                voice_shapes: [
                    "sine".into(),
                    "triangle".into(),
                    "sine".into(),
                    "sine".into(),
                ],
            },
            audio: AudioConfig {
                reverb_wet: 0.35,
                delay_ms: 200.0,
                delay_feedback: 0.35,
                master_volume: 0.74,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.15,
                chorus_rate: 0.4,
                chorus_depth: 0.3,
                waveshaper_drive: 2.0,
                waveshaper_mix: 0.4,
                ..Default::default()
            },
            chua: ChuaConfig {
                alpha: 15.6,
                beta: 28.0,
                m0: -1.143,
                m1: -0.714,
            },
            ..Default::default()
        },

        // ------------------------------------------------------------------ MELODIC
        "Glass Harp" => Config {
            system: SystemConfig {
                name: "aizawa".into(),
                dt: 0.002,
                speed: 1.0,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "pentatonic".into(),
                base_frequency: 440.0,
                octave_range: 3.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.0, 0.0, 0.0],
                portamento_ms: 250.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.60,
                delay_ms: 300.0,
                delay_feedback: 0.3,
                master_volume: 0.68,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.2,
                chorus_rate: 0.2,
                chorus_depth: 0.3,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            aizawa: AizawaConfig {
                a: 0.95,
                b: 0.7,
                c: 0.6,
                d: 3.5,
                e: 0.25,
                f: 0.1,
            },
            ..Default::default()
        },

        "Electric Kelp" => Config {
            system: SystemConfig {
                name: "rossler".into(),
                dt: 0.002,
                speed: 0.9,
            },
            sonification: SonificationConfig {
                mode: "fm".into(),
                scale: "pentatonic".into(),
                base_frequency: 110.0,
                octave_range: 3.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.6, 0.4, 0.2],
                portamento_ms: 180.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.58,
                delay_ms: 350.0,
                delay_feedback: 0.35,
                master_volume: 0.70,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.4,
                chorus_rate: 0.18,
                chorus_depth: 0.5,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            rossler: RosslerConfig {
                a: 0.15,
                b: 0.2,
                c: 7.0,
            },
            ..Default::default()
        },

        "The Butterfly's Aria" => Config {
            system: SystemConfig {
                name: "lorenz".into(),
                dt: 0.001,
                speed: 0.8,
            },
            sonification: SonificationConfig {
                mode: "orbital".into(),
                scale: "pentatonic".into(),
                base_frequency: 220.0,
                octave_range: 2.5,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.6, 0.4, 0.2],
                portamento_ms: 300.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.62,
                delay_ms: 350.0,
                delay_feedback: 0.35,
                master_volume: 0.70,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.3,
                chorus_rate: 0.15,
                chorus_depth: 0.4,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            lorenz: LorenzConfig {
                sigma: 10.0,
                rho: 24.5,
                beta: 2.6667,
            },
            ..Default::default()
        },

        "Solar Wind" => Config {
            system: SystemConfig {
                name: "halvorsen".into(),
                dt: 0.001,
                speed: 1.3,
            },
            sonification: SonificationConfig {
                mode: "fm".into(),
                scale: "chromatic".into(),
                base_frequency: 220.0,
                octave_range: 3.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 120.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.55,
                delay_ms: 250.0,
                delay_feedback: 0.3,
                master_volume: 0.72,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.6,
                chorus_rate: 0.2,
                chorus_depth: 0.6,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            halvorsen: HalvorsenConfig { a: 1.89 },
            ..Default::default()
        },

        "Möbius Lead" => Config {
            system: SystemConfig {
                name: "aizawa".into(),
                dt: 0.002,
                speed: 1.4,
            },
            sonification: SonificationConfig {
                mode: "orbital".into(),
                scale: "chromatic".into(),
                base_frequency: 330.0,
                octave_range: 2.5,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.0, 0.0, 0.0],
                portamento_ms: 200.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.45,
                delay_ms: 280.0,
                delay_feedback: 0.3,
                master_volume: 0.70,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.2,
                chorus_rate: 0.25,
                chorus_depth: 0.3,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            aizawa: AizawaConfig {
                a: 0.95,
                b: 0.7,
                c: 0.6,
                d: 3.5,
                e: 0.25,
                f: 0.1,
            },
            ..Default::default()
        },

        // ------------------------------------------------------------------ CINEMATIC
        "Last Light" => Config {
            system: SystemConfig {
                name: "rossler".into(),
                dt: 0.002,
                speed: 0.5,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "just_intonation".into(),
                base_frequency: 82.0,
                octave_range: 3.5,
                chord_mode: "major".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.7, 0.5],
                portamento_ms: 600.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.85,
                delay_ms: 700.0,
                delay_feedback: 0.55,
                master_volume: 0.68,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.2,
                chorus_rate: 0.08,
                chorus_depth: 0.5,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            rossler: RosslerConfig {
                a: 0.2,
                b: 0.2,
                c: 4.0,
            },
            ..Default::default()
        },

        "Seismic Event" => Config {
            system: SystemConfig {
                name: "three_body".into(),
                dt: 0.001,
                speed: 0.6,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "pentatonic".into(),
                base_frequency: 50.0,
                octave_range: 2.0,
                chord_mode: "power".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.3],
                portamento_ms: 200.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.60,
                delay_ms: 500.0,
                delay_feedback: 0.4,
                master_volume: 0.72,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.2,
                chorus_rate: 0.08,
                chorus_depth: 0.4,
                waveshaper_drive: 2.5,
                waveshaper_mix: 0.4,
                ..Default::default()
            },
            ..Default::default()
        },

        "Ancient Algorithm" => Config {
            system: SystemConfig {
                name: "three_body".into(),
                dt: 0.001,
                speed: 0.7,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "microtonal".into(),
                base_frequency: 100.0,
                octave_range: 4.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4],
                portamento_ms: 400.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.80,
                delay_ms: 600.0,
                delay_feedback: 0.5,
                master_volume: 0.70,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.3,
                chorus_rate: 0.1,
                chorus_depth: 0.4,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            ..Default::default()
        },

        "Cathedral Organ" => Config {
            system: SystemConfig {
                name: "lorenz".into(),
                dt: 0.001,
                speed: 0.7,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "just_intonation".into(),
                base_frequency: 55.0,
                octave_range: 3.0,
                chord_mode: "major".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.8, 0.7],
                portamento_ms: 350.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.82,
                delay_ms: 450.0,
                delay_feedback: 0.4,
                master_volume: 0.72,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.25,
                chorus_rate: 0.1,
                chorus_depth: 0.45,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            lorenz: LorenzConfig {
                sigma: 10.0,
                rho: 28.0,
                beta: 2.6667,
            },
            ..Default::default()
        },

        // ------------------------------------------------------------------ EXPERIMENTAL
        "Neon Labyrinth" => Config {
            system: SystemConfig {
                name: "chua".into(),
                dt: 0.0004,
                speed: 2.5,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "chromatic".into(),
                base_frequency: 200.0,
                octave_range: 4.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4],
                portamento_ms: 80.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.40,
                delay_ms: 150.0,
                delay_feedback: 0.5,
                master_volume: 0.70,
                bit_depth: 12.0,
                rate_crush: 0.15,
                chorus_mix: 0.2,
                chorus_rate: 1.0,
                chorus_depth: 0.4,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            chua: ChuaConfig {
                alpha: 15.6,
                beta: 28.0,
                m0: -1.143,
                m1: -0.714,
            },
            ..Default::default()
        },

        "Dissociation" => Config {
            system: SystemConfig {
                name: "double_pendulum".into(),
                dt: 0.001,
                speed: 3.0,
            },
            sonification: SonificationConfig {
                mode: "granular".into(),
                scale: "chromatic".into(),
                base_frequency: 180.0,
                octave_range: 4.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 40.0,
                voice_shapes: ["saw".into(), "triangle".into(), "saw".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.45,
                delay_ms: 100.0,
                delay_feedback: 0.5,
                master_volume: 0.72,
                bit_depth: 10.0,
                rate_crush: 0.2,
                chorus_mix: 0.2,
                chorus_rate: 2.0,
                chorus_depth: 0.4,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            ..Default::default()
        },

        "Tungsten Filament" => Config {
            system: SystemConfig {
                name: "chua".into(),
                dt: 0.0005,
                speed: 1.8,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "chromatic".into(),
                base_frequency: 120.0,
                octave_range: 3.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4],
                portamento_ms: 60.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.30,
                delay_ms: 150.0,
                delay_feedback: 0.4,
                master_volume: 0.72,
                bit_depth: 14.0,
                rate_crush: 0.0,
                chorus_mix: 0.15,
                chorus_rate: 0.8,
                chorus_depth: 0.3,
                waveshaper_drive: 7.0,
                waveshaper_mix: 0.8,
                ..Default::default()
            },
            chua: ChuaConfig {
                alpha: 15.6,
                beta: 28.0,
                m0: -1.143,
                m1: -0.714,
            },
            ..Default::default()
        },

        "The Double Scroll" => Config {
            system: SystemConfig {
                name: "chua".into(),
                dt: 0.0005,
                speed: 2.0,
            },
            sonification: SonificationConfig {
                mode: "orbital".into(),
                scale: "chromatic".into(),
                base_frequency: 150.0,
                octave_range: 3.5,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 40.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.25,
                delay_ms: 120.0,
                delay_feedback: 0.45,
                master_volume: 0.74,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.15,
                chorus_rate: 0.6,
                chorus_depth: 0.3,
                waveshaper_drive: 5.0,
                waveshaper_mix: 0.7,
                ..Default::default()
            },
            chua: ChuaConfig {
                alpha: 15.6,
                beta: 28.0,
                m0: -1.143,
                m1: -0.714,
            },
            ..Default::default()
        },

        // ------------------------------------------------------------------ MEDITATIVE
        "Memory of Water" => Config {
            system: SystemConfig {
                name: "lorenz".into(),
                dt: 0.0005,
                speed: 0.4,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "just_intonation".into(),
                base_frequency: 55.0,
                octave_range: 2.0,
                chord_mode: "major".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 1000.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.88,
                delay_ms: 900.0,
                delay_feedback: 0.6,
                master_volume: 0.65,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.3,
                chorus_rate: 0.06,
                chorus_depth: 0.6,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            lorenz: LorenzConfig {
                sigma: 10.0,
                rho: 28.0,
                beta: 2.6667,
            },
            ..Default::default()
        },

        "Monk's Bell" => Config {
            system: SystemConfig {
                name: "double_pendulum".into(),
                dt: 0.001,
                speed: 0.7,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "pentatonic".into(),
                base_frequency: 200.0,
                octave_range: 2.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.6, 0.3, 0.1],
                portamento_ms: 500.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.75,
                delay_ms: 800.0,
                delay_feedback: 0.4,
                master_volume: 0.68,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.2,
                chorus_rate: 0.1,
                chorus_depth: 0.4,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            ..Default::default()
        },

        "Deep Hypnosis" => Config {
            system: SystemConfig {
                name: "geodesic_torus".into(),
                dt: 0.01,
                speed: 0.2,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "microtonal".into(),
                base_frequency: 40.0,
                octave_range: 1.5,
                chord_mode: "major".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 1500.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.90,
                delay_ms: 1200.0,
                delay_feedback: 0.65,
                master_volume: 0.62,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.25,
                chorus_rate: 0.04,
                chorus_depth: 0.65,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            geodesic_torus: GeodesicTorusConfig { big_r: 2.5, r: 1.2 },
            ..Default::default()
        },

        "Aurora Borealis" => Config {
            system: SystemConfig {
                name: "lorenz".into(),
                dt: 0.001,
                speed: 0.6,
            },
            sonification: SonificationConfig {
                mode: "fm".into(),
                scale: "pentatonic".into(),
                base_frequency: 110.0,
                octave_range: 3.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 400.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.78,
                delay_ms: 400.0,
                delay_feedback: 0.4,
                master_volume: 0.70,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.7,
                chorus_rate: 0.15,
                chorus_depth: 0.7,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            lorenz: LorenzConfig {
                sigma: 12.0,
                rho: 30.0,
                beta: 2.6667,
            },
            ..Default::default()
        },

        "The Synchronization" => Config {
            system: SystemConfig {
                name: "kuramoto".into(),
                dt: 0.002,
                speed: 1.0,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "pentatonic".into(),
                base_frequency: 165.0,
                octave_range: 2.5,
                chord_mode: "major".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.8, 0.8],
                portamento_ms: 200.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.55,
                delay_ms: 300.0,
                delay_feedback: 0.35,
                master_volume: 0.72,
                bit_depth: 24.0,
                rate_crush: 0.0,
                chorus_mix: 0.35,
                chorus_rate: 0.2,
                chorus_depth: 0.4,
                waveshaper_drive: 1.0,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            kuramoto: KuramotoConfig {
                n_oscillators: 8,
                coupling: 2.5,
            },
            ..Default::default()
        },

        // ------------------------------------------------------------------ NEW SYSTEMS
        "Cyclic Tangle" => Config {
            system: SystemConfig {
                name: "thomas".into(),
                dt: 0.01,
                speed: 0.5,
            },
            sonification: SonificationConfig {
                mode: "fm".into(),
                scale: "pentatonic".into(),
                base_frequency: 110.0,
                octave_range: 3.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.6, 0.4, 0.2],
                portamento_ms: 180.0,
                voice_shapes: ["triangle".into(), "sine".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.45,
                delay_ms: 220.0,
                delay_feedback: 0.4,
                master_volume: 0.72,
                chorus_mix: 0.25,
                chorus_rate: 0.3,
                chorus_depth: 0.35,
                waveshaper_drive: 2.0,
                waveshaper_mix: 0.15,
                ..Default::default()
            },
            thomas: ThomasConfig { b: 0.208186 },
            ..Default::default()
        },
        "Polarity Reversal" => Config {
            system: SystemConfig {
                name: "rikitake".into(),
                dt: 0.001,
                speed: 1.2,
            },
            sonification: SonificationConfig {
                mode: "orbital".into(),
                scale: "chromatic".into(),
                base_frequency: 82.4,
                octave_range: 3.5,
                chord_mode: "none".into(),
                transpose_semitones: -12.0,
                voice_levels: [1.0, 0.8, 0.5, 0.2],
                portamento_ms: 60.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.6,
                delay_ms: 180.0,
                delay_feedback: 0.55,
                master_volume: 0.70,
                chorus_mix: 0.1,
                waveshaper_drive: 4.0,
                waveshaper_mix: 0.4,
                ..Default::default()
            },
            rikitake: RikitakeConfig { mu: 1.0, a: 5.0 },
            ..Default::default()
        },
        "Anti-Lorenz" => Config {
            system: SystemConfig {
                name: "chen".into(),
                dt: 0.0005,
                speed: 1.0,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "chromatic".into(),
                base_frequency: 220.0,
                octave_range: 3.0,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3],
                portamento_ms: 40.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.3,
                delay_ms: 140.0,
                delay_feedback: 0.4,
                master_volume: 0.73,
                chorus_mix: 0.2,
                chorus_rate: 0.5,
                chorus_depth: 0.3,
                waveshaper_drive: 3.5,
                waveshaper_mix: 0.35,
                ..Default::default()
            },
            chen: ChenConfig { a: 40.0, b: 3.0, c: 28.0 },
            ..Default::default()
        },
        "Double Convection" => Config {
            system: SystemConfig {
                name: "rucklidge".into(),
                dt: 0.002,
                speed: 0.8,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "just_intonation".into(),
                base_frequency: 165.0,
                octave_range: 2.5,
                chord_mode: "power".into(),
                transpose_semitones: 5.0,
                voice_levels: [1.0, 0.7, 0.4, 0.0],
                portamento_ms: 120.0,
                voice_shapes: ["sine".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.5,
                delay_ms: 300.0,
                delay_feedback: 0.35,
                master_volume: 0.71,
                chorus_mix: 0.3,
                chorus_rate: 0.2,
                chorus_depth: 0.4,
                waveshaper_drive: 1.5,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            rucklidge: RucklidgeConfig { kappa: 2.0, lambda: 6.7 },
            ..Default::default()
        },
        "Mirror Attractor" => Config {
            system: SystemConfig {
                name: "shimizu_morioka".into(),
                dt: 0.002,
                speed: 0.9,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "phrygian".into(),
                base_frequency: 110.0,
                octave_range: 3.0,
                chord_mode: "minor".into(),
                transpose_semitones: -3.0,
                voice_levels: [1.0, 0.8, 0.5, 0.2],
                portamento_ms: 80.0,
                voice_shapes: ["triangle".into(), "sine".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.55,
                delay_ms: 250.0,
                delay_feedback: 0.38,
                master_volume: 0.72,
                chorus_mix: 0.35,
                chorus_rate: 0.3,
                chorus_depth: 0.45,
                waveshaper_drive: 2.0,
                waveshaper_mix: 0.15,
                ..Default::default()
            },
            shimizu_morioka: ShimizuMoriokaConfig { a: 0.75, b: 0.45 },
            ..Default::default()
        },
        "Xz Knot" => Config {
            system: SystemConfig {
                name: "sprott_d".into(),
                dt: 0.005,
                speed: 1.0,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "chromatic".into(),
                base_frequency: 220.0,
                octave_range: 3.5,
                chord_mode: "none".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.6, 0.4, 0.2],
                portamento_ms: 30.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.25,
                delay_ms: 110.0,
                delay_feedback: 0.4,
                master_volume: 0.75,
                chorus_mix: 0.15,
                chorus_rate: 0.8,
                chorus_depth: 0.2,
                waveshaper_drive: 4.0,
                waveshaper_mix: 0.4,
                ..Default::default()
            },
            ..Default::default()
        },
        "Equilibrium Fugue" => Config {
            system: SystemConfig {
                name: "sprott_e".into(),
                dt: 0.005,
                speed: 0.8,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "major".into(),
                base_frequency: 261.63,
                octave_range: 2.5,
                chord_mode: "major".into(),
                transpose_semitones: 2.0,
                voice_levels: [1.0, 0.75, 0.5, 0.25],
                portamento_ms: 60.0,
                voice_shapes: ["sine".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.4,
                delay_ms: 200.0,
                delay_feedback: 0.3,
                master_volume: 0.7,
                chorus_mix: 0.2,
                chorus_rate: 0.4,
                chorus_depth: 0.35,
                waveshaper_drive: 1.5,
                waveshaper_mix: 0.1,
                ..Default::default()
            },
            ..Default::default()
        },
        "Half-Speed Spiral" => Config {
            system: SystemConfig {
                name: "sprott_f".into(),
                dt: 0.005,
                speed: 0.7,
            },
            sonification: SonificationConfig {
                mode: "orbital".into(),
                scale: "pentatonic".into(),
                base_frequency: 130.81,
                octave_range: 2.0,
                chord_mode: "minor".into(),
                transpose_semitones: -5.0,
                voice_levels: [1.0, 0.8, 0.55, 0.3],
                portamento_ms: 100.0,
                voice_shapes: ["triangle".into(), "sine".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.6,
                delay_ms: 320.0,
                delay_feedback: 0.45,
                master_volume: 0.7,
                chorus_mix: 0.3,
                chorus_rate: 0.25,
                chorus_depth: 0.5,
                waveshaper_drive: 1.2,
                waveshaper_mix: 0.0,
                ..Default::default()
            },
            ..Default::default()
        },
        "Jerk Circuit" => Config {
            system: SystemConfig {
                name: "genesio_tesi".into(),
                dt: 0.005,
                speed: 1.1,
            },
            sonification: SonificationConfig {
                mode: "fm".into(),
                scale: "chromatic".into(),
                base_frequency: 220.0,
                octave_range: 3.0,
                chord_mode: "none".into(),
                transpose_semitones: 3.0,
                voice_levels: [1.0, 0.7, 0.45, 0.2],
                portamento_ms: 25.0,
                voice_shapes: ["saw".into(), "triangle".into(), "saw".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.2,
                delay_ms: 90.0,
                delay_feedback: 0.45,
                master_volume: 0.74,
                chorus_mix: 0.1,
                chorus_rate: 1.0,
                chorus_depth: 0.15,
                waveshaper_drive: 5.0,
                waveshaper_mix: 0.45,
                ..Default::default()
            },
            genesio_tesi: GenesioTesiConfig { a: 1.2, b: 2.92, c: 6.0 },
            ..Default::default()
        },
        "Velocity Band" => Config {
            system: SystemConfig {
                name: "liu".into(),
                dt: 0.001,
                speed: 1.2,
            },
            sonification: SonificationConfig {
                mode: "direct".into(),
                scale: "chromatic".into(),
                base_frequency: 196.0,
                octave_range: 2.5,
                chord_mode: "power".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.75, 0.5, 0.25],
                portamento_ms: 35.0,
                voice_shapes: ["saw".into(), "triangle".into(), "saw".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.3,
                delay_ms: 130.0,
                delay_feedback: 0.4,
                master_volume: 0.75,
                chorus_mix: 0.15,
                chorus_rate: 0.6,
                chorus_depth: 0.25,
                waveshaper_drive: 3.0,
                waveshaper_mix: 0.3,
                ..Default::default()
            },
            liu: LiuConfig { a: 1.0, b: 2.5, c: 5.0, e: 1.0, k: 4.0, m: 4.0 },
            ..Default::default()
        },

        "Substorm" => Config {
            system: SystemConfig {
                name: "windmi".into(),
                dt: 0.01,
                speed: 0.8,
            },
            sonification: SonificationConfig {
                mode: "spectral".into(),
                scale: "whole_tone".into(),
                base_frequency: 110.0,
                octave_range: 3.5,
                chord_mode: "open_fifth".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.5, 0.2],
                portamento_ms: 60.0,
                voice_shapes: ["sine".into(), "sine".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.65,
                delay_ms: 320.0,
                delay_feedback: 0.5,
                master_volume: 0.72,
                chorus_mix: 0.2,
                chorus_rate: 0.3,
                chorus_depth: 0.4,
                waveshaper_drive: 1.5,
                waveshaper_mix: 0.1,
                ..Default::default()
            },
            windmi: WindmiConfig { a: 0.9, b: 2.5 },
            ..Default::default()
        },
        "Invisible Hand" => Config {
            system: SystemConfig {
                name: "finance".into(),
                dt: 0.002,
                speed: 1.0,
            },
            sonification: SonificationConfig {
                mode: "orbital".into(),
                scale: "pentatonic_minor".into(),
                base_frequency: 164.81,
                octave_range: 2.5,
                chord_mode: "minor".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.45, 0.2],
                portamento_ms: 80.0,
                voice_shapes: ["triangle".into(), "sine".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.5,
                delay_ms: 220.0,
                delay_feedback: 0.35,
                master_volume: 0.7,
                chorus_mix: 0.25,
                chorus_rate: 0.5,
                chorus_depth: 0.3,
                waveshaper_drive: 2.0,
                waveshaper_mix: 0.15,
                ..Default::default()
            },
            finance: FinanceConfig { a: 3.0, b: 0.1, c: 1.0 },
            ..Default::default()
        },
        "Hyperchaos Engine" => Config {
            system: SystemConfig {
                name: "hyperchaos".into(),
                dt: 0.0005,
                speed: 1.5,
            },
            sonification: SonificationConfig {
                mode: "fm".into(),
                scale: "chromatic".into(),
                base_frequency: 55.0,
                octave_range: 4.0,
                chord_mode: "cluster".into(),
                transpose_semitones: 0.0,
                voice_levels: [1.0, 0.85, 0.65, 0.45],
                portamento_ms: 20.0,
                voice_shapes: ["saw".into(), "saw".into(), "square".into(), "saw".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.4,
                delay_ms: 90.0,
                delay_feedback: 0.6,
                master_volume: 0.68,
                chorus_mix: 0.3,
                chorus_rate: 1.2,
                chorus_depth: 0.5,
                waveshaper_drive: 4.0,
                waveshaper_mix: 0.4,
                ..Default::default()
            },
            hyperchaos: HyperchaosConfig { a: 35.0, b: 3.0, c: 28.0, d: -7.0 },
            ..Default::default()
        },

        _ => Config::default(),
    }
}

/// Save a named patch to the patches/ directory, backing up any existing version.
pub fn save_patch(name: &str, config: &Config) {
    let dir = std::path::PathBuf::from("patches");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    let filename = dir.join(format!("{}.toml", sanitize_name(name)));
    // Backup existing patch before overwriting
    if filename.exists() {
        let bak_dir = dir.join(".bak");
        let _ = std::fs::create_dir_all(&bak_dir);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let bak_name = format!("{}_{}.toml", sanitize_name(name), ts);
        let _ = std::fs::copy(&filename, bak_dir.join(bak_name));
        prune_backups(&bak_dir, sanitize_name(name).as_str(), 5);
    }
    let toml_str = toml::to_string_pretty(config).unwrap_or_default();
    let _ = std::fs::write(&filename, toml_str);
}

fn prune_backups(bak_dir: &std::path::Path, base_name: &str, keep: usize) {
    let prefix = format!("{}_", base_name);
    let mut backups: Vec<std::path::PathBuf> = match std::fs::read_dir(bak_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let p = e.path();
                let fname = p.file_name()?.to_str()?.to_string();
                if fname.starts_with(&prefix) && fname.ends_with(".toml") {
                    Some(p)
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => return,
    };
    backups.sort();
    if backups.len() > keep {
        for old in &backups[..backups.len() - keep] {
            let _ = std::fs::remove_file(old);
        }
    }
}

/// Return (filename, timestamp) pairs for backups of a given patch name, oldest-first.
pub fn list_backups(name: &str) -> Vec<(String, u64)> {
    let bak_dir = std::path::PathBuf::from("patches").join(".bak");
    let prefix = format!("{}_", sanitize_name(name));
    let mut out: Vec<(String, u64)> = match std::fs::read_dir(&bak_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let p = e.path();
                let fname = p.file_name()?.to_str()?.to_string();
                if fname.starts_with(&prefix) && fname.ends_with(".toml") {
                    let stem = fname.strip_suffix(".toml")?;
                    let ts_str = stem.rsplit('_').next()?;
                    let ts: u64 = ts_str.parse().ok()?;
                    Some((fname, ts))
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort_by_key(|(_, ts)| *ts);
    out
}

/// Load a patch from the backup directory by filename.
pub fn load_backup(filename: &str) -> Option<Config> {
    let path = std::path::PathBuf::from("patches")
        .join(".bak")
        .join(filename);
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).ok(),
        Err(_) => None,
    }
}

/// List all saved patch names (without .toml extension).
pub fn list_patches() -> Vec<String> {
    let dir = std::path::PathBuf::from("patches");
    if !dir.exists() {
        return Vec::new();
    }
    match std::fs::read_dir(&dir) {
        Ok(entries) => {
            let mut names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().and_then(|x| x.to_str()) == Some("toml") {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
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
    let filename =
        std::path::PathBuf::from("patches").join(format!("{}.toml", sanitize_name(name)));
    match std::fs::read_to_string(&filename) {
        Ok(text) => toml::from_str(&text).ok(),
        Err(_) => None,
    }
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .replace(' ', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name_alphanumeric_passthrough() {
        assert_eq!(sanitize_name("hello123"), "hello123");
    }

    #[test]
    fn test_sanitize_name_spaces_become_underscores() {
        assert_eq!(sanitize_name("my patch"), "my_patch");
    }

    #[test]
    fn test_sanitize_name_special_chars_replaced() {
        assert_eq!(sanitize_name("foo/bar.baz"), "foo_bar_baz");
    }

    #[test]
    fn test_sanitize_name_dashes_and_underscores_kept() {
        assert_eq!(sanitize_name("my-patch_v2"), "my-patch_v2");
    }

    #[test]
    fn test_load_preset_unknown_returns_default() {
        let c = load_preset("nonexistent");
        let d = Config::default();
        assert_eq!(c.system.name, d.system.name, "unknown preset should return default system");
    }

    #[test]
    fn test_all_presets_loadable_and_valid() {
        for p in PRESETS {
            let mut c = load_preset(p.name);
            c.validate(); // should not panic
            assert!(!c.system.name.is_empty(), "system name empty for preset '{}'", p.name);
        }
    }

    #[test]
    fn test_preset_names_unique() {
        let mut seen = std::collections::HashSet::new();
        for p in PRESETS {
            assert!(seen.insert(p.name), "duplicate preset name: {}", p.name);
        }
    }

    #[test]
    fn test_preset_categories_nonempty() {
        for p in PRESETS {
            assert!(!p.category.is_empty(), "empty category for preset '{}'", p.name);
        }
    }
}
