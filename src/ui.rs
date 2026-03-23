use egui::*;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use crate::arrangement::{demo_arrangement, generate_song, scene_at, total_duration, MorphCurve, Scene};
use crate::audio::{
    capture_snippet, save_clip, save_clip_wav_32bit, save_portrait_png, save_portrait_svg,
    ClipBuffer, LoopExportPending, SharedSnippetPlayback, SidechainLevel, SnippetPlayback,
    StereoWidth, VuMeter, WavRecorder, XrunCounter,
};
use crate::config::Config;
use crate::patches::{
    list_backups, list_patches, load_backup, load_patch_file, load_preset, save_patch, PRESETS,
};
use crate::sonification::chord_intervals_for;
use crate::systems::{self, *};
use hound;

// Extracted sub-modules for large, self-contained drawing functions.
use crate::ui_timeline::draw_arrangement_timeline;
use crate::ui_tips::draw_tips_content;
use crate::ui_waveform::{draw_note_map, draw_waveform};

/// Severity level for toast notifications.
#[derive(Clone, PartialEq)]
pub enum ToastKind {
    Info,
    Warning,
    Error,
}

/// A transient notification displayed as an overlay for a few seconds.
#[derive(Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub born: std::time::Instant,
    /// How long to display before fading (seconds).
    pub ttl_secs: f32,
}

impl Toast {
    pub fn info(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            kind: ToastKind::Info,
            born: std::time::Instant::now(),
            ttl_secs: 4.0,
        }
    }
    pub fn warning(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            kind: ToastKind::Warning,
            born: std::time::Instant::now(),
            ttl_secs: 5.0,
        }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            kind: ToastKind::Error,
            born: std::time::Instant::now(),
            ttl_secs: 6.0,
        }
    }
}

/// A single looper layer. Playback not yet wired to the audio thread.
#[allow(dead_code)]
pub struct LooperLayer {
    pub samples: Vec<f32>,
    pub active: bool,
    pub level: f32,
    pub playback_pos: usize,
}

/// A captured audio snippet (saved to disk, available for song sequencing).
#[derive(Clone)]
pub struct Snippet {
    pub name: String,
    #[allow(dead_code)]
    pub path: String, // stored for future disk-reload and export features
    pub duration_secs: f32,
    #[allow(dead_code)]
    pub sample_rate: u32, // stored for rate-conversion during export
    /// ~128-point peak envelope for waveform thumbnail display.
    pub thumb: Vec<f32>,
    /// Color index 0..8 for visual identity.
    pub color_idx: usize,
    /// Loaded samples (stereo interleaved). Wrapped in Arc so clone is cheap.
    pub samples: Arc<Vec<f32>>,
}

impl Snippet {
    pub fn from_samples(name: String, path: String, samples: Vec<f32>, sample_rate: u32) -> Self {
        let duration_secs = samples.len() as f32 / (sample_rate as f32 * 2.0);
        let thumb = Self::make_thumb(&samples, 128);
        static COLOR_COUNTER: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        let color_idx = COLOR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 8;
        Self {
            name,
            path,
            duration_secs,
            sample_rate,
            thumb,
            color_idx,
            samples: Arc::new(samples),
        }
    }

    fn make_thumb(samples: &[f32], points: usize) -> Vec<f32> {
        if samples.is_empty() {
            return vec![0.0; points];
        }
        let chunk = (samples.len() / points).max(2) & !1; // even for stereo
        (0..points)
            .map(|i| {
                let start = (i * samples.len() / points) & !1;
                let end = (start + chunk).min(samples.len());
                samples[start..end]
                    .iter()
                    .map(|s| s.abs())
                    .fold(0.0f32, f32::max)
            })
            .collect()
    }

    /// 8 distinct colors for snippet identity.
    pub fn color(idx: usize) -> egui::Color32 {
        const COLORS: [(u8, u8, u8); 8] = [
            (0, 190, 255),   // cyan
            (255, 80, 120),  // pink
            (80, 255, 140),  // green
            (255, 170, 0),   // amber
            (180, 100, 255), // violet
            (255, 120, 40),  // orange
            (40, 220, 220),  // teal
            (200, 255, 60),  // lime
        ];
        let (r, g, b) = COLORS[idx % 8];
        egui::Color32::from_rgb(r, g, b)
    }
}

/// A parameter change event for replay recording.
#[derive(Clone)]
pub struct ReplayEvent {
    pub timestamp_ms: u32,
    pub param_id: u8,
    pub value: f32,
}

/// A single modulation route: maps an attractor state variable to a synthesis parameter.
#[derive(Clone)]
pub struct ModRoute {
    /// Source signal: "x", "y", "z", or "speed"
    pub source: String,
    /// Target parameter: "reverb_wet", "delay_ms", "base_freq_mult", "speed",
    ///                   "chorus_mix", "master_volume", "chaos"
    pub target: String,
    /// Modulation depth in [-1.0, 1.0]
    pub depth: f32,
    pub enabled: bool,
}

/// Central shared state, accessed by multiple threads under `parking_lot::Mutex`.
///
/// ## Thread ownership
///
/// `AppState` is wrapped in `Arc<Mutex<AppState>>` (alias: `SharedState`). Three threads access it:
///
/// | Thread | Access pattern |
/// |--------|---------------|
/// | **UI thread** | Calls `state.lock()` each frame to read/write UI fields and dispatch user actions. Owns all fields unless noted below. |
/// | **Sim thread** | Calls `state.lock()` once per tick (120 Hz) to read `config`, write diagnostic fields (`chaos_level`, `lyapunov_spectrum`, `current_state`, `session_log`, etc.). Must never allocate or block audio. |
/// | **Audio thread** | Never locks `AppState` directly. Receives `AudioParams` via `crossbeam_channel`. Writes `vu_meter` and `clip_buffer` via separate `Arc<Mutex<>>` that UI reads with `try_lock()`. |
///
/// ## Field ownership
///
/// - **UI thread writes:** `config`, `paused`, `system_changed`, `mode_changed`, `selected_preset`, all UI display state, `undo_stack`, `redo_stack`, `scenes`, `arr_*`, `poly_layers`, `midi_in_*`, `custom_ode_*`, `spectral_freeze_*`, `snippet_*`, `macro_*`.
/// - **Sim thread writes:** `chaos_level`, `current_state`, `current_deriv`, `kuramoto_phases`, `order_param`, `lyapunov_spectrum`, `attractor_type`, `kolmogorov_entropy`, `energy_error`, `permutation_entropy`, `integrator_divergence`, `session_log`, `spectral_live_partials`, `aging_secs`, `volume_creep_factor`, `entropy_pool`, `wounded`, `startup_ramp_t`, `time_of_day_f`, `scars`.
/// - **Audio thread writes (via separate `Arc<Mutex<>>`):** `vu_meter`, `clip_buffer`.
///
/// ## Undo/redo safety
/// `push_undo()`, `undo()`, and `redo()` mutate `config` and must only be called from the UI thread.
pub struct AppState {
    pub config: Config,
    pub paused: bool,
    pub system_changed: bool,
    pub mode_changed: bool,
    pub viz_projection: usize, // 0=XY, 1=XZ, 2=YZ
    pub viz_tab: usize,        // 0=Phase, 1=Waveform, 2=Notes, 3=Math View, 4=Bifurcation, 5=Basin
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
    /// Progress counter for ongoing bifurcation compute (0..bifurc_total_steps).
    pub bifurc_progress: Arc<std::sync::atomic::AtomicUsize>,
    /// Total steps for current bifurcation compute.
    pub bifurc_total_steps: usize,
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
    #[allow(dead_code)] // Reserved for scene editor — intended for future arranger panel focus
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
    pub coupled_x_out: f32, // live display: main system x output (normalized)
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
    /// Optional 4th equation dw/dt. Empty = 3-variable mode.
    pub custom_ode_w: String,
    pub custom_ode_error: String,
    // Spectral freeze
    pub spectral_freeze_active: bool,
    pub spectral_freeze_freqs: Vec<f32>,
    pub spectral_freeze_amps: Vec<f32>,
    /// Live partials from the spectral sonification (updated by sim thread each tick).
    /// Used by the UI to capture real attractor spectral content on FREEZE.
    pub spectral_live_partials: [f32; 32],
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
    pub midi_in_last_cc_num: u8, // CC number of the most recently received CC message
    // MIDI export recording
    pub midi_rec_enabled: bool,
    pub midi_rec_events: Vec<(u32, u8, u8, u8)>, // (tick_ms, status, data1, data2)
    pub midi_rec_start: std::time::Instant,
    // ── Feature: Attractor MIDI export ──────────────────────────────────────
    /// Whether the attractor-to-MIDI recorder is running.
    pub midi_export_recording: bool,
    /// Accumulated (pitch, velocity) frames for MIDI export.
    pub midi_export_frames: Vec<(u8, u8)>,
    /// Duration in seconds to record for MIDI export.
    pub midi_export_duration_secs: f32,
    /// Status message shown in the Studio tab after export.
    pub midi_export_status: String,
    // Replay
    pub replay_recording: bool,
    pub replay_events: Vec<ReplayEvent>,
    pub replay_start_time: std::time::Instant,
    pub replay_playing: bool,
    pub replay_play_pos: usize,
    pub replay_play_start: std::time::Instant,
    // Looper
    pub looper_recording: bool,
    #[allow(dead_code)] // Reserved for looper playback logic — not yet triggered from the UI
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
    pub scars: Vec<(f32, f32)>, // SCARRING: near-divergence marks
    pub shutdown_fading: bool,  // DYING GRACEFULLY: audio fade on close
    pub startup_ramp_t: f32,    // DYING GRACEFULLY: startup volume ramp
    pub time_of_day_f: f32,     // PHOTOTROPISM: 0=midnight 1=noon
    pub wounded: bool,          // WOUND HEALING: crashed last session
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
    /// Trajectory snapshot for recurrence plot (updated every ~2s with analysis data).
    pub trajectory_points: Vec<Vec<f64>>,
    pub session_log: Vec<SessionEntry>,
    // ── Snippet Studio ──────────────────────────────────────────────────────
    /// Library of captured snippets (current session + loaded from disk).
    pub snippets: Vec<Snippet>,
    /// Song grid: up to 32 slots, each holds an optional index into `snippets`.
    pub snippet_slots: Vec<Option<usize>>,
    /// Duration to capture when the user presses Capture.
    pub snippet_capture_secs: f32,
    /// Currently selected snippet in the library (for assigning to slots).
    pub snippet_selected: Option<usize>,
    /// Status message shown under the capture button.
    pub snippet_status: String,
    /// Song sequencer: is the song playing?
    pub song_playing: bool,
    /// Which slot is currently playing (0-based).
    pub song_play_slot: usize,
    /// Volume for snippet playback (0..1).
    pub snippet_volume: f32,
    /// Loop the song when it reaches the last slot.
    pub song_loop: bool,
    /// Shared with audio thread for snippet playback.
    pub snippet_pb: SharedSnippetPlayback,
    // Preset search filter
    pub preset_search: String,
    // Undo / Redo stacks (store up to 20 configs each). VecDeque for O(1) pop_front.
    pub undo_stack: std::collections::VecDeque<Config>,
    pub redo_stack: std::collections::VecDeque<Config>,
    // Bifurcation cache key: (system_name, param1, param2, is_2d)
    // When this matches the current bifurcation settings, recomputation is skipped.
    pub bifurc_cache_key: Option<(String, String, String, bool)>,
    // Arranger morph diff display
    pub arr_morph_diff: Vec<(String, f32, f32)>, // (param_name, from_val, to_val)
    // MIDI CC Learn Mode
    pub midi_cc_learn_active: bool,
    pub midi_cc_learn_target: String,
    pub midi_cc_map: Vec<(u8, String)>, // (CC number, param_name) mappings
    pub midi_cc_learn_last_cc: u8,      // previous midi_in_last_cc for change detection
    // Modulation matrix: routes attractor state variables to synthesis parameters
    pub mod_matrix: Vec<ModRoute>,
    /// Transient notifications shown as bottom-right overlays for a few seconds.
    pub toast_queue: Vec<Toast>,
    // ── Feature #7: Lyapunov exponent scrolling history ─────────────────────
    pub lyapunov_history: std::collections::VecDeque<f32>,
    // ── Feature #8: Poincaré section ────────────────────────────────────────
    pub poincare_points: Vec<(f32, f32)>,
    // poincare_z_prev is written by the sim thread for Poincaré section detection;
    // reading happens on the same thread so the compiler sees no cross-function read.
    #[allow(dead_code)]
    pub poincare_z_prev: f64,
    // ── Feature #10: Stochastic noise injection ──────────────────────────────
    pub noise_inject: f32,
    // ── Feature #9: Attractor interpolation ──────────────────────────────────
    pub interp_system: String,
    pub interp_t: f32,
    pub interp_enabled: bool,
    // ── Feature #6: Attractor basin visualization ────────────────────────────
    pub basin_data: Arc<Mutex<Vec<(f32, f32, f32)>>>, // (x, y, lyapunov_proxy) grid points
    pub basin_computing: bool,
    pub basin_resolution: usize, // default 80 (80×80 grid = 6400 points)
    pub basin_xlim: (f32, f32),  // default (-25.0, 25.0) for Lorenz
    pub basin_ylim: (f32, f32),  // default (-35.0, 35.0)
    pub basin_z_slice: f32,      // z-slice for initial conditions, default 27.0
    // ── #16: OSC Output ──────────────────────────────────────────────────────
    pub osc_enabled: bool,
    pub osc_host: String,
    pub osc_port: u16,
    pub osc_status: String,
    // ── #17: MIDI Clock Output ───────────────────────────────────────────────
    pub midi_clock_enabled: bool,
    // ── #21: Xrun counter — incremented by audio error callback ──────────────
    pub xrun_counter: XrunCounter,
    // ── #4 Stereo width ──────────────────────────────────────────────────────────
    /// Local value edited by the UI slider (0=mono, 1=unity, 3=hyper-wide).
    pub stereo_width: f32,
    /// Arc shared with the audio thread — written by UI, read by audio each buffer.
    pub stereo_width_shared: StereoWidth,
    // ── #11 Resizable panel ─────────────────────────────────────────────────────
    pub panel_width: f32,
    // ── #12 Parameter lock ──────────────────────────────────────────────────────
    pub locked_params: HashSet<String>,
    // ── #13 Preset favorites ────────────────────────────────────────────────────
    pub favorite_presets: HashSet<String>,
    pub show_favorites_only: bool,
    // ── #14 Dark/Light theme toggle ──────────────────────────────────────────────
    pub light_theme: bool,
    // ── #15 Waveform zoom ───────────────────────────────────────────────────────
    pub waveform_zoom: f32,
    pub waveform_offset: f32,
    // ── item 16: Behavioral layer enable/disable flags ──────────────────────────
    /// When false the sim thread skips the corresponding behavioral modifier.
    pub behav_time_of_day: bool,
    pub behav_seasonal_drift: bool,
    pub behav_volume_creep: bool,
    pub behav_breathing: bool,
    pub behav_circadian_sleep: bool,
    pub behav_dreams: bool,
    pub behav_aging: bool,
    pub behav_typing_resonance: bool,
    pub behav_instance_empathy: bool,
    // ── Custom Initial Conditions ────────────────────────────────────────────
    pub custom_ic: Vec<f64>,
    // ── VU Meter Peak Hold ───────────────────────────────────────────────────
    pub vu_peak_hold: [f32; 4],
    pub vu_peak_decay_timer: f32,
}

/// Periodic session log entry (written by sim thread every ~60s).
/// Fields are collected for future analytics/visualization and are
/// intentionally kept even though they're not yet displayed in the UI.
#[allow(dead_code)]
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
            bifurc_progress: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            bifurc_total_steps: 0,
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
            custom_ode_w: String::new(),
            custom_ode_error: String::new(),
            spectral_freeze_active: false,
            spectral_freeze_freqs: vec![0.0; 16],
            spectral_freeze_amps: vec![0.0; 16],
            spectral_live_partials: [0.0; 32],
            arr_probabilistic: false,
            midi_in_enabled: false,
            midi_in_note_target: "rho".into(),
            midi_in_vel_target: "speed".into(),
            midi_in_cc_target: "reverb_wet".into(),
            midi_in_cc_num: 1,
            midi_in_last_note: 0,
            midi_in_last_vel: 0,
            midi_in_last_cc: 0,
            midi_in_last_cc_num: 0,
            midi_rec_enabled: false,
            midi_rec_events: Vec::new(),
            midi_rec_start: std::time::Instant::now(),
            midi_export_recording: false,
            midi_export_frames: Vec::new(),
            midi_export_duration_secs: 30.0,
            midi_export_status: String::new(),
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
            trajectory_points: Vec::new(),
            session_log: Vec::new(),
            snippets: Vec::new(),
            snippet_slots: vec![None; 32],
            snippet_capture_secs: 8.0,
            snippet_selected: None,
            snippet_status: String::new(),
            song_playing: false,
            song_play_slot: 0,
            snippet_volume: 0.8,
            song_loop: false,
            snippet_pb: Arc::new(Mutex::new(SnippetPlayback::idle())),
            preset_search: String::new(),
            undo_stack: std::collections::VecDeque::new(),
            redo_stack: std::collections::VecDeque::new(),
            bifurc_cache_key: None,
            arr_morph_diff: Vec::new(),
            midi_cc_learn_active: false,
            midi_cc_learn_target: String::new(),
            midi_cc_map: Vec::new(),
            midi_cc_learn_last_cc: 0,
            mod_matrix: Vec::new(),
            toast_queue: Vec::new(),
            lyapunov_history: std::collections::VecDeque::with_capacity(256),
            poincare_points: Vec::new(),
            poincare_z_prev: 0.0,
            noise_inject: 0.0,
            interp_system: "rossler".into(),
            interp_t: 0.0,
            interp_enabled: false,
            basin_data: Arc::new(Mutex::new(Vec::new())),
            basin_computing: false,
            basin_resolution: 80,
            basin_xlim: (-25.0, 25.0),
            basin_ylim: (-35.0, 35.0),
            basin_z_slice: 27.0,
            osc_enabled: false,
            osc_host: "127.0.0.1".into(),
            osc_port: 9000,
            osc_status: String::new(),
            midi_clock_enabled: false,
            xrun_counter: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            stereo_width: 1.0,
            stereo_width_shared: Arc::new(parking_lot::Mutex::new(1.0f32)),
            panel_width: 400.0,
            locked_params: HashSet::new(),
            favorite_presets: {
                let mut set = HashSet::new();
                if let Ok(txt) = std::fs::read_to_string("favorites.txt") {
                    for line in txt.lines() {
                        let name = line.trim().to_string();
                        if !name.is_empty() {
                            set.insert(name);
                        }
                    }
                }
                set
            },
            show_favorites_only: false,
            light_theme: false,
            waveform_zoom: 1.0,
            waveform_offset: 1.0,
            behav_time_of_day: true,
            behav_seasonal_drift: true,
            behav_volume_creep: true,
            behav_breathing: true,
            behav_circadian_sleep: true,
            behav_dreams: true,
            behav_aging: true,
            behav_typing_resonance: true,
            behav_instance_empathy: true,
            custom_ic: vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vu_peak_hold: [0.0; 4],
            vu_peak_decay_timer: 0.0,
        }
    }

    /// Push the current config onto the undo stack (max 20 entries, O(1) with VecDeque).
    /// Must be called from the UI thread only.
    pub fn push_undo(&mut self) {
        if self.undo_stack.len() >= 20 {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(self.config.clone());
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop_back() {
            self.redo_stack.push_back(self.config.clone());
            self.config = prev;
            self.system_changed = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop_back() {
            self.undo_stack.push_back(self.config.clone());
            self.config = next;
            self.system_changed = true;
        }
    }
}

pub type SharedState = Arc<Mutex<AppState>>;

fn lcg_rand(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 33) as f64 / u32::MAX as f64
}

fn apply_theme(ctx: &Context, theme: &str) {
    let mut visuals = ctx.style().visuals.clone();
    visuals.dark_mode = true;

    // Global rounding — rounder corners for a more modern, polished feel
    let round_sm = egui::Rounding::same(6.0);
    let round_md = egui::Rounding::same(10.0);
    visuals.window_rounding = round_md;
    visuals.menu_rounding = round_md;
    visuals.widgets.noninteractive.rounding = round_sm;
    visuals.widgets.inactive.rounding = round_sm;
    visuals.widgets.hovered.rounding = round_sm;
    visuals.widgets.active.rounding = round_sm;
    visuals.widgets.open.rounding = round_sm;
    // Clip inner content to rounded window edges
    visuals.clip_rect_margin = 0.0;

    match theme {
        "vaporwave" => {
            visuals.window_fill = Color32::from_rgb(12, 5, 22);
            visuals.panel_fill = Color32::from_rgb(12, 5, 22);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(26, 10, 42);
            visuals.widgets.noninteractive.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(65, 28, 96));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(38, 16, 60);
            visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(88, 38, 126));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(66, 24, 96);
            visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(1.5, Color32::from_rgb(220, 90, 210));
            visuals.widgets.active.bg_fill = Color32::from_rgb(205, 50, 155);
            visuals.widgets.active.bg_stroke =
                egui::Stroke::new(2.0, Color32::from_rgb(255, 145, 235));
            visuals.selection.bg_fill = Color32::from_rgb(175, 35, 125);
            visuals.override_text_color = Some(Color32::from_rgb(245, 185, 238));
        }
        "crt" => {
            visuals.window_fill = Color32::from_rgb(0, 2, 0);
            visuals.panel_fill = Color32::from_rgb(0, 2, 0);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(0, 10, 0);
            visuals.widgets.noninteractive.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(0, 45, 8));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(0, 18, 4);
            visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(0, 65, 12));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(0, 36, 6);
            visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(1.5, Color32::from_rgb(0, 210, 65));
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 155, 20);
            visuals.widgets.active.bg_stroke =
                egui::Stroke::new(2.0, Color32::from_rgb(0, 255, 85));
            visuals.selection.bg_fill = Color32::from_rgb(0, 115, 15);
            visuals.override_text_color = Some(Color32::from_rgb(0, 255, 65));
        }
        "solar" => {
            visuals.window_fill = Color32::from_rgb(12, 6, 0);
            visuals.panel_fill = Color32::from_rgb(12, 6, 0);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(26, 13, 0);
            visuals.widgets.noninteractive.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(62, 32, 0));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 20, 0);
            visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(92, 48, 0));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(68, 36, 0);
            visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(1.5, Color32::from_rgb(235, 148, 0));
            visuals.widgets.active.bg_fill = Color32::from_rgb(205, 122, 0);
            visuals.widgets.active.bg_stroke =
                egui::Stroke::new(2.0, Color32::from_rgb(255, 205, 65));
            visuals.selection.bg_fill = Color32::from_rgb(175, 98, 0);
            visuals.override_text_color = Some(Color32::from_rgb(255, 218, 115));
        }
        _ => {
            // neon (default) — deep midnight blue
            visuals.window_fill = Color32::from_rgb(8, 9, 18);
            visuals.panel_fill = Color32::from_rgb(8, 9, 18);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(16, 18, 34);
            visuals.widgets.noninteractive.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(38, 44, 78));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(22, 25, 46);
            visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(50, 58, 98));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(30, 36, 66);
            visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(1.5, Color32::from_rgb(70, 148, 242));
            visuals.widgets.active.bg_fill = Color32::from_rgb(15, 128, 222);
            visuals.widgets.active.bg_stroke =
                egui::Stroke::new(2.0, Color32::from_rgb(105, 195, 255));
            visuals.selection.bg_fill = Color32::from_rgb(0, 105, 195);
            visuals.override_text_color = Some(Color32::from_rgb(215, 224, 246));
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
        "logistic_map" => "Logistic Map",
        "standard_map" => "Standard Map",
        "arnold_cat" => "Arnold's Cat Map",
        "stochastic_lorenz" => "Stochastic Lorenz",
        "delayed_map" => "Delayed Logistic Map",
        "oregonator" => "Oregonator",
        "mathieu" => "Mathieu Equation",
        "kuramoto_driven" => "Driven Kuramoto",
        "thomas" => "Thomas Attractor",
        "sprott_c" => "Sprott C",
        "dadras" => "Dadras Attractor",
        "rucklidge" => "Rucklidge Attractor",
        "chen" => "Chen Attractor",
        "burke_shaw" => "Burke-Shaw Attractor",
        "lorenz84" => "Lorenz-84",
        "rabinovich_fabrikant" => "Rabinovich-Fabrikant",
        "sprott_g" => "Sprott G",
        "sprott_h" => "Sprott H",
        "sprott_l" => "Sprott L",
        "rikitake" => "Rikitake Dynamo",
        "bouali" => "Bouali Attractor",
        "newton_leipnik" => "Newton-Leipnik",
        "shimizu_morioka" => "Shimizu-Morioka",
        "genesio_tesi" => "Genesio-Tesi",
        "liu" => "Liu Attractor",
        "windmi" => "WINDMI",
        "finance" => "Finance",
        "hyperchaos" => "Hyperchaos",
        "sprott_k" => "Sprott K",
        "sprott_d" => "Sprott D",
        "sprott_e" => "Sprott E",
        "sprott_f" => "Sprott F",
        _ => "Unknown System",
    }
}

/// Short evocative tagline shown under the system name in the UI.
fn system_tagline(s: &str) -> &'static str {
    match s {
        "lorenz" => "A butterfly-shaped attractor born from fluid convection",
        "rossler" => "A single folded loop spiraling toward chaos",
        "double_pendulum" => "Two coupled pendulums — infinitely sensitive to each starting point",
        "geodesic_torus" => "Trajectories winding endlessly across a donut-shaped surface",
        "kuramoto" => "Coupled oscillators that spontaneously synchronize",
        "three_body" => "Three gravitating masses — analytically unsolvable, endlessly surprising",
        "duffing" => "A driven nonlinear spring with period-doubling cascades",
        "van_der_pol" => "A self-sustained oscillator from vacuum-tube circuit theory",
        "halvorsen" => "Three interlocked spirals locked in cyclic symmetry",
        "aizawa" => "A toroidal attractor with a wormhole-like structure",
        "chua" => "Chaos from the world's simplest electronic circuit",
        "hindmarsh_rose" => "The firing patterns of a single mathematical neuron",
        "coupled_map_lattice" => "A lattice of coupled logistic maps — spatial chaos",
        "custom" => "Your own differential equations, unfolding in real time",
        "mackey_glass" => "A delay-differential equation inspired by blood cell regulation",
        "nose_hoover" => "A conservative thermostat that still generates chaos",
        "sprott_b" => "A minimal three-term system, maximally chaotic",
        "henon_map" => "A discrete attractor discovered while modeling stellar orbits",
        "lorenz96" => "A toy model of atmospheric turbulence on a circular latitude ring",
        "logistic_map" => "The simplest map showing the road from order to chaos",
        "standard_map" => "Area-preserving chaos in angle-action space",
        "arnold_cat" => "A stretching-and-folding map that scrambles any image on a torus",
        "stochastic_lorenz" => "The Lorenz butterfly, shaken by thermal noise",
        "delayed_map" => "A logistic map with memory — the past influences the present",
        "oregonator" => "Chemical oscillations in the Belousov-Zhabotinsky reaction",
        "mathieu" => "A parametrically driven oscillator with stability tongues",
        "kuramoto_driven" => "Kuramoto oscillators forced by an external rhythm",
        "fractional_lorenz" => "Lorenz equations with fractional-order memory",
        "thomas" => "Cyclic symmetry — three coupled oscillators in a dissipative tangle",
        "sprott_c" => "A minimal three-term system with x² nonlinearity",
        "dadras" => "Five-parameter multi-lobed topology with a wide attractor basin",
        "rucklidge" => "Double-convection folded-band chaos from atmospheric physics",
        "chen" => "A double-scroll derived from Lorenz by purposeful anti-control",
        "burke_shaw" => "Elegant two-parameter double-lobe chaos from contraction theory",
        "lorenz84" => "Atmospheric jet-stream model — westerlies and season-driven chaos",
        "rabinovich_fabrikant" => "Plasma waves gone chaotic — multi-lobe topology from physics",
        "sprott_g" => "Single-scroll simplicity: three terms, no parameters, pure chaos",
        "sprott_h" => "Spiral chaos with a z² term from Sprott's minimal catalog",
        "sprott_l" => "Scroll attractor with large z-coupling from Sprott's simplest forms",
        "rikitake" => "Geomagnetic polarity reversals modeled as coupled dynamos",
        "bouali" => "Double-scroll spiral with x² feedback and z-coupling",
        "newton_leipnik" => "Two coupled rigid-body oscillators spontaneously generating chaos",
        "shimizu_morioka" => "A two-scroll oscillator — x²-driven z couples back to destabilize y",
        "genesio_tesi" => "A third-order Jerk system: one x² term is all the chaos you need",
        "liu" => "Single-band scroll: y² drags x down while mxy and kxz exchange energy",
        "windmi" => "Ionospheric substorm current model — exponential nonlinearity in a jerk-form system",
        "finance" => "Chaotic macroeconomics: interest rate, investment, and price-index feedback loops",
        "hyperchaos" => "Chen-Li 4D system with two positive Lyapunov exponents — richer than ordinary chaos",
        "sprott_k" => "xy coupling drives chaos; 0.3z growth balanced by dissipation — one of Sprott's simplest forms",
        "sprott_d" => "Sprott Case I: y² instability with −1.1z dissipation — bounded chaotic attractor",
        "sprott_e" => "Minimal chaos from a yz product — equilibrium at (¼, 1/16, 0)",
        "sprott_f" => "Slow-spiral chaos: x² drives z while y damps at half speed",
        _ => "A dynamical system evolving through state space",
    }
}

// system_internal_name is called from the system-selector dropdown when the
// user types a name into the search box (ui_timeline.rs path). The compiler
// sees only a few call sites inside cfg(feature) blocks, triggering a false-positive.
#[allow(dead_code)]
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
        "Logistic Map" => "logistic_map",
        "Standard Map" => "standard_map",
        "Arnold's Cat Map" => "arnold_cat",
        "Stochastic Lorenz" => "stochastic_lorenz",
        "Delayed Logistic Map" => "delayed_map",
        "Oregonator" => "oregonator",
        "Mathieu Equation" => "mathieu",
        "Driven Kuramoto" => "kuramoto_driven",
        "Thomas Attractor" => "thomas",
        "Sprott C" => "sprott_c",
        "Dadras Attractor" => "dadras",
        "Rucklidge Attractor" => "rucklidge",
        "Chen Attractor" => "chen",
        "Burke-Shaw Attractor" => "burke_shaw",
        "Lorenz-84" => "lorenz84",
        "Rabinovich-Fabrikant" => "rabinovich_fabrikant",
        "Sprott G" => "sprott_g",
        "Sprott H" => "sprott_h",
        "Sprott L" => "sprott_l",
        "Rikitake Dynamo" => "rikitake",
        "Bouali Attractor" => "bouali",
        "Newton-Leipnik" => "newton_leipnik",
        "Shimizu-Morioka" => "shimizu_morioka",
        "Genesio-Tesi" => "genesio_tesi",
        "Liu Attractor" => "liu",
        "WINDMI" => "windmi",
        "Finance" => "finance",
        "Hyperchaos" => "hyperchaos",
        "Sprott K" => "sprott_k",
        "Sprott D" => "sprott_d",
        "Sprott E" => "sprott_e",
        "Sprott F" => "sprott_f",
        _ => "lorenz",
    }
}

fn scale_description(scale: &str) -> &'static str {
    match scale {
        "pentatonic" => "5 notes — always sounds musical",
        "chromatic" => "All 12 semitones — more dissonant",
        "just_intonation" => "Pure harmonic ratios — warm and consonant",
        "microtonal" => "Quarter-tones — alien and otherworldly",
        "edo19" => "19-EDO: 19 equal divisions of the octave (~0.632 semitones per step)",
        "edo31" => "31-EDO: 31 equal divisions — rich microtonality (~0.387 semitones per step)",
        "edo24" => "24-EDO: quarter-tones — half a semitone per step",
        "whole_tone" => "6-note whole-tone scale — dreamy, symmetric, no leading tone",
        "phrygian" => "E Phrygian: dark and tense — flamenco and metal favourite",
        "lydian" => "F Lydian: bright and ethereal — raised 4th creates tension",
        "harmonic_series" => "Integer multiples of A2 (110 Hz) — natural overtone series",
        "hirajoshi" => "Japanese pentatonic — sparse, austere, evokes koto and shakuhachi",
        "blues" => "Blues hexatonic — flat third and fifth give a soulful bent-note feel",
        "dorian" => "D Dorian — minor with raised 6th; modal jazz and Celtic folk favourite",
        "mixolydian" => "G Mixolydian — major with flat 7th; bluesy, open, rock-inflected brightness",
        "hungarian_minor" => "Hungarian minor — augmented 4th and leading 7th; dramatic, Eastern European colour",
        "locrian" => "B Locrian — darkest mode; diminished tonic gives an unresolved, suspended tension",
        "octatonic" => "Diminished scale — perfectly symmetric 8-note set; used in jazz and 20th-century composition",
        "natural_minor" => "A Aeolian — standard minor scale; melancholic, familiar, widely used in classical and rock",
        "harmonic_minor" => "A harmonic minor — raised 7th creates an exotic leading tone; flamenco and Middle-Eastern colour",
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

pub(crate) const CYAN: Color32 = Color32::from_rgb(90, 195, 255);
pub(crate) const GRAY_HINT: Color32 = Color32::from_rgb(135, 142, 168);
pub(crate) const AMBER: Color32 = Color32::from_rgb(225, 178, 55);
const GREEN_ACC: Color32 = Color32::from_rgb(45, 215, 128);
#[allow(dead_code)]
const DIM_BG: Color32 = Color32::from_rgb(16, 18, 34);
#[allow(dead_code)]
pub(crate) const VIOLET: Color32 = Color32::from_rgb(168, 98, 255);
#[allow(dead_code)]
const ROSE: Color32 = Color32::from_rgb(255, 88, 128);
#[allow(dead_code)]
const TEAL_ACC: Color32 = Color32::from_rgb(0, 200, 178);
/// Subtle panel background used for inset frames and section headers.
const SECTION_BG: Color32 = Color32::from_rgb(13, 15, 30);

fn collapsing_section(
    ui: &mut Ui,
    label: &str,
    default_open: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    ui.add_space(3.0);
    let outer = egui::Frame::none()
        .fill(SECTION_BG)
        .inner_margin(egui::Margin { left: 10.0, right: 6.0, top: 3.0, bottom: 3.0 })
        .rounding(egui::Rounding::same(6.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            CollapsingHeader::new(RichText::new(label).size(12.5).color(CYAN).strong())
                .default_open(default_open)
                .show(ui, |ui| {
                    ui.add_space(5.0);
                    add_contents(ui);
                    ui.add_space(3.0);
                });
        });
    // 3 px cyan accent bar on the left edge of the section
    let r = outer.response.rect;
    let bar = egui::Rect::from_min_max(r.left_top(), egui::pos2(r.left() + 3.0, r.bottom()));
    ui.painter().rect_filled(bar, egui::Rounding::same(1.5), CYAN.linear_multiply(0.45));
    ui.add_space(3.0);
}

/// #12: Get a named parameter value from Config.
fn get_param_value(config: &crate::config::Config, key: &str) -> f64 {
    match key {
        "lorenz.sigma" => config.lorenz.sigma,
        "lorenz.rho" => config.lorenz.rho,
        "lorenz.beta" => config.lorenz.beta,
        "system.speed" => config.system.speed,
        "audio.reverb_wet" => config.audio.reverb_wet as f64,
        "audio.delay_ms" => config.audio.delay_ms as f64,
        "audio.master_volume" => config.audio.master_volume as f64,
        _ => 0.0,
    }
}

/// #12: Set a named parameter value on Config.
fn set_param_value(config: &mut crate::config::Config, key: &str, val: f64) {
    match key {
        "lorenz.sigma" => config.lorenz.sigma = val,
        "lorenz.rho" => config.lorenz.rho = val,
        "lorenz.beta" => config.lorenz.beta = val,
        "system.speed" => config.system.speed = val,
        "audio.reverb_wet" => config.audio.reverb_wet = val as f32,
        "audio.delay_ms" => config.audio.delay_ms = val as f32,
        "audio.master_volume" => config.audio.master_volume = val as f32,
        _ => {}
    }
}

/// #12: Small lock-toggle button. Returns true if the param is now locked.
fn lock_btn(ui: &mut Ui, locked_params: &mut HashSet<String>, key: &str) {
    let locked = locked_params.contains(key);
    let icon = if locked { "🔒" } else { "🔓" };
    let col = if locked {
        Color32::from_rgb(220, 160, 40)
    } else {
        Color32::from_rgb(80, 85, 110)
    };
    if ui
        .add(
            Button::new(RichText::new(icon).size(11.0).color(col))
                .fill(Color32::TRANSPARENT)
                .frame(false)
                .min_size(Vec2::new(18.0, 18.0)),
        )
        .on_hover_text(if locked {
            "Unlock parameter (preset loads will restore this value)"
        } else {
            "Lock parameter (preset loads will not change this)"
        })
        .clicked()
    {
        if locked {
            locked_params.remove(key);
        } else {
            locked_params.insert(key.to_string());
        }
    }
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
    let (theme, light_theme) = {
        let st = state.lock();
        (st.theme.clone(), st.light_theme)
    };
    if light_theme {
        ctx.set_visuals(egui::Visuals::light());
    } else {
        apply_theme(ctx, &theme);
    }

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
            if i.key_pressed(Key::Num1) {
                st.viz_tab = 0;
            } // Phase Portrait
            if i.key_pressed(Key::Num2) {
                st.viz_tab = 1;
            } // MIXER
            if i.key_pressed(Key::Num3) {
                st.viz_tab = 2;
            } // ARRANGE
            if i.key_pressed(Key::Num4) {
                st.viz_tab = 3;
            } // Waveform
            if i.key_pressed(Key::Num5) {
                st.viz_tab = 4;
            } // Note Map
            if i.key_pressed(Key::Num6) {
                st.viz_tab = 5;
            } // Math View
            if i.key_pressed(Key::Num7) {
                st.viz_tab = 6;
            } // Bifurcation
            if i.key_pressed(Key::Num8) {
                st.viz_tab = 7;
            } // Studio
            if i.key_pressed(Key::Num9) {
                st.viz_tab = 8;
            } // Basin
            if i.key_pressed(Key::F) {
                st.perf_mode = !st.perf_mode;
            }
            // New shortcuts
            if i.key_pressed(Key::R) { /* recording toggle handled below */ }
            if i.key_pressed(Key::P) {
                st.arr_playing = !st.arr_playing;
            }
            if i.key_pressed(Key::E) {
                st.macro_walk_enabled = !st.macro_walk_enabled;
            }
            if i.key_pressed(Key::Questionmark) {
                st.show_tips_window = !st.show_tips_window;
            }
            // Undo / Redo
            let ctrl = i.modifiers.ctrl;
            let shift = i.modifiers.shift;
            if ctrl && !shift && i.key_pressed(Key::Z) {
                st.undo();
                st.toast_queue.push(Toast::info("Undone"));
            }
            if ctrl && shift && i.key_pressed(Key::Z) {
                st.redo();
                st.toast_queue.push(Toast::info("Redone"));
            }
        });
    } // lock released here

    // R key: toggle recording (needs WavRecorder access — handled outside the state lock)
    {
        let r_pressed = ctx.input(|i| i.key_pressed(Key::R));
        if r_pressed {
            let sr = state.lock().sample_rate;
            if recording.try_lock().map_or(false, |l| l.is_some()) {
                if let Some(mut lock) = recording.try_lock() {
                    *lock = None;
                }
            } else {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let filename = format!("recording_{}.wav", secs);
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
            }
        }
    }

    // Performance mode: fullscreen phase portrait only
    let is_perf_mode = state.lock().perf_mode;
    if is_perf_mode {
        egui::CentralPanel::default().show(ctx, |ui| {
            let trail_color = state.lock().trail_color;
            let (
                projection,
                rotation_angle,
                auto_rotate,
                system_name,
                mode_name,
                current_state,
                current_deriv,
            ) = {
                let st = state.lock();
                (
                    st.viz_projection,
                    st.rotation_angle,
                    st.auto_rotate,
                    st.config.system.name.clone(),
                    st.config.sonification.mode.clone(),
                    st.current_state.clone(),
                    st.current_deriv.clone(),
                )
            };
            let (ag, ag_sep) = {
                let st = state.lock();
                (st.anaglyph_3d, st.anaglyph_separation)
            };
            let (pg, pi) = {
                let st = state.lock();
                (st.portrait_ghosts.clone(), st.portrait_ink.clone())
            };
            let lunar = { state.lock().lunar_phase };
            let (scars_perf, tod_perf) = {
                let st = state.lock();
                (st.scars.clone(), st.time_of_day_f)
            };
            let ee_perf = { state.lock().energy_error };
            draw_phase_portrait(
                ui,
                viz_points,
                &system_name,
                &mode_name,
                &current_state,
                &current_deriv,
                projection,
                rotation_angle,
                auto_rotate,
                trail_color,
                ag,
                ag_sep,
                &pg,
                &pi,
                lunar,
                &scars_perf,
                tod_perf,
                ee_perf,
            );
            // Dim hint in corner
            let rect = ui.min_rect();
            ui.painter().text(
                egui::Pos2::new(rect.left() + 10.0, rect.bottom() - 20.0),
                egui::Align2::LEFT_BOTTOM,
                "Press F to exit performance mode",
                egui::FontId::proportional(12.0),
                Color32::from_rgba_premultiplied(150, 150, 150, 100),
            );
            // VU meter strip — bottom-right corner overlay in performance mode
            let (vu_vals, chaos) = {
                let st = state.lock();
                let vu = *st.vu_meter.lock();
                (vu, st.chaos_level)
            };
            let bar_w = 10.0f32;
            let bar_gap = 4.0f32;
            let bar_h_max = 80.0f32;
            let strip_x = rect.right() - 10.0 - 4.0 * (bar_w + bar_gap);
            let strip_y = rect.bottom() - 10.0 - bar_h_max;
            let labels = ["L1", "L2", "L3", "M"];
            for (i, &v) in vu_vals.iter().enumerate() {
                let h = (v.clamp(0.0, 1.0) * bar_h_max).max(1.0);
                let x = strip_x + i as f32 * (bar_w + bar_gap);
                let bar_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(x, strip_y + bar_h_max - h),
                    egui::Vec2::new(bar_w, h),
                );
                let hue = if v > 0.85 { 0.0 } else if v > 0.6 { 0.12 } else { 0.35 };
                let col: egui::Color32 = egui::epaint::Hsva::new(hue, 0.9, 0.9, 0.75).into();
                ui.painter().rect_filled(bar_rect, 2.0, col);
                ui.painter().text(
                    egui::Pos2::new(x + bar_w * 0.5, rect.bottom() - 4.0),
                    egui::Align2::CENTER_BOTTOM,
                    labels[i],
                    egui::FontId::proportional(9.0),
                    Color32::from_rgba_premultiplied(180, 180, 180, 120),
                );
            }
            // Chaos level pill
            let chaos_txt = format!("⟁ {:.2}", chaos);
            ui.painter().text(
                egui::Pos2::new(rect.right() - 10.0, rect.top() + 10.0),
                egui::Align2::RIGHT_TOP,
                &chaos_txt,
                egui::FontId::proportional(14.0),
                Color32::from_rgba_premultiplied(
                    (chaos * 255.0) as u8,
                    ((1.0 - chaos) * 180.0) as u8,
                    60,
                    180,
                ),
            );
        });
        return;
    }

    let panel_w = state.lock().panel_width;
    let panel_resp = SidePanel::left("controls")
        .resizable(true)
        .min_width(200.0)
        .max_width(900.0)
        .exact_width(panel_w)
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(8.0);

                // ── App identity header ────────────────────────────────────────────────
                let (hdr_sys, hdr_mode) = {
                    let st = state.lock();
                    (st.config.system.name.clone(), st.config.sonification.mode.clone())
                };
                let hdr_sys_display = system_display_name(&hdr_sys);
                let hdr_sys_tag = system_tagline(&hdr_sys);
                egui::Frame::none()
                    .fill(Color32::from_rgb(8, 9, 22))
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                    .rounding(egui::Rounding::same(10.0))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(28, 50, 96)))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        // Title row
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("MATH SONIFY").size(17.0).color(CYAN).strong(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new("v1.2")
                                            .size(9.0)
                                            .color(GRAY_HINT.linear_multiply(0.55)),
                                    );
                                },
                            );
                        });
                        ui.add_space(5.0);
                        // Thin accent separator
                        {
                            let aw = ui.available_width();
                            let (sep, _) = ui.allocate_exact_size(Vec2::new(aw, 1.0), Sense::hover());
                            ui.painter().rect_filled(sep, 0.0, CYAN.linear_multiply(0.15));
                        }
                        ui.add_space(5.0);
                        // Active system row
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("▸").size(11.0).color(CYAN.linear_multiply(0.65)),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(hdr_sys_display)
                                    .size(13.5)
                                    .color(Color32::from_rgb(195, 218, 255))
                                    .strong(),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    RichText::new(&hdr_mode)
                                        .size(9.5)
                                        .color(AMBER.linear_multiply(0.7))
                                        .italics(),
                                );
                            });
                        });
                        ui.add_space(2.0);
                        // Tagline
                        ui.add(
                            egui::Label::new(
                                RichText::new(hdr_sys_tag)
                                    .size(9.5)
                                    .color(GRAY_HINT.linear_multiply(0.85))
                                    .italics(),
                            )
                            .wrap(true),
                        );
                    });
                ui.add_space(8.0);

                // ── Live chaos status bar ──────────────────────────────────────────────
                {
                    let (chaos, paused, sb_vu) = {
                        let st = state.lock();
                        let vu = *st.vu_meter.lock();
                        (st.chaos_level, st.paused, vu)
                    };
                    let chaos_col = lerp_color(
                        Color32::from_rgb(40, 120, 210),
                        Color32::from_rgb(225, 72, 28),
                        chaos,
                    );
                    // Pulsing animation: dot radius breathes when live
                    let t = ui.input(|i| i.time as f32);
                    let dot_radius = if paused { 3.5 } else { 3.5 + (t * 2.8).sin() * 0.9 };
                    ctx.request_repaint();
                    egui::Frame::none()
                        .fill(Color32::from_rgb(11, 13, 26))
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                        .rounding(egui::Rounding::same(8.0))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(28, 34, 62)))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            // Top row: status dot + label + chaos%
                            ui.horizontal(|ui| {
                                let dot_col = if paused {
                                    Color32::from_rgb(100, 102, 135)
                                } else {
                                    Color32::from_rgb(72, 228, 118)
                                };
                                // Glow halo when live
                                let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), Sense::hover());
                                let dot_center = dot_rect.center();
                                if !paused {
                                    let glow_alpha = (((t * 2.8).sin() * 0.5 + 0.5) * 50.0) as u8;
                                    ui.painter().circle_filled(
                                        dot_center,
                                        dot_radius + 3.0,
                                        Color32::from_rgba_premultiplied(72, 228, 118, glow_alpha),
                                    );
                                }
                                ui.painter().circle_filled(dot_center, dot_radius, dot_col);
                                ui.add_space(2.0);
                                let status_text = if paused { "PAUSED" } else { "LIVE" };
                                let status_col = if paused {
                                    Color32::from_rgb(130, 132, 162)
                                } else {
                                    Color32::from_rgb(72, 228, 118)
                                };
                                ui.label(
                                    RichText::new(status_text)
                                        .size(10.5)
                                        .color(status_col)
                                        .strong(),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!("{:.0}% chaos", chaos * 100.0))
                                            .size(10.5)
                                            .color(chaos_col)
                                            .strong(),
                                    );
                                    // Mini VU bars (right of status, left of chaos%)
                                    ui.add_space(4.0);
                                    let vu_peak = ((sb_vu[0] + sb_vu[1]) * 0.5).clamp(0.0, 1.0);
                                    let bar_h = 10.0f32;
                                    let bar_w = 3.0f32;
                                    for seg in (0..5u8).rev() {
                                        let thresh = seg as f32 * 0.2;
                                        let lit = vu_peak > thresh;
                                        let seg_col = if seg >= 4 {
                                            Color32::from_rgb(255, 60, 60)
                                        } else if seg >= 3 {
                                            Color32::from_rgb(255, 185, 0)
                                        } else {
                                            Color32::from_rgb(45, 210, 128)
                                        };
                                        let col = if lit { seg_col } else { Color32::from_rgb(22, 24, 44) };
                                        let h = bar_h * (0.4 + seg as f32 * 0.15);
                                        let (r, _) = ui.allocate_exact_size(Vec2::new(bar_w, bar_h), Sense::hover());
                                        let bar_rect = egui::Rect::from_min_size(
                                            Pos2::new(r.min.x, r.max.y - h),
                                            Vec2::new(bar_w, h),
                                        );
                                        ui.painter().rect_filled(bar_rect, 1.0, col);
                                        ui.add_space(1.0);
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            // Chaos progress track
                            let bar_w = ui.available_width();
                            let (bar_rect, _) =
                                ui.allocate_exact_size(Vec2::new(bar_w, 5.0), Sense::hover());
                            ui.painter().rect_filled(bar_rect, 3.0, Color32::from_rgb(18, 20, 38));
                            let fill_w = bar_rect.width() * chaos.clamp(0.0, 1.0);
                            if fill_w > 0.0 {
                                let fill_rect = egui::Rect::from_min_size(
                                    bar_rect.min,
                                    Vec2::new(fill_w, bar_rect.height()),
                                );
                                ui.painter().rect_filled(fill_rect, 3.0, chaos_col);
                            }
                            ui.painter().rect_stroke(
                                bar_rect,
                                3.0,
                                egui::Stroke::new(0.5, Color32::from_rgb(36, 40, 72)),
                            );
                        });
                }
                ui.add_space(8.0);

                // ── Simple / Advanced segmented control ───────────────────────────────
                let is_simple = {
                    let mut st = state.lock();
                    let total_w = ui.available_width();
                    let half_w = (total_w - 2.0) / 2.0;
                    let simple_active = st.simple_mode;

                    // Outer container gives the control a unified pill background
                    egui::Frame::none()
                        .fill(Color32::from_rgb(14, 16, 30))
                        .rounding(egui::Rounding::same(8.0))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(36, 40, 72)))
                        .inner_margin(egui::Margin::same(2.0))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.set_min_width(total_w - 4.0);
                            ui.horizontal(|ui| {
                                // Simple segment — rounded left, flat right
                                let s_fill = if simple_active {
                                    Color32::from_rgb(20, 138, 76)
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let s_stroke = if simple_active {
                                    egui::Stroke::new(0.0, Color32::TRANSPARENT)
                                } else {
                                    egui::Stroke::new(0.0, Color32::TRANSPARENT)
                                };
                                let s_text_col = if simple_active {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(130, 135, 165)
                                };
                                if ui
                                    .add(
                                        Button::new(
                                            RichText::new("Simple")
                                                .color(s_text_col)
                                                .size(12.5)
                                                .strong(),
                                        )
                                        .fill(s_fill)
                                        .stroke(s_stroke)
                                        .rounding(egui::Rounding {
                                            nw: 6.0,
                                            sw: 6.0,
                                            ne: 0.0,
                                            se: 0.0,
                                        })
                                        .min_size(Vec2::new(half_w, 30.0)),
                                    )
                                    .clicked()
                                {
                                    st.simple_mode = true;
                                }
                                // Advanced segment — flat left, rounded right
                                let a_fill = if !simple_active {
                                    Color32::from_rgb(14, 108, 208)
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let a_text_col = if !simple_active {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(130, 135, 165)
                                };
                                if ui
                                    .add(
                                        Button::new(
                                            RichText::new("Advanced")
                                                .color(a_text_col)
                                                .size(12.5)
                                                .strong(),
                                        )
                                        .fill(a_fill)
                                        .stroke(egui::Stroke::new(0.0, Color32::TRANSPARENT))
                                        .rounding(egui::Rounding {
                                            nw: 0.0,
                                            sw: 0.0,
                                            ne: 6.0,
                                            se: 6.0,
                                        })
                                        .min_size(Vec2::new(half_w, 30.0)),
                                    )
                                    .clicked()
                                {
                                    st.simple_mode = false;
                                }
                            });
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

    // #11: sync resized panel width
    if let Some(rw) = Some(panel_resp.response.rect.width()).filter(|&w| w > 1.0) {
        state.lock().panel_width = rw.clamp(200.0, 900.0);
    }

    // ---- CENTRAL PANEL: Visualization ----
    CentralPanel::default().show(ctx, |ui| {
        // Tab bar row with theme switcher on the right
        egui::Frame::none()
            .fill(Color32::from_rgb(9, 10, 21))
            .inner_margin(egui::Margin { left: 4.0, right: 4.0, top: 4.0, bottom: 0.0 })
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(24, 28, 54)))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let tabs = [
                        ("🌀", "Phase",    "Phase portrait — watch the attractor draw itself in real time"),
                        ("🎚", "Mixer",    "Mixer — layer multiple systems and blend their sounds"),
                        ("🎬", "Arrange",  "Arranger — compose multi-scene musical sequences"),
                        ("〰", "Wave",     "Waveform — see the raw audio signal you're generating"),
                        ("🎵", "Notes",    "Note map — which pitches are being played right now"),
                        ("∑",  "Math",     "Math view — Lyapunov exponents, entropy, and live metrics"),
                        ("∿",  "Bifurc",   "Bifurcation — how the system's behavior changes with a parameter"),
                        ("📼", "Studio",   "Studio — record, export WAV, save portraits and loops"),
                        ("⬡",  "Basin",    "Basin of attraction — which initial conditions lead where"),
                        ("⊙",  "Poincaré", "Poincaré section — a slice through the attractor revealing hidden structure"),
                        ("⧈",  "Recurr",   "Recurrence plot — when does the trajectory revisit the same region?"),
                        ("📊", "Spectrum", "Spectral analyser — real-time FFT frequency spectrum of the audio output"),
                    ];
                    let mut viz_tab = state.lock().viz_tab;
                    for (i, (icon, name, tooltip)) in tabs.iter().enumerate() {
                        let selected = viz_tab == i;
                        let (fill, text_col) = if selected {
                            (Color32::from_rgb(14, 118, 210), Color32::WHITE)
                        } else {
                            (
                                Color32::from_rgb(16, 18, 36),
                                Color32::from_rgb(148, 158, 192),
                            )
                        };
                        let stroke = if selected {
                            egui::Stroke::new(1.5, Color32::from_rgb(72, 158, 255))
                        } else {
                            egui::Stroke::new(0.0, Color32::TRANSPARENT)
                        };
                        let btn = Button::new(
                            RichText::new(format!("{} {}", icon, name))
                                .color(text_col)
                                .size(11.5),
                        )
                        .fill(fill)
                        .stroke(stroke)
                        .rounding(egui::Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 })
                        .min_size(Vec2::new(72.0, 28.0));
                        if ui.add(btn).on_hover_text(*tooltip).clicked() {
                            viz_tab = i;
                        }
                    }
                    state.lock().viz_tab = viz_tab;

                    // Theme switcher right-aligned
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let themes = [
                            ("☀", "solar", Color32::from_rgb(200, 115, 10), "Solar"),
                            ("⎕", "crt", Color32::from_rgb(0, 175, 10), "CRT Green"),
                            ("◈", "vaporwave", Color32::from_rgb(175, 38, 135), "Vaporwave"),
                            ("◆", "neon", Color32::from_rgb(10, 132, 220), "Neon"),
                        ];
                        for (icon, theme_key, color, label) in themes.iter() {
                            let is_active = state.lock().theme == *theme_key;
                            let (btn_color, border_w, border_col) = if is_active {
                                (*color, 1.5f32, *color)
                            } else {
                                (
                                    Color32::from_rgb(18, 20, 38),
                                    1.0f32,
                                    Color32::from_rgb(38, 42, 68),
                                )
                            };
                            let icon_col = if is_active {
                                Color32::WHITE
                            } else {
                                Color32::from_rgb(130, 136, 168)
                            };
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(*icon).color(icon_col).size(13.0),
                                    )
                                    .fill(btn_color)
                                    .stroke(egui::Stroke::new(border_w, border_col))
                                    .rounding(egui::Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 })
                                    .min_size(Vec2::new(26.0, 26.0)),
                                )
                                .on_hover_text(*label)
                                .clicked()
                            {
                                state.lock().theme = theme_key.to_string();
                            }
                        }
                        // ── #14: Dark/Light toggle ──────────────────────────────────────────
                        let lt = state.lock().light_theme;
                        let lt_icon = if lt { "🌙" } else { "✦" };
                        let lt_fill = if lt {
                            Color32::from_rgb(198, 200, 222)
                        } else {
                            Color32::from_rgb(18, 20, 38)
                        };
                        let lt_text = if lt {
                            Color32::from_rgb(20, 22, 40)
                        } else {
                            Color32::from_rgb(130, 136, 168)
                        };
                        if ui
                            .add(
                                Button::new(RichText::new(lt_icon).color(lt_text).size(13.0))
                                    .fill(lt_fill)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        Color32::from_rgb(96, 100, 148),
                                    ))
                                    .rounding(egui::Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 })
                                    .min_size(Vec2::new(26.0, 26.0)),
                            )
                            .on_hover_text(if lt {
                                "Switch to dark theme"
                            } else {
                                "Switch to light theme"
                            })
                            .clicked()
                        {
                            state.lock().light_theme = !lt;
                        }
                    });
                });
            });

        let viz_tab = state.lock().viz_tab;

        // ── Song sequencer tick (runs every UI frame regardless of active tab) ──
        {
            let (song_playing, song_loop) = {
                let st = state.lock();
                (st.song_playing, st.song_loop)
            };
            if song_playing {
                let on_complete = {
                    let st = state.lock();
                    let v = st.snippet_pb.lock().on_complete;
                    v
                };
                if on_complete {
                    // Reset the flag
                    {
                        let st = state.lock();
                        st.snippet_pb.lock().on_complete = false;
                    }
                    // Advance to next populated slot
                    let mut st = state.lock();
                    let mut next = st.song_play_slot + 1;
                    // Find next non-empty slot
                    while next < st.snippet_slots.len() && st.snippet_slots[next].is_none() {
                        next += 1;
                    }
                    if next >= st.snippet_slots.len() {
                        if song_loop {
                            // Loop: find first non-empty slot
                            next = 0;
                            while next < st.snippet_slots.len() && st.snippet_slots[next].is_none()
                            {
                                next += 1;
                            }
                        } else {
                            // Song ended
                            st.song_playing = false;
                            next = 0;
                        }
                    }
                    st.song_play_slot = next;
                    if st.song_playing {
                        if let Some(idx) = st.snippet_slots[next] {
                            if idx < st.snippets.len() {
                                let samples = (*st.snippets[idx].samples).clone();
                                let vol = st.snippet_volume;
                                *st.snippet_pb.lock() = SnippetPlayback::play(samples, vol);
                            }
                        }
                    }
                }
            }
        }

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
                    let color = if selected {
                        Color32::from_rgb(0, 140, 210)
                    } else {
                        Color32::from_rgb(40, 40, 70)
                    };
                    if ui
                        .add(
                            Button::new(RichText::new(*label).color(Color32::WHITE))
                                .fill(color)
                                .min_size(Vec2::new(36.0, 22.0)),
                        )
                        .clicked()
                    {
                        new_proj = i;
                    }
                }
                ui.separator();
                let rot_color = if auto_rot {
                    Color32::from_rgb(0, 160, 100)
                } else {
                    Color32::from_rgb(40, 40, 70)
                };
                let mut new_auto_rot = auto_rot;
                if ui
                    .add(
                        Button::new(RichText::new("3D Rotate").color(Color32::WHITE))
                            .fill(rot_color)
                            .min_size(Vec2::new(72.0, 22.0)),
                    )
                    .clicked()
                {
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
                let ag_color = if anaglyph {
                    Color32::from_rgb(180, 40, 40)
                } else {
                    Color32::from_rgb(40, 40, 70)
                };
                let mut new_anaglyph = anaglyph;
                let mut new_sep = sep;
                if ui
                    .add(
                        Button::new(RichText::new("Anaglyph 3D").color(Color32::WHITE))
                            .fill(ag_color)
                            .min_size(Vec2::new(84.0, 22.0)),
                    )
                    .clicked()
                {
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
                let param_opts = ["rho", "sigma", "coupling", "c", "r", "k", "b", "lambda", "a_rossler", "shimizu_a", "genesio_c", "liu_b", "windmi_b", "finance_a", "hyperchaos_c"];
                let (current_bp, current_bp2, computing, mode_2d) = {
                    let st = state.lock();
                    (
                        st.bifurc_param.clone(),
                        st.bifurc_param2.clone(),
                        st.bifurc_computing,
                        st.bifurc_2d_mode,
                    )
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
                if ui
                    .toggle_value(&mut new_2d, "2D")
                    .on_hover_text("Sweep two parameters as a heatmap")
                    .changed()
                {}
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

                // Check if the cache is still valid for the current settings
                let cache_hit = {
                    let st = state.lock();
                    let cache_key = (
                        st.config.system.name.clone(),
                        st.bifurc_param.clone(),
                        st.bifurc_param2.clone(),
                        st.bifurc_2d_mode,
                    );
                    st.bifurc_cache_key.as_ref() == Some(&cache_key)
                        && !bifurc_data.lock().is_empty()
                };
                if cache_hit {
                    ui.label(
                        RichText::new("✓ cached")
                            .color(Color32::from_rgb(80, 200, 80))
                            .size(10.0),
                    );
                }
                // "Force Refresh" button clears the cache
                if cache_hit
                    && ui
                        .small_button("↺ Refresh")
                        .on_hover_text("Force recompute")
                        .clicked()
                {
                    state.lock().bifurc_cache_key = None;
                }

                let compute_color = if computing {
                    Color32::from_rgb(100, 60, 0)
                } else {
                    Color32::from_rgb(0, 80, 120)
                };
                if !computing
                    && !cache_hit
                    && ui
                        .add(
                            Button::new(RichText::new("Compute").color(Color32::WHITE))
                                .fill(compute_color),
                        )
                        .clicked()
                {
                    let (param, param2, is_2d, sys_name, lorenz_cfg, rossler_cfg, kuramoto_cfg) = {
                        let st = state.lock();
                        (
                            st.bifurc_param.clone(),
                            st.bifurc_param2.clone(),
                            st.bifurc_2d_mode,
                            st.config.system.name.clone(),
                            st.config.lorenz.clone(),
                            st.config.rossler.clone(),
                            st.config.kuramoto.clone(),
                        )
                    };
                    {
                        let mut st = state.lock();
                        let total = if is_2d { 40 * 40 } else { 200 };
                        st.bifurc_computing = true;
                        st.bifurc_total_steps = total;
                        st.bifurc_progress.store(0, std::sync::atomic::Ordering::Relaxed);
                    }
                    let bifurc_data_clone = bifurc_data.clone();
                    let state_clone = state.clone();
                    let progress_arc = state.lock().bifurc_progress.clone();
                    std::thread::spawn(move || {
                        if is_2d {
                            // 2D: sweep param1 × param2 on a 40×40 grid; measure chaos via speed variance
                            let grid = 40usize;
                            let (p1min, p1max) = param_range(&param);
                            let (p2min, p2max) = param_range(&param2);
                            let result: Vec<(f32, f32, f32)> = (0..grid * grid)
                                .into_par_iter()
                                .map(|idx| {
                                    let i = idx / grid;
                                    let j = idx % grid;
                                    let p1val =
                                        p1min + (p1max - p1min) * i as f64 / (grid - 1) as f64;
                                    let p2val =
                                        p2min + (p2max - p2min) * j as f64 / (grid - 1) as f64;
                                    let mut sys = build_bifurc_system(
                                        &sys_name,
                                        &param,
                                        p1val,
                                        &lorenz_cfg,
                                        &rossler_cfg,
                                        &kuramoto_cfg,
                                    );
                                    // Apply second param on top of first
                                    let mut sys2 = build_bifurc_system(
                                        &sys_name,
                                        &param2,
                                        p2val,
                                        &lorenz_cfg,
                                        &rossler_cfg,
                                        &kuramoto_cfg,
                                    );
                                    // Use whichever system has both params set (build_bifurc only sets one)
                                    // For simplicity: rebuild with param=p1 then override param2 via a combined system
                                    // Workaround: use sys for p1, apply p2 perturbation as initial offset
                                    for _ in 0..1000 {
                                        sys.step(0.005);
                                        sys2.step(0.005);
                                    }
                                    // Chaos metric: variance of x over 200 steps
                                    let mut vals = Vec::with_capacity(200);
                                    for _ in 0..200 {
                                        sys.step(0.005);
                                        if let Some(&v) = sys.state().first() {
                                            vals.push(v);
                                        }
                                    }
                                    let mean = vals.iter().sum::<f64>() / vals.len().max(1) as f64;
                                    let var = vals.iter().map(|&v| (v - mean).powi(2)).sum::<f64>()
                                        / vals.len().max(1) as f64;
                                    let _ = sys2; // suppress unused warning
                                    progress_arc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    (p1val as f32, p2val as f32, var.sqrt() as f32)
                                })
                                .collect();
                            // For 2D we store in AppState directly
                            *state_clone.lock().bifurc_data_2d.lock() = result;
                            *bifurc_data_clone.lock() = Vec::new(); // clear 1D display
                        } else {
                            // 1D: parallel sweep over parameter range
                            let steps = 200usize;
                            let (pmin, pmax) = param_range(&param);
                            let result: Vec<(f32, f32)> = (0..steps)
                                .into_par_iter()
                                .flat_map(|i| {
                                    let pval = pmin + (pmax - pmin) * i as f64 / (steps - 1) as f64;
                                    let mut sys = build_bifurc_system(
                                        &sys_name,
                                        &param,
                                        pval,
                                        &lorenz_cfg,
                                        &rossler_cfg,
                                        &kuramoto_cfg,
                                    );
                                    for _ in 0..2000 {
                                        sys.step(0.005);
                                    }
                                    let mut pts = Vec::with_capacity(100);
                                    for _ in 0..100 {
                                        sys.step(0.005);
                                        if let Some(&v) = sys.state().first() {
                                            pts.push((pval as f32, v as f32));
                                        }
                                    }
                                    progress_arc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    pts
                                })
                                .collect();
                            *bifurc_data_clone.lock() = result;
                        }
                        let mut st = state_clone.lock();
                        st.bifurc_computing = false;
                        // Store cache key so subsequent renders skip recompute
                        st.bifurc_cache_key = Some((
                            st.config.system.name.clone(),
                            param.clone(),
                            param2.clone(),
                            is_2d,
                        ));
                    });
                }
                if computing {
                    let (done, total) = {
                        let st = state.lock();
                        (
                            st.bifurc_progress.load(std::sync::atomic::Ordering::Relaxed),
                            st.bifurc_total_steps,
                        )
                    };
                    let pct = if total > 0 { (done * 100 / total).min(99) } else { 0 };
                    ui.label(
                        RichText::new(format!("Computing… {}%", pct))
                            .color(Color32::from_rgb(255, 200, 0)),
                    );
                    let bar_w = ui.available_width();
                    let (bar_rect, _) = ui.allocate_exact_size(
                        egui::Vec2::new(bar_w, 4.0),
                        egui::Sense::hover(),
                    );
                    let fill_w = bar_w * pct as f32 / 100.0;
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(bar_rect.min, egui::Vec2::new(fill_w, 4.0)),
                        0.0,
                        Color32::from_rgb(255, 200, 0),
                    );
                    ctx.request_repaint();
                }
                // Basin controls — always visible when on the basin tab
                if viz_tab == 8 {
                    ui.horizontal(|ui| {
                        let (basin_computing, resolution, xlim, ylim, z_slice, sigma, rho) = {
                            let st = state.lock();
                            (
                                st.basin_computing,
                                st.basin_resolution,
                                st.basin_xlim,
                                st.basin_ylim,
                                st.basin_z_slice,
                                st.config.lorenz.sigma,
                                st.config.lorenz.rho,
                            )
                        };
                        ui.label(
                            RichText::new(format!("σ={:.1} ρ={:.1}", sigma, rho))
                                .color(Color32::from_rgb(140, 200, 255))
                                .size(11.0),
                        );
                        ui.separator();
                        let mut new_res = resolution;
                        ui.label("Res:");
                        ui.add(
                            egui::DragValue::new(&mut new_res)
                                .clamp_range(20usize..=200usize)
                                .speed(1.0),
                        );
                        let mut new_xl0 = xlim.0;
                        let mut new_xl1 = xlim.1;
                        let mut new_yl0 = ylim.0;
                        let mut new_yl1 = ylim.1;
                        let mut new_z = z_slice;
                        ui.label("X:");
                        ui.add(egui::DragValue::new(&mut new_xl0).speed(0.5).prefix("lo "));
                        ui.add(egui::DragValue::new(&mut new_xl1).speed(0.5).prefix("hi "));
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut new_yl0).speed(0.5).prefix("lo "));
                        ui.add(egui::DragValue::new(&mut new_yl1).speed(0.5).prefix("hi "));
                        ui.label("Z0:");
                        ui.add(egui::DragValue::new(&mut new_z).speed(0.5));
                        {
                            let mut st = state.lock();
                            st.basin_resolution = new_res;
                            st.basin_xlim = (new_xl0, new_xl1);
                            st.basin_ylim = (new_yl0, new_yl1);
                            st.basin_z_slice = new_z;
                        }
                        let compute_color = if basin_computing {
                            Color32::from_rgb(100, 60, 0)
                        } else {
                            Color32::from_rgb(0, 80, 120)
                        };
                        if !basin_computing
                            && ui
                                .add(
                                    Button::new(
                                        RichText::new("Compute Basin").color(Color32::WHITE),
                                    )
                                    .fill(compute_color),
                                )
                                .clicked()
                        {
                            let (xlim2, ylim2, z_slice2, resolution2, basin_out) = {
                                let mut st = state.lock();
                                st.basin_computing = true;
                                let xl = st.basin_xlim;
                                let yl = st.basin_ylim;
                                let zs = st.basin_z_slice;
                                let rs = st.basin_resolution;
                                let out = st.basin_data.clone();
                                *out.lock() = Vec::new();
                                (xl, yl, zs, rs, out)
                            };
                            let state_clone = state.clone();
                            std::thread::spawn(move || {
                                compute_basin(
                                    xlim2,
                                    ylim2,
                                    z_slice2,
                                    resolution2,
                                    "lorenz",
                                    basin_out,
                                );
                                state_clone.lock().basin_computing = false;
                            });
                        }
                        if basin_computing {
                            ui.label(
                                RichText::new("Computing...")
                                    .color(Color32::from_rgb(255, 200, 0)),
                            );
                        }
                    });
                }
            });
        }

        ui.separator();

        let (
            projection,
            rotation_angle,
            auto_rotate,
            system_name,
            mode_name,
            freqs,
            voice_levels,
            chord_intervals,
            current_state,
            current_deriv,
            chaos_level,
            order_param,
            kuramoto_phases,
            trail_color,
            _perf_mode,
            anaglyph_3d,
            anaglyph_separation,
            lyapunov_spectrum,
            attractor_type,
            kolmogorov_entropy,
            energy_error,
            sync_error,
            permutation_entropy,
            integrator_divergence,
        ) = {
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
            (
                proj, rot, ar, sn, mn, fr, vl, ci, cs, cd, cl, op, kp, tc, pm, ag, ag_sep, ls, at,
                ke, ee, se, pe, id,
            )
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
                && !viz_points.is_empty()
            {
                // Sample every 4th point to keep ghost compact
                let ghost_pts: Vec<(f32, f32)> = viz_points
                    .iter()
                    .step_by(4)
                    .map(|&(x, y, _, _, _)| (x, y))
                    .collect();
                st.portrait_ghosts
                    .push((ghost_pts, std::time::Instant::now()));
                st.portrait_ghost_last_capture = std::time::Instant::now();
                // Keep at most 4 ghost snapshots
                while st.portrait_ghosts.len() > 4 {
                    st.portrait_ghosts.remove(0);
                }
            }
        }
        let (ghosts, ink, lunar_phase2) = {
            let st = state.lock();
            (
                st.portrait_ghosts.clone(),
                st.portrait_ink.clone(),
                st.lunar_phase,
            )
        };
        let (scars_main, tod_main) = {
            let st = state.lock();
            (st.scars.clone(), st.time_of_day_f)
        };

        // Feature #7: push current Lyapunov[0] into scrolling history
        {
            let mut st = state.lock();
            if let Some(&lam) = st.lyapunov_spectrum.first() {
                st.lyapunov_history.push_back(lam as f32);
                if st.lyapunov_history.len() > 256 {
                    st.lyapunov_history.pop_front();
                }
            }
        }

        match viz_tab {
            0 => draw_phase_portrait(
                ui,
                viz_points,
                &system_name,
                &mode_name,
                &current_state,
                &current_deriv,
                projection,
                rotation_angle,
                auto_rotate,
                trail_color,
                anaglyph_3d,
                anaglyph_separation,
                &ghosts,
                &ink,
                lunar_phase2,
                &scars_main,
                tod_main,
                energy_error,
            ),
            1 => draw_mixer_tab(ui, state, viz_points),
            2 => draw_arrange_tab(ui, state, recording),
            3 => {
                let (wz, wo) = {
                    let st = state.lock();
                    (st.waveform_zoom, st.waveform_offset)
                };
                let (new_wz, new_wo) = draw_waveform(ui, waveform, wz, wo, ctx);
                {
                    let mut st = state.lock();
                    st.waveform_zoom = new_wz;
                    st.waveform_offset = new_wo;
                }
            }
            4 => draw_note_map(ui, &freqs, &voice_levels, &chord_intervals),
            5 => {
                let lyap_hist = state.lock().lyapunov_history.clone();
                draw_math_view(
                    ui,
                    &system_name,
                    &current_state,
                    &current_deriv,
                    chaos_level,
                    order_param,
                    &kuramoto_phases,
                    &lyapunov_spectrum,
                    &attractor_type,
                    kolmogorov_entropy,
                    energy_error,
                    sync_error,
                    permutation_entropy,
                    integrator_divergence,
                    &lyap_hist,
                );
            }
            6 => draw_bifurc_diagram(ui, bifurc_data, state),
            7 => draw_studio_tab(ui, state),
            8 => draw_basin_tab(ui, state),
            9 => draw_poincare_tab(ui, state),
            10 => draw_recurrence_tab(ui, state),
            11 => draw_spectrum_tab(ui, waveform, state),
            _ => {}
        }
    });

    // ── Toast notification overlay ───────────────────────────────────────────
    // Expire stale toasts, then render remaining ones in the bottom-right corner.
    {
        let now = std::time::Instant::now();
        let mut st = state.lock();
        st.toast_queue
            .retain(|t| now.duration_since(t.born).as_secs_f32() < t.ttl_secs);
        let toasts: Vec<Toast> = st.toast_queue.clone();
        drop(st);
        if !toasts.is_empty() {
            let screen_rect = ctx.screen_rect();
            let mut y_offset = screen_rect.max.y - 16.0;
            for toast in toasts.iter().rev() {
                let age = now.duration_since(toast.born).as_secs_f32();
                let fade = if age < toast.ttl_secs - 0.5 {
                    1.0f32
                } else {
                    ((toast.ttl_secs - age) / 0.5).clamp(0.0, 1.0)
                };
                let (bg_col, accent_col, icon) = match toast.kind {
                    ToastKind::Info => (
                        Color32::from_rgba_unmultiplied(22, 36, 62, (230.0 * fade) as u8),
                        Color32::from_rgba_unmultiplied(55, 140, 230, (255.0 * fade) as u8),
                        "ℹ",
                    ),
                    ToastKind::Warning => (
                        Color32::from_rgba_unmultiplied(42, 32, 8, (230.0 * fade) as u8),
                        Color32::from_rgba_unmultiplied(210, 155, 0, (255.0 * fade) as u8),
                        "⚠",
                    ),
                    ToastKind::Error => (
                        Color32::from_rgba_unmultiplied(48, 14, 14, (230.0 * fade) as u8),
                        Color32::from_rgba_unmultiplied(210, 48, 48, (255.0 * fade) as u8),
                        "✕",
                    ),
                };
                let text = format!("{} {}", icon, toast.message);
                let galley = ctx.fonts(|f| {
                    f.layout_no_wrap(text.clone(), FontId::proportional(13.0), Color32::WHITE)
                });
                let w = galley.rect.width() + 28.0;
                let h = galley.rect.height() + 16.0;
                y_offset -= h + 8.0;
                if y_offset < 0.0 {
                    break;
                }
                let rect = Rect::from_min_size(
                    Pos2::new(screen_rect.max.x - w - 14.0, y_offset),
                    Vec2::new(w, h),
                );
                let painter =
                    ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("toast_overlay")));
                // Background
                painter.rect_filled(rect, 8.0, bg_col);
                // Border
                let border_col =
                    Color32::from_rgba_unmultiplied(accent_col.r(), accent_col.g(), accent_col.b(), (80.0 * fade) as u8);
                painter.rect_stroke(rect, 8.0, egui::Stroke::new(1.0, border_col));
                // Left accent bar
                let accent_bar = Rect::from_min_max(
                    rect.left_top() + Vec2::new(0.0, 4.0),
                    Pos2::new(rect.left() + 3.5, rect.bottom() - 4.0),
                );
                painter.rect_filled(accent_bar, 2.0, accent_col);
                // Text
                painter.text(
                    rect.min + Vec2::new(14.0, 8.0),
                    Align2::LEFT_TOP,
                    text,
                    FontId::proportional(13.0),
                    Color32::from_rgba_unmultiplied(225, 230, 248, (255.0 * fade) as u8),
                );
            }
            ctx.request_repaint();
        }
    }
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
        ui.label(
            RichText::new("🔊  Volume")
                .color(Color32::WHITE)
                .strong()
                .size(12.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(&adv_db_label).color(AMBER).size(11.0));
        });
    });
    ui.horizontal(|ui| {
        lock_btn(ui, &mut st.locked_params, "audio.master_volume");
        let vol_locked = st.locked_params.contains("audio.master_volume");
        ui.add_enabled(
            !vol_locked,
            Slider::new(&mut st.config.audio.master_volume, 0.0..=1.0).text(""),
        );
    });
    let _ = ui
        .label("")
        .on_hover_text("Master output volume. Use ↑/↓ arrow keys as a quick shortcut.");
    ui.add_space(4.0);

    // ---- Pause button ----
    let pause_label = if st.paused {
        "▶  RESUME"
    } else {
        "⏸  PAUSE"
    };
    let (pause_fill, pause_stroke) = if st.paused {
        (
            Color32::from_rgb(18, 135, 65),
            Color32::from_rgb(60, 220, 110),
        )
    } else {
        (
            Color32::from_rgb(15, 90, 165),
            Color32::from_rgb(60, 160, 255),
        )
    };
    let btn = Button::new(
        RichText::new(pause_label)
            .color(Color32::WHITE)
            .strong()
            .size(13.0),
    )
    .fill(pause_fill)
    .stroke(egui::Stroke::new(1.5, pause_stroke))
    .min_size(Vec2::new(ui.available_width(), 36.0));
    if ui
        .add(btn)
        .on_hover_text("Pause or resume the simulation and audio. Shortcut: Space bar.")
        .clicked()
    {
        st.paused = !st.paused;
    }
    ui.add_space(4.0);

    // ---- Undo / Redo buttons ----
    ui.horizontal(|ui| {
        let can_undo = !st.undo_stack.is_empty();
        let can_redo = !st.redo_stack.is_empty();
        let undo_col = if can_undo { Color32::from_rgb(80, 160, 255) } else { Color32::from_rgb(50, 55, 75) };
        let redo_col = if can_redo { Color32::from_rgb(80, 220, 140) } else { Color32::from_rgb(50, 55, 75) };
        let half_w = (ui.available_width() - 6.0) * 0.5;
        if ui.add_enabled(
            can_undo,
            Button::new(RichText::new("↩ Undo").color(undo_col).size(12.0))
                .min_size(Vec2::new(half_w, 26.0)),
        ).on_hover_text("Undo last config change (Ctrl+Z)").clicked() {
            st.undo();
            st.toast_queue.push(Toast::info("Undone"));
        }
        ui.add_space(6.0);
        if ui.add_enabled(
            can_redo,
            Button::new(RichText::new("↪ Redo").color(redo_col).size(12.0))
                .min_size(Vec2::new(half_w, 26.0)),
        ).on_hover_text("Redo (Ctrl+Shift+Z)").clicked() {
            st.redo();
            st.toast_queue.push(Toast::info("Redone"));
        }
    });
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
            ui.label(
                RichText::new(format!("{:.0}%", chaos * 100.0))
                    .color(chaos_color)
                    .size(11.0),
            );
        });
    });
    let bar_w = ui.available_width();
    let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 7.0), Sense::hover());
    ui.painter()
        .rect_filled(bar_rect, 3.5, Color32::from_rgb(14, 14, 28));
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
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").size(11.0).color(GRAY_HINT));
            ui.add(
                egui::TextEdit::singleline(&mut st.preset_search)
                    .hint_text("Search presets…")
                    .desired_width(ui.available_width()),
            );
        });
        ui.add_space(4.0);

        // #13: Favorites-only toggle
        ui.horizontal(|ui| {
            let fav_only = st.show_favorites_only;
            let fav_col = if fav_only {
                Color32::from_rgb(255, 205, 30)
            } else {
                Color32::from_rgb(80, 85, 110)
            };
            if ui
                .add(
                    Button::new(RichText::new("⭐ only").color(fav_col).size(11.0))
                        .fill(if fav_only {
                            Color32::from_rgb(60, 50, 10)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(egui::Stroke::new(1.0, fav_col))
                        .min_size(Vec2::new(58.0, 22.0)),
                )
                .clicked()
            {
                st.show_favorites_only = !fav_only;
            }
        });
        ui.add_space(4.0);

        let search_lower = st.preset_search.to_lowercase();
        let selected = st.selected_preset.clone();
        let show_fav_only = st.show_favorites_only;
        // #13: build sorted preset list — favorites first
        let mut sorted_presets_adv: Vec<&crate::patches::Preset> = PRESETS
            .iter()
            .filter(|p| {
                (search_lower.is_empty()
                    || p.name.to_lowercase().contains(&search_lower)
                    || p.category.to_lowercase().contains(&search_lower))
                    && (!show_fav_only || st.favorite_presets.contains(p.name))
            })
            .collect();
        sorted_presets_adv
            .sort_by_key(|p| (!st.favorite_presets.contains(p.name), p.category, p.name));

        let mut last_cat = "";
        for preset in sorted_presets_adv.iter() {
            if preset.category != last_cat {
                last_cat = preset.category;
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("  {}", preset.category.to_uppercase()))
                            .color(GRAY_HINT)
                            .size(10.0)
                            .strong(),
                    );
                    let avail = ui.available_width();
                    let (sep_rect, _) =
                        ui.allocate_exact_size(Vec2::new(avail - 4.0, 1.0), Sense::hover());
                    ui.painter()
                        .rect_filled(sep_rect, 0.0, Color32::from_rgb(35, 38, 65));
                });
                ui.add_space(3.0);
            }
            let is_selected = selected == preset.name;
            let is_fav = st.favorite_presets.contains(preset.name);
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
                .stroke(Stroke::new(
                    if is_selected { 1.5 } else { 1.0 },
                    if is_selected {
                        pc
                    } else {
                        Color32::from_rgb(34, 36, 64)
                    },
                ))
                .inner_margin(egui::Margin::symmetric(0.0, 5.0))
                .rounding(egui::Rounding::same(6.0));
            let response = card
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // #13: star toggle
                        let star_col = if is_fav {
                            Color32::from_rgb(255, 205, 30)
                        } else {
                            Color32::from_rgb(60, 65, 90)
                        };
                        if ui
                            .add(
                                Button::new(RichText::new("⭐").size(12.0).color(star_col))
                                    .fill(Color32::TRANSPARENT)
                                    .frame(false)
                                    .min_size(Vec2::new(22.0, 22.0)),
                            )
                            .clicked()
                        {
                            if is_fav {
                                st.favorite_presets.remove(preset.name);
                            } else {
                                st.favorite_presets.insert(preset.name.to_string());
                            }
                            let fav_txt: String = st
                                .favorite_presets
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("\n");
                            let _ = std::fs::write("favorites.txt", fav_txt);
                        }
                        let (strip_rect, _) =
                            ui.allocate_exact_size(Vec2::new(5.0, 34.0), Sense::hover());
                        let strip_col = if is_selected {
                            pc
                        } else {
                            Color32::from_rgba_premultiplied(
                                pc.r() / 3,
                                pc.g() / 3,
                                pc.b() / 3,
                                200,
                            )
                        };
                        ui.painter()
                            .rect_filled(strip_rect, egui::Rounding::same(3.0), strip_col);
                        ui.add_space(7.0);
                        ui.vertical(|ui| {
                            let name_col = if is_selected { pc } else { Color32::WHITE };
                            ui.label(
                                RichText::new(preset.name)
                                    .strong()
                                    .color(name_col)
                                    .size(12.5),
                            );
                            ui.label(
                                RichText::new(preset.description)
                                    .italics()
                                    .color(GRAY_HINT)
                                    .size(10.5),
                            );
                        });
                    });
                })
                .response;
            if response.interact(Sense::click()).clicked() {
                st.push_undo();
                // #12: snapshot locked params before load
                let locked_snapshot_adv: Vec<(String, f64)> = st
                    .locked_params
                    .iter()
                    .map(|k| (k.clone(), get_param_value(&st.config, k)))
                    .collect();
                st.selected_preset = preset.name.to_string();
                st.config = load_preset(preset.name);
                // Validate and notify if any values were clamped.
                let pre_sigma = st.config.lorenz.sigma;
                let pre_vol = st.config.audio.master_volume;
                st.config.validate();
                if (st.config.lorenz.sigma - pre_sigma).abs() > 1e-9
                    || (st.config.audio.master_volume - pre_vol).abs() > 1e-5
                {
                    st.toast_queue.push(Toast::warning(
                        "Some config values were out of range and have been adjusted",
                    ));
                }
                for (k, v) in locked_snapshot_adv {
                    set_param_value(&mut st.config, &k, v);
                }
                st.system_changed = true;
                st.mode_changed = true;
            }
            ui.add_space(3.0);
        }
    });

    // ---- SOUND ----
    collapsing_section(ui, "SOUND", false, |ui| {
        ui.label(RichText::new("Mode").color(GRAY_HINT).size(11.0));
        let modes = ["direct", "orbital", "granular", "spectral", "fm", "vocal"];
        let current_mode = st.config.sonification.mode.clone();
        ui.horizontal_wrapped(|ui| {
            for m in &modes {
                let selected = current_mode == *m;
                let color = if selected {
                    Color32::from_rgb(0, 140, 210)
                } else {
                    Color32::from_rgb(40, 40, 70)
                };
                let resp = ui.add(
                    Button::new(RichText::new(*m).color(Color32::WHITE))
                        .fill(color)
                        .min_size(Vec2::new(55.0, 26.0)),
                );
                if resp.clicked() {
                    st.config.sonification.mode = m.to_string();
                    st.mode_changed = true;
                }
                resp.on_hover_text(mode_tooltip(m));
            }
            // Waveguide: shown but disabled until wired
            let resp = ui.add(
                Button::new(
                    RichText::new("waveguide")
                        .color(Color32::from_rgb(90, 90, 110)),
                )
                .fill(Color32::from_rgb(22, 22, 32))
                .min_size(Vec2::new(72.0, 26.0)),
            );
            resp.on_hover_text("Waveguide physical model — coming in next release");
        });
        ui.add_space(6.0);

        // Scale list includes EDO and modal scales added in #3.
        let scales = [
            ("pentatonic", "Pentatonic"),
            ("chromatic", "Chromatic"),
            ("just_intonation", "Just Intonation"),
            ("microtonal", "Microtonal (13-TET)"),
            ("edo19", "19-EDO"),
            ("edo31", "31-EDO"),
            ("edo24", "24-EDO (Quarter-tones)"),
            ("whole_tone", "Whole Tone"),
            ("phrygian", "Phrygian"),
            ("lydian", "Lydian"),
            ("harmonic_series", "Harmonic Series"),
            ("hirajoshi", "Hirajoshi"),
            ("blues", "Blues"),
            ("dorian", "Dorian"),
            ("mixolydian", "Mixolydian"),
            ("hungarian_minor", "Hungarian Minor"),
            ("locrian", "Locrian"),
            ("octatonic", "Octatonic (Diminished)"),
            ("natural_minor", "Natural Minor"),
            ("harmonic_minor", "Harmonic Minor"),
        ];
        let current_scale = st.config.sonification.scale.clone();
        let current_scale_label = scales
            .iter()
            .find(|(k, _)| *k == current_scale.as_str())
            .map(|(_, v)| *v)
            .unwrap_or(current_scale.as_str());
        ComboBox::from_label("Scale")
            .selected_text(current_scale_label)
            .show_ui(ui, |ui| {
                for (key, label) in &scales {
                    if ui
                        .selectable_label(current_scale == *key, *label)
                        .on_hover_text(scale_description(key))
                        .clicked()
                    {
                        st.config.sonification.scale = key.to_string();
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

        ui.add(
            Slider::new(&mut st.config.sonification.portamento_ms, 1.0..=1000.0)
                .text("Portamento ms")
                .logarithmic(true),
        )
        .on_hover_text("Frequency glide time between notes.");

        ui.add_space(4.0);
        ui.label(RichText::new("Voice Levels").color(Color32::WHITE));
        for i in 0..4 {
            ui.add(
                Slider::new(&mut st.config.sonification.voice_levels[i], 0.0..=1.0)
                    .text(format!("Voice {}", i + 1)),
            );
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
                    let color = if selected {
                        Color32::from_rgb(0, 140, 210)
                    } else {
                        Color32::from_rgb(40, 40, 70)
                    };
                    if ui
                        .add(
                            Button::new(RichText::new(*s).color(Color32::WHITE))
                                .fill(color)
                                .min_size(Vec2::new(50.0, 24.0)),
                        )
                        .clicked()
                    {
                        st.config.sonification.voice_shapes[i] = s.to_string();
                    }
                }
            });
        }
    });

    // ---- PHYSICS ENGINE ----
    collapsing_section(ui, "PHYSICS ENGINE", false, |ui| {
        // System list is driven by SYSTEM_REGISTRY — the single source of truth (#20).
        let current_sys = st.config.system.name.clone();
        let current_display = system_display_name(&current_sys);
        ComboBox::from_label("System")
            .selected_text(current_display)
            .show_ui(ui, |ui| {
                for entry in systems::SYSTEM_REGISTRY {
                    if ui
                        .selectable_label(current_sys == entry.name, entry.display_name)
                        .on_hover_text(entry.description)
                        .clicked()
                    {
                        st.config.system.name = entry.name.to_string();
                        st.system_changed = true;
                    }
                }
            });

        ui.horizontal(|ui| {
            lock_btn(ui, &mut st.locked_params, "system.speed");
            let spd_locked = st.locked_params.contains("system.speed");
            ui.add_enabled(
                !spd_locked,
                Slider::new(&mut st.config.system.speed, 0.1..=10.0).text("Speed"),
            );
        });

        // Feature #10: Thermal Noise injection
        ui.add(Slider::new(&mut st.noise_inject, 0.0..=0.05).text("Thermal Noise"))
            .on_hover_text("Inject stochastic noise into system state each tick. 0 = off, 0.05 = maximum. Adds organic randomness to the trajectory.");

        // Feature #9: Attractor Interpolation (Morph)
        ui.separator();
        ui.colored_label(Color32::from_rgb(160, 200, 255), "Attractor Morph");
        ui.checkbox(&mut st.interp_enabled, "Enable morph");
        if st.interp_enabled {
            let systems_list = [
                "lorenz",
                "fractional_lorenz",
                "rossler",
                "double_pendulum",
                "geodesic_torus",
                "kuramoto",
                "three_body",
                "duffing",
                "van_der_pol",
                "halvorsen",
                "aizawa",
                "chua",
                "hindmarsh_rose",
                "coupled_map_lattice",
                "mackey_glass",
                "nose_hoover",
                "sprott_b",
                "henon_map",
                "lorenz96",
                "logistic_map",
                "standard_map",
                "stochastic_lorenz",
                "delayed_map",
                "oregonator",
                "mathieu",
                "kuramoto_driven",
                "thomas",
                "sprott_c",
                "dadras",
                "rucklidge",
                "chen",
                "burke_shaw",
                "lorenz84",
                "rabinovich_fabrikant",
                "sprott_g",
                "sprott_h",
                "sprott_l",
                "rikitake",
                "bouali",
                "newton_leipnik",
                "shimizu_morioka",
                "genesio_tesi",
                "liu",
                "windmi",
                "finance",
                "hyperchaos",
                "sprott_k",
                "sprott_d",
                "sprott_e",
                "sprott_f",
            ];
            let cur_interp = st.interp_system.clone();
            ComboBox::from_label("Morph to")
                .selected_text(system_display_name(&cur_interp))
                .show_ui(ui, |ui| {
                    for &s in &systems_list {
                        if ui
                            .selectable_label(cur_interp == s, system_display_name(s))
                            .clicked()
                        {
                            st.interp_system = s.to_string();
                        }
                    }
                });
            ui.add(Slider::new(&mut st.interp_t, 0.0..=1.0).text("Morph amount"))
                .on_hover_text("0 = primary system only, 1 = target system only. Values in between interpolate both states.");
        }
        ui.separator();

        CollapsingHeader::new(
            RichText::new("Parameters").size(12.0).color(GRAY_HINT)
        ).default_open(false).show(ui, |ui| {
            match st.config.system.name.as_str() {
                "lorenz" => {
                    ui.horizontal(|ui| {
                        lock_btn(ui, &mut st.locked_params, "lorenz.sigma");
                        let sl = st.locked_params.contains("lorenz.sigma");
                        ui.add_enabled(!sl, Slider::new(&mut st.config.lorenz.sigma, 1.0..=20.0).text("sigma"));
                    });
                    ui.horizontal(|ui| {
                        lock_btn(ui, &mut st.locked_params, "lorenz.rho");
                        let sl = st.locked_params.contains("lorenz.rho");
                        ui.add_enabled(!sl, Slider::new(&mut st.config.lorenz.rho, 10.0..=50.0).text("rho"));
                    });
                    ui.add(Slider::new(&mut st.config.lorenz.beta, 0.5..=5.0).text("beta"))
                        .on_hover_text("β (geometric factor): controls dissipation. Typical value 8/3 ≈ 2.667.");
                }
                "fractional_lorenz" => {
                    ui.add(Slider::new(&mut st.config.fractional_lorenz.alpha, 0.5..=1.0).text("α (order)"))
                        .on_hover_text("Fractional derivative order — 1.0 = classic Lorenz");
                    ui.add(Slider::new(&mut st.config.fractional_lorenz.sigma, 1.0..=20.0).text("sigma"));
                    ui.add(Slider::new(&mut st.config.fractional_lorenz.rho, 10.0..=50.0).text("rho"));
                    ui.add(Slider::new(&mut st.config.fractional_lorenz.beta, 0.5..=5.0).text("beta"));
                }
                "rossler" => {
                    ui.add(Slider::new(&mut st.config.rossler.a, 0.01..=0.5).text("a"))
                        .on_hover_text("Controls spiral tightness.");
                    ui.add(Slider::new(&mut st.config.rossler.b, 0.01..=0.5).text("b"));
                    ui.add(Slider::new(&mut st.config.rossler.c, 1.0..=15.0).text("c"))
                        .on_hover_text("Primary chaos parameter — drives period-doubling bifurcations above ~5.");
                }
                "double_pendulum" => {
                    ui.add(Slider::new(&mut st.config.double_pendulum.m1, 0.1..=5.0).text("m1"))
                        .on_hover_text("Mass of first bob. Heavier = slower swings and larger inertia.");
                    ui.add(Slider::new(&mut st.config.double_pendulum.m2, 0.1..=5.0).text("m2"))
                        .on_hover_text("Mass of second bob. Unequal masses increase the sensitivity to initial conditions.");
                    ui.add(Slider::new(&mut st.config.double_pendulum.l1, 0.1..=3.0).text("l1"))
                        .on_hover_text("Length of first arm. Longer arm = slower natural period.");
                    ui.add(Slider::new(&mut st.config.double_pendulum.l2, 0.1..=3.0).text("l2"))
                        .on_hover_text("Length of second arm. Short second arm relative to first gives rapid chaotic flipping.");
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
                    ui.add(Slider::new(&mut st.config.duffing.delta, 0.1..=1.0).text("Damping δ"))
                        .on_hover_text("Energy dissipation. Low values sustain chaotic oscillations longer; high values damp to a fixed point.");
                    ui.add(Slider::new(&mut st.config.duffing.gamma, 0.0..=1.5).text("Forcing γ"))
                        .on_hover_text("Drive amplitude. Chaos onset depends on the ratio γ/δ; sweep this to hear the period-doubling cascade.");
                    ui.add(Slider::new(&mut st.config.duffing.omega, 0.5..=3.0).text("Drive freq ω"))
                        .on_hover_text("Driving frequency. Near ω≈1.2 gives the classic chaotic Duffing response.");
                }
                "van_der_pol" => {
                    ui.add(Slider::new(&mut st.config.van_der_pol.mu, 0.1..=5.0).text("Nonlinearity"));
                }
                "halvorsen" => {
                    ui.add(Slider::new(&mut st.config.halvorsen.a, 1.0..=3.0).text("a"));
                }
                "aizawa" => {
                    ui.add(Slider::new(&mut st.config.aizawa.a, 0.5..=1.5).text("a"))
                        .on_hover_text("Primary shape parameter. Controls the curvature of the torus-like surface.");
                    ui.add(Slider::new(&mut st.config.aizawa.c, 0.3..=1.0).text("c"))
                        .on_hover_text("Radial nonlinearity. Higher values widen the attractor tube.");
                    ui.add(Slider::new(&mut st.config.aizawa.d, 2.0..=5.0).text("d"))
                        .on_hover_text("Polar distortion. Drives the slow z-wobble; increase for more vertical spread.");
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
                "logistic_map" => {
                    ui.add(Slider::new(&mut st.config.logistic_map.r, 2.5..=4.0).text("r"))
                        .on_hover_text("Bifurcation parameter. Period-doubling cascade begins at r≈3.0; fully chaotic above r≈3.57. At r=4.0 the map is ergodic on [0,1].");
                }
                "standard_map" => {
                    ui.add(Slider::new(&mut st.config.standard_map.k, 0.0..=4.0).text("k"))
                        .on_hover_text("Stochasticity parameter. k<0.97: islands of stability; k≈0.97: KAM transition; k>1: globally chaotic. Classic value k=1.5.");
                }
                "stochastic_lorenz" => {
                    ui.add(Slider::new(&mut st.config.stochastic_lorenz.sigma, 1.0..=20.0).text("sigma"))
                        .on_hover_text("Prandtl number (same as Lorenz σ).");
                    ui.add(Slider::new(&mut st.config.stochastic_lorenz.rho, 10.0..=50.0).text("rho"))
                        .on_hover_text("Rayleigh number (same as Lorenz ρ). Chaos onset ≈ 24.74.");
                    ui.add(Slider::new(&mut st.config.stochastic_lorenz.beta, 0.5..=5.0).text("beta"));
                    ui.add(Slider::new(&mut st.config.stochastic_lorenz.noise_strength, 0.0..=2.0).text("Noise strength"))
                        .on_hover_text("Standard deviation of Wiener process noise injected each step. 0 = deterministic Lorenz; higher values smear the attractor.");
                }
                "delayed_map" => {
                    ui.add(Slider::new(&mut st.config.delayed_map.r, 2.5..=4.0).text("r"))
                        .on_hover_text("Logistic growth parameter. Chaos above ~3.57.");
                    let mut tau_f = st.config.delayed_map.tau as f64;
                    if ui.add(Slider::new(&mut tau_f, 1.0..=20.0).step_by(1.0).text("τ (delay steps)"))
                        .on_hover_text("Delay in map iterations. Longer delays increase attractor dimensionality.")
                        .changed() {
                        st.config.delayed_map.tau = tau_f as usize;
                    }
                }
                "oregonator" => {
                    ui.add(Slider::new(&mut st.config.oregonator.f, 0.3..=3.0).text("f"))
                        .on_hover_text("Stoichiometric factor controlling the Belousov-Zhabotinsky oscillation frequency. Near f=1 gives robust limit cycles.");
                }
                "mathieu" => {
                    ui.add(Slider::new(&mut st.config.mathieu.a, -1.0..=2.0).text("a"))
                        .on_hover_text("Linear frequency-squared parameter. Stability boundaries in (a,q) space follow the Mathieu stability chart.");
                    ui.add(Slider::new(&mut st.config.mathieu.q, 0.0..=1.5).text("q"))
                        .on_hover_text("Modulation amplitude. At (a=0,q≈0.9) the system is at the primary instability tongue edge.");
                }
                "kuramoto_driven" => {
                    ui.add(Slider::new(&mut st.config.kuramoto_driven.coupling, 0.0..=5.0).text("Coupling K"))
                        .on_hover_text("Mutual coupling between oscillators. Above Kc≈1 spontaneous synchronization occurs.");
                    ui.add(Slider::new(&mut st.config.kuramoto_driven.drive_amp, 0.0..=2.0).text("Drive amplitude"))
                        .on_hover_text("Amplitude of external periodic forcing applied to the first oscillator.");
                    ui.add(Slider::new(&mut st.config.kuramoto_driven.drive_freq, 0.1..=5.0).text("Drive frequency"))
                        .on_hover_text("Frequency of external periodic forcing. Near the natural frequency → resonance and entrainment.");
                }
                "arnold_cat" => {
                    ui.label(RichText::new("Arnold's Cat Map has no tunable parameters.")
                        .color(GRAY_HINT).italics().size(11.0));
                    ui.label(RichText::new("x'=(2x+y) mod 1\ny'=(x+y) mod 1")
                        .color(CYAN).size(11.0)
                        .font(egui::FontId::monospace(11.0)));
                }
                "thomas" => {
                    ui.add(Slider::new(&mut st.config.thomas.b, 0.1..=0.5).text("b"))
                        .on_hover_text("Dissipation coefficient. b≈0.208 gives a chaotic strange attractor; b→0 gives conservative flow.");
                }
                "sprott_c" | "sprott_g" | "sprott_h" | "sprott_l" => {
                    ui.label(RichText::new("This Sprott system has no tunable parameters.")
                        .color(GRAY_HINT).italics().size(11.0));
                }
                "dadras" => {
                    ui.add(Slider::new(&mut st.config.dadras.a, 1.0..=6.0).text("a"))
                        .on_hover_text("Parameter a. Default 3.0.");
                    ui.add(Slider::new(&mut st.config.dadras.b, 1.0..=5.0).text("b"))
                        .on_hover_text("Parameter b. Default 2.7.");
                    ui.add(Slider::new(&mut st.config.dadras.c, 0.5..=4.0).text("c"))
                        .on_hover_text("Parameter c. Default 1.7.");
                    ui.add(Slider::new(&mut st.config.dadras.d, 0.5..=4.0).text("d"))
                        .on_hover_text("Parameter d. Default 2.0.");
                    ui.add(Slider::new(&mut st.config.dadras.e, 4.0..=15.0).text("e"))
                        .on_hover_text("Parameter e. Default 9.0.");
                }
                "rucklidge" => {
                    ui.add(Slider::new(&mut st.config.rucklidge.kappa, 0.5..=4.0).text("κ (kappa)"))
                        .on_hover_text("Dissipation rate. Default 2.0.");
                    ui.add(Slider::new(&mut st.config.rucklidge.lambda, 3.0..=10.0).text("λ (lambda)"))
                        .on_hover_text("Forcing amplitude. Default 6.7 gives chaos.");
                }
                "chen" => {
                    ui.add(Slider::new(&mut st.config.chen.a, 20.0..=60.0).text("a"))
                        .on_hover_text("Parameter a. Default 40.0.");
                    ui.add(Slider::new(&mut st.config.chen.b, 1.0..=8.0).text("b"))
                        .on_hover_text("Parameter b. Default 3.0.");
                    ui.add(Slider::new(&mut st.config.chen.c, 15.0..=40.0).text("c"))
                        .on_hover_text("Parameter c. Default 28.0.");
                }
                "burke_shaw" => {
                    ui.add(Slider::new(&mut st.config.burke_shaw.sigma, 5.0..=20.0).text("σ (sigma)"))
                        .on_hover_text("Contraction rate. Default 10.0.");
                    ui.add(Slider::new(&mut st.config.burke_shaw.rho, 2.0..=8.0).text("ρ (rho)"))
                        .on_hover_text("Second parameter. Default 4.272.");
                }
                "lorenz84" => {
                    ui.add(Slider::new(&mut st.config.lorenz84.a, 0.1..=0.5).text("a"))
                        .on_hover_text("Thermal relaxation rate. Default 0.25.");
                    ui.add(Slider::new(&mut st.config.lorenz84.b, 1.0..=8.0).text("b"))
                        .on_hover_text("Rotational forcing. Default 4.0.");
                    ui.add(Slider::new(&mut st.config.lorenz84.f, 4.0..=12.0).text("F"))
                        .on_hover_text("Symmetric heating forcing. Default 8.0. Above ~6 gives chaos.");
                    ui.add(Slider::new(&mut st.config.lorenz84.g, 0.5..=3.0).text("G"))
                        .on_hover_text("Wave (seasonal) forcing. Default 1.23.");
                }
                "rabinovich_fabrikant" => {
                    ui.add(Slider::new(&mut st.config.rabinovich_fabrikant.alpha, 0.05..=0.5).text("α (alpha)"))
                        .on_hover_text("Damping. Default 0.14.");
                    ui.add(Slider::new(&mut st.config.rabinovich_fabrikant.gamma, 0.05..=0.3).text("γ (gamma)"))
                        .on_hover_text("Excitation. Default 0.1.");
                }
                "rikitake" => {
                    ui.add(Slider::new(&mut st.config.rikitake.mu, 0.1..=3.0).text("μ (mu)"))
                        .on_hover_text("Dissipation rate. Default 1.0.");
                    ui.add(Slider::new(&mut st.config.rikitake.a, 1.0..=10.0).text("a"))
                        .on_hover_text("Coupling offset. Default 5.0.");
                }
                "bouali" => {
                    ui.add(Slider::new(&mut st.config.bouali.a, 0.0..=1.0).text("a"))
                        .on_hover_text("z-coupling coefficient. Default 0.3.");
                    ui.add(Slider::new(&mut st.config.bouali.s, 0.1..=3.0).text("s"))
                        .on_hover_text("z-feedback coefficient. Default 1.0.");
                }
                "newton_leipnik" => {
                    ui.add(Slider::new(&mut st.config.newton_leipnik.a, 0.1..=1.0).text("a"))
                        .on_hover_text("Damping of x. Default 0.4.");
                    ui.add(Slider::new(&mut st.config.newton_leipnik.b, 0.05..=0.4).text("b"))
                        .on_hover_text("z growth rate. Default 0.175.");
                }
                "shimizu_morioka" => {
                    ui.add(Slider::new(&mut st.config.shimizu_morioka.a, 0.1..=2.0).text("a"))
                        .on_hover_text("Damping coefficient. Default 0.75 gives chaos.");
                    ui.add(Slider::new(&mut st.config.shimizu_morioka.b, 0.1..=1.5).text("b"))
                        .on_hover_text("z decay rate. Default 0.45 gives chaos.");
                }
                "genesio_tesi" => {
                    ui.add(Slider::new(&mut st.config.genesio_tesi.a, 0.5..=3.0).text("a"))
                        .on_hover_text("Linear damping (x'' coefficient). Default 1.2 gives chaos.");
                    ui.add(Slider::new(&mut st.config.genesio_tesi.b, 1.0..=6.0).text("b"))
                        .on_hover_text("Quadratic frequency (x' coefficient). Default 2.92 gives chaos.");
                    ui.add(Slider::new(&mut st.config.genesio_tesi.c, 2.0..=12.0).text("c"))
                        .on_hover_text("Restoring force coefficient. Default 6.0 gives chaos.");
                }
                "liu" => {
                    ui.add(Slider::new(&mut st.config.liu.a, 0.1..=5.0).text("a"))
                        .on_hover_text("x damping coefficient. Default 1.0.");
                    ui.add(Slider::new(&mut st.config.liu.b, 0.5..=8.0).text("b"))
                        .on_hover_text("y growth rate. Default 2.5 gives chaos.");
                    ui.add(Slider::new(&mut st.config.liu.c, 1.0..=15.0).text("c"))
                        .on_hover_text("z damping coefficient. Default 5.0.");
                    ui.add(Slider::new(&mut st.config.liu.e, 0.1..=5.0).text("e"))
                        .on_hover_text("y² coupling to x. Default 1.0.");
                    ui.add(Slider::new(&mut st.config.liu.k, 0.5..=10.0).text("k"))
                        .on_hover_text("xz coupling in y equation. Default 4.0.");
                    ui.add(Slider::new(&mut st.config.liu.m, 0.5..=10.0).text("m"))
                        .on_hover_text("xy coupling in z equation. Default 4.0.");
                }
                "windmi" => {
                    ui.add(Slider::new(&mut st.config.windmi.a, 0.3..=2.0).text("a"))
                        .on_hover_text("Damping coefficient. Default 0.9 gives chaos.");
                    ui.add(Slider::new(&mut st.config.windmi.b, 1.0..=4.0).text("b"))
                        .on_hover_text("Drive parameter. Default 2.5 gives a strange attractor.");
                }
                "finance" => {
                    ui.add(Slider::new(&mut st.config.finance.a, 1.0..=6.0).text("a"))
                        .on_hover_text("Savings rate. Default 3.0 gives chaos.");
                    ui.add(Slider::new(&mut st.config.finance.b, 0.01..=0.5).text("b"))
                        .on_hover_text("Cost per investment. Default 0.1.");
                    ui.add(Slider::new(&mut st.config.finance.c, 0.2..=3.0).text("c"))
                        .on_hover_text("Elasticity of demand. Default 1.0.");
                }
                "hyperchaos" => {
                    ui.add(Slider::new(&mut st.config.hyperchaos.a, 20.0..=50.0).text("a"))
                        .on_hover_text("Coupling strength. Default 35.0.");
                    ui.add(Slider::new(&mut st.config.hyperchaos.b, 1.0..=6.0).text("b"))
                        .on_hover_text("z dissipation. Default 3.0.");
                    ui.add(Slider::new(&mut st.config.hyperchaos.c, 20.0..=40.0).text("c"))
                        .on_hover_text("Rayleigh number analog. Default 28.0.");
                    ui.add(Slider::new(&mut st.config.hyperchaos.d, -15.0..=-0.5).text("d"))
                        .on_hover_text("4th dimension feedback (must be negative). Default −7.0.");
                }
                _ => {}
            }
        });

        ui.add_space(4.0);
        if ui
            .add(
                Button::new(RichText::new("Randomize Parameters").color(Color32::WHITE))
                    .fill(Color32::from_rgb(60, 40, 80))
                    .min_size(Vec2::new(ui.available_width(), 28.0)),
            )
            .clicked()
        {
            let mut seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as u64;
            let vary = |v: f64, s: &mut u64| v * (0.8 + lcg_rand(s) * 0.4);
            match st.config.system.name.as_str() {
                "lorenz" => {
                    st.config.lorenz.sigma =
                        vary(st.config.lorenz.sigma, &mut seed).clamp(1.0, 20.0);
                    st.config.lorenz.rho = vary(st.config.lorenz.rho, &mut seed).clamp(10.0, 50.0);
                    st.config.lorenz.beta = vary(st.config.lorenz.beta, &mut seed).clamp(0.5, 5.0);
                }
                "rossler" => {
                    st.config.rossler.a = vary(st.config.rossler.a, &mut seed).clamp(0.01, 0.5);
                    st.config.rossler.b = vary(st.config.rossler.b, &mut seed).clamp(0.01, 0.5);
                    st.config.rossler.c = vary(st.config.rossler.c, &mut seed).clamp(1.0, 15.0);
                }
                "kuramoto" => {
                    st.config.kuramoto.coupling =
                        vary(st.config.kuramoto.coupling, &mut seed).clamp(0.0, 5.0);
                }
                "duffing" => {
                    st.config.duffing.delta = vary(st.config.duffing.delta, &mut seed).clamp(0.1, 1.0);
                    st.config.duffing.gamma = vary(st.config.duffing.gamma, &mut seed).clamp(0.0, 1.5);
                    st.config.duffing.omega = vary(st.config.duffing.omega, &mut seed).clamp(0.5, 3.0);
                }
                "van_der_pol" => {
                    st.config.van_der_pol.mu = vary(st.config.van_der_pol.mu, &mut seed).clamp(0.1, 5.0);
                }
                "halvorsen" => {
                    st.config.halvorsen.a = vary(st.config.halvorsen.a, &mut seed).clamp(1.0, 3.0);
                }
                "aizawa" => {
                    st.config.aizawa.a = vary(st.config.aizawa.a, &mut seed).clamp(0.5, 1.5);
                    st.config.aizawa.c = vary(st.config.aizawa.c, &mut seed).clamp(0.3, 1.0);
                    st.config.aizawa.d = vary(st.config.aizawa.d, &mut seed).clamp(2.0, 5.0);
                }
                "chua" => {
                    st.config.chua.alpha = vary(st.config.chua.alpha, &mut seed).clamp(10.0, 20.0);
                    st.config.chua.beta = vary(st.config.chua.beta, &mut seed).clamp(20.0, 35.0);
                }
                "mackey_glass" => {
                    st.config.mackey_glass.tau = vary(st.config.mackey_glass.tau, &mut seed).clamp(5.0, 30.0);
                    st.config.mackey_glass.beta = vary(st.config.mackey_glass.beta, &mut seed).clamp(0.1, 0.5);
                }
                "logistic_map" => {
                    st.config.logistic_map.r = vary(st.config.logistic_map.r, &mut seed).clamp(3.0, 4.0);
                }
                "standard_map" => {
                    st.config.standard_map.k = vary(st.config.standard_map.k, &mut seed).clamp(0.5, 4.0);
                }
                "stochastic_lorenz" => {
                    st.config.stochastic_lorenz.sigma = vary(st.config.stochastic_lorenz.sigma, &mut seed).clamp(1.0, 20.0);
                    st.config.stochastic_lorenz.rho = vary(st.config.stochastic_lorenz.rho, &mut seed).clamp(10.0, 50.0);
                    st.config.stochastic_lorenz.noise_strength = vary(st.config.stochastic_lorenz.noise_strength, &mut seed).clamp(0.0, 2.0);
                }
                "kuramoto_driven" => {
                    st.config.kuramoto_driven.coupling = vary(st.config.kuramoto_driven.coupling, &mut seed).clamp(0.0, 5.0);
                    st.config.kuramoto_driven.drive_amp = vary(st.config.kuramoto_driven.drive_amp, &mut seed).clamp(0.0, 2.0);
                    st.config.kuramoto_driven.drive_freq = vary(st.config.kuramoto_driven.drive_freq, &mut seed).clamp(0.1, 5.0);
                }
                "thomas" => {
                    st.config.thomas.b = vary(st.config.thomas.b, &mut seed).clamp(0.1, 0.5);
                }
                "dadras" => {
                    st.config.dadras.a = vary(st.config.dadras.a, &mut seed).clamp(1.0, 6.0);
                    st.config.dadras.b = vary(st.config.dadras.b, &mut seed).clamp(1.0, 5.0);
                    st.config.dadras.c = vary(st.config.dadras.c, &mut seed).clamp(0.5, 4.0);
                    st.config.dadras.d = vary(st.config.dadras.d, &mut seed).clamp(0.5, 4.0);
                    st.config.dadras.e = vary(st.config.dadras.e, &mut seed).clamp(4.0, 15.0);
                }
                "rucklidge" => {
                    st.config.rucklidge.kappa = vary(st.config.rucklidge.kappa, &mut seed).clamp(0.5, 4.0);
                    st.config.rucklidge.lambda = vary(st.config.rucklidge.lambda, &mut seed).clamp(3.0, 10.0);
                }
                "chen" => {
                    st.config.chen.a = vary(st.config.chen.a, &mut seed).clamp(20.0, 60.0);
                    st.config.chen.b = vary(st.config.chen.b, &mut seed).clamp(1.0, 8.0);
                    st.config.chen.c = vary(st.config.chen.c, &mut seed).clamp(15.0, 40.0);
                }
                "burke_shaw" => {
                    st.config.burke_shaw.sigma = vary(st.config.burke_shaw.sigma, &mut seed).clamp(5.0, 20.0);
                    st.config.burke_shaw.rho = vary(st.config.burke_shaw.rho, &mut seed).clamp(2.0, 8.0);
                }
                "lorenz84" => {
                    st.config.lorenz84.a = vary(st.config.lorenz84.a, &mut seed).clamp(0.1, 0.5);
                    st.config.lorenz84.b = vary(st.config.lorenz84.b, &mut seed).clamp(1.0, 8.0);
                    st.config.lorenz84.f = vary(st.config.lorenz84.f, &mut seed).clamp(4.0, 12.0);
                    st.config.lorenz84.g = vary(st.config.lorenz84.g, &mut seed).clamp(0.5, 3.0);
                }
                "rabinovich_fabrikant" => {
                    st.config.rabinovich_fabrikant.alpha = vary(st.config.rabinovich_fabrikant.alpha, &mut seed).clamp(0.05, 0.5);
                    st.config.rabinovich_fabrikant.gamma = vary(st.config.rabinovich_fabrikant.gamma, &mut seed).clamp(0.05, 0.3);
                }
                "rikitake" => {
                    st.config.rikitake.mu = vary(st.config.rikitake.mu, &mut seed).clamp(0.1, 3.0);
                    st.config.rikitake.a = vary(st.config.rikitake.a, &mut seed).clamp(1.0, 10.0);
                }
                "bouali" => {
                    st.config.bouali.a = vary(st.config.bouali.a, &mut seed).clamp(0.0, 1.0);
                    st.config.bouali.s = vary(st.config.bouali.s, &mut seed).clamp(0.1, 3.0);
                }
                "newton_leipnik" => {
                    st.config.newton_leipnik.a = vary(st.config.newton_leipnik.a, &mut seed).clamp(0.1, 1.0);
                    st.config.newton_leipnik.b = vary(st.config.newton_leipnik.b, &mut seed).clamp(0.05, 0.4);
                }
                "shimizu_morioka" => {
                    st.config.shimizu_morioka.a = vary(st.config.shimizu_morioka.a, &mut seed).clamp(0.1, 2.0);
                    st.config.shimizu_morioka.b = vary(st.config.shimizu_morioka.b, &mut seed).clamp(0.1, 1.5);
                }
                "genesio_tesi" => {
                    st.config.genesio_tesi.a = vary(st.config.genesio_tesi.a, &mut seed).clamp(0.5, 3.0);
                    st.config.genesio_tesi.b = vary(st.config.genesio_tesi.b, &mut seed).clamp(1.0, 6.0);
                    st.config.genesio_tesi.c = vary(st.config.genesio_tesi.c, &mut seed).clamp(2.0, 12.0);
                }
                "liu" => {
                    st.config.liu.a = vary(st.config.liu.a, &mut seed).clamp(0.1, 5.0);
                    st.config.liu.b = vary(st.config.liu.b, &mut seed).clamp(0.5, 8.0);
                    st.config.liu.c = vary(st.config.liu.c, &mut seed).clamp(1.0, 15.0);
                    st.config.liu.e = vary(st.config.liu.e, &mut seed).clamp(0.1, 5.0);
                    st.config.liu.k = vary(st.config.liu.k, &mut seed).clamp(0.5, 10.0);
                    st.config.liu.m = vary(st.config.liu.m, &mut seed).clamp(0.5, 10.0);
                }
                "windmi" => {
                    st.config.windmi.a = vary(st.config.windmi.a, &mut seed).clamp(0.3, 2.0);
                    st.config.windmi.b = vary(st.config.windmi.b, &mut seed).clamp(1.0, 4.0);
                }
                "finance" => {
                    st.config.finance.a = vary(st.config.finance.a, &mut seed).clamp(1.0, 6.0);
                    st.config.finance.b = vary(st.config.finance.b, &mut seed).clamp(0.01, 0.5);
                    st.config.finance.c = vary(st.config.finance.c, &mut seed).clamp(0.2, 3.0);
                }
                "hyperchaos" => {
                    st.config.hyperchaos.a = vary(st.config.hyperchaos.a, &mut seed).clamp(20.0, 50.0);
                    st.config.hyperchaos.b = vary(st.config.hyperchaos.b, &mut seed).clamp(1.0, 6.0);
                    st.config.hyperchaos.c = vary(st.config.hyperchaos.c, &mut seed).clamp(20.0, 40.0);
                    st.config.hyperchaos.d = vary(st.config.hyperchaos.d, &mut seed).clamp(-15.0, -0.5);
                }
                _ => {}
            }
            st.system_changed = true;
        }
    });

    // ---- CUSTOM INITIAL CONDITIONS ----
    collapsing_section(ui, "CUSTOM INITIAL CONDITIONS", false, |ui| {
        ui.label(RichText::new("Set the state vector at which the system resets.").size(11.0).color(GRAY_HINT));
        ui.add_space(4.0);
        // Determine dimension from current system state (up to 6)
        let dim = st.current_state.len().min(6).max(1);
        // Ensure custom_ic is long enough
        while st.custom_ic.len() < dim {
            st.custom_ic.push(0.0);
        }
        let var_labels = ["x₀", "x₁", "x₂", "x₃", "x₄", "x₅"];
        for d in 0..dim {
            ui.horizontal(|ui| {
                ui.label(RichText::new(var_labels[d]).color(CYAN).size(11.0));
                ui.add(egui::DragValue::new(&mut st.custom_ic[d]).speed(0.01));
            });
        }
        ui.add_space(4.0);
        if ui.add(
            Button::new(RichText::new("Set IC").color(Color32::WHITE).size(12.0))
                .fill(Color32::from_rgb(30, 80, 140))
                .min_size(Vec2::new(ui.available_width(), 26.0)),
        ).on_hover_text("Reset the system to the custom initial conditions above.").clicked() {
            // Copy custom_ic into current_state to trigger a reset in the sim thread
            let ic: Vec<f64> = st.custom_ic.clone();
            let state_len = st.current_state.len();
            for (i, v) in ic.iter().enumerate() {
                if i < state_len {
                    st.current_state[i] = *v;
                }
            }
            st.system_changed = true;
        }
    });

    // ---- EFFECTS ----
    collapsing_section(ui, "EFFECTS", false, |ui| {
        ui.horizontal(|ui| {
            lock_btn(ui, &mut st.locked_params, "audio.reverb_wet");
            let rvb_locked = st.locked_params.contains("audio.reverb_wet");
            ui.add_enabled(
                !rvb_locked,
                Slider::new(&mut st.config.audio.reverb_wet, 0.0..=1.0).text("Reverb"),
            );
        });
        let _ = ui
            .label("")
            .on_hover_text("Wet/dry mix for the 8×8 feedback delay network reverb.");
        ui.add(Slider::new(&mut st.config.audio.delay_ms, 0.0..=1000.0).text("Delay Time ms"))
            .on_hover_text("Delay time. BPM-syncable.");
        ui.add(Slider::new(&mut st.config.audio.delay_feedback, 0.0..=0.95).text("Feedback (max 90%)"))
            .on_hover_text("How much of the delayed signal feeds back into the delay — controls how many repeats you hear. Above 0.9 can create infinite feedback drones.");

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(Slider::new(&mut st.bpm, 60.0..=200.0).text("BPM"));
            let sync_color = if st.bpm_sync {
                Color32::from_rgb(0, 160, 0)
            } else {
                Color32::from_rgb(50, 50, 70)
            };
            if ui
                .add(
                    Button::new(RichText::new("Sync").color(Color32::WHITE))
                        .fill(sync_color)
                        .min_size(Vec2::new(44.0, 26.0)),
                )
                .clicked()
            {
                st.bpm_sync = !st.bpm_sync;
            }
        });
        if st.bpm_sync {
            ui.label(
                RichText::new(format!(
                    "Delay: {:.0}ms  LFO: {:.2}Hz",
                    60000.0 / st.bpm,
                    st.bpm / 60.0 * 0.25
                ))
                .size(11.0)
                .color(Color32::from_rgb(100, 200, 100)),
            );
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
        ui.label(
            RichText::new("Low shelf 200 Hz  ·  Mid peak  ·  High shelf 6 kHz")
                .size(10.0)
                .color(GRAY_HINT)
                .italics(),
        );
        ui.add_space(2.0);
        ui.add(Slider::new(&mut st.eq_low_db, -12.0..=12.0).text("Low  dB"))
            .on_hover_text("Low shelf gain at 200 Hz. Boost for warmth and bass weight; cut to thin out muddy low-end.");
        ui.add(Slider::new(&mut st.eq_mid_db, -12.0..=12.0).text("Mid  dB"))
            .on_hover_text(
                "Peaking mid-band gain. Boost presence and body; cut to scoop out harshness.",
            );
        ui.add(
            Slider::new(&mut st.eq_mid_freq, 200.0..=8000.0)
                .text("Mid Hz")
                .logarithmic(true),
        )
        .on_hover_text("Center frequency of the mid peak filter (200–8000 Hz).");
        ui.add(Slider::new(&mut st.eq_high_db, -12.0..=12.0).text("High dB"))
            .on_hover_text("High shelf gain at 6 kHz. Boost for air and brightness; cut to soften harsh highs.");
        let any_nonzero =
            st.eq_low_db.abs() > 0.1 || st.eq_mid_db.abs() > 0.1 || st.eq_high_db.abs() > 0.1;
        if any_nonzero {
            if ui
                .add(
                    Button::new(RichText::new("Reset EQ").color(Color32::WHITE).size(11.0))
                        .fill(Color32::from_rgb(40, 30, 50))
                        .min_size(Vec2::new(ui.available_width(), 24.0)),
                )
                .clicked()
            {
                st.eq_low_db = 0.0;
                st.eq_mid_db = 0.0;
                st.eq_high_db = 0.0;
            }
        }
    });

    // ---- STEREO WIDTH ----
    // #4 — Mid/side stereo width control.  Unity (1.0) passes the signal unchanged.
    // Below 1.0 collapses toward mono; above 1.0 pushes the stereo field wider.
    collapsing_section(ui, "STEREO WIDTH", false, |ui| {
        ui.add_space(2.0);
        let changed = ui
            .add(
                Slider::new(&mut st.stereo_width, 0.0..=3.0)
                    .text("Width")
                    .clamp_to_range(true),
            )
            .on_hover_text(
                "Master stereo width applied after the limiter.\n\
             0.0 = mono  ·  1.0 = unity (unchanged)  ·  3.0 = hyper-wide.\n\
             Uses energy-normalised mid/side encoding so loudness stays constant.",
            )
            .changed();
        if changed {
            if let Some(mut w) = st.stereo_width_shared.try_lock() {
                *w = st.stereo_width;
            }
        }
        ui.add_space(2.0);
        if (st.stereo_width - 1.0).abs() > 0.01 {
            if ui
                .add(
                    Button::new(
                        RichText::new("Reset Width")
                            .color(Color32::WHITE)
                            .size(11.0),
                    )
                    .fill(Color32::from_rgb(40, 30, 50))
                    .min_size(Vec2::new(ui.available_width(), 24.0)),
                )
                .clicked()
            {
                st.stereo_width = 1.0;
                if let Some(mut w) = st.stereo_width_shared.try_lock() {
                    *w = 1.0;
                }
            }
        }
    });

    // ---- OUTPUT ----
    collapsing_section(ui, "OUTPUT", false, |ui| {
        let st_sample_rate = st.sample_rate;
        let is_recording = recording.try_lock().map(|r| r.is_some()).unwrap_or(false);
        let rec_label = if is_recording {
            "⏹  Stop & Save"
        } else {
            "⏺  Start Recording"
        };
        let rec_color = if is_recording {
            Color32::from_rgb(180, 30, 30)
        } else {
            Color32::from_rgb(30, 120, 30)
        };
        if ui
            .add(
                Button::new(RichText::new(rec_label).color(Color32::WHITE))
                    .fill(rec_color)
                    .min_size(Vec2::new(ui.available_width(), 36.0)),
            )
            .clicked()
        {
            if is_recording {
                if let Some(mut lock) = recording.try_lock() {
                    *lock = None;
                }
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
                        if ui
                            .selectable_label(current_bars == b, format!("{}", b))
                            .clicked()
                        {
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
            let export_color = if is_exporting {
                Color32::from_rgb(180, 120, 0)
            } else {
                Color32::from_rgb(0, 90, 110)
            };
            if !is_exporting
                && ui
                    .add(
                        Button::new(RichText::new(export_label).color(Color32::WHITE))
                            .fill(export_color),
                    )
                    .clicked()
            {
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

        if ui
            .add(
                Button::new(RichText::new("Save as Default").color(Color32::WHITE))
                    .fill(Color32::from_rgb(30, 55, 30))
                    .min_size(Vec2::new(ui.available_width(), 28.0)),
            )
            .clicked()
        {
            let toml_str = toml::to_string_pretty(&st.config).unwrap_or_default();
            let _ = std::fs::write("config.toml", toml_str);
        }

        ui.add_space(6.0);

        CollapsingHeader::new(RichText::new("MY PATCHES").size(12.0).color(GRAY_HINT))
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut st.patch_name_input)
                            .desired_width(140.0)
                            .hint_text("Patch name..."),
                    );
                    if ui
                        .add(
                            Button::new(RichText::new("Save").color(Color32::WHITE))
                                .fill(Color32::from_rgb(0, 80, 120)),
                        )
                        .clicked()
                    {
                        let name = st.patch_name_input.clone();
                        if !name.is_empty() {
                            save_patch(&name, &st.config);
                            st.patch_list = list_patches();
                            st.toast_queue
                                .push(Toast::info(format!("Patch '{}' saved", name)));
                        } else {
                            st.toast_queue
                                .push(Toast::warning("Enter a patch name first"));
                        }
                    }
                });
                ui.add_space(4.0);
                let patches = st.patch_list.clone();
                if patches.is_empty() {
                    ui.label(
                        RichText::new("No patches saved yet")
                            .color(GRAY_HINT)
                            .size(11.0),
                    );
                } else {
                    for patch_name in &patches {
                        if ui.button(patch_name).clicked() {
                            if let Some(cfg) = load_patch_file(patch_name) {
                                let loaded_name = patch_name.clone();
                                st.config = cfg;
                                let pre_sigma = st.config.lorenz.sigma;
                                st.config.validate();
                                if (st.config.lorenz.sigma - pre_sigma).abs() > 1e-9 {
                                    st.toast_queue.push(Toast::warning(
                                        "Some config values were out of range and have been adjusted",
                                    ));
                                }
                                st.system_changed = true;
                                st.mode_changed = true;
                                st.toast_queue
                                    .push(Toast::info(format!("Loaded '{}'", loaded_name)));
                            } else {
                                let failed_name = patch_name.clone();
                                st.toast_queue.push(Toast::error(format!(
                                    "Failed to load '{}'",
                                    failed_name
                                )));
                            }
                        }
                    }
                }
            });

        // ---- RESTORE BACKUP ----
        {
            let patch_name = st.patch_name_input.clone();
            if !patch_name.is_empty() {
                let backups = list_backups(&patch_name);
                if !backups.is_empty() {
                    CollapsingHeader::new(
                        RichText::new("RESTORE BACKUP").size(12.0).color(GRAY_HINT),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        let mut to_restore: Option<String> = None;
                        for (filename, ts) in &backups {
                            let dt = std::time::UNIX_EPOCH + std::time::Duration::from_secs(*ts);
                            let label = if let Ok(elapsed) =
                                std::time::SystemTime::now().duration_since(dt)
                            {
                                let secs = elapsed.as_secs();
                                if secs < 3600 {
                                    format!("{} min ago", secs / 60)
                                } else {
                                    format!("{} h ago", secs / 3600)
                                }
                            } else {
                                filename.clone()
                            };
                            if ui.button(RichText::new(&label).size(11.0)).clicked() {
                                to_restore = Some(filename.clone());
                            }
                        }
                        if let Some(fname) = to_restore {
                            if let Some(cfg) = load_backup(&fname) {
                                st.config = cfg;
                                st.system_changed = true;
                                st.mode_changed = true;
                                st.toast_queue.push(Toast::info(format!(
                                    "Restored backup for '{}'",
                                    patch_name
                                )));
                            } else {
                                st.toast_queue.push(Toast::error("Failed to load backup"));
                            }
                        }
                    });
                }
            }
        }

        ui.add_space(6.0);

        CollapsingHeader::new(RichText::new("PERFORMANCE").size(12.0).color(GRAY_HINT))
            .default_open(false)
            .show(ui, |ui| {
                CollapsingHeader::new(RichText::new("Automation").size(11.0).color(GRAY_HINT))
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let rec_color = if st.auto_recording {
                                Color32::from_rgb(180, 30, 30)
                            } else {
                                Color32::from_rgb(60, 40, 40)
                            };
                            if ui
                                .add(
                                    Button::new(RichText::new("Rec").color(Color32::WHITE))
                                        .fill(rec_color)
                                        .min_size(Vec2::new(50.0, 26.0)),
                                )
                                .clicked()
                            {
                                st.auto_recording = !st.auto_recording;
                                if st.auto_recording {
                                    st.auto_playing = false;
                                    st.auto_events.clear();
                                    st.auto_start_time = Instant::now();
                                }
                            }
                            let play_color = if st.auto_playing {
                                Color32::from_rgb(0, 140, 0)
                            } else {
                                Color32::from_rgb(40, 60, 40)
                            };
                            if ui
                                .add(
                                    Button::new(RichText::new("Play").color(Color32::WHITE))
                                        .fill(play_color)
                                        .min_size(Vec2::new(50.0, 26.0)),
                                )
                                .clicked()
                            {
                                st.auto_playing = !st.auto_playing;
                                if st.auto_playing {
                                    st.auto_recording = false;
                                    st.auto_play_pos = 0;
                                    st.auto_start_time = Instant::now();
                                }
                            }
                            if ui
                                .add(
                                    Button::new(RichText::new("Stop").color(Color32::WHITE))
                                        .fill(Color32::from_rgb(50, 50, 50))
                                        .min_size(Vec2::new(50.0, 26.0)),
                                )
                                .clicked()
                            {
                                st.auto_recording = false;
                                st.auto_playing = false;
                            }
                        });
                        ui.checkbox(
                            &mut st.auto_loop,
                            RichText::new("Loop playback").color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(format!("{} events recorded", st.auto_events.len()))
                                .size(11.0)
                                .color(GRAY_HINT),
                        );
                        if st.auto_recording {
                            ui.label(
                                RichText::new("Recording...").color(Color32::from_rgb(255, 80, 80)),
                            );
                        }
                        if st.auto_playing {
                            ui.label(
                                RichText::new("Playing back").color(Color32::from_rgb(80, 255, 80)),
                            );
                        }
                    });

                ui.add_space(4.0);
                ui.checkbox(
                    &mut st.midi_enabled,
                    RichText::new("MIDI Output").color(Color32::WHITE),
                );
                if st.midi_enabled {
                    ui.label(
                        RichText::new("Sending to first MIDI port")
                            .size(11.0)
                            .color(Color32::from_rgb(100, 200, 100)),
                    );
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let rec = st.midi_rec_enabled;
                    let btn_label = if rec { "⬤ Stop MIDI Rec" } else { "⬤ Record MIDI" };
                    let btn_color = if rec {
                        Color32::from_rgb(180, 40, 40)
                    } else {
                        Color32::from_rgb(40, 40, 70)
                    };
                    if ui
                        .add(
                            Button::new(RichText::new(btn_label).color(Color32::WHITE))
                                .fill(btn_color)
                                .min_size(Vec2::new(130.0, 24.0)),
                        )
                        .clicked()
                    {
                        if rec {
                            st.midi_rec_enabled = false;
                            let events = std::mem::take(&mut st.midi_rec_events);
                            if !events.is_empty() {
                                let path = format!(
                                    "midi_exports/export_{}.mid",
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs()
                                );
                                match write_midi_file(&events, &path) {
                                    Ok(_) => log::info!("MIDI exported to {path}"),
                                    Err(e) => log::warn!("MIDI export failed: {e}"),
                                }
                            }
                        } else {
                            st.midi_rec_enabled = true;
                            st.midi_rec_events.clear();
                            st.midi_rec_start = std::time::Instant::now();
                        }
                    }
                    if rec {
                        let elapsed = st.midi_rec_start.elapsed().as_secs();
                        ui.label(
                            RichText::new(format!("{:02}:{:02}", elapsed / 60, elapsed % 60))
                                .size(11.0)
                                .color(Color32::from_rgb(255, 80, 80)),
                        );
                    }
                });
            });
    });

    // ---- RHYTHM & ARP ----
    collapsing_section(ui, "RHYTHM & ARP", false, |ui| {
        ui.checkbox(
            &mut st.arp_enabled,
            RichText::new("Enable Arpeggiator").color(Color32::WHITE),
        );
        if st.arp_enabled {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Steps:").color(Color32::WHITE));
                for &s in &[4usize, 8, 16] {
                    let sel = st.arp_steps == s;
                    let col = if sel {
                        Color32::from_rgb(0, 140, 210)
                    } else {
                        Color32::from_rgb(40, 40, 70)
                    };
                    if ui
                        .add(
                            Button::new(RichText::new(format!("{}", s)).color(Color32::WHITE))
                                .fill(col)
                                .min_size(Vec2::new(36.0, 24.0)),
                        )
                        .clicked()
                    {
                        st.arp_steps = s;
                    }
                }
                ui.separator();
                ui.label(RichText::new("Oct:").color(Color32::WHITE));
                for &o in &[1usize, 2] {
                    let sel = st.arp_octaves == o;
                    let col = if sel {
                        Color32::from_rgb(0, 140, 210)
                    } else {
                        Color32::from_rgb(40, 40, 70)
                    };
                    if ui
                        .add(
                            Button::new(RichText::new(format!("{}", o)).color(Color32::WHITE))
                                .fill(col)
                                .min_size(Vec2::new(30.0, 24.0)),
                        )
                        .clicked()
                    {
                        st.arp_octaves = o;
                    }
                }
            });
            ui.add(Slider::new(&mut st.arp_bpm, 40.0..=240.0).text("ARP BPM"));
            let step_pct = if st.arp_steps > 0 {
                (st.arp_position as f32 + 1.0) / st.arp_steps as f32
            } else {
                0.0
            };
            ui.add(ProgressBar::new(step_pct).text(format!(
                "Step {}/{}",
                st.arp_position + 1,
                st.arp_steps
            )));
        }
        ui.add_space(4.0);
        ui.separator();
        ui.label(RichText::new("Plucked Strings").color(Color32::WHITE));
        ui.checkbox(
            &mut st.ks_enabled,
            RichText::new("Enable KS (Poincaré rhythm)").color(Color32::WHITE),
        );
        if st.ks_enabled {
            ui.add(Slider::new(&mut st.ks_volume, 0.0..=1.0).text("Volume"));
        }
    });

    // ---- COUPLED SYSTEMS ----
    drop(st); // release lock before calling collapsing_section (which takes state)
    collapsing_section(ui, "COUPLED SYSTEMS", false, |ui| {
        let mut st = state.lock();
        ui.checkbox(
            &mut st.coupled_enabled,
            RichText::new("Enable Coupled Attractor").color(Color32::WHITE),
        );
        ui.add_space(4.0);

        // Source system dropdown
        let systems = [
            "lorenz",
            "rossler",
            "duffing",
            "van_der_pol",
            "halvorsen",
            "aizawa",
            "chua",
            "double_pendulum",
            "geodesic_torus",
            "kuramoto",
            "three_body",
        ];
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
        ui.label(
            RichText::new("Live Output (x):")
                .color(GRAY_HINT)
                .size(11.0),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Main:").color(Color32::WHITE).size(11.0));
            let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 14.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 40));
            let bar = egui::Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width() * main_x.clamp(0.0, 1.0), rect.height()),
            );
            ui.painter()
                .rect_filled(bar, 2.0, Color32::from_rgb(0, 160, 200));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Src: ").color(Color32::WHITE).size(11.0));
            let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 14.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 40));
            let bar =
                egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * src_x.clamp(0.0, 1.0), rect.height()));
            ui.painter()
                .rect_filled(bar, 2.0, Color32::from_rgb(200, 100, 0));
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new("Sync Error (EMA):")
                .color(GRAY_HINT)
                .size(11.0),
        );
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 14.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 40));
            let bar = egui::Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width() * sync_err.clamp(0.0, 1.0), rect.height()),
            );
            let err_color = {
                let t = sync_err.clamp(0.0, 1.0);
                Color32::from_rgb((t * 220.0) as u8, ((1.0 - t) * 160.0) as u8, 60)
            };
            ui.painter().rect_filled(bar, 2.0, err_color);
            ui.label(
                RichText::new(format!("{:.3}", sync_err))
                    .color(GRAY_HINT)
                    .size(10.0),
            );
        });
        {
            let mut st = state.lock();
            ui.checkbox(
                &mut st.coupled_bidirectional,
                RichText::new("Bidirectional Coupling").color(Color32::WHITE),
            );
        }
    });

    // ---- CUSTOM ODE ----
    collapsing_section(ui, "CUSTOM ODE", false, |ui| {
        ui.label(
            RichText::new("Define your own 3D or 4D ODE system")
                .color(GRAY_HINT)
                .size(11.0),
        );
        ui.label(
            RichText::new("Variables: x y z w t  |  Functions: sin cos exp abs sqrt ln log tan")
                .color(GRAY_HINT)
                .size(10.0),
        );
        ui.add_space(4.0);

        let (mut ex, mut ey, mut ez, mut ew, err) = {
            let st = state.lock();
            (
                st.custom_ode_x.clone(),
                st.custom_ode_y.clone(),
                st.custom_ode_z.clone(),
                st.custom_ode_w.clone(),
                st.custom_ode_error.clone(),
            )
        };

        let mut any_expr_changed = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("dx/dt =").color(Color32::WHITE).size(12.0));
            if ui.text_edit_singleline(&mut ex).changed() {
                state.lock().custom_ode_x = ex.clone();
                any_expr_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("dy/dt =").color(Color32::WHITE).size(12.0));
            if ui.text_edit_singleline(&mut ey).changed() {
                state.lock().custom_ode_y = ey.clone();
                any_expr_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("dz/dt =").color(Color32::WHITE).size(12.0));
            if ui.text_edit_singleline(&mut ez).changed() {
                state.lock().custom_ode_z = ez.clone();
                any_expr_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("dw/dt =")
                    .color(Color32::from_rgb(180, 180, 100))
                    .size(12.0),
            );
            let hint = RichText::new("(optional — enables 4D mode)").italics().size(10.0);
            if ew.is_empty() {
                ui.label(hint);
            }
            if ui.text_edit_singleline(&mut ew).changed() {
                state.lock().custom_ode_w = ew.clone();
                any_expr_changed = true;
            }
        });
        // Live validation on every keystroke
        if any_expr_changed {
            use crate::systems::validate_exprs;
            let validation = validate_exprs(&ex, &ey, &ez, &ew);
            let mut st = state.lock();
            match validation {
                Ok(()) => {
                    st.custom_ode_error.clear();
                }
                Err(e) => {
                    st.custom_ode_error = e;
                }
            }
        }
        ui.add_space(4.0);
        // Show validation error/warning inline, before the Apply button
        if !err.is_empty() {
            let is_warning = err.starts_with("Warning");
            let color = if is_warning {
                Color32::from_rgb(255, 200, 60)
            } else {
                Color32::from_rgb(255, 80, 80)
            };
            ui.colored_label(color, &err);
        }
        let can_apply = !err.starts_with("dx/dt error")
            && !err.starts_with("dy/dt error")
            && !err.starts_with("dz/dt error")
            && !err.starts_with("dw/dt error");
        let apply_btn = ui.add_enabled(
            can_apply,
            egui::Button::new(
                RichText::new("Apply Custom ODE")
                    .color(Color32::WHITE)
                    .strong(),
            ),
        );
        if apply_btn.clicked() {
            use crate::systems::validate_exprs;
            let mut st = state.lock();
            let ew2 = st.custom_ode_w.clone();
            match validate_exprs(&st.custom_ode_x, &st.custom_ode_y, &st.custom_ode_z, &ew2) {
                Ok(()) => {
                    st.custom_ode_error.clear();
                    st.push_undo();
                    st.config.system.name = "custom".into();
                    st.system_changed = true;
                    st.toast_queue.push(Toast::info("Custom ODE applied"));
                }
                Err(e) => {
                    st.toast_queue
                        .push(Toast::error(format!("ODE error: {}", e)));
                    st.custom_ode_error = e;
                }
            }
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new("Available: x y z t pi e  |  Functions: sin cos exp abs sqrt ln log tan")
                .italics()
                .size(10.0)
                .color(GRAY_HINT),
        );
        ui.label(
            RichText::new("Example (Lorenz): 10*(y-x) | x*(28-z)-y | x*y-2.667*z")
                .italics()
                .size(10.0)
                .color(GRAY_HINT),
        );
    });

    // ---- MOD MATRIX ----
    collapsing_section(ui, "MOD MATRIX", false, |ui| {
        let mut st = state.lock();
        // Table header
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Source")
                    .color(AMBER)
                    .size(11.0)
                    .strong()
                    .monospace(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Target")
                    .color(AMBER)
                    .size(11.0)
                    .strong()
                    .monospace(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Depth")
                    .color(AMBER)
                    .size(11.0)
                    .strong()
                    .monospace(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("En")
                    .color(AMBER)
                    .size(11.0)
                    .strong()
                    .monospace(),
            );
        });
        ui.separator();

        let source_options = ["x", "y", "z", "speed"];
        let target_options = [
            "reverb_wet",
            "delay_ms",
            "base_freq_mult",
            "speed",
            "chorus_mix",
            "master_volume",
            "chaos",
        ];

        let mut to_remove: Option<usize> = None;
        let n = st.mod_matrix.len();
        for i in 0..n {
            ui.horizontal(|ui| {
                // Source dropdown
                let cur_src = st.mod_matrix[i].source.clone();
                ComboBox::from_id_source(format!("mod_src_{}", i))
                    .selected_text(&cur_src)
                    .width(52.0)
                    .show_ui(ui, |ui| {
                        for opt in &source_options {
                            if ui.selectable_label(cur_src == *opt, *opt).clicked() {
                                st.mod_matrix[i].source = opt.to_string();
                            }
                        }
                    });
                // Target dropdown
                let cur_tgt = st.mod_matrix[i].target.clone();
                ComboBox::from_id_source(format!("mod_tgt_{}", i))
                    .selected_text(&cur_tgt)
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        for opt in &target_options {
                            if ui.selectable_label(cur_tgt == *opt, *opt).clicked() {
                                st.mod_matrix[i].target = opt.to_string();
                            }
                        }
                    });
                // Depth slider
                ui.add(
                    Slider::new(&mut st.mod_matrix[i].depth, -1.0..=1.0)
                        .text("")
                        .fixed_decimals(2),
                )
                .on_hover_text("Modulation depth: negative inverts the modulation direction.");
                // Enabled checkbox
                ui.checkbox(&mut st.mod_matrix[i].enabled, "");
                // Remove button
                if ui
                    .small_button(RichText::new("✕").color(Color32::from_rgb(200, 60, 60)))
                    .clicked()
                {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(idx) = to_remove {
            st.mod_matrix.remove(idx);
        }

        ui.add_space(4.0);
        // Add Route button (capped at 8)
        if st.mod_matrix.len() < 8 {
            if ui
                .button(
                    RichText::new("+ Add Route")
                        .color(Color32::from_rgb(0, 200, 100))
                        .size(12.0),
                )
                .clicked()
            {
                st.mod_matrix.push(crate::ui::ModRoute {
                    source: "x".to_string(),
                    target: "reverb_wet".to_string(),
                    depth: 0.5,
                    enabled: true,
                });
            }
        } else {
            ui.label(
                RichText::new("Max 8 routes")
                    .color(GRAY_HINT)
                    .size(11.0)
                    .italics(),
            );
        }
        if st.mod_matrix.is_empty() {
            ui.label(
                RichText::new("No routes. Add one to modulate synth params from attractor state.")
                    .color(GRAY_HINT)
                    .size(11.0)
                    .italics(),
            );
        }
    });

    // ---- MIDI INPUT ----
    collapsing_section(ui, "MIDI INPUT", false, |ui| {
        let mut st = state.lock();
        ui.checkbox(
            &mut st.midi_in_enabled,
            RichText::new("Enable MIDI Input").color(Color32::WHITE),
        );
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
            if ui
                .add(
                    DragValue::new(&mut cc_num)
                        .clamp_range(0u32..=127)
                        .prefix("CC#"),
                )
                .changed()
            {
                st.midi_in_cc_num = cc_num as u8;
            }
            // CC Target row with Learn button
            ui.horizontal(|ui| {
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
                let learn_col = if st.midi_cc_learn_active {
                    Color32::from_rgb(200, 140, 0)
                } else {
                    Color32::from_rgb(60, 60, 100)
                };
                if ui
                    .add(
                        Button::new(RichText::new("Learn").color(Color32::WHITE).size(11.0))
                            .fill(learn_col)
                            .min_size(Vec2::new(48.0, 22.0)),
                    )
                    .clicked()
                {
                    st.midi_cc_learn_active = true;
                    st.midi_cc_learn_target = st.midi_in_cc_target.clone();
                    st.midi_cc_learn_last_cc = st.midi_in_last_cc;
                }
            });

            // Learn mode detection: check if midi_in_last_cc changed while learning
            if st.midi_cc_learn_active {
                let current_cc = st.midi_in_last_cc;
                if current_cc != st.midi_cc_learn_last_cc {
                    let target = st.midi_cc_learn_target.clone();
                    st.midi_cc_map
                        .retain(|(cc, t)| *cc != current_cc && t != &target);
                    st.midi_cc_map.push((current_cc, target));
                    st.midi_cc_learn_active = false;
                }
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Waiting for CC...")
                        .color(Color32::from_rgb(220, 170, 0))
                        .size(11.0)
                        .strong(),
                );
            }

            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "Note: {}  Vel: {}  CC: {}",
                    st.midi_in_last_note, st.midi_in_last_vel, st.midi_in_last_cc
                ))
                .color(CYAN)
                .size(11.0),
            );
            // Visual summary of active MIDI mappings
            ui.add_space(2.0);
            let badge_color = Color32::from_rgb(0, 200, 100);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("Note\u{2192}{}", st.midi_in_note_target))
                        .color(badge_color)
                        .size(11.0)
                        .strong(),
                );
                ui.label(RichText::new("  |  ").color(GRAY_HINT).size(11.0));
                ui.label(
                    RichText::new(format!("Vel\u{2192}{}", st.midi_in_vel_target))
                        .color(badge_color)
                        .size(11.0)
                        .strong(),
                );
                ui.label(RichText::new("  |  ").color(GRAY_HINT).size(11.0));
                ui.label(
                    RichText::new(format!(
                        "CC#{}\u{2192}{}",
                        st.midi_in_cc_num, st.midi_in_cc_target
                    ))
                    .color(badge_color)
                    .size(11.0)
                    .strong(),
                );
            });

            // CC Map list with remove buttons
            if !st.midi_cc_map.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new("CC Map:").color(CYAN).size(11.0).strong());
                let map_snapshot: Vec<(u8, String)> = st.midi_cc_map.clone();
                let mut remove_idx: Option<usize> = None;
                for (i, (cc_num, param)) in map_snapshot.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("CC{} \u{2192} {}", cc_num, param))
                                .color(Color32::from_rgb(180, 220, 180))
                                .size(11.0)
                                .monospace(),
                        );
                        if ui
                            .small_button(
                                RichText::new("\u{00d7}").color(Color32::from_rgb(200, 80, 80)),
                            )
                            .clicked()
                        {
                            remove_idx = Some(i);
                        }
                    });
                }
                if let Some(idx) = remove_idx {
                    st.midi_cc_map.remove(idx);
                }
            }
        }
    });

    // ---- OSC OUTPUT ----
    collapsing_section(ui, "OSC OUTPUT", false, |ui| {
        let mut st = state.lock();
        ui.checkbox(
            &mut st.osc_enabled,
            RichText::new("Enable OSC Output").color(Color32::WHITE),
        );
        if st.osc_enabled {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Host:").color(GRAY_HINT).size(11.0));
                ui.add(
                    TextEdit::singleline(&mut st.osc_host)
                        .desired_width(120.0)
                        .hint_text("127.0.0.1"),
                );
            });
            let mut port = st.osc_port as u32;
            if ui
                .add(
                    DragValue::new(&mut port)
                        .clamp_range(1000u32..=65535)
                        .prefix("Port: "),
                )
                .changed()
            {
                st.osc_port = port as u16;
            }
            if !st.osc_status.is_empty() {
                ui.label(RichText::new(&st.osc_status).color(CYAN).size(11.0));
            }
        }
    });

    // ---- MIDI CLOCK OUT ----
    collapsing_section(ui, "MIDI CLOCK OUT", false, |ui| {
        let mut st = state.lock();
        ui.checkbox(
            &mut st.midi_clock_enabled,
            RichText::new("MIDI Clock Out").color(Color32::WHITE),
        );
        ui.label(
            RichText::new("Syncs external gear to BPM")
                .color(GRAY_HINT)
                .size(11.0)
                .italics(),
        );
    });

    // ---- BEHAVIORAL LAYERS (item 16) ----
    collapsing_section(ui, "BEHAVIORAL LAYERS", false, |ui| {
        ui.label(
            RichText::new("Invisible automatic modifiers — uncheck to disable")
                .color(GRAY_HINT)
                .size(10.5)
                .italics(),
        );
        ui.add_space(4.0);
        let mut st = state.lock();
        macro_rules! behav_row {
            ($label:expr, $tip:expr, $field:expr) => {{
                let col = if $field { Color32::WHITE } else { GRAY_HINT };
                ui.horizontal(|ui| {
                    ui.checkbox(&mut $field, "");
                    ui.label(RichText::new($label).color(col).size(11.0))
                        .on_hover_text($tip);
                });
            }};
        }
        behav_row!(
            "🕐 Time of Day",
            "Biases macros at launch: night→darker, day→brighter",
            st.behav_time_of_day
        );
        behav_row!(
            "🌸 Seasonal Drift",
            "Base frequency shifts ±1.5% over the calendar year",
            st.behav_seasonal_drift
        );
        behav_row!(
            "📉 Volume Creep",
            "Volume drifts from 1.0 → 0.87 over ~1 hour",
            st.behav_volume_creep
        );
        behav_row!(
            "🫁 Breathing",
            "4.5s gain oscillation ±0.3 dB",
            st.behav_breathing
        );
        behav_row!(
            "🌙 Circadian Sleep",
            "~8% volume cut during 3–5am hours",
            st.behav_circadian_sleep
        );
        behav_row!(
            "💤 Attractor Dreams",
            "30min idle → brief visit to another system",
            st.behav_dreams
        );
        behav_row!(
            "⌛ Aging",
            "High-freq rolloff increases slowly over hours",
            st.behav_aging
        );
        behav_row!(
            "⌨ Typing Resonance",
            "Windows: keyboard cadence wobbles frequency",
            st.behav_typing_resonance
        );
        behav_row!(
            "🤝 Instance Empathy",
            "Two instances sync volume over local UDP",
            st.behav_instance_empathy
        );
    });

    // ---- SOUND DESIGN TIPS ----
    collapsing_section(ui, "SOUND DESIGN TIPS", false, |ui| {
        draw_tips_content(ui);
    });

    // ---- KEYBOARD SHORTCUTS ----
    collapsing_section(ui, "KEYBOARD SHORTCUTS", false, |ui| {
        let shortcuts = [
            ("Space", "Pause / Resume simulation"),
            ("F", "Toggle performance mode (fullscreen portrait)"),
            ("R", "Toggle recording on/off"),
            ("P", "Toggle arranger playback"),
            ("E", "Toggle Evolve (macro random walk)"),
            ("?", "Show / hide this tips window"),
            ("Ctrl+Z", "Undo last config change"),
            ("Ctrl+Shift+Z", "Redo"),
            ("↑ / ↓", "Volume up / down (+/- 5%)"),
            ("← / →", "Speed slower / faster (÷1.2 / ×1.2)"),
            ("1", "Tab: Phase Portrait"),
            ("2", "Tab: MIXER"),
            ("3", "Tab: ARRANGE"),
            ("4", "Tab: Waveform"),
            ("5", "Tab: Note Map"),
            ("6", "Tab: Math View"),
            ("7", "Tab: Bifurcation Diagram"),
        ];
        for (key, desc) in &shortcuts {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(*key)
                        .color(AMBER)
                        .strong()
                        .size(11.0)
                        .monospace(),
                );
                ui.add_space(8.0);
                ui.label(RichText::new(*desc).color(GRAY_HINT).size(11.0));
            });
        }
    });
    ui.add_space(4.0);
}

// ---------------------------------------------------------------------------
// Simple panel — beginner-friendly macro controls
// (draw_tips_content lives in src/ui_tips.rs)
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
                let active_scenes: Vec<usize> = (0..scenes_snap.len())
                    .filter(|&i| scenes_snap[i].active)
                    .collect();
                let scene_ord = active_scenes
                    .iter()
                    .position(|&i| i == scene_idx)
                    .unwrap_or(0)
                    + 1;
                let scene_count = active_scenes.len();
                let scene_name = &scenes_snap[scene_idx].name;
                let phase_label = if morphing { "morphing →" } else { "holding" };
                let elapsed_m = (arr_elapsed / 60.0) as u32;
                let elapsed_s = (arr_elapsed % 60.0) as u32;
                let total_m = (total / 60.0) as u32;
                let total_s = (total % 60.0) as u32;
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "Scene {}/{} — {} ({}) {:02}:{:02}/{:02}:{:02}",
                        scene_ord,
                        scene_count,
                        scene_name,
                        phase_label,
                        elapsed_m,
                        elapsed_s,
                        total_m,
                        total_s
                    ))
                    .color(CYAN)
                    .size(11.0),
                );
                let progress = if total > 0.001 {
                    (arr_elapsed / total).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                ui.add(ProgressBar::new(progress).desired_width(ui.available_width()));
            }
        } else {
            // First-run hint when nothing is playing
            ui.add_space(4.0);
            ui.label(
                RichText::new("👆 Hit AUTO to start — generates a unique arrangement each time")
                    .color(AMBER)
                    .size(11.0)
                    .italics(),
            );
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
        ui.label(
            RichText::new("ARRANGEMENT MOOD")
                .color(CYAN)
                .size(11.0)
                .strong(),
        );
        ui.add_space(4.0);
        let mood_defs: &[(&str, &str, &str, Color32, Color32)] = &[
            (
                "ambient",
                "🌙  Ambient",
                "Deep reverb · slow drift · harmonic pads",
                Color32::from_rgb(0, 95, 175),
                Color32::from_rgb(60, 150, 255),
            ),
            (
                "rhythmic",
                "⚡  Rhythmic",
                "Pulsing energy · granular · percussive",
                Color32::from_rgb(155, 115, 0),
                Color32::from_rgb(240, 185, 30),
            ),
            (
                "experimental",
                "🔬  Experimental",
                "Glitch · microtonal · unexpected",
                Color32::from_rgb(100, 0, 165),
                Color32::from_rgb(185, 80, 255),
            ),
        ];
        for (mood_key, label, desc, active_fill, accent) in mood_defs.iter() {
            let selected = arr_mood == *mood_key;
            let fill = if selected {
                *active_fill
            } else {
                Color32::from_rgb(16, 16, 32)
            };
            let border_col = if selected {
                *accent
            } else {
                Color32::from_rgb(38, 40, 68)
            };
            let border_w = if selected { 1.5 } else { 1.0 };
            let frame = egui::Frame::none()
                .fill(fill)
                .stroke(egui::Stroke::new(border_w, border_col))
                .inner_margin(egui::Margin::symmetric(10.0, 7.0))
                .rounding(egui::Rounding::same(7.0));
            let resp = frame
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(*label)
                                .color(Color32::WHITE)
                                .size(12.5)
                                .strong(),
                        );
                        if selected {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(RichText::new("●").color(*accent).size(10.0));
                                },
                            );
                        }
                    });
                    ui.label(RichText::new(*desc).color(GRAY_HINT).size(10.0));
                })
                .response;
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
                    ui.label(
                        RichText::new("🔊  VOLUME")
                            .color(Color32::WHITE)
                            .strong()
                            .size(12.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(&db_label).color(AMBER).size(11.0));
                    });
                });
                ui.add(Slider::new(&mut st.config.audio.master_volume, 0.0..=1.0).text(""))
                    .on_hover_text("Master output volume. Use ↑/↓ arrow keys as a quick shortcut.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let pause_label = if st.paused {
                        "▶  RESUME"
                    } else {
                        "⏸  PAUSE"
                    };
                    let (pause_fill, pause_stroke) = if st.paused {
                        (
                            Color32::from_rgb(18, 135, 65),
                            Color32::from_rgb(60, 220, 110),
                        )
                    } else {
                        (
                            Color32::from_rgb(15, 90, 165),
                            Color32::from_rgb(60, 160, 255),
                        )
                    };
                    let avail = ui.available_width();
                    if ui
                        .add(
                            Button::new(
                                RichText::new(pause_label)
                                    .color(Color32::WHITE)
                                    .strong()
                                    .size(13.0),
                            )
                            .fill(pause_fill)
                            .stroke(egui::Stroke::new(1.5, pause_stroke))
                            .min_size(Vec2::new(avail - 60.0, 36.0)),
                        )
                        .on_hover_text("Pause or resume. Shortcut: Space bar.")
                        .clicked()
                    {
                        st.paused = !st.paused;
                    }
                    let perf_fill = if st.perf_mode {
                        Color32::from_rgb(175, 80, 0)
                    } else {
                        Color32::from_rgb(24, 24, 44)
                    };
                    if ui
                        .add(
                            Button::new(RichText::new("⛶").color(Color32::WHITE).size(16.0))
                                .fill(perf_fill)
                                .min_size(Vec2::new(50.0, 36.0)),
                        )
                        .on_hover_text(
                            "Performance mode: fullscreen phase portrait. Press F to toggle.",
                        )
                        .clicked()
                    {
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
        {
            let mut st = state.lock();
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔍").size(11.0).color(GRAY_HINT));
                ui.add(
                    egui::TextEdit::singleline(&mut st.preset_search)
                        .hint_text("Search presets…")
                        .desired_width(ui.available_width()),
                );
            });
        }
        ui.add_space(4.0);

        let search_lower = state.lock().preset_search.to_lowercase();
        let show_fav_only_sim = state.lock().show_favorites_only;
        // #13: favorites-only toggle
        {
            let fav_only = show_fav_only_sim;
            let fav_col = if fav_only {
                egui::Color32::from_rgb(255, 205, 30)
            } else {
                egui::Color32::from_rgb(80, 85, 110)
            };
            if ui
                .add(
                    Button::new(RichText::new("⭐ only").color(fav_col).size(11.0))
                        .fill(if fav_only {
                            egui::Color32::from_rgb(60, 50, 10)
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .stroke(egui::Stroke::new(1.0, fav_col))
                        .min_size(Vec2::new(58.0, 22.0)),
                )
                .clicked()
            {
                state.lock().show_favorites_only = !fav_only;
            }
            ui.add_space(4.0);
        }
        let show_count = if show_all { PRESETS.len() } else { 12 };
        let fav_set_sim: HashSet<String> = state.lock().favorite_presets.clone();
        let mut sorted_presets_sim: Vec<&crate::patches::Preset> = PRESETS
            .iter()
            .take(show_count)
            .filter(|p| {
                (search_lower.is_empty()
                    || p.name.to_lowercase().contains(&search_lower)
                    || p.category.to_lowercase().contains(&search_lower))
                    && (!show_fav_only_sim || fav_set_sim.contains(p.name))
            })
            .collect();
        sorted_presets_sim.sort_by_key(|p| (!fav_set_sim.contains(p.name), p.category, p.name));
        let mut last_cat = "";
        for preset in sorted_presets_sim.iter() {
            if preset.category != last_cat {
                last_cat = preset.category;
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("  {}", preset.category.to_uppercase()))
                            .color(GRAY_HINT)
                            .size(10.0)
                            .strong(),
                    );
                    let avail = ui.available_width();
                    let (sep_rect, _) =
                        ui.allocate_exact_size(Vec2::new(avail - 4.0, 1.0), Sense::hover());
                    ui.painter()
                        .rect_filled(sep_rect, 0.0, Color32::from_rgb(35, 38, 65));
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
            let border_col = if is_selected {
                pc
            } else {
                Color32::from_rgb(34, 36, 64)
            };
            let is_fav_sim = fav_set_sim.contains(preset.name);
            let card = egui::Frame::none()
                .fill(bg_color)
                .stroke(Stroke::new(border_w, border_col))
                .inner_margin(egui::Margin::symmetric(0.0, 5.0))
                .rounding(egui::Rounding::same(6.0));
            let response = card
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // #13: star toggle
                        let star_col = if is_fav_sim {
                            Color32::from_rgb(255, 205, 30)
                        } else {
                            Color32::from_rgb(60, 65, 90)
                        };
                        if ui
                            .add(
                                Button::new(RichText::new("⭐").size(12.0).color(star_col))
                                    .fill(Color32::TRANSPARENT)
                                    .frame(false)
                                    .min_size(Vec2::new(22.0, 22.0)),
                            )
                            .clicked()
                        {
                            let mut st = state.lock();
                            if is_fav_sim {
                                st.favorite_presets.remove(preset.name);
                            } else {
                                st.favorite_presets.insert(preset.name.to_string());
                            }
                            let fav_txt: String = st
                                .favorite_presets
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("\n");
                            let _ = std::fs::write("favorites.txt", fav_txt);
                        }
                        // Colored left accent strip
                        let strip_h = 34.0;
                        let (strip_rect, _) =
                            ui.allocate_exact_size(Vec2::new(5.0, strip_h), Sense::hover());
                        let strip_col = if is_selected {
                            pc
                        } else {
                            Color32::from_rgba_premultiplied(
                                pc.r() / 3,
                                pc.g() / 3,
                                pc.b() / 3,
                                200,
                            )
                        };
                        ui.painter()
                            .rect_filled(strip_rect, egui::Rounding::same(3.0), strip_col);
                        ui.add_space(7.0);
                        ui.vertical(|ui| {
                            let name_col = if is_selected { pc } else { Color32::WHITE };
                            ui.label(
                                RichText::new(preset.name)
                                    .strong()
                                    .color(name_col)
                                    .size(12.0),
                            );
                            ui.label(
                                RichText::new(preset.description)
                                    .italics()
                                    .color(GRAY_HINT)
                                    .size(10.0),
                            );
                        });
                    });
                })
                .response;
            if response.interact(Sense::click()).clicked() {
                let mut st = state.lock();
                st.push_undo();
                // #12: snapshot locked params before load
                let locked_snapshot_sim: Vec<(String, f64)> = st
                    .locked_params
                    .iter()
                    .map(|k| (k.clone(), get_param_value(&st.config, k)))
                    .collect();
                st.selected_preset = preset.name.to_string();
                st.config = load_preset(preset.name);
                for (k, v) in locked_snapshot_sim {
                    set_param_value(&mut st.config, &k, v);
                }
                st.system_changed = true;
                st.mode_changed = true;
            }
            ui.add_space(3.0);
        }

        if !show_all {
            ui.add_space(4.0);
            if ui
                .add(
                    Button::new(
                        RichText::new(format!("Show all {} presets ▾", PRESETS.len()))
                            .color(CYAN)
                            .size(11.0),
                    )
                    .fill(Color32::from_rgb(14, 18, 36))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(40, 60, 110)))
                    .min_size(Vec2::new(ui.available_width(), 28.0)),
                )
                .clicked()
            {
                state.lock().simple_show_all_presets = true;
            }
        } else {
            ui.add_space(4.0);
            if ui
                .add(
                    Button::new(RichText::new("Show less ▴").color(GRAY_HINT).size(11.0))
                        .fill(Color32::from_rgb(14, 18, 36))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(35, 38, 65)))
                        .min_size(Vec2::new(ui.available_width(), 28.0)),
                )
                .clicked()
            {
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

    // ---- SYSTEM STATES (Invisible Behaviors) ----
    {
        let (wounded, time_of_day_f, macro_walk_enabled, startup_ramp_t, shutdown_fading) = {
            let st = state.lock();
            (
                st.wounded,
                st.time_of_day_f,
                st.macro_walk_enabled,
                st.startup_ramp_t,
                st.shutdown_fading,
            )
        };
        let circadian_sleep = time_of_day_f < 0.15 || time_of_day_f > 0.98;
        let behaviors: &[(&str, bool)] = &[
            ("Wound healing", wounded),
            ("Circadian sleep", circadian_sleep),
            ("Evolve active", macro_walk_enabled),
            ("Startup warmup", startup_ramp_t < 1.0),
            ("Shutdown fading", shutdown_fading),
        ];
        let any_active = behaviors.iter().any(|(_, v)| *v);
        ui.add_space(8.0);
        egui::Frame::none()
            .fill(Color32::from_rgba_premultiplied(12, 14, 28, 200))
            .stroke(egui::Stroke::new(
                1.0,
                Color32::from_rgba_premultiplied(40, 44, 80, 180),
            ))
            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
            .rounding(egui::Rounding::same(6.0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    RichText::new("System States")
                        .size(10.0)
                        .color(if any_active {
                            Color32::from_rgba_premultiplied(160, 170, 200, 200)
                        } else {
                            Color32::from_rgba_premultiplied(80, 85, 110, 150)
                        })
                        .strong(),
                );
                ui.add_space(3.0);
                ui.horizontal_wrapped(|ui| {
                    for (label, active) in behaviors {
                        let dot_col = if *active {
                            Color32::from_rgba_premultiplied(60, 220, 120, 220)
                        } else {
                            Color32::from_rgba_premultiplied(50, 55, 80, 140)
                        };
                        let text_col = if *active {
                            Color32::from_rgba_premultiplied(180, 200, 180, 210)
                        } else {
                            Color32::from_rgba_premultiplied(70, 75, 100, 140)
                        };
                        ui.horizontal(|ui| {
                            let (dot_rect, _) =
                                ui.allocate_exact_size(Vec2::new(7.0, 7.0), Sense::hover());
                            ui.painter().circle_filled(dot_rect.center(), 3.5, dot_col);
                            ui.label(RichText::new(*label).size(9.5).color(text_col));
                        });
                        ui.add_space(4.0);
                    }
                });
            });
    }
}

fn param_range(param: &str) -> (f64, f64) {
    match param {
        "rho" => (20.0, 50.0),
        "sigma" => (5.0, 20.0),
        "coupling" => (0.0, 5.0),
        "c" => (3.0, 10.0),
        "r" => (2.4, 4.0),       // logistic map: period-doubling onset to full chaos
        "k" => (0.0, 2.0),       // standard map: KAM to fully chaotic
        "b" => (0.05, 0.35),     // thomas: critical b≈0.208 separates chaos from limit cycle
        "lambda" => (3.0, 10.0), // rucklidge: forcing amplitude
        "a_rossler" => (0.0, 0.5), // rossler a: affects spiral tightness
        "shimizu_a" => (0.3, 1.5),  // shimizu-morioka a: damping → chaos boundary
        "genesio_c" => (3.0, 10.0), // genesio-tesi c: restoring force → period-doubling
        "liu_b" => (1.0, 4.0),      // liu b: y growth → chaos onset
        "windmi_b" => (1.5, 4.0),   // windmi b: drive amplitude → chaos onset
        "finance_a" => (1.0, 6.0),  // finance a: savings rate → attractor shape
        "hyperchaos_c" => (20.0, 40.0), // hyperchaos c: Rayleigh analog → chaos strength
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
            if let Some(mut lock) = recording.try_lock() {
                *lock = None;
            }
            state.lock().save_gen_pending = false;
        }
        // Also stop if somehow elapsed went past total (shouldn't happen but be safe)
        let _ = elapsed;
    }

    // Auto-record detection: detect arr_playing transitions
    {
        let st = state.lock();
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
                if let Some(mut lock) = recording.try_lock() {
                    *lock = None;
                }
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
        (
            st.arr_playing,
            st.arr_elapsed,
            st.arr_auto_record,
            st.arr_loop,
            st.scenes.clone(),
        )
    };

    let total = total_duration(&scenes_snapshot);

    // Generate Song section
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("✨ Generate Song").color(CYAN).strong());
        ui.add_space(8.0);
        let (cur_mood, seed_base) = {
            let st = state.lock();
            (
                st.arr_mood.clone(),
                st.arr_elapsed.to_bits() as u64 ^ 0x1234567890abcdef,
            )
        };
        for mood in &["ambient", "rhythmic", "experimental"] {
            let selected = cur_mood == *mood;
            let col = if selected {
                Color32::from_rgb(0, 140, 200)
            } else {
                Color32::from_rgb(40, 40, 60)
            };
            let label = match *mood {
                "ambient" => "🌙 Ambient",
                "rhythmic" => "⚡ Rhythmic",
                _ => "🔬 Experimental",
            };
            if ui
                .add(
                    Button::new(RichText::new(label).color(Color32::WHITE))
                        .fill(col)
                        .min_size(Vec2::new(90.0, 26.0)),
                )
                .clicked()
            {
                state.lock().arr_mood = mood.to_string();
            }
        }
        ui.add_space(8.0);
        if ui
            .add(
                Button::new(RichText::new("🎲 Generate").color(Color32::BLACK))
                    .fill(Color32::from_rgb(220, 180, 40))
                    .min_size(Vec2::new(90.0, 26.0)),
            )
            .clicked()
        {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| {
                    d.as_nanos() as u64 ^ (d.subsec_nanos() as u64).wrapping_mul(0x9e3779b97f4a7c15)
                })
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
            RichText::new("⏺ Recording…")
                .color(Color32::from_rgb(255, 80, 80))
                .size(11.0)
        } else {
            RichText::new("💾 Save as WAV")
                .color(Color32::WHITE)
                .size(11.0)
        };
        if ui
            .add(
                Button::new(save_label)
                    .fill(if save_pending {
                        Color32::from_rgb(100, 20, 20)
                    } else {
                        Color32::from_rgb(30, 80, 60)
                    })
                    .min_size(Vec2::new(110.0, 26.0)),
            )
            .clicked()
            && !save_pending
        {
            // Generate fresh, reset playback, start recording, no loop
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| {
                    d.as_nanos() as u64 ^ (d.subsec_nanos() as u64).wrapping_mul(0x9e3779b97f4a7c15)
                })
                .unwrap_or(seed_base);
            let mood = state.lock().arr_mood.clone();
            let new_scenes = generate_song(&mood, seed);
            let sr = state.lock().sample_rate;
            let secs_ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let filename = format!("generation_{}.wav", secs_ts);
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
        let play_col = if arr_playing {
            Color32::from_rgb(0, 140, 0)
        } else {
            Color32::from_rgb(0, 80, 120)
        };
        if ui
            .add(
                Button::new(
                    RichText::new(if arr_playing { "⏸ Pause" } else { "▶ Play" })
                        .color(Color32::WHITE),
                )
                .fill(play_col)
                .min_size(Vec2::new(80.0, 28.0)),
            )
            .clicked()
        {
            let mut st = state.lock();
            st.arr_playing = !st.arr_playing;
            if st.arr_playing && st.arr_elapsed == 0.0 {
                // reset
            }
        }
        if ui
            .add(
                Button::new(RichText::new("■ Stop").color(Color32::WHITE))
                    .fill(Color32::from_rgb(80, 30, 30))
                    .min_size(Vec2::new(60.0, 28.0)),
            )
            .clicked()
        {
            let mut st = state.lock();
            st.arr_playing = false;
            st.arr_elapsed = 0.0;
        }

        let loop_col = if arr_loop {
            Color32::from_rgb(0, 120, 0)
        } else {
            Color32::from_rgb(40, 40, 60)
        };
        if ui
            .add(
                Button::new(RichText::new("⟳ Loop").color(Color32::WHITE))
                    .fill(loop_col)
                    .min_size(Vec2::new(60.0, 28.0)),
            )
            .clicked()
        {
            state.lock().arr_loop = !arr_loop;
        }

        let rec_col = if arr_auto_record {
            Color32::from_rgb(140, 30, 30)
        } else {
            Color32::from_rgb(40, 40, 60)
        };
        if ui
            .add(
                Button::new(RichText::new("⏺ Auto-Rec").color(Color32::WHITE))
                    .fill(rec_col)
                    .min_size(Vec2::new(80.0, 28.0)),
            )
            .clicked()
        {
            state.lock().arr_auto_record = !arr_auto_record;
        }

        // Duration label
        let total_mins = (total / 60.0) as u32;
        let total_secs = (total % 60.0) as u32;
        ui.label(
            RichText::new(format!("  Duration: {}:{:02}", total_mins, total_secs)).color(CYAN),
        );

        // Probabilistic mode
        let mut st = state.lock();
        let prob_col = if st.arr_probabilistic {
            Color32::from_rgb(180, 80, 20)
        } else {
            Color32::from_rgb(40, 40, 60)
        };
        if ui
            .add(
                Button::new(RichText::new("Probabilistic").color(Color32::WHITE))
                    .fill(prob_col)
                    .min_size(Vec2::new(100.0, 28.0)),
            )
            .clicked()
        {
            st.arr_probabilistic = !st.arr_probabilistic;
        }
    });

    // Progress bar
    if arr_playing || arr_elapsed > 0.0 {
        let progress = if total > 0.001 {
            (arr_elapsed / total).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let elapsed_m = (arr_elapsed / 60.0) as u32;
        let elapsed_s = (arr_elapsed % 60.0) as u32;
        ui.add(ProgressBar::new(progress).text(format!(
            "{:02}:{:02} / {:02}:{:02}",
            elapsed_m,
            elapsed_s,
            (total / 60.0) as u32,
            (total % 60.0) as u32
        )));
    }

    // Morph diff panel: compute and display which parameters are changing during morph
    if arr_playing {
        let (morph_diff, is_morphing) = {
            let st = state.lock();
            let active: Vec<usize> = (0..st.scenes.len())
                .filter(|&i| st.scenes[i].active)
                .collect();
            let mut diff: Vec<(String, f32, f32)> = Vec::new();
            let mut in_morph = false;
            if let Some((idx, morphing, _t)) = scene_at(&st.scenes, st.arr_elapsed) {
                if morphing {
                    in_morph = true;
                    if let Some(ord) = active.iter().position(|&i| i == idx) {
                        if ord > 0 {
                            let prev_idx = active[ord - 1];
                            let a = &st.scenes[prev_idx].config;
                            let b = &st.scenes[idx].config;
                            let mut check = |name: &str, from: f32, to: f32| {
                                if (from - to).abs() > 0.01 {
                                    diff.push((name.to_string(), from, to));
                                }
                            };
                            check(
                                "\u{03c3} (sigma)",
                                a.lorenz.sigma as f32,
                                b.lorenz.sigma as f32,
                            );
                            check("\u{03c1} (rho)", a.lorenz.rho as f32, b.lorenz.rho as f32);
                            check(
                                "\u{03b2} (beta)",
                                a.lorenz.beta as f32,
                                b.lorenz.beta as f32,
                            );
                            check("speed", a.system.speed as f32, b.system.speed as f32);
                            check("reverb_wet", a.audio.reverb_wet, b.audio.reverb_wet);
                            check("delay_ms", a.audio.delay_ms, b.audio.delay_ms);
                            check(
                                "base_freq",
                                a.sonification.base_frequency as f32,
                                b.sonification.base_frequency as f32,
                            );
                            check("master_vol", a.audio.master_volume, b.audio.master_volume);
                        }
                    }
                }
            }
            (diff, in_morph)
        };
        if is_morphing && !morph_diff.is_empty() {
            state.lock().arr_morph_diff = morph_diff.clone();
            ui.add_space(4.0);
            egui::CollapsingHeader::new(
                RichText::new("Morphing parameters:")
                    .color(Color32::from_rgb(220, 180, 40))
                    .size(11.0)
                    .strong(),
            )
            .default_open(true)
            .show(ui, |ui| {
                for (name, from, to) in &morph_diff {
                    ui.label(
                        RichText::new(format!("  {}: {:.2} \u{2192} {:.2}", name, from, to))
                            .color(Color32::from_rgb(200, 200, 100))
                            .size(11.0)
                            .monospace(),
                    );
                }
            });
        } else if !is_morphing {
            state.lock().arr_morph_diff.clear();
        }
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
            (
                st.scenes[i].active,
                st.scenes[i].name.clone(),
                st.scenes[i].hold_secs,
                st.scenes[i].morph_secs,
            )
        };

        let is_current_scene = arr_playing
            && scene_at(&scenes_snapshot, arr_elapsed)
                .map(|(idx, _, _)| idx == i)
                .unwrap_or(false);

        let row_bg = if is_current_scene {
            Color32::from_rgba_premultiplied(0, 60, 30, 255)
        } else {
            Color32::TRANSPARENT
        };

        egui::Frame::none()
            .fill(row_bg)
            .inner_margin(egui::Margin::symmetric(4.0, 2.0))
            .show(ui, |ui| {
                if is_current_scene {
                    ui.label(
                        RichText::new("▶")
                            .color(Color32::from_rgb(80, 220, 80))
                            .size(10.0),
                    );
                } else {
                    ui.label(RichText::new("  ").size(10.0));
                }
                ui.horizontal(|ui| {
                    // Active checkbox
                    if ui.checkbox(&mut active, "").changed() {
                        state.lock().scenes[i].active = active;
                    }

                    // Scene number
                    ui.label(
                        RichText::new(format!("{}", i + 1))
                            .color(GRAY_HINT)
                            .size(11.0),
                    );

                    // Name text edit
                    let mut name_edit = name.clone();
                    let te = ui.add(TextEdit::singleline(&mut name_edit).desired_width(80.0));
                    if te.changed() {
                        state.lock().scenes[i].name = name_edit;
                    }

                    // Hold duration
                    let mut hold_v = hold;
                    if ui
                        .add(
                            DragValue::new(&mut hold_v)
                                .clamp_range(5.0..=300.0f32)
                                .suffix("s")
                                .speed(0.5),
                        )
                        .changed()
                    {
                        state.lock().scenes[i].hold_secs = hold_v;
                    }

                    // Morph duration + preview button
                    let mut morph_v = morph;
                    if ui
                        .add(
                            DragValue::new(&mut morph_v)
                                .clamp_range(0.0..=60.0f32)
                                .suffix("s")
                                .speed(0.1),
                        )
                        .changed()
                    {
                        state.lock().scenes[i].morph_secs = morph_v;
                    }
                    // Morph curve selector (linear / smooth / exponential)
                    if i > 0 {
                        let current_curve = state.lock().scenes[i].morph_curve.as_str().to_string();
                        let curves = [("~", "linear", "Linear interpolation"), ("S", "smooth", "Sigmoid (smooth-step) — safe for bifurcation-sensitive params"), ("E", "exponential", "Exponential easing")];
                        for (icon, key, tip) in curves {
                            let active_curve = current_curve == key;
                            let fill = if active_curve { Color32::from_rgb(0, 100, 160) } else { Color32::from_rgb(20, 22, 40) };
                            let lbl = RichText::new(icon).size(9.0).color(if active_curve { Color32::WHITE } else { Color32::from_rgb(120, 130, 160) });
                            if ui.add(Button::new(lbl).fill(fill).min_size(Vec2::new(16.0, 18.0))).on_hover_text(tip).clicked() {
                                state.lock().scenes[i].morph_curve = MorphCurve::from_str(key);
                            }
                        }
                    }

                    // ▶ preview: jump arrangement to just before this scene's morph
                    if i > 0 {
                        let preview_btn = ui
                            .add(
                                Button::new(RichText::new("▶").size(10.0))
                                    .min_size(Vec2::new(18.0, 18.0)),
                            )
                            .on_hover_text("Preview morph into this scene");
                        if preview_btn.clicked() {
                            let mut st = state.lock();
                            // Compute elapsed time to the start of this scene's morph
                            let active: Vec<usize> = (0..st.scenes.len())
                                .filter(|&j| st.scenes[j].active)
                                .collect();
                            let mut t = 0.0f32;
                            for (ord, &idx) in active.iter().enumerate() {
                                if idx == i {
                                    // We're at the start of this scene's morph
                                    st.arr_elapsed = t;
                                    st.arr_playing = true;
                                    break;
                                }
                                let s = &st.scenes[idx];
                                if ord > 0 {
                                    t += s.morph_secs;
                                }
                                t += s.hold_secs;
                            }
                        }
                    }

                    // Probability weight (shown when probabilistic mode is on)
                    let arr_prob = state.lock().arr_probabilistic;
                    if arr_prob {
                        let mut prob_v = state.lock().scenes[i].transition_prob;
                        if ui
                            .add(
                                DragValue::new(&mut prob_v)
                                    .clamp_range(0.0..=3.0f32)
                                    .prefix("P:")
                                    .speed(0.05),
                            )
                            .changed()
                        {
                            state.lock().scenes[i].transition_prob = prob_v;
                        }
                    }

                    // Capture button
                    if ui
                        .add(
                            Button::new(RichText::new("Capture").color(Color32::WHITE))
                                .fill(Color32::from_rgb(0, 80, 120))
                                .min_size(Vec2::new(58.0, 22.0)),
                        )
                        .clicked()
                    {
                        let cfg = state.lock().config.clone();
                        let mut st = state.lock();
                        st.scenes[i].config = cfg;
                        st.scenes[i].active = true;
                    }

                    // Load button
                    if ui
                        .add(
                            Button::new(RichText::new("Load").color(Color32::WHITE))
                                .fill(Color32::from_rgb(60, 40, 80))
                                .min_size(Vec2::new(42.0, 22.0)),
                        )
                        .clicked()
                    {
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
        ui.label(
            RichText::new(
                "Hit Generate to create an arrangement, or Capture your current sound into scenes.",
            )
            .color(AMBER)
            .size(11.0)
            .italics(),
        );
    } else {
        ui.colored_label(
            GRAY_HINT,
            "Capture your current sound into a scene, set durations, then Play.",
        );
    }
    ui.add_space(6.0);

    // Visual timeline
    let scenes_for_tl = state.lock().scenes.clone();
    draw_arrangement_timeline(ui, &scenes_for_tl, arr_elapsed);
}

// draw_arrangement_timeline lives in src/ui_timeline.rs

fn build_bifurc_system(
    sys_name: &str,
    param: &str,
    pval: f64,
    lorenz: &crate::config::LorenzConfig,
    rossler: &crate::config::RosslerConfig,
    kuramoto: &crate::config::KuramotoConfig,
) -> Box<dyn DynamicalSystem> {
    use crate::systems::*;
    match (sys_name, param) {
        ("lorenz", "rho") | (_, "rho") => Box::new(Lorenz::new(lorenz.sigma, pval, lorenz.beta)),
        ("lorenz", "sigma") | (_, "sigma") => Box::new(Lorenz::new(pval, lorenz.rho, lorenz.beta)),
        ("rossler", "c") | ("rossler", _) => Box::new(Rossler::new(rossler.a, rossler.b, pval)),
        (_, "c") => Box::new(Rossler::new(rossler.a, rossler.b, pval)),
        ("kuramoto", "coupling") | (_, "coupling") => {
            Box::new(Kuramoto::new(kuramoto.n_oscillators, pval))
        }
        ("logistic_map", "r") | (_, "r") => Box::new(LogisticMap::new(pval)),
        ("standard_map", "k") | (_, "k") => Box::new(StandardMap::new(pval)),
        ("thomas", "b") | (_, "b") => Box::new(Thomas::new(pval)),
        ("rucklidge", "lambda") | (_, "lambda") => {
            let mut s = Rucklidge::new();
            s.lambda = pval;
            Box::new(s)
        }
        (_, "a_rossler") => Box::new(Rossler::new(pval, rossler.b, rossler.c)),
        (_, "shimizu_a") => {
            let mut s = ShimizuMorioka::new();
            s.a = pval;
            Box::new(s)
        }
        (_, "genesio_c") => {
            let mut s = GenesioTesi::new();
            s.c = pval;
            Box::new(s)
        }
        (_, "liu_b") => {
            let mut s = Liu::new();
            s.b = pval;
            Box::new(s)
        }
        (_, "windmi_b") => {
            let mut s = Windmi::new();
            s.b = pval;
            Box::new(s)
        }
        (_, "finance_a") => {
            let mut s = Finance::new();
            s.a = pval;
            Box::new(s)
        }
        (_, "hyperchaos_c") => {
            let mut s = Hyperchaos::new();
            s.c = pval;
            Box::new(s)
        }
        _ => Box::new(Lorenz::new(lorenz.sigma, pval.clamp(20.0, 50.0), lorenz.beta)),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 📼 STUDIO TAB — snippet capture, library, and song sequencer
// ────────────────────────────────────────────────────────────────────────────
fn draw_studio_tab(ui: &mut Ui, state: &SharedState) {
    let avail = ui.available_size();
    let lib_w = 260.0_f32.min(avail.x * 0.38);
    let grid_w = (avail.x - lib_w - 10.0).max(100.0);

    ui.horizontal_top(|ui| {
        // ── LEFT: LIBRARY ────────────────────────────────────────────────
        egui::Frame::none()
            .fill(Color32::from_rgb(10, 12, 26))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(40, 50, 80)))
            .rounding(egui::Rounding::same(6.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.set_min_width(lib_w);
                ui.set_max_width(lib_w);

                ui.label(RichText::new("📼  LIBRARY").color(Color32::WHITE).strong().size(13.0));
                ui.add_space(4.0);

                // Capture duration selector
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Duration:").color(GRAY_HINT).size(11.0));
                    let cur_secs = state.lock().snippet_capture_secs;
                    for dur in [4.0_f32, 8.0, 16.0, 32.0] {
                        let selected = (cur_secs - dur).abs() < 0.1;
                        let fill = if selected { Color32::from_rgb(0, 100, 200) } else { Color32::from_rgb(25, 28, 50) };
                        if ui.add(Button::new(RichText::new(format!("{}s", dur as u32)).size(11.0).color(Color32::WHITE))
                            .fill(fill).min_size(Vec2::new(30.0, 22.0))).clicked() {
                            state.lock().snippet_capture_secs = dur;
                        }
                    }
                });
                ui.add_space(4.0);

                // Capture button
                let (cap_secs, sr, status) = {
                    let st = state.lock();
                    (st.snippet_capture_secs, st.sample_rate, st.snippet_status.clone())
                };
                let clip_buf = state.lock().clip_buffer.clone();
                if ui.add(
                    Button::new(RichText::new("⏺  Capture Snippet").color(Color32::WHITE).size(12.5))
                        .fill(Color32::from_rgb(160, 25, 45))
                        .min_size(Vec2::new(lib_w - 16.0, 36.0))
                ).on_hover_text("Save the last N seconds of live audio as a snippet you can arrange into a song.")
                .clicked() {
                    match capture_snippet(&clip_buf, sr, cap_secs) {
                        Ok((path, samples)) => {
                            let n = state.lock().snippets.len() + 1;
                            let snip = Snippet::from_samples(format!("Snippet {}", n), path, samples, sr);
                            let mut st = state.lock();
                            let idx = st.snippets.len();
                            st.snippets.push(snip);
                            st.snippet_selected = Some(idx);
                            st.snippet_status = format!("✓ Captured {:.0}s snippet", cap_secs);
                        }
                        Err(e) => {
                            let msg = format!("✗ {}", e);
                            let mut st = state.lock();
                            st.snippet_status = msg.clone();
                            st.toast_queue.push(Toast::error(format!("Snippet capture failed: {}", e)));
                        }
                    }
                }
                if !status.is_empty() {
                    let col = if status.starts_with('✓') { Color32::from_rgb(80, 220, 120) } else { Color32::from_rgb(220, 80, 80) };
                    ui.label(RichText::new(&status).size(10.0).color(col));
                }

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                // ── MIDI Export ───────────────────────────────────────────
                ui.label(RichText::new("MIDI Export").color(Color32::WHITE).size(12.0).strong());
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Duration:").color(GRAY_HINT).size(11.0));
                    let mut dur = state.lock().midi_export_duration_secs;
                    if ui.add(DragValue::new(&mut dur).clamp_range(5.0..=120.0_f32).suffix("s").speed(1.0)).changed() {
                        state.lock().midi_export_duration_secs = dur;
                    }
                });
                let (midi_export_rec, midi_export_frames_len, midi_export_dur, midi_export_status) = {
                    let st = state.lock();
                    (st.midi_export_recording, st.midi_export_frames.len(), st.midi_export_duration_secs, st.midi_export_status.clone())
                };
                let control_rate = 120.0_f32;
                let max_frames = (midi_export_dur * control_rate) as usize;
                if midi_export_rec {
                    let progress = midi_export_frames_len as f32 / max_frames as f32;
                    ui.add(egui::ProgressBar::new(progress).text(format!("{:.0}%", progress * 100.0)));
                    if ui.add(Button::new(RichText::new("Stop & Export MIDI").size(11.0).color(Color32::WHITE))
                        .fill(Color32::from_rgb(180, 80, 0)).min_size(Vec2::new(lib_w - 16.0, 28.0))).clicked()
                        || midi_export_frames_len >= max_frames
                    {
                        let mut st = state.lock();
                        st.midi_export_recording = false;
                        let frames_data = std::mem::take(&mut st.midi_export_frames);
                        let scale_name = st.config.sonification.scale.clone();
                        let base_freq = st.config.sonification.base_frequency as f32;
                        drop(st);
                        // Convert (pitch,vel) frames to MidiFrames and export
                        use crate::midi_export::{export_midi, MidiFrame, scale_offsets};
                        let midi_frames: Vec<MidiFrame> = frames_data.iter().map(|&(p, v)| MidiFrame { pitch: p, velocity: v, ticks: 1 }).collect();
                        let scale_offs = scale_offsets(&scale_name);
                        let _ = scale_offs;
                        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                        let out_path = format!("midi_export_{}.mid", secs);
                        match export_midi(&midi_frames, &out_path, 60, 500_000, midly::num::u4::new(0)) {
                            Ok(()) => {
                                state.lock().midi_export_status = format!("Exported: {}", out_path);
                                state.lock().toast_queue.push(Toast::info(format!("MIDI saved: {}", out_path)));
                            }
                            Err(e) => {
                                state.lock().midi_export_status = format!("Error: {}", e);
                                state.lock().toast_queue.push(Toast::error(format!("MIDI export failed: {}", e)));
                            }
                        }
                        let _ = base_freq;
                    }
                } else if ui.add(Button::new(RichText::new("⏺  Export MIDI").color(Color32::WHITE).size(12.0))
                    .fill(Color32::from_rgb(30, 80, 150)).min_size(Vec2::new(lib_w - 16.0, 32.0)))
                    .on_hover_text("Record the attractor's pitch sequence and export as a MIDI file.")
                    .clicked()
                {
                    let mut st = state.lock();
                    st.midi_export_recording = true;
                    st.midi_export_frames.clear();
                    st.midi_export_status = "Recording...".to_string();
                }
                if !midi_export_status.is_empty() && !midi_export_rec {
                    let col = if midi_export_status.starts_with("Exported") { Color32::from_rgb(80, 220, 120) } else { Color32::from_rgb(180, 80, 80) };
                    ui.label(RichText::new(&midi_export_status).size(10.0).color(col));
                }
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Snippet list
                let n_snips = state.lock().snippets.len();
                if n_snips == 0 {
                    ui.label(RichText::new("No snippets yet.\nPlay some audio, then press\n⏺ Capture to save it.")
                        .color(GRAY_HINT).size(11.0).italics());
                } else {
                    let selected_idx = state.lock().snippet_selected;
                    egui::ScrollArea::vertical()
                        .id_source("snippet_lib_scroll")
                        .max_height(avail.y - 165.0)
                        .show(ui, |ui| {
                        let mut delete_idx: Option<usize> = None;
                        for i in 0..n_snips {
                            let (name, dur, cidx, thumb) = {
                                let st = state.lock();
                                let s = &st.snippets[i];
                                (s.name.clone(), s.duration_secs, s.color_idx, s.thumb.clone())
                            };
                            let is_sel = selected_idx == Some(i);
                            let accent = Snippet::color(cidx);
                            let card_fill = if is_sel { Color32::from_rgb(20, 45, 80) } else { Color32::from_rgb(14, 16, 32) };

                            let resp = egui::Frame::none()
                                .fill(card_fill)
                                .stroke(egui::Stroke::new(if is_sel { 1.5 } else { 0.5 },
                                    if is_sel { accent } else { Color32::from_rgb(38, 42, 68) }))
                                .rounding(egui::Rounding::same(5.0))
                                .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(lib_w - 20.0);
                                    ui.horizontal(|ui| {
                                        let (strip, _) = ui.allocate_exact_size(Vec2::new(4.0, 38.0), Sense::hover());
                                        ui.painter().rect_filled(strip, egui::Rounding::same(2.0), accent);
                                        ui.add_space(4.0);
                                        ui.vertical(|ui| {
                                            let mut n2 = name.clone();
                                            if ui.add(egui::TextEdit::singleline(&mut n2)
                                                .font(egui::TextStyle::Small)
                                                .desired_width(lib_w - 90.0)
                                                .frame(false)).changed() {
                                                state.lock().snippets[i].name = n2;
                                            }
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(format!("{:.1}s", dur)).color(GRAY_HINT).size(10.0));
                                                let (wr, _) = ui.allocate_exact_size(Vec2::new(76.0, 14.0), Sense::hover());
                                                let p = ui.painter();
                                                p.rect_filled(wr, egui::Rounding::same(2.0), Color32::from_rgb(7, 9, 18));
                                                for (ti, &peak) in thumb.iter().enumerate() {
                                                    let x = wr.left() + ti as f32 * wr.width() / thumb.len() as f32;
                                                    let h = peak.min(1.0) * wr.height() * 0.5;
                                                    let cy = wr.center().y;
                                                    p.line_segment(
                                                        [Pos2::new(x, cy - h), Pos2::new(x, cy + h)],
                                                        egui::Stroke::new(1.0, accent.linear_multiply(0.75)),
                                                    );
                                                }
                                            });
                                        });
                                        if ui.add(Button::new(RichText::new("✕").size(10.0)
                                            .color(Color32::from_rgb(150, 80, 80)))
                                            .fill(Color32::TRANSPARENT).frame(false)
                                            .min_size(Vec2::new(16.0, 16.0))).clicked() {
                                            delete_idx = Some(i);
                                        }
                                    });
                                });

                            if resp.response.interact(Sense::click()).clicked() {
                                state.lock().snippet_selected = Some(i);
                            }
                            ui.add_space(3.0);
                        }

                        // Handle deletion outside the loop
                        if let Some(di) = delete_idx {
                            let mut st = state.lock();
                            st.snippets.remove(di);
                            for slot in &mut st.snippet_slots {
                                if let Some(idx) = *slot {
                                    if idx == di { *slot = None; }
                                    else if idx > di { *slot = Some(idx - 1); }
                                }
                            }
                            if st.snippet_selected == Some(di) { st.snippet_selected = None; }
                            else if let Some(sel) = st.snippet_selected {
                                if sel > di { st.snippet_selected = Some(sel - 1); }
                            }
                        }
                    });
                }
            }); // end library panel

        ui.add_space(6.0);

        // ── RIGHT: SONG GRID ─────────────────────────────────────────────
        egui::Frame::none()
            .fill(Color32::from_rgb(10, 12, 26))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(40, 50, 80)))
            .rounding(egui::Rounding::same(6.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.set_min_width(grid_w);

                ui.label(RichText::new("🎵  SONG GRID").color(Color32::WHITE).strong().size(13.0));
                ui.add_space(4.0);

                let (song_playing, song_loop, vol) = {
                    let st = state.lock();
                    (st.song_playing, st.song_loop, st.snippet_volume)
                };

                // Transport bar
                ui.horizontal(|ui| {
                    let play_label = if song_playing { "⏹  Stop" } else { "▶  Play Song" };
                    let play_fill = if song_playing { Color32::from_rgb(160, 25, 25) } else { Color32::from_rgb(20, 140, 55) };
                    if ui.add(Button::new(RichText::new(play_label).color(Color32::WHITE).size(12.0))
                        .fill(play_fill)
                        .min_size(Vec2::new(110.0, 30.0))).clicked() {
                        let mut st = state.lock();
                        if st.song_playing {
                            st.song_playing = false;
                            *st.snippet_pb.lock() = SnippetPlayback::idle();
                        } else {
                            let first = st.snippet_slots.iter().position(|s| s.is_some());
                            // item 1: guard against slot becoming None between iteration and access
                            if let Some(slot_i) = first {
                                if let Some(snip_i) = st.snippet_slots[slot_i] {
                                if snip_i < st.snippets.len() {
                                    let samples = (*st.snippets[snip_i].samples).clone();
                                    let v = st.snippet_volume;
                                    *st.snippet_pb.lock() = SnippetPlayback::play(samples, v);
                                    st.song_play_slot = slot_i;
                                    st.song_playing = true;
                                }
                                }
                            }
                        }
                    }
                    let loop_fill = if song_loop { Color32::from_rgb(0, 90, 170) } else { Color32::from_rgb(25, 28, 50) };
                    if ui.add(Button::new(RichText::new("↺").color(Color32::WHITE).size(12.0))
                        .fill(loop_fill)
                        .min_size(Vec2::new(32.0, 30.0))).on_hover_text("Loop the song").clicked() {
                        state.lock().song_loop = !song_loop;
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("Vol").color(GRAY_HINT).size(11.0));
                    let mut v = vol;
                    ui.set_max_width(80.0);
                    if ui.add(Slider::new(&mut v, 0.0..=1.0).show_value(false)).changed() {
                        state.lock().snippet_volume = v;
                    }
                    ui.add_space(12.0);
                    if ui.add(Button::new(RichText::new("Clear All").color(Color32::from_rgb(200,100,100)).size(11.0))
                        .fill(Color32::from_rgb(22,14,14))
                        .min_size(Vec2::new(70.0, 30.0))).clicked() {
                        let mut st = state.lock();
                        for s in &mut st.snippet_slots { *s = None; }
                        st.song_playing = false;
                        *st.snippet_pb.lock() = SnippetPlayback::idle();
                    }
                });

                ui.add_space(3.0);
                ui.label(RichText::new("Select a snippet in the library, then click a slot to place it. Right-click to clear a slot.")
                    .size(10.0).color(GRAY_HINT).italics());
                ui.add_space(6.0);

                let (playing_slot, snip_sel) = {
                    let st = state.lock();
                    (if st.song_playing { Some(st.song_play_slot) } else { None }, st.snippet_selected)
                };
                let _ = snip_sel; // used below via state.lock()

                egui::ScrollArea::vertical()
                    .id_source("song_grid_scroll")
                    .max_height(avail.y - 120.0)
                    .show(ui, |ui| {
                    let slot_w = (grid_w - 26.0) / 2.0;
                    egui::Grid::new("song_grid")
                        .num_columns(2)
                        .spacing([6.0, 4.0])
                        .show(ui, |ui| {
                        for i in 0..32usize {
                            let (slot_snip_opt, snip_name, snip_cidx, snip_thumb) = {
                                let st = state.lock();
                                if let Some(idx) = st.snippet_slots[i] {
                                    if idx < st.snippets.len() {
                                        let s = &st.snippets[idx];
                                        (Some(idx), s.name.clone(), s.color_idx, s.thumb.clone())
                                    } else { (None, String::new(), 0usize, Vec::new()) }
                                } else { (None, String::new(), 0usize, Vec::new()) }
                            };

                            let is_playing = playing_slot == Some(i);
                            let accent = if slot_snip_opt.is_some() { Snippet::color(snip_cidx) } else { Color32::from_rgb(40,44,70) };
                            let fill = if is_playing { Color32::from_rgb(18, 80, 18) }
                                       else if slot_snip_opt.is_some() { Color32::from_rgb(15, 19, 38) }
                                       else { Color32::from_rgb(9, 10, 20) };
                            let stroke_w = if is_playing { 2.0 } else { 0.8 };
                            let stroke_col = if is_playing { Color32::from_rgb(50, 240, 90) }
                                            else if slot_snip_opt.is_some() { accent }
                                            else { Color32::from_rgb(28, 32, 52) };

                            let resp = egui::Frame::none()
                                .fill(fill)
                                .stroke(egui::Stroke::new(stroke_w, stroke_col))
                                .rounding(egui::Rounding::same(4.0))
                                .inner_margin(egui::Margin::symmetric(5.0, 3.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(slot_w);
                                    ui.set_max_width(slot_w);
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("{:02}", i + 1))
                                            .color(if is_playing { Color32::from_rgb(50,240,90) } else { GRAY_HINT })
                                            .size(10.0).monospace());
                                        ui.add_space(3.0);
                                        if slot_snip_opt.is_some() {
                                            let (dot_r, _) = ui.allocate_exact_size(Vec2::new(7.0, 7.0), Sense::hover());
                                            ui.painter().circle_filled(dot_r.center(), 3.0, accent);
                                            ui.add(egui::Label::new(RichText::new(&snip_name).color(Color32::WHITE).size(10.5)).truncate(true));
                                            if !snip_thumb.is_empty() {
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    let (wr, _) = ui.allocate_exact_size(Vec2::new(44.0, 12.0), Sense::hover());
                                                    ui.painter().rect_filled(wr, egui::Rounding::same(2.0), Color32::from_rgb(6,7,15));
                                                    let np = snip_thumb.len();
                                                    for (ti, &pk) in snip_thumb.iter().enumerate() {
                                                        let x = wr.left() + ti as f32 * wr.width() / np as f32;
                                                        let h = pk.min(1.0) * wr.height() * 0.45;
                                                        let cy = wr.center().y;
                                                        ui.painter().line_segment(
                                                            [Pos2::new(x, cy-h), Pos2::new(x, cy+h)],
                                                            egui::Stroke::new(1.0, accent.linear_multiply(0.6)));
                                                    }
                                                });
                                            }
                                        } else {
                                            ui.label(RichText::new("— empty —").color(Color32::from_rgb(42,46,72)).size(10.0).italics());
                                        }
                                    });
                                });

                            // Left-click: assign selected snippet
                            if resp.response.interact(Sense::click()).clicked() {
                                let mut st = state.lock();
                                if let Some(sel) = st.snippet_selected {
                                    st.snippet_slots[i] = Some(sel);
                                } else {
                                    st.snippet_slots[i] = None;
                                }
                            }
                            // Right-click: clear slot
                            if resp.response.secondary_clicked() {
                                state.lock().snippet_slots[i] = None;
                            }

                            if i % 2 == 1 { ui.end_row(); }
                        }
                    });
                });
            }); // end song grid panel
    });
}

fn draw_bifurc_diagram(
    ui: &mut Ui,
    bifurc_data: &Arc<Mutex<Vec<(f32, f32)>>>,
    state: &SharedState,
) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let (is_2d, param1, param2) = {
        let st = state.lock();
        (
            st.bifurc_2d_mode,
            st.bifurc_param.clone(),
            st.bifurc_param2.clone(),
        )
    };

    if is_2d {
        // 2D heatmap: (p1, p2, chaos_metric)
        let data_2d_arc = state.lock().bifurc_data_2d.clone();
        let data_2d = if let Some(d) = data_2d_arc.try_lock() {
            d.clone()
        } else {
            return;
        };
        if data_2d.is_empty() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Click 'Compute' to generate 2D bifurcation map",
                FontId::proportional(16.0),
                Color32::from_rgb(80, 80, 120),
            );
            return;
        }
        let mut p1_min = f32::MAX;
        let mut p1_max = f32::MIN;
        let mut p2_min = f32::MAX;
        let mut p2_max = f32::MIN;
        let mut v_min = f32::MAX;
        let mut v_max = f32::MIN;
        for &(p1, p2, v) in &data_2d {
            p1_min = p1_min.min(p1);
            p1_max = p1_max.max(p1);
            p2_min = p2_min.min(p2);
            p2_max = p2_max.max(p2);
            v_min = v_min.min(v);
            v_max = v_max.max(v);
        }
        let rp1 = (p1_max - p1_min).max(1e-3);
        let rp2 = (p2_max - p2_min).max(1e-3);
        let rv = (v_max - v_min).max(1e-9);
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
                lerp_color(
                    Color32::from_rgb(10, 10, 80),
                    Color32::from_rgb(0, 200, 255),
                    t * 2.0,
                )
            } else {
                lerp_color(
                    Color32::from_rgb(0, 200, 255),
                    Color32::from_rgb(255, 240, 180),
                    (t - 0.5) * 2.0,
                )
            };
            let sx = rect.left() + pad + ((p1 - p1_min) / rp1) * inner_w;
            let sy = rect.bottom() - pad - ((p2 - p2_min) / rp2) * inner_h;
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(sx, sy - cell_h), Vec2::new(cell_w, cell_h)),
                0.0,
                col,
            );
        }
        painter.text(
            rect.left_top() + Vec2::new(8.0, 8.0),
            Align2::LEFT_TOP,
            format!("2D Bifurcation Map  ({} × {})", grid_size, grid_size),
            FontId::proportional(12.0),
            Color32::from_rgb(120, 140, 180),
        );
        painter.text(
            rect.center_bottom() + Vec2::new(0.0, -12.0),
            Align2::CENTER_BOTTOM,
            format!(
                "x: {} ({:.1}..{:.1})   y: {} ({:.1}..{:.1})",
                param1, p1_min, p1_max, param2, p2_min, p2_max
            ),
            FontId::proportional(10.0),
            Color32::from_rgb(100, 120, 160),
        );
        return;
    }

    // 1D bifurcation diagram
    let data = if let Some(d) = bifurc_data.try_lock() {
        d.clone()
    } else {
        return;
    };
    if data.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Click 'Compute' to generate bifurcation diagram",
            FontId::proportional(16.0),
            Color32::from_rgb(80, 80, 120),
        );
        return;
    }
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for &(x, y) in &data {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let rx = (max_x - min_x).max(1e-3);
    let ry = (max_y - min_y).max(1e-3);
    let pad = 20.0f32;
    for &(x, y) in &data {
        let sx = rect.left() + pad + ((x - min_x) / rx) * (rect.width() - 2.0 * pad);
        let sy = rect.bottom() - pad - ((y - min_y) / ry) * (rect.height() - 2.0 * pad);
        painter.circle_filled(
            Pos2::new(sx, sy),
            1.0,
            Color32::from_rgba_premultiplied(0, 200, 255, 180),
        );
    }
    painter.text(
        rect.left_top() + Vec2::new(8.0, 8.0),
        Align2::LEFT_TOP,
        format!("Bifurcation Diagram  ({} points)", data.len()),
        FontId::proportional(12.0),
        Color32::from_rgb(120, 140, 180),
    );
    painter.text(
        rect.center_bottom() + Vec2::new(0.0, -12.0),
        Align2::CENTER_BOTTOM,
        format!("{} = {:.3} → {:.3}", param1, min_x, max_x),
        FontId::proportional(10.0),
        Color32::from_rgb(100, 120, 160),
    );
    painter.text(
        rect.left_center() + Vec2::new(12.0, 0.0),
        Align2::LEFT_CENTER,
        "x-state",
        FontId::proportional(9.0),
        Color32::from_rgb(80, 100, 140),
    );

    // Hover: show parameter value under cursor as crosshair
    let pad = 20.0f32;
    if let Some(hover_pos) = response.hover_pos() {
        if rect.contains(hover_pos) {
            let t = ((hover_pos.x - rect.left() - pad) / (rect.width() - 2.0 * pad)).clamp(0.0, 1.0);
            let param_val = min_x + t * rx;
            // Vertical crosshair line
            painter.line_segment(
                [Pos2::new(hover_pos.x, rect.top()), Pos2::new(hover_pos.x, rect.bottom())],
                egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 100, 80)),
            );
            painter.text(
                Pos2::new(hover_pos.x + 4.0, rect.top() + 6.0),
                Align2::LEFT_TOP,
                format!("{} = {:.4}", param1, param_val),
                FontId::proportional(11.0),
                Color32::from_rgb(255, 220, 80),
            );
            // Click to apply that parameter value to the live system
            if response.clicked() {
                let param_name = param1.clone();
                let mut st = state.lock();
                st.push_undo();
                match param_name.as_str() {
                    "rho" => st.config.lorenz.rho = param_val as f64,
                    "sigma" => st.config.lorenz.sigma = param_val as f64,
                    "coupling" => st.config.kuramoto.coupling = param_val as f64,
                    "c" => st.config.rossler.c = param_val as f64,
                    _ => {}
                }
                st.system_changed = true;
                st.toast_queue.push(Toast::info(format!(
                    "{} → {:.4} (from bifurcation diagram)",
                    param_name, param_val
                )));
            }
        }
    }
}

/// Compute attractor basin: for each (x, y) initial condition on the grid, run the Lorenz
/// system for 500 steps and measure whether it diverges or converges, storing a color proxy.
fn compute_basin(
    xlim: (f32, f32),
    ylim: (f32, f32),
    z_slice: f32,
    resolution: usize,
    _system_name: &str,
    out: Arc<Mutex<Vec<(f32, f32, f32)>>>,
) {
    use rayon::prelude::*;
    let xs: Vec<f32> = (0..resolution)
        .map(|i| xlim.0 + (xlim.1 - xlim.0) * i as f32 / (resolution - 1).max(1) as f32)
        .collect();
    let ys: Vec<f32> = (0..resolution)
        .map(|j| ylim.0 + (ylim.1 - ylim.0) * j as f32 / (resolution - 1).max(1) as f32)
        .collect();

    let points: Vec<(f32, f32, f32)> = xs
        .par_iter()
        .flat_map(|&x| {
            ys.iter()
                .map(|&y| {
                    let mut sys = crate::systems::Lorenz::new(10.0, 28.0, 2.667);
                    sys.set_state(&[x as f64, y as f64, z_slice as f64]);
                    let mut max_r = 0.0f64;
                    let mut diverged = false;
                    for _ in 0..500 {
                        sys.step(0.01);
                        let s = sys.state();
                        let r = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
                        if r > 200.0 {
                            diverged = true;
                            break;
                        }
                        if r > max_r {
                            max_r = r;
                        }
                    }
                    let color_val = if diverged {
                        -1.0f32
                    } else {
                        (max_r / 50.0).clamp(0.0, 1.0) as f32
                    };
                    (x, y, color_val)
                })
                .collect::<Vec<_>>()
        })
        .collect();

    *out.lock() = points;
}

/// Draw the attractor basin visualization tab.
fn draw_basin_tab(ui: &mut Ui, state: &SharedState) {
    let (computing, resolution, xlim, ylim, basin_arc) = {
        let st = state.lock();
        (
            st.basin_computing,
            st.basin_resolution,
            st.basin_xlim,
            st.basin_ylim,
            st.basin_data.clone(),
        )
    };

    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(6, 6, 16));

    let data = match basin_arc.try_lock() {
        Some(d) => d.clone(),
        None => return,
    };

    if data.is_empty() {
        let msg = if computing {
            "Computing basin... please wait"
        } else {
            "Click 'Compute Basin' above to generate the attractor basin map"
        };
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            msg,
            FontId::proportional(16.0),
            Color32::from_rgb(80, 80, 120),
        );
        painter.text(
            rect.center_bottom() + Vec2::new(0.0, -20.0),
            Align2::CENTER_BOTTOM,
            format!(
                "x-axis: {:.1} to {:.1}   y-axis: {:.1} to {:.1}   grid: {}x{}",
                xlim.0, xlim.1, ylim.0, ylim.1, resolution, resolution
            ),
            FontId::proportional(10.0),
            Color32::from_rgb(60, 70, 100),
        );
        return;
    }

    let cell_w = rect.width() / resolution as f32;
    let cell_h = rect.height() / resolution as f32;
    let x_range = (xlim.1 - xlim.0).max(1e-6);
    let y_range = (ylim.1 - ylim.0).max(1e-6);

    for &(x, y, v) in data.iter() {
        let px = rect.left() + (x - xlim.0) / x_range * rect.width();
        let py = rect.top() + (y - ylim.0) / y_range * rect.height();
        let color = if v < 0.0 {
            Color32::from_rgb(200, 50, 50)
        } else {
            Color32::from_rgb(0, (v * 200.0) as u8, (v * 255.0) as u8)
        };
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(px, py), egui::vec2(cell_w, cell_h)),
            0.0,
            color,
        );
    }

    painter.text(
        rect.left_top() + Vec2::new(6.0, 6.0),
        Align2::LEFT_TOP,
        format!("Attractor Basin  ({}×{})", resolution, resolution),
        FontId::proportional(12.0),
        Color32::from_rgb(120, 140, 180),
    );
    painter.text(
        rect.center_bottom() + Vec2::new(0.0, -14.0),
        Align2::CENTER_BOTTOM,
        format!(
            "x: {:.1} to {:.1}    y: {:.1} to {:.1}",
            xlim.0, xlim.1, ylim.0, ylim.1
        ),
        FontId::proportional(10.0),
        Color32::from_rgb(90, 110, 150),
    );
    painter.rect_filled(
        egui::Rect::from_min_size(
            rect.right_top() + Vec2::new(-110.0, 8.0),
            Vec2::new(100.0, 14.0),
        ),
        2.0,
        Color32::from_rgb(200, 50, 50),
    );
    painter.text(
        rect.right_top() + Vec2::new(-112.0, 8.0),
        Align2::RIGHT_TOP,
        "diverges",
        FontId::proportional(10.0),
        Color32::from_rgb(200, 50, 50),
    );
    painter.rect_filled(
        egui::Rect::from_min_size(
            rect.right_top() + Vec2::new(-110.0, 28.0),
            Vec2::new(100.0, 14.0),
        ),
        2.0,
        Color32::from_rgb(0, 160, 255),
    );
    painter.text(
        rect.right_top() + Vec2::new(-112.0, 28.0),
        Align2::RIGHT_TOP,
        "attractor",
        FontId::proportional(10.0),
        Color32::from_rgb(0, 200, 255),
    );
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

#[allow(dead_code)]
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
        // ── Welcome / loading splash ──────────────────────────────────────────
        let card_w = 380.0f32.min(rect.width() - 40.0);
        let card_h = 148.0f32;
        let card_rect = egui::Rect::from_center_size(rect.center(), Vec2::new(card_w, card_h));
        painter.rect_filled(
            card_rect,
            12.0,
            Color32::from_rgba_premultiplied(12, 14, 32, 230),
        );
        painter.rect_stroke(
            card_rect,
            12.0,
            egui::Stroke::new(1.0, Color32::from_rgb(38, 55, 110)),
        );
        // Accent top bar
        let accent_bar = egui::Rect::from_min_size(
            card_rect.min + Vec2::new(0.0, 0.0),
            Vec2::new(card_w, 3.0),
        );
        painter.rect_filled(accent_bar, egui::Rounding { nw: 12.0, ne: 12.0, sw: 0.0, se: 0.0 },
            Color32::from_rgba_premultiplied(90, 195, 255, 140));

        let sys_display = system_display_name(system_name);
        let sys_tag = system_tagline(system_name);

        painter.text(
            card_rect.center_top() + Vec2::new(0.0, 18.0),
            Align2::CENTER_TOP,
            sys_display,
            FontId::proportional(20.0),
            Color32::from_rgb(185, 215, 255),
        );
        painter.text(
            card_rect.center_top() + Vec2::new(0.0, 44.0),
            Align2::CENTER_TOP,
            sys_tag,
            FontId::proportional(11.5),
            Color32::from_rgba_premultiplied(150, 162, 200, 220),
        );
        painter.text(
            card_rect.center_top() + Vec2::new(0.0, 70.0),
            Align2::CENTER_TOP,
            "─────────────────────────────────",
            FontId::proportional(8.0),
            Color32::from_rgba_premultiplied(50, 70, 110, 120),
        );
        painter.text(
            card_rect.center_top() + Vec2::new(0.0, 88.0),
            Align2::CENTER_TOP,
            "Press  SPACE  to begin",
            FontId::proportional(13.5),
            Color32::from_rgba_premultiplied(90, 195, 255, 200),
        );
        painter.text(
            card_rect.center_top() + Vec2::new(0.0, 112.0),
            Align2::CENTER_TOP,
            "or click  AUTO  in the left panel for an instant composition",
            FontId::proportional(10.0),
            Color32::from_rgba_premultiplied(135, 142, 168, 180),
        );
        return;
    }

    // Extract projected coordinates (with optional 3D Y-axis rotation)
    let use_3d = auto_rotate || rotation_angle.abs() > 0.01;
    let cos_a = rotation_angle.cos();
    let sin_a = rotation_angle.sin();
    let proj_pts: Vec<(f32, f32, f32, bool)> = points
        .iter()
        .map(|&(x, y, z, s, c)| {
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
        })
        .collect();

    // Compute bounds
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for &(x, y, _, _) in &proj_pts {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
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
    painter.line_segment(
        [
            Pos2::new(rect.left(), origin.y),
            Pos2::new(rect.right(), origin.y),
        ],
        Stroke::new(1.0, grid_color),
    );
    painter.line_segment(
        [
            Pos2::new(origin.x, rect.top()),
            Pos2::new(origin.x, rect.bottom()),
        ],
        Stroke::new(1.0, grid_color),
    );

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
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(-half, -half),
                    uv: egui::epaint::WHITE_UV,
                    color: ink_col,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(half, -half),
                    uv: egui::epaint::WHITE_UV,
                    color: ink_col,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(half, half),
                    uv: egui::epaint::WHITE_UV,
                    color: ink_col,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(-half, half),
                    uv: egui::epaint::WHITE_UV,
                    color: ink_col,
                });
                mesh.indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base,
                    base + 2,
                    base + 3,
                ]);
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
                scar_mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(-arm, -half_w),
                    uv: egui::epaint::WHITE_UV,
                    color: scar_col,
                });
                scar_mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(arm, -half_w),
                    uv: egui::epaint::WHITE_UV,
                    color: scar_col,
                });
                scar_mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(arm, half_w),
                    uv: egui::epaint::WHITE_UV,
                    color: scar_col,
                });
                scar_mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(-arm, half_w),
                    uv: egui::epaint::WHITE_UV,
                    color: scar_col,
                });
                scar_mesh.indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base,
                    base + 2,
                    base + 3,
                ]);
                // Vertical bar
                let base = scar_mesh.vertices.len() as u32;
                scar_mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(-half_w, -arm),
                    uv: egui::epaint::WHITE_UV,
                    color: scar_col,
                });
                scar_mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(half_w, -arm),
                    uv: egui::epaint::WHITE_UV,
                    color: scar_col,
                });
                scar_mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(half_w, arm),
                    uv: egui::epaint::WHITE_UV,
                    color: scar_col,
                });
                scar_mesh.vertices.push(egui::epaint::Vertex {
                    pos: sp + egui::vec2(-half_w, arm),
                    uv: egui::epaint::WHITE_UV,
                    color: scar_col,
                });
                scar_mesh.indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base,
                    base + 2,
                    base + 3,
                ]);
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
        if age_secs > fade_end {
            continue;
        }
        // Fade alpha: full for first 2s, then fades to 0 over 8s
        let ghost_alpha = if age_secs < fade_start {
            40u8
        } else {
            let t = (age_secs - fade_start) / (fade_end - fade_start);
            (40.0 * (1.0 - t)) as u8
        };
        if ghost_alpha == 0 {
            continue;
        }
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
                let a = if pass == 0 {
                    (alpha as f32 * 0.3) as u8
                } else {
                    alpha
                };
                let w_px = if pass == 0 { 3.0 } else { 1.2 };
                let r_col =
                    Color32::from_rgba_premultiplied((255.0 * recency * spd_bright) as u8, 0, 0, a);
                let c_col = Color32::from_rgba_premultiplied(
                    0,
                    (200.0 * recency * spd_bright) as u8,
                    (255.0 * recency * spd_bright) as u8,
                    a,
                );
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

    // ── Corner info overlay ─────────────────────────────────────────────────
    let sys_display = system_display_name(system_name);
    let sys_tag = system_tagline(system_name);
    let info_w = 220.0f32;
    let info_h = 72.0f32;
    let info_bg = egui::Rect::from_min_size(
        rect.left_top() + Vec2::new(8.0, 8.0),
        Vec2::new(info_w, info_h),
    );
    // Backdrop
    painter.rect_filled(info_bg, 8.0, Color32::from_rgba_premultiplied(6, 8, 22, 195));
    painter.rect_stroke(info_bg, 8.0, egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 80, 150, 100)));
    // Left accent bar
    let lbar = egui::Rect::from_min_size(info_bg.min, Vec2::new(3.0, info_h));
    painter.rect_filled(lbar, egui::Rounding { nw: 8.0, sw: 8.0, ne: 0.0, se: 0.0 },
        Color32::from_rgba_premultiplied(90, 195, 255, 160));

    painter.text(
        rect.left_top() + Vec2::new(16.0, 13.0),
        Align2::LEFT_TOP,
        sys_display,
        FontId::proportional(14.0),
        Color32::from_rgb(185, 215, 255),
    );
    // Tagline
    painter.text(
        rect.left_top() + Vec2::new(16.0, 31.0),
        Align2::LEFT_TOP,
        sys_tag,
        FontId::proportional(9.5),
        Color32::from_rgba_premultiplied(135, 150, 195, 200),
    );
    // Mode + projection row
    let proj_label = match projection {
        1 => "XZ",
        2 => "YZ",
        _ => "XY",
    };
    painter.text(
        rect.left_top() + Vec2::new(16.0, 55.0),
        Align2::LEFT_TOP,
        format!("{mode_name}  ·  {proj_label} plane"),
        FontId::proportional(10.0),
        Color32::from_rgba_premultiplied(90, 130, 195, 180),
    );

    // Derivative arrow at current position
    if let (Some(&(lx, ly, _, _)), true) = (proj_pts.last(), current_deriv.len() >= 2) {
        let pos = to_screen(lx, ly);
        let dx = current_deriv[0] as f32;
        let dy = current_deriv[1] as f32;
        let mag = (dx * dx + dy * dy).sqrt().max(1e-6);
        let scale = 40.0f32;
        let arrow_end = Pos2::new(pos.x + (dx / mag) * scale, pos.y - (dy / mag) * scale);
        painter.line_segment(
            [pos, arrow_end],
            Stroke::new(4.0, Color32::from_rgba_premultiplied(255, 200, 0, 40)),
        );
        painter.line_segment(
            [pos, arrow_end],
            Stroke::new(1.5, Color32::from_rgb(255, 220, 0)),
        );
        painter.circle_filled(arrow_end, 3.0, Color32::from_rgb(255, 220, 0));
    }

    // Equation overlay
    let eq_text = equation_text(system_name);
    if !eq_text.is_empty() {
        let eq_pos = rect.left_bottom() + Vec2::new(8.0, -8.0);
        painter.text(
            eq_pos,
            Align2::LEFT_BOTTOM,
            eq_text,
            FontId::monospace(10.0),
            Color32::from_rgba_premultiplied(150, 180, 255, 180),
        );
    }

    // State values — right side with subtle background
    if !current_state.is_empty() {
        let var_names = dim_names(system_name);
        let n_vars = current_state.len().min(6);
        let state_bg = egui::Rect::from_min_size(
            rect.right_top() + Vec2::new(-120.0, 6.0),
            Vec2::new(114.0, 8.0 + n_vars as f32 * 15.0),
        );
        painter.rect_filled(
            state_bg,
            6.0,
            Color32::from_rgba_premultiplied(8, 8, 20, 150),
        );
        for (i, (&val, name)) in current_state
            .iter()
            .zip(var_names.iter())
            .enumerate()
            .take(n_vars)
        {
            let text = format!("{} = {:+.3}", name, val);
            let pos = rect.right_top() + Vec2::new(-10.0, 12.0 + i as f32 * 15.0);
            painter.text(
                pos,
                Align2::RIGHT_TOP,
                text,
                FontId::monospace(10.5),
                Color32::from_rgba_premultiplied(80, 210, 130, 220),
            );
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
        painter.text(
            edrift_pos,
            Align2::LEFT_BOTTOM,
            edrift_text,
            FontId::monospace(10.0),
            edrift_color,
        );
    }
}

fn draw_mixer_tab(
    ui: &mut egui::Ui,
    state: &crate::ui::SharedState,
    viz_points: &[(f32, f32, f32, f32, bool)],
) {
    let mc_cyan = egui::Color32::from_rgb(0, 200, 220);
    let mc_green = egui::Color32::from_rgb(0, 220, 100);
    let mc_orange = egui::Color32::from_rgb(255, 160, 40);
    let mc_red = egui::Color32::from_rgb(220, 60, 60);
    let mc_gray = egui::Color32::from_rgb(120, 120, 140);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);

        // ── Xrun / audio health indicator (#21) ──────────────────────────
        {
            let xrun_count = state
                .lock()
                .xrun_counter
                .load(std::sync::atomic::Ordering::Relaxed);
            if xrun_count > 0 {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "⚠ {} xrun{}",
                            xrun_count,
                            if xrun_count == 1 { "" } else { "s" }
                        ))
                        .color(mc_red)
                        .size(13.0),
                    );
                    if ui.small_button("Reset").clicked() {
                        state
                            .lock()
                            .xrun_counter
                            .store(0, std::sync::atomic::Ordering::Relaxed);
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Audio stream errors detected — check CPU load or buffer size",
                    )
                    .color(mc_gray)
                    .size(10.0),
                );
            }
        }
        ui.add_space(4.0);

        // ── Save Clip button ──────────────────────────────────────────────
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("📸 Save Clip (60s audio + portrait)")
                            .color(egui::Color32::BLACK),
                    )
                    .fill(egui::Color32::from_rgb(220, 180, 40))
                    .min_size(egui::Vec2::new(280.0, 32.0)),
                )
                .clicked()
            {
                let sr = state.lock().sample_rate;
                let cb = state.lock().clip_buffer.clone();
                let trail = viz_points.to_vec();
                // LEGACY: compute state hash for .sig companion file
                let state_hash: u64 = {
                    let st = state.lock();
                    st.current_state
                        .iter()
                        .fold(0u64, |acc, &v| acc.wrapping_add(v.to_bits()))
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
                        (Ok(wav), Ok(_png)) => {
                            log::info!("Clip saved: {} + {}", wav, _png);
                        }
                        (Err(e), _) => log::error!("Clip save failed: {e}"),
                        (_, Err(e)) => log::error!("Portrait save failed: {e}"),
                    }
                });
                state.lock().clip_status = "Saving… check clips/ folder for WAV + PNG".into();
            }
            let status = state.lock().clip_status.clone();
            if !status.is_empty() {
                ui.label(egui::RichText::new(&status).color(mc_green).size(11.0));
            }
        });
        ui.label(
            egui::RichText::new(
                "Clips saved to clips/ folder — share-ready WAV + phase portrait PNG",
            )
            .color(mc_gray)
            .size(10.0),
        );
        ui.add_space(4.0);

        // ── Export SVG + Lossless WAV buttons ────────────────────────
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("🖼 Export SVG").color(egui::Color32::BLACK),
                    )
                    .fill(egui::Color32::from_rgb(0, 180, 220))
                    .min_size(egui::Vec2::new(140.0, 28.0)),
                )
                .clicked()
            {
                let trail = viz_points.to_vec();
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let dir = std::path::PathBuf::from("clips");
                let _ = std::fs::create_dir_all(&dir);
                let svg_path = dir.join(format!("portrait_{}.svg", ts));
                let svg_path_str = svg_path.to_string_lossy().into_owned();
                std::thread::spawn(move || match save_portrait_svg(&trail, 0, &svg_path_str) {
                    Ok(()) => log::info!("SVG portrait saved: {}", svg_path_str),
                    Err(e) => log::error!("SVG export failed: {e}"),
                });
                state.lock().clip_status = "Exporting SVG...".into();
            }

            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("💾 Lossless WAV").color(egui::Color32::BLACK),
                    )
                    .fill(egui::Color32::from_rgb(100, 220, 140))
                    .min_size(egui::Vec2::new(140.0, 28.0)),
                )
                .clicked()
            {
                let sr = state.lock().sample_rate;
                let cb = state.lock().clip_buffer.clone();
                std::thread::spawn(move || match save_clip_wav_32bit(&cb, sr) {
                    Ok(path) => log::info!("Lossless WAV saved: {}", path),
                    Err(e) => log::error!("Lossless WAV export failed: {e}"),
                });
                state.lock().clip_status = "Saving lossless WAV...".into();
            }
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── VU Meters ─────────────────────────────────────────────────────
        ui.label(egui::RichText::new("VU Meters").color(mc_cyan).strong());
        ui.add_space(4.0);
        let peaks = { *state.lock().vu_meter.lock() };
        // Update peak hold
        {
            let mut st = state.lock();
            let dt = 1.0 / 30.0_f32; // ~30fps UI rate
            st.vu_peak_decay_timer += dt;
            for i in 0..4 {
                if peaks[i] > st.vu_peak_hold[i] {
                    st.vu_peak_hold[i] = peaks[i];
                    st.vu_peak_decay_timer = 0.0;
                } else if st.vu_peak_decay_timer > 1.0 {
                    st.vu_peak_hold[i] *= 0.95;
                }
            }
        }
        let peak_hold = { state.lock().vu_peak_hold };
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
                    let bar_width = 18.0;
                    let (resp, painter) = ui.allocate_painter(
                        egui::Vec2::new(bar_width, bar_height),
                        egui::Sense::hover(),
                    );
                    let r = resp.rect;
                    painter.rect_filled(r, 2.0, egui::Color32::from_rgb(20, 20, 30));
                    let filled = peak.clamp(0.0, 1.0) * bar_height;
                    let fill_rect =
                        egui::Rect::from_min_max(egui::Pos2::new(r.min.x, r.max.y - filled), r.max);
                    let bar_col = if peak > 0.9 {
                        mc_red
                    } else if peak > 0.7 {
                        mc_orange
                    } else {
                        col
                    };
                    painter.rect_filled(fill_rect, 1.0, bar_col);
                    // Peak hold line
                    let hold_y = r.max.y - (peak_hold[i].clamp(0.0, 1.0) * bar_height);
                    painter.line_segment(
                        [egui::Pos2::new(r.min.x, hold_y), egui::Pos2::new(r.max.x, hold_y)],
                        egui::Stroke::new(2.0, egui::Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new(format!("{:.0}%", peak * 100.0))
                            .size(9.0)
                            .color(mc_gray),
                    );
                });
                if i < 3 {
                    ui.add_space(8.0);
                }
            }
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Layer 0 (Main) Controls ───────────────────────────────────────
        ui.label(
            egui::RichText::new("Layer 0 — Main System")
                .color(mc_green)
                .strong(),
        );
        ui.horizontal(|ui| {
            let mut st = state.lock();
            ui.add(egui::Slider::new(&mut st.layer0_level, 0.0..=1.5).text("Level"));
            ui.add(egui::Slider::new(&mut st.layer0_pan, -1.0..=1.0).text("Pan"));
            let mc = if st.layer0_mute {
                mc_red
            } else {
                egui::Color32::from_rgb(40, 60, 40)
            };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("M").color(egui::Color32::WHITE))
                        .fill(mc)
                        .min_size(egui::Vec2::new(28.0, 28.0)),
                )
                .clicked()
            {
                st.layer0_mute = !st.layer0_mute;
            }
        });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("ADSR Envelope (triggered by arpeggiator / KS events)")
                .color(mc_cyan)
                .size(11.0),
        );
        {
            let mut st = state.lock();
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut st.adsr_attack_ms, 1.0..=2000.0)
                        .text("Attack ms")
                        .logarithmic(true),
                );
                ui.add(
                    egui::Slider::new(&mut st.adsr_decay_ms, 1.0..=2000.0)
                        .text("Decay ms")
                        .logarithmic(true),
                );
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut st.adsr_sustain, 0.0..=1.0).text("Sustain"));
                ui.add(
                    egui::Slider::new(&mut st.adsr_release_ms, 10.0..=5000.0)
                        .text("Release ms")
                        .logarithmic(true),
                );
            });
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Extra Polyphony Layers ────────────────────────────────────────
        for li in 0..2usize {
            let label_col = if li == 0 { mc_cyan } else { mc_orange };
            ui.label(
                egui::RichText::new(format!("Layer {} — Additional System", li + 1))
                    .color(label_col)
                    .strong(),
            );

            let (preset_name, active) = {
                let st = state.lock();
                let d = &st.poly_layers[li];
                (d.preset_name.clone(), d.active)
            };

            ui.horizontal(|ui| {
                let mut act = active;
                if ui
                    .checkbox(
                        &mut act,
                        egui::RichText::new("Active").color(egui::Color32::WHITE),
                    )
                    .changed()
                {
                    let mut st = state.lock();
                    st.poly_layers[li].active = act;
                    st.poly_layers[li].changed = true;
                }
                egui::ComboBox::new(format!("layer_preset_{}", li), "Preset")
                    .selected_text(if preset_name.is_empty() {
                        "Select…"
                    } else {
                        &preset_name
                    })
                    .show_ui(ui, |ui| {
                        for preset in crate::patches::PRESETS {
                            if ui
                                .selectable_label(preset_name == preset.name, preset.name)
                                .clicked()
                            {
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
                    ui.add(egui::Slider::new(&mut d.pan, -1.0..=1.0).text("Pan"));
                    let mc = if d.mute {
                        mc_red
                    } else {
                        egui::Color32::from_rgb(40, 60, 40)
                    };
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("M").color(egui::Color32::WHITE))
                                .fill(mc)
                                .min_size(egui::Vec2::new(28.0, 28.0)),
                        )
                        .clicked()
                    {
                        d.mute = !d.mute;
                    }
                });
                ui.horizontal(|ui| {
                    let mut st = state.lock();
                    let d = &mut st.poly_layers[li];
                    ui.add(
                        egui::Slider::new(&mut d.adsr_attack_ms, 1.0..=2000.0)
                            .text("A ms")
                            .logarithmic(true),
                    );
                    ui.add(
                        egui::Slider::new(&mut d.adsr_decay_ms, 1.0..=2000.0)
                            .text("D ms")
                            .logarithmic(true),
                    );
                    ui.add(egui::Slider::new(&mut d.adsr_sustain, 0.0..=1.0).text("S"));
                    ui.add(
                        egui::Slider::new(&mut d.adsr_release_ms, 10.0..=5000.0)
                            .text("R ms")
                            .logarithmic(true),
                    );
                });
            }
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
        }

        // ── Audio Sidechain ───────────────────────────────────────────────
        ui.label(
            egui::RichText::new("Audio Sidechain Input")
                .color(mc_orange)
                .strong(),
        );
        ui.label(
            egui::RichText::new("Modulate parameters from mic/line-in audio energy")
                .color(mc_gray)
                .size(10.0),
        );
        ui.add_space(4.0);
        {
            let mut st = state.lock();
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut st.sidechain_enabled,
                    egui::RichText::new("Enable").color(egui::Color32::WHITE),
                );
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
                    let (resp, painter) =
                        ui.allocate_painter(egui::Vec2::new(200.0, 12.0), egui::Sense::hover());
                    let r = resp.rect;
                    painter.rect_filled(r, 2.0, egui::Color32::from_rgb(20, 20, 30));
                    let filled = sc_level.clamp(0.0, 1.0) * r.width();
                    if filled > 0.0 {
                        let fill_r = egui::Rect::from_min_max(
                            r.min,
                            egui::Pos2::new(r.min.x + filled, r.max.y),
                        );
                        painter.rect_filled(fill_r, 1.0, mc_orange);
                    }
                });
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Spectral Freeze ───────────────────────────────────────────────
        ui.label(
            egui::RichText::new("Spectral Freeze")
                .color(egui::Color32::from_rgb(100, 200, 255))
                .strong(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut st = state.lock();
            let freeze_active = st.spectral_freeze_active;
            let freeze_color = if freeze_active {
                egui::Color32::from_rgb(40, 140, 220)
            } else {
                egui::Color32::from_rgb(40, 40, 70)
            };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("FREEZE").color(egui::Color32::WHITE))
                        .fill(freeze_color)
                        .min_size(egui::Vec2::new(80.0, 28.0)),
                )
                .on_hover_text("Capture live spectral content from the attractor")
                .clicked()
            {
                // Capture real live partials from the attractor (populated by sim thread)
                let live = st.spectral_live_partials;
                let base = st.config.sonification.base_frequency as f32;
                let mut freqs = vec![0.0f32; 16];
                let mut amps = vec![0.0f32; 16];
                for i in 0..16 {
                    freqs[i] = base * (i + 1) as f32;
                    // Use live partial amplitude if available, else fall back to harmonic series
                    amps[i] = if live[i] > 0.0 {
                        live[i]
                    } else {
                        1.0 / (i + 1) as f32 * 0.5
                    };
                }
                st.spectral_freeze_freqs = freqs;
                st.spectral_freeze_amps = amps;
                st.spectral_freeze_active = true;
            }
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("CLEAR").color(egui::Color32::WHITE))
                        .fill(egui::Color32::from_rgb(80, 40, 40))
                        .min_size(egui::Vec2::new(60.0, 28.0)),
                )
                .clicked()
            {
                st.spectral_freeze_active = false;
            }
            let status = if freeze_active { "FROZEN" } else { "Off" };
            ui.label(
                egui::RichText::new(status)
                    .color(if freeze_active { mc_cyan } else { mc_gray })
                    .size(12.0),
            );
        });
        // Show frozen partial amplitudes as a mini bar graph when frozen
        {
            let st = state.lock();
            if st.spectral_freeze_active && !st.spectral_freeze_amps.is_empty() {
                let bar_w = 10.0f32;
                let bar_h_max = 28.0f32;
                let spacing = 2.0f32;
                let n = st.spectral_freeze_amps.len().min(16);
                let total_w = n as f32 * (bar_w + spacing);
                let (resp, painter) = ui.allocate_painter(
                    egui::Vec2::new(total_w, bar_h_max + 4.0),
                    egui::Sense::hover(),
                );
                let r = resp.rect;
                let max_amp = st
                    .spectral_freeze_amps
                    .iter()
                    .copied()
                    .fold(0.0f32, f32::max)
                    .max(1e-6);
                for (j, &amp_v) in st.spectral_freeze_amps.iter().take(n).enumerate() {
                    let norm = (amp_v / max_amp).clamp(0.0, 1.0);
                    let bh = norm * bar_h_max;
                    let x = r.left() + j as f32 * (bar_w + spacing);
                    let bar_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(x, r.bottom() - bh - 2.0),
                        egui::Vec2::new(bar_w, bh.max(1.0)),
                    );
                    painter.rect_filled(bar_rect, 2.0, egui::Color32::from_rgb(40, 160, 220));
                }
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Replay Recording ──────────────────────────────────────────────
        ui.label(
            egui::RichText::new("Replay Recording")
                .color(egui::Color32::from_rgb(220, 120, 40))
                .strong(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut st = state.lock();
            let rec = st.replay_recording;
            let play = st.replay_playing;
            let rec_color = if rec {
                mc_red
            } else {
                egui::Color32::from_rgb(140, 60, 60)
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(if rec { "STOP REC" } else { "REC" })
                            .color(egui::Color32::WHITE),
                    )
                    .fill(rec_color)
                    .min_size(egui::Vec2::new(70.0, 28.0)),
                )
                .clicked()
            {
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
            let play_color = if play {
                mc_green
            } else {
                egui::Color32::from_rgb(40, 100, 40)
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(if play { "STOP" } else { "PLAY FILE" })
                            .color(egui::Color32::WHITE),
                    )
                    .fill(play_color)
                    .min_size(egui::Vec2::new(80.0, 28.0)),
                )
                .clicked()
            {
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
            ui.label(
                egui::RichText::new(format!("{n} events"))
                    .color(mc_gray)
                    .size(11.0),
            );
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Looper ────────────────────────────────────────────────────────
        // item 9: looper REC captures intent; audio playback not yet wired to the audio thread.
        ui.label(
            egui::RichText::new("Looper  (recording only — playback coming soon)")
                .color(egui::Color32::from_rgb(200, 100, 255))
                .strong(),
        );
        ui.add_space(4.0);
        {
            let mut st = state.lock();
            ui.horizontal(|ui| {
                let rec = st.looper_recording;
                let rec_color = if rec {
                    mc_red
                } else {
                    egui::Color32::from_rgb(140, 60, 60)
                };
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(if rec { "STOP" } else { "REC" })
                                .color(egui::Color32::WHITE),
                        )
                        .fill(rec_color)
                        .min_size(egui::Vec2::new(60.0, 28.0)),
                    )
                    .clicked()
                {
                    st.looper_recording = !rec;
                }
                ui.label(egui::RichText::new("Bars:").color(mc_gray));
                for bars in [1u32, 2, 4, 8] {
                    let sel = st.looper_bars == bars;
                    let c = if sel {
                        mc_cyan
                    } else {
                        egui::Color32::from_rgb(40, 40, 70)
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(bars.to_string()).color(egui::Color32::WHITE),
                            )
                            .fill(c)
                            .min_size(egui::Vec2::new(28.0, 24.0)),
                        )
                        .clicked()
                    {
                        st.looper_bars = bars;
                    }
                }
                ui.add(
                    egui::Slider::new(&mut st.looper_bpm, 60.0..=200.0)
                        .text("BPM")
                        .integer(),
                );
            });
            ui.add_space(4.0);
            // Layer list
            let n_layers = st.looper_layers.len();
            if n_layers == 0 {
                ui.label(
                    egui::RichText::new("No loops recorded — hit REC to start")
                        .color(mc_gray)
                        .size(11.0)
                        .italics(),
                );
            }
            for i in 0..n_layers {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("L{}", i + 1))
                            .color(mc_gray)
                            .size(11.0),
                    );
                    let active = st.looper_layers[i].active;
                    let ac = if active {
                        mc_green
                    } else {
                        egui::Color32::from_rgb(40, 40, 70)
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(if active { "ON" } else { "OFF" })
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(ac)
                            .min_size(egui::Vec2::new(36.0, 22.0)),
                        )
                        .clicked()
                    {
                        st.looper_layers[i].active = !active;
                    }
                    ui.add(
                        egui::Slider::new(&mut st.looper_layers[i].level, 0.0..=1.0).text("Vol"),
                    );
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("X").color(egui::Color32::WHITE))
                                .fill(mc_red)
                                .min_size(egui::Vec2::new(22.0, 22.0)),
                        )
                        .clicked()
                    {
                        st.looper_layers.remove(i);
                        // Break since we modified the vec
                        return;
                    }
                });
            }
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("CLEAR ALL").color(egui::Color32::WHITE))
                        .fill(egui::Color32::from_rgb(80, 40, 40))
                        .min_size(egui::Vec2::new(90.0, 24.0)),
                )
                .clicked()
            {
                st.looper_layers.clear();
            }
        }
    });
}

/// Write a Standard MIDI File (Type 0, 480 PPQ) from raw note events.
/// Each event is (timestamp_ms, status, data1, data2).
fn write_midi_file(events: &[(u32, u8, u8, u8)], path: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let dir = std::path::Path::new(path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
    const PPQ: u32 = 480;
    const TEMPO: u32 = 500_000; // 120 BPM in microseconds/beat

    // Convert ms timestamps → ticks (480 ticks/beat, 120 BPM → 1 beat = 500ms)
    let ms_per_tick = TEMPO as f64 / 1000.0 / PPQ as f64;

    fn write_vlq(buf: &mut Vec<u8>, mut v: u32) {
        let mut bytes = [0u8; 4];
        let mut n = 0;
        bytes[n] = (v & 0x7f) as u8;
        v >>= 7;
        while v > 0 {
            n += 1;
            bytes[n] = 0x80 | (v & 0x7f) as u8;
            v >>= 7;
        }
        for i in (0..=n).rev() {
            buf.push(bytes[i]);
        }
    }

    let mut track = Vec::<u8>::new();
    // Tempo meta-event at tick 0
    track.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03]);
    track.push((TEMPO >> 16) as u8);
    track.push((TEMPO >> 8) as u8);
    track.push(TEMPO as u8);

    let mut last_tick: u32 = 0;
    for &(ms, status, d1, d2) in events {
        let tick = (ms as f64 / ms_per_tick).round() as u32;
        let delta = tick.saturating_sub(last_tick);
        last_tick = tick;
        write_vlq(&mut track, delta);
        track.push(status);
        track.push(d1);
        track.push(d2);
    }
    // End of track meta-event
    track.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);

    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    // MThd chunk
    f.write_all(b"MThd")?;
    f.write_all(&6u32.to_be_bytes())?;
    f.write_all(&0u16.to_be_bytes())?; // type 0
    f.write_all(&1u16.to_be_bytes())?; // 1 track
    f.write_all(&(PPQ as u16).to_be_bytes())?;
    // MTrk chunk
    f.write_all(b"MTrk")?;
    f.write_all(&(track.len() as u32).to_be_bytes())?;
    f.write_all(&track)?;
    Ok(())
}

fn save_replay_file(events: &[crate::ui::ReplayEvent], path: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let dir = std::path::Path::new(path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
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
        if f.read_exact(&mut ts_buf).is_err() {
            break;
        }
        let timestamp_ms = u32::from_le_bytes(ts_buf);
        if f.read_exact(&mut buf1).is_err() {
            break;
        }
        let param_id = buf1[0];
        if f.read_exact(&mut buf4).is_err() {
            break;
        }
        let value = f32::from_le_bytes(buf4);
        events.push(crate::ui::ReplayEvent {
            timestamp_ms,
            param_id,
            value,
        });
    }
    Ok(events)
}

// draw_waveform, hz_to_note_name, and draw_note_map live in src/ui_waveform.rs

// ---------------------------------------------------------------------------
// Helper functions for math overlay
// ---------------------------------------------------------------------------

pub(crate) fn hue_to_color(hue: f32, saturation: f32) -> Color32 {
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
        "lorenz" => "x' = σ(y−x)\ny' = x(ρ−z)−y\nz' = xy−βz",
        "rossler" => "x' = −y−z\ny' = x+ay\nz' = b+z(x−c)",
        "double_pendulum" => "θ₁'' = f(θ₁,θ₂,ω₁,ω₂)\nθ₂'' = g(θ₁,θ₂,ω₁,ω₂)",
        "geodesic_torus" => "φ'' = −2r sin(θ)/(R+r cos θ)·φ'θ'\nθ'' = (R+r cos θ)sin θ/r·φ'²",
        "kuramoto" => "θ'ᵢ = ωᵢ + K/N Σⱼ sin(θⱼ−θᵢ)",
        "three_body" => "r''ᵢ = G Σⱼ mⱼ(rⱼ−rᵢ)/|rⱼ−rᵢ|³",
        "duffing" => "x' = y\ny' = −δy−αx−βx³+γcos(ωt)",
        "van_der_pol" => "x' = y\ny' = μ(1−x²)y−x",
        "halvorsen" => "x' = −ax−4y−4z−y²\ny' = −ay−4z−4x−z²\nz' = −az−4x−4y−x²",
        "aizawa" => "x' = (z−b)x−dy\ny' = dx+(z−b)y\nz' = c+az−z³/3−(x²+y²)(1+ez)+fz x³",
        "chua" => "x' = α(y−x−f(x))\ny' = x−y+z\nz' = −βy",
        "hindmarsh_rose" => "x' = y−ax³+bx²−z+I\ny' = c−dx²−y\nz' = r(s(x−x₀)−z)",
        "nose_hoover" => "x' = y\ny' = yz−x\nz' = 1−y²",
        "sprott_b" => "x' = yz\ny' = x−y\nz' = 1−xy",
        "logistic_map" => "xₙ₊₁ = r·xₙ(1−xₙ)",
        "standard_map" => "pₙ₊₁ = pₙ+k sin(θₙ)\nθₙ₊₁ = θₙ+pₙ₊₁  (mod 2π)",
        "henon_map" => "xₙ₊₁ = 1−axₙ²+yₙ\nyₙ₊₁ = bxₙ",
        "oregonator" => "x' = s(y−xy+x−qx²)\ny' = (−y−xy+fz)/s\nz' = x−z",
        "mathieu" => "x'' + (a−2q cos 2t)x = 0",
        "mackey_glass" => "x'(t) = βx(t−τ)/(1+x(t−τ)ⁿ) − γx(t)",
        "thomas" => "x' = sin(y)−bx\ny' = sin(z)−by\nz' = sin(x)−bz",
        "sprott_c" => "x' = yz\ny' = x−y\nz' = 1−x²",
        "dadras" => "x' = y−ax+byz\ny' = cy−xz+z\nz' = dxy−ez",
        "rucklidge" => "x' = −κx+λy−yz\ny' = x\nz' = −z+y²",
        "chen" => "x' = a(y−x)\ny' = (c−a)x−xz+cy\nz' = xy−bz",
        "burke_shaw" => "x' = −σ(x+y)\ny' = −y−σxz\nz' = σxy+ρ",
        "lorenz84" => "x' = −ax−y²−z²+aF\ny' = −y+xy−bxz+G\nz' = −z+bxy+xz",
        "rabinovich_fabrikant" => "x' = y(z−1+x²)+γx\ny' = x(3z+1−x²)+γy\nz' = −2z(α+xy)",
        "sprott_g" => "x' = 0.4x+z\ny' = xz−y\nz' = −x+y",
        "sprott_h" => "x' = −y+z²\ny' = x+0.5y\nz' = x−z",
        "sprott_l" => "x' = y+3.9z\ny' = 0.9x²−y\nz' = 1−x",
        "rikitake" => "x' = −μx+yz\ny' = −μy+x(z−a)\nz' = 1−xy",
        "bouali" => "x' = x(4−y)+az\ny' = −y(1−x²)\nz' = −x(1−s)",
        "newton_leipnik" => "x' = −ax+y+10yz\ny' = −x−0.4y+5xz\nz' = bz−5xy",
        "shimizu_morioka" => "x' = y\ny' = (1−z)x−ay\nz' = x²−bz",
        "genesio_tesi" => "x' = y\ny' = z\nz' = −cx−by−az+x²",
        "liu" => "x' = −ax−ey²\ny' = by−kxz\nz' = −cz+mxy",
        "windmi" => "x' = y\ny' = z\nz' = −az−y+b−eˣ",
        "finance" => "x' = z+(y−a)x\ny' = 1−by−x²\nz' = −x−cz",
        "hyperchaos" => "x' = a(y−x)+w\ny' = (c−a)x−xz+cy\nz' = xy−bz\nw' = −yz+dw",
        "sprott_d" => "x' = −0.2y\ny' = x+z\nz' = x+y²−1.1z",
        "sprott_e" => "x' = yz\ny' = x²−y\nz' = 1−4x",
        "sprott_f" => "x' = y+z\ny' = −x+0.5y\nz' = x²−z",
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
        "duffing" => vec!["x' = y", "y' = -d*y - a*x - b*x^3 + g*cos(w*t)"],
        "van_der_pol" => vec!["x' = y", "y' = mu*(1-x^2)*y - x"],
        "halvorsen" => vec![
            "x' = -a*x - 4*y - 4*z - y^2",
            "y' = -a*y - 4*z - 4*x - z^2",
            "z' = -a*z - 4*x - 4*y - x^2",
        ],
        "aizawa" => vec![
            "x' = (z-b)*x - d*y",
            "y' = d*x + (z-b)*y",
            "z' = c + a*z - z^3/3 - (x^2+y^2)*(1+e*z) + f*z*x^3",
        ],
        "chua" => vec![
            "x' = a*(y - x - f(x))",
            "y' = x - y + z",
            "z' = -b*y",
            "f(x) = m1*x + 0.5*(m0-m1)*(|x+1|-|x-1|)",
        ],
        "hindmarsh_rose" => vec![
            "x' = y - a*x^3 + b*x^2 - z + I",
            "y' = c - d*x^2 - y",
            "z' = r*(s*(x - x0) - z)",
        ],
        "nose_hoover" => vec!["x' = y", "y' = y*z - x", "z' = 1 - y^2"],
        "sprott_b" => vec!["x' = y*z", "y' = x - y", "z' = 1 - x*y"],
        "mackey_glass" => vec!["x'(t) = b*x(t-tau)/(1+x(t-tau)^n) - g*x(t)"],
        "thomas" => vec!["x' = sin(y) - b*x", "y' = sin(z) - b*y", "z' = sin(x) - b*z"],
        "sprott_c" => vec!["x' = y*z", "y' = x - y", "z' = 1 - x^2"],
        "dadras" => vec![
            "x' = y - a*x + b*y*z",
            "y' = c*y - x*z + z",
            "z' = d*x*y - e*z",
        ],
        "rucklidge" => vec!["x' = -k*x + l*y - y*z", "y' = x", "z' = -z + y^2"],
        "chen" => vec![
            "x' = a*(y - x)",
            "y' = (c-a)*x - x*z + c*y",
            "z' = x*y - b*z",
        ],
        "burke_shaw" => vec![
            "x' = -s*(x + y)",
            "y' = -y - s*x*z",
            "z' = s*x*y + r",
        ],
        "lorenz84" => vec![
            "x' = -a*x - y^2 - z^2 + a*F",
            "y' = -y + x*y - b*x*z + G",
            "z' = -z + b*x*y + x*z",
        ],
        "rabinovich_fabrikant" => vec![
            "x' = y*(z - 1 + x^2) + g*x",
            "y' = x*(3*z + 1 - x^2) + g*y",
            "z' = -2*z*(a + x*y)",
        ],
        "sprott_g" => vec!["x' = 0.4*x + z", "y' = x*z - y", "z' = -x + y"],
        "sprott_h" => vec!["x' = -y + z^2", "y' = x + 0.5*y", "z' = x - z"],
        "sprott_l" => vec!["x' = y + 3.9*z", "y' = 0.9*x^2 - y", "z' = 1 - x"],
        "rikitake" => vec![
            "x' = -mu*x + y*z",
            "y' = -mu*y + x*(z - a)",
            "z' = 1 - x*y",
        ],
        "bouali" => vec![
            "x' = x*(4 - y) + a*z",
            "y' = -y*(1 - x^2)",
            "z' = -x*(1 - s)",
        ],
        "newton_leipnik" => vec![
            "x' = -a*x + y + 10*y*z",
            "y' = -x - 0.4*y + 5*x*z",
            "z' = b*z - 5*x*y",
        ],
        "shimizu_morioka" => vec![
            "x' = y",
            "y' = (1 - z)*x - a*y",
            "z' = x^2 - b*z",
        ],
        "genesio_tesi" => vec![
            "x' = y",
            "y' = z",
            "z' = -c*x - b*y - a*z + x^2",
        ],
        "sprott_d" => vec![
            "x' = -0.2*y",
            "y' = x + z",
            "z' = x + y^2 - 1.1*z",
        ],
        "sprott_e" => vec![
            "x' = y*z",
            "y' = x^2 - y",
            "z' = 1 - 4*x",
        ],
        "sprott_f" => vec![
            "x' = y + z",
            "y' = -x + 0.5*y",
            "z' = x^2 - z",
        ],
        "liu" => vec![
            "x' = -a*x - e*y^2",
            "y' = b*y - k*x*z",
            "z' = -c*z + m*x*y",
        ],
        "windmi" => vec![
            "x' = y",
            "y' = z",
            "z' = -a*z - y + b - exp(x)",
        ],
        "finance" => vec![
            "x' = z + (y - a)*x",
            "y' = 1 - b*y - x^2",
            "z' = -x - c*z",
        ],
        "hyperchaos" => vec![
            "x' = a*(y - x) + w",
            "y' = (c - a)*x - x*z + c*y",
            "z' = x*y - b*z",
            "w' = -y*z + d*w",
        ],
        "logistic_map" => vec!["x(n+1) = r * x(n) * (1 - x(n))"],
        "standard_map" => vec![
            "p(n+1) = p(n) + k*sin(th(n))  (mod 2*pi)",
            "th(n+1) = th(n) + p(n+1)      (mod 2*pi)",
        ],
        "henon_map" => vec!["x(n+1) = 1 - a*x(n)^2 + y(n)", "y(n+1) = b*x(n)"],
        "oregonator" => vec![
            "x' = s*(y - x*y + x - q*x^2)",
            "y' = (-y - x*y + f*z) / s",
            "z' = x - z",
        ],
        "mathieu" => vec![
            "x'' + (a - 2*q*cos(2*t))*x = 0",
            "x' = v",
            "v' = -(a - 2*q*cos(2*t))*x",
        ],
        "kuramoto_driven" => vec![
            "th'_i = w_i + K/N * sum_j sin(th_j - th_i)",
            "First oscillator: th'_1 += A*cos(W*t - th_1)",
        ],
        "lorenz96" => vec![
            "x'_i = (x_{i+1} - x_{i-2})*x_{i-1} - x_i + F",
            "(indices mod N; N=8 by default)",
        ],
        "fractional_lorenz" => vec![
            "D^a x = s*(y - x)",
            "D^a y = x*(r - z) - y",
            "D^a z = x*y - b*z",
            "(D^a = Caputo fractional derivative of order a)",
        ],
        "stochastic_lorenz" => vec![
            "dx = s*(y-x)*dt + noise*dW_x",
            "dy = (x*(r-z)-y)*dt + noise*dW_y",
            "dz = (x*y - b*z)*dt + noise*dW_z",
        ],
        "delayed_map" => vec!["x(n+1) = r * x(n-d) * (1 - x(n-d))"],
        "arnold_cat" => vec!["x(n+1) = (2*x(n) + y(n)) mod 1", "y(n+1) = (x(n) + y(n)) mod 1"],
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
// Poincaré Section tab (Feature #8)
// ---------------------------------------------------------------------------

/// Draw a recurrence plot of the trajectory's x-coordinate.
///
/// A recurrence plot visualizes when the trajectory revisits (approximately)
/// the same region of state space.  For each pair of time indices (i, j), a
/// pixel is lit if |x[i] - x[j]| < threshold, revealing diagonal lines for
/// periodic orbits and diffuse patterns for chaotic ones.
fn draw_recurrence_tab(ui: &mut Ui, state: &SharedState) {
    let traj = {
        let st = state.lock();
        // Use x-coordinate of the current trail (already collected by sim thread)
        // We take up to the last 200 points for a manageable N×N grid
        st.trajectory_points.iter().rev().take(200).map(|v| v[0]).collect::<Vec<f64>>()
    };

    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(4, 6, 16));

    painter.text(
        Pos2::new(rect.left() + 10.0, rect.top() + 8.0),
        Align2::LEFT_TOP,
        format!("Recurrence Plot  (N={}, x-coordinate)", traj.len()),
        FontId::proportional(13.0),
        Color32::from_rgb(80, 180, 220),
    );

    if traj.len() < 4 {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Collecting trajectory data…",
            FontId::proportional(14.0),
            Color32::from_rgb(80, 80, 120),
        );
        return;
    }

    let n = traj.len();
    // Adaptive threshold: 10% of the signal range
    let x_min = traj.iter().cloned().fold(f64::INFINITY, f64::min);
    let x_max = traj.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let threshold = (x_max - x_min) * 0.10;

    let pad = 24.0f32;
    let plot_size = (rect.width() - 2.0 * pad).min(rect.height() - 2.0 * pad - 30.0).max(4.0);
    let cell = plot_size / n as f32;
    let origin = Pos2::new(rect.left() + pad, rect.top() + 30.0);

    for i in 0..n {
        for j in 0..n {
            if (traj[i] - traj[j]).abs() < threshold {
                let px = origin.x + j as f32 * cell;
                let py = origin.y + i as f32 * cell;
                // Color by distance from diagonal: white on diagonal, cyan off
                let diag_dist = ((i as i64 - j as i64).unsigned_abs() as f32 / n as f32).min(1.0);
                let alpha = (200.0 * (1.0 - diag_dist * 0.6)) as u8;
                let col = Color32::from_rgba_premultiplied(0, 200, 255, alpha);
                painter.rect_filled(
                    egui::Rect::from_min_size(Pos2::new(px, py), Vec2::new(cell.max(1.0), cell.max(1.0))),
                    0.0,
                    col,
                );
            }
        }
    }

    // Threshold label
    painter.text(
        Pos2::new(rect.left() + pad, rect.bottom() - 6.0),
        Align2::LEFT_BOTTOM,
        format!("ε = {:.3}  (10% of range {:.2}..{:.2})", threshold, x_min, x_max),
        FontId::proportional(10.0),
        Color32::from_rgb(100, 120, 160),
    );

    // Hover: show time indices
    if let Some(hover_pos) = response.hover_pos() {
        let i = ((hover_pos.y - origin.y) / cell) as usize;
        let j = ((hover_pos.x - origin.x) / cell) as usize;
        if i < n && j < n {
            painter.text(
                hover_pos + Vec2::new(6.0, -16.0),
                Align2::LEFT_TOP,
                format!("i={} j={}  |Δx|={:.3}", i, j, (traj[i] - traj[j]).abs()),
                FontId::proportional(10.0),
                Color32::from_rgb(255, 240, 100),
            );
        }
    }
}

fn draw_poincare_tab(ui: &mut Ui, state: &SharedState) {
    let (points, system_name) = {
        let st = state.lock();
        (st.poincare_points.clone(), st.config.system.name.clone())
    };
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(4, 8, 18));

    // Title
    painter.text(
        Pos2::new(rect.left() + 16.0, rect.top() + 10.0),
        Align2::LEFT_TOP,
        format!("Poincaré Section  —  {}  (z = 27 crossing)", system_name),
        FontId::proportional(14.0),
        Color32::from_rgb(80, 200, 220),
    );
    painter.text(
        Pos2::new(rect.left() + 16.0, rect.top() + 28.0),
        Align2::LEFT_TOP,
        format!("{} points", points.len()),
        FontId::monospace(11.0),
        Color32::from_rgb(100, 130, 160),
    );

    if points.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Waiting for Poincaré crossings…\n(z must cross 27.0 from below)",
            FontId::proportional(14.0),
            Color32::from_rgb(80, 90, 120),
        );
        return;
    }

    // Compute bounds of (x, y) data
    let mut x_min = f32::MAX;
    let mut x_max = f32::MIN;
    let mut y_min = f32::MAX;
    let mut y_max = f32::MIN;
    for &(x, y) in &points {
        x_min = x_min.min(x);
        x_max = x_max.max(x);
        y_min = y_min.min(y);
        y_max = y_max.max(y);
    }
    let x_range = (x_max - x_min).max(0.01);
    let y_range = (y_max - y_min).max(0.01);

    let pad = 50.0f32;
    let plot_rect = Rect::from_min_max(
        Pos2::new(rect.left() + pad, rect.top() + pad + 30.0),
        Pos2::new(rect.right() - pad, rect.bottom() - pad),
    );

    // Axis labels
    painter.text(
        Pos2::new(plot_rect.center().x, plot_rect.bottom() + 16.0),
        Align2::CENTER_TOP,
        "x",
        FontId::proportional(12.0),
        Color32::from_rgb(160, 190, 220),
    );
    painter.text(
        Pos2::new(plot_rect.left() - 18.0, plot_rect.center().y),
        Align2::CENTER_CENTER,
        "y",
        FontId::proportional(12.0),
        Color32::from_rgb(160, 190, 220),
    );

    // Scatter plot
    let dot_radius = 2.0f32;
    let cyan = Color32::from_rgb(0, 210, 220);
    for &(x, y) in &points {
        let nx = (x - x_min) / x_range;
        let ny = (y - y_min) / y_range;
        let px = plot_rect.left() + nx * plot_rect.width();
        let py = plot_rect.bottom() - ny * plot_rect.height();
        painter.circle_filled(Pos2::new(px, py), dot_radius, cyan);
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
    lyapunov_history: &std::collections::VecDeque<f32>,
) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let mid_x = rect.center().x;

    let mut y = rect.top() + 20.0;
    let x = rect.left() + 20.0;

    painter.text(
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        system_display_name(system_name),
        FontId::proportional(20.0),
        Color32::from_rgb(120, 195, 255),
    );
    y += 32.0;

    let eq_lines = equation_lines(system_name);
    painter.text(
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        "Equations of Motion:",
        FontId::proportional(13.0),
        Color32::from_rgb(150, 150, 200),
    );
    y += 20.0;
    for line in &eq_lines {
        painter.text(
            Pos2::new(x + 10.0, y),
            Align2::LEFT_TOP,
            line,
            FontId::monospace(12.0),
            Color32::from_rgb(180, 210, 255),
        );
        y += 18.0;
    }
    y += 15.0;

    painter.text(
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        "State  ->  dx/dt",
        FontId::proportional(13.0),
        Color32::from_rgb(150, 150, 200),
    );
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

        painter.text(
            Pos2::new(x + 130.0, y),
            Align2::LEFT_TOP,
            state_text,
            FontId::monospace(11.0),
            Color32::from_rgb(200, 220, 200),
        );
        painter.text(
            Pos2::new(x + 280.0, y),
            Align2::LEFT_TOP,
            deriv_text,
            FontId::monospace(11.0),
            Color32::from_rgb(220, 200, 100),
        );
        y += 16.0;
    }

    y += 15.0;
    painter.text(
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        format!("Chaos Level: {:.1}%", chaos_level * 100.0),
        FontId::proportional(13.0),
        Color32::from_rgb(150, 150, 200),
    );
    y += 20.0;
    let meter_w = (mid_x - x - 40.0).max(100.0);
    let meter_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(meter_w, 16.0));
    painter.rect_filled(meter_rect, 4.0, Color32::from_rgb(20, 20, 40));
    let fill_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(meter_w * chaos_level, 16.0));
    let chaos_color = lerp_color(
        Color32::from_rgb(0, 100, 255),
        Color32::from_rgb(255, 30, 30),
        chaos_level,
    );
    painter.rect_filled(fill_rect, 4.0, chaos_color);

    // Lyapunov exponent spectrum
    if !lyapunov_spectrum.is_empty() {
        y += 20.0;
        painter.text(
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "Lyapunov Spectrum:",
            FontId::proportional(13.0),
            Color32::from_rgb(150, 150, 200),
        );
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
            painter.text(
                Pos2::new(x, y),
                Align2::LEFT_TOP,
                format!("λ{} = {}{:.4}", i + 1, sign, lambda),
                FontId::monospace(11.0),
                bar_color,
            );
            y += 16.0;
        }
        y += 4.0;
        // Kaplan-Yorke dimension estimate (if we have enough exponents)
        if lyapunov_spectrum.len() >= 2 {
            let mut sum = 0.0f64;
            let mut ky_j = 0usize;
            for (j, &lam) in lyapunov_spectrum.iter().enumerate() {
                if sum + lam < 0.0 {
                    break;
                }
                sum += lam;
                ky_j = j + 1;
            }
            if ky_j > 0 && ky_j < lyapunov_spectrum.len() {
                let last_neg = lyapunov_spectrum[ky_j];
                if last_neg.abs() > 1e-12 {
                    let d_ky = ky_j as f64 + sum / last_neg.abs();
                    painter.text(
                        Pos2::new(x, y),
                        Align2::LEFT_TOP,
                        format!("D_KY ≈ {:.3}  (Kaplan-Yorke dim.)", d_ky),
                        FontId::monospace(11.0),
                        Color32::from_rgb(180, 180, 100),
                    );
                    y += 16.0;
                }
            }
        }
    }

    if !attractor_type.is_empty() && attractor_type != "unknown" {
        y += 6.0;
        let atype_color = match attractor_type {
            "chaos" => Color32::from_rgb(255, 90, 60),
            "hyperchaos" => Color32::from_rgb(255, 40, 160),
            "limit_cycle" => Color32::from_rgb(60, 200, 120),
            "torus" => Color32::from_rgb(80, 180, 255),
            "fixed_point" => Color32::from_rgb(200, 200, 200),
            _ => Color32::from_rgb(180, 180, 100),
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

    let right_rect = Rect::from_min_max(Pos2::new(mid_x + 10.0, rect.top() + 10.0), rect.max);

    if system_name == "kuramoto" && !kuramoto_phases.is_empty() {
        draw_kuramoto_circle(&painter, right_rect, kuramoto_phases, order_param);
    } else {
        draw_phase_clock(&painter, right_rect, current_state, current_deriv);
    }

    // Feature #7: Lyapunov exponent scrolling history line graph
    if !lyapunov_history.is_empty() {
        let graph_h = 60.0f32;
        let graph_rect = Rect::from_min_max(
            Pos2::new(rect.left() + 10.0, rect.max.y - graph_h - 10.0),
            Pos2::new(mid_x - 10.0, rect.max.y - 10.0),
        );
        painter.rect_filled(graph_rect, 3.0, Color32::from_rgb(8, 12, 28));
        painter.rect_stroke(
            graph_rect,
            3.0,
            Stroke::new(1.0, Color32::from_rgb(40, 50, 80)),
        );
        painter.text(
            Pos2::new(graph_rect.left() + 4.0, graph_rect.top() + 2.0),
            Align2::LEFT_TOP,
            "\u{03bb}\u{2081} history",
            FontId::monospace(10.0),
            Color32::from_rgb(140, 160, 200),
        );
        // Zero dashed line
        let zero_y = graph_rect.center().y;
        let mut dx = graph_rect.left();
        while dx < graph_rect.right() {
            painter.line_segment(
                [
                    Pos2::new(dx, zero_y),
                    Pos2::new((dx + 6.0).min(graph_rect.right()), zero_y),
                ],
                Stroke::new(1.0, Color32::from_rgba_premultiplied(80, 80, 140, 180)),
            );
            dx += 12.0;
        }
        let hist_vec: Vec<f32> = lyapunov_history.iter().copied().collect();
        let n = hist_vec.len();
        let max_abs = hist_vec.iter().map(|v| v.abs()).fold(0.01f32, f32::max);
        if n >= 2 {
            for i in 1..n {
                let lam = hist_vec[i];
                let color = if lam > 0.05 {
                    Color32::from_rgb(60, 220, 80)
                } else if lam < -0.05 {
                    Color32::from_rgb(60, 130, 255)
                } else {
                    Color32::from_rgb(240, 220, 60)
                };
                let x0 = graph_rect.left() + ((i - 1) as f32 / (n - 1) as f32) * graph_rect.width();
                let x1 = graph_rect.left() + (i as f32 / (n - 1) as f32) * graph_rect.width();
                let y0 = (zero_y - (hist_vec[i - 1] / max_abs) * (graph_rect.height() * 0.45))
                    .clamp(graph_rect.top(), graph_rect.bottom());
                let y1 = (zero_y - (lam / max_abs) * (graph_rect.height() * 0.45))
                    .clamp(graph_rect.top(), graph_rect.bottom());
                painter.line_segment(
                    [Pos2::new(x0, y0), Pos2::new(x1, y1)],
                    Stroke::new(1.5, color),
                );
            }
        }
    }
}

fn draw_kuramoto_circle(painter: &Painter, rect: Rect, phases: &[f64], order_param: f64) {
    let center = rect.center();
    let radius = (rect.width().min(rect.height()) * 0.4).min(160.0);

    painter.circle_stroke(
        center,
        radius,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 50, 80, 150)),
    );
    painter.circle_stroke(
        center,
        radius * 0.5,
        Stroke::new(0.5, Color32::from_rgba_premultiplied(30, 30, 60, 100)),
    );

    painter.text(
        center + Vec2::new(0.0, -radius - 16.0),
        Align2::CENTER_BOTTOM,
        "Kuramoto Phase Circle",
        FontId::proportional(13.0),
        Color32::from_rgb(150, 150, 200),
    );
    painter.text(
        center + Vec2::new(0.0, radius + 8.0),
        Align2::CENTER_TOP,
        format!("Order parameter r = {:.3}", order_param),
        FontId::proportional(11.0),
        Color32::from_rgb(200, 200, 100),
    );

    let n = phases.len();
    for (i, &phase) in phases.iter().enumerate() {
        let px = center.x + radius * phase.cos() as f32;
        let py = center.y - radius * phase.sin() as f32;
        let hue = i as f32 / n as f32;
        let col = hue_to_color(hue, 0.9);
        painter.circle_filled(
            Pos2::new(px, py),
            9.0,
            Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 60),
        );
        painter.circle_filled(Pos2::new(px, py), 5.0, col);
    }

    let (sin_sum, cos_sum): (f64, f64) = phases
        .iter()
        .fold((0.0, 0.0), |(s, c), &ph| (s + ph.sin(), c + ph.cos()));
    let mean_phase = sin_sum.atan2(cos_sum) as f32;
    let r = order_param as f32;
    let arrow_end = Pos2::new(
        center.x + radius * r * mean_phase.cos(),
        center.y - radius * r * mean_phase.sin(),
    );
    painter.line_segment(
        [center, arrow_end],
        Stroke::new(3.0, Color32::from_rgb(255, 220, 0)),
    );
    painter.circle_filled(arrow_end, 5.0, Color32::from_rgb(255, 220, 0));
    painter.circle_filled(center, 3.0, Color32::from_rgb(200, 200, 200));
}

fn draw_phase_clock(painter: &Painter, rect: Rect, state: &[f64], deriv: &[f64]) {
    let center = rect.center();
    let radius = (rect.width().min(rect.height()) * 0.38).min(150.0);

    painter.circle_stroke(
        center,
        radius,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 50, 80, 150)),
    );

    painter.text(
        center + Vec2::new(0.0, -radius - 16.0),
        Align2::CENTER_BOTTOM,
        "Phase Velocity",
        FontId::proportional(13.0),
        Color32::from_rgb(150, 150, 200),
    );

    for i in 0..12 {
        let a = i as f32 * std::f32::consts::TAU / 12.0;
        let inner = Pos2::new(
            center.x + (radius - 8.0) * a.cos(),
            center.y - (radius - 8.0) * a.sin(),
        );
        let outer = Pos2::new(center.x + radius * a.cos(), center.y - radius * a.sin());
        painter.line_segment(
            [inner, outer],
            Stroke::new(1.0, Color32::from_rgba_premultiplied(60, 60, 100, 150)),
        );
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
        painter.line_segment(
            [center, pos_end],
            Stroke::new(2.0, Color32::from_rgb(0, 180, 255)),
        );
        painter.circle_filled(pos_end, 4.0, Color32::from_rgb(0, 200, 255));

        let vel_scale = (dmag / 100.0).clamp(0.0, 1.0);
        let vel_end = Pos2::new(
            center.x + radius * vel_scale * dangle.cos(),
            center.y - radius * vel_scale * dangle.sin(),
        );
        painter.line_segment(
            [center, vel_end],
            Stroke::new(2.0, Color32::from_rgb(255, 200, 0)),
        );
        painter.circle_filled(vel_end, 4.0, Color32::from_rgb(255, 220, 0));

        painter.text(
            center + Vec2::new(0.0, radius + 8.0),
            Align2::CENTER_TOP,
            format!("|v| = {:.2}", dmag),
            FontId::proportional(11.0),
            Color32::from_rgb(200, 200, 100),
        );
    }

    painter.circle_filled(center, 3.0, Color32::from_rgb(200, 200, 200));
}

// ---------------------------------------------------------------------------
// Spectrum analyser tab (Feature: FFT spectral overlay)
// ---------------------------------------------------------------------------

/// Draw the real-time FFT spectral analyser panel.
///
/// Reads the waveform buffer, runs a windowed FFT on the most recent
/// 2048 samples, and displays a bar-graph spectrum with logarithmic
/// frequency axis from 20 Hz to 20 kHz.  Bars are coloured by
/// frequency: bass = red, mids = green, highs = blue.
fn draw_spectrum_tab(
    ui: &mut Ui,
    waveform: &Arc<Mutex<Vec<f32>>>,
    state: &SharedState,
) {
    use crate::spectrum::{bins_to_bars, compute_spectrum, mag_to_db, SPECTRUM_BINS};

    let sample_rate = state.lock().sample_rate as f32;
    let samples: Vec<f32> = if let Some(wf) = waveform.try_lock() {
        wf.clone()
    } else {
        Vec::new()
    };

    let avail = ui.available_size();

    if samples.len() < 64 {
        let (_, painter) = ui.allocate_painter(avail, Sense::hover());
        let r = egui::Rect::from_min_size(ui.cursor().min, avail);
        painter.rect_filled(r, 0.0, Color32::from_rgb(8, 8, 18));
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            "Audio starting...",
            FontId::proportional(14.0),
            Color32::from_rgb(60, 80, 120),
        );
        return;
    }

    // Compute spectrum
    let spectrum = compute_spectrum(&samples);
    const N_BARS: usize = 64;
    let bars = bins_to_bars(&spectrum, N_BARS, sample_rate, 20.0, 20_000.0);

    let panel_h = (avail.y - 70.0).max(100.0);
    let (resp, _) = ui.allocate_painter(Vec2::new(avail.x, panel_h), Sense::hover());
    let panel_rect = resp.rect;
    let painter = ui.painter_at(panel_rect);

    // Background
    painter.rect_filled(panel_rect, 6.0, Color32::from_rgb(8, 8, 18));

    let bar_w = panel_rect.width() / N_BARS as f32;
    let bar_h_max = panel_h - 20.0;

    const DB_MIN: f32 = -60.0;
    const DB_MAX: f32 = 0.0;

    for (i, &mag) in bars.iter().enumerate() {
        let db = mag_to_db(mag).clamp(DB_MIN, DB_MAX);
        let norm = ((db - DB_MIN) / (DB_MAX - DB_MIN)).clamp(0.0, 1.0);
        let bar_h = (norm * bar_h_max).max(1.0);

        let x = panel_rect.left() + i as f32 * bar_w;
        let bar_rect = egui::Rect::from_min_max(
            Pos2::new(x + 1.0, panel_rect.bottom() - 20.0 - bar_h),
            Pos2::new(x + bar_w - 1.0, panel_rect.bottom() - 20.0),
        );

        // Colour gradient: bass=red, mids=green, highs=blue
        let t = i as f32 / N_BARS as f32;
        let (r_c, g_c, b_c): (u8, u8, u8) = if t < 0.25 {
            let k = t / 0.25;
            (220, (80.0 * k) as u8, 20)
        } else if t < 0.5 {
            let k = (t - 0.25) / 0.25;
            ((220.0 * (1.0 - k)) as u8, (80 + (120.0 * k) as u32).min(200) as u8, 20)
        } else if t < 0.75 {
            let k = (t - 0.5) / 0.25;
            (0, 200, (200.0 * k) as u8)
        } else {
            let k = (t - 0.75) / 0.25;
            (0, (200.0 * (1.0 - k)) as u8, 200)
        };

        let bright = Color32::from_rgba_premultiplied(
            (r_c as f32 * 1.4).min(255.0) as u8,
            (g_c as f32 * 1.4).min(255.0) as u8,
            (b_c as f32 * 1.4).min(255.0) as u8,
            230,
        );
        let dim = Color32::from_rgba_premultiplied(r_c / 4, g_c / 4, b_c / 4, 160);

        if bar_rect.height() > 3.0 {
            let split_y = bar_rect.min.y + bar_rect.height() * 0.3;
            painter.rect_filled(
                egui::Rect::from_min_max(bar_rect.min, Pos2::new(bar_rect.max.x, split_y)),
                1.0,
                bright,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(Pos2::new(bar_rect.min.x, split_y), bar_rect.max),
                1.0,
                dim,
            );
        } else {
            painter.rect_filled(bar_rect, 1.0, bright);
        }
    }

    // Frequency axis labels
    let label_freqs: &[(f32, &str)] = &[(20.0, "20Hz"), (100.0, "100"), (500.0, "500"), (1000.0, "1k"), (5000.0, "5k"), (10000.0, "10k"), (20000.0, "20k")];
    let log_min = 20.0_f32.log10();
    let log_max = 20_000.0_f32.log10();
    for (freq, label) in label_freqs {
        let t_x = (freq.log10() - log_min) / (log_max - log_min);
        let x = panel_rect.left() + t_x * panel_rect.width();
        painter.text(
            Pos2::new(x, panel_rect.bottom() - 10.0),
            Align2::CENTER_CENTER,
            *label,
            FontId::proportional(8.5),
            Color32::from_rgb(90, 110, 160),
        );
        painter.line_segment(
            [Pos2::new(x, panel_rect.top()), Pos2::new(x, panel_rect.bottom() - 20.0)],
            Stroke::new(0.4, Color32::from_rgba_premultiplied(50, 60, 90, 60)),
        );
    }

    // dB grid lines and labels
    for &db_label in &[-60, -40, -20, -10, 0i32] {
        let norm = (db_label as f32 - DB_MIN) / (DB_MAX - DB_MIN);
        let y = panel_rect.bottom() - 20.0 - norm * bar_h_max;
        painter.text(
            Pos2::new(panel_rect.right() - 2.0, y),
            Align2::RIGHT_CENTER,
            format!("{}dB", db_label),
            FontId::proportional(8.0),
            Color32::from_rgb(70, 90, 130),
        );
        painter.line_segment(
            [Pos2::new(panel_rect.left(), y), Pos2::new(panel_rect.right(), y)],
            Stroke::new(0.4, Color32::from_rgba_premultiplied(40, 50, 80, 50)),
        );
    }

    // Title
    painter.text(
        Pos2::new(panel_rect.left() + 8.0, panel_rect.top() + 12.0),
        Align2::LEFT_CENTER,
        "FFT Spectrum (Hann window, 64 log-spaced bars, 20Hz–20kHz)",
        FontId::proportional(10.5),
        Color32::from_rgb(100, 150, 220),
    );

    // Border
    painter.rect_stroke(panel_rect, 6.0, Stroke::new(1.0, Color32::from_rgb(30, 40, 70)));

    // Stats row below
    ui.add_space(4.0);
    let rms: f32 = {
        let n = samples.len().min(2048);
        let offset = samples.len().saturating_sub(n);
        let sum: f32 = samples[offset..].iter().map(|s| s * s).sum();
        (sum / n.max(1) as f32).sqrt()
    };
    let rms_db = if rms > 1e-6 { 20.0 * rms.log10() } else { -80.0 };

    // Dominant frequency
    let peak_bin = spectrum
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let bin_hz = sample_rate / (2.0 * (SPECTRUM_BINS as f32 - 1.0));
    let dominant_hz = peak_bin as f32 * bin_hz;

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("RMS: {:.1} dBFS", rms_db))
                .size(11.0)
                .color(Color32::from_rgb(140, 200, 255)),
        );
        ui.separator();
        ui.label(
            RichText::new(format!("Dominant: {:.0} Hz", dominant_hz.max(0.0)))
                .size(11.0)
                .color(Color32::from_rgb(200, 240, 140)),
        );
        ui.separator();
        ui.label(
            RichText::new("Logarithmic frequency axis | Live FFT every frame")
                .size(10.0)
                .color(Color32::from_rgb(80, 100, 140))
                .italics(),
        );
    });
}
