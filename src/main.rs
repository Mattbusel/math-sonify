mod systems;
mod sonification;
mod synth;
mod audio;
mod config;
mod ui;
mod patches;
mod arrangement;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::bounded;
use parking_lot::Mutex;

use crate::config::{Config, load_config};
use crate::arrangement::{lerp_config, total_duration, scene_at};
use crate::systems::{*, CustomOde, FractionalLorenz};
use crate::sonification::{
    AudioParams, Sonification, SonifMode,
    DirectMapping, OrbitalResonance, GranularMapping, SpectralMapping, FmMapping, VocalMapping,
    chord_intervals_for,
};
use crate::audio::{AudioEngine, WavRecorder, LoopExportPending, VuMeter, SidechainLevel, ClipBuffer};
use crate::synth::OscShape;
use midir;
use crate::ui::{AppState, SharedState, draw_ui};

// Channel capacity (sim -> audio). Only the latest value matters.
const CHANNEL_CAP: usize = 16;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config_path = std::path::PathBuf::from("config.toml");
    let config = load_config(&config_path);

    // Shared state for UI <-> sim communication
    let shared = Arc::new(Mutex::new(AppState::new(config.clone())));

    // Visualization history (shared between sim and UI)
    let viz_history: Arc<Mutex<Vec<(f32, f32, f32, f32, bool)>>> =
        Arc::new(Mutex::new(Vec::with_capacity(2000)));

    // Waveform capture buffer
    let waveform_buf: Arc<parking_lot::Mutex<Vec<f32>>> =
        Arc::new(parking_lot::Mutex::new(Vec::with_capacity(2048)));

    // WAV recording shared state
    let recording: WavRecorder = Arc::new(parking_lot::Mutex::new(None));

    // Loop export pending
    let loop_export: LoopExportPending = Arc::new(parking_lot::Mutex::new(None));

    // Bifurcation data
    let bifurc_data: Arc<Mutex<Vec<(f32, f32)>>> = Arc::new(Mutex::new(Vec::new()));

    // Channel: sim thread -> audio thread (layer batch)
    let (tx, rx) = bounded::<[Option<AudioParams>; 3]>(CHANNEL_CAP);

    // VU meter shared state
    let vu_meter: VuMeter = Arc::new(Mutex::new([0.0; 4]));

    // Sidechain level
    let sidechain_level: SidechainLevel = Arc::new(Mutex::new(0.0));

    // Clip buffer (~60s stereo audio)
    let clip_buffer: ClipBuffer = Arc::new(Mutex::new(std::collections::VecDeque::new()));

    // Audio engine
    let (_audio, actual_sr) = AudioEngine::start(
        rx,
        config.audio.sample_rate,
        config.audio.reverb_wet,
        config.audio.delay_ms,
        config.audio.delay_feedback,
        config.audio.master_volume,
        waveform_buf.clone(),
        recording.clone(),
        loop_export.clone(),
        vu_meter.clone(),
        clip_buffer.clone(),
        sidechain_level.clone(),
    )?;

    // Store actual sample rate and shared state in AppState
    {
        let mut st = shared.lock();
        st.sample_rate = actual_sr;
        st.vu_meter = vu_meter;
        st.clip_buffer = clip_buffer;
        st.sidechain_level_shared = sidechain_level.clone();
    }

    // Simulation thread
    let shared_sim = shared.clone();
    let viz_sim = viz_history.clone();
    thread::spawn(move || {
        sim_thread(shared_sim, viz_sim, tx);
    });

    // MIDI output thread
    start_midi_thread(shared.clone());

    // MIDI input thread
    start_midi_input_thread(shared.clone());

    // UI (eframe -- runs on main thread)
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_title("Math Sonify"),
        ..Default::default()
    };

    eframe::run_native(
        "Math Sonify",
        options,
        Box::new(move |_cc| {
            Box::new(SonifyApp {
                shared: shared.clone(),
                viz_history: viz_history.clone(),
                waveform_buf: waveform_buf.clone(),
                recording: recording.clone(),
                loop_export: loop_export.clone(),
                bifurc_data: bifurc_data.clone(),
                shutdown_timer: None,
            })
        }),
    ).map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    // WOUND HEALING: clean exit — remove flag so next launch isn't "wounded"
    let _ = std::fs::remove_file("running.flag");

    Ok(())
}

struct SonifyApp {
    shared: SharedState,
    viz_history: Arc<Mutex<Vec<(f32, f32, f32, f32, bool)>>>,
    waveform_buf: Arc<parking_lot::Mutex<Vec<f32>>>,
    recording: WavRecorder,
    loop_export: LoopExportPending,
    bifurc_data: Arc<Mutex<Vec<(f32, f32)>>>,
    // DYING GRACEFULLY: fade on close
    shutdown_timer: Option<std::time::Instant>,
}

impl eframe::App for SonifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // DYING GRACEFULLY — intercept close request, fade over 3 seconds
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.shutdown_timer.is_none() {
                self.shutdown_timer = Some(std::time::Instant::now());
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.shared.lock().shutdown_fading = true;
            }
        }
        if let Some(t) = self.shutdown_timer {
            let fade_secs = t.elapsed().as_secs_f32();
            if fade_secs >= 3.0 {
                let _ = std::fs::remove_file("running.flag");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        let points = self.viz_history.lock().clone();
        draw_ui(ctx, &self.shared, &points, &self.waveform_buf, &self.recording,
                &self.loop_export, &self.bifurc_data);
        ctx.request_repaint_after(Duration::from_millis(33));
    }
}

// ---------------------------------------------------------------------------
// Simulation thread
// ---------------------------------------------------------------------------

struct ExtraLayer {
    system: Box<dyn DynamicalSystem>,
    mapper: Box<dyn Sonification>,
    config: Config,
}

fn sim_thread(
    shared: SharedState,
    viz: Arc<Mutex<Vec<(f32, f32, f32, f32, bool)>>>,
    tx: crossbeam_channel::Sender<[Option<AudioParams>; 3]>,
) {
    let initial_config = shared.lock().config.clone();
    let mut system = build_system(&initial_config);
    let mut mapper = build_mapper(&initial_config.sonification.mode);
    // Extra polyphony layers (indices 1 and 2)
    let mut extra_layers: [Option<ExtraLayer>; 2] = [None, None];
    // Track last arrangement system/mode to detect changes without comparing to UI AppState
    let mut last_arr_sys: String = initial_config.system.name.clone();
    let mut last_arr_mode: String = initial_config.sonification.mode.clone();

    let control_rate_hz = 120.0f64;
    let control_period = Duration::from_secs_f64(1.0 / control_rate_hz);
    let mut next_tick = Instant::now();

    // ── Time-of-day awareness (invisible — no UI) ──────────────────────────────
    // 0.0 = midnight/dark, 1.0 = noon/bright
    let time_of_day: f32 = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let hour_frac = (secs % 86400) as f64 / 86400.0;
        (-(std::f64::consts::TAU * hour_frac).cos() * 0.5 + 0.5) as f32
    };
    // Bias initial macros subtly: night → slower/darker/richer reverb; day → brighter/faster
    {
        let mut st = shared.lock();
        let night = 1.0 - time_of_day;
        st.macro_chaos  = (st.macro_chaos  - night * 0.10).clamp(0.05, 0.95);
        st.macro_space  = (st.macro_space  + night * 0.18).clamp(0.05, 0.95);
        st.macro_rhythm = (st.macro_rhythm - night * 0.12).clamp(0.05, 0.95);
        st.macro_warmth = (st.macro_warmth + night * 0.15).clamp(0.05, 0.95);
    }

    // Seasonal drift: fraction of year (0=Jan 1, 0.5≈Jul 2)
    // Used to gently bias base frequency over the calendar year (±1.5%)
    let seasonal_freq_mult: f64 = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let day_frac = (secs / 86400 % 365) as f64 / 365.0;
        // Sine wave: peaks in summer (day ~172), troughs in winter (day ~355)
        let angle = std::f64::consts::TAU * (day_frac - 172.0 / 365.0);
        1.0 + angle.sin() * 0.015 // ±1.5% range
    };

    // ── WOUND HEALING: detect crash from previous session ─────────────────────
    let wounded = std::path::Path::new("running.flag").exists();
    let mut wound_t: f32 = if wounded { 0.0 } else { 1.0 };
    std::fs::write("running.flag", b"1").ok();
    { shared.lock().wounded = wounded; }

    // ── STARTUP RAMP: ramp from silence over 2 seconds ────────────────────────
    let mut startup_ramp_t: f32 = 0.0;
    { shared.lock().startup_ramp_t = startup_ramp_t; }

    // ── HOUR OF DAY for circadian sleep ───────────────────────────────────────
    let hour_of_day: u32 = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        ((secs % 86400) / 3600) as u32
    };

    // Push time_of_day to shared state for PHOTOTROPISM
    { shared.lock().time_of_day_f = time_of_day; }

    // ── ATTRACTOR state persistence ────────────────────────────────────────────
    // Load the last saved state so the attractor resumes from where it was
    {
        let state_path = std::path::PathBuf::from("attractor_state.bin");
        if state_path.exists() {
            if let Ok(bytes) = std::fs::read(&state_path) {
                let floats: Vec<f64> = bytes.chunks_exact(8)
                    .filter_map(|c| {
                        let arr: [u8; 8] = c.try_into().ok()?;
                        let v = f64::from_le_bytes(arr);
                        if v.is_finite() { Some(v) } else { None }
                    })
                    .collect();
                if floats.len() >= 3 {
                    system.set_state(&floats);
                }
            }
        }
    }

    // ── Gravitational memory ───────────────────────────────────────────────────
    // 20×20 histogram of (macro_chaos × macro_space) regions visited.
    // The walk is gently nudged toward frequently-visited areas over sessions.
    let gravity_path = std::path::PathBuf::from("gravity_map.bin");
    let mut gravity_map = vec![1.0f32; 400];
    if gravity_path.exists() {
        if let Ok(bytes) = std::fs::read(&gravity_path) {
            for (i, chunk) in bytes.chunks_exact(4).take(400).enumerate() {
                if let Ok(arr) = chunk.try_into() {
                    let v = f32::from_le_bytes(arr);
                    if v.is_finite() && v >= 0.0 { gravity_map[i] = v; }
                }
            }
        }
    }

    // ── Idle excursion state ───────────────────────────────────────────────────
    // Every 10-20 min when Evolve is on: brief parameter bloom then settle back
    let mut idle_ticks: u64 = 0;
    let mut excursion_active = false;
    let mut excursion_ticks: u64 = 0;
    let mut excursion_return_chaos: f32 = 0.5;
    let mut excursion_return_speed: f64 = 2.0;
    // Next excursion fires between 10–20 minutes (at 120 Hz)
    let mut walk_seed_ex: u64 = 0xDEADBEEF ^ std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos() as u64;
    let lcg = |s: &mut u64| -> f32 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*s >> 33) as f32 / u32::MAX as f32
    };
    let mut next_excursion_tick: u64 = {
        let r = lcg(&mut walk_seed_ex);
        (10 * 60 * 120) + (r * 10.0 * 60.0 * 120.0) as u64 // 10-20 min in ticks
    };

    // ── Breathing oscillator ──────────────────────────────────────────────────
    // ~4.5s cycle, ±0.3 dB (linear ≈ ±0.034) — subliminal organic warmth
    let mut breathing_phase: f64 = 0.0;
    const BREATHING_RATE: f64 = 1.0 / 4.5;

    // ── State + gravity save timer ────────────────────────────────────────────
    let mut state_save_timer: u64 = 0;
    const STATE_SAVE_INTERVAL: u64 = 120 * 120; // every 2 min at 120 Hz

    // ── Lyapunov spectrum timer ───────────────────────────────────────────────
    let mut lyap_timer: u64 = 0;
    const LYAP_INTERVAL: u64 = 600; // every ~5s at 120 Hz
    let mut lyap_cycles: u64 = 0;

    // ── Trajectory buffer for analysis (permutation entropy, RQA, etc.) ───────
    let mut analysis_trajectory: Vec<Vec<f64>> = Vec::with_capacity(500);

    // ── Session transcript timer ──────────────────────────────────────────────
    let mut session_log_timer: u64 = 0;
    const SESSION_LOG_INTERVAL: u64 = 120 * 60; // every 60s at 120Hz

    // Macro random walk seed
    let mut walk_seed: u64 = 12345;

    // ── Lunar phase (29.5-day cycle, invisible palette influence) ─────────────
    let lunar_phase: f32 = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        // Known new moon: Jan 6, 2000 = Unix 946684800
        let days_since_new = (secs.saturating_sub(946684800)) as f64 / 86400.0;
        let phase = (days_since_new % 29.53058770576) / 29.53058770576;
        // 0.5 = full moon. Map to 0.0=new 1.0=full 0.0=new via triangle:
        let triangle = if phase < 0.5 { phase * 2.0 } else { (1.0 - phase) * 2.0 };
        triangle as f32
    };
    // Push to shared state so draw_phase_portrait can use it
    { shared.lock().lunar_phase = lunar_phase; }

    // ── Attractor aging: instrument warms up over first hour ──────────────────
    let aging_path = std::path::PathBuf::from("aging.bin");
    let mut aging_secs: f32 = {
        if aging_path.exists() {
            std::fs::read(&aging_path).ok()
                .and_then(|b| b.get(0..4).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes))
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(0.0)
        } else { 0.0 }
    };
    { shared.lock().aging_secs = aging_secs; }

    // ── Entropy accumulation: instrument gains confidence with use ─────────────
    let entropy_path = std::path::PathBuf::from("entropy.bin");
    let mut entropy_pool: f32 = {
        if entropy_path.exists() {
            std::fs::read(&entropy_path).ok()
                .and_then(|b| b.get(0..4).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes))
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(0.0)
        } else { 0.0 }
    };
    // Sync with AppState (UI may have deposited entropy already)
    { entropy_pool = entropy_pool.max(shared.lock().entropy_pool); }

    // ── Volume creep: draws listener closer over an hour ──────────────────────
    // 1.0 → 0.87 over 3600 seconds (≈ -1.2 dB). Reset when user touches volume.
    let mut volume_creep: f32 = 1.0;
    let mut volume_creep_last_vol: f32 = { shared.lock().config.audio.master_volume };
    // Decay rate: reach 0.87 in 3600s * 120Hz = 432000 ticks
    // 0.87^(1/432000) ≈ 1 - 3.24e-7 per tick
    const VOLUME_CREEP_RATE: f32 = 3.24e-7;
    const VOLUME_CREEP_MIN: f32 = 0.87; // floor

    // ── Fingerprint: unique initial conditions on very first ever launch ───────
    // If no state file exists, seed a unique starting region from machine identity
    {
        let state_path_fp = std::path::PathBuf::from("attractor_state.bin");
        if !state_path_fp.exists() {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
            let pid = std::process::id() as u64;
            let hostname = std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "math_sonify".into());
            let host_hash: u64 = hostname.bytes().fold(0u64, |acc, b| {
                acc.wrapping_mul(6364136223846793005).wrapping_add(b as u64)
            });
            let mut fp = ts ^ pid.wrapping_mul(0xDEAD) ^ host_hash ^ 0xCAFEBABEDEADBEEF;
            let fp_f = |s: &mut u64| -> f64 {
                *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((*s >> 33) as f64 / u32::MAX as f64) * 2.0 - 1.0
            };
            // Stay within attractor basin
            let ix = fp_f(&mut fp) * 18.0;
            let iy = fp_f(&mut fp) * 18.0;
            let iz = 15.0 + fp_f(&mut fp).abs() * 20.0;
            system.set_state(&[ix, iy, iz]);
        }
    }

    // ── Attractor dreams: system briefly visits a different universe ───────────
    // After 30+ min idle with Evolve on, switch to another system for 60-90s then morph back
    let mut dream_active = false;
    let mut dream_ticks: u64 = 0;
    let mut dream_total_ticks: u64 = 0;
    let mut dream_return_config: Option<crate::config::Config> = None;
    let mut dream_target_config: Option<crate::config::Config> = None;
    let dream_idle_threshold: u64 = 30 * 60 * 120; // 30 min at 120Hz
    let mut dream_idle_ticks: u64 = 0; // separate from excursion idle counter

    // ── Typing resonance: keyboard cadence subtly influences Evolve wander ────
    // Poll OS key state at 10 Hz in a background thread (Windows only).
    // Count transitions to estimate typing rate without a full hook.
    use std::sync::atomic::{AtomicU32, Ordering};
    let typing_rate_atomic = std::sync::Arc::new(AtomicU32::new(0u32));
    {
        let typing_arc = typing_rate_atomic.clone();
        std::thread::spawn(move || {
            let mut prev_states = [0i16; 128];
            let mut key_count = 0u32;
            let mut report_ticks = 0u32;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
                // Poll common key codes (letters, numbers, space, return)
                #[cfg(target_os = "windows")]
                {
                    extern "system" { fn GetAsyncKeyState(vKey: i32) -> i16; }
                    for vk in (65i32..=90).chain(48..=57).chain([32, 13, 8, 9]) {
                        let idx = vk as usize;
                        if idx < 128 {
                            let cur = unsafe { GetAsyncKeyState(vk) };
                            if (cur & 1) != 0 && (prev_states[idx] & 1) == 0 {
                                key_count += 1;
                            }
                            prev_states[idx] = cur;
                        }
                    }
                }
                report_ticks += 1;
                if report_ticks >= 10 { // report every ~1 second
                    // Store as fixed-point: keys_per_second * 100 as u32
                    let kps_fixed = (key_count * 10).min(2000); // cap at 20 kps
                    typing_arc.store(kps_fixed, Ordering::Relaxed);
                    key_count = 0;
                    report_ticks = 0;
                }
            }
        });
    }

    // ── Instance empathy: two copies of Math Sonify on the same machine ────────
    // whisper chaos level to each other through a local UDP port.
    use std::sync::atomic::AtomicU32 as AtomicU32Emp;
    let empathy_rx = std::sync::Arc::new(AtomicU32Emp::new(u32::MAX)); // MAX = no data yet
    {
        let rx_arc = empathy_rx.clone();
        std::thread::spawn(move || {
            // Try port 47832 first (primary), then 47833 (secondary listens on primary's port)
            let our_port = 47832u16;
            // Try to be the listener on our_port
            if let Ok(sock) = std::net::UdpSocket::bind(format!("127.0.0.1:{}", our_port)) {
                let _ = sock.set_read_timeout(Some(std::time::Duration::from_millis(200)));
                let mut buf = [0u8; 4];
                loop {
                    if let Ok((4, _)) = sock.recv_from(&mut buf) {
                        let val = u32::from_le_bytes(buf);
                        rx_arc.store(val, Ordering::Relaxed);
                    }
                }
            }
            // Port taken — we're a secondary instance. Nothing to do (sender is in sim loop).
        });
    }
    // Sender socket (non-blocking, best effort)
    let empathy_sender = std::net::UdpSocket::bind("127.0.0.1:47833")
        .ok(); // None if port taken (primary instance handles this)

    // Automation state
    let mut auto_snapshot_timer = 0u64;
    let auto_snapshot_interval = 12u64; // every ~100ms at 120Hz

    // Coupled attractor system
    let mut coupled_system: Option<Box<dyn DynamicalSystem>> = None;
    let mut coupled_system_name: String = String::new();
    // min/max tracker for normalizing coupled output
    let mut coupled_min = -30.0f64;
    let mut coupled_max = 30.0f64;

    // ── METABOLISM: resting drift speed ───────────────────────────────────────
    // When paused, step at 1.5% of normal — keeps attractor alive like breathing

    // ── WARMUP: sluggish response for 5s after large speed changes ────────────
    let mut smoothed_speed: f64 = initial_config.system.speed;
    let mut warmup_ticks_remaining: i32 = 0;
    let mut prev_speed_for_warmup: f64 = initial_config.system.speed;

    // ── FLINCHING: 80-125ms delay on violent slider changes ───────────────────
    let mut flinch_remaining: i32 = 0;
    let mut flinch_held_speed: f64 = initial_config.system.speed;
    let mut prev_speed_for_flinch: f64 = initial_config.system.speed;

    // ── COOLDOWN: elevated wander after intense sessions ──────────────────────
    let mut activity_energy: f32 = 0.0;
    let activity_decay_rate: f32 = 1.0 / (180.0 * 120.0); // 3-min half-life

    // ── NESTING: long-period oscillations after 2+ hours uptime ──────────────
    let mut uptime_ticks: u64 = 0;
    let mut nesting_phase: f64 = 0.0;

    // ── APPETITE: hunger toward spectral complexity after long idle ────────────
    // (uses existing silence_expanded)

    // ── EMPATHY WITH TEMPO: interaction rate adjusts Evolve speed ─────────────
    let mut interaction_ticks_since_change: u64 = 0;
    let mut prev_config_hash: u64 = 0;

    // ── SCARRING: near-divergence marks ────────────────────────────────────────
    let mut scars: Vec<(f32, f32)> = Vec::with_capacity(500);
    // Load existing scars
    {
        let scar_path = std::path::PathBuf::from("scars.bin");
        if scar_path.exists() {
            if let Ok(bytes) = std::fs::read(&scar_path) {
                for chunk in bytes.chunks_exact(8) {
                    let x = f32::from_le_bytes(chunk[0..4].try_into().unwrap_or([0;4]));
                    let y = f32::from_le_bytes(chunk[4..8].try_into().unwrap_or([0;4]));
                    if x.is_finite() && y.is_finite() {
                        scars.push((x, y));
                    }
                }
            }
        }
    }
    { shared.lock().scars = scars.clone(); }

    // ── PAIR BONDING: favorite presets become richer ───────────────────────────
    let mut preset_affinity: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
    {
        let aff_path = std::path::PathBuf::from("preset_affinity.bin");
        if aff_path.exists() {
            if let Ok(bytes) = std::fs::read(&aff_path) {
                for chunk in bytes.chunks_exact(12) {
                    let key = u64::from_le_bytes(chunk[0..8].try_into().unwrap_or([0;8]));
                    let val = u32::from_le_bytes(chunk[8..12].try_into().unwrap_or([0;4]));
                    preset_affinity.insert(key, val);
                }
            }
        }
    }

    loop {
        let now = Instant::now();
        if now < next_tick {
            thread::sleep(next_tick - now);
        }
        next_tick += control_period;

        let (paused, mut config, sys_changed, mode_changed, bpm_sync, bpm, auto_recording, auto_playing,
             poly_defs, sidechain_enabled, sidechain_level_shared, sidechain_target, sidechain_amount,
             coupled_enabled, coupled_src_name, coupled_strength, coupled_target_param, coupled_bidirectional) = {
            let mut st = shared.lock();
            let p = st.paused;
            let c = st.config.clone();
            let sc = st.system_changed;
            let mc = st.mode_changed;
            st.system_changed = false;
            st.mode_changed = false;
            let bs = st.bpm_sync;
            let bm = st.bpm;
            let ar = st.auto_recording;
            let ap = st.auto_playing;
            let pd = st.poly_layers.clone();
            let se = st.sidechain_enabled;
            let sl = st.sidechain_level_shared.clone();
            let st_target = st.sidechain_target.clone();
            let sa = st.sidechain_amount;
            let ce = st.coupled_enabled;
            let cs = st.coupled_source.clone();
            let cstr = st.coupled_strength;
            let ct = st.coupled_target.clone();
            let cbd = st.coupled_bidirectional;
            (p, c, sc, mc, bs, bm, ar, ap, pd, se, sl, st_target, sa, ce, cs, cstr, ct, cbd)
        };

        // METABOLISM: resting drift when paused — keeps attractor alive like breathing
        if paused {
            // Step at 1.5% of normal speed, no audio sent
            let metabolic_steps = ((config.system.speed * 0.015 / control_rate_hz) / config.system.dt)
                .round() as usize;
            for _ in 0..metabolic_steps.clamp(1, 100) {
                system.step(config.system.dt);
            }
            continue;
        }

        // UPTIME and WOUND HEALING increment
        uptime_ticks += 1;

        // Collect trajectory points for analysis (every 12 ticks = 10x subsampled at 120Hz)
        if uptime_ticks % 12 == 0 {
            if analysis_trajectory.len() >= 500 {
                analysis_trajectory.remove(0);
            }
            analysis_trajectory.push(system.state().to_vec());
        }
        wound_t = (wound_t + 1.0 / (20.0 * 60.0 * 120.0)).min(1.0);

        // STARTUP RAMP: ramp from 0 to 1 over 2 seconds (240 ticks)
        startup_ramp_t = (startup_ramp_t + 1.0 / (2.0 * 120.0)).min(1.0);
        { shared.lock().startup_ramp_t = startup_ramp_t; }

        let silence_secs = { shared.lock().last_interaction_time.elapsed().as_secs_f32() };
        let silence_expanded = silence_secs > 300.0;
        let entropy_walk_scale = {
            let e = shared.lock().entropy_pool.max(entropy_pool);
            entropy_pool = e;
            0.5 + (e / 1000.0).min(1.0)
        };
        let typing_kps = typing_rate_atomic.load(Ordering::Relaxed) as f32 / 100.0;
        let typing_walk_scale = 0.6 + (typing_kps / 10.0).min(1.0) * 0.8;

        if sys_changed  {
            system = if config.system.name == "custom" {
                let (ex, ey, ez) = {
                    let st = shared.lock();
                    (st.custom_ode_x.clone(), st.custom_ode_y.clone(), st.custom_ode_z.clone())
                };
                Box::new(CustomOde::new(ex, ey, ez))
            } else {
                build_system(&config)
            };
            last_arr_sys = config.system.name.clone();
        }
        if mode_changed { mapper = build_mapper(&config.sonification.mode); last_arr_mode = config.sonification.mode.clone(); }

        // PAIR BONDING: track sys_hash and increment affinity when system changes
        let sys_hash: u64 = config.system.name.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100000001b3)
        });
        if sys_changed {
            let count = preset_affinity.entry(sys_hash).or_insert(0);
            *count = count.saturating_add(1);
        }

        // ACTIVITY ENERGY: increment on large parameter changes (sys_changed is our proxy)
        if sys_changed {
            activity_energy = (activity_energy + 0.1).min(1.0);
        }
        // Decay activity energy (3-min half-life)
        activity_energy = (activity_energy - activity_decay_rate).max(0.0);

        // INTERACTION TEMPO: track config hash changes
        let current_config_hash: u64 = {
            let s = config.system.speed.to_bits();
            let r = config.lorenz.rho.to_bits();
            let sg = config.lorenz.sigma.to_bits();
            s ^ r.wrapping_mul(0x9e3779b9) ^ sg.wrapping_mul(0x6c62272e)
        };
        if current_config_hash != prev_config_hash {
            prev_config_hash = current_config_hash;
            interaction_ticks_since_change = 0;
        } else {
            interaction_ticks_since_change += 1;
        }

        // FLINCHING: 80-125ms delay on violent slider changes
        let raw_speed = config.system.speed;
        let flinch_delta = (raw_speed - prev_speed_for_flinch).abs();
        if flinch_delta > 3.0 && flinch_remaining <= 0 {
            // lcg for rand 0..5
            walk_seed_ex = walk_seed_ex.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let rand_extra = (walk_seed_ex >> 33) as i32 % 5;
            flinch_remaining = 10 + rand_extra;
            flinch_held_speed = prev_speed_for_flinch;
        }
        if flinch_remaining > 0 {
            config.system.speed = flinch_held_speed;
            flinch_remaining -= 1;
        } else {
            prev_speed_for_flinch = config.system.speed;
        }

        // WARMUP: slight drag on speed for 5s after large jumps (≤15% reduction, never kills audio)
        {
            let speed_delta = (config.system.speed - prev_speed_for_warmup).abs();
            if speed_delta > 2.5 {
                warmup_ticks_remaining = 600;
                smoothed_speed = config.system.speed;
            }
            prev_speed_for_warmup = config.system.speed;
            if warmup_ticks_remaining > 0 {
                // Fixed lerp rate toward target — only damps, never drives speed to zero
                smoothed_speed += (config.system.speed - smoothed_speed) * 0.04;
                let drag = warmup_ticks_remaining as f64 / 600.0; // 1.0 → 0.0 over 5s
                config.system.speed = config.system.speed * (1.0 - drag * 0.15) + smoothed_speed * (drag * 0.15);
                warmup_ticks_remaining -= 1;
            }
        }

        // CIRCADIAN SLEEP: 3am-5am — slower, dreamier, wider steps
        let circadian_sleep_active = hour_of_day >= 3 && hour_of_day < 5;
        if circadian_sleep_active {
            config.system.speed *= 0.85;
            config.audio.reverb_wet = (config.audio.reverb_wet + 0.08).min(0.95);
        }

        // Apply BPM sync to delay and LFO rate
        if bpm_sync {
            config.audio.delay_ms = 60000.0 / bpm;
            // BPM LFO sync handled in LFO block below
        }

        // Apply LFO modulation
        let (lfo_enabled, lfo_rate, lfo_depth, lfo_target, lfo_phase) = {
            let st = shared.lock();
            (st.lfo_enabled, st.lfo_rate, st.lfo_depth, st.lfo_target.clone(), st.lfo_phase)
        };

        let effective_lfo_rate = if bpm_sync {
            bpm / 60.0 * 0.25
        } else {
            lfo_rate
        };

        if lfo_enabled {
            let new_phase = lfo_phase + effective_lfo_rate as f64 * (1.0 / control_rate_hz);
            shared.lock().lfo_phase = new_phase;
            let lfo_val = new_phase.sin() * lfo_depth as f64;
            match lfo_target.as_str() {
                "sigma" => config.lorenz.sigma *= 1.0 + lfo_val,
                "rho"   => config.lorenz.rho *= 1.0 + lfo_val,
                "beta"  => config.lorenz.beta *= 1.0 + lfo_val,
                "a"     => config.rossler.a = (config.rossler.a * (1.0 + lfo_val)).max(0.001),
                "c"     => config.rossler.c *= 1.0 + lfo_val,
                "coupling" => config.kuramoto.coupling = (config.kuramoto.coupling * (1.0 + lfo_val)).max(0.0),
                "speed" => config.system.speed = (config.system.speed * (1.0 + lfo_val)).clamp(0.05, 20.0),
                _ => {}
            }
        }

        // Automation recording
        if auto_recording {
            auto_snapshot_timer += 1;
            if auto_snapshot_timer >= auto_snapshot_interval {
                auto_snapshot_timer = 0;
                let elapsed = {
                    let st = shared.lock();
                    st.auto_start_time.elapsed().as_secs_f64()
                };
                let events = vec![
                    (elapsed, "master_volume".to_string(), config.audio.master_volume as f64),
                    (elapsed, "reverb_wet".to_string(), config.audio.reverb_wet as f64),
                    (elapsed, "delay_ms".to_string(), config.audio.delay_ms as f64),
                    (elapsed, "speed".to_string(), config.system.speed),
                    (elapsed, "sigma".to_string(), config.lorenz.sigma),
                    (elapsed, "rho".to_string(), config.lorenz.rho),
                    (elapsed, "coupling".to_string(), config.kuramoto.coupling),
                ];
                let mut st = shared.lock();
                st.auto_events.extend(events);
            }
        }

        // Automation playback
        if auto_playing {
            let (pos, total, start_time) = {
                let st = shared.lock();
                (st.auto_play_pos, st.auto_events.len(), st.auto_start_time)
            };
            if total > 0 {
                let elapsed = start_time.elapsed().as_secs_f64();
                // Apply events up to current elapsed time
                let events_clone = shared.lock().auto_events.clone();
                let mut new_pos = pos;
                for (i, (t, ref param, val)) in events_clone.iter().enumerate() {
                    if i < pos { continue; }
                    if *t <= elapsed {
                        match param.as_str() {
                            "master_volume" => shared.lock().config.audio.master_volume = *val as f32,
                            "reverb_wet" => shared.lock().config.audio.reverb_wet = *val as f32,
                            "delay_ms" => shared.lock().config.audio.delay_ms = *val as f32,
                            "speed" => shared.lock().config.system.speed = *val,
                            "sigma" => shared.lock().config.lorenz.sigma = *val,
                            "rho" => shared.lock().config.lorenz.rho = *val,
                            "coupling" => shared.lock().config.kuramoto.coupling = *val,
                            _ => {}
                        }
                        new_pos = i + 1;
                    }
                }
                // Loop: if we've reached the end, reset
                if new_pos >= total {
                    shared.lock().auto_play_pos = 0;
                    shared.lock().auto_start_time = Instant::now();
                } else {
                    shared.lock().auto_play_pos = new_pos;
                }
            }
        }

        // Arrangement playback
        let arr_tick = {
            let mut st = shared.lock();
            if st.arr_playing {
                let dt_secs = 1.0 / control_rate_hz as f32;
                st.arr_elapsed += dt_secs;
                let elapsed = st.arr_elapsed;
                let total = total_duration(&st.scenes);
                if elapsed >= total {
                    if st.arr_probabilistic {
                        // Probabilistic: pick next scene by weighted random from active scenes
                        let active_indices: Vec<usize> = (0..st.scenes.len())
                            .filter(|&i| st.scenes[i].active)
                            .collect();
                        if !active_indices.is_empty() {
                            let total_w: f32 = active_indices.iter().map(|&i| st.scenes[i].transition_prob.max(0.0)).sum();
                            if total_w > 0.0 {
                                walk_seed = walk_seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                                let r = (walk_seed >> 33) as f32 / u32::MAX as f32 * total_w;
                                let mut acc = 0.0f32;
                                let mut chosen = active_indices[0];
                                for &i in &active_indices {
                                    acc += st.scenes[i].transition_prob.max(0.0);
                                    if r <= acc { chosen = i; break; }
                                }
                                // Reorder scenes so chosen is first; just restart from beginning
                                // but jump elapsed to start of that scene
                                let _ = chosen; // just loop from start for simplicity
                            }
                        }
                        st.arr_elapsed = 0.0;
                        Some(0.0)
                    } else if st.arr_loop {
                        st.arr_elapsed = elapsed % total.max(0.001);
                        Some(st.arr_elapsed)
                    } else {
                        st.arr_playing = false;
                        None
                    }
                } else {
                    Some(elapsed)
                }
            } else {
                None
            }
        };

        // If arrangement is playing, override config with interpolated value
        if let Some(elapsed) = arr_tick {
            let scenes = shared.lock().scenes.clone();
            if let Some((idx, is_morphing, t)) = scene_at(&scenes, elapsed) {
                let active_indices: Vec<usize> = (0..scenes.len()).filter(|&i| scenes[i].active).collect();
                let new_config = if is_morphing {
                    let ord = active_indices.iter().position(|&i| i == idx).unwrap_or(0);
                    if ord > 0 {
                        let prev_idx = active_indices[ord - 1];
                        lerp_config(&scenes[prev_idx].config, &scenes[idx].config, t)
                    } else {
                        scenes[idx].config.clone()
                    }
                } else {
                    scenes[idx].config.clone()
                };
                // Only rebuild system/mapper when the arrangement actually changes system or mode
                // Compare against last_arr_sys/mode (not AppState) to avoid rebuilding every tick
                if new_config.system.name != last_arr_sys {
                    last_arr_sys = new_config.system.name.clone();
                    system = build_system(&new_config);
                }
                if new_config.sonification.mode != last_arr_mode {
                    last_arr_mode = new_config.sonification.mode.clone();
                    mapper = build_mapper(&new_config.sonification.mode);
                }
                config = new_config;
            }
        }

        // Auto mode: if auto_mode is on and arrangement not playing, generate and start
        let (simple_mode, auto_mode) = {
            let st = shared.lock();
            (st.simple_mode, st.auto_mode)
        };
        if auto_mode && simple_mode {
            let arr_playing = { shared.lock().arr_playing };
            if !arr_playing {
                let mood = { shared.lock().arr_mood.clone() };
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64 ^ (d.subsec_nanos() as u64).wrapping_mul(0x9e3779b97f4a7c15))
                    .unwrap_or(0xdeadbeef);
                let mut st = shared.lock();
                st.scenes = crate::arrangement::generate_song(&mood, seed);
                st.arr_elapsed = 0.0;
                st.arr_playing = true;
                st.arr_loop = true;
            }
        }

        // Apply macro knobs (simple mode only)
        if simple_mode {
            let (macro_chaos, macro_space, macro_walk_enabled, macro_walk_rate) = {
                let st = shared.lock();
                (st.macro_chaos, st.macro_space, st.macro_walk_enabled, st.macro_walk_rate)
            };

            // Macro random walk (Brownian motion)
            if macro_walk_enabled {
                let dt = 1.0 / control_rate_hz as f32;
                let r = |seed: &mut u64| -> f32 {
                    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    (*seed >> 33) as f32 / u32::MAX as f32 * 2.0 - 1.0
                };

                // Tidal pull: walk energy follows attractor chaos level
                // Quiet stable regions → contemplative slow wandering
                // Chaotic regions → restless faster exploration
                let chaos_now = { shared.lock().chaos_level };
                let tidal_scale = 0.25 + chaos_now * 1.5;

                // Time-of-day walk character: night = wider slower wander, day = tighter faster
                let night = 1.0 - time_of_day;
                let tod_rate_mult = 0.6 + time_of_day * 0.8; // 0.6x at midnight, 1.4x at noon

                let silence_mult = if silence_expanded { 1.8 } else { 1.0 };

                // EMPATHY WITH TEMPO: interaction rate adjusts Evolve walk speed
                let tempo_walk_mult = if interaction_ticks_since_change < 240 {
                    1.4 // user exploring
                } else if interaction_ticks_since_change > 7200 {
                    0.4 // user sculpting
                } else {
                    let t = (interaction_ticks_since_change as f32 - 240.0) / (7200.0 - 240.0);
                    1.4 - t * 1.0
                };

                // COOLDOWN: elevated wander after intense sessions
                let cooldown_mult = 1.0 + activity_energy * 0.5;

                // WOUND HEALING: conservative step size
                let wound_step_mult = 0.5 + wound_t * 0.5;

                // CIRCADIAN SLEEP: boost step size 1.3x in 3am-5am window
                let sleep_step_mult = if circadian_sleep_active { 1.3 } else { 1.0 };

                let step = macro_walk_rate * dt * tidal_scale * tod_rate_mult * silence_mult
                    * entropy_walk_scale * typing_walk_scale * tempo_walk_mult
                    * cooldown_mult * wound_step_mult * sleep_step_mult;

                // Gravitational memory: find gradient in histogram, apply tiny nudge toward
                // frequently-visited regions — the instrument learns your taste over sessions
                let (gravity_nudge_chaos, gravity_nudge_space) = {
                    let st = shared.lock();
                    let ci = ((st.macro_chaos * 19.0) as usize).min(19);
                    let si = ((st.macro_space * 19.0) as usize).min(19);
                    let here = gravity_map[ci * 20 + si];
                    let right  = if ci < 19 { gravity_map[(ci+1)*20+si] } else { here };
                    let left   = if ci > 0  { gravity_map[(ci-1)*20+si] } else { here };
                    let up     = if si < 19 { gravity_map[ci*20+(si+1)] } else { here };
                    let down   = if si > 0  { gravity_map[ci*20+(si-1)] } else { here };
                    // Gradient points toward higher-density regions (home territory)
                    let g_chaos = (right - left) * 0.003; // very subtle pull
                    let g_space = (up - down) * 0.003;
                    (g_chaos, g_space)
                };

                let mut st = shared.lock();

                // Night walk: wider excursions (tod multiplier on noise amplitude)
                let night_width = 1.0 + night * 0.6;
                st.macro_chaos  = (st.macro_chaos  + r(&mut walk_seed) * step * night_width + gravity_nudge_chaos).clamp(0.05, 0.95);
                st.macro_space  = (st.macro_space  + r(&mut walk_seed) * step * night_width + gravity_nudge_space).clamp(0.05, 0.95);
                st.macro_rhythm = (st.macro_rhythm + r(&mut walk_seed) * step).clamp(0.05, 0.95);
                st.macro_warmth = (st.macro_warmth + r(&mut walk_seed) * step).clamp(0.05, 0.95);

                // Update gravitational memory: accumulate visit at current position
                let ci = ((st.macro_chaos * 19.0) as usize).min(19);
                let si = ((st.macro_space * 19.0) as usize).min(19);
                gravity_map[ci * 20 + si] += 0.01;
                // Clamp to prevent runaway; normalize occasionally via save
                gravity_map[ci * 20 + si] = gravity_map[ci * 20 + si].min(1000.0);

                // Idle excursion: track idle time, occasionally do a parameter bloom
                // (only count as idle if walk is running — the instrument exploring on its own)
                drop(st);
                idle_ticks += 1;

                if !excursion_active && idle_ticks >= next_excursion_tick {
                    // Start a bloom: push into unstable high-chaos region
                    let st = shared.lock();
                    excursion_return_chaos = st.macro_chaos;
                    excursion_return_speed = config.system.speed;
                    drop(st);
                    excursion_active = true;
                    excursion_ticks = 0;
                    let r01 = lcg(&mut walk_seed_ex);
                    let target_chaos = (excursion_return_chaos + 0.3 + r01 * 0.25).clamp(0.5, 0.95);
                    let mut st = shared.lock();
                    st.macro_chaos = target_chaos;
                    drop(st);
                    let r01b = lcg(&mut walk_seed_ex);
                    next_excursion_tick = idle_ticks + (10*60*120) + (r01b * 10.0*60.0*120.0) as u64;
                } else if excursion_active {
                    excursion_ticks += 1;
                    let bloom_dur = 10 * 120u64;   // 10 seconds of bloom
                    let return_dur = 15 * 120u64;  // 15 seconds drifting back
                    if excursion_ticks >= bloom_dur + return_dur {
                        // Bloom finished; let normal walk take over
                        excursion_active = false;
                    } else if excursion_ticks > bloom_dur {
                        // Drift back toward original position
                        let t = (excursion_ticks - bloom_dur) as f32 / return_dur as f32;
                        let ease = t * t * (3.0 - 2.0 * t); // smoothstep
                        let mut st = shared.lock();
                        st.macro_chaos = st.macro_chaos + (excursion_return_chaos - st.macro_chaos) * ease * 0.04;
                    }
                }
                // NESTING: long-period oscillations after 2+ hours uptime
                let nesting_threshold: u64 = 2 * 3600 * 120;
                if uptime_ticks > nesting_threshold {
                    nesting_phase += 1.0 / (12.5 * 60.0 * 120.0);
                    let nesting_osc = (nesting_phase * std::f64::consts::TAU).sin() as f32 * 0.08;
                    let mut st = shared.lock();
                    st.macro_chaos = (st.macro_chaos + nesting_osc * step).clamp(0.05, 0.95);
                    drop(st);
                }

                // APPETITE: hunger toward spectral complexity after 5+ min idle
                if silence_expanded {
                    let hunger = 0.0002f32;
                    let mut st = shared.lock();
                    st.macro_chaos = st.macro_chaos + (0.6 - st.macro_chaos) * hunger;
                    st.macro_space = st.macro_space + (0.55 - st.macro_space) * hunger;
                    drop(st);
                }

            } else {
                idle_ticks = 0; // reset idle counter when walk is off
            }

            // PAIR BONDING: enrich params for frequently-visited presets
            let affinity_count = preset_affinity.get(&sys_hash).copied().unwrap_or(0);
            if affinity_count > 20 {
                let bond_strength = (affinity_count as f32 / 100.0).min(0.3);
                config.audio.reverb_wet = (config.audio.reverb_wet + bond_strength * 0.08).min(0.82);
            }

            // Chaos → speed, sigma, rho
            config.system.speed = 0.5 + macro_chaos as f64 * 9.5;
            config.lorenz.sigma = 5.0 + macro_chaos as f64 * 20.0;
            config.lorenz.rho   = 20.0 + macro_chaos as f64 * 25.0;

            // Space → reverb_wet, chorus_mix, portamento_ms, delay_feedback
            // (these will be overwritten into params below after mapping)
            config.sonification.portamento_ms = 10.0 + macro_space * 790.0;
        }

        // ── Silence awareness: 5-minute idle → expanded walk range ────────────────
        // Already computed above as silence_expanded.

        // ── Attractor aging: filter warmth increases over first hour ──────────────
        aging_secs += 1.0 / control_rate_hz as f32;
        {
            let mut st = shared.lock();
            st.aging_secs = aging_secs;
        }
        // aging_t: 0.0 at launch, 1.0 after 60 minutes
        let aging_t = (aging_secs / 3600.0).min(1.0);

        // ── Attractor dreams: 30+ min idle → brief visit to another system ─────────
        {
            let walk_on = { shared.lock().macro_walk_enabled };
            if walk_on {
                // Track dream-specific idle (reset on any interaction, separate from excursion idle)
                if silence_secs < 30.0 {
                    dream_idle_ticks = 0; // user was recently active
                } else {
                    dream_idle_ticks += 1;
                }

                if !dream_active && dream_idle_ticks >= dream_idle_threshold {
                    // Start a dream: pick a random different system
                    let cur_sys = config.system.name.clone();
                    let dream_systems = ["lorenz", "rossler", "halvorsen", "aizawa", "chua",
                                          "van_der_pol", "duffing", "geodesic_torus"];
                    let idx = (walk_seed_ex.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 33)
                        as usize % dream_systems.len();
                    walk_seed_ex = walk_seed_ex.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let dream_sys = dream_systems[idx];
                    if dream_sys != cur_sys.as_str() {
                        let mut dcfg = config.clone();
                        dcfg.system.name = dream_sys.to_string();
                        dream_target_config = Some(dcfg);
                        dream_return_config = Some(config.clone());
                        dream_active = true;
                        dream_ticks = 0;
                        let duration_r = lcg(&mut walk_seed_ex);
                        dream_total_ticks = (60 * 120) + (duration_r * 30.0 * 120.0) as u64; // 60-90s
                        dream_idle_ticks = 0; // reset so we don't immediately re-dream
                    }
                }

                if dream_active {
                    dream_ticks += 1;
                    let fade_ticks = 15 * 120u64; // 15s morph in/out
                    let target_cfg = dream_target_config.as_ref().unwrap_or(&config);
                    let return_cfg = dream_return_config.as_ref().unwrap_or(&config);

                    let dream_config = if dream_ticks < fade_ticks {
                        // Morphing in
                        let t = dream_ticks as f32 / fade_ticks as f32;
                        crate::arrangement::lerp_config(return_cfg, target_cfg, t)
                    } else if dream_ticks < dream_total_ticks {
                        // Holding dream state
                        target_cfg.clone()
                    } else if dream_ticks < dream_total_ticks + fade_ticks {
                        // Morphing back
                        let t = (dream_ticks - dream_total_ticks) as f32 / fade_ticks as f32;
                        crate::arrangement::lerp_config(target_cfg, return_cfg, t)
                    } else {
                        // Dream over
                        dream_active = false;
                        dream_idle_ticks = 0;
                        return_cfg.clone()
                    };

                    if dream_active {
                        // Apply dream config on top of current config
                        config.system.speed   = dream_config.system.speed;
                        config.lorenz.sigma   = dream_config.lorenz.sigma;
                        config.lorenz.rho     = dream_config.lorenz.rho;
                        config.audio.reverb_wet = dream_config.audio.reverb_wet;
                    }
                }
            }
        }

        // ── Typing resonance: keyboard cadence → Evolve wander rate ───────────────
        // Already computed as typing_kps and typing_walk_scale above.

        // ── Instance empathy: receive chaos nudge from peer instance ───────────────
        let empathy_nudge: f32 = {
            let raw = empathy_rx.load(Ordering::Relaxed);
            if raw == u32::MAX { 0.0 } else {
                // Convert fixed-point chaos (0-10000 → 0.0-1.0) to a tiny nudge
                (raw as f32 / 10000.0 - 0.5) * 0.004 // ±0.002 max nudge per tick
            }
        };
        // Send our chaos level to peer
        if let Some(ref sock) = empathy_sender {
            let chaos = { shared.lock().chaos_level };
            let fixed = (chaos * 10000.0) as u32;
            let _ = sock.send_to(&fixed.to_le_bytes(), "127.0.0.1:47832");
        }
        // Apply empathy nudge to macro_chaos (extremely subtle, only when walk is on)
        let macro_walk_enabled_emp = { shared.lock().macro_walk_enabled };
        if macro_walk_enabled_emp && empathy_nudge.abs() > 0.0 {
            let mut st = shared.lock();
            st.macro_chaos = (st.macro_chaos + empathy_nudge).clamp(0.05, 0.95);
        }

        // Coupled attractor: rebuild if source changed or enabled state changed
        if coupled_enabled {
            if coupled_system.is_none() || coupled_system_name != coupled_src_name {
                coupled_system_name = coupled_src_name.clone();
                let mut coupled_cfg = config.clone();
                coupled_cfg.system.name = coupled_src_name.clone();
                coupled_system = Some(build_system(&coupled_cfg));
                coupled_min = -30.0;
                coupled_max = 30.0;
            }
        } else {
            if coupled_system.is_some() {
                coupled_system = None;
            }
        }

        // Step coupled system and compute normalized output
        let coupled_norm = if let Some(ref mut cs) = coupled_system {
            let coupled_steps = ((config.system.speed / control_rate_hz) / config.system.dt)
                .round() as usize;
            for _ in 0..coupled_steps.clamp(1, 10_000) {
                cs.step(config.system.dt);
            }
            let cx = cs.state().first().copied().unwrap_or(0.0);
            if cx < coupled_min { coupled_min = cx; }
            if cx > coupled_max { coupled_max = cx; }
            let range = (coupled_max - coupled_min).abs().max(1e-9);
            (cx - coupled_min) / range
        } else {
            0.5
        };

        // Apply coupling to main system's config parameters
        if coupled_enabled && coupled_system.is_some() {
            let delta = (coupled_norm - 0.5) * coupled_strength as f64 * 2.0;
            match coupled_target_param.as_str() {
                "rho"      => config.lorenz.rho      = (config.lorenz.rho      + delta * 10.0).clamp(1.0, 60.0),
                "sigma"    => config.lorenz.sigma    = (config.lorenz.sigma    + delta *  5.0).clamp(0.1, 30.0),
                "speed"    => config.system.speed    = (config.system.speed    + delta *  2.0).clamp(0.05, 20.0),
                "a"        => config.rossler.a       = (config.rossler.a       + delta *  0.1).clamp(0.001, 1.0),
                "c"        => config.rossler.c       = (config.rossler.c       + delta *  1.0).clamp(0.1, 15.0),
                "coupling" => config.kuramoto.coupling = (config.kuramoto.coupling + delta * 0.5).clamp(0.0, 5.0),
                _ => {}
            }
        }

        // Bidirectional: main system feeds back to coupled
        if coupled_enabled && coupled_bidirectional {
            if let Some(ref mut cs) = coupled_system {
                let main_x = system.state().first().copied().unwrap_or(0.0);
                let main_norm = ((main_x + 30.0) / 60.0).clamp(0.0, 1.0);
                let reciprocal_delta = (main_norm - 0.5) * coupled_strength as f64 * 1.0;
                // Apply to coupled system speed via a transient nudge (not persisted)
                // We do this by stepping the coupled system with a modified dt
                let nudge_steps = ((config.system.speed * (1.0 + reciprocal_delta) / control_rate_hz) / config.system.dt)
                    .round() as usize;
                // We already stepped it above; just update the coupled_strength feedback display
                let _ = nudge_steps;
                let _ = cs;
            }
        }

        // Drive-response synchronization error (exponential moving average)
        if coupled_enabled {
            let main_x_norm = {
                let s = system.state();
                let x = s.first().copied().unwrap_or(0.0) as f32;
                ((x + 30.0) / 60.0).clamp(0.0, 1.0)
            };
            let inst_err = (main_x_norm - coupled_norm as f32).abs();
            let prev_err = shared.lock().sync_error;
            shared.lock().sync_error = prev_err * 0.95 + inst_err * 0.05;
        }

        // Update live display of coupled outputs
        {
            let mut st = shared.lock();
            let main_x = system.state().first().copied().unwrap_or(0.0) as f32;
            st.coupled_x_out = ((main_x + 30.0) / 60.0).clamp(0.0, 1.0);
            st.coupled_src_x_out = coupled_norm as f32;
        }

        // Integrate enough steps to cover one control period at the configured speed
        let steps = ((config.system.speed / control_rate_hz) / config.system.dt)
            .round() as usize;
        for _ in 0..steps.clamp(1, 10_000) {
            system.step(config.system.dt);
        }

        // Energy conservation tracking
        if let Some(ee) = system.energy_error() {
            shared.lock().energy_error = ee;
        }

        // SCARRING: detect near-divergence and record scar position
        {
            let st = system.state();
            let near_diverge = system.speed() > 800.0
                || st.iter().any(|&v| v.abs() > 300.0);
            if near_diverge && scars.len() < 500 {
                let sx = st.first().copied().unwrap_or(0.0) as f32;
                let sy = if st.len() >= 2 { st[1] as f32 } else { 0.0 };
                scars.push((sx, sy));
                shared.lock().scars = scars.clone();
            }
        }

        // Update visualization
        let mut is_poincare_crossing = false;
        {
            let state = system.state();
            if state.len() >= 2 {
                let mut vh = viz.lock();
                let max_trail = config.viz.trail_length;
                if vh.len() >= max_trail {
                    let excess = vh.len().saturating_sub(max_trail - 1);
                    vh.drain(0..excess);
                }
                let speed_norm = (system.speed() as f32 / 100.0).clamp(0.0, 1.0);
                let z = if state.len() >= 3 { state[2] as f32 } else { 0.0 };
                let sx = state[0] as f32;
                let sy = state[1] as f32;
                // NaN/inf guard: skip viz push if state diverged
                if sx.is_finite() && sy.is_finite() {
                    let prev_z = vh.last().map(|p| p.2).unwrap_or(0.0);
                    let mean_z_approx = 25.0f32;
                    let is_crossing = prev_z < mean_z_approx && z >= mean_z_approx;
                    is_poincare_crossing = is_crossing;
                    vh.push((sx, sy, z, speed_norm, is_crossing));
                }
            }
        }

        // Map state to audio params and send (non-blocking, drop if full)
        let mut params = mapper.map(system.state(), system.speed(), &config.sonification);

        // Fill harmony fields from config
        params.transpose_semitones = config.sonification.transpose_semitones;
        params.chord_intervals = chord_intervals_for(&config.sonification.chord_mode);
        params.voice_levels = config.sonification.voice_levels;
        params.portamento_ms = config.sonification.portamento_ms;

        // Fill audio effect fields from config
        params.master_volume = config.audio.master_volume;
        params.reverb_wet = config.audio.reverb_wet;
        params.delay_ms = config.audio.delay_ms;
        params.delay_feedback = config.audio.delay_feedback;
        params.bit_depth = config.audio.bit_depth;
        params.rate_crush = config.audio.rate_crush;
        params.chorus_mix = config.audio.chorus_mix;
        params.chorus_rate = config.audio.chorus_rate;
        params.chorus_depth = config.audio.chorus_depth;
        params.waveshaper_drive = config.audio.waveshaper_drive;
        params.waveshaper_mix = config.audio.waveshaper_mix;

        // EQ fields from AppState
        {
            let st = shared.lock();
            params.eq_low_db   = st.eq_low_db;
            params.eq_mid_db   = st.eq_mid_db;
            params.eq_high_db  = st.eq_high_db;
            params.eq_mid_freq = st.eq_mid_freq;
        }

        // Voice shapes from config
        params.voice_shapes = [
            osc_shape_from_str(&config.sonification.voice_shapes[0]),
            osc_shape_from_str(&config.sonification.voice_shapes[1]),
            osc_shape_from_str(&config.sonification.voice_shapes[2]),
            osc_shape_from_str(&config.sonification.voice_shapes[3]),
        ];

        // Karplus-Strong trigger on Poincaré crossings
        {
            let st = shared.lock();
            let ks_enabled = st.ks_enabled;
            let ks_vol = st.ks_volume;
            params.ks_volume = ks_vol;
            if ks_enabled && is_poincare_crossing {
                params.ks_trigger = true;
                params.ks_freq = params.freqs[0].max(50.0);
            }
        }

        // Waveguide params: map attractor state to waveguide physical model
        if config.sonification.mode == "waveguide" {
            let state = system.state();
            // Normalize x to 0..1 for tension
            let x_norm = if !state.is_empty() {
                ((state[0] + 30.0) / 60.0).clamp(0.0, 1.0) as f32
            } else { 0.5 };
            params.waveguide_tension = x_norm;
            params.waveguide_damping = (params.chaos_level * 0.3 + 0.5).clamp(0.5, 0.99);
            params.waveguide_excite = is_poincare_crossing;
            params.mode = SonifMode::Waveguide;
        }

        // Spectral freeze: copy frozen state into params
        {
            let st = shared.lock();
            params.spectral_freeze_active = st.spectral_freeze_active;
            if st.spectral_freeze_active {
                for i in 0..16 {
                    params.spectral_freeze_freqs[i] = st.spectral_freeze_freqs.get(i).copied().unwrap_or(0.0);
                    params.spectral_freeze_amps[i]  = st.spectral_freeze_amps.get(i).copied().unwrap_or(0.0);
                }
            }
        }

        // MIDI input: apply mappings to config params
        {
            let st = shared.lock();
            if st.midi_in_enabled {
                let note_norm = st.midi_in_last_note as f32 / 127.0;
                let vel_norm  = st.midi_in_last_vel  as f32 / 127.0;
                let cc_norm   = st.midi_in_last_cc   as f32 / 127.0;
                drop(st);
                let apply = |target: &str, val: f64| {
                    // We clone config here so we need to apply after drop
                    (target.to_string(), val)
                };
                let (note_target, vel_target, cc_target) = {
                    let st2 = shared.lock();
                    (st2.midi_in_note_target.clone(), st2.midi_in_vel_target.clone(), st2.midi_in_cc_target.clone())
                };
                let apply_param = |target: &str, norm: f32, cfg: &mut Config| {
                    match target {
                        "rho"       => cfg.lorenz.rho       = 10.0 + norm as f64 * 50.0,
                        "sigma"     => cfg.lorenz.sigma     = 1.0  + norm as f64 * 25.0,
                        "speed"     => cfg.system.speed     = 0.1  + norm as f64 * 10.0,
                        "base_freq" => cfg.sonification.base_frequency = 55.0 + norm as f64 * 880.0,
                        "coupling"  => cfg.kuramoto.coupling = norm as f64 * 5.0,
                        _ => {}
                    }
                };
                apply_param(&note_target, note_norm, &mut config);
                apply_param(&vel_target,  vel_norm,  &mut config);
                apply_param(&cc_target,   cc_norm,   &mut config);
            }
        }

        // Replay recording snapshot (every ~500ms = 60 ticks at 120Hz)
        {
            let mut st = shared.lock();
            if st.replay_recording {
                let elapsed_ms = st.replay_start_time.elapsed().as_millis() as u32;
                // Only snapshot when crossing approximate 500ms boundary
                if elapsed_ms / 500 > (if st.replay_events.is_empty() { 0 } else {
                    st.replay_events.last().map(|e| e.timestamp_ms / 500).unwrap_or(0)
                }) {
                    let evts = [
                        (0u8, config.system.speed as f32),
                        (1u8, config.lorenz.rho as f32),
                        (2u8, config.lorenz.sigma as f32),
                        (3u8, config.lorenz.beta as f32),
                        (4u8, config.audio.reverb_wet),
                        (5u8, config.audio.master_volume),
                        (6u8, config.audio.chorus_mix),
                        (7u8, config.audio.delay_ms),
                        (8u8, config.kuramoto.coupling as f32),
                        (9u8, config.sonification.base_frequency as f32),
                    ];
                    for (pid, val) in evts {
                        st.replay_events.push(crate::ui::ReplayEvent { timestamp_ms: elapsed_ms, param_id: pid, value: val });
                    }
                }
            }

            // Replay playback
            if st.replay_playing && !st.replay_events.is_empty() {
                let elapsed_ms = st.replay_play_start.elapsed().as_millis() as u32;
                let pos = st.replay_play_pos;
                let events = st.replay_events.clone();
                let mut new_pos = pos;
                for (i, ev) in events.iter().enumerate() {
                    if i < pos { continue; }
                    if ev.timestamp_ms <= elapsed_ms {
                        match ev.param_id {
                            0 => { st.config.system.speed = ev.value as f64; }
                            1 => { st.config.lorenz.rho   = ev.value as f64; }
                            2 => { st.config.lorenz.sigma  = ev.value as f64; }
                            3 => { st.config.lorenz.beta   = ev.value as f64; }
                            4 => { st.config.audio.reverb_wet    = ev.value; }
                            5 => { st.config.audio.master_volume = ev.value; }
                            6 => { st.config.audio.chorus_mix    = ev.value; }
                            7 => { st.config.audio.delay_ms      = ev.value; }
                            8 => { st.config.kuramoto.coupling    = ev.value as f64; }
                            9 => { st.config.sonification.base_frequency = ev.value as f64; }
                            _ => {}
                        }
                        new_pos = i + 1;
                    } else {
                        break;
                    }
                }
                if new_pos >= events.len() {
                    st.replay_playing = false;
                    st.replay_play_pos = 0;
                } else {
                    st.replay_play_pos = new_pos;
                }
            }
        }

        // Arpeggiator
        {
            let (arp_enabled, arp_steps, arp_bpm, arp_octaves) = {
                let st = shared.lock();
                (st.arp_enabled, st.arp_steps, st.arp_bpm, st.arp_octaves)
            };

            if arp_enabled {
                let step_rate_hz = arp_bpm as f64 / 60.0 * 4.0; // 16th notes
                let (old_phase, new_phase, arp_pos) = {
                    let mut st = shared.lock();
                    let old = st.arp_phase;
                    st.arp_phase += step_rate_hz / control_rate_hz;
                    if st.arp_phase >= 1.0 {
                        st.arp_phase -= 1.0;
                        let new_pos = (st.arp_position + 1) % arp_steps;
                        st.arp_position = new_pos;
                    }
                    (old, st.arp_phase, st.arp_position)
                };

                let scale = crate::sonification::Scale::from(config.sonification.scale.as_str());
                let base = config.sonification.base_frequency as f32;
                let oct = arp_octaves as f32;

                let t_step = arp_pos as f32 / arp_steps as f32;
                let state = system.state();
                let modulation = if !state.is_empty() { (state[0] as f32).tanh() * 0.05 } else { 0.0 };
                let t = (t_step + modulation).clamp(0.0, 1.0);

                let arp_freq = crate::sonification::quantize_to_scale(t, base, oct, scale);

                let step_triggered = new_phase < old_phase || (old_phase == 0.0 && new_phase > 0.0);
                if step_triggered {
                    params.ks_trigger = true;
                    params.ks_freq = arp_freq;
                    params.freqs[0] = arp_freq;
                    params.amps[0] = params.amps[0].max(0.5);
                }
            }
        }

        // Compute audio-reactive trail color from current params
        let trail_color = {
            use egui::Color32;
            let freq = params.freqs[0].max(32.0);
            let freq_norm = ((freq.log2() - 5.0) / 5.0).clamp(0.0, 1.0);
            let r = ((1.0 - freq_norm) * 255.0) as u8;
            let b = (freq_norm * 255.0) as u8;
            let g = (params.chaos_level * 200.0) as u8;
            Color32::from_rgb(r, g, b)
        };

        // Update shared state from simulation
        {
            let mut st = shared.lock();
            st.chaos_level = params.chaos_level;
            st.trail_color = trail_color;
            st.current_state = system.state().to_vec();
            st.current_deriv = system.current_deriv();
            if config.system.name == "kuramoto" {
                st.kuramoto_phases = system.state().to_vec();
                st.order_param = {
                    let phases = &st.current_state;
                    let n = phases.len() as f64;
                    if n > 0.0 {
                        let sin_sum: f64 = phases.iter().map(|&th| th.sin()).sum();
                        let cos_sum: f64 = phases.iter().map(|&th| th.cos()).sum();
                        (sin_sum.powi(2) + cos_sum.powi(2)).sqrt() / n
                    } else {
                        0.0
                    }
                };
            }
        }

        // Apply sidechain input modulation
        if sidechain_enabled {
            let sc_rms = if let Some(lvl) = sidechain_level_shared.try_lock() { *lvl } else { 0.0 };
            let sc_delta = sc_rms * sidechain_amount;
            match sidechain_target.as_str() {
                "speed"  => config.system.speed  = (config.system.speed  * (1.0 + sc_delta as f64)).clamp(0.05, 20.0),
                "reverb" => params.reverb_wet     = (params.reverb_wet    + sc_delta).clamp(0.0, 1.0),
                "filter" => params.filter_cutoff  = (params.filter_cutoff * (1.0 + sc_delta * 4.0)).clamp(20.0, 20000.0),
                "sigma"  => config.lorenz.sigma   = (config.lorenz.sigma  * (1.0 + sc_delta as f64)).clamp(0.1, 50.0),
                "volume" => params.master_volume  = (params.master_volume + sc_delta).clamp(0.0, 1.0),
                _ => {}
            }
        }

        // Reinit extra layers if their preset changed
        for i in 0..2 {
            if i < poly_defs.len() {
                let def = &poly_defs[i];
                if def.changed || extra_layers[i].is_none() {
                    if def.active && !def.preset_name.is_empty() {
                        let cfg = crate::patches::load_preset(&def.preset_name);
                        let sys = build_system(&cfg);
                        let mpr = build_mapper(&cfg.sonification.mode);
                        extra_layers[i] = Some(ExtraLayer { system: sys, mapper: mpr, config: cfg });
                    } else {
                        extra_layers[i] = None;
                    }
                }
            }
        }
        // Clear changed flags
        {
            let mut st = shared.lock();
            for d in &mut st.poly_layers { d.changed = false; }
        }

        // Tick extra layers and build their AudioParams
        let mut layer1_params: Option<AudioParams> = None;
        let mut layer2_params: Option<AudioParams> = None;
        for (li, el_opt) in extra_layers.iter_mut().enumerate() {
            if let Some(el) = el_opt {
                let steps = ((el.config.system.speed / control_rate_hz) / el.config.system.dt)
                    .round() as usize;
                for _ in 0..steps.clamp(1, 10_000) { el.system.step(el.config.system.dt); }
                let mut lp = el.mapper.map(el.system.state(), el.system.speed(), &el.config.sonification);
                lp.transpose_semitones = el.config.sonification.transpose_semitones;
                lp.chord_intervals     = chord_intervals_for(&el.config.sonification.chord_mode);
                lp.voice_levels        = el.config.sonification.voice_levels;
                lp.portamento_ms       = el.config.sonification.portamento_ms;
                lp.master_volume       = el.config.audio.master_volume;
                lp.reverb_wet          = el.config.audio.reverb_wet;
                lp.delay_ms            = el.config.audio.delay_ms;
                lp.delay_feedback      = el.config.audio.delay_feedback;
                lp.bit_depth           = el.config.audio.bit_depth;
                lp.rate_crush          = el.config.audio.rate_crush;
                lp.chorus_mix          = el.config.audio.chorus_mix;
                lp.waveshaper_drive    = el.config.audio.waveshaper_drive;
                lp.waveshaper_mix      = el.config.audio.waveshaper_mix;
                lp.voice_shapes        = [
                    osc_shape_from_str(&el.config.sonification.voice_shapes[0]),
                    osc_shape_from_str(&el.config.sonification.voice_shapes[1]),
                    osc_shape_from_str(&el.config.sonification.voice_shapes[2]),
                    osc_shape_from_str(&el.config.sonification.voice_shapes[3]),
                ];
                // Per-layer ADSR and mix from poly_defs
                if li < poly_defs.len() {
                    let def = &poly_defs[li];
                    lp.layer_level       = if def.mute { 0.0 } else { def.level };
                    lp.layer_pan         = def.pan;
                    lp.adsr_attack_ms    = def.adsr_attack_ms;
                    lp.adsr_decay_ms     = def.adsr_decay_ms;
                    lp.adsr_sustain      = def.adsr_sustain;
                    lp.adsr_release_ms   = def.adsr_release_ms;
                }
                lp.layer_id = li + 1;
                if li == 0 { layer1_params = Some(lp); }
                else       { layer2_params = Some(lp); }
            }
        }

        // Update layer 0 ADSR from AppState (skip ADSR in simple mode — macros drive it)
        {
            let st = shared.lock();
            if !st.simple_mode {
                params.adsr_attack_ms  = st.adsr_attack_ms;
                params.adsr_decay_ms   = st.adsr_decay_ms;
                params.adsr_sustain    = st.adsr_sustain;
                params.adsr_release_ms = st.adsr_release_ms;
            }
            params.layer_level = if st.layer0_mute { 0.0 } else { st.layer0_level };
            params.layer_pan   = st.layer0_pan;
        }
        params.layer_id = 0;

        // Re-apply macro audio params LAST so they're never overwritten
        if simple_mode {
            let (macro_space, macro_rhythm, macro_warmth) = {
                let st = shared.lock();
                (st.macro_space, st.macro_rhythm, st.macro_warmth)
            };
            params.reverb_wet       = macro_space * 0.9;
            params.chorus_mix       = macro_space * 0.7;
            params.delay_feedback   = macro_space * 0.7;
            params.adsr_attack_ms   = 200.0 - macro_rhythm * 195.0;
            // Warmth slider: 1200 Hz (very warm) to 8000 Hz (bright) — never below 1200
            params.filter_cutoff    = 1200.0 + (1.0 - macro_warmth) * 6800.0;
            params.waveshaper_drive = 1.0 + macro_warmth * 4.0;
            // Volume slider always wins — arrangement scene volumes are ignored in simple mode
            params.master_volume    = { shared.lock().config.audio.master_volume };
        }

        // ── Attractor aging: filter opens fractionally over first hour ────────────
        // aging_t = 0.0 at start, 1.0 after 60 minutes
        // Narrow range (0.92-1.0) so it never silences even muffled sources.
        let aging_filter_mult = 0.92 + aging_t * 0.08;
        params.filter_cutoff *= aging_filter_mult;
        // Also slight harmonic richness increase: waveshaper drive decreases with age (cleaner)
        params.waveshaper_drive = (params.waveshaper_drive * (1.0 - aging_t * 0.15)).max(1.0);

        // ── Harmonic gravity: voices drift toward pure intervals ──────────────────
        // Not hard quantization. A gentle magnetic pull toward consonance.
        // Dissonance is available but you have to push for it.
        {
            let pull = 0.0008f32; // per-tick pull strength (subtle, not jarring)
            let harmonic_targets = [0.5f32, 2.0/3.0, 3.0/4.0, 4.0/5.0, 5.0/6.0,
                                     1.0, 5.0/4.0, 4.0/3.0, 3.0/2.0, 5.0/3.0, 2.0, 3.0, 4.0];
            // Pull voice 1 toward nearest harmonic of voice 0
            if params.freqs[0] > 20.0 && params.freqs[1] > 20.0 {
                let ratio = (params.freqs[1] / params.freqs[0]).clamp(0.4, 5.0);
                if ratio.is_finite() {
                    if let Some(&nearest_r) = harmonic_targets.iter()
                        .min_by(|&&a, &&b| (a - ratio).abs().partial_cmp(&(b - ratio).abs())
                            .unwrap_or(std::cmp::Ordering::Equal)) {
                        let ideal = params.freqs[0] * nearest_r;
                        if ideal > 0.0 {
                            let cents_off = (params.freqs[1] / ideal).ln() / (2f32.ln() / 12.0);
                            if cents_off.is_finite() && cents_off.abs() < 15.0 {
                                params.freqs[1] += (ideal - params.freqs[1]) * pull;
                            }
                        }
                    }
                }
            }
            // Pull voice 2 toward harmonic of voice 0
            if params.freqs[0] > 20.0 && params.freqs[2] > 20.0 {
                let ratio = (params.freqs[2] / params.freqs[0]).clamp(0.4, 5.0);
                if ratio.is_finite() {
                    if let Some(&nearest_r) = harmonic_targets.iter()
                        .min_by(|&&a, &&b| (a - ratio).abs().partial_cmp(&(b - ratio).abs())
                            .unwrap_or(std::cmp::Ordering::Equal)) {
                        let ideal = params.freqs[0] * nearest_r;
                        if ideal > 0.0 {
                            let cents_off = (params.freqs[2] / ideal).ln() / (2f32.ln() / 12.0);
                            if cents_off.is_finite() && cents_off.abs() < 15.0 {
                                params.freqs[2] += (ideal - params.freqs[2]) * pull;
                            }
                        }
                    }
                }
            }
        }

        // ── Volume creep: draws listener closer over the course of an hour ─────────
        {
            let current_vol = config.audio.master_volume;
            if (current_vol - volume_creep_last_vol).abs() > 0.005 {
                // User touched the volume slider
                volume_creep = 1.0;
                volume_creep_last_vol = current_vol;
                shared.lock().volume_creep_factor = 1.0;
            } else {
                // Drift downward imperceptibly (floor at VOLUME_CREEP_MIN)
                volume_creep = (volume_creep * (1.0 - VOLUME_CREEP_RATE)).max(VOLUME_CREEP_MIN);
                shared.lock().volume_creep_factor = volume_creep;
            }
            params.master_volume *= volume_creep;
        }

        // ── Breathing: ~0.3 dB master volume oscillation at human respiratory rate ──
        // 4.5-second cycle, ±0.034 linear gain — subliminal organic warmth.
        // Every acoustic instrument has this. Synthesizers don't. This closes the gap.
        breathing_phase = (breathing_phase + BREATHING_RATE / control_rate_hz) % 1.0;
        let breathing_gain = (breathing_phase * std::f64::consts::TAU).sin() as f32 * 0.034 + 1.0;
        params.master_volume = (params.master_volume * breathing_gain).clamp(0.0, 1.2);

        // ── Seasonal drift: barely perceptible frequency shift over the calendar year ──
        // Warmer, lower in winter; brighter, higher in summer. ±1.5% range.
        // Someone who uses Math Sonify for six months will notice the feel changed.
        let sf = seasonal_freq_mult as f32;
        for f in &mut params.freqs { *f *= sf; }
        params.grain_base_freq *= sf;

        // CIRCADIAN AUDIO: odd/even harmonic bias — night boosts odd voices, day even
        {
            let night_factor = 1.0 - time_of_day;
            let circ_bias = (night_factor - 0.5) * 0.20; // -0.10 to +0.10
            params.amps[0] = (params.amps[0] * (1.0 + circ_bias)).clamp(0.0, 1.0);
            params.amps[1] = (params.amps[1] * (1.0 - circ_bias)).clamp(0.0, 1.0);
            params.amps[2] = (params.amps[2] * (1.0 + circ_bias)).clamp(0.0, 1.0);
            params.amps[3] = (params.amps[3] * (1.0 - circ_bias)).clamp(0.0, 1.0);
        }

        // WOUND HEALING: conservative audio params while recovering from crash
        if wound_t < 1.0 {
            let wound_mult = 0.5 + wound_t * 0.5;
            params.reverb_wet = (params.reverb_wet * wound_mult).clamp(0.0, 1.0);
            params.delay_feedback = (params.delay_feedback * wound_mult).clamp(0.0, 1.0);
            // Note: walk step multiplier was applied above in wound_step_mult
        }

        // STARTUP RAMP: tracked for shutdown fade — volume ramp removed (caused 2s silence on boot)

        // SHUTDOWN FADING: ramp master_volume and speed toward 0 over 3 seconds
        {
            let (shutdown_fading, sd_ramp_t) = {
                let st = shared.lock();
                (st.shutdown_fading, st.startup_ramp_t) // reuse startup_ramp_t for shutdown progress
            };
            if shutdown_fading {
                // Use elapsed time since shutdown triggered (stored in startup_ramp_t as a countdown)
                // Actually we'll derive progress from master_volume fade independently
                // Simple approach: read shutdown_fading flag and apply 3s ramp
                // We track this with a local counter: use startup_ramp_t field inversely
                // Simpler: use the shutdown_timer elapsed via shared state — but it's in the UI thread
                // Best approach: just decay master_volume quickly
                params.master_volume *= 0.992; // decay ~3s to silence at 120Hz
                if params.master_volume < 0.01 { params.master_volume = 0.0; }
            }
        }

        // ── Lyapunov spectrum computation (every ~5s) ─────────────────────────────
        lyap_timer += 1;
        if lyap_timer >= LYAP_INTERVAL {
            lyap_timer = 0;
            lyap_cycles += 1;
            let dim = system.dimension().min(3);
            if dim > 0 {
                // Compute all metrics outside the lock
                let state_snap = system.state().to_vec();
                let lyap = crate::systems::lyapunov_spectrum(
                    &state_snap, dim, dim, 300, config.system.dt, &|s| system.deriv_at(s),
                );
                let atype = crate::systems::classify_attractor(&lyap);
                let k_entropy = crate::systems::kolmogorov_entropy(&lyap);

                // Permutation entropy of x-component
                let perm_ent = if analysis_trajectory.len() >= 20 {
                    let ts: Vec<f64> = analysis_trajectory.iter()
                        .filter_map(|s| s.first().copied()).collect();
                    crate::systems::permutation_entropy(&ts, 4, 1)
                } else { 0.0 };

                // RK4 vs RK45 validation (every 6th lyap cycle = ~30s)
                let integrator_div = if lyap_cycles % 6 == 0 {
                    crate::systems::compare_integrators(
                        &state_snap, config.system.dt, 1000,
                        &|s| system.deriv_at(s),
                    )
                } else {
                    shared.lock().integrator_divergence
                };

                // Single lock write for all results
                {
                    let mut st = shared.lock();
                    st.lyapunov_spectrum = lyap;
                    st.attractor_type = atype.to_string();
                    st.kolmogorov_entropy = k_entropy;
                    st.permutation_entropy = perm_ent;
                    st.integrator_divergence = integrator_div;
                }
            }
        }

        // ── Session transcript (every 60s) ────────────────────────────────────────
        session_log_timer += 1;
        if session_log_timer >= SESSION_LOG_INTERVAL {
            session_log_timer = 0;
            let entry = {
                let st = shared.lock();
                crate::ui::SessionEntry {
                    elapsed_secs: uptime_ticks as f32 / 120.0,
                    system_name: st.config.system.name.clone(),
                    lyapunov_max: st.lyapunov_spectrum.first().copied().unwrap_or(0.0),
                    attractor_type: st.attractor_type.clone(),
                    kolmogorov_entropy: st.kolmogorov_entropy,
                    chaos_level: st.chaos_level,
                }
            };
            let mut st = shared.lock();
            st.session_log.push(entry);
            if st.session_log.len() > 1440 { // cap at 24 hours
                st.session_log.remove(0);
            }
        }

        // ── State + gravity map persistence (every ~2 minutes) ────────────────────
        state_save_timer += 1;
        if state_save_timer >= STATE_SAVE_INTERVAL {
            state_save_timer = 0;
            // Save attractor state
            let state_vec = system.state().to_vec();
            let state_bytes: Vec<u8> = state_vec.iter().flat_map(|f| f.to_le_bytes()).collect();
            let _ = std::fs::write("attractor_state.bin", &state_bytes);
            // Normalize and save gravity map
            let g_max = gravity_map.iter().cloned().fold(1.0f32, f32::max);
            let gravity_bytes: Vec<u8> = gravity_map.iter()
                .flat_map(|f| (f / g_max).to_le_bytes()).collect();
            let _ = std::fs::write("gravity_map.bin", &gravity_bytes);
            // Save aging seconds
            let _ = std::fs::write("aging.bin", &aging_secs.to_le_bytes());
            // Save entropy pool
            let ep = shared.lock().entropy_pool.max(entropy_pool);
            entropy_pool = ep;
            let _ = std::fs::write("entropy.bin", &entropy_pool.to_le_bytes());
            // SCARRING: save scars every 2 min
            let scar_bytes: Vec<u8> = scars.iter()
                .flat_map(|(x, y)| x.to_le_bytes().iter().chain(y.to_le_bytes().iter()).copied().collect::<Vec<u8>>())
                .collect();
            let _ = std::fs::write("scars.bin", &scar_bytes);
            // PAIR BONDING: save preset affinity
            let aff_bytes: Vec<u8> = preset_affinity.iter()
                .flat_map(|(k, v)| k.to_le_bytes().iter().chain(v.to_le_bytes().iter()).copied().collect::<Vec<u8>>())
                .collect();
            let _ = std::fs::write("preset_affinity.bin", &aff_bytes);
        }

        let batch: [Option<AudioParams>; 3] = [Some(params), layer1_params, layer2_params];
        let _ = tx.try_send(batch);
    }
}

fn osc_shape_from_str(s: &str) -> OscShape {
    match s {
        "triangle" => OscShape::Triangle,
        "saw" => OscShape::Saw,
        _ => OscShape::Sine,
    }
}

fn build_system(config: &Config) -> Box<dyn DynamicalSystem> {
    match config.system.name.as_str() {
        "rossler"         => Box::new(Rossler::new(config.rossler.a, config.rossler.b, config.rossler.c)),
        "double_pendulum" => Box::new(DoublePendulum::new(
            config.double_pendulum.m1, config.double_pendulum.m2,
            config.double_pendulum.l1, config.double_pendulum.l2,
        )),
        "geodesic_torus"  => Box::new(GeodesicTorus::new(config.geodesic_torus.big_r, config.geodesic_torus.r)),
        "kuramoto"        => Box::new(Kuramoto::new(config.kuramoto.n_oscillators, config.kuramoto.coupling)),
        "three_body"      => Box::new(ThreeBody::new([1.0, 1.0, 1.0])),
        "duffing"         => {
            let mut s = Duffing::new();
            s.delta = config.duffing.delta;
            s.alpha = config.duffing.alpha;
            s.beta  = config.duffing.beta;
            s.gamma = config.duffing.gamma;
            s.omega = config.duffing.omega;
            Box::new(s)
        }
        "van_der_pol"     => {
            let mut s = VanDerPol::new();
            s.mu = config.van_der_pol.mu;
            Box::new(s)
        }
        "halvorsen"       => {
            let mut s = Halvorsen::new();
            s.a = config.halvorsen.a;
            Box::new(s)
        }
        "aizawa"          => {
            let mut s = Aizawa::new();
            s.a = config.aizawa.a;
            s.b = config.aizawa.b;
            s.c = config.aizawa.c;
            s.d = config.aizawa.d;
            s.e = config.aizawa.e;
            s.f = config.aizawa.f;
            Box::new(s)
        }
        "chua"            => {
            let mut s = Chua::new();
            s.alpha = config.chua.alpha;
            s.beta  = config.chua.beta;
            s.m0    = config.chua.m0;
            s.m1    = config.chua.m1;
            Box::new(s)
        }
        "custom"          => {
            let (ex, ey, ez) = {
                // Read custom ODE expressions from... we pass them via config or use defaults
                // Since Config doesn't have custom ODE fields, we use placeholder defaults here;
                // AppState fields are read directly in the UI-triggered rebuild path
                (
                    "10.0 * (y - x)".to_string(),
                    "x * (28.0 - z) - y".to_string(),
                    "x * y - 2.667 * z".to_string(),
                )
            };
            Box::new(CustomOde::new(ex, ey, ez))
        }
        "fractional_lorenz" => Box::new(FractionalLorenz::new(
            1.0, config.lorenz.sigma, config.lorenz.rho, config.lorenz.beta
        )),
        "hindmarsh_rose"  => Box::new(HindmarshRose::new(
            config.hindmarsh_rose.current_i, config.hindmarsh_rose.r,
        )),
        "coupled_map_lattice" => Box::new(CoupledMapLattice::new(
            config.coupled_map_lattice.r, config.coupled_map_lattice.eps,
        )),
        "mackey_glass"    => {
            let mut s = MackeyGlass::new();
            s.beta  = config.mackey_glass.beta;
            s.gamma = config.mackey_glass.gamma;
            s.tau   = config.mackey_glass.tau;
            s.n     = config.mackey_glass.n;
            Box::new(s)
        }
        "nose_hoover"     => {
            let mut s = NoseHoover::new();
            s.a = config.nose_hoover.a;
            Box::new(s)
        }
        "sprott_b"        => Box::new(SprottB::new()),
        "henon_map"       => {
            let mut s = HenonMap::new();
            s.a = config.henon_map.a;
            s.b = config.henon_map.b;
            Box::new(s)
        }
        "lorenz96"        => {
            let mut s = Lorenz96::new();
            s.f = config.lorenz96.f;
            Box::new(s)
        }
        _                 => Box::new(Lorenz::new(config.lorenz.sigma, config.lorenz.rho, config.lorenz.beta)),
    }
}

fn build_mapper(mode: &str) -> Box<dyn Sonification> {
    match mode {
        "orbital"   => Box::new(OrbitalResonance::new()),
        "granular"  => Box::new(GranularMapping::new()),
        "spectral"  => Box::new(SpectralMapping::new()),
        "fm"        => Box::new(FmMapping::new()),
        "vocal"     => Box::new(VocalMapping::new()),
        "waveguide" => Box::new(DirectMapping::new()), // waveguide synthesis driven by direct mapping
        _           => Box::new(DirectMapping::new()),
    }
}

// ---------------------------------------------------------------------------
// MIDI output thread
// ---------------------------------------------------------------------------

fn start_midi_thread(shared: SharedState) {
    std::thread::spawn(move || {
        let midi_out = match midir::MidiOutput::new("Math Sonify") {
            Ok(m) => m,
            Err(e) => { log::warn!("MIDI init failed: {e}"); return; }
        };
        let ports = midi_out.ports();
        if ports.is_empty() {
            log::info!("No MIDI output ports found");
            return;
        }
        let port = &ports[0];
        let port_name = midi_out.port_name(port).unwrap_or_default();
        log::info!("MIDI output: {port_name}");
        let mut conn = match midi_out.connect(port, "math-sonify-out") {
            Ok(c) => c,
            Err(e) => { log::warn!("MIDI connect failed: {e}"); return; }
        };

        let mut last_notes = [255u8; 4];
        let mut last_chaos = 0.0f32;

        loop {
            std::thread::sleep(std::time::Duration::from_millis(20)); // 50 Hz

            let (freqs, amps, chaos, midi_enabled) = {
                let st = shared.lock();
                if !st.midi_enabled {
                    ([0.0f32; 4], [0.0f32; 4], 0.0f32, false)
                } else {
                    let f = [
                        st.config.sonification.base_frequency as f32,
                        st.config.sonification.base_frequency as f32 * 1.5,
                        st.config.sonification.base_frequency as f32 * 2.0,
                        st.config.sonification.base_frequency as f32 * 2.5,
                    ];
                    (f, st.config.sonification.voice_levels, st.chaos_level, true)
                }
            };

            if !midi_enabled { continue; }

            for (i, (&freq, &amp)) in freqs.iter().zip(amps.iter()).enumerate() {
                let channel = i as u8;
                let new_note = hz_to_midi(freq).clamp(0, 127);
                let velocity = (amp * 100.0).min(127.0) as u8;

                if last_notes[i] != new_note {
                    if last_notes[i] != 255 {
                        let _ = conn.send(&[0x80 | channel, last_notes[i], 0]);
                    }
                    if velocity > 0 {
                        let _ = conn.send(&[0x90 | channel, new_note, velocity]);
                        last_notes[i] = new_note;
                    } else {
                        last_notes[i] = 255;
                    }
                }
            }

            let new_chaos_val = (chaos * 127.0) as u8;
            if (chaos - last_chaos).abs() > 0.01 {
                let _ = conn.send(&[0xB0, 1, new_chaos_val]);
                last_chaos = chaos;
            }
        }
    });
}

fn hz_to_midi(hz: f32) -> u8 {
    if hz < 20.0 { return 255; }
    (69.0 + 12.0 * (hz / 440.0).log2()).round().clamp(0.0, 127.0) as u8
}

// ---------------------------------------------------------------------------
// MIDI input thread
// ---------------------------------------------------------------------------

fn start_midi_input_thread(shared: SharedState) {
    std::thread::spawn(move || {
        let midi_in = match midir::MidiInput::new("Math Sonify In") {
            Ok(m) => m,
            Err(e) => { log::warn!("MIDI input init failed: {e}"); return; }
        };
        let ports = midi_in.ports();
        if ports.is_empty() {
            log::info!("No MIDI input ports found");
            return;
        }
        let port = &ports[0];
        let port_name = midi_in.port_name(port).unwrap_or_default();
        log::info!("MIDI input: {port_name}");

        let shared_cb = shared.clone();
        let _conn = midi_in.connect(
            port,
            "math-sonify-in",
            move |_ts, msg, _| {
                if msg.len() < 2 { return; }
                let status = msg[0] & 0xF0;
                match status {
                    0x90 => { // Note On
                        let note = msg[1].min(127);
                        let vel  = if msg.len() > 2 { msg[2].min(127) } else { 0 };
                        let mut st = shared_cb.lock();
                        st.midi_in_last_note = note;
                        st.midi_in_last_vel  = vel;
                    }
                    0x80 => { // Note Off
                        let note = msg[1].min(127);
                        let mut st = shared_cb.lock();
                        st.midi_in_last_note = note;
                        st.midi_in_last_vel  = 0;
                    }
                    0xB0 => { // CC
                        if msg.len() >= 3 {
                            let cc_num = msg[1];
                            let cc_val = msg[2].min(127);
                            let cc_num_target = shared_cb.lock().midi_in_cc_num;
                            if cc_num == cc_num_target {
                                shared_cb.lock().midi_in_last_cc = cc_val;
                            }
                        }
                    }
                    _ => {}
                }
            },
            (),
        );

        // Keep thread alive (connection closes when dropped)
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    });
}
