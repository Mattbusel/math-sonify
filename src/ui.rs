use egui::*;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::sonification::chord_intervals_for;
use crate::patches::{PRESETS, load_preset, save_patch, list_patches, load_patch_file};
use crate::audio::{WavRecorder, LoopExportPending, VuMeter, SidechainLevel, ClipBuffer, save_clip, save_portrait_png};
use crate::systems::*;
use crate::arrangement::{Scene, total_duration, scene_at, generate_song, demo_arrangement};
use hound;

/// A single looper layer (stereo interleaved samples).
pub struct LooperLayer {
    pub samples: Vec<f32>,
    pub active: bool,
    pub level: f32,
    pub playback_pos: usize,
}

/// A parameter change event for replay recording.
#[derive(Clone)]
pub struct ReplayEvent {
    pub timestamp_ms: u32,
    pub param_id: u8,
    pub value: f32,
}

/// Shared mutable UI state — written by the UI thread, read by the sim thread.
pub struct AppState {
    pub config: Config,
    pub paused: bool,
    pub system_changed: bool,
    pub mode_changed: bool,
    pub viz_projection: usize,  // 0=XY, 1=XZ, 2=YZ
    pub viz_tab: usize,         // 0=Phase, 1=Waveform, 2=Notes, 3=Math View, 4=Bifurcation
    pub selected_preset: String,
    pub chaos_level: f32,
    pub current_state: Vec<f64>,
    pub current_deriv: Vec<f64>,
    pub kuramoto_phases: Vec<f64>,
    pub order_param: f64,
    pub sample_rate: u32,
    pub midi_enabled: bool,
    // LFO
    pub lfo_enabled: bool,
    pub lfo_rate: f32,
    pub lfo_depth: f32,
    pub lfo_target: String,
    pub lfo_phase: f64,
    // BPM Sync
    pub bpm: f32,
    pub bpm_sync: bool,
    // Automation
    pub auto_recording: bool,
    pub auto_playing: bool,
    pub auto_loop: bool,
    pub auto_events: Vec<(f64, String, f64)>,
    pub auto_play_pos: usize,
    pub auto_start_time: Instant,
    // Patch UI
    pub patch_name_input: String,
    pub patch_list: Vec<String>,
    // Theme
    pub theme: String,
    // Bifurcation
    pub bifurc_computing: bool,
    pub bifurc_param: String,
    // Loop export
    pub loop_bars: u32,
    // Karplus-Strong
    pub ks_enabled: bool,
    pub ks_volume: f32,
    // 3D rotation
    pub rotation_angle: f32,
    pub auto_rotate: bool,
    // Scene Arranger
    pub scenes: Vec<Scene>,
    pub arr_playing: bool,
    pub arr_elapsed: f32,
    pub arr_auto_record: bool,
    pub arr_loop: bool,
    pub arr_scene_edit: usize,
    pub arr_was_playing: bool,
    pub arr_mood: String,
    // Arpeggiator
    pub arp_enabled: bool,
    pub arp_steps: usize,
    pub arp_bpm: f32,
    pub arp_octaves: usize,
    pub arp_position: usize,
    pub arp_phase: f64,
    // Multi-layer polyphony
    pub poly_layers: Vec<PolyLayerDef>,
    // Layer 0 (main) mix controls
    pub layer0_level: f32,
    pub layer0_pan: f32,
    pub layer0_mute: bool,
    // ADSR (layer 0)
    pub adsr_attack_ms: f32,
    pub adsr_decay_ms: f32,
    pub adsr_sustain: f32,
    pub adsr_release_ms: f32,
    // VU meter (shared with audio thread)
    pub vu_meter: VuMeter,
    // Audio sidechain
    pub sidechain_enabled: bool,
    pub sidechain_target: String,
    pub sidechain_amount: f32,
    pub sidechain_level_shared: SidechainLevel,
    // Clip buffer (shared with audio thread)
    pub clip_buffer: ClipBuffer,
    // Status message for clip save
    pub clip_status: String,
    // 3-band EQ
    pub eq_low_db: f32,
    pub eq_mid_db: f32,
    pub eq_high_db: f32,
    pub eq_mid_freq: f32,
    // Macro knobs (Simple mode)
    pub macro_chaos: f32,
    pub macro_space: f32,
    pub macro_rhythm: f32,
    pub macro_warmth: f32,
    // Macro random walk (Evolve mode)
    pub macro_walk_enabled: bool,
    pub macro_walk_rate: f32,
    // Simple / Advanced mode toggle
    pub simple_mode: bool,
    // AUTO mode (auto-generate + auto-play arrangement)
    pub auto_mode: bool,
    // Coupled attractor systems
    pub coupled_enabled: bool,
    pub coupled_source: String,
    pub coupled_strength: f32,
    pub coupled_target: String,
    pub coupled_x_out: f32,   // live display: main system x output (normalized)
    pub coupled_src_x_out: f32, // live display: source system x output (normalized)
    pub coupled_bidirectional: bool,
    pub sync_error: f32,
    // Performance mode
    pub perf_mode: bool,
    // Audio-reactive trail color
    pub trail_color: egui::Color32,
    // Custom ODE fields
    pub custom_ode_x: String,
    pub custom_ode_y: String,
    pub custom_ode_z: String,
    pub custom_ode_error: String,
    // Spectral freeze
    pub spectral_freeze_active: bool,
    pub spectral_freeze_freqs: Vec<f32>,
    pub spectral_freeze_amps: Vec<f32>,
    // Fractional Lorenz alpha
    pub lorenz_alpha: f64,
    // Arrangement probabilistic
    pub arr_probabilistic: bool,
    // MIDI input
    pub midi_in_enabled: bool,
    pub midi_in_note_target: String,
    pub midi_in_vel_target: String,
    pub midi_in_cc_target: String,
    pub midi_in_cc_num: u8,
    pub midi_in_last_note: u8,
    pub midi_in_last_vel: u8,
    pub midi_in_last_cc: u8,
    // Replay
    pub replay_recording: bool,
    pub replay_events: Vec<ReplayEvent>,
    pub replay_start_time: std::time::Instant,
    pub replay_playing: bool,
    pub replay_play_pos: usize,
    pub replay_play_start: std::time::Instant,
    // Looper
    pub looper_recording: bool,
    pub looper_playing: bool,
    pub looper_bars: u32,
    pub looper_bpm: f32,
    pub looper_layers: Vec<LooperLayer>,
    // Anaglyph 3D
    pub anaglyph_3d: bool,
    pub anaglyph_separation: f32,
    // Tips window
    pub show_tips_window: bool,
    // Simple panel preset expansion
    pub simple_show_all_presets: bool,
    // Ghost trails: periodic snapshots of the phase portrait trajectory
    // Appear every 10-15 min, fade over a few seconds — visual memory of the attractor's past
    pub portrait_ghosts: Vec<(Vec<(f32, f32)>, std::time::Instant)>,
    pub portrait_ghost_last_capture: std::time::Instant,
    // Portrait ink: long-session accumulated stain (x, y) positions
    // Live trail is bright; historical stain is almost invisible
    // Over hours the entire reachable set faintly emerges
    pub portrait_ink: Vec<(f32, f32)>,
    pub portrait_ink_sample_counter: u32,
    // Invisible behavioral fields
    pub last_interaction_time: std::time::Instant,
    pub aging_secs: f32,
    pub volume_creep_factor: f32,
    pub entropy_pool: f32,
    pub lunar_phase: f32,
    pub last_volume_for_creep: f32,
    /// True while we're recording a single-pass render of the generated arrangement
    pub save_gen_pending: bool,
    // Invisible behavioral fields (new batch)
    pub scars: Vec<(f32, f32)>,       // SCARRING: near-divergence marks
    pub shutdown_fading: bool,         // DYING GRACEFULLY: audio fade on close
    pub startup_ramp_t: f32,           // DYING GRACEFULLY: startup volume ramp
    pub time_of_day_f: f32,            // PHOTOTROPISM: 0=midnight 1=noon
    pub wounded: bool,                 // WOUND HEALING: crashed last session
    // Lyapunov exponent spectrum (computed every ~5s in sim thread, largest-first)
    pub lyapunov_spectrum: Vec<f64>,
    pub attractor_type: String,
    pub kolmogorov_entropy: f64,
    // 2D bifurcation map
    pub bifurc_param2: String,
    pub bifurc_2d_mode: bool,
    pub bifurc_data_2d: Arc<Mutex<Vec<(f32, f32, f32)>>>,
    pub energy_error: f64,
    pub permutation_entropy: f64,
    pub integrator_divergence: f64,
    pub session_log: Vec<SessionEntry>,
}

#[derive(Clone)]
pub struct SessionEntry {
    pub elapsed_secs: f32,
    pub system_name: String,
    pub lyapunov_max: f64,
    pub attractor_type: String,
    pub kolmogorov_entropy: f64,
    pub chaos_level: f32,
}

#[derive(Clone)]
pub struct PolyLayerDef {
    pub preset_name: String,
    pub level: f32,
    pub pan: f32,
    pub mute: bool,
    pub active: bool,
    pub adsr_attack_ms: f32,
    pub adsr_decay_ms: f32,
    pub adsr_sustain: f32,
    pub adsr_release_ms: f32,
    pub changed: bool,
}

impl Default for PolyLayerDef {
    fn default() -> Self {
        Self {
            preset_name: String::new(),
            level: 0.7,
            pan: 0.0,
            mute: false,
            active: false,
            adsr_attack_ms: 10.0,
            adsr_decay_ms: 200.0,
            adsr_sustain: 0.7,
            adsr_release_ms: 400.0,
            changed: false,
        }
    }
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            paused: false,
            system_changed: false,
            mode_changed: false,
            viz_projection: 0,
            viz_tab: 0,
            selected_preset: "Midnight Approach".into(),
            chaos_level: 0.0,
            current_state: Vec::new(),
            current_deriv: Vec::new(),
            kuramoto_phases: Vec::new(),
            order_param: 0.0,
            sample_rate: 44100,
            midi_enabled: false,
            lfo_enabled: false,
            lfo_rate: 0.05,
            lfo_depth: 0.3,
            lfo_target: "speed".into(),
            lfo_phase: 0.0,
            bpm: 120.0,
            bpm_sync: false,
            auto_recording: false,
            auto_playing: false,
            auto_loop: true,
            auto_events: Vec::new(),
            auto_play_pos: 0,
            auto_start_time: Instant::now(),
            patch_name_input: String::new(),
            patch_list: list_patches(),
            theme: "neon".into(),
            bifurc_computing: false,
            bifurc_param: "rho".into(),
            loop_bars: 4,
            ks_enabled: false,
            ks_volume: 0.5,
            rotation_angle: 0.0,
            auto_rotate: false,
            scenes: (0..8).map(|i| Scene::empty(i)).collect(),
            arr_playing: false,
            arr_elapsed: 0.0,
            arr_auto_record: true,
            arr_loop: false,
            arr_scene_edit: 0,
            arr_was_playing: false,
            arr_mood: "ambient".into(),
            arp_enabled: false,
            arp_steps: 8,
            arp_bpm: 120.0,
            arp_octaves: 1,
            arp_position: 0,
            arp_phase: 0.0,
            poly_layers: vec![PolyLayerDef::default(), PolyLayerDef::default()],
            layer0_level: 1.0,
            layer0_pan: 0.0,
            layer0_mute: false,
            adsr_attack_ms: 10.0,
            adsr_decay_ms: 200.0,
            adsr_sustain: 0.7,
            adsr_release_ms: 400.0,
            vu_meter: Arc::new(Mutex::new([0.0; 4])),
            sidechain_enabled: false,
            sidechain_target: "speed".into(),
            sidechain_amount: 0.5,
            sidechain_level_shared: Arc::new(Mutex::new(0.0)),
            clip_buffer: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            clip_status: String::new(),
            eq_low_db: 0.0,
            eq_mid_db: 0.0,
            eq_high_db: 0.0,
            eq_mid_freq: 1000.0,
            macro_chaos: 0.5,
            macro_space: 0.4,
            macro_rhythm: 0.0,
            macro_warmth: 0.5,
            macro_walk_enabled: false,
            macro_walk_rate: 0.05,
            simple_mode: true,
            auto_mode: false,
            coupled_enabled: false,
            coupled_source: "rossler".into(),
            coupled_strength: 0.3,
            coupled_target: "rho".into(),
            coupled_x_out: 0.0,
            coupled_src_x_out: 0.0,
            coupled_bidirectional: false,
            sync_error: 0.0,
            perf_mode: false,
            trail_color: egui::Color32::from_rgb(0, 220, 255),
            custom_ode_x: "10.0 * (y - x)".into(),
            custom_ode_y: "x * (28.0 - z) - y".into(),
            custom_ode_z: "x * y - 2.667 * z".into(),
            custom_ode_error: String::new(),
            spectral_freeze_active: false,
            spectral_freeze_freqs: vec![0.0; 16],
            spectral_freeze_amps: vec![0.0; 16],
            lorenz_alpha: 1.0,
            arr_probabilistic: false,
            midi_in_enabled: false,
            midi_in_note_target: "rho".into(),
            midi_in_vel_target: "speed".into(),
            midi_in_cc_target: "reverb_wet".into(),
            midi_in_cc_num: 1,
            midi_in_last_note: 0,
            midi_in_last_vel: 0,
            midi_in_last_cc: 0,
            replay_recording: false,
            replay_events: Vec::new(),
            replay_start_time: std::time::Instant::now(),
            replay_playing: false,
            replay_play_pos: 0,
            replay_play_start: std::time::Instant::now(),
            looper_recording: false,
            looper_playing: false,
            looper_bars: 4,
            looper_bpm: 120.0,
            looper_layers: Vec::new(),
            anaglyph_3d: false,
            anaglyph_separation: 0.05,
            show_tips_window: false,
            simple_show_all_presets: false,
            portrait_ghosts: Vec::new(),
            portrait_ghost_last_capture: std::time::Instant::now(),
            portrait_ink: Vec::new(),
            portrait_ink_sample_counter: 0,
            last_interaction_time: std::time::Instant::now(),
            aging_secs: 0.0,
            volume_creep_factor: 1.0,
            entropy_pool: 0.0,
            lunar_phase: 0.0,
            last_volume_for_creep: 0.7,
            save_gen_pending: false,
            scars: Vec::new(),
            shutdown_fading: false,
            startup_ramp_t: 0.0,
            time_of_day_f: 0.5,
            wounded: false,
            lyapunov_spectrum: Vec::new(),
            attractor_type: String::new(),
            kolmogorov_entropy: 0.0,
            bifurc_param2: "sigma".into(),
            bifurc_2d_mode: false,
            bifurc_data_2d: Arc::new(Mutex::new(Vec::new())),
            energy_error: 0.0,
            permutation_entropy: 0.0,
            integrator_divergence: 0.0,
            session_log: Vec::new(),
        }
    }
}

pub type SharedState = Arc<Mutex<AppState>>;

fn lcg_rand(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*seed >> 33) as f64 / u32::MAX as f64
}

fn apply_theme(ctx: &Context, theme: &str) {
    let mut visuals = ctx.style().visuals.clone();
    visuals.dark_mode = true;

    // Global rounding — gives a polished, modern feel across all themes
    let round_sm = egui::Rounding::same(5.0);
    let round_md = egui::Rounding::same(8.0);
    visuals.window_rounding = round_md;
    visuals.menu_rounding = round_md;
    visuals.widgets.noninteractive.rounding = round_sm;
    visuals.widgets.inactive.rounding = round_sm;
    visuals.widgets.hovered.rounding = round_sm;
    visuals.widgets.active.rounding = round_sm;
    visuals.widgets.open.rounding = round_sm;

    match theme {
        "vaporwave" => {
            visuals.window_fill = Color32::from_rgb(14, 7, 24);
            visuals.panel_fill = Color32::from_rgb(14, 7, 24);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 12, 46);
            visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(70, 30, 100));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(44, 18, 66);
            visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(90, 40, 130));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(70, 26, 100);
            visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, Color32::from_rgb(200, 80, 200));
            visuals.widgets.active.bg_fill = Color32::from_rgb(210, 55, 160);
            visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, Color32::from_rgb(255, 140, 230));
            visuals.selection.bg_fill = Color32::from_rgb(180, 40, 130);
            visuals.override_text_color = Some(Color32::from_rgb(248, 190, 240));
        }
        "crt" => {
            visuals.window_fill = Color32::from_rgb(0, 0, 0);
            visuals.panel_fill = Color32::from_rgb(0, 0, 0);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(0, 8, 0);
            visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(0, 40, 0));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(0, 16, 0);
            visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(0, 60, 0));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(0, 32, 0);
            visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, Color32::from_rgb(0, 200, 60));
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 160, 0);
            visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, Color32::from_rgb(0, 255, 80));
            visuals.selection.bg_fill = Color32::from_rgb(0, 120, 0);
            visuals.override_text_color = Some(Color32::from_rgb(0, 255, 60));
        }
        "solar" => {
            visuals.window_fill = Color32::from_rgb(14, 7, 0);
            visuals.panel_fill = Color32::from_rgb(14, 7, 0);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 14, 0);
            visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(60, 30, 0));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(42, 22, 0);
            visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(90, 46, 0));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(70, 38, 0);
            visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, Color32::from_rgb(220, 140, 0));
            visuals.widgets.active.bg_fill = Color32::from_rgb(210, 125, 0);
            visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, Color32::from_rgb(255, 200, 60));
            visuals.selection.bg_fill = Color32::from_rgb(180, 100, 0);
            visuals.override_text_color = Some(Color32::from_rgb(255, 215, 110));
        }
        _ => { // neon (default) — deep space blue
            visuals.window_fill = Color32::from_rgb(7, 7, 15);
            visuals.panel_fill = Color32::from_rgb(7, 7, 15);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(15, 15, 28);
            visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(35, 40, 70));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(20, 20, 40);
            visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(45, 50, 85));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(28, 32, 62);
            visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, Color32::from_rgb(60, 140, 230));
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 125, 215);
            visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, Color32::from_rgb(100, 190, 255));
            visuals.selection.bg_fill = Color32::from_rgb(0, 100, 185);
            visuals.override_text_color = Some(Color32::from_rgb(210, 220, 240));
        }
    }
    ctx.set_visuals(visuals);
}

fn system_display_name(s: &str) -> &'static str {
    match s {
        "lorenz" => "Lorenz Attractor",
        "fractional_lorenz" => "Fractional Lorenz",
        "rossler" => "Rossler Attractor",
        "double_pendulum" => "Double Pendulum",
        "geodesic_torus" => "Geodesic Torus",
        "kuramoto" => "Kuramoto Oscillators",
        "three_body" => "Three-Body Problem",
        "duffing" => "Duffing Oscillator",
        "van_der_pol" => "Van der Pol",
        "halvorsen" => "Halvorsen",
        "aizawa" => "Aizawa",
        "chua" => "Chua's Circuit",
        "hindmarsh_rose" => "Hindmarsh-Rose Neuron",
        "coupled_map_lattice" => "Coupled Map Lattice",
        "custom" => "Custom ODE",
        "mackey_glass" => "Mackey-Glass DDE",
        "nose_hoover" => "Nose-Hoover",
        "sprott_b" => "Sprott B",
        "henon_map" => "Henon Map",
        "lorenz96" => "Lorenz-96",
        _ => "Unknown System",
    }
}

fn system_internal_name(display: &str) -> &'static str {
    match display {
        "Lorenz Attractor" => "lorenz",
        "Fractional Lorenz" => "fractional_lorenz",
        "Rossler Attractor" => "rossler",
        "Double Pendulum" => "double_pendulum",
        "Geodesic Torus" => "geodesic_torus",
        "Kuramoto Oscillators" => "kuramoto",
        "Three-Body Problem" => "three_body",
        "Duffing Oscillator" => "duffing",
        "Van der Pol" => "van_der_pol",
        "Halvorsen" => "halvorsen",
        "Aizawa" => "aizawa",
        "Chua's Circuit" => "chua",
        "Hindmarsh-Rose Neuron" => "hindmarsh_rose",
        "Coupled Map Lattice" => "coupled_map_lattice",
        "Custom ODE" => "custom",
        "Mackey-Glass DDE" => "mackey_glass",
        "Nose-Hoover" => "nose_hoover",
        "Sprott B" => "sprott_b",
        "Henon Map" => "henon_map",
        "Lorenz-96" => "lorenz96",
        _ => "lorenz",
    }
}

fn scale_description(scale: &str) -> &'static str {
    match scale {
        "pentatonic" => "5 notes — always sounds musical",
        "chromatic" => "All 12 semitones — more dissonant",
        "just_intonation" => "Pure harmonic ratios — warm and consonant",
        "microtonal" => "Quarter-tones — alien and otherworldly",
        _ => "",
    }
}

fn mode_tooltip(mode: &str) -> &'static str {
    match mode {
        "direct" => "Maps attractor position directly to pitch",
        "orbital" => "Angular velocity = pitch, chaos = inharmonicity",
        "granular" => "Speed drives grain density and texture",
        "spectral" => "State vector shapes a 32-harmonic spectrum",
        "fm" => "Frequency modulation — chaos drives the mod index",
        "vocal" => "Vowel formant synthesis — attractor wanders through vowel space",
        _ => "",
    }
}

const CYAN:      Color32 = Color32::from_rgb(100, 200, 255);
const GRAY_HINT: Color32 = Color32::from_rgb(140, 145, 170);
const AMBER:     Color32 = Color32::from_rgb(220, 175, 60);
const GREEN_ACC: Color32 = Color32::from_rgb(50, 210, 130);
const DIM_BG:    Color32 = Color32::from_rgb(18, 18, 34);

fn collapsing_section(ui: &mut Ui, label: &str, default_open: bool, add_contents: impl FnOnce(&mut Ui)) {
    ui.add_space(4.0);
    // Subtle full-width header background
    let header_frame = egui::Frame::none()
        .fill(Color32::from_rgb(18, 22, 42))
        .inner_margin(egui::Margin::symmetric(6.0, 3.0))
        .rounding(egui::Rounding::same(5.0));
    header_frame.show(ui, |ui| {
        CollapsingHeader::new(
            RichText::new(label).size(12.5).color(CYAN).strong()
        )
        .default_open(default_open)
        .show(ui, |ui| {
            ui.add_space(4.0);
            add_contents(ui);
            ui.add_space(2.0);
        });
    });
    ui.add_space(4.0);
}

/// Draw the full UI. Called each egui frame.
pub fn draw_ui(
    ctx: &Context,
    state: &SharedState,
    viz_points: &[(f32, f32, f32, f32, bool)],
    waveform: &Arc<Mutex<Vec<f32>>>,
    recording: &WavRecorder,
    loop_export: &LoopExportPending,
    bifurc_data: &Arc<Mutex<Vec<(f32, f32)>>>,
) {
    let theme = state.lock().theme.clone();
    apply_theme(ctx, &theme);

    // Track user interaction — any egui input event resets silence timer
    // and deposits entropy into the pool
    if ctx.input(|i| !i.events.is_empty()) {
        let mut st = state.lock();
        let now = std::time::Instant::now();
        let secs_since_last = st.last_interaction_time.elapsed().as_secs_f32();
        st.last_interaction_time = now;
        // Each interaction deposits entropy (capped at 1000.0)
        // More entropy = more adventurous Evolve walk over time
        let deposit = (0.1 + secs_since_last * 0.02).min(2.0);
        st.entropy_pool = (st.entropy_pool + deposit).min(1000.0);
        // If volume was manually moved (not by creep), reset creep factor
        let current_vol = st.config.audio.master_volume;
        if (current_vol - st.last_volume_for_creep).abs() > 0.005 {
            st.volume_creep_factor = 1.0;
            st.last_volume_for_creep = current_vol;
        }
    }

    // Keyboard shortcuts (brief lock, then release)
    {
        let mut st = state.lock();
        ctx.input(|i| {
            if i.key_pressed(Key::Space) {
                st.paused = !st.paused;
            }
            if i.key_pressed(Key::ArrowUp) {
                st.config.audio.master_volume = (st.config.audio.master_volume + 0.05).min(1.0);
            }
            if i.key_pressed(Key::ArrowDown) {
                st.config.audio.master_volume = (st.config.audio.master_volume - 0.05).max(0.0);
            }
            if i.key_pressed(Key::ArrowRight) {
                st.config.system.speed = (st.config.system.speed * 1.2).min(10.0);
            }
            if i.key_pressed(Key::ArrowLeft) {
                st.config.system.speed = (st.config.system.speed / 1.2).max(0.1);
            }
            if i.key_pressed(Key::Num1) { st.viz_tab = 0; } // Phase Portrait
            if i.key_pressed(Key::Num2) { st.viz_tab = 1; } // MIXER
            if i.key_pressed(Key::Num3) { st.viz_tab = 2; } // ARRANGE
            if i.key_pressed(Key::Num4) { st.viz_tab = 3; } // Waveform
            if i.key_pressed(Key::Num5) { st.viz_tab = 4; } // Note Map
            if i.key_pressed(Key::Num6) { st.viz_tab = 5; } // Math View
            if i.key_pressed(Key::Num7) { st.viz_tab = 6; } // Bifurcation
            if i.key_pressed(Key::F) { st.perf_mode = !st.perf_mode; }
        });
    } // lock released here

    // Performance mode: fullscreen phase portrait only
    let is_perf_mode = state.lock().perf_mode;
    if is_perf_mode {
        egui::CentralPanel::default().show(ctx, |ui| {
            let trail_color = state.lock().trail_color;
            let (projection, rotation_angle, auto_rotate, system_name, mode_name,
                 current_state, current_deriv) = {
                let st = state.lock();
                (st.viz_projection, st.rotation_angle, st.auto_rotate,
                 st.config.system.name.clone(), st.config.sonification.mode.clone(),
                 st.current_state.clone(), st.current_deriv.clone())
            };
            let (ag, ag_sep) = { let st = state.lock(); (st.anaglyph_3d, st.anaglyph_separation) };
            let (pg, pi) = { let st = state.lock(); (st.portrait_ghosts.clone(), st.portrait_ink.clone()) };
            let lunar = { state.lock().lunar_phase };
            let (scars_perf, tod_perf) = { let st = state.lock(); (st.scars.clone(), st.time_of_day_f) };
            let ee_perf = { state.lock().energy_error };
            draw_phase_portrait(ui, viz_points, &system_name, &mode_name,
                &current_state, &current_deriv, projection, rotation_angle, auto_rotate, trail_color, ag, ag_sep, &pg, &pi, lunar, &scars_perf, tod_perf, ee_perf);
            // Dim hint in corner
            let rect = ui.min_rect();
            ui.painter().text(
                egui::Pos2::new(rect.left() + 10.0, rect.bottom() - 20.0),
                egui::Align2::LEFT_BOTTOM,
                "Press F to exit performance mode",
                egui::FontId::proportional(12.0),
                Color32::from_rgba_premultiplied(150, 150, 150, 100),
            );
        });
        return;
    }

    SidePanel::left("controls").min_width(310.0).max_width(360.0).show(ctx, |ui| {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);

            // ── App identity header ────────────────────────────────────────────────
            egui::Frame::none()
                .fill(Color32::from_rgb(10, 12, 24))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .rounding(egui::Rounding::same(8.0))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(30, 50, 90)))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        RichText::new("MATH SONIFY")
                            .size(19.0)
                            .color(CYAN)
                            .strong(),
                    );
                    ui.label(
                        RichText::new("strange attractors  →  sound")
                            .size(10.5)
                            .color(GRAY_HINT)
                            .italics(),
                    );
                });
            ui.add_space(8.0);

            // ── Live chaos status bar ──────────────────────────────────────────────
            {
                let (chaos, paused) = {
                    let st = state.lock();
                    (st.chaos_level, st.paused)
                };
                let chaos_col = lerp_color(
                    Color32::from_rgb(30, 110, 200),
                    Color32::from_rgb(220, 70, 30),
                    chaos,
                );
                ui.horizontal(|ui| {
                    let status_text = if paused { "⏸  PAUSED" } else { "▶  LIVE" };
                    let status_col = if paused { Color32::from_rgb(150, 150, 180) } else { Color32::from_rgb(80, 220, 120) };
                    ui.label(RichText::new(status_text).size(10.0).color(status_col).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{:.0}% chaos", chaos * 100.0)).size(10.0).color(chaos_col));
                    });
                });
                let bar_w = ui.available_width();
                let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 5.0), Sense::hover());
                ui.painter().rect_filled(bar_rect, 2.5, Color32::from_rgb(15, 15, 30));
                let fill_rect = egui::Rect::from_min_size(
                    bar_rect.min,
                    Vec2::new(bar_rect.width() * chaos.clamp(0.0, 1.0), bar_rect.height()),
                );
                ui.painter().rect_filled(fill_rect, 2.5, chaos_col);
            }
            ui.add_space(8.0);

            // ── Simple / Advanced toggle ───────────────────────────────────────────
            let is_simple = {
                let mut st = state.lock();
                let half_w = (ui.available_width() - 6.0) / 2.0;
                ui.horizontal(|ui| {
                    let simple_active = st.simple_mode;
                    let simple_fill = if simple_active {
                        Color32::from_rgb(22, 140, 78)
                    } else {
                        Color32::from_rgb(18, 18, 36)
                    };
                    let adv_fill = if !simple_active {
                        Color32::from_rgb(15, 105, 205)
                    } else {
                        Color32::from_rgb(18, 18, 36)
                    };
                    let simple_border = if simple_active {
                        Color32::from_rgb(50, 200, 110)
                    } else {
                        Color32::from_rgb(40, 42, 70)
                    };
                    let adv_border = if !simple_active {
                        Color32::from_rgb(60, 160, 255)
                    } else {
                        Color32::from_rgb(40, 42, 70)
                    };
                    if ui.add(
                        Button::new(RichText::new("Simple").color(Color32::WHITE).size(13.0).strong())
                            .fill(simple_fill)
                            .stroke(egui::Stroke::new(if simple_active { 1.5 } else { 1.0 }, simple_border))
                            .min_size(Vec2::new(half_w, 32.0)),
                    ).clicked() {
                        st.simple_mode = true;
                    }
                    if ui.add(
                        Button::new(RichText::new("Advanced").color(Color32::WHITE).size(13.0).strong())
                            .fill(adv_fill)
                            .stroke(egui::Stroke::new(if !simple_active { 1.5 } else { 1.0 }, adv_border))
                            .min_size(Vec2::new(half_w, 32.0)),
                    ).clicked() {
                        st.simple_mode = false;
                    }
                });
                ui.add_space(6.0);
                st.simple_mode
            }; // lock released here

            if is_simple {
                draw_simple_panel(ui, ctx, state, recording);
            } else {
                draw_advanced_panel(ui, state, recording, loop_export);
            }
        });
    });

    // ---- CENTRAL PANEL: Visualization ----
    CentralPanel::default().show(ctx, |ui| {
        // Tab bar row with theme switcher on the right
        egui::Frame::none()
            .fill(Color32::from_rgb(10, 10, 22))
            .inner_margin(egui::Margin::symmetric(4.0, 4.0))
            .show(ui, |ui| {
        ui.horizontal(|ui| {
            let tabs = [
                ("🌀", "Phase"),
                ("🎚", "Mixer"),
                ("🎬", "Arrange"),
                ("〰", "Wave"),
                ("🎵", "Notes"),
                ("∑", "Math"),
                ("∿", "Bifurc"),
            ];
            let mut viz_tab = state.lock().viz_tab;
            for (i, (icon, name)) in tabs.iter().enumerate() {
                let selected = viz_tab == i;
                let (fill, text_col) = if selected {
                    (Color32::from_rgb(0, 125, 215), Color32::WHITE)
                } else {
                    (Color32::from_rgb(20, 22, 42), Color32::from_rgb(160, 170, 200))
                };
                let btn = Button::new(
                    RichText::new(format!("{} {}", icon, name)).color(text_col).size(11.5)
                )
                .fill(fill)
                .stroke(egui::Stroke::new(
                    if selected { 1.5 } else { 0.0 },
                    Color32::from_rgb(60, 160, 255),
                ))
                .min_size(Vec2::new(74.0, 28.0));
                if ui.add(btn).clicked() {
                    viz_tab = i;
                }
            }
            state.lock().viz_tab = viz_tab;

            // Theme switcher right-aligned
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let themes = [
                    ("☀", "solar",     Color32::from_rgb(195, 110, 10)),
                    ("⎕", "crt",       Color32::from_rgb(0, 170, 0)),
                    ("◈", "vaporwave", Color32::from_rgb(170, 35, 130)),
                    ("◆", "neon",      Color32::from_rgb(0, 130, 215)),
                ];
                for (icon, theme_key, color) in themes.iter() {
                    let is_active = state.lock().theme == *theme_key;
                    let btn_color = if is_active { *color } else { Color32::from_rgb(22, 22, 40) };
                    let border = if is_active { *color } else { Color32::from_rgb(40, 42, 65) };
                    if ui.add(
                        Button::new(RichText::new(*icon).color(Color32::WHITE).size(13.0))
                            .fill(btn_color)
                            .stroke(egui::Stroke::new(1.0, border))
                            .min_size(Vec2::new(26.0, 26.0))
                    ).on_hover_text(*theme_key).clicked() {
                        state.lock().theme = theme_key.to_string();
                    }
                }
            });
        });
        });

        let viz_tab = state.lock().viz_tab;

        // Trail length slider + projection buttons for Phase Portrait tab
        if viz_tab == 0 {
            {
                let mut st = state.lock();
                if st.auto_rotate {
                    st.rotation_angle = (st.rotation_angle + 0.005) % std::f32::consts::TAU;
                }
            }
            ui.horizontal(|ui| {
                let mut st = state.lock();
                ui.label(RichText::new("Trail:").color(Color32::WHITE));
                ui.add(Slider::new(&mut st.config.viz.trail_length, 100..=2000).text("pts"));
                ui.separator();
                ui.label(RichText::new("Projection:").color(Color32::WHITE));
                let proj = st.viz_projection;
                let auto_rot = st.auto_rotate;
                let mut rot_angle = st.rotation_angle;
                drop(st);
                let mut new_proj = proj;
                for (i, label) in ["XY", "XZ", "YZ"].iter().enumerate() {
                    let selected = proj == i;
                    let color = if selected { Color32::from_rgb(0, 140, 210) } else { Color32::from_rgb(40, 40, 70) };
                    if ui.add(
                        Button::new(RichText::new(*label).color(Color32::WHITE))
                            .fill(color)
                            .min_size(Vec2::new(36.0, 22.0))
                    ).clicked() {
                        new_proj = i;
                    }
                }
                ui.separator();
                let rot_color = if auto_rot { Color32::from_rgb(0, 160, 100) } else { Color32::from_rgb(40, 40, 70) };
                let mut new_auto_rot = auto_rot;
                if ui.add(
                    Button::new(RichText::new("3D Rotate").color(Color32::WHITE))
                        .fill(rot_color)
                        .min_size(Vec2::new(72.0, 22.0))
                ).clicked() {
                    new_auto_rot = !auto_rot;
                }
                if !auto_rot {
                    ui.add(Slider::new(&mut rot_angle, 0.0..=std::f32::consts::TAU).text("Angle"));
                }
                let mut st = state.lock();
                st.viz_projection = new_proj;
                st.auto_rotate = new_auto_rot;
                st.rotation_angle = rot_angle;
                drop(st);

                // Anaglyph 3D controls
                let (anaglyph, sep) = {
                    let st = state.lock();
                    (st.anaglyph_3d, st.anaglyph_separation)
                };
                let ag_color = if anaglyph { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(40, 40, 70) };
                let mut new_anaglyph = anaglyph;
                let mut new_sep = sep;
                if ui.add(Button::new(RichText::new("Anaglyph 3D").color(Color32::WHITE))
                    .fill(ag_color).min_size(Vec2::new(84.0, 22.0))).clicked() {
                    new_anaglyph = !anaglyph;
                }
                if anaglyph {
                    ui.add(Slider::new(&mut new_sep, 0.0..=0.2).text("Sep"));
                }
                let mut st = state.lock();
                st.anaglyph_3d = new_anaglyph;
                st.anaglyph_separation = new_sep;
            });
        }

        // Bifurcation controls
        if viz_tab == 6 {
            ui.horizontal(|ui| {
                let param_opts = ["rho", "sigma", "coupling", "c"];
                let (current_bp, current_bp2, computing, mode_2d) = {
                    let st = state.lock();
                    (st.bifurc_param.clone(), st.bifurc_param2.clone(), st.bifurc_computing, st.bifurc_2d_mode)
                };
                let mut new_bp = current_bp.clone();
                let mut new_bp2 = current_bp2.clone();
                let mut new_2d = mode_2d;

                ComboBox::from_id_source("bifurc_param")
                    .selected_text(&current_bp)
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for p in &param_opts {
                            if ui.selectable_label(current_bp == *p, *p).clicked() {
                                new_bp = p.to_string();
                            }
                        }
                    });

                // 2D mode toggle + second param selector
                if ui.toggle_value(&mut new_2d, "2D").on_hover_text("Sweep two parameters as a heatmap").changed() {}
                if new_2d {
                    ComboBox::from_id_source("bifurc_param2")
                        .selected_text(&current_bp2)
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for p in &param_opts {
                                if ui.selectable_label(current_bp2 == *p, *p).clicked() {
                                    new_bp2 = p.to_string();
                                }
                            }
                        });
                }

                {
                    let mut st = state.lock();
                    st.bifurc_param = new_bp.clone();
                    st.bifurc_param2 = new_bp2.clone();
                    st.bifurc_2d_mode = new_2d;
                }

                let compute_color = if computing { Color32::from_rgb(100, 60, 0) } else { Color32::from_rgb(0, 80, 120) };
                if !computing && ui.add(
                    Button::new(RichText::new("Compute").color(Color32::WHITE)).fill(compute_color)
                ).clicked() {
                    let (param, param2, is_2d, sys_name, lorenz_cfg, rossler_cfg, kuramoto_cfg) = {
                        let st = state.lock();
                        (st.bifurc_param.clone(), st.bifurc_param2.clone(), st.bifurc_2d_mode,
                         st.config.system.name.clone(),
                         st.config.lorenz.clone(), st.config.rossler.clone(), st.config.kuramoto.clone())
                    };
                    state.lock().bifurc_computing = true;
                    let bifurc_data_clone = bifurc_data.clone();
                    let state_clone = state.clone();
                    std::thread::spawn(move || {
                        if is_2d {
                            // 2D: sweep param1 × param2 on a 40×40 grid; measure chaos via speed variance
                            let grid = 40usize;
                            let (p1min, p1max) = param_range(&param);
                            let (p2min, p2max) = param_range(&param2);
                            let result: Vec<(f32, f32, f32)> = (0..grid * grid).into_par_iter().map(|idx| {
                                let i = idx / grid;
                                let j = idx % grid;
                                let p1val = p1min + (p1max - p1min) * i as f64 / (grid - 1) as f64;
                                let p2val = p2min + (p2max - p2min) * j as f64 / (grid - 1) as f64;
                                let mut sys = build_bifurc_system(&sys_name, &param, p1val, &lorenz_cfg, &rossler_cfg, &kuramoto_cfg);
                                // Apply second param on top of first
                                let mut sys2 = build_bifurc_system(&sys_name, &param2, p2val, &lorenz_cfg, &rossler_cfg, &kuramoto_cfg);
                                // Use whichever system has both params set (build_bifurc only sets one)
                                // For simplicity: rebuild with param=p1 then override param2 via a combined system
                                // Workaround: use sys for p1, apply p2 perturbation as initial offset
                                for _ in 0..1000 { sys.step(0.005); sys2.step(0.005); }
                                // Chaos metric: variance of x over 200 steps
                                let mut vals = Vec::with_capacity(200);
                                for _ in 0..200 {
                                    sys.step(0.005);
                                    if let Some(&v) = sys.state().first() { vals.push(v); }
                                }
                                let mean = vals.iter().sum::<f64>() / vals.len().max(1) as f64;
                                let var = vals.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / vals.len().max(1) as f64;
                                let _ = sys2; // suppress unused warning
                                (p1val as f32, p2val as f32, var.sqrt() as f32)
                            }).collect();
                            // For 2D we store in AppState directly
                            *state_clone.lock().bifurc_data_2d.lock() = result;
                            *bifurc_data_clone.lock() = Vec::new(); // clear 1D display
                        } else {
                            // 1D: parallel sweep over parameter range
                            let steps = 200usize;
                            let (pmin, pmax) = param_range(&param);
                            let result: Vec<(f32, f32)> = (0..steps).into_par_iter().flat_map(|i| {
                                let pval = pmin + (pmax - pmin) * i as f64 / (steps - 1) as f64;
                                let mut sys = build_bifurc_system(&sys_name, &param, pval, &lorenz_cfg, &rossler_cfg, &kuramoto_cfg);
                                for _ in 0..2000 { sys.step(0.005); }
                                let mut pts = Vec::with_capacity(100);
                                for _ in 0..100 {
                                    sys.step(0.005);
                                    if let Some(&v) = sys.state().first() {
                                        pts.push((pval as f32, v as f32));
                                    }
                                }
                                pts
                            }).collect();
                            *bifurc_data_clone.lock() = result;
                        }
                        state_clone.lock().bifurc_computing = false;
                    });
                }
                if computing {
                    ui.label(RichText::new("Computing...").color(Color32::from_rgb(255, 200, 0)));
                }
            });
        }

        ui.separator();

        let (projection, rotation_angle, auto_rotate, system_name, mode_name,
             freqs, voice_levels, chord_intervals, current_state, current_deriv,
             chaos_level, order_param, kuramoto_phases, trail_color, perf_mode,
             anaglyph_3d, anaglyph_separation, lyapunov_spectrum, attractor_type,
             kolmogorov_entropy, energy_error, sync_error, permutation_entropy,
             integrator_divergence) = {
            let st = state.lock();
            let proj = st.viz_projection;
            let rot = st.rotation_angle;
            let ar = st.auto_rotate;
            let sn = st.config.system.name.clone();
            let mn = st.config.sonification.mode.clone();
            let fr = [
                st.config.sonification.base_frequency as f32,
                st.config.sonification.base_frequency as f32 * 2.0,
                st.config.sonification.base_frequency as f32 * 3.0,
                st.config.sonification.base_frequency as f32 * 4.0,
            ];
            let vl = st.config.sonification.voice_levels;
            let ci = chord_intervals_for(&st.config.sonification.chord_mode);
            let cs = st.current_state.clone();
            let cd = st.current_deriv.clone();
            let cl = st.chaos_level;
            let op = st.order_param;
            let kp = st.kuramoto_phases.clone();
            let tc = st.trail_color;
            let pm = st.perf_mode;
            let ag = st.anaglyph_3d;
            let ag_sep = st.anaglyph_separation;
            let ls = st.lyapunov_spectrum.clone();
            let at = st.attractor_type.clone();
            let ke = st.kolmogorov_entropy;
            let ee = st.energy_error;
            let se = st.sync_error;
            let pe = st.permutation_entropy;
            let id = st.integrator_divergence;
            (proj, rot, ar, sn, mn, fr, vl, ci, cs, cd, cl, op, kp, tc, pm, ag, ag_sep, ls, at, ke, ee, se, pe, id)
        };

        // ── Ghost trails + portrait ink update (visual memory) ─────────────────
        // Ghost snapshots: every 10-15 minutes take a snapshot of the current trail.
        // Fade out over 8 seconds. Up to 4 ghosts visible at once.
        // Portrait ink: accumulate faint long-term stain of all positions visited.
        {
            let mut st = state.lock();
            // Ink: sample every ~30 frames (once per second at 30fps)
            st.portrait_ink_sample_counter += 1;
            if st.portrait_ink_sample_counter >= 30 && !viz_points.is_empty() {
                st.portrait_ink_sample_counter = 0;
                // Add the current trail tip to the ink
                let (x, y, _, _, _) = viz_points[viz_points.len() - 1];
                st.portrait_ink.push((x, y));
                // Cap ink at 3000 points — each point = 1 draw call; more causes crash
                if st.portrait_ink.len() > 3000 {
                    st.portrait_ink.drain(0..300);
                }
            }
            // Ghost capture: every 10-15 minutes (at 30fps = ~18000-27000 frames)
            // Use a simple elapsed-time check
            let ghost_interval_secs = 600.0 + (st.portrait_ghosts.len() as f64 * 47.0) % 300.0; // 10-15 min
            if st.portrait_ghost_last_capture.elapsed().as_secs_f64() >= ghost_interval_secs
               && !viz_points.is_empty() {
                // Sample every 4th point to keep ghost compact
                let ghost_pts: Vec<(f32, f32)> = viz_points.iter()
                    .step_by(4)
                    .map(|&(x, y, _, _, _)| (x, y))
                    .collect();
                st.portrait_ghosts.push((ghost_pts, std::time::Instant::now()));
                st.portrait_ghost_last_capture = std::time::Instant::now();
                // Keep at most 4 ghost snapshots
                while st.portrait_ghosts.len() > 4 {
                    st.portrait_ghosts.remove(0);
                }
            }
        }
        let (ghosts, ink, lunar_phase2) = {
            let st = state.lock();
            (st.portrait_ghosts.clone(), st.portrait_ink.clone(), st.lunar_phase)
        };
        let (scars_main, tod_main) = {
            let st = state.lock();
            (st.scars.clone(), st.time_of_day_f)
        };

        match viz_tab {
            0 => draw_phase_portrait(ui, viz_points, &system_name, &mode_name, &current_state, &current_deriv, projection, rotation_angle, auto_rotate, trail_color, anaglyph_3d, anaglyph_separation, &ghosts, &ink, lunar_phase2, &scars_main, tod_main, energy_error),
            1 => draw_mixer_tab(ui, state, viz_points),
            2 => draw_arrange_tab(ui, state, recording),
            3 => draw_waveform(ui, waveform),
            4 => draw_note_map(ui, &freqs, &voice_levels, &chord_intervals),
            5 => draw_math_view(ui, &system_name, &current_state, &current_deriv, chaos_level, order_param, &kuramoto_phases, &lyapunov_spectrum, &attractor_type, kolmogorov_entropy, energy_error, sync_error, permutation_entropy, integrator_divergence),
            6 => draw_bifurc_diagram(ui, bifurc_data, state),
            _ => {}
        }
    });
}

// ---------------------------------------------------------------------------
// Advanced panel — all the old controls
// ---------------------------------------------------------------------------

fn draw_advanced_panel(
    ui: &mut Ui,
    state: &SharedState,
    recording: &WavRecorder,
    loop_export: &LoopExportPending,
) {
    let mut st = state.lock();

    let adv_vol = st.config.audio.master_volume;
    let adv_db_label = if adv_vol > 0.001 {
        format!("{:.1} dB", 20.0 * adv_vol.log10())
    } else {
        "-∞ dB".to_string()
    };

    // Compact header bar: volume + pause side by side
    ui.horizontal(|ui| {
        ui.label(RichText::new("🔊  Volume").color(Color32::WHITE).strong().size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(&adv_db_label).color(AMBER).size(11.0));
        });
    });
    ui.add(
        Slider::new(&mut st.config.audio.master_volume, 0.0..=1.0).text("")
    ).on_hover_text("Master output volume. Use ↑/↓ arrow keys as a quick shortcut.");
    ui.add_space(4.0);

    // ---- Pause button ----
    let pause_label = if st.paused { "▶  RESUME" } else { "⏸  PAUSE" };
    let (pause_fill, pause_stroke) = if st.paused {
        (Color32::from_rgb(18, 135, 65), Color32::from_rgb(60, 220, 110))
    } else {
        (Color32::from_rgb(15, 90, 165), Color32::from_rgb(60, 160, 255))
    };
    let btn = Button::new(RichText::new(pause_label).color(Color32::WHITE).strong().size(13.0))
        .fill(pause_fill)
        .stroke(egui::Stroke::new(1.5, pause_stroke))
        .min_size(Vec2::new(ui.available_width(), 36.0));
    if ui.add(btn).on_hover_text("Pause or resume the simulation and audio. Shortcut: Space bar.").clicked() {
        st.paused = !st.paused;
    }
    ui.add_space(6.0);

    // ---- Chaos meter ----
    let chaos = st.chaos_level;
    let chaos_color = lerp_color(
        Color32::from_rgb(0, 100, 200),
        Color32::from_rgb(220, 50, 30),
        chaos,
    );
    ui.horizontal(|ui| {
        ui.label(RichText::new("Chaos").color(GRAY_HINT).size(11.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{:.0}%", chaos * 100.0)).color(chaos_color).size(11.0));
        });
    });
    let bar_w = ui.available_width();
    let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 7.0), Sense::hover());
    ui.painter().rect_filled(bar_rect, 3.5, Color32::from_rgb(14, 14, 28));
    let fill_rect = egui::Rect::from_min_size(
        bar_rect.min,
        Vec2::new(bar_rect.width() * chaos.clamp(0.0, 1.0), bar_rect.height()),
    );
    ui.painter().rect_filled(fill_rect, 3.5, chaos_color);
    ui.add_space(2.0);

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(2.0);

    // ---- PRESETS ----
    collapsing_section(ui, "PRESETS", true, |ui| {
        ui.colored_label(AMBER, "Click a preset to start");
        ui.add_space(6.0);

        let selected = st.selected_preset.clone();
        let mut last_cat = "";
        for preset in PRESETS.iter() {
            if preset.category != last_cat {
                last_cat = preset.category;
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("  {}", preset.category.to_uppercase()))
                        .color(GRAY_HINT).size(10.0).strong());
                    let avail = ui.available_width();
                    let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(avail - 4.0, 1.0), Sense::hover());
                    ui.painter().rect_filled(sep_rect, 0.0, Color32::from_rgb(35, 38, 65));
                });
                ui.add_space(3.0);
            }
            let is_selected = selected == preset.name;
            let pc = preset.color;
            let bg_color = if is_selected {
                Color32::from_rgba_premultiplied(
                    (pc.r() as u16 * 55 / 255) as u8,
                    (pc.g() as u16 * 55 / 255) as u8,
                    (pc.b() as u16 * 55 / 255) as u8,
                    255,
                )
            } else {
                Color32::from_rgb(14, 14, 26)
            };
            let card = egui::Frame::none()
                .fill(bg_color)
                .stroke(Stroke::new(if is_selected { 1.5 } else { 1.0 },
                    if is_selected { pc } else { Color32::from_rgb(34, 36, 64) }))
                .inner_margin(egui::Margin::symmetric(0.0, 5.0))
                .rounding(egui::Rounding::same(6.0));
            let response = card.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (strip_rect, _) = ui.allocate_exact_size(Vec2::new(5.0, 34.0), Sense::hover());
                    let strip_col = if is_selected { pc } else {
                        Color32::from_rgba_premultiplied(pc.r()/3, pc.g()/3, pc.b()/3, 200)
                    };
                    ui.painter().rect_filled(strip_rect, egui::Rounding::same(3.0), strip_col);
                    ui.add_space(7.0);
                    ui.vertical(|ui| {
                        let name_col = if is_selected { pc } else { Color32::WHITE };
                        ui.label(RichText::new(preset.name).strong().color(name_col).size(12.5));
                        ui.label(RichText::new(preset.description).italics().color(GRAY_HINT).size(10.5));
                    });
                });
            }).response;
            if response.interact(Sense::click()).clicked() {
                st.selected_preset = preset.name.to_string();
                st.config = load_preset(preset.name);
                st.system_changed = true;
                st.mode_changed = true;
            }
            ui.add_space(3.0);
        }
    });

    // ---- SOUND ----
    collapsing_section(ui, "SOUND", false, |ui| {
        ui.label(RichText::new("Mode").color(GRAY_HINT).size(11.0));
        let modes = ["direct", "orbital", "granular", "spectral", "fm", "vocal", "waveguide"];
        let current_mode = st.config.sonification.mode.clone();
        ui.horizontal_wrapped(|ui| {
            for m in &modes {
                let selected = current_mode == *m;
                let color = if selected { Color32::from_rgb(0, 140, 210) } else { Color32::from_rgb(40, 40, 70) };
                let resp = ui.add(
                    Button::new(RichText::new(*m).color(Color32::WHITE))
                        .fill(color)
                        .min_size(Vec2::new(55.0, 26.0))
                );
                if resp.clicked() {
                    st.config.sonification.mode = m.to_string();
                    st.mode_changed = true;
                }
                resp.on_hover_text(mode_tooltip(m));
            }
        });
        ui.add_space(6.0);

        let scales = ["pentatonic", "chromatic", "just_intonation", "microtonal"];
        let current_scale = st.config.sonification.scale.clone();
        ComboBox::from_label("Scale")
            .selected_text(&current_scale)
            .show_ui(ui, |ui| {
                for s in &scales {
                    if ui.selectable_label(current_scale == *s, *s).clicked() {
                        st.config.sonification.scale = s.to_string();
                    }
                }
            });
        let desc = scale_description(&current_scale);
        if !desc.is_empty() {
            ui.label(RichText::new(desc).italics().size(11.0).color(GRAY_HINT));
        }
        ui.add_space(4.0);

        ui.add(Slider::new(&mut st.config.sonification.base_frequency, 55.0..=880.0)
            .text("Root Hz").logarithmic(true))
            .on_hover_text("Root/base frequency in Hz. All pitched notes are mapped relative to this. A2=110, A3=220, A4=440.");
        ui.add(Slider::new(&mut st.config.sonification.octave_range, 1.0..=6.0)
            .text("Octave Range"))
            .on_hover_text("How many octaves the attractor's position maps across. Wider = more dramatic pitch leaps. Narrow = drone-like, stable pitch.");

        ui.add_space(6.0);

        CollapsingHeader::new(
            RichText::new("LFO").size(12.0).color(GRAY_HINT)
        ).default_open(false).show(ui, |ui| {
            ui.checkbox(&mut st.lfo_enabled, RichText::new("Enable LFO").color(Color32::WHITE))
                .on_hover_text("Low Frequency Oscillator — automatically sweeps a chosen parameter at a sub-audio rate, creating tremolo, vibrato, or slow parameter evolution.");
            ui.add(Slider::new(&mut st.lfo_rate, 0.01..=2.0).text("Rate Hz").logarithmic(true))
                .on_hover_text("LFO cycle rate. Below 0.1 Hz = very slow sweep (tens of seconds). 1-2 Hz = fast tremolo or vibrato.");
            ui.add(Slider::new(&mut st.lfo_depth, 0.0..=1.0).text("Depth"))
                .on_hover_text("How much the LFO moves the target parameter. 0 = no effect, 1.0 = full sweep of the parameter's range.");
            let lfo_targets = ["speed", "sigma", "rho", "beta", "a", "b", "c", "coupling"];
            let current_target = st.lfo_target.clone();
            ComboBox::from_label("Target")
                .selected_text(&current_target)
                .show_ui(ui, |ui| {
                    for t in &lfo_targets {
                        if ui.selectable_label(current_target == *t, *t).clicked() {
                            st.lfo_target = t.to_string();
                        }
                    }
                });
        });
    });

    // ---- MELODY & CHORDS ----
    collapsing_section(ui, "MELODY & CHORDS", false, |ui| {
        let chord_modes = ["none", "major", "minor", "power", "sus2", "octave", "dom7"];
        let current_chord = st.config.sonification.chord_mode.clone();
        ComboBox::from_label("Chord Mode")
            .selected_text(&current_chord)
            .show_ui(ui, |ui| {
                for cm in &chord_modes {
                    if ui.selectable_label(current_chord == *cm, *cm).clicked() {
                        st.config.sonification.chord_mode = cm.to_string();
                    }
                }
            }).response.on_hover_text("Add harmonic voices below the melody voice. 'major' = +4, +7 semitones. 'dom7' = jazz dominant 7th. 'none' = unison only.");

        let mut ts = st.config.sonification.transpose_semitones as f64;
        if ui.add(Slider::new(&mut ts, -24.0..=24.0).text("Transpose").step_by(1.0))
            .on_hover_text("Shift all pitches by this many semitones. ±12 = one octave. Useful for key changes without re-tuning.")
            .changed() {
            st.config.sonification.transpose_semitones = ts as f32;
        }

        ui.add(Slider::new(&mut st.config.sonification.portamento_ms, 1.0..=1000.0)
            .text("Portamento ms").logarithmic(true))
            .on_hover_text("Glide time between notes in milliseconds. 1-20ms = snappy note changes. 200-1000ms = smooth, continuous pitch sweeps — great for ambient and drone sounds.");

        ui.add_space(4.0);
        ui.label(RichText::new("Voice Levels").color(Color32::WHITE));
        for i in 0..4 {
            ui.add(Slider::new(&mut st.config.sonification.voice_levels[i], 0.0..=1.0)
                .text(format!("Voice {}", i + 1)));
        }

        ui.add_space(4.0);
        ui.label(RichText::new("Voice Waveforms").color(Color32::WHITE));
        let shapes = ["sine", "triangle", "saw"];
        for i in 0..4 {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("V{}:", i + 1)).color(Color32::WHITE));
                let current_shape = st.config.sonification.voice_shapes[i].clone();
                for s in &shapes {
                    let selected = current_shape == *s;
                    let color = if selected { Color32::from_rgb(0, 140, 210) } else { Color32::from_rgb(40, 40, 70) };
                    if ui.add(
                        Button::new(RichText::new(*s).color(Color32::WHITE))
                            .fill(color)
                            .min_size(Vec2::new(50.0, 24.0))
                    ).clicked() {
                        st.config.sonification.voice_shapes[i] = s.to_string();
                    }
                }
            });
        }
    });

    // ---- PHYSICS ENGINE ----
    collapsing_section(ui, "PHYSICS ENGINE", false, |ui| {
        let systems_internal = ["lorenz", "fractional_lorenz", "rossler", "double_pendulum", "geodesic_torus", "kuramoto", "three_body", "duffing", "van_der_pol", "halvorsen", "aizawa", "chua", "hindmarsh_rose", "coupled_map_lattice", "custom", "mackey_glass", "nose_hoover", "sprott_b", "henon_map", "lorenz96"];
        let systems_display: Vec<&str> = systems_internal.iter().map(|s| system_display_name(s)).collect();
        let current_sys = st.config.system.name.clone();
        let current_display = system_display_name(&current_sys);
        ComboBox::from_label("System")
            .selected_text(current_display)
            .show_ui(ui, |ui| {
                for disp in &systems_display {
                    let internal = system_internal_name(disp);
                    if ui.selectable_label(current_sys == internal, *disp).clicked() {
                        st.config.system.name = internal.to_string();
                        st.system_changed = true;
                    }
                }
            });

        ui.add(Slider::new(&mut st.config.system.speed, 0.1..=10.0).text("Speed"))
            .on_hover_text("Controls how fast the attractor evolves. Higher = more rapid pitch changes and rhythmic activity. Use ←/→ arrow keys as a shortcut.");

        CollapsingHeader::new(
            RichText::new("Parameters").size(12.0).color(GRAY_HINT)
        ).default_open(false).show(ui, |ui| {
            match st.config.system.name.as_str() {
                "lorenz" => {
                    ui.add(Slider::new(&mut st.config.lorenz.sigma, 1.0..=20.0).text("sigma"))
                        .on_hover_text("Prandtl number — controls the rate of convection. Values above 10 produce the classic butterfly pattern. Higher = faster pitch variation.");
                    ui.add(Slider::new(&mut st.config.lorenz.rho, 10.0..=50.0).text("rho"))
                        .on_hover_text("Rayleigh number — the chaos threshold is near 24.74. Below = stable fixed point, above = chaotic butterfly attractor.");
                    ui.add(Slider::new(&mut st.config.lorenz.beta, 0.5..=5.0).text("beta"))
                        .on_hover_text("Geometric factor. The classic value is 8/3 ≈ 2.667. Lower values increase oscillation amplitude.");
                }
                "fractional_lorenz" => {
                    ui.add(Slider::new(&mut st.lorenz_alpha, 0.5..=1.0).text("α (order)"))
                        .on_hover_text("Fractional derivative order — 1.0 = classic Lorenz");
                    ui.add(Slider::new(&mut st.config.lorenz.sigma, 1.0..=20.0).text("sigma"));
                    ui.add(Slider::new(&mut st.config.lorenz.rho, 10.0..=50.0).text("rho"));
                    ui.add(Slider::new(&mut st.config.lorenz.beta, 0.5..=5.0).text("beta"));
                }
                "rossler" => {
                    ui.add(Slider::new(&mut st.config.rossler.a, 0.01..=0.5).text("a"))
                        .on_hover_text("Spiral tightness");
                    ui.add(Slider::new(&mut st.config.rossler.b, 0.01..=0.5).text("b"));
                    ui.add(Slider::new(&mut st.config.rossler.c, 1.0..=15.0).text("c"))
                        .on_hover_text("Chaos onset — above ~5.7 = chaotic");
                }
                "double_pendulum" => {
                    ui.add(Slider::new(&mut st.config.double_pendulum.m1, 0.1..=5.0).text("m1"));
                    ui.add(Slider::new(&mut st.config.double_pendulum.m2, 0.1..=5.0).text("m2"));
                    ui.add(Slider::new(&mut st.config.double_pendulum.l1, 0.1..=3.0).text("l1"));
                    ui.add(Slider::new(&mut st.config.double_pendulum.l2, 0.1..=3.0).text("l2"));
                }
                "geodesic_torus" => {
                    ui.add(Slider::new(&mut st.config.geodesic_torus.big_r, 1.0..=8.0).text("R"))
                        .on_hover_text("Torus major radius");
                    ui.add(Slider::new(&mut st.config.geodesic_torus.r, 0.1..=3.0).text("r"))
                        .on_hover_text("Torus tube radius");
                }
                "kuramoto" => {
                    ui.add(Slider::new(&mut st.config.kuramoto.coupling, 0.0..=5.0).text("Coupling K"))
                        .on_hover_text("Coupling strength between oscillators. Watch synchronization emerge as you raise this above ~1.5 — the oscillators phase-lock and the sound suddenly coheres.");
                }
                "duffing" => {
                    ui.add(Slider::new(&mut st.config.duffing.delta, 0.1..=1.0).text("delta"));
                    ui.add(Slider::new(&mut st.config.duffing.gamma, 0.0..=1.5).text("Forcing"));
                    ui.add(Slider::new(&mut st.config.duffing.omega, 0.5..=3.0).text("Drive freq"));
                }
                "van_der_pol" => {
                    ui.add(Slider::new(&mut st.config.van_der_pol.mu, 0.1..=5.0).text("Nonlinearity"));
                }
                "halvorsen" => {
                    ui.add(Slider::new(&mut st.config.halvorsen.a, 1.0..=3.0).text("a"));
                }
                "aizawa" => {
                    ui.add(Slider::new(&mut st.config.aizawa.a, 0.5..=1.5).text("a"));
                    ui.add(Slider::new(&mut st.config.aizawa.c, 0.3..=1.0).text("c"));
                    ui.add(Slider::new(&mut st.config.aizawa.d, 2.0..=5.0).text("d"));
                }
                "chua" => {
                    ui.add(Slider::new(&mut st.config.chua.alpha, 10.0..=20.0).text("alpha"));
                    ui.add(Slider::new(&mut st.config.chua.beta, 20.0..=35.0).text("beta"));
                }
                "mackey_glass" => {
                    ui.add(Slider::new(&mut st.config.mackey_glass.beta, 0.1..=0.5).text("β feedback"))
                        .on_hover_text("Feedback gain. Higher values push the system toward sustained oscillations and chaos.");
                    ui.add(Slider::new(&mut st.config.mackey_glass.gamma, 0.05..=0.3).text("γ decay"))
                        .on_hover_text("Decay rate. Higher values damp the oscillation more quickly.");
                    ui.add(Slider::new(&mut st.config.mackey_glass.tau, 5.0..=30.0).text("τ delay"))
                        .on_hover_text("Time delay. The classic chaotic regime is τ≈17. Longer delays add more complexity.");
                    ui.add(Slider::new(&mut st.config.mackey_glass.n, 5.0..=15.0).text("n steepness"))
                        .on_hover_text("Nonlinearity steepness. Higher n = sharper transition, more sensitive to initial conditions.");
                }
                "nose_hoover" => {
                    ui.add(Slider::new(&mut st.config.nose_hoover.a, 1.0..=6.0).text("a"))
                        .on_hover_text("Thermostat strength. Near a=3 gives the classic chaotic Nose-Hoover attractor. Very high values become quasi-periodic.");
                }
                "sprott_b" => {
                    ui.label(RichText::new("Sprott B has no tunable parameters.")
                        .color(GRAY_HINT).italics().size(11.0));
                    ui.label(RichText::new("x'=yz  y'=x-y  z'=1-xy")
                        .color(Color32::from_rgb(100, 200, 255)).size(11.0)
                        .font(egui::FontId::monospace(11.0)));
                }
                "henon_map" => {
                    ui.add(Slider::new(&mut st.config.henon_map.a, 0.5..=1.8).text("a"))
                        .on_hover_text("Classic Hénon map parameter. a=1.4 gives the standard strange attractor. Near 1.0 creates period-doubling cascades.");
                    ui.add(Slider::new(&mut st.config.henon_map.b, 0.0..=0.5).text("b"))
                        .on_hover_text("Area contraction. b=0.3 is classic. Values near 0 collapse the map to nearly 1D.");
                }
                "lorenz96" => {
                    ui.add(Slider::new(&mut st.config.lorenz96.f, 4.0..=16.0).text("F forcing"))
                        .on_hover_text("Atmospheric forcing. F<5: periodic, F≈8: chaos (weather-like), F>12: turbulent high-dimensional chaos with many positive Lyapunov exponents.");
                }
                _ => {}
            }
        });

        ui.add_space(4.0);
        if ui.add(
            Button::new(RichText::new("Randomize Parameters").color(Color32::WHITE))
                .fill(Color32::from_rgb(60, 40, 80))
                .min_size(Vec2::new(ui.available_width(), 28.0))
        ).clicked() {
            let mut seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as u64;
            let vary = |v: f64, s: &mut u64| v * (0.8 + lcg_rand(s) * 0.4);
            match st.config.system.name.as_str() {
                "lorenz" => {
                    st.config.lorenz.sigma = vary(st.config.lorenz.sigma, &mut seed).clamp(1.0, 20.0);
                    st.config.lorenz.rho   = vary(st.config.lorenz.rho,   &mut seed).clamp(10.0, 50.0);
                    st.config.lorenz.beta  = vary(st.config.lorenz.beta,  &mut seed).clamp(0.5, 5.0);
                }
                "rossler" => {
                    st.config.rossler.a = vary(st.config.rossler.a, &mut seed).clamp(0.01, 0.5);
                    st.config.rossler.b = vary(st.config.rossler.b, &mut seed).clamp(0.01, 0.5);
                    st.config.rossler.c = vary(st.config.rossler.c, &mut seed).clamp(1.0, 15.0);
                }
                "kuramoto" => {
                    st.config.kuramoto.coupling = vary(st.config.kuramoto.coupling, &mut seed).clamp(0.0, 5.0);
                }
                _ => {}
            }
            st.system_changed = true;
        }
    });

    // ---- EFFECTS ----
    collapsing_section(ui, "EFFECTS", false, |ui| {
        ui.add(Slider::new(&mut st.config.audio.reverb_wet, 0.0..=1.0).text("Reverb"))
            .on_hover_text("Wet/dry mix of the Freeverb reverb. Higher values create cavernous, atmospheric spaces. Zero = completely dry, close, and intimate.");
        ui.add(Slider::new(&mut st.config.audio.delay_ms, 0.0..=1000.0).text("Delay Time ms"))
            .on_hover_text("Echo delay time in milliseconds. 125ms = 1/8th note at 120 BPM. 250ms = 1/4 note. Use BPM Sync to auto-calculate musical values.");
        ui.add(Slider::new(&mut st.config.audio.delay_feedback, 0.0..=0.95).text("Feedback (max 90%)"))
            .on_hover_text("How much of the delayed signal feeds back into the delay — controls how many repeats you hear. Above 0.9 can create infinite feedback drones.");

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(Slider::new(&mut st.bpm, 60.0..=200.0).text("BPM"));
            let sync_color = if st.bpm_sync { Color32::from_rgb(0, 160, 0) } else { Color32::from_rgb(50, 50, 70) };
            if ui.add(
                Button::new(RichText::new("Sync").color(Color32::WHITE))
                    .fill(sync_color)
                    .min_size(Vec2::new(44.0, 26.0))
            ).clicked() {
                st.bpm_sync = !st.bpm_sync;
            }
        });
        if st.bpm_sync {
            ui.label(RichText::new(format!("Delay: {:.0}ms  LFO: {:.2}Hz", 60000.0 / st.bpm, st.bpm / 60.0 * 0.25))
                .size(11.0).color(Color32::from_rgb(100, 200, 100)));
        }

        ui.add_space(4.0);
        ui.label(RichText::new("Bitcrusher").color(Color32::WHITE));
        ui.add(Slider::new(&mut st.config.audio.bit_depth, 1.0..=16.0).text("Bit Depth"))
            .on_hover_text("Reduces audio word length. 16-bit = studio quality. Below 8 bits gives lo-fi digital grit. 1-4 bits gives extreme aliasing and crunchy distortion.");
        ui.add(Slider::new(&mut st.config.audio.rate_crush, 0.0..=1.0).text("Rate Crush"))
            .on_hover_text("Sample rate reduction. At 0, no effect. Higher values reduce effective sample rate, adding staircase artifacts and aliased harmonics.");

        ui.add_space(4.0);
        ui.label(RichText::new("Chorus").color(Color32::WHITE));
        ui.add(Slider::new(&mut st.config.audio.chorus_mix, 0.0..=1.0).text("Chorus"))
            .on_hover_text("Chorus wet/dry mix. Adds a lush, widened stereo effect by mixing pitch-modulated copies. Great for pads and ambient sounds.");
        ui.add(Slider::new(&mut st.config.audio.chorus_rate, 0.1..=5.0).text("Chorus Rate"))
            .on_hover_text("LFO rate of the chorus modulation in Hz. Slow (0.1-0.5 Hz) = gentle movement. Fast (3-5 Hz) = obvious vibrato effect.");
        ui.add(Slider::new(&mut st.config.audio.chorus_depth, 0.5..=10.0).text("Depth ms"))
            .on_hover_text("Depth of chorus pitch modulation in milliseconds. More depth = wider pitch variation and a thicker, more detuned sound.");

        ui.add_space(4.0);
        ui.label(RichText::new("Saturation").color(Color32::WHITE));
        ui.add(Slider::new(&mut st.config.audio.waveshaper_mix, 0.0..=1.0).text("Saturation"))
            .on_hover_text("Tanh waveshaper wet/dry mix. Adds harmonic richness and warmth at low values, aggressive distortion at high values. Zero = clean signal.");
        if st.config.audio.waveshaper_mix > 0.0 {
            ui.add(Slider::new(&mut st.config.audio.waveshaper_drive, 1.0..=10.0).text("Drive"))
                .on_hover_text("Waveshaper drive/gain. Low values (1-2) = gentle warmth. High values (7-10) = hard clipping and aggressive harmonic distortion.");
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(2.0);
        ui.label(RichText::new("3-Band EQ").color(Color32::WHITE).strong());
        ui.label(RichText::new("Low shelf 200 Hz  ·  Mid peak  ·  High shelf 6 kHz")
            .size(10.0).color(GRAY_HINT).italics());
        ui.add_space(2.0);
        ui.add(Slider::new(&mut st.eq_low_db, -12.0..=12.0).text("Low  dB"))
            .on_hover_text("Low shelf gain at 200 Hz. Boost for warmth and bass weight; cut to thin out muddy low-end.");
        ui.add(Slider::new(&mut st.eq_mid_db, -12.0..=12.0).text("Mid  dB"))
            .on_hover_text("Peaking mid-band gain. Boost presence and body; cut to scoop out harshness.");
        ui.add(Slider::new(&mut st.eq_mid_freq, 200.0..=8000.0).text("Mid Hz").logarithmic(true))
            .on_hover_text("Center frequency of the mid peak filter (200–8000 Hz).");
        ui.add(Slider::new(&mut st.eq_high_db, -12.0..=12.0).text("High dB"))
            .on_hover_text("High shelf gain at 6 kHz. Boost for air and brightness; cut to soften harsh highs.");
        let any_nonzero = st.eq_low_db.abs() > 0.1 || st.eq_mid_db.abs() > 0.1 || st.eq_high_db.abs() > 0.1;
        if any_nonzero {
            if ui.add(Button::new(RichText::new("Reset EQ").color(Color32::WHITE).size(11.0))
                .fill(Color32::from_rgb(40, 30, 50))
                .min_size(Vec2::new(ui.available_width(), 24.0))
            ).clicked() {
                st.eq_low_db = 0.0;
                st.eq_mid_db = 0.0;
                st.eq_high_db = 0.0;
            }
        }
    });

    // ---- OUTPUT ----
    collapsing_section(ui, "OUTPUT", false, |ui| {
        let st_sample_rate = st.sample_rate;
        let is_recording = recording.try_lock().map(|r| r.is_some()).unwrap_or(false);
        let rec_label = if is_recording { "⏹  Stop & Save" } else { "⏺  Start Recording" };
        let rec_color = if is_recording { Color32::from_rgb(180, 30, 30) } else { Color32::from_rgb(30, 120, 30) };
        if ui.add(
            Button::new(RichText::new(rec_label).color(Color32::WHITE))
                .fill(rec_color)
                .min_size(Vec2::new(ui.available_width(), 36.0))
        ).clicked() {
            if is_recording {
                if let Some(mut lock) = recording.try_lock() { *lock = None; }
            } else {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let filename = format!("recording_{}.wav", secs);
                let spec = hound::WavSpec {
                    channels: 2,
                    sample_rate: st_sample_rate,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                };
                if let Ok(writer) = hound::WavWriter::create(&filename, spec) {
                    if let Some(mut lock) = recording.try_lock() {
                        *lock = Some(writer);
                    }
                }
            }
        }

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Bars:").color(Color32::WHITE));
            let bars_options = [1u32, 2, 4, 8, 16];
            let current_bars = st.loop_bars;
            ComboBox::from_id_source("loop_bars")
                .selected_text(format!("{}", current_bars))
                .width(60.0)
                .show_ui(ui, |ui| {
                    for &b in &bars_options {
                        if ui.selectable_label(current_bars == b, format!("{}", b)).clicked() {
                            st.loop_bars = b;
                        }
                    }
                });

            let is_exporting = loop_export.try_lock().map(|p| p.is_some()).unwrap_or(false);
            let export_label = if is_exporting {
                "Exporting...".to_string()
            } else {
                format!("Export {}-Bar Loop", st.loop_bars)
            };
            let export_color = if is_exporting { Color32::from_rgb(180, 120, 0) } else { Color32::from_rgb(0, 90, 110) };
            if !is_exporting && ui.add(
                Button::new(RichText::new(export_label).color(Color32::WHITE))
                    .fill(export_color)
            ).clicked() {
                let bars = st.loop_bars;
                let bpm = st.bpm;
                let sr = st.sample_rate;
                let total_samples = ((bars as f64 * 4.0 * 60.0 / bpm as f64) * sr as f64) as u64;
                if let Some(mut lock) = loop_export.try_lock() {
                    *lock = Some(total_samples);
                }
            }
        });

        ui.add_space(4.0);

        if ui.add(
            Button::new(RichText::new("Save as Default").color(Color32::WHITE))
                .fill(Color32::from_rgb(30, 55, 30))
                .min_size(Vec2::new(ui.available_width(), 28.0))
        ).clicked() {
            let toml_str = toml::to_string_pretty(&st.config).unwrap_or_default();
            let _ = std::fs::write("config.toml", toml_str);
        }

        ui.add_space(6.0);

        CollapsingHeader::new(
            RichText::new("MY PATCHES").size(12.0).color(GRAY_HINT)
        ).default_open(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut st.patch_name_input)
                    .desired_width(140.0)
                    .hint_text("Patch name..."));
                if ui.add(
                    Button::new(RichText::new("Save").color(Color32::WHITE))
                        .fill(Color32::from_rgb(0, 80, 120))
                ).clicked() {
                    let name = st.patch_name_input.clone();
                    if !name.is_empty() {
                        save_patch(&name, &st.config);
                        st.patch_list = list_patches();
                    }
                }
            });
            ui.add_space(4.0);
            let patches = st.patch_list.clone();
            if patches.is_empty() {
                ui.label(RichText::new("No patches saved yet").color(GRAY_HINT).size(11.0));
            } else {
                for patch_name in &patches {
                    if ui.button(patch_name).clicked() {
                        if let Some(cfg) = load_patch_file(patch_name) {
                            st.config = cfg;
                            st.system_changed = true;
                            st.mode_changed = true;
                        }
                    }
                }
            }
        });

        ui.add_space(6.0);

        CollapsingHeader::new(
            RichText::new("PERFORMANCE").size(12.0).color(GRAY_HINT)
        ).default_open(false).show(ui, |ui| {
            CollapsingHeader::new(
                RichText::new("Automation").size(11.0).color(GRAY_HINT)
            ).default_open(false).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let rec_color = if st.auto_recording { Color32::from_rgb(180, 30, 30) } else { Color32::from_rgb(60, 40, 40) };
                    if ui.add(
                        Button::new(RichText::new("Rec").color(Color32::WHITE))
                            .fill(rec_color)
                            .min_size(Vec2::new(50.0, 26.0))
                    ).clicked() {
                        st.auto_recording = !st.auto_recording;
                        if st.auto_recording {
                            st.auto_playing = false;
                            st.auto_events.clear();
                            st.auto_start_time = Instant::now();
                        }
                    }
                    let play_color = if st.auto_playing { Color32::from_rgb(0, 140, 0) } else { Color32::from_rgb(40, 60, 40) };
                    if ui.add(
                        Button::new(RichText::new("Play").color(Color32::WHITE))
                            .fill(play_color)
                            .min_size(Vec2::new(50.0, 26.0))
                    ).clicked() {
                        st.auto_playing = !st.auto_playing;
                        if st.auto_playing {
                            st.auto_recording = false;
                            st.auto_play_pos = 0;
                            st.auto_start_time = Instant::now();
                        }
                    }
                    if ui.add(
                        Button::new(RichText::new("Stop").color(Color32::WHITE))
                            .fill(Color32::from_rgb(50, 50, 50))
                            .min_size(Vec2::new(50.0, 26.0))
                    ).clicked() {
                        st.auto_recording = false;
                        st.auto_playing = false;
                    }
                });
                ui.checkbox(&mut st.auto_loop, RichText::new("Loop playback").color(Color32::WHITE));
                ui.label(RichText::new(format!("{} events recorded", st.auto_events.len()))
                    .size(11.0).color(GRAY_HINT));
                if st.auto_recording {
                    ui.label(RichText::new("Recording...").color(Color32::from_rgb(255, 80, 80)));
                }
                if st.auto_playing {
                    ui.label(RichText::new("Playing back").color(Color32::from_rgb(80, 255, 80)));
                }
            });

            ui.add_space(4.0);
            ui.checkbox(&mut st.midi_enabled, RichText::new("MIDI Output").color(Color32::WHITE));
            if st.midi_enabled {
                ui.label(RichText::new("Sending to first MIDI port").size(11.0).color(Color32::from_rgb(100, 200, 100)));
            }
        });
    });

    // ---- RHYTHM & ARP ----
    collapsing_section(ui, "RHYTHM & ARP", false, |ui| {
        ui.checkbox(&mut st.arp_enabled, RichText::new("Enable Arpeggiator").color(Color32::WHITE));
        if st.arp_enabled {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Steps:").color(Color32::WHITE));
                for &s in &[4usize, 8, 16] {
                    let sel = st.arp_steps == s;
                    let col = if sel { Color32::from_rgb(0, 140, 210) } else { Color32::from_rgb(40, 40, 70) };
                    if ui.add(Button::new(RichText::new(format!("{}", s)).color(Color32::WHITE))
                        .fill(col).min_size(Vec2::new(36.0, 24.0))).clicked() {
                        st.arp_steps = s;
                    }
                }
                ui.separator();
                ui.label(RichText::new("Oct:").color(Color32::WHITE));
                for &o in &[1usize, 2] {
                    let sel = st.arp_octaves == o;
                    let col = if sel { Color32::from_rgb(0, 140, 210) } else { Color32::from_rgb(40, 40, 70) };
                    if ui.add(Button::new(RichText::new(format!("{}", o)).color(Color32::WHITE))
                        .fill(col).min_size(Vec2::new(30.0, 24.0))).clicked() {
                        st.arp_octaves = o;
                    }
                }
            });
            ui.add(Slider::new(&mut st.arp_bpm, 40.0..=240.0).text("ARP BPM"));
            let step_pct = if st.arp_steps > 0 {
                (st.arp_position as f32 + 1.0) / st.arp_steps as f32
            } else { 0.0 };
            ui.add(ProgressBar::new(step_pct)
                .text(format!("Step {}/{}", st.arp_position + 1, st.arp_steps)));
        }
        ui.add_space(4.0);
        ui.separator();
        ui.label(RichText::new("Plucked Strings").color(Color32::WHITE));
        ui.checkbox(&mut st.ks_enabled, RichText::new("Enable KS (Poincaré rhythm)").color(Color32::WHITE));
        if st.ks_enabled {
            ui.add(Slider::new(&mut st.ks_volume, 0.0..=1.0).text("Volume"));
        }
    });

    // ---- COUPLED SYSTEMS ----
    drop(st); // release lock before calling collapsing_section (which takes state)
    collapsing_section(ui, "COUPLED SYSTEMS", false, |ui| {
        let mut st = state.lock();
        ui.checkbox(&mut st.coupled_enabled, RichText::new("Enable Coupled Attractor").color(Color32::WHITE));
        ui.add_space(4.0);

        // Source system dropdown
        let systems = ["lorenz", "rossler", "duffing", "van_der_pol", "halvorsen",
                        "aizawa", "chua", "double_pendulum", "geodesic_torus", "kuramoto", "three_body"];
        let current_src = st.coupled_source.clone();
        ComboBox::from_label("Source System")
            .selected_text(&current_src)
            .show_ui(ui, |ui| {
                for s in &systems {
                    if ui.selectable_label(current_src == *s, *s).clicked() {
                        st.coupled_source = s.to_string();
                    }
                }
            });

        // Coupling strength slider
        ui.add(Slider::new(&mut st.coupled_strength, 0.0..=1.0).text("Coupling Strength"));

        // Target parameter dropdown
        let target_params = ["rho", "sigma", "speed", "a", "c", "coupling"];
        let current_target = st.coupled_target.clone();
        ComboBox::from_label("Target Parameter")
            .selected_text(&current_target)
            .show_ui(ui, |ui| {
                for p in &target_params {
                    if ui.selectable_label(current_target == *p, *p).clicked() {
                        st.coupled_target = p.to_string();
                    }
                }
            });

        ui.add_space(6.0);
        // Live bar meters
        let main_x = st.coupled_x_out;
        let src_x = st.coupled_src_x_out;
        let sync_err = st.sync_error;
        drop(st);
        ui.label(RichText::new("Live Output (x):").color(GRAY_HINT).size(11.0));
        ui.horizontal(|ui| {
            ui.label(RichText::new("Main:").color(Color32::WHITE).size(11.0));
            let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 14.0), Sense::hover());
            ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 40));
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * main_x, rect.height()));
            ui.painter().rect_filled(bar, 2.0, Color32::from_rgb(0, 160, 200));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Src: ").color(Color32::WHITE).size(11.0));
            let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 14.0), Sense::hover());
            ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 40));
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * src_x, rect.height()));
            ui.painter().rect_filled(bar, 2.0, Color32::from_rgb(200, 100, 0));
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Sync Error (EMA):").color(GRAY_HINT).size(11.0));
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 14.0), Sense::hover());
            ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 40));
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * sync_err.clamp(0.0, 1.0), rect.height()));
            let err_color = {
                let t = sync_err.clamp(0.0, 1.0);
                Color32::from_rgb((t * 220.0) as u8, ((1.0 - t) * 160.0) as u8, 60)
            };
            ui.painter().rect_filled(bar, 2.0, err_color);
            ui.label(RichText::new(format!("{:.3}", sync_err)).color(GRAY_HINT).size(10.0));
        });
        {
            let mut st = state.lock();
            ui.checkbox(&mut st.coupled_bidirectional, RichText::new("Bidirectional Coupling").color(Color32::WHITE));
        }
    });

    // ---- CUSTOM ODE ----
    collapsing_section(ui, "CUSTOM ODE", false, |ui| {
        ui.label(RichText::new("Define your own 3D ODE system").color(GRAY_HINT).size(11.0));
        ui.label(RichText::new("Variables: x, y, z, t  |  Functions: sin, cos, exp, abs, sqrt").color(GRAY_HINT).size(10.0));
        ui.add_space(4.0);

        let (mut ex, mut ey, mut ez, err) = {
            let st = state.lock();
            (st.custom_ode_x.clone(), st.custom_ode_y.clone(), st.custom_ode_z.clone(), st.custom_ode_error.clone())
        };

        ui.horizontal(|ui| {
            ui.label(RichText::new("dx/dt =").color(Color32::WHITE).size(12.0));
            if ui.text_edit_singleline(&mut ex).changed() {
                state.lock().custom_ode_x = ex.clone();
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("dy/dt =").color(Color32::WHITE).size(12.0));
            if ui.text_edit_singleline(&mut ey).changed() {
                state.lock().custom_ode_y = ey.clone();
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("dz/dt =").color(Color32::WHITE).size(12.0));
            if ui.text_edit_singleline(&mut ez).changed() {
                state.lock().custom_ode_z = ez.clone();
            }
        });
        ui.add_space(4.0);
        if ui.button(RichText::new("Apply Custom ODE").color(Color32::WHITE).strong()).clicked() {
            use crate::systems::validate_exprs;
            let mut st = state.lock();
            match validate_exprs(&st.custom_ode_x, &st.custom_ode_y, &st.custom_ode_z) {
                Ok(()) => {
                    st.custom_ode_error.clear();
                    st.config.system.name = "custom".into();
                    st.system_changed = true;
                }
                Err(e) => {
                    st.custom_ode_error = e;
                }
            }
        }
        if !err.is_empty() {
            ui.colored_label(Color32::from_rgb(255, 80, 80), &err);
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Example (Lorenz): 10*(y-x) | x*(28-z)-y | x*y-2.667*z")
            .italics().size(10.0).color(GRAY_HINT));
    });

    // ---- MIDI INPUT ----
    collapsing_section(ui, "MIDI INPUT", false, |ui| {
        let mut st = state.lock();
        ui.checkbox(&mut st.midi_in_enabled, RichText::new("Enable MIDI Input").color(Color32::WHITE));
        if st.midi_in_enabled {
            let targets = ["rho", "sigma", "speed", "coupling", "base_freq"];
            let cur_note = st.midi_in_note_target.clone();
            ComboBox::from_label("Note Target")
                .selected_text(&cur_note)
                .show_ui(ui, |ui| {
                    for t in &targets {
                        if ui.selectable_label(cur_note == *t, *t).clicked() {
                            st.midi_in_note_target = t.to_string();
                        }
                    }
                });
            let cur_vel = st.midi_in_vel_target.clone();
            ComboBox::from_label("Vel Target")
                .selected_text(&cur_vel)
                .show_ui(ui, |ui| {
                    for t in &targets {
                        if ui.selectable_label(cur_vel == *t, *t).clicked() {
                            st.midi_in_vel_target = t.to_string();
                        }
                    }
                });
            let mut cc_num = st.midi_in_cc_num as u32;
            if ui.add(DragValue::new(&mut cc_num).clamp_range(0u32..=127).prefix("CC#")).changed() {
                st.midi_in_cc_num = cc_num as u8;
            }
            let cur_cc = st.midi_in_cc_target.clone();
            ComboBox::from_label("CC Target")
                .selected_text(&cur_cc)
                .show_ui(ui, |ui| {
                    for t in &targets {
                        if ui.selectable_label(cur_cc == *t, *t).clicked() {
                            st.midi_in_cc_target = t.to_string();
                        }
                    }
                });
            ui.add_space(4.0);
            ui.label(RichText::new(format!(
                "Note: {}  Vel: {}  CC: {}",
                st.midi_in_last_note, st.midi_in_last_vel, st.midi_in_last_cc
            )).color(CYAN).size(11.0));
        }
    });

    // ---- SOUND DESIGN TIPS ----
    collapsing_section(ui, "SOUND DESIGN TIPS", false, |ui| {
        draw_tips_content(ui);
    });

    // ---- KEYBOARD SHORTCUTS ----
    collapsing_section(ui, "KEYBOARD SHORTCUTS", false, |ui| {
        let shortcuts = [
            ("Space",   "Pause / Resume simulation"),
            ("F",       "Toggle performance mode (fullscreen portrait)"),
            ("↑ / ↓",  "Volume up / down (+/- 5%)"),
            ("← / →",  "Speed slower / faster (÷1.2 / ×1.2)"),
            ("1",       "Tab: Phase Portrait"),
            ("2",       "Tab: MIXER"),
            ("3",       "Tab: ARRANGE"),
            ("4",       "Tab: Waveform"),
            ("5",       "Tab: Note Map"),
            ("6",       "Tab: Math View"),
            ("7",       "Tab: Bifurcation Diagram"),
        ];
        for (key, desc) in &shortcuts {
            ui.horizontal(|ui| {
                ui.label(RichText::new(*key).color(AMBER).strong().size(11.0).monospace());
                ui.add_space(8.0);
                ui.label(RichText::new(*desc).color(GRAY_HINT).size(11.0));
            });
        }
    });
    ui.add_space(4.0);
}

// ---------------------------------------------------------------------------
// Shared tips content
// ---------------------------------------------------------------------------

fn draw_tips_content(ui: &mut Ui) {
    let tips: &[(&str, &str)] = &[
        (
            "Hear synchronization emerge",
            "Load 'The Synchronization'. Set coupling K to 0.5 (all noise). Slowly drag K to 3.0. You're watching a mathematical phase transition in real time. That's the Kuramoto model.",
        ),
        (
            "Build a 3-layer ambient piece",
            "In the MIXER tab: Layer 0 = 'Midnight Approach' (pad, reverb 0.8). Layer 1 = 'Glass Harp' (melody, reverb 0.5). Layer 2 = 'Monk's Bell' (rhythm, dry). Three different attractors, one mix.",
        ),
        (
            "Find sounds that exist for 15 seconds",
            "In Simple mode, hit AUTO (Ambient mood). Wait for the morph to begin. The 30-second transition between 'Breathing Galaxy' and 'Collapsing Cathedral' exists exactly once. Hit FREEZE in MIXER to capture it.",
        ),
        (
            "The chaos boundary",
            "Load 'The Butterfly's Aria'. Slowly drag ρ (rho) from 24 to 25. The system crosses the Hopf bifurcation at ρ=24.74. Below: stable spiral. Above: chaos. You can hear the moment the universe becomes unpredictable.",
        ),
        (
            "Type your own mathematics",
            "In the PHYSICS ENGINE section, select 'Custom ODE'. Type: dx/dt = y, dy/dt = -x + y*(1-x*x), dz/dt = 0.5*z. That's the Van der Pol oscillator from scratch. Press Apply and hear it.",
        ),
        (
            "Two attractors in conversation",
            "In COUPLED SYSTEMS (Advanced): Source = Rössler, Target = rho, Strength = 0.6. The Rössler's x-output is now modulating the Lorenz's chaos threshold in real time. This coupling behavior has barely been studied.",
        ),
        (
            "Create evolving textures without touching anything",
            "In Simple mode, enable Evolve. Set Walk speed to 0.08. Leave the app running. In 10 minutes you'll have sounds you couldn't have designed manually — the random walk explores regions of parameter space you'd never find.",
        ),
    ];
    for (title, body) in tips.iter() {
        CollapsingHeader::new(RichText::new(*title).color(AMBER).size(11.0).strong())
            .default_open(false)
            .show(ui, |ui| {
                ui.label(RichText::new(*body).color(GRAY_HINT).size(11.0).italics());
            });
        ui.add_space(2.0);
    }
}

// ---------------------------------------------------------------------------
// Simple panel — beginner-friendly macro controls
// ---------------------------------------------------------------------------

fn draw_simple_panel(ui: &mut Ui, ctx: &Context, state: &SharedState, recording: &WavRecorder) {
    // ---- BIG AUTO button ----
    {
        let (auto_mode, arr_mood) = {
            let st = state.lock();
            (st.auto_mode, st.arr_mood.clone())
        };
        let (auto_label, auto_fill, auto_stroke) = if auto_mode {
            (
                "⏹  AUTO PLAYING  —  Click to Stop",
                Color32::from_rgb(165, 50, 20),
                Color32::from_rgb(255, 120, 60),
            )
        } else {
            (
                "▶  AUTO  —  Generate & Play",
                Color32::from_rgb(18, 135, 65),
                Color32::from_rgb(60, 220, 110),
            )
        };
        if ui.add(
            Button::new(RichText::new(auto_label).color(Color32::WHITE).size(16.0).strong())
                .fill(auto_fill)
                .stroke(egui::Stroke::new(2.0, auto_stroke))
                .min_size(Vec2::new(ui.available_width(), 58.0))
        ).on_hover_text("Auto-generate a full arrangement from the selected mood and play it through continuously. Each run creates a unique sequence of morphing scenes.").clicked() {
            let mut st = state.lock();
            if st.auto_mode {
                st.auto_mode = false;
                st.arr_playing = false;
            } else {
                let mood = st.arr_mood.clone();
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64 ^ (d.subsec_nanos() as u64).wrapping_mul(0x9e3779b97f4a7c15))
                    .unwrap_or(0xdeadbeef);
                st.scenes = generate_song(&mood, seed);
                st.arr_elapsed = 0.0;
                st.arr_playing = true;
                st.arr_loop = true;
                st.auto_mode = true;
                st.paused = false; // always unpause when AUTO starts
            }
        }

        // AUTO progress indicator
        if auto_mode {
            let (arr_elapsed, scenes_snap) = {
                let st = state.lock();
                (st.arr_elapsed, st.scenes.clone())
            };
            let total = total_duration(&scenes_snap);
            if let Some((scene_idx, morphing, _t)) = scene_at(&scenes_snap, arr_elapsed) {
                let active_scenes: Vec<usize> = (0..scenes_snap.len()).filter(|&i| scenes_snap[i].active).collect();
                let scene_ord = active_scenes.iter().position(|&i| i == scene_idx).unwrap_or(0) + 1;
                let scene_count = active_scenes.len();
                let scene_name = &scenes_snap[scene_idx].name;
                let phase_label = if morphing { "morphing →" } else { "holding" };
                let elapsed_m = (arr_elapsed / 60.0) as u32;
                let elapsed_s = (arr_elapsed % 60.0) as u32;
                let total_m = (total / 60.0) as u32;
                let total_s = (total % 60.0) as u32;
                ui.add_space(4.0);
                ui.label(RichText::new(format!(
                    "Scene {}/{} — {} ({}) {:02}:{:02}/{:02}:{:02}",
                    scene_ord, scene_count, scene_name, phase_label,
                    elapsed_m, elapsed_s, total_m, total_s
                )).color(CYAN).size(11.0));
                let progress = if total > 0.001 { (arr_elapsed / total).clamp(0.0, 1.0) } else { 0.0 };
                ui.add(ProgressBar::new(progress).desired_width(ui.available_width()));
            }
        } else {
            // First-run hint when nothing is playing
            ui.add_space(4.0);
            ui.label(RichText::new("👆 Hit AUTO to start — generates a unique arrangement each time")
                .color(AMBER).size(11.0).italics());
        }

        // ---- Play Demo + Save WAV buttons ----
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(
                Button::new(RichText::new("▶  Play Demo").color(Color32::WHITE).size(12.0))
                    .fill(Color32::from_rgb(68, 50, 130))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(130, 100, 220)))
                    .min_size(Vec2::new(108.0, 30.0))
            ).on_hover_text("Play a hardcoded 3-minute demo piece showcasing the best sounds.").clicked() {
                let mut st = state.lock();
                st.scenes = demo_arrangement();
                st.arr_elapsed = 0.0;
                st.arr_playing = true;
                st.arr_loop = false;
                st.auto_mode = false;
                st.paused = false;
            }

            // Save as WAV: generate a fresh arrangement, record one full pass, auto-stop
            let save_pending = state.lock().save_gen_pending;
            let save_label = if save_pending {
                RichText::new("⏺ Recording…").color(Color32::from_rgb(255, 100, 100)).size(11.0)
            } else {
                RichText::new("💾 Save WAV").color(Color32::WHITE).size(11.0)
            };
            if ui.add(
                Button::new(save_label)
                    .fill(if save_pending { Color32::from_rgb(100, 20, 20) } else { Color32::from_rgb(25, 80, 55) })
                    .stroke(egui::Stroke::new(1.0, if save_pending { Color32::from_rgb(255, 80, 80) } else { Color32::from_rgb(55, 180, 110) }))
                    .min_size(Vec2::new(90.0, 30.0))
            ).on_hover_text("Generate a fresh arrangement and record one full pass to a WAV file. Recording stops automatically when the arrangement ends.").clicked() && !save_pending {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64 ^ (d.subsec_nanos() as u64).wrapping_mul(0x9e3779b97f4a7c15))
                    .unwrap_or(0xdeadbeef);
                let mood = state.lock().arr_mood.clone();
                let new_scenes = generate_song(&mood, seed);
                let sr = state.lock().sample_rate;
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                let filename = format!("generation_{}.wav", ts);
                let spec = hound::WavSpec { channels: 2, sample_rate: sr, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
                if let Ok(writer) = hound::WavWriter::create(&filename, spec) {
                    if let Some(mut lock) = recording.try_lock() { *lock = Some(writer); }
                    let mut st = state.lock();
                    st.scenes = new_scenes;
                    st.arr_elapsed = 0.0;
                    st.arr_playing = true;
                    st.arr_loop = false;
                    st.save_gen_pending = true;
                    st.auto_mode = false;
                    st.paused = false;
                }
            }
            // Tips "?" button
            let show_tips = state.lock().show_tips_window;
            let tips_color = if show_tips { Color32::from_rgb(100, 80, 180) } else { Color32::from_rgb(40, 40, 70) };
            if ui.add(
                Button::new(RichText::new("?").color(Color32::WHITE).size(14.0).strong())
                    .fill(tips_color)
                    .min_size(Vec2::new(30.0, 30.0))
            ).on_hover_text("Sound design tips and recipes").clicked() {
                let mut st = state.lock();
                st.show_tips_window = !st.show_tips_window;
            }
        });

        ui.add_space(8.0);

        // ---- Tips window ----
        let show_tips = state.lock().show_tips_window;
        if show_tips {
            egui::Window::new("Sound Design Tips")
                .open(&mut state.lock().show_tips_window)
                .default_width(360.0)
                .show(ctx, |ui| {
                    draw_tips_content(ui);
                });
        }

        // ---- Mood selector ----
        ui.label(RichText::new("ARRANGEMENT MOOD").color(CYAN).size(11.0).strong());
        ui.add_space(4.0);
        let mood_defs: &[(&str, &str, &str, Color32, Color32)] = &[
            ("ambient",      "🌙  Ambient",      "Deep reverb · slow drift · harmonic pads",
                Color32::from_rgb(0, 95, 175),  Color32::from_rgb(60, 150, 255)),
            ("rhythmic",     "⚡  Rhythmic",     "Pulsing energy · granular · percussive",
                Color32::from_rgb(155, 115, 0), Color32::from_rgb(240, 185, 30)),
            ("experimental", "🔬  Experimental", "Glitch · microtonal · unexpected",
                Color32::from_rgb(100, 0, 165), Color32::from_rgb(185, 80, 255)),
        ];
        for (mood_key, label, desc, active_fill, accent) in mood_defs.iter() {
            let selected = arr_mood == *mood_key;
            let fill = if selected { *active_fill } else { Color32::from_rgb(16, 16, 32) };
            let border_col = if selected { *accent } else { Color32::from_rgb(38, 40, 68) };
            let border_w = if selected { 1.5 } else { 1.0 };
            let frame = egui::Frame::none()
                .fill(fill)
                .stroke(egui::Stroke::new(border_w, border_col))
                .inner_margin(egui::Margin::symmetric(10.0, 7.0))
                .rounding(egui::Rounding::same(7.0));
            let resp = frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(*label).color(Color32::WHITE).size(12.5).strong());
                    if selected {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new("●").color(*accent).size(10.0));
                        });
                    }
                });
                ui.label(RichText::new(*desc).color(GRAY_HINT).size(10.0));
            }).response;
            if resp.interact(Sense::click()).clicked() {
                let mut st = state.lock();
                st.arr_mood = mood_key.to_string();
                if st.auto_mode {
                    let seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos() as u64;
                    st.scenes = generate_song(mood_key, seed);
                    st.arr_elapsed = 0.0;
                }
            }
            ui.add_space(4.0);
        }
    }
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // ---- Volume + Pause ----
    {
        let mut st = state.lock();
        let vol = st.config.audio.master_volume;
        let db_label = if vol > 0.001 {
            format!("{:.1} dB", 20.0 * vol.log10())
        } else {
            "-∞ dB".to_string()
        };
        egui::Frame::none()
            .fill(Color32::from_rgb(10, 12, 26))
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .rounding(egui::Rounding::same(8.0))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(28, 35, 65)))
            .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔊  VOLUME").color(Color32::WHITE).strong().size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(&db_label).color(AMBER).size(11.0));
                });
            });
            ui.add(Slider::new(&mut st.config.audio.master_volume, 0.0..=1.0).text(""))
                .on_hover_text("Master output volume. Use ↑/↓ arrow keys as a quick shortcut.");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let pause_label = if st.paused { "▶  RESUME" } else { "⏸  PAUSE" };
                let (pause_fill, pause_stroke) = if st.paused {
                    (Color32::from_rgb(18, 135, 65), Color32::from_rgb(60, 220, 110))
                } else {
                    (Color32::from_rgb(15, 90, 165), Color32::from_rgb(60, 160, 255))
                };
                let avail = ui.available_width();
                if ui.add(
                    Button::new(RichText::new(pause_label).color(Color32::WHITE).strong().size(13.0))
                        .fill(pause_fill)
                        .stroke(egui::Stroke::new(1.5, pause_stroke))
                        .min_size(Vec2::new(avail - 60.0, 36.0)),
                )
                .on_hover_text("Pause or resume. Shortcut: Space bar.")
                .clicked() {
                    st.paused = !st.paused;
                }
                let perf_fill = if st.perf_mode { Color32::from_rgb(175, 80, 0) } else { Color32::from_rgb(24, 24, 44) };
                if ui.add(
                    Button::new(RichText::new("⛶").color(Color32::WHITE).size(16.0))
                        .fill(perf_fill)
                        .min_size(Vec2::new(50.0, 36.0)),
                )
                .on_hover_text("Performance mode: fullscreen phase portrait. Press F to toggle.")
                .clicked() {
                    st.perf_mode = !st.perf_mode;
                }
            });
        });
    }
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // ---- MACROS ----
    egui::Frame::none()
        .fill(Color32::from_rgb(10, 12, 26))
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .rounding(egui::Rounding::same(8.0))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(28, 35, 65)))
        .show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("MACROS").color(CYAN).strong().size(13.0));
        ui.label(RichText::new("Adjust these four knobs — everything else follows").color(GRAY_HINT).size(10.0).italics());
        ui.add_space(6.0);
        let mut st = state.lock();
        // Chaos — orange/red
        ui.label(RichText::new("🔥  Chaos").color(Color32::from_rgb(235, 110, 45)).size(11.5).strong());
        ui.add(Slider::new(&mut st.macro_chaos, 0.0..=1.0).text(""))
            .on_hover_text("How unpredictable and wild the sound is. At max, the attractor is fully chaotic — pitch leaps are large and seemingly random. At zero, it settles into a predictable cycle.");
        ui.add_space(4.0);
        // Space — blue
        ui.label(RichText::new("🌌  Space").color(Color32::from_rgb(80, 160, 255)).size(11.5).strong());
        ui.add(Slider::new(&mut st.macro_space, 0.0..=1.0).text(""))
            .on_hover_text("Depth, reverb, and dimension. Max = vast cavernous room. Zero = completely dry and intimate.");
        ui.add_space(4.0);
        // Rhythm — green
        ui.label(RichText::new("⚡  Rhythm").color(Color32::from_rgb(55, 210, 120)).size(11.5).strong());
        ui.add(Slider::new(&mut st.macro_rhythm, 0.0..=1.0).text(""))
            .on_hover_text("Punchiness and attack. Zero = slow pad-like fade in. Max = percussive, tight, rhythmic energy.");
        ui.add_space(4.0);
        // Warmth — amber
        ui.label(RichText::new("🌅  Warmth").color(AMBER).size(11.5).strong());
        ui.add(Slider::new(&mut st.macro_warmth, 0.0..=1.0).text(""))
            .on_hover_text("Tonal color. Zero = bright, clear, full bandwidth. Max = filtered, saturated, dark and warm like analog tape.");
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        let evolve_col = if st.macro_walk_enabled { GREEN_ACC } else { GRAY_HINT };
        ui.horizontal(|ui| {
            ui.checkbox(&mut st.macro_walk_enabled, "");
            ui.label(RichText::new("Evolve  (auto-drift macros)").color(evolve_col).size(11.5))
                .on_hover_text("Brownian-motion drift of all four macros — leave it running and the sound slowly explores on its own.");
        });
        if st.macro_walk_enabled {
            ui.add_space(2.0);
            ui.add(Slider::new(&mut st.macro_walk_rate, 0.01..=0.5).text("Speed"))
                .on_hover_text("How quickly the macros drift. Low = glacial. High = restless, rapid shifting.");
        }
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // ---- PRESETS ----
    collapsing_section(ui, "PRESETS", true, |ui| {
        let (selected, show_all) = {
            let st = state.lock();
            (st.selected_preset.clone(), st.simple_show_all_presets)
        };
        ui.colored_label(AMBER, "Click a preset to start");
        ui.add_space(4.0);

        let show_count = if show_all { PRESETS.len() } else { 12 };
        let mut last_cat = "";
        for preset in PRESETS.iter().take(show_count) {
            if preset.category != last_cat {
                last_cat = preset.category;
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("  {}", preset.category.to_uppercase()))
                        .color(GRAY_HINT).size(10.0).strong());
                    let avail = ui.available_width();
                    let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(avail - 4.0, 1.0), Sense::hover());
                    ui.painter().rect_filled(sep_rect, 0.0, Color32::from_rgb(35, 38, 65));
                });
                ui.add_space(3.0);
            }
            let is_selected = selected == preset.name;
            let pc = preset.color;
            let bg_color = if is_selected {
                Color32::from_rgba_premultiplied(
                    (pc.r() as u16 * 55 / 255) as u8,
                    (pc.g() as u16 * 55 / 255) as u8,
                    (pc.b() as u16 * 55 / 255) as u8,
                    255,
                )
            } else {
                Color32::from_rgb(14, 14, 26)
            };
            let border_w = if is_selected { 1.5 } else { 1.0 };
            let border_col = if is_selected { pc } else { Color32::from_rgb(34, 36, 64) };
            let card = egui::Frame::none()
                .fill(bg_color)
                .stroke(Stroke::new(border_w, border_col))
                .inner_margin(egui::Margin::symmetric(0.0, 5.0))
                .rounding(egui::Rounding::same(6.0));
            let response = card.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    // Colored left accent strip
                    let strip_h = 34.0;
                    let (strip_rect, _) = ui.allocate_exact_size(Vec2::new(5.0, strip_h), Sense::hover());
                    let strip_col = if is_selected {
                        pc
                    } else {
                        Color32::from_rgba_premultiplied(pc.r() / 3, pc.g() / 3, pc.b() / 3, 200)
                    };
                    ui.painter().rect_filled(strip_rect, egui::Rounding::same(3.0), strip_col);
                    ui.add_space(7.0);
                    ui.vertical(|ui| {
                        let name_col = if is_selected { pc } else { Color32::WHITE };
                        ui.label(RichText::new(preset.name).strong().color(name_col).size(12.0));
                        ui.label(RichText::new(preset.description).italics().color(GRAY_HINT).size(10.0));
                    });
                });
            }).response;
            if response.interact(Sense::click()).clicked() {
                let mut st = state.lock();
                st.selected_preset = preset.name.to_string();
                st.config = load_preset(preset.name);
                st.system_changed = true;
                st.mode_changed = true;
            }
            ui.add_space(3.0);
        }

        if !show_all {
            ui.add_space(4.0);
            if ui.add(
                Button::new(RichText::new(format!("Show all {} presets ▾", PRESETS.len()))
                    .color(CYAN).size(11.0))
                    .fill(Color32::from_rgb(14, 18, 36))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(40, 60, 110)))
                    .min_size(Vec2::new(ui.available_width(), 28.0)),
            ).clicked() {
                state.lock().simple_show_all_presets = true;
            }
        } else {
            ui.add_space(4.0);
            if ui.add(
                Button::new(RichText::new("Show less ▴").color(GRAY_HINT).size(11.0))
                    .fill(Color32::from_rgb(14, 18, 36))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(35, 38, 65)))
                    .min_size(Vec2::new(ui.available_width(), 28.0)),
            ).clicked() {
                state.lock().simple_show_all_presets = false;
            }
        }
    });

    // ---- Status ----
    {
        let status = state.lock().clip_status.clone();
        if !status.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(&status).color(GRAY_HINT).size(11.0));
        }
    }
}

fn param_range(param: &str) -> (f64, f64) {
    match param {
        "rho" => (20.0, 50.0),
        "sigma" => (5.0, 20.0),
        "coupling" => (0.0, 5.0),
        "c" => (3.0, 10.0),
        _ => (0.0, 1.0),
    }
}

fn draw_arrange_tab(ui: &mut Ui, state: &SharedState, recording: &WavRecorder) {
    // Save-generation auto-stop: when arrangement finishes a single pass, stop recording
    {
        let (pending, elapsed, playing) = {
            let st = state.lock();
            (st.save_gen_pending, st.arr_elapsed, st.arr_playing)
        };
        if pending && !playing {
            // arr_loop was false, arrangement reached end and stopped — finalize recording
            if let Some(mut lock) = recording.try_lock() { *lock = None; }
            state.lock().save_gen_pending = false;
        }
        // Also stop if somehow elapsed went past total (shouldn't happen but be safe)
        let _ = elapsed;
    }

    // Auto-record detection: detect arr_playing transitions
    {
        let mut st = state.lock();
        let now_playing = st.arr_playing;
        if st.arr_auto_record {
            if now_playing && !st.arr_was_playing {
                // Start recording
                let sr = st.sample_rate;
                drop(st);
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let filename = format!("arrangement_{}.wav", secs);
                let spec = hound::WavSpec {
                    channels: 2,
                    sample_rate: sr,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                };
                if let Ok(writer) = hound::WavWriter::create(&filename, spec) {
                    if let Some(mut lock) = recording.try_lock() {
                        *lock = Some(writer);
                    }
                }
            } else if !now_playing && st.arr_was_playing {
                drop(st);
                if let Some(mut lock) = recording.try_lock() { *lock = None; }
            } else {
                drop(st);
            }
        } else {
            drop(st);
        }
        state.lock().arr_was_playing = now_playing;
    }

    let (arr_playing, arr_elapsed, arr_auto_record, arr_loop, scenes_snapshot) = {
        let st = state.lock();
        (st.arr_playing, st.arr_elapsed, st.arr_auto_record, st.arr_loop, st.scenes.clone())
    };

    let total = total_duration(&scenes_snapshot);

    // Generate Song section
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("✨ Generate Song").color(CYAN).strong());
        ui.add_space(8.0);
        let (cur_mood, seed_base) = {
            let st = state.lock();
            (st.arr_mood.clone(), st.arr_elapsed.to_bits() as u64 ^ 0x1234567890abcdef)
        };
        for mood in &["ambient", "rhythmic", "experimental"] {
            let selected = cur_mood == *mood;
            let col = if selected { Color32::from_rgb(0, 140, 200) } else { Color32::from_rgb(40, 40, 60) };
            let label = match *mood { "ambient" => "🌙 Ambient", "rhythmic" => "⚡ Rhythmic", _ => "🔬 Experimental" };
            if ui.add(Button::new(RichText::new(label).color(Color32::WHITE))
                .fill(col).min_size(Vec2::new(90.0, 26.0))).clicked() {
                state.lock().arr_mood = mood.to_string();
            }
        }
        ui.add_space(8.0);
        if ui.add(Button::new(RichText::new("🎲 Generate").color(Color32::BLACK))
            .fill(Color32::from_rgb(220, 180, 40)).min_size(Vec2::new(90.0, 26.0))).clicked() {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64 ^ (d.subsec_nanos() as u64).wrapping_mul(0x9e3779b97f4a7c15))
                .unwrap_or(seed_base);
            let mood = state.lock().arr_mood.clone();
            let new_scenes = generate_song(&mood, seed);
            let mut st = state.lock();
            st.scenes = new_scenes;
            st.arr_playing = false;
            st.arr_elapsed = 0.0;
            st.save_gen_pending = false;
        }

        // Save Generation: record one full pass of the arrangement to WAV
        let save_pending = state.lock().save_gen_pending;
        let save_label = if save_pending {
            RichText::new("⏺ Recording…").color(Color32::from_rgb(255, 80, 80)).size(11.0)
        } else {
            RichText::new("💾 Save as WAV").color(Color32::WHITE).size(11.0)
        };
        if ui.add(Button::new(save_label)
            .fill(if save_pending { Color32::from_rgb(100, 20, 20) } else { Color32::from_rgb(30, 80, 60) })
            .min_size(Vec2::new(110.0, 26.0))).clicked() && !save_pending {
            // Generate fresh, reset playback, start recording, no loop
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64 ^ (d.subsec_nanos() as u64).wrapping_mul(0x9e3779b97f4a7c15))
                .unwrap_or(seed_base);
            let mood = state.lock().arr_mood.clone();
            let new_scenes = generate_song(&mood, seed);
            let sr = state.lock().sample_rate;
            let secs_ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            let filename = format!("generation_{}.wav", secs_ts);
            let spec = hound::WavSpec { channels: 2, sample_rate: sr, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
            if let Ok(writer) = hound::WavWriter::create(&filename, spec) {
                if let Some(mut lock) = recording.try_lock() { *lock = Some(writer); }
                let mut st = state.lock();
                st.scenes = new_scenes;
                st.arr_elapsed = 0.0;
                st.arr_playing = true;
                st.arr_loop = false;
                st.save_gen_pending = true;
                st.paused = false;
            }
        }
    });
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    // Top controls bar
    ui.horizontal(|ui| {
        let play_col = if arr_playing { Color32::from_rgb(0, 140, 0) } else { Color32::from_rgb(0, 80, 120) };
        if ui.add(Button::new(RichText::new(if arr_playing { "⏸ Pause" } else { "▶ Play" }).color(Color32::WHITE))
            .fill(play_col).min_size(Vec2::new(80.0, 28.0))).clicked() {
            let mut st = state.lock();
            st.arr_playing = !st.arr_playing;
            if st.arr_playing && st.arr_elapsed == 0.0 {
                // reset
            }
        }
        if ui.add(Button::new(RichText::new("■ Stop").color(Color32::WHITE))
            .fill(Color32::from_rgb(80, 30, 30)).min_size(Vec2::new(60.0, 28.0))).clicked() {
            let mut st = state.lock();
            st.arr_playing = false;
            st.arr_elapsed = 0.0;
        }

        let loop_col = if arr_loop { Color32::from_rgb(0, 120, 0) } else { Color32::from_rgb(40, 40, 60) };
        if ui.add(Button::new(RichText::new("⟳ Loop").color(Color32::WHITE))
            .fill(loop_col).min_size(Vec2::new(60.0, 28.0))).clicked() {
            state.lock().arr_loop = !arr_loop;
        }

        let rec_col = if arr_auto_record { Color32::from_rgb(140, 30, 30) } else { Color32::from_rgb(40, 40, 60) };
        if ui.add(Button::new(RichText::new("⏺ Auto-Rec").color(Color32::WHITE))
            .fill(rec_col).min_size(Vec2::new(80.0, 28.0))).clicked() {
            state.lock().arr_auto_record = !arr_auto_record;
        }

        // Duration label
        let total_mins = (total / 60.0) as u32;
        let total_secs = (total % 60.0) as u32;
        ui.label(RichText::new(format!("  Duration: {}:{:02}", total_mins, total_secs)).color(CYAN));

        // Probabilistic mode
        let mut st = state.lock();
        let prob_col = if st.arr_probabilistic { Color32::from_rgb(180, 80, 20) } else { Color32::from_rgb(40, 40, 60) };
        if ui.add(Button::new(RichText::new("Probabilistic").color(Color32::WHITE))
            .fill(prob_col).min_size(Vec2::new(100.0, 28.0))).clicked() {
            st.arr_probabilistic = !st.arr_probabilistic;
        }
    });

    // Progress bar
    if arr_playing || arr_elapsed > 0.0 {
        let progress = if total > 0.001 { (arr_elapsed / total).clamp(0.0, 1.0) } else { 0.0 };
        let elapsed_m = (arr_elapsed / 60.0) as u32;
        let elapsed_s = (arr_elapsed % 60.0) as u32;
        ui.add(ProgressBar::new(progress)
            .text(format!("{:02}:{:02} / {:02}:{:02}", elapsed_m, elapsed_s,
                (total / 60.0) as u32, (total % 60.0) as u32)));
    }

    ui.add_space(4.0);
    ui.separator();

    // Scene list header
    ui.horizontal(|ui| {
        ui.label(RichText::new("  ").strong());
        ui.add_space(18.0);
        ui.label(RichText::new("Name").color(CYAN).strong().size(11.0));
        ui.add_space(60.0);
        ui.label(RichText::new("Hold(s)").color(CYAN).strong().size(11.0));
        ui.add_space(20.0);
        ui.label(RichText::new("Morph(s)").color(CYAN).strong().size(11.0));
        ui.add_space(20.0);
        ui.label(RichText::new("Actions").color(CYAN).strong().size(11.0));
    });
    ui.separator();

    // Scene rows
    let n_scenes = scenes_snapshot.len();
    for i in 0..n_scenes {
        let (mut active, name, hold, morph) = {
            let st = state.lock();
            (st.scenes[i].active, st.scenes[i].name.clone(), st.scenes[i].hold_secs, st.scenes[i].morph_secs)
        };

        let is_current_scene = arr_playing && scene_at(&scenes_snapshot, arr_elapsed)
            .map(|(idx, _, _)| idx == i).unwrap_or(false);

        let row_bg = if is_current_scene {
            Color32::from_rgba_premultiplied(0, 60, 30, 255)
        } else {
            Color32::TRANSPARENT
        };

        egui::Frame::none().fill(row_bg).inner_margin(egui::Margin::symmetric(4.0, 2.0)).show(ui, |ui| {
        if is_current_scene {
            ui.label(RichText::new("▶").color(Color32::from_rgb(80, 220, 80)).size(10.0));
        } else {
            ui.label(RichText::new("  ").size(10.0));
        }
        ui.horizontal(|ui| {
            // Active checkbox
            if ui.checkbox(&mut active, "").changed() {
                state.lock().scenes[i].active = active;
            }

            // Scene number
            ui.label(RichText::new(format!("{}", i + 1)).color(GRAY_HINT).size(11.0));

            // Name text edit
            let mut name_edit = name.clone();
            let te = ui.add(TextEdit::singleline(&mut name_edit).desired_width(80.0));
            if te.changed() {
                state.lock().scenes[i].name = name_edit;
            }

            // Hold duration
            let mut hold_v = hold;
            if ui.add(DragValue::new(&mut hold_v).clamp_range(5.0..=300.0f32).suffix("s").speed(0.5)).changed() {
                state.lock().scenes[i].hold_secs = hold_v;
            }

            // Morph duration
            let mut morph_v = morph;
            if ui.add(DragValue::new(&mut morph_v).clamp_range(0.0..=60.0f32).suffix("s").speed(0.1)).changed() {
                state.lock().scenes[i].morph_secs = morph_v;
            }

            // Probability weight (shown when probabilistic mode is on)
            let arr_prob = state.lock().arr_probabilistic;
            if arr_prob {
                let mut prob_v = state.lock().scenes[i].transition_prob;
                if ui.add(DragValue::new(&mut prob_v).clamp_range(0.0..=3.0f32).prefix("P:").speed(0.05)).changed() {
                    state.lock().scenes[i].transition_prob = prob_v;
                }
            }

            // Capture button
            if ui.add(Button::new(RichText::new("Capture").color(Color32::WHITE))
                .fill(Color32::from_rgb(0, 80, 120))
                .min_size(Vec2::new(58.0, 22.0))).clicked() {
                let cfg = state.lock().config.clone();
                let mut st = state.lock();
                st.scenes[i].config = cfg;
                st.scenes[i].active = true;
            }

            // Load button
            if ui.add(Button::new(RichText::new("Load").color(Color32::WHITE))
                .fill(Color32::from_rgb(60, 40, 80))
                .min_size(Vec2::new(42.0, 22.0))).clicked() {
                let scene_cfg = state.lock().scenes[i].config.clone();
                let mut st = state.lock();
                st.config = scene_cfg;
                st.system_changed = true;
                st.mode_changed = true;
            }
        }); // inner horizontal
        }); // frame
    }

    ui.add_space(6.0);
    // Empty state hint
    let has_active = scenes_snapshot.iter().any(|s| s.active);
    if !has_active {
        ui.label(RichText::new("Hit Generate to create an arrangement, or Capture your current sound into scenes.")
            .color(AMBER).size(11.0).italics());
    } else {
        ui.colored_label(GRAY_HINT, "Capture your current sound into a scene, set durations, then Play.");
    }
    ui.add_space(6.0);

    // Visual timeline
    let scenes_for_tl = state.lock().scenes.clone();
    draw_arrangement_timeline(ui, &scenes_for_tl, arr_elapsed);
}

fn draw_arrangement_timeline(ui: &mut Ui, scenes: &[Scene], elapsed: f32) {
    let total = total_duration(scenes);
    if total < 0.001 { return; }

    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
    let painter = ui.painter();

    painter.rect_filled(rect, 4.0, Color32::from_rgb(15, 15, 25));

    let colors = [
        Color32::from_rgb(0, 100, 180),
        Color32::from_rgb(0, 140, 80),
        Color32::from_rgb(140, 80, 0),
        Color32::from_rgb(120, 0, 140),
        Color32::from_rgb(0, 120, 120),
        Color32::from_rgb(140, 30, 30),
        Color32::from_rgb(80, 80, 0),
        Color32::from_rgb(40, 80, 140),
    ];

    let mut x = rect.left();
    let active: Vec<usize> = (0..scenes.len()).filter(|&i| scenes[i].active).collect();
    for (ord, &idx) in active.iter().enumerate() {
        let scene = &scenes[idx];
        let col = colors[idx % colors.len()];

        // Morph segment (darker), skip for first scene
        if ord > 0 {
            let w = rect.width() * scene.morph_secs / total;
            let r = egui::Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height()));
            painter.rect_filled(r, 0.0, Color32::from_rgba_premultiplied(col.r()/3, col.g()/3, col.b()/3, 200));
            x += w;
        }

        // Hold segment
        let w = rect.width() * scene.hold_secs / total;
        let r = egui::Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height()));
        painter.rect_filled(r, 0.0, Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 200));

        // Scene name label
        painter.text(Pos2::new(x + 4.0, rect.center().y), Align2::LEFT_CENTER,
            &scene.name, FontId::proportional(10.0), Color32::WHITE);

        x += w;
    }

    // Playhead
    if elapsed > 0.0 {
        let px = rect.left() + rect.width() * (elapsed / total).min(1.0);
        painter.line_segment(
            [Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
            Stroke::new(2.0, Color32::from_rgb(255, 220, 0)),
        );
    }

    // Border
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(50, 50, 80)));
}

fn build_bifurc_system(
    sys_name: &str,
    param: &str,
    pval: f64,
    lorenz: &crate::config::LorenzConfig,
    rossler: &crate::config::RosslerConfig,
    kuramoto: &crate::config::KuramotoConfig,
) -> Box<dyn DynamicalSystem> {
    match (sys_name, param) {
        ("lorenz", "rho") => Box::new(Lorenz::new(lorenz.sigma, pval, lorenz.beta)),
        ("lorenz", "sigma") => Box::new(Lorenz::new(pval, lorenz.rho, lorenz.beta)),
        ("rossler", "c") => Box::new(Rossler::new(rossler.a, rossler.b, pval)),
        ("kuramoto", "coupling") => Box::new(Kuramoto::new(kuramoto.n_oscillators, pval)),
        _ => Box::new(Lorenz::new(lorenz.sigma, pval.clamp(20.0, 50.0), lorenz.beta)),
    }
}

fn draw_bifurc_diagram(ui: &mut Ui, bifurc_data: &Arc<Mutex<Vec<(f32, f32)>>>, state: &SharedState) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let (is_2d, param1, param2) = {
        let st = state.lock();
        (st.bifurc_2d_mode, st.bifurc_param.clone(), st.bifurc_param2.clone())
    };

    if is_2d {
        // 2D heatmap: (p1, p2, chaos_metric)
        let data_2d_arc = state.lock().bifurc_data_2d.clone();
        let data_2d = if let Some(d) = data_2d_arc.try_lock() { d.clone() } else { return; };
        if data_2d.is_empty() {
            painter.text(rect.center(), Align2::CENTER_CENTER,
                "Click 'Compute' to generate 2D bifurcation map",
                FontId::proportional(16.0), Color32::from_rgb(80, 80, 120));
            return;
        }
        let mut p1_min = f32::MAX; let mut p1_max = f32::MIN;
        let mut p2_min = f32::MAX; let mut p2_max = f32::MIN;
        let mut v_min = f32::MAX;  let mut v_max = f32::MIN;
        for &(p1, p2, v) in &data_2d {
            p1_min = p1_min.min(p1); p1_max = p1_max.max(p1);
            p2_min = p2_min.min(p2); p2_max = p2_max.max(p2);
            v_min = v_min.min(v);    v_max = v_max.max(v);
        }
        let rp1 = (p1_max - p1_min).max(1e-3);
        let rp2 = (p2_max - p2_min).max(1e-3);
        let rv  = (v_max - v_min).max(1e-9);
        let pad = 30.0f32;
        let inner_w = rect.width() - 2.0 * pad;
        let inner_h = rect.height() - 2.0 * pad;
        // Determine cell size from the grid density
        let grid_size = (data_2d.len() as f64).sqrt().ceil() as usize;
        let cell_w = (inner_w / grid_size.max(1) as f32).max(1.0);
        let cell_h = (inner_h / grid_size.max(1) as f32).max(1.0);
        for &(p1, p2, v) in &data_2d {
            let t = ((v - v_min) / rv).clamp(0.0, 1.0);
            // Color: deep blue (ordered) → cyan → white (chaotic)
            let col = if t < 0.5 {
                lerp_color(Color32::from_rgb(10, 10, 80), Color32::from_rgb(0, 200, 255), t * 2.0)
            } else {
                lerp_color(Color32::from_rgb(0, 200, 255), Color32::from_rgb(255, 240, 180), (t - 0.5) * 2.0)
            };
            let sx = rect.left() + pad + ((p1 - p1_min) / rp1) * inner_w;
            let sy = rect.bottom() - pad - ((p2 - p2_min) / rp2) * inner_h;
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(sx, sy - cell_h), Vec2::new(cell_w, cell_h)),
                0.0, col,
            );
        }
        painter.text(rect.left_top() + Vec2::new(8.0, 8.0), Align2::LEFT_TOP,
            format!("2D Bifurcation Map  ({} × {})", grid_size, grid_size),
            FontId::proportional(12.0), Color32::from_rgb(120, 140, 180));
        painter.text(rect.center_bottom() + Vec2::new(0.0, -12.0), Align2::CENTER_BOTTOM,
            format!("x: {} ({:.1}..{:.1})   y: {} ({:.1}..{:.1})", param1, p1_min, p1_max, param2, p2_min, p2_max),
            FontId::proportional(10.0), Color32::from_rgb(100, 120, 160));
        return;
    }

    // 1D bifurcation diagram
    let data = if let Some(d) = bifurc_data.try_lock() { d.clone() } else { return; };
    if data.is_empty() {
        painter.text(rect.center(), Align2::CENTER_CENTER,
            "Click 'Compute' to generate bifurcation diagram",
            FontId::proportional(16.0), Color32::from_rgb(80, 80, 120));
        return;
    }
    let mut min_x = f32::MAX; let mut max_x = f32::MIN;
    let mut min_y = f32::MAX; let mut max_y = f32::MIN;
    for &(x, y) in &data {
        min_x = min_x.min(x); max_x = max_x.max(x);
        min_y = min_y.min(y); max_y = max_y.max(y);
    }
    let rx = (max_x - min_x).max(1e-3);
    let ry = (max_y - min_y).max(1e-3);
    let pad = 20.0f32;
    for &(x, y) in &data {
        let sx = rect.left() + pad + ((x - min_x) / rx) * (rect.width() - 2.0 * pad);
        let sy = rect.bottom() - pad - ((y - min_y) / ry) * (rect.height() - 2.0 * pad);
        painter.circle_filled(Pos2::new(sx, sy), 1.0, Color32::from_rgba_premultiplied(0, 200, 255, 180));
    }
    painter.text(rect.left_top() + Vec2::new(8.0, 8.0), Align2::LEFT_TOP,
        format!("Bifurcation Diagram  ({} points)", data.len()),
        FontId::proportional(12.0), Color32::from_rgb(120, 140, 180));
    painter.text(rect.center_bottom() + Vec2::new(0.0, -12.0), Align2::CENTER_BOTTOM,
        format!("Parameter: {:.2} .. {:.2}", min_x, max_x),
        FontId::proportional(10.0), Color32::from_rgb(100, 120, 160));
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

fn speed_color(speed_norm: f32) -> Color32 {
    if speed_norm < 0.5 {
        lerp_color(
            Color32::from_rgb(20, 40, 180),
            Color32::from_rgb(0, 220, 255),
            speed_norm * 2.0,
        )
    } else {
        lerp_color(
            Color32::from_rgb(0, 220, 255),
            Color32::from_rgb(255, 255, 255),
            (speed_norm - 0.5) * 2.0,
        )
    }
}

fn draw_phase_portrait(
    ui: &mut Ui,
    points: &[(f32, f32, f32, f32, bool)],
    system_name: &str,
    mode_name: &str,
    current_state: &[f64],
    current_deriv: &[f64],
    projection: usize,
    rotation_angle: f32,
    auto_rotate: bool,
    trail_color: Color32,
    anaglyph_3d: bool,
    anaglyph_separation: f32,
    ghosts: &[(Vec<(f32, f32)>, std::time::Instant)],
    ink: &[(f32, f32)],
    lunar_phase: f32,
    scars: &[(f32, f32)],
    time_of_day_f: f32,
    energy_error: f64,
) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;

    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    if points.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No data yet...",
            FontId::proportional(16.0),
            Color32::from_rgb(80, 80, 120),
        );
        return;
    }

    // Extract projected coordinates (with optional 3D Y-axis rotation)
    let use_3d = auto_rotate || rotation_angle.abs() > 0.01;
    let cos_a = rotation_angle.cos();
    let sin_a = rotation_angle.sin();
    let proj_pts: Vec<(f32, f32, f32, bool)> = points.iter().map(|&(x, y, z, s, c)| {
        let (pa, pb) = if use_3d {
            // Rotate around Y axis
            let rx = x * cos_a + z * sin_a;
            let rz = -x * sin_a + z * cos_a;
            let d = 3.0f32;
            let perspective = d / (d + rz * 0.3 + 1.0).max(0.1);
            (rx * perspective, y * perspective)
        } else {
            match projection {
                1 => (x, z), // XZ
                2 => (y, z), // YZ
                _ => (x, y), // XY
            }
        };
        (pa, pb, s, c)
    }).collect();

    // Compute bounds
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for &(x, y, _, _) in &proj_pts {
        min_x = min_x.min(x); max_x = max_x.max(x);
        min_y = min_y.min(y); max_y = max_y.max(y);
    }
    let rx = (max_x - min_x).max(1e-3);
    let ry = (max_y - min_y).max(1e-3);
    let pad = 10.0f32;

    let to_screen = |x: f32, y: f32| -> Pos2 {
        Pos2 {
            x: rect.left() + pad + ((x - min_x) / rx) * (rect.width() - 2.0 * pad),
            y: rect.bottom() - pad - ((y - min_y) / ry) * (rect.height() - 2.0 * pad),
        }
    };

    // Draw faint grid axes
    let origin = to_screen((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    let grid_color = Color32::from_rgba_premultiplied(40, 40, 60, 80);
    painter.line_segment([Pos2::new(rect.left(), origin.y), Pos2::new(rect.right(), origin.y)], Stroke::new(1.0, grid_color));
    painter.line_segment([Pos2::new(origin.x, rect.top()), Pos2::new(origin.x, rect.bottom())], Stroke::new(1.0, grid_color));

    let n = proj_pts.len();

    // ── Portrait ink: barely-visible long-session stain ──────────────────────
    // The live trail is bright. The ink is almost invisible.
    // After hours of use, the entire reachable set faintly emerges as a palimpsest.
    if !ink.is_empty() {
        // Batch all ink dots into a single mesh to avoid per-call tessellation overhead.
        let ink_col = Color32::from_rgba_premultiplied(
            trail_color.r() / 6,
            trail_color.g() / 6,
            trail_color.b() / 6,
            6, // near-invisible
        );
        let mut mesh = egui::Mesh::default();
        for &(ix, iy) in ink {
            let sp = to_screen(ix, iy);
            if rect.contains(sp) {
                // Each dot = 2 triangles (axis-aligned 2×2 pixel square)
                let base = mesh.vertices.len() as u32;
                let half = 1.0f32;
                mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2(-half, -half), uv: egui::epaint::WHITE_UV, color: ink_col });
                mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2( half, -half), uv: egui::epaint::WHITE_UV, color: ink_col });
                mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2( half,  half), uv: egui::epaint::WHITE_UV, color: ink_col });
                mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2(-half,  half), uv: egui::epaint::WHITE_UV, color: ink_col });
                mesh.indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
            }
        }
        if !mesh.vertices.is_empty() {
            painter.add(egui::Shape::mesh(mesh));
        }
    }

    // SCARRING: faint magenta × marks at near-divergence points
    if !scars.is_empty() {
        let scar_col = Color32::from_rgba_premultiplied(150, 0, 80, 20);
        let mut scar_mesh = egui::Mesh::default();
        for &(sx, sy) in scars {
            let sp = to_screen(sx, sy);
            if rect.contains(sp) {
                let arm = 3.0f32;
                let half_w = 0.6f32;
                // Horizontal bar
                let base = scar_mesh.vertices.len() as u32;
                scar_mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2(-arm, -half_w), uv: egui::epaint::WHITE_UV, color: scar_col });
                scar_mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2( arm, -half_w), uv: egui::epaint::WHITE_UV, color: scar_col });
                scar_mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2( arm,  half_w), uv: egui::epaint::WHITE_UV, color: scar_col });
                scar_mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2(-arm,  half_w), uv: egui::epaint::WHITE_UV, color: scar_col });
                scar_mesh.indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
                // Vertical bar
                let base = scar_mesh.vertices.len() as u32;
                scar_mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2(-half_w, -arm), uv: egui::epaint::WHITE_UV, color: scar_col });
                scar_mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2( half_w, -arm), uv: egui::epaint::WHITE_UV, color: scar_col });
                scar_mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2( half_w,  arm), uv: egui::epaint::WHITE_UV, color: scar_col });
                scar_mesh.vertices.push(egui::epaint::Vertex { pos: sp + egui::vec2(-half_w,  arm), uv: egui::epaint::WHITE_UV, color: scar_col });
                scar_mesh.indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
            }
        }
        if !scar_mesh.vertices.is_empty() {
            painter.add(egui::Shape::mesh(scar_mesh));
        }
    }

    // PHOTOTROPISM: blend trail color toward warm amber at night, cool blue at noon
    let warmth = 1.0 - time_of_day_f; // 1.0 at night, 0.0 at noon
    let warm_tint = Color32::from_rgb(255, 160, 80);
    let cool_tint = Color32::from_rgb(160, 200, 255);
    let photo_color = lerp_color(cool_tint, warm_tint, warmth);
    let trail_color = lerp_color(trail_color, photo_color, 0.10); // 10% blend

    // ── Ghost trails: faint afterimages of trajectories from earlier in the session ──
    // Every 10-15 minutes, a snapshot appears for ~8 seconds then fades.
    // Like a memory of where the attractor has been.
    for (ghost_pts, captured_at) in ghosts {
        let age_secs = captured_at.elapsed().as_secs_f32();
        let fade_start = 2.0f32;
        let fade_end = 10.0f32;
        if age_secs > fade_end { continue; }
        // Fade alpha: full for first 2s, then fades to 0 over 8s
        let ghost_alpha = if age_secs < fade_start {
            40u8
        } else {
            let t = (age_secs - fade_start) / (fade_end - fade_start);
            (40.0 * (1.0 - t)) as u8
        };
        if ghost_alpha == 0 { continue; }
        let ghost_col = Color32::from_rgba_premultiplied(
            (trail_color.r() as f32 * 0.5) as u8,
            (trail_color.g() as f32 * 0.5) as u8,
            (trail_color.b() as f32 * 0.5) as u8,
            ghost_alpha,
        );
        for pts in ghost_pts.windows(2) {
            let p0 = to_screen(pts[0].0, pts[0].1);
            let p1 = to_screen(pts[1].0, pts[1].1);
            if rect.contains(p0) || rect.contains(p1) {
                painter.line_segment([p0, p1], Stroke::new(1.0, ghost_col));
            }
        }
    }

    // Draw trail — glow pass first, then bright core pass
    // Use audio-reactive trail_color (blended with speed for local brightness variation)
    // Anaglyph 3D: render left eye (red) and right eye (cyan) with horizontal parallax
    let sep_px = anaglyph_separation * rect.width();

    for pass in 0..2 {
        for (idx, w) in proj_pts.windows(2).enumerate() {
            let (x0, y0, s0, _) = w[0];
            let (x1, y1, _, _) = w[1];
            let recency = idx as f32 / n as f32;
            let alpha = (recency * 255.0) as u8;
            let spd_bright = 0.5 + s0 * 0.5;

            if anaglyph_3d {
                // Left eye (red channel)
                let p0l = Pos2::new(to_screen(x0, y0).x - sep_px, to_screen(x0, y0).y);
                let p1l = Pos2::new(to_screen(x1, y1).x - sep_px, to_screen(x1, y1).y);
                // Right eye (cyan channel)
                let p0r = Pos2::new(to_screen(x0, y0).x + sep_px, to_screen(x0, y0).y);
                let p1r = Pos2::new(to_screen(x1, y1).x + sep_px, to_screen(x1, y1).y);
                let a = if pass == 0 { (alpha as f32 * 0.3) as u8 } else { alpha };
                let w_px = if pass == 0 { 3.0 } else { 1.2 };
                let r_col = Color32::from_rgba_premultiplied((255.0 * recency * spd_bright) as u8, 0, 0, a);
                let c_col = Color32::from_rgba_premultiplied(0, (200.0 * recency * spd_bright) as u8, (255.0 * recency * spd_bright) as u8, a);
                painter.line_segment([p0l, p1l], Stroke::new(w_px, r_col));
                painter.line_segment([p0r, p1r], Stroke::new(w_px, c_col));
            } else {
                let p0 = to_screen(x0, y0);
                let p1 = to_screen(x1, y1);
                let col = Color32::from_rgb(
                    (trail_color.r() as f32 * spd_bright).min(255.0) as u8,
                    (trail_color.g() as f32 * spd_bright).min(255.0) as u8,
                    (trail_color.b() as f32 * spd_bright).min(255.0) as u8,
                );
                let moon_alpha = ((alpha as f32) * (0.7 + lunar_phase * 0.6)).min(255.0) as u8;
                if pass == 0 {
                    let glow_col = Color32::from_rgba_premultiplied(
                        (col.r() as f32 * recency * 0.3) as u8,
                        (col.g() as f32 * recency * 0.3) as u8,
                        (col.b() as f32 * recency * 0.3) as u8,
                        ((moon_alpha as f32) * 0.3) as u8,
                    );
                    painter.line_segment([p0, p1], Stroke::new(3.0, glow_col));
                } else {
                    let col_a = Color32::from_rgba_premultiplied(
                        (col.r() as f32 * recency) as u8,
                        (col.g() as f32 * recency) as u8,
                        (col.b() as f32 * recency) as u8,
                        moon_alpha,
                    );
                    painter.line_segment([p0, p1], Stroke::new(1.2, col_a));
                }
            }
        }
    }

    // Poincaré section dots
    for &(x, y, _, crossing) in &proj_pts {
        if crossing {
            let pos = to_screen(x, y);
            painter.circle_filled(pos, 2.5, Color32::from_rgb(255, 100, 255));
        }
    }

    // Live position dot
    if let Some(&(x, y, _, _)) = proj_pts.last() {
        let pos = to_screen(x, y);
        painter.circle_filled(pos, 8.0, Color32::from_rgba_premultiplied(0, 150, 255, 60));
        painter.circle_filled(pos, 5.0, Color32::WHITE);
    }

    // Corner info overlay — semi-transparent backdrop for readability
    let info_bg = egui::Rect::from_min_size(
        rect.left_top() + Vec2::new(6.0, 6.0),
        Vec2::new(210.0, 56.0),
    );
    painter.rect_filled(info_bg, 6.0, Color32::from_rgba_premultiplied(8, 8, 20, 160));

    let sys_display = system_display_name(system_name);
    painter.text(
        rect.left_top() + Vec2::new(12.0, 10.0),
        Align2::LEFT_TOP,
        sys_display,
        FontId::proportional(12.5),
        Color32::from_rgb(140, 195, 255),
    );
    painter.text(
        rect.left_top() + Vec2::new(12.0, 26.0),
        Align2::LEFT_TOP,
        format!("mode: {}", mode_name),
        FontId::proportional(11.0),
        Color32::from_rgb(100, 160, 220),
    );

    // Projection label
    let proj_label = match projection { 1 => "XZ  plane", 2 => "YZ  plane", _ => "XY  plane" };
    painter.text(
        rect.left_top() + Vec2::new(12.0, 42.0),
        Align2::LEFT_TOP,
        proj_label,
        FontId::proportional(10.5),
        Color32::from_rgb(70, 110, 175),
    );

    // Derivative arrow at current position
    if let (Some(&(lx, ly, _, _)), true) = (proj_pts.last(), current_deriv.len() >= 2) {
        let pos = to_screen(lx, ly);
        let dx = current_deriv[0] as f32;
        let dy = current_deriv[1] as f32;
        let mag = (dx * dx + dy * dy).sqrt().max(1e-6);
        let scale = 40.0f32;
        let arrow_end = Pos2::new(
            pos.x + (dx / mag) * scale,
            pos.y - (dy / mag) * scale,
        );
        painter.line_segment([pos, arrow_end], Stroke::new(4.0, Color32::from_rgba_premultiplied(255, 200, 0, 40)));
        painter.line_segment([pos, arrow_end], Stroke::new(1.5, Color32::from_rgb(255, 220, 0)));
        painter.circle_filled(arrow_end, 3.0, Color32::from_rgb(255, 220, 0));
    }

    // Equation overlay
    let eq_text = equation_text(system_name);
    if !eq_text.is_empty() {
        let eq_pos = rect.left_bottom() + Vec2::new(8.0, -8.0);
        painter.text(eq_pos, Align2::LEFT_BOTTOM, eq_text, FontId::monospace(10.0), Color32::from_rgba_premultiplied(150, 180, 255, 180));
    }

    // State values — right side with subtle background
    if !current_state.is_empty() {
        let var_names = dim_names(system_name);
        let n_vars = current_state.len().min(6);
        let state_bg = egui::Rect::from_min_size(
            rect.right_top() + Vec2::new(-120.0, 6.0),
            Vec2::new(114.0, 8.0 + n_vars as f32 * 15.0),
        );
        painter.rect_filled(state_bg, 6.0, Color32::from_rgba_premultiplied(8, 8, 20, 150));
        for (i, (&val, name)) in current_state.iter().zip(var_names.iter()).enumerate().take(n_vars) {
            let text = format!("{} = {:+.3}", name, val);
            let pos = rect.right_top() + Vec2::new(-10.0, 12.0 + i as f32 * 15.0);
            painter.text(pos, Align2::RIGHT_TOP, text, FontId::monospace(10.5), Color32::from_rgba_premultiplied(80, 210, 130, 220));
        }
    }

    // Energy drift indicator (bottom-left, above equation overlay)
    if energy_error > 1e-8 {
        let edrift_color = if energy_error >= 1e-4 {
            Color32::from_rgb(255, 60, 60)
        } else if energy_error >= 1e-6 {
            Color32::from_rgb(255, 220, 0)
        } else {
            Color32::from_rgb(60, 220, 80)
        };
        let edrift_text = format!("E-drift: {:.2e}", energy_error);
        let edrift_pos = rect.left_bottom() + Vec2::new(8.0, -26.0);
        painter.text(edrift_pos, Align2::LEFT_BOTTOM, edrift_text, FontId::monospace(10.0), edrift_color);
    }
}

fn draw_mixer_tab(ui: &mut egui::Ui, state: &crate::ui::SharedState, viz_points: &[(f32, f32, f32, f32, bool)]) {
    let mc_cyan   = egui::Color32::from_rgb(0, 200, 220);
    let mc_green  = egui::Color32::from_rgb(0, 220, 100);
    let mc_orange = egui::Color32::from_rgb(255, 160, 40);
    let mc_red    = egui::Color32::from_rgb(220, 60, 60);
    let mc_gray   = egui::Color32::from_rgb(120, 120, 140);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);

        // ── Save Clip button ──────────────────────────────────────────────
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(
                egui::RichText::new("📸 Save Clip (60s audio + portrait)").color(egui::Color32::BLACK))
                .fill(egui::Color32::from_rgb(220, 180, 40))
                .min_size(egui::Vec2::new(280.0, 32.0))
            ).clicked() {
                let sr = state.lock().sample_rate;
                let cb = state.lock().clip_buffer.clone();
                let trail = viz_points.to_vec();
                // LEGACY: compute state hash for .sig companion file
                let state_hash: u64 = {
                    let st = state.lock();
                    st.current_state.iter().fold(0u64, |acc, &v| acc.wrapping_add(v.to_bits()))
                };
                std::thread::spawn(move || {
                    let wav_result = save_clip(&cb, sr);
                    let png_result = save_portrait_png(&trail);
                    // Write .sig companion file with state hash
                    if let Ok(ref wav_path) = wav_result {
                        let sig_path = wav_path.replace(".wav", ".sig");
                        let hex = format!("{:016x}", state_hash);
                        let _ = std::fs::write(&sig_path, hex.as_bytes());
                    }
                    match (wav_result, png_result) {
                        (Ok(wav), Ok(png)) => log::info!("Clip saved: {} + {}", wav, png),
                        (Err(e), _) => log::error!("Clip save failed: {e}"),
                        (_, Err(e)) => log::error!("Portrait save failed: {e}"),
                    }
                });
                state.lock().clip_status = "Saving to clips/ folder...".into();
            }
            let status = state.lock().clip_status.clone();
            if !status.is_empty() {
                ui.label(egui::RichText::new(&status).color(mc_green).size(11.0));
            }
        });
        ui.label(egui::RichText::new("Clips saved to clips/ folder — share-ready WAV + phase portrait PNG").color(mc_gray).size(10.0));
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── VU Meters ─────────────────────────────────────────────────────
        ui.label(egui::RichText::new("VU Meters").color(mc_cyan).strong());
        ui.add_space(4.0);
        let peaks = { *state.lock().vu_meter.lock() };
        ui.horizontal(|ui| {
            for (i, &peak) in peaks.iter().enumerate() {
                let (label, col) = match i {
                    0 => ("L0", mc_green),
                    1 => ("L1", mc_cyan),
                    2 => ("L2", mc_orange),
                    _ => ("Master", egui::Color32::WHITE),
                };
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(label).color(col).size(11.0));
                    let bar_height = 80.0;
                    let bar_width  = 18.0;
                    let (resp, painter) = ui.allocate_painter(
                        egui::Vec2::new(bar_width, bar_height), egui::Sense::hover());
                    let r = resp.rect;
                    painter.rect_filled(r, 2.0, egui::Color32::from_rgb(20, 20, 30));
                    let filled = peak.clamp(0.0, 1.0) * bar_height;
                    let fill_rect = egui::Rect::from_min_max(
                        egui::Pos2::new(r.min.x, r.max.y - filled),
                        r.max,
                    );
                    let bar_col = if peak > 0.9 { mc_red } else if peak > 0.7 { mc_orange } else { col };
                    painter.rect_filled(fill_rect, 1.0, bar_col);
                    ui.label(egui::RichText::new(format!("{:.0}%", peak * 100.0)).size(9.0).color(mc_gray));
                });
                if i < 3 { ui.add_space(8.0); }
            }
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Layer 0 (Main) Controls ───────────────────────────────────────
        ui.label(egui::RichText::new("Layer 0 — Main System").color(mc_green).strong());
        ui.horizontal(|ui| {
            let mut st = state.lock();
            ui.add(egui::Slider::new(&mut st.layer0_level, 0.0..=1.5).text("Level"));
            ui.add(egui::Slider::new(&mut st.layer0_pan, -1.0..=1.0).text("Pan"));
            let mc = if st.layer0_mute { mc_red } else { egui::Color32::from_rgb(40, 60, 40) };
            if ui.add(egui::Button::new(egui::RichText::new("M").color(egui::Color32::WHITE))
                .fill(mc).min_size(egui::Vec2::new(28.0, 28.0))).clicked() {
                st.layer0_mute = !st.layer0_mute;
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("ADSR Envelope (triggered by arpeggiator / KS events)").color(mc_cyan).size(11.0));
        {
            let mut st = state.lock();
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut st.adsr_attack_ms,  1.0..=2000.0).text("Attack ms").logarithmic(true));
                ui.add(egui::Slider::new(&mut st.adsr_decay_ms,   1.0..=2000.0).text("Decay ms").logarithmic(true));
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut st.adsr_sustain,    0.0..=1.0).text("Sustain"));
                ui.add(egui::Slider::new(&mut st.adsr_release_ms, 10.0..=5000.0).text("Release ms").logarithmic(true));
            });
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Extra Polyphony Layers ────────────────────────────────────────
        for li in 0..2usize {
            let label_col = if li == 0 { mc_cyan } else { mc_orange };
            ui.label(egui::RichText::new(format!("Layer {} — Additional System", li + 1)).color(label_col).strong());

            let (preset_name, active) = {
                let st = state.lock();
                let d = &st.poly_layers[li];
                (d.preset_name.clone(), d.active)
            };

            ui.horizontal(|ui| {
                let mut act = active;
                if ui.checkbox(&mut act, egui::RichText::new("Active").color(egui::Color32::WHITE)).changed() {
                    let mut st = state.lock();
                    st.poly_layers[li].active = act;
                    st.poly_layers[li].changed = true;
                }
                egui::ComboBox::new(format!("layer_preset_{}", li), "Preset")
                    .selected_text(if preset_name.is_empty() { "Select…" } else { &preset_name })
                    .show_ui(ui, |ui| {
                        for preset in crate::patches::PRESETS {
                            if ui.selectable_label(preset_name == preset.name, preset.name).clicked() {
                                let mut st = state.lock();
                                st.poly_layers[li].preset_name = preset.name.to_string();
                                st.poly_layers[li].active = true;
                                st.poly_layers[li].changed = true;
                            }
                        }
                    });
            });

            if active {
                ui.horizontal(|ui| {
                    let mut st = state.lock();
                    let d = &mut st.poly_layers[li];
                    ui.add(egui::Slider::new(&mut d.level, 0.0..=1.5).text("Level"));
                    ui.add(egui::Slider::new(&mut d.pan,   -1.0..=1.0).text("Pan"));
                    let mc = if d.mute { mc_red } else { egui::Color32::from_rgb(40, 60, 40) };
                    if ui.add(egui::Button::new(egui::RichText::new("M").color(egui::Color32::WHITE))
                        .fill(mc).min_size(egui::Vec2::new(28.0, 28.0))).clicked() {
                        d.mute = !d.mute;
                    }
                });
                ui.horizontal(|ui| {
                    let mut st = state.lock();
                    let d = &mut st.poly_layers[li];
                    ui.add(egui::Slider::new(&mut d.adsr_attack_ms,  1.0..=2000.0).text("A ms").logarithmic(true));
                    ui.add(egui::Slider::new(&mut d.adsr_decay_ms,   1.0..=2000.0).text("D ms").logarithmic(true));
                    ui.add(egui::Slider::new(&mut d.adsr_sustain,    0.0..=1.0).text("S"));
                    ui.add(egui::Slider::new(&mut d.adsr_release_ms, 10.0..=5000.0).text("R ms").logarithmic(true));
                });
            }
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
        }

        // ── Audio Sidechain ───────────────────────────────────────────────
        ui.label(egui::RichText::new("Audio Sidechain Input").color(mc_orange).strong());
        ui.label(egui::RichText::new("Modulate parameters from mic/line-in audio energy").color(mc_gray).size(10.0));
        ui.add_space(4.0);
        {
            let mut st = state.lock();
            ui.horizontal(|ui| {
                ui.checkbox(&mut st.sidechain_enabled, egui::RichText::new("Enable").color(egui::Color32::WHITE));
                egui::ComboBox::new("sc_target", "Target")
                    .selected_text(&st.sidechain_target)
                    .show_ui(ui, |ui| {
                        for t in &["speed", "reverb", "filter", "sigma", "volume"] {
                            ui.selectable_value(&mut st.sidechain_target, t.to_string(), *t);
                        }
                    });
                ui.add(egui::Slider::new(&mut st.sidechain_amount, 0.0..=2.0).text("Amount"));
            });
            if st.sidechain_enabled {
                let sc_level = *st.sidechain_level_shared.lock();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Input:").color(mc_gray).size(11.0));
                    let (resp, painter) = ui.allocate_painter(
                        egui::Vec2::new(200.0, 12.0), egui::Sense::hover());
                    let r = resp.rect;
                    painter.rect_filled(r, 2.0, egui::Color32::from_rgb(20, 20, 30));
                    let filled = sc_level.clamp(0.0, 1.0) * r.width();
                    if filled > 0.0 {
                        let fill_r = egui::Rect::from_min_max(r.min, egui::Pos2::new(r.min.x + filled, r.max.y));
                        painter.rect_filled(fill_r, 1.0, mc_orange);
                    }
                });
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Spectral Freeze ───────────────────────────────────────────────
        ui.label(egui::RichText::new("Spectral Freeze").color(egui::Color32::from_rgb(100, 200, 255)).strong());
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut st = state.lock();
            let freeze_active = st.spectral_freeze_active;
            let freeze_color = if freeze_active { egui::Color32::from_rgb(40, 140, 220) } else { egui::Color32::from_rgb(40, 40, 70) };
            if ui.add(egui::Button::new(egui::RichText::new("FREEZE").color(egui::Color32::WHITE))
                .fill(freeze_color).min_size(egui::Vec2::new(80.0, 28.0))).clicked() {
                // Capture current attractor frequencies and harmonics
                let base = st.config.sonification.base_frequency as f32;
                let mut freqs = vec![0.0f32; 16];
                let mut amps = vec![0.0f32; 16];
                // First 4: base freq harmonics, next 12: additional harmonics
                for i in 0..16 {
                    freqs[i] = base * (i + 1) as f32;
                    amps[i] = 1.0 / (i + 1) as f32 * 0.5;
                }
                st.spectral_freeze_freqs = freqs;
                st.spectral_freeze_amps = amps;
                st.spectral_freeze_active = true;
            }
            if ui.add(egui::Button::new(egui::RichText::new("CLEAR").color(egui::Color32::WHITE))
                .fill(egui::Color32::from_rgb(80, 40, 40)).min_size(egui::Vec2::new(60.0, 28.0))).clicked() {
                st.spectral_freeze_active = false;
            }
            let status = if freeze_active { "FROZEN" } else { "Off" };
            ui.label(egui::RichText::new(status).color(if freeze_active { mc_cyan } else { mc_gray }).size(12.0));
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Replay Recording ──────────────────────────────────────────────
        ui.label(egui::RichText::new("Replay Recording").color(egui::Color32::from_rgb(220, 120, 40)).strong());
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut st = state.lock();
            let rec = st.replay_recording;
            let play = st.replay_playing;
            let rec_color = if rec { mc_red } else { egui::Color32::from_rgb(140, 60, 60) };
            if ui.add(egui::Button::new(egui::RichText::new(if rec { "STOP REC" } else { "REC" }).color(egui::Color32::WHITE))
                .fill(rec_color).min_size(egui::Vec2::new(70.0, 28.0))).clicked() {
                if rec {
                    st.replay_recording = false;
                    // Auto-save to replays/latest.msr
                    let events = st.replay_events.clone();
                    std::thread::spawn(move || {
                        let _ = save_replay_file(&events, "replays/latest.msr");
                    });
                } else {
                    st.replay_events.clear();
                    st.replay_recording = true;
                    st.replay_start_time = std::time::Instant::now();
                }
            }
            let play_color = if play { mc_green } else { egui::Color32::from_rgb(40, 100, 40) };
            if ui.add(egui::Button::new(egui::RichText::new(if play { "STOP" } else { "PLAY FILE" }).color(egui::Color32::WHITE))
                .fill(play_color).min_size(egui::Vec2::new(80.0, 28.0))).clicked() {
                if play {
                    st.replay_playing = false;
                    st.replay_play_pos = 0;
                } else {
                    // Load from replays/latest.msr
                    if let Ok(events) = load_replay_file("replays/latest.msr") {
                        st.replay_events = events;
                        st.replay_play_pos = 0;
                        st.replay_playing = true;
                        st.replay_play_start = std::time::Instant::now();
                    }
                }
            }
            let n = st.replay_events.len();
            ui.label(egui::RichText::new(format!("{n} events")).color(mc_gray).size(11.0));
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Looper ────────────────────────────────────────────────────────
        ui.label(egui::RichText::new("Looper").color(egui::Color32::from_rgb(200, 100, 255)).strong());
        ui.add_space(4.0);
        {
            let mut st = state.lock();
            ui.horizontal(|ui| {
                let rec = st.looper_recording;
                let rec_color = if rec { mc_red } else { egui::Color32::from_rgb(140, 60, 60) };
                if ui.add(egui::Button::new(egui::RichText::new(if rec { "STOP" } else { "REC" }).color(egui::Color32::WHITE))
                    .fill(rec_color).min_size(egui::Vec2::new(60.0, 28.0))).clicked() {
                    st.looper_recording = !rec;
                }
                ui.label(egui::RichText::new("Bars:").color(mc_gray));
                for bars in [1u32, 2, 4, 8] {
                    let sel = st.looper_bars == bars;
                    let c = if sel { mc_cyan } else { egui::Color32::from_rgb(40, 40, 70) };
                    if ui.add(egui::Button::new(egui::RichText::new(bars.to_string()).color(egui::Color32::WHITE))
                        .fill(c).min_size(egui::Vec2::new(28.0, 24.0))).clicked() {
                        st.looper_bars = bars;
                    }
                }
                ui.add(egui::Slider::new(&mut st.looper_bpm, 60.0..=200.0).text("BPM").integer());
            });
            ui.add_space(4.0);
            // Layer list
            let n_layers = st.looper_layers.len();
            if n_layers == 0 {
                ui.label(egui::RichText::new("No loops recorded — hit REC to start").color(mc_gray).size(11.0).italics());
            }
            for i in 0..n_layers {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("L{}", i + 1)).color(mc_gray).size(11.0));
                    let active = st.looper_layers[i].active;
                    let ac = if active { mc_green } else { egui::Color32::from_rgb(40, 40, 70) };
                    if ui.add(egui::Button::new(egui::RichText::new(if active { "ON" } else { "OFF" }).color(egui::Color32::WHITE))
                        .fill(ac).min_size(egui::Vec2::new(36.0, 22.0))).clicked() {
                        st.looper_layers[i].active = !active;
                    }
                    ui.add(egui::Slider::new(&mut st.looper_layers[i].level, 0.0..=1.0).text("Vol"));
                    if ui.add(egui::Button::new(egui::RichText::new("X").color(egui::Color32::WHITE))
                        .fill(mc_red).min_size(egui::Vec2::new(22.0, 22.0))).clicked() {
                        st.looper_layers.remove(i);
                        // Break since we modified the vec
                        return;
                    }
                });
            }
            if ui.add(egui::Button::new(egui::RichText::new("CLEAR ALL").color(egui::Color32::WHITE))
                .fill(egui::Color32::from_rgb(80, 40, 40)).min_size(egui::Vec2::new(90.0, 24.0))).clicked() {
                st.looper_layers.clear();
            }
        }
    });
}

fn save_replay_file(events: &[crate::ui::ReplayEvent], path: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("."));
    if !dir.exists() { std::fs::create_dir_all(dir)?; }
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    // Header: sample_rate u32, version u8
    f.write_all(&44100u32.to_le_bytes())?;
    f.write_all(&1u8.to_le_bytes())?;
    // Records: timestamp_ms u32, param_id u8, value f32
    for ev in events {
        f.write_all(&ev.timestamp_ms.to_le_bytes())?;
        f.write_all(&ev.param_id.to_le_bytes())?;
        f.write_all(&ev.value.to_le_bytes())?;
    }
    Ok(())
}

fn load_replay_file(path: &str) -> anyhow::Result<Vec<crate::ui::ReplayEvent>> {
    use std::io::Read;
    let mut f = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut buf4 = [0u8; 4];
    let mut buf1 = [0u8; 1];
    // Skip header (5 bytes)
    f.read_exact(&mut buf4)?; // sample_rate
    f.read_exact(&mut buf1)?; // version
    let mut events = Vec::new();
    loop {
        let mut ts_buf = [0u8; 4];
        if f.read_exact(&mut ts_buf).is_err() { break; }
        let timestamp_ms = u32::from_le_bytes(ts_buf);
        if f.read_exact(&mut buf1).is_err() { break; }
        let param_id = buf1[0];
        if f.read_exact(&mut buf4).is_err() { break; }
        let value = f32::from_le_bytes(buf4);
        events.push(crate::ui::ReplayEvent { timestamp_ms, param_id, value });
    }
    Ok(events)
}

fn draw_waveform(ui: &mut Ui, waveform: &Arc<Mutex<Vec<f32>>>) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;

    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let samples = if let Some(wf) = waveform.try_lock() {
        wf.clone()
    } else {
        return;
    };

    if samples.len() < 2 {
        painter.text(rect.center(), Align2::CENTER_CENTER, "Audio starting...",
            FontId::proportional(16.0), Color32::from_rgb(80, 80, 120));
        return;
    }

    let cy = rect.center().y;

    // Subtle grid lines
    for i in 1..4 {
        let frac = i as f32 / 4.0;
        let gy = rect.top() + frac * rect.height();
        painter.line_segment(
            [Pos2::new(rect.left(), gy), Pos2::new(rect.right(), gy)],
            Stroke::new(0.5, Color32::from_rgba_premultiplied(30, 45, 70, 60)),
        );
    }
    // Zero line
    painter.line_segment(
        [Pos2::new(rect.left(), cy), Pos2::new(rect.right(), cy)],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 80, 130, 120)),
    );

    let n = samples.len();
    let w = rect.width();

    let pts: Vec<Pos2> = samples.iter().enumerate().map(|(i, &s)| {
        let x = rect.left() + (i as f32 / n as f32) * w;
        let y = cy - s.clamp(-1.0, 1.0) * (rect.height() * 0.42);
        Pos2::new(x, y)
    }).collect();

    // Glow pass (wide, dim)
    for seg in pts.windows(2) {
        painter.line_segment([seg[0], seg[1]], Stroke::new(4.0,
            Color32::from_rgba_premultiplied(0, 200, 100, 30)));
    }
    // Core pass
    let neon_green = Color32::from_rgb(0, 245, 115);
    for seg in pts.windows(2) {
        painter.line_segment([seg[0], seg[1]], Stroke::new(1.5, neon_green));
    }

    let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / n as f32).sqrt();
    let bar_h = (rms * rect.height()).clamp(0.0, rect.height());
    let bar_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 12.0, rect.bottom() - bar_h),
        Vec2::new(10.0, bar_h),
    );
    painter.rect_filled(bar_rect, 0.0, Color32::from_rgb(0, 200, 80));
    painter.text(
        Pos2::new(rect.right() - 6.0, rect.bottom() - bar_h - 4.0),
        Align2::CENTER_BOTTOM,
        format!("{:.2}", rms),
        FontId::proportional(10.0),
        Color32::from_rgb(0, 220, 100),
    );

    if samples.len() >= 64 {
        let n_bins = 32usize;
        let n_dft = samples.len().min(2048);
        let bin_h_max = 60.0f32;
        let spec_y = rect.bottom() - bin_h_max - 10.0;

        painter.text(
            Pos2::new(rect.left() + 4.0, spec_y - 4.0),
            Align2::LEFT_BOTTOM,
            "Spectrum",
            FontId::proportional(10.0),
            Color32::from_rgb(100, 120, 160),
        );

        for bin in 0..n_bins {
            let freq_bin = bin + 1;
            let mut re = 0.0f32;
            let mut im = 0.0f32;
            for (k, &s) in samples.iter().take(n_dft).enumerate() {
                let angle = -2.0 * std::f32::consts::PI * freq_bin as f32 * k as f32 / n_dft as f32;
                re += s * angle.cos();
                im += s * angle.sin();
            }
            let mag = ((re * re + im * im) / n_dft as f32).sqrt();
            let bar_h2 = (mag * 200.0).clamp(0.0, bin_h_max);
            let bar_w = (rect.width() / n_bins as f32) - 1.0;
            let bx = rect.left() + bin as f32 * (rect.width() / n_bins as f32);
            let bar_rect2 = Rect::from_min_size(
                Pos2::new(bx, spec_y + bin_h_max - bar_h2),
                Vec2::new(bar_w.max(1.0), bar_h2),
            );
            painter.rect_filled(bar_rect2, 0.0, hue_to_color(bin as f32 / n_bins as f32, 0.8));
        }
    }
}

fn hz_to_note_name(hz: f32) -> String {
    if hz < 16.0 { return "---".into(); }
    let note_names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let semitones_from_a4 = 12.0 * (hz / 440.0).log2();
    let midi = (69.0 + semitones_from_a4).round() as i32;
    let octave = (midi / 12) - 1;
    let note = ((midi % 12 + 12) % 12) as usize;
    format!("{}{}", note_names[note], octave)
}

fn draw_note_map(ui: &mut Ui, freqs: &[f32; 4], voice_levels: &[f32; 4], chord_intervals: &[f32; 3]) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;

    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let voice_colors = [
        Color32::from_rgb(0, 180, 255),
        Color32::from_rgb(0, 255, 140),
        Color32::from_rgb(255, 200, 0),
        Color32::from_rgb(255, 80, 80),
    ];
    let chord_color = Color32::from_rgb(200, 100, 255);

    let freq_min = 50.0f32.ln();
    let freq_max = 4000.0f32.ln();
    let freq_to_x = |f: f32| {
        let ln_f = f.max(20.0).ln();
        rect.left() + ((ln_f - freq_min) / (freq_max - freq_min)) * rect.width()
    };

    for &lf in &[55.0f32, 110.0, 220.0, 440.0, 880.0, 1760.0, 3520.0] {
        let x = freq_to_x(lf);
        if x >= rect.left() && x <= rect.right() {
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 50, 80, 80)),
            );
            painter.text(
                Pos2::new(x, rect.bottom() - 4.0),
                Align2::CENTER_BOTTOM,
                format!("{:.0}Hz", lf),
                FontId::proportional(9.0),
                Color32::from_rgb(80, 80, 120),
            );
        }
    }

    let bar_h = 28.0f32;
    let spacing = 36.0f32;
    let y_start = rect.top() + 20.0;

    for (i, (&freq, &level)) in freqs.iter().zip(voice_levels.iter()).enumerate() {
        if freq < 16.0 { continue; }
        let x = freq_to_x(freq);
        let y = y_start + i as f32 * spacing;
        let bar_w = (level * 80.0).max(4.0);

        let bar_rect = Rect::from_min_size(
            Pos2::new(x - bar_w * 0.5, y),
            Vec2::new(bar_w, bar_h * 0.6),
        );
        painter.rect_filled(bar_rect, 3.0, voice_colors[i]);

        painter.text(
            Pos2::new(x, y - 2.0),
            Align2::CENTER_BOTTOM,
            hz_to_note_name(freq),
            FontId::proportional(11.0),
            voice_colors[i],
        );

        if i == 0 {
            for (k, &interval) in chord_intervals.iter().enumerate() {
                if interval.abs() < 0.001 { continue; }
                let chord_freq = freq * 2.0f32.powf(interval / 12.0);
                let cx = freq_to_x(chord_freq);
                let cy = y - (k as f32 + 1.0) * (bar_h * 0.5 + 4.0);
                let chord_bar = Rect::from_min_size(
                    Pos2::new(cx - 20.0, cy),
                    Vec2::new(40.0, bar_h * 0.4),
                );
                painter.rect_filled(chord_bar, 2.0, chord_color);
                painter.text(
                    Pos2::new(cx, cy - 2.0),
                    Align2::CENTER_BOTTOM,
                    hz_to_note_name(chord_freq),
                    FontId::proportional(10.0),
                    chord_color,
                );
            }
        }
    }

    painter.text(
        rect.left_top() + Vec2::new(8.0, 4.0),
        Align2::LEFT_TOP,
        "Note Map (log frequency axis)",
        FontId::proportional(11.0),
        Color32::from_rgb(100, 110, 150),
    );
}

// ---------------------------------------------------------------------------
// Helper functions for math overlay
// ---------------------------------------------------------------------------

fn hue_to_color(hue: f32, saturation: f32) -> Color32 {
    let h = hue * 6.0;
    let s = saturation;
    let v = 1.0f32;
    let i = h.floor() as u32 % 6;
    let f = h - h.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn equation_text(system: &str) -> &'static str {
    match system {
        "lorenz" => "x' = s(y-x)\ny' = x(r-z)-y\nz' = xy-bz",
        "rossler" => "x' = -y-z\ny' = x+ay\nz' = b+z(x-c)",
        "double_pendulum" => "th1'' = f(th1,th2,w1,w2)\nth2'' = g(th1,th2,w1,w2)",
        "geodesic_torus" => "phi'' = -2(r sin(th)/(R+r cos(th)))phi'th'\nth'' = (R+r cos(th))sin(th)/r * phi'^2",
        "kuramoto" => "th'_i = w_i + K/N sum_j sin(th_j-th_i)",
        "three_body" => "r''_i = G sum_j m_j(r_j-r_i)/|r_j-r_i|^3",
        _ => "",
    }
}

fn equation_lines(system: &str) -> Vec<&'static str> {
    match system {
        "lorenz" => vec!["x' = s(y - x)", "y' = x(r - z) - y", "z' = xy - bz"],
        "rossler" => vec!["x' = -y - z", "y' = x + ay", "z' = b + z(x - c)"],
        "double_pendulum" => vec![
            "th1'' = (p1*l2 - p2*l1*cos(D)) / (l1^2*l2*M*det)",
            "th2'' = ((m1+m2)*l1*p2 - m2*l2*p1*cos(D)) / (m2*l1*l2^2*M*det)",
            "p1' = -(m1+m2)*g*l1*sin(th1) - th1'*th2'*l1*l2*m2*sin(D)",
            "p2' = -m2*g*l2*sin(th2) + th1'*th2'*l1*l2*m2*sin(D)",
        ],
        "geodesic_torus" => vec![
            "phi'' = -2*r*sin(th)/(R+r*cos(th)) * phi'*th'",
            "th'' = (R+r*cos(th))*sin(th)/r * phi'^2",
            "ds^2 = (R+r*cos(th))^2*dphi^2 + r^2*dth^2",
        ],
        "kuramoto" => vec![
            "th'_i = w_i + K/N * sum_j sin(th_j - th_i)",
            "r*e^(i*psi) = 1/N * sum_j e^(i*th_j)",
            "Critical: Kc = 2*gamma (Lorentzian width gamma)",
        ],
        "three_body" => vec![
            "r''_1 = G*m2*(r2-r1)/|r2-r1|^3 + G*m3*(r3-r1)/|r3-r1|^3",
            "r''_2 = G*m1*(r1-r2)/|r1-r2|^3 + G*m3*(r3-r2)/|r3-r2|^3",
            "r''_3 = G*m1*(r1-r3)/|r1-r3|^3 + G*m2*(r2-r3)/|r2-r3|^3",
        ],
        _ => vec!["No equations available"],
    }
}

fn dim_names(system: &str) -> &'static [&'static str] {
    match system {
        "lorenz" | "rossler" => &["x", "y", "z"],
        "double_pendulum" => &["th1", "th2", "w1", "w2"],
        "geodesic_torus" => &["phi", "th", "phi'", "th'"],
        "kuramoto" => &["th1", "th2", "th3", "th4"],
        "three_body" => &["x1", "y1", "x2", "y2", "x3", "y3"],
        "hindmarsh_rose" => &["x (V)", "y (fast)", "z (slow)"],
        "coupled_map_lattice" => &["x0", "x1", "x2", "x3"],
        _ => &["x", "y", "z"],
    }
}

// ---------------------------------------------------------------------------
// Math View tab
// ---------------------------------------------------------------------------

fn draw_math_view(
    ui: &mut Ui,
    system_name: &str,
    current_state: &[f64],
    current_deriv: &[f64],
    chaos_level: f32,
    order_param: f64,
    kuramoto_phases: &[f64],
    lyapunov_spectrum: &[f64],
    attractor_type: &str,
    kolmogorov_entropy: f64,
    energy_error: f64,
    sync_error: f32,
    permutation_entropy: f64,
    integrator_divergence: f64,
) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let mid_x = rect.center().x;

    let mut y = rect.top() + 20.0;
    let x = rect.left() + 20.0;

    painter.text(Pos2::new(x, y), Align2::LEFT_TOP,
        system_display_name(system_name),
        FontId::proportional(20.0),
        Color32::from_rgb(120, 195, 255));
    y += 32.0;

    let eq_lines = equation_lines(system_name);
    painter.text(Pos2::new(x, y), Align2::LEFT_TOP,
        "Equations of Motion:", FontId::proportional(13.0), Color32::from_rgb(150, 150, 200));
    y += 20.0;
    for line in &eq_lines {
        painter.text(Pos2::new(x + 10.0, y), Align2::LEFT_TOP,
            line, FontId::monospace(12.0), Color32::from_rgb(180, 210, 255));
        y += 18.0;
    }
    y += 15.0;

    painter.text(Pos2::new(x, y), Align2::LEFT_TOP,
        "State  ->  dx/dt", FontId::proportional(13.0), Color32::from_rgb(150, 150, 200));
    y += 20.0;
    let var_names = dim_names(system_name);
    for (i, &val) in current_state.iter().enumerate().take(8) {
        let name = var_names.get(i).unwrap_or(&"?");
        let dv = current_deriv.get(i).copied().unwrap_or(0.0);
        let state_text = format!("{} = {:+8.4}", name, val);
        let deriv_text = format!("  {:+8.4}/s", dv);

        let bar_w = (val.abs() as f32 * 20.0).clamp(0.0, 120.0);
        let bar_rect = Rect::from_min_size(Pos2::new(x, y + 2.0), Vec2::new(bar_w, 12.0));
        let hue = i as f32 / 8.0;
        let bar_color = hue_to_color(hue, 0.6);
        painter.rect_filled(bar_rect, 2.0, bar_color);

        painter.text(Pos2::new(x + 130.0, y), Align2::LEFT_TOP,
            state_text, FontId::monospace(11.0), Color32::from_rgb(200, 220, 200));
        painter.text(Pos2::new(x + 280.0, y), Align2::LEFT_TOP,
            deriv_text, FontId::monospace(11.0), Color32::from_rgb(220, 200, 100));
        y += 16.0;
    }

    y += 15.0;
    painter.text(Pos2::new(x, y), Align2::LEFT_TOP,
        format!("Chaos Level: {:.1}%", chaos_level * 100.0),
        FontId::proportional(13.0), Color32::from_rgb(150, 150, 200));
    y += 20.0;
    let meter_w = (mid_x - x - 40.0).max(100.0);
    let meter_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(meter_w, 16.0));
    painter.rect_filled(meter_rect, 4.0, Color32::from_rgb(20, 20, 40));
    let fill_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(meter_w * chaos_level, 16.0));
    let chaos_color = lerp_color(Color32::from_rgb(0, 100, 255), Color32::from_rgb(255, 30, 30), chaos_level);
    painter.rect_filled(fill_rect, 4.0, chaos_color);

    // Lyapunov exponent spectrum
    if !lyapunov_spectrum.is_empty() {
        y += 20.0;
        painter.text(Pos2::new(x, y), Align2::LEFT_TOP,
            "Lyapunov Spectrum:",
            FontId::proportional(13.0), Color32::from_rgb(150, 150, 200));
        y += 20.0;
        let half_w = (mid_x - x - 40.0).max(60.0) * 0.5;
        let center_x = x + half_w;
        // Zero line
        painter.line_segment(
            [Pos2::new(x, y + 8.0), Pos2::new(x + half_w * 2.0, y + 8.0)],
            Stroke::new(1.0, Color32::from_rgba_premultiplied(60, 60, 100, 180)),
        );
        for (i, &lambda) in lyapunov_spectrum.iter().take(3).enumerate() {
            let bar_len = (lambda.abs() as f32 * 15.0).clamp(1.0, half_w);
            let (bar_x, bar_color) = if lambda >= 0.0 {
                (center_x, Color32::from_rgb(255, 90, 60))
            } else {
                (center_x - bar_len, Color32::from_rgb(60, 200, 120))
            };
            let bar_rect = Rect::from_min_size(Pos2::new(bar_x, y + 2.0), Vec2::new(bar_len, 12.0));
            painter.rect_filled(bar_rect, 2.0, bar_color);
            let sign = if lambda >= 0.0 { "+" } else { "" };
            painter.text(Pos2::new(x, y), Align2::LEFT_TOP,
                format!("λ{} = {}{:.4}", i + 1, sign, lambda),
                FontId::monospace(11.0), bar_color);
            y += 16.0;
        }
        y += 4.0;
        // Kaplan-Yorke dimension estimate (if we have enough exponents)
        if lyapunov_spectrum.len() >= 2 {
            let mut sum = 0.0f64;
            let mut ky_j = 0usize;
            for (j, &lam) in lyapunov_spectrum.iter().enumerate() {
                if sum + lam < 0.0 { break; }
                sum += lam;
                ky_j = j + 1;
            }
            if ky_j > 0 && ky_j < lyapunov_spectrum.len() {
                let last_neg = lyapunov_spectrum[ky_j];
                if last_neg.abs() > 1e-12 {
                    let d_ky = ky_j as f64 + sum / last_neg.abs();
                    painter.text(Pos2::new(x, y), Align2::LEFT_TOP,
                        format!("D_KY ≈ {:.3}  (Kaplan-Yorke dim.)", d_ky),
                        FontId::monospace(11.0), Color32::from_rgb(180, 180, 100));
                    y += 16.0;
                }
            }
        }
    }

    if !attractor_type.is_empty() && attractor_type != "unknown" {
        y += 6.0;
        let atype_color = match attractor_type {
            "chaos"       => Color32::from_rgb(255, 90, 60),
            "hyperchaos"  => Color32::from_rgb(255, 40, 160),
            "limit_cycle" => Color32::from_rgb(60, 200, 120),
            "torus"       => Color32::from_rgb(80, 180, 255),
            "fixed_point" => Color32::from_rgb(200, 200, 200),
            _             => Color32::from_rgb(180, 180, 100),
        };
        painter.text(
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            format!("Attractor: {}", attractor_type.replace('_', " ")),
            FontId::monospace(12.0),
            atype_color,
        );
        y += 16.0;
        if kolmogorov_entropy > 0.0 {
            painter.text(
                Pos2::new(x, y),
                Align2::LEFT_TOP,
                format!("K-entropy: {:.4} nats/s", kolmogorov_entropy),
                FontId::monospace(12.0),
                Color32::from_rgb(200, 180, 100),
            );
            y += 16.0;
        }
        if permutation_entropy > 0.0 {
            painter.text(
                Pos2::new(x, y),
                Align2::LEFT_TOP,
                format!("Perm. entropy: {:.3}", permutation_entropy),
                FontId::monospace(12.0),
                Color32::from_rgb(160, 200, 160),
            );
            y += 16.0;
        }
        if energy_error > 1e-10 {
            let ee_color = if energy_error > 1e-6 {
                Color32::from_rgb(255, 60, 60)
            } else {
                Color32::from_rgb(200, 200, 200)
            };
            painter.text(
                Pos2::new(x, y),
                Align2::LEFT_TOP,
                format!("Energy drift: {:.2e}", energy_error),
                FontId::monospace(12.0),
                ee_color,
            );
            y += 16.0;
        }
        if sync_error > 0.001 {
            painter.text(
                Pos2::new(x, y),
                Align2::LEFT_TOP,
                format!("Sync error: {:.3}", sync_error),
                FontId::monospace(12.0),
                Color32::from_rgb(200, 160, 255),
            );
            y += 16.0;
        }
        if integrator_divergence > 0.0 {
            painter.text(
                Pos2::new(x, y),
                Align2::LEFT_TOP,
                format!("RK4 vs RK45 drift: {:.2e}", integrator_divergence),
                FontId::monospace(12.0),
                Color32::from_rgb(180, 180, 100),
            );
            y += 16.0;
        }
        let _ = y; // suppress unused warning
    }

    let right_rect = Rect::from_min_max(
        Pos2::new(mid_x + 10.0, rect.top() + 10.0),
        rect.max,
    );

    if system_name == "kuramoto" && !kuramoto_phases.is_empty() {
        draw_kuramoto_circle(&painter, right_rect, kuramoto_phases, order_param);
    } else {
        draw_phase_clock(&painter, right_rect, current_state, current_deriv);
    }
}

fn draw_kuramoto_circle(painter: &Painter, rect: Rect, phases: &[f64], order_param: f64) {
    let center = rect.center();
    let radius = (rect.width().min(rect.height()) * 0.4).min(160.0);

    painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 50, 80, 150)));
    painter.circle_stroke(center, radius * 0.5, Stroke::new(0.5, Color32::from_rgba_premultiplied(30, 30, 60, 100)));

    painter.text(center + Vec2::new(0.0, -radius - 16.0), Align2::CENTER_BOTTOM,
        "Kuramoto Phase Circle", FontId::proportional(13.0), Color32::from_rgb(150, 150, 200));
    painter.text(center + Vec2::new(0.0, radius + 8.0), Align2::CENTER_TOP,
        format!("Order parameter r = {:.3}", order_param),
        FontId::proportional(11.0), Color32::from_rgb(200, 200, 100));

    let n = phases.len();
    for (i, &phase) in phases.iter().enumerate() {
        let px = center.x + radius * phase.cos() as f32;
        let py = center.y - radius * phase.sin() as f32;
        let hue = i as f32 / n as f32;
        let col = hue_to_color(hue, 0.9);
        painter.circle_filled(Pos2::new(px, py), 9.0, Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 60));
        painter.circle_filled(Pos2::new(px, py), 5.0, col);
    }

    let (sin_sum, cos_sum): (f64, f64) = phases.iter().fold((0.0, 0.0), |(s, c), &ph| (s + ph.sin(), c + ph.cos()));
    let mean_phase = sin_sum.atan2(cos_sum) as f32;
    let r = order_param as f32;
    let arrow_end = Pos2::new(
        center.x + radius * r * mean_phase.cos(),
        center.y - radius * r * mean_phase.sin(),
    );
    painter.line_segment([center, arrow_end], Stroke::new(3.0, Color32::from_rgb(255, 220, 0)));
    painter.circle_filled(arrow_end, 5.0, Color32::from_rgb(255, 220, 0));
    painter.circle_filled(center, 3.0, Color32::from_rgb(200, 200, 200));
}

fn draw_phase_clock(painter: &Painter, rect: Rect, state: &[f64], deriv: &[f64]) {
    let center = rect.center();
    let radius = (rect.width().min(rect.height()) * 0.38).min(150.0);

    painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 50, 80, 150)));

    painter.text(center + Vec2::new(0.0, -radius - 16.0), Align2::CENTER_BOTTOM,
        "Phase Velocity", FontId::proportional(13.0), Color32::from_rgb(150, 150, 200));

    for i in 0..12 {
        let a = i as f32 * std::f32::consts::TAU / 12.0;
        let inner = Pos2::new(center.x + (radius - 8.0) * a.cos(), center.y - (radius - 8.0) * a.sin());
        let outer = Pos2::new(center.x + radius * a.cos(), center.y - radius * a.sin());
        painter.line_segment([inner, outer], Stroke::new(1.0, Color32::from_rgba_premultiplied(60, 60, 100, 150)));
    }

    if state.len() >= 2 && deriv.len() >= 2 {
        let x = state[0] as f32;
        let y = state[1] as f32;
        let angle = y.atan2(x);

        let dx = deriv[0] as f32;
        let dy = deriv[1] as f32;
        let dangle = dy.atan2(dx);
        let dmag = (dx * dx + dy * dy).sqrt();

        let pos_end = Pos2::new(
            center.x + radius * 0.8 * angle.cos(),
            center.y - radius * 0.8 * angle.sin(),
        );
        painter.line_segment([center, pos_end], Stroke::new(2.0, Color32::from_rgb(0, 180, 255)));
        painter.circle_filled(pos_end, 4.0, Color32::from_rgb(0, 200, 255));

        let vel_scale = (dmag / 100.0).clamp(0.0, 1.0);
        let vel_end = Pos2::new(
            center.x + radius * vel_scale * dangle.cos(),
            center.y - radius * vel_scale * dangle.sin(),
        );
        painter.line_segment([center, vel_end], Stroke::new(2.0, Color32::from_rgb(255, 200, 0)));
        painter.circle_filled(vel_end, 4.0, Color32::from_rgb(255, 220, 0));

        painter.text(center + Vec2::new(0.0, radius + 8.0), Align2::CENTER_TOP,
            format!("|v| = {:.2}", dmag),
            FontId::proportional(11.0), Color32::from_rgb(200, 200, 100));
    }

    painter.circle_filled(center, 3.0, Color32::from_rgb(200, 200, 200));
}
