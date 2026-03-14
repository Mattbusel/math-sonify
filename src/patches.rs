use crate::config::*;
use egui::Color32;

pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub color: egui::Color32,
}

pub const PRESETS: &[Preset] = &[
    // --- Original 11 ---
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

    // --- Techno / Industrial ---
    Preset { name: "Industrial Lorenz", description: "Distorted butterfly chaos — bitcrushed metallic grind", color: Color32::from_rgb(80, 80, 80) },
    Preset { name: "Duffing Techno", description: "Hard-driven Duffing oscillator — pounding four-on-the-floor pulse", color: Color32::from_rgb(200, 0, 30) },
    Preset { name: "Chua Factory", description: "Chua double-scroll hammered through waveshaper distortion", color: Color32::from_rgb(160, 0, 0) },
    Preset { name: "Van Der Pol Stomp", description: "Limit cycle grind — relentless rhythmic stomping saw waves", color: Color32::from_rgb(120, 40, 0) },
    Preset { name: "Aizawa Dark Rave", description: "Aizawa attractor at high speed — techno arpeggio chaos", color: Color32::from_rgb(40, 0, 80) },

    // --- Ambient / Space ---
    Preset { name: "Deep Space Lorenz", description: "Ultra-slow butterfly — vast reverberant cosmic drift", color: Color32::from_rgb(10, 20, 80) },
    Preset { name: "Nebula Torus", description: "Geodesic torus at glacial speed — spatial sine clouds", color: Color32::from_rgb(60, 0, 120) },
    Preset { name: "Aizawa Nebula", description: "Aizawa attractor whispered into long reverb tails", color: Color32::from_rgb(20, 60, 120) },
    Preset { name: "Halvorsen Void", description: "Sparse spiral decay — minimalist ambient texture", color: Color32::from_rgb(0, 40, 60) },
    Preset { name: "Rössler Aurora", description: "Slow Rössler spiral painted across a microtonal sky", color: Color32::from_rgb(0, 200, 180) },

    // --- Jazz / Harmonic ---
    Preset { name: "Kuramoto Bebop", description: "Oscillator coupling races through chromatic jazz changes", color: Color32::from_rgb(200, 160, 40) },
    Preset { name: "Halvorsen Modal", description: "Halvorsen spiral voiced in just intonation sus2 chords", color: Color32::from_rgb(180, 140, 60) },
    Preset { name: "Three-Body Ballad", description: "Slow three-body orbit with lush dom7 chord voicings", color: Color32::from_rgb(220, 200, 80) },
    Preset { name: "Aizawa Bossa", description: "Gentle Aizawa loop — pentatonic bossa groove", color: Color32::from_rgb(160, 200, 80) },
    Preset { name: "Rossler Cool Jazz", description: "Rössler c=4 drift over just intonation minor voicings", color: Color32::from_rgb(200, 180, 100) },

    // --- Glitch / Experimental ---
    Preset { name: "Lorenz Glitch", description: "Butterfly attractor decimated — bitcrushed granular chaos", color: Color32::from_rgb(0, 255, 100) },
    Preset { name: "Duffing Splice", description: "Duffing period-doubled into extreme rate-crushed fragments", color: Color32::from_rgb(255, 0, 180) },
    Preset { name: "Chua Corrupt", description: "Chua double-scroll melted through maximum waveshaper drive", color: Color32::from_rgb(255, 80, 0) },
    Preset { name: "Pendulum Stutter", description: "Double pendulum granular — hard-clipped stutter edit", color: Color32::from_rgb(0, 255, 200) },
    Preset { name: "Van Der Pol Mangle", description: "Limit cycle chewed through bitcrusher and chorus", color: Color32::from_rgb(200, 0, 255) },

    // --- Cinematic / Orchestral ---
    Preset { name: "Torus Strings", description: "Geodesic flow voiced as sweeping orchestral string pads", color: Color32::from_rgb(180, 60, 60) },
    Preset { name: "Lorenz Brass", description: "Butterfly chaos through triangle waves — cinematic brass swells", color: Color32::from_rgb(200, 120, 20) },
    Preset { name: "Three-Body Epic", description: "Gravitational chaos drives a massive orchestral climax", color: Color32::from_rgb(180, 40, 40) },
    Preset { name: "Halvorsen Score", description: "Halvorsen spiral unfolding over a lush cinematic soundscape", color: Color32::from_rgb(160, 80, 100) },
    Preset { name: "Aizawa Overture", description: "Aizawa attractor opening — slow build with deep reverb tails", color: Color32::from_rgb(120, 60, 160) },

    // --- Bass / Sub ---
    Preset { name: "Lorenz Sub", description: "Butterfly attractor locked into sub-bass saw drones", color: Color32::from_rgb(0, 80, 40) },
    Preset { name: "Duffing Bass", description: "Duffing oscillator in the sub-bass — heavy power chords", color: Color32::from_rgb(40, 60, 0) },
    Preset { name: "Chua Sub Rumble", description: "Chua double-scroll at sub frequency — chest-felt rumble", color: Color32::from_rgb(60, 0, 40) },
    Preset { name: "Van Der Pol Bass", description: "Van der Pol limit cycle voiced as a massive bass drone", color: Color32::from_rgb(20, 40, 20) },

    // --- Lead / Melody ---
    Preset { name: "Rossler Lead", description: "Rössler spiral as a plaintive pentatonic lead voice", color: Color32::from_rgb(255, 120, 180) },
    Preset { name: "Lorenz Melody", description: "Butterfly attractor tracing a chromatic melodic line", color: Color32::from_rgb(100, 200, 255) },
    Preset { name: "Kuramoto Lead", description: "Synchronized oscillators locked onto a single singing lead", color: Color32::from_rgb(160, 255, 160) },
    Preset { name: "Aizawa Flute", description: "Aizawa loop — delicate sine lead floating over sparse delay", color: Color32::from_rgb(200, 240, 200) },

    // --- Drone / Meditation ---
    Preset { name: "Torus Om", description: "Geodesic torus at near-zero speed — pure tonal meditation", color: Color32::from_rgb(80, 60, 140) },
    Preset { name: "Rössler Tanpura", description: "Rössler drone tuned to just intonation — Indian tanpura feel", color: Color32::from_rgb(180, 100, 40) },
    Preset { name: "Van Der Pol Throat", description: "Van der Pol limit cycle as a sustained throat-singing drone", color: Color32::from_rgb(100, 80, 60) },
    Preset { name: "Halvorsen Ohm", description: "Halvorsen spiral at minimal speed — crystalline sine drone", color: Color32::from_rgb(60, 120, 100) },

    // --- Arpeggio ---
    Preset { name: "Lorenz Arp", description: "Fast butterfly chaos stepping through pentatonic arpeggios", color: Color32::from_rgb(255, 200, 0) },
    Preset { name: "Duffing Arp", description: "Duffing chaos driving rapid chromatic arpeggio bursts", color: Color32::from_rgb(255, 160, 80) },
    Preset { name: "Kuramoto Arp", description: "Kuramoto coupling cascading through fast pentatonic steps", color: Color32::from_rgb(100, 255, 200) },
    Preset { name: "Aizawa Cascade", description: "Aizawa attractor rapid-fire microtonal arpeggio cascade", color: Color32::from_rgb(255, 100, 255) },

    // --- Hybrid / Unusual ---
    Preset { name: "Torus FM", description: "Geodesic torus trajectories feeding FM synthesis — alien bells", color: Color32::from_rgb(0, 220, 255) },
    Preset { name: "Lorenz Granular Cloud", description: "Butterfly attractor scattered into a granular texture cloud", color: Color32::from_rgb(180, 220, 255) },
    Preset { name: "Chua Spectral Web", description: "Chua double-scroll mapped to spectral synthesis — eerie web", color: Color32::from_rgb(200, 100, 200) },
    Preset { name: "Pendulum Orbital Funk", description: "Double pendulum orbital mode — syncopated microtonal funk", color: Color32::from_rgb(220, 180, 0) },
    Preset { name: "Three-Body Microtonal", description: "Three-body gravitational chaos through microtonal spectral mode", color: Color32::from_rgb(100, 180, 100) },
];

pub fn load_preset(name: &str) -> Config {
    match name {
        // ------------------------------------------------------------------ original 11
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
                base_frequency: 65.0, octave_range: 3.0,
                chord_mode: "major".into(), transpose_semitones: -7.0,
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

        // ------------------------------------------------------------------ Techno / Industrial
        "Industrial Lorenz" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 3.0 },
            sonification: SonificationConfig {
                mode: "fm".into(), scale: "chromatic".into(),
                base_frequency: 55.0, octave_range: 3.0,
                chord_mode: "power".into(), transpose_semitones: -12.0,
                voice_levels: [1.0, 0.9, 0.7, 0.6], portamento_ms: 20.0,
                voice_shapes: ["saw".into(), "saw".into(), "saw".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.2, delay_ms: 125.0, delay_feedback: 0.6,
                master_volume: 0.85, bit_depth: 8.0, rate_crush: 0.4,
                waveshaper_drive: 0.8, waveshaper_mix: 0.9,
                chorus_mix: 0.0, chorus_rate: 0.0, chorus_depth: 0.0,
                ..Default::default()
            },
            lorenz: LorenzConfig { sigma: 16.0, rho: 45.0, beta: 4.0 },
            ..Default::default()
        },
        "Duffing Techno" => Config {
            system: SystemConfig { name: "duffing".into(), dt: 0.0008, speed: 3.5 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "chromatic".into(),
                base_frequency: 60.0, octave_range: 2.0,
                chord_mode: "power".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.0, 0.0, 0.0], portamento_ms: 10.0,
                voice_shapes: ["saw".into(), "saw".into(), "saw".into(), "saw".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.1, delay_ms: 187.5, delay_feedback: 0.5,
                master_volume: 0.82, bit_depth: 16.0, rate_crush: 0.0,
                waveshaper_drive: 0.7, waveshaper_mix: 0.8,
                chorus_mix: 0.0, chorus_rate: 0.0, chorus_depth: 0.0,
                ..Default::default()
            },
            duffing: DuffingConfig { delta: 0.15, alpha: -1.0, beta: 1.0, gamma: 0.8, omega: 1.0 },
            ..Default::default()
        },
        "Chua Factory" => Config {
            system: SystemConfig { name: "chua".into(), dt: 0.0004, speed: 2.5 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "chromatic".into(),
                base_frequency: 80.0, octave_range: 2.5,
                chord_mode: "power".into(), transpose_semitones: -5.0,
                voice_levels: [1.0, 0.8, 0.6, 0.0], portamento_ms: 15.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.15, delay_ms: 150.0, delay_feedback: 0.55,
                master_volume: 0.85, bit_depth: 10.0, rate_crush: 0.3,
                waveshaper_drive: 0.9, waveshaper_mix: 1.0,
                chorus_mix: 0.1, chorus_rate: 0.5, chorus_depth: 0.2,
                ..Default::default()
            },
            chua: ChuaConfig { alpha: 15.6, beta: 35.0, m0: -1.143, m1: -0.714 },
            ..Default::default()
        },
        "Van Der Pol Stomp" => Config {
            system: SystemConfig { name: "van_der_pol".into(), dt: 0.001, speed: 2.8 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 80.0, octave_range: 2.0,
                chord_mode: "power".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.0, 0.0], portamento_ms: 15.0,
                voice_shapes: ["saw".into(), "saw".into(), "saw".into(), "saw".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.1, delay_ms: 250.0, delay_feedback: 0.4,
                master_volume: 0.80, bit_depth: 16.0, rate_crush: 0.0,
                waveshaper_drive: 0.6, waveshaper_mix: 0.7,
                chorus_mix: 0.0, chorus_rate: 0.0, chorus_depth: 0.0,
                ..Default::default()
            },
            van_der_pol: VanDerPolConfig { mu: 3.5 },
            ..Default::default()
        },
        "Aizawa Dark Rave" => Config {
            system: SystemConfig { name: "aizawa".into(), dt: 0.001, speed: 4.0 },
            sonification: SonificationConfig {
                mode: "fm".into(), scale: "chromatic".into(),
                base_frequency: 110.0, octave_range: 3.0,
                chord_mode: "minor".into(), transpose_semitones: -5.0,
                voice_levels: [1.0, 0.8, 0.5, 0.3], portamento_ms: 10.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.2, delay_ms: 100.0, delay_feedback: 0.65,
                master_volume: 0.85, bit_depth: 12.0, rate_crush: 0.2,
                waveshaper_drive: 0.75, waveshaper_mix: 0.85,
                chorus_mix: 0.2, chorus_rate: 1.0, chorus_depth: 0.3,
                ..Default::default()
            },
            aizawa: AizawaConfig { a: 0.95, b: 0.7, c: 0.6, d: 3.5, e: 0.25, f: 0.1 },
            ..Default::default()
        },

        // ------------------------------------------------------------------ Ambient / Space
        "Deep Space Lorenz" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.0005, speed: 0.3 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 55.0, octave_range: 4.0,
                chord_mode: "major".into(), transpose_semitones: -12.0,
                voice_levels: [1.0, 0.6, 0.4, 0.2], portamento_ms: 800.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.9, delay_ms: 900.0, delay_feedback: 0.5,
                master_volume: 0.55, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.4, chorus_rate: 0.1, chorus_depth: 0.6,
                ..Default::default()
            },
            lorenz: LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 },
            ..Default::default()
        },
        "Nebula Torus" => Config {
            system: SystemConfig { name: "geodesic_torus".into(), dt: 0.01, speed: 0.2 },
            sonification: SonificationConfig {
                mode: "granular".into(), scale: "just_intonation".into(),
                base_frequency: 80.0, octave_range: 3.5,
                chord_mode: "major".into(), transpose_semitones: -7.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3], portamento_ms: 600.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.88, delay_ms: 750.0, delay_feedback: 0.45,
                master_volume: 0.6, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.5, chorus_rate: 0.08, chorus_depth: 0.7,
                ..Default::default()
            },
            geodesic_torus: GeodesicTorusConfig { big_r: 4.0, r: 0.8 },
            ..Default::default()
        },
        "Aizawa Nebula" => Config {
            system: SystemConfig { name: "aizawa".into(), dt: 0.002, speed: 0.4 },
            sonification: SonificationConfig {
                mode: "granular".into(), scale: "pentatonic".into(),
                base_frequency: 80.0, octave_range: 3.0,
                chord_mode: "sus2".into(), transpose_semitones: -7.0,
                voice_levels: [1.0, 0.6, 0.4, 0.2], portamento_ms: 700.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.85, delay_ms: 800.0, delay_feedback: 0.42,
                master_volume: 0.58, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.45, chorus_rate: 0.12, chorus_depth: 0.55,
                ..Default::default()
            },
            aizawa: AizawaConfig { a: 0.95, b: 0.7, c: 0.6, d: 3.5, e: 0.25, f: 0.1 },
            ..Default::default()
        },
        "Halvorsen Void" => Config {
            system: SystemConfig { name: "halvorsen".into(), dt: 0.002, speed: 0.25 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 80.0, octave_range: 2.5,
                chord_mode: "none".into(), transpose_semitones: -7.0,
                voice_levels: [1.0, 0.3, 0.1, 0.0], portamento_ms: 900.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.92, delay_ms: 1100.0, delay_feedback: 0.35,
                master_volume: 0.5, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.35, chorus_rate: 0.06, chorus_depth: 0.5,
                ..Default::default()
            },
            halvorsen: HalvorsenConfig { a: 1.89 },
            ..Default::default()
        },
        "Rössler Aurora" => Config {
            system: SystemConfig { name: "rossler".into(), dt: 0.003, speed: 0.35 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "microtonal".into(),
                base_frequency: 90.0, octave_range: 4.5,
                chord_mode: "sus2".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3], portamento_ms: 600.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.82, delay_ms: 650.0, delay_feedback: 0.4,
                master_volume: 0.62, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.4, chorus_rate: 0.15, chorus_depth: 0.6,
                ..Default::default()
            },
            rossler: RosslerConfig { a: 0.1, b: 0.1, c: 14.0 },
            ..Default::default()
        },

        // ------------------------------------------------------------------ Jazz / Harmonic
        "Kuramoto Bebop" => Config {
            system: SystemConfig { name: "kuramoto".into(), dt: 0.001, speed: 2.0 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "chromatic".into(),
                base_frequency: 146.83, octave_range: 3.0,
                chord_mode: "dom7".into(), transpose_semitones: 2.0,
                voice_levels: [1.0, 0.85, 0.7, 0.55], portamento_ms: 45.0,
                voice_shapes: ["sine".into(), "triangle".into(), "sine".into(), "triangle".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.4, delay_ms: 220.0, delay_feedback: 0.35,
                master_volume: 0.72, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.1, waveshaper_mix: 0.15,
                chorus_mix: 0.2, chorus_rate: 0.3, chorus_depth: 0.3,
                ..Default::default()
            },
            kuramoto: KuramotoConfig { n_oscillators: 12, coupling: 0.8 },
            ..Default::default()
        },
        "Halvorsen Modal" => Config {
            system: SystemConfig { name: "halvorsen".into(), dt: 0.001, speed: 1.2 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "just_intonation".into(),
                base_frequency: 130.81, octave_range: 3.0,
                chord_mode: "sus2".into(), transpose_semitones: 5.0,
                voice_levels: [1.0, 0.8, 0.65, 0.5], portamento_ms: 120.0,
                voice_shapes: ["sine".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.5, delay_ms: 370.0, delay_feedback: 0.38,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.05, waveshaper_mix: 0.08,
                chorus_mix: 0.25, chorus_rate: 0.25, chorus_depth: 0.35,
                ..Default::default()
            },
            halvorsen: HalvorsenConfig { a: 1.89 },
            ..Default::default()
        },
        "Three-Body Ballad" => Config {
            system: SystemConfig { name: "three_body".into(), dt: 0.001, speed: 0.7 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 110.0, octave_range: 2.5,
                chord_mode: "dom7".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.85, 0.7, 0.55], portamento_ms: 250.0,
                voice_shapes: ["sine".into(), "sine".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.6, delay_ms: 500.0, delay_feedback: 0.38,
                master_volume: 0.68, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.3, chorus_rate: 0.2, chorus_depth: 0.4,
                ..Default::default()
            },
            ..Default::default()
        },
        "Aizawa Bossa" => Config {
            system: SystemConfig { name: "aizawa".into(), dt: 0.002, speed: 1.1 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 196.0, octave_range: 2.5,
                chord_mode: "major".into(), transpose_semitones: 3.0,
                voice_levels: [1.0, 0.75, 0.55, 0.35], portamento_ms: 90.0,
                voice_shapes: ["sine".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.48, delay_ms: 290.0, delay_feedback: 0.32,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.3, chorus_rate: 0.28, chorus_depth: 0.4,
                ..Default::default()
            },
            aizawa: AizawaConfig { a: 0.95, b: 0.7, c: 0.6, d: 3.5, e: 0.25, f: 0.1 },
            ..Default::default()
        },
        "Rossler Cool Jazz" => Config {
            system: SystemConfig { name: "rossler".into(), dt: 0.002, speed: 1.0 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "just_intonation".into(),
                base_frequency: 174.61, octave_range: 3.0,
                chord_mode: "minor".into(), transpose_semitones: -2.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4], portamento_ms: 80.0,
                voice_shapes: ["sine".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.45, delay_ms: 340.0, delay_feedback: 0.36,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.05, waveshaper_mix: 0.1,
                chorus_mix: 0.2, chorus_rate: 0.2, chorus_depth: 0.3,
                ..Default::default()
            },
            rossler: RosslerConfig { a: 0.2, b: 0.2, c: 4.0 },
            ..Default::default()
        },

        // ------------------------------------------------------------------ Glitch / Experimental
        "Lorenz Glitch" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 2.5 },
            sonification: SonificationConfig {
                mode: "granular".into(), scale: "chromatic".into(),
                base_frequency: 220.0, octave_range: 4.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.6, 0.4, 0.2], portamento_ms: 5.0,
                voice_shapes: ["saw".into(), "triangle".into(), "saw".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.25, delay_ms: 80.0, delay_feedback: 0.7,
                master_volume: 0.78, bit_depth: 6.0, rate_crush: 0.65,
                waveshaper_drive: 0.5, waveshaper_mix: 0.6,
                chorus_mix: 0.3, chorus_rate: 2.0, chorus_depth: 0.5,
                ..Default::default()
            },
            lorenz: LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 },
            ..Default::default()
        },
        "Duffing Splice" => Config {
            system: SystemConfig { name: "duffing".into(), dt: 0.0006, speed: 3.0 },
            sonification: SonificationConfig {
                mode: "granular".into(), scale: "microtonal".into(),
                base_frequency: 440.0, octave_range: 4.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.4, 0.2], portamento_ms: 5.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.2, delay_ms: 60.0, delay_feedback: 0.75,
                master_volume: 0.8, bit_depth: 5.0, rate_crush: 0.75,
                waveshaper_drive: 0.8, waveshaper_mix: 0.9,
                chorus_mix: 0.1, chorus_rate: 3.0, chorus_depth: 0.4,
                ..Default::default()
            },
            duffing: DuffingConfig { delta: 0.05, alpha: -1.0, beta: 1.0, gamma: 0.9, omega: 1.5 },
            ..Default::default()
        },
        "Chua Corrupt" => Config {
            system: SystemConfig { name: "chua".into(), dt: 0.0003, speed: 2.0 },
            sonification: SonificationConfig {
                mode: "fm".into(), scale: "chromatic".into(),
                base_frequency: 330.0, octave_range: 3.5,
                chord_mode: "none".into(), transpose_semitones: 6.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4], portamento_ms: 8.0,
                voice_shapes: ["saw".into(), "saw".into(), "saw".into(), "triangle".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.15, delay_ms: 90.0, delay_feedback: 0.65,
                master_volume: 0.82, bit_depth: 7.0, rate_crush: 0.55,
                waveshaper_drive: 1.0, waveshaper_mix: 1.0,
                chorus_mix: 0.2, chorus_rate: 2.5, chorus_depth: 0.6,
                ..Default::default()
            },
            chua: ChuaConfig { alpha: 15.6, beta: 28.0, m0: -1.143, m1: -0.714 },
            ..Default::default()
        },
        "Pendulum Stutter" => Config {
            system: SystemConfig { name: "double_pendulum".into(), dt: 0.001, speed: 3.0 },
            sonification: SonificationConfig {
                mode: "granular".into(), scale: "chromatic".into(),
                base_frequency: 880.0, octave_range: 2.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.5, 0.0, 0.0], portamento_ms: 3.0,
                voice_shapes: ["saw".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.12, delay_ms: 50.0, delay_feedback: 0.8,
                master_volume: 0.8, bit_depth: 8.0, rate_crush: 0.6,
                waveshaper_drive: 0.6, waveshaper_mix: 0.7,
                chorus_mix: 0.15, chorus_rate: 4.0, chorus_depth: 0.3,
                ..Default::default()
            },
            ..Default::default()
        },
        "Van Der Pol Mangle" => Config {
            system: SystemConfig { name: "van_der_pol".into(), dt: 0.001, speed: 2.2 },
            sonification: SonificationConfig {
                mode: "fm".into(), scale: "microtonal".into(),
                base_frequency: 260.0, octave_range: 3.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.75, 0.5, 0.3], portamento_ms: 10.0,
                voice_shapes: ["saw".into(), "saw".into(), "triangle".into(), "saw".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.2, delay_ms: 70.0, delay_feedback: 0.7,
                master_volume: 0.78, bit_depth: 9.0, rate_crush: 0.5,
                waveshaper_drive: 0.7, waveshaper_mix: 0.8,
                chorus_mix: 0.4, chorus_rate: 3.5, chorus_depth: 0.55,
                ..Default::default()
            },
            van_der_pol: VanDerPolConfig { mu: 4.0 },
            ..Default::default()
        },

        // ------------------------------------------------------------------ Cinematic / Orchestral
        "Torus Strings" => Config {
            system: SystemConfig { name: "geodesic_torus".into(), dt: 0.005, speed: 0.6 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "just_intonation".into(),
                base_frequency: 130.81, octave_range: 3.0,
                chord_mode: "major".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.8, 0.7], portamento_ms: 350.0,
                voice_shapes: ["triangle".into(), "triangle".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.72, delay_ms: 480.0, delay_feedback: 0.38,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.05, waveshaper_mix: 0.1,
                chorus_mix: 0.5, chorus_rate: 0.15, chorus_depth: 0.5,
                ..Default::default()
            },
            geodesic_torus: GeodesicTorusConfig { big_r: 3.5, r: 1.2 },
            ..Default::default()
        },
        "Lorenz Brass" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 1.0 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "just_intonation".into(),
                base_frequency: 87.31, octave_range: 2.5,
                chord_mode: "major".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.75, 0.6], portamento_ms: 200.0,
                voice_shapes: ["triangle".into(), "triangle".into(), "saw".into(), "triangle".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.65, delay_ms: 420.0, delay_feedback: 0.35,
                master_volume: 0.75, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.2, waveshaper_mix: 0.3,
                chorus_mix: 0.35, chorus_rate: 0.12, chorus_depth: 0.4,
                ..Default::default()
            },
            lorenz: LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 },
            ..Default::default()
        },
        "Three-Body Epic" => Config {
            system: SystemConfig { name: "three_body".into(), dt: 0.001, speed: 1.2 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "just_intonation".into(),
                base_frequency: 65.41, octave_range: 4.0,
                chord_mode: "major".into(), transpose_semitones: -5.0,
                voice_levels: [1.0, 1.0, 0.9, 0.8], portamento_ms: 300.0,
                voice_shapes: ["triangle".into(), "triangle".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.78, delay_ms: 560.0, delay_feedback: 0.42,
                master_volume: 0.78, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.15, waveshaper_mix: 0.2,
                chorus_mix: 0.4, chorus_rate: 0.1, chorus_depth: 0.5,
                ..Default::default()
            },
            ..Default::default()
        },
        "Halvorsen Score" => Config {
            system: SystemConfig { name: "halvorsen".into(), dt: 0.001, speed: 0.9 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 98.0, octave_range: 3.5,
                chord_mode: "minor".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.8, 0.7], portamento_ms: 280.0,
                voice_shapes: ["triangle".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.75, delay_ms: 500.0, delay_feedback: 0.4,
                master_volume: 0.72, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.05, waveshaper_mix: 0.1,
                chorus_mix: 0.45, chorus_rate: 0.1, chorus_depth: 0.45,
                ..Default::default()
            },
            halvorsen: HalvorsenConfig { a: 1.89 },
            ..Default::default()
        },
        "Aizawa Overture" => Config {
            system: SystemConfig { name: "aizawa".into(), dt: 0.002, speed: 0.5 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "just_intonation".into(),
                base_frequency: 73.42, octave_range: 3.5,
                chord_mode: "major".into(), transpose_semitones: -7.0,
                voice_levels: [1.0, 0.9, 0.8, 0.7], portamento_ms: 450.0,
                voice_shapes: ["triangle".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.8, delay_ms: 620.0, delay_feedback: 0.4,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.5, chorus_rate: 0.08, chorus_depth: 0.55,
                ..Default::default()
            },
            aizawa: AizawaConfig { a: 0.95, b: 0.7, c: 0.6, d: 3.5, e: 0.25, f: 0.1 },
            ..Default::default()
        },

        // ------------------------------------------------------------------ Bass / Sub
        "Lorenz Sub" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 0.7 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 55.0, octave_range: 1.5,
                chord_mode: "power".into(), transpose_semitones: -12.0,
                voice_levels: [1.0, 0.8, 0.0, 0.0], portamento_ms: 80.0,
                voice_shapes: ["saw".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.2, delay_ms: 300.0, delay_feedback: 0.35,
                master_volume: 0.75, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.3, waveshaper_mix: 0.4,
                chorus_mix: 0.1, chorus_rate: 0.1, chorus_depth: 0.2,
                ..Default::default()
            },
            lorenz: LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 },
            ..Default::default()
        },
        "Duffing Bass" => Config {
            system: SystemConfig { name: "duffing".into(), dt: 0.001, speed: 1.0 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 55.0, octave_range: 1.0,
                chord_mode: "power".into(), transpose_semitones: -12.0,
                voice_levels: [1.0, 0.6, 0.0, 0.0], portamento_ms: 60.0,
                voice_shapes: ["saw".into(), "saw".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.15, delay_ms: 250.0, delay_feedback: 0.3,
                master_volume: 0.75, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.4, waveshaper_mix: 0.5,
                chorus_mix: 0.0, chorus_rate: 0.0, chorus_depth: 0.0,
                ..Default::default()
            },
            duffing: DuffingConfig { delta: 0.3, alpha: -1.0, beta: 1.0, gamma: 0.4, omega: 1.0 },
            ..Default::default()
        },
        "Chua Sub Rumble" => Config {
            system: SystemConfig { name: "chua".into(), dt: 0.0005, speed: 0.5 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "chromatic".into(),
                base_frequency: 40.0, octave_range: 1.5,
                chord_mode: "power".into(), transpose_semitones: -12.0,
                voice_levels: [1.0, 0.7, 0.0, 0.0], portamento_ms: 120.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.25, delay_ms: 400.0, delay_feedback: 0.4,
                master_volume: 0.72, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.2, waveshaper_mix: 0.3,
                chorus_mix: 0.1, chorus_rate: 0.05, chorus_depth: 0.3,
                ..Default::default()
            },
            chua: ChuaConfig { alpha: 15.6, beta: 28.0, m0: -1.143, m1: -0.714 },
            ..Default::default()
        },
        "Van Der Pol Bass" => Config {
            system: SystemConfig { name: "van_der_pol".into(), dt: 0.001, speed: 0.6 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 55.0, octave_range: 1.5,
                chord_mode: "power".into(), transpose_semitones: -12.0,
                voice_levels: [1.0, 0.5, 0.0, 0.0], portamento_ms: 100.0,
                voice_shapes: ["saw".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.18, delay_ms: 280.0, delay_feedback: 0.3,
                master_volume: 0.86, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.35, waveshaper_mix: 0.45,
                chorus_mix: 0.0, chorus_rate: 0.0, chorus_depth: 0.0,
                ..Default::default()
            },
            van_der_pol: VanDerPolConfig { mu: 2.0 },
            ..Default::default()
        },

        // ------------------------------------------------------------------ Lead / Melody
        "Rossler Lead" => Config {
            system: SystemConfig { name: "rossler".into(), dt: 0.002, speed: 1.2 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 440.0, octave_range: 2.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.0, 0.0, 0.0], portamento_ms: 100.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.38, delay_ms: 310.0, delay_feedback: 0.32,
                master_volume: 0.72, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.2, chorus_rate: 0.3, chorus_depth: 0.35,
                ..Default::default()
            },
            rossler: RosslerConfig { a: 0.2, b: 0.2, c: 5.7 },
            ..Default::default()
        },
        "Lorenz Melody" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 1.5 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "chromatic".into(),
                base_frequency: 261.63, octave_range: 2.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.0, 0.0, 0.0], portamento_ms: 70.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.42, delay_ms: 260.0, delay_feedback: 0.3,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.25, chorus_rate: 0.25, chorus_depth: 0.3,
                ..Default::default()
            },
            lorenz: LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 },
            ..Default::default()
        },
        "Kuramoto Lead" => Config {
            system: SystemConfig { name: "kuramoto".into(), dt: 0.001, speed: 1.5 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 523.25, octave_range: 2.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.0, 0.0, 0.0], portamento_ms: 55.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.4, delay_ms: 240.0, delay_feedback: 0.3,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.3, chorus_rate: 0.3, chorus_depth: 0.35,
                ..Default::default()
            },
            kuramoto: KuramotoConfig { n_oscillators: 4, coupling: 1.5 },
            ..Default::default()
        },
        "Aizawa Flute" => Config {
            system: SystemConfig { name: "aizawa".into(), dt: 0.002, speed: 0.9 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 523.25, octave_range: 1.5,
                chord_mode: "none".into(), transpose_semitones: 5.0,
                voice_levels: [1.0, 0.0, 0.0, 0.0], portamento_ms: 110.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.52, delay_ms: 360.0, delay_feedback: 0.28,
                master_volume: 0.65, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.15, chorus_rate: 0.2, chorus_depth: 0.25,
                ..Default::default()
            },
            aizawa: AizawaConfig { a: 0.95, b: 0.7, c: 0.6, d: 3.5, e: 0.25, f: 0.1 },
            ..Default::default()
        },

        // ------------------------------------------------------------------ Drone / Meditation
        "Torus Om" => Config {
            system: SystemConfig { name: "geodesic_torus".into(), dt: 0.01, speed: 0.1 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 136.1, octave_range: 1.5,
                chord_mode: "octave".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.8, 0.7], portamento_ms: 1000.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.9, delay_ms: 1200.0, delay_feedback: 0.35,
                master_volume: 0.6, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.5, chorus_rate: 0.05, chorus_depth: 0.6,
                ..Default::default()
            },
            geodesic_torus: GeodesicTorusConfig { big_r: 5.0, r: 1.5 },
            ..Default::default()
        },
        "Rössler Tanpura" => Config {
            system: SystemConfig { name: "rossler".into(), dt: 0.003, speed: 0.4 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 261.63, octave_range: 2.0,
                chord_mode: "octave".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4], portamento_ms: 800.0,
                voice_shapes: ["sine".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.7, delay_ms: 950.0, delay_feedback: 0.45,
                master_volume: 0.65, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.3, chorus_rate: 0.08, chorus_depth: 0.5,
                ..Default::default()
            },
            rossler: RosslerConfig { a: 0.1, b: 0.1, c: 14.0 },
            ..Default::default()
        },
        "Van Der Pol Throat" => Config {
            system: SystemConfig { name: "van_der_pol".into(), dt: 0.001, speed: 0.3 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "just_intonation".into(),
                base_frequency: 110.0, octave_range: 1.5,
                chord_mode: "octave".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.5, 0.3], portamento_ms: 700.0,
                voice_shapes: ["triangle".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.75, delay_ms: 800.0, delay_feedback: 0.4,
                master_volume: 0.62, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.1, waveshaper_mix: 0.15,
                chorus_mix: 0.35, chorus_rate: 0.06, chorus_depth: 0.55,
                ..Default::default()
            },
            van_der_pol: VanDerPolConfig { mu: 1.0 },
            ..Default::default()
        },
        "Halvorsen Ohm" => Config {
            system: SystemConfig { name: "halvorsen".into(), dt: 0.002, speed: 0.15 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "just_intonation".into(),
                base_frequency: 174.61, octave_range: 1.0,
                chord_mode: "octave".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4], portamento_ms: 1100.0,
                voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.88, delay_ms: 1300.0, delay_feedback: 0.3,
                master_volume: 0.58, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.45, chorus_rate: 0.04, chorus_depth: 0.65,
                ..Default::default()
            },
            halvorsen: HalvorsenConfig { a: 1.89 },
            ..Default::default()
        },

        // ------------------------------------------------------------------ Arpeggio
        "Lorenz Arp" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 4.0 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 220.0, octave_range: 3.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.5, 0.0, 0.0], portamento_ms: 8.0,
                voice_shapes: ["triangle".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.35, delay_ms: 166.0, delay_feedback: 0.55,
                master_volume: 0.72, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.1, waveshaper_mix: 0.15,
                chorus_mix: 0.2, chorus_rate: 0.4, chorus_depth: 0.3,
                ..Default::default()
            },
            lorenz: LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 },
            ..Default::default()
        },
        "Duffing Arp" => Config {
            system: SystemConfig { name: "duffing".into(), dt: 0.001, speed: 4.5 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "chromatic".into(),
                base_frequency: 330.0, octave_range: 3.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.4, 0.0, 0.0], portamento_ms: 6.0,
                voice_shapes: ["triangle".into(), "sine".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.3, delay_ms: 142.0, delay_feedback: 0.5,
                master_volume: 0.73, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.05, waveshaper_mix: 0.1,
                chorus_mix: 0.15, chorus_rate: 0.5, chorus_depth: 0.25,
                ..Default::default()
            },
            duffing: DuffingConfig { delta: 0.2, alpha: -1.0, beta: 1.0, gamma: 0.6, omega: 1.3 },
            ..Default::default()
        },
        "Kuramoto Arp" => Config {
            system: SystemConfig { name: "kuramoto".into(), dt: 0.001, speed: 3.5 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "pentatonic".into(),
                base_frequency: 440.0, octave_range: 3.0,
                chord_mode: "none".into(), transpose_semitones: 7.0,
                voice_levels: [1.0, 0.6, 0.3, 0.0], portamento_ms: 6.0,
                voice_shapes: ["triangle".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.32, delay_ms: 150.0, delay_feedback: 0.52,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.2, chorus_rate: 0.45, chorus_depth: 0.28,
                ..Default::default()
            },
            kuramoto: KuramotoConfig { n_oscillators: 6, coupling: 0.3 },
            ..Default::default()
        },
        "Aizawa Cascade" => Config {
            system: SystemConfig { name: "aizawa".into(), dt: 0.001, speed: 5.0 },
            sonification: SonificationConfig {
                mode: "direct".into(), scale: "microtonal".into(),
                base_frequency: 220.0, octave_range: 4.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.7, 0.4, 0.2], portamento_ms: 5.0,
                voice_shapes: ["triangle".into(), "sine".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.28, delay_ms: 125.0, delay_feedback: 0.58,
                master_volume: 0.72, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.05, waveshaper_mix: 0.08,
                chorus_mix: 0.25, chorus_rate: 0.6, chorus_depth: 0.32,
                ..Default::default()
            },
            aizawa: AizawaConfig { a: 0.95, b: 0.7, c: 0.6, d: 3.5, e: 0.25, f: 0.1 },
            ..Default::default()
        },

        // ------------------------------------------------------------------ Hybrid / Unusual
        "Torus FM" => Config {
            system: SystemConfig { name: "geodesic_torus".into(), dt: 0.003, speed: 1.5 },
            sonification: SonificationConfig {
                mode: "fm".into(), scale: "just_intonation".into(),
                base_frequency: 220.0, octave_range: 3.0,
                chord_mode: "major".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4], portamento_ms: 120.0,
                voice_shapes: ["sine".into(), "sine".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.55, delay_ms: 380.0, delay_feedback: 0.38,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.1, waveshaper_mix: 0.15,
                chorus_mix: 0.35, chorus_rate: 0.2, chorus_depth: 0.4,
                ..Default::default()
            },
            geodesic_torus: GeodesicTorusConfig { big_r: 2.5, r: 1.5 },
            ..Default::default()
        },
        "Lorenz Granular Cloud" => Config {
            system: SystemConfig { name: "lorenz".into(), dt: 0.001, speed: 0.8 },
            sonification: SonificationConfig {
                mode: "granular".into(), scale: "microtonal".into(),
                base_frequency: 160.0, octave_range: 4.0,
                chord_mode: "sus2".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.8, 0.6, 0.4], portamento_ms: 200.0,
                voice_shapes: ["sine".into(), "sine".into(), "triangle".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.7, delay_ms: 520.0, delay_feedback: 0.42,
                master_volume: 0.65, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.0, waveshaper_mix: 0.0,
                chorus_mix: 0.5, chorus_rate: 0.18, chorus_depth: 0.6,
                ..Default::default()
            },
            lorenz: LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 },
            ..Default::default()
        },
        "Chua Spectral Web" => Config {
            system: SystemConfig { name: "chua".into(), dt: 0.0005, speed: 1.3 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "microtonal".into(),
                base_frequency: 130.81, octave_range: 4.5,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.85, 0.7, 0.55], portamento_ms: 150.0,
                voice_shapes: ["sine".into(), "triangle".into(), "sine".into(), "triangle".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.6, delay_ms: 430.0, delay_feedback: 0.4,
                master_volume: 0.68, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.2, waveshaper_mix: 0.25,
                chorus_mix: 0.4, chorus_rate: 0.25, chorus_depth: 0.5,
                ..Default::default()
            },
            chua: ChuaConfig { alpha: 15.6, beta: 28.0, m0: -1.143, m1: -0.714 },
            ..Default::default()
        },
        "Pendulum Orbital Funk" => Config {
            system: SystemConfig { name: "double_pendulum".into(), dt: 0.001, speed: 2.2 },
            sonification: SonificationConfig {
                mode: "orbital".into(), scale: "microtonal".into(),
                base_frequency: 196.0, octave_range: 3.0,
                chord_mode: "dom7".into(), transpose_semitones: -2.0,
                voice_levels: [1.0, 0.9, 0.7, 0.5], portamento_ms: 35.0,
                voice_shapes: ["triangle".into(), "saw".into(), "sine".into(), "triangle".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.35, delay_ms: 200.0, delay_feedback: 0.45,
                master_volume: 0.75, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.15, waveshaper_mix: 0.2,
                chorus_mix: 0.3, chorus_rate: 0.35, chorus_depth: 0.4,
                ..Default::default()
            },
            ..Default::default()
        },
        "Three-Body Microtonal" => Config {
            system: SystemConfig { name: "three_body".into(), dt: 0.001, speed: 1.8 },
            sonification: SonificationConfig {
                mode: "spectral".into(), scale: "microtonal".into(),
                base_frequency: 220.0, octave_range: 4.0,
                chord_mode: "none".into(), transpose_semitones: 0.0,
                voice_levels: [1.0, 0.9, 0.75, 0.6], portamento_ms: 90.0,
                voice_shapes: ["sine".into(), "triangle".into(), "sine".into(), "sine".into()],
            },
            audio: AudioConfig {
                reverb_wet: 0.55, delay_ms: 350.0, delay_feedback: 0.38,
                master_volume: 0.7, bit_depth: 24.0, rate_crush: 0.0,
                waveshaper_drive: 0.05, waveshaper_mix: 0.1,
                chorus_mix: 0.35, chorus_rate: 0.22, chorus_depth: 0.45,
                ..Default::default()
            },
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
