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
use crate::systems::*;
use crate::sonification::{
    AudioParams, Sonification,
    DirectMapping, OrbitalResonance, GranularMapping, SpectralMapping, FmMapping,
    chord_intervals_for,
};
use crate::audio::{AudioEngine, WavRecorder, LoopExportPending};
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

    // Channel: sim thread -> audio thread
    let (tx, rx) = bounded::<AudioParams>(CHANNEL_CAP);

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
    )?;

    // Store actual sample rate in AppState
    shared.lock().sample_rate = actual_sr;

    // Simulation thread
    let shared_sim = shared.clone();
    let viz_sim = viz_history.clone();
    thread::spawn(move || {
        sim_thread(shared_sim, viz_sim, tx);
    });

    // MIDI output thread
    start_midi_thread(shared.clone());

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
            })
        }),
    ).map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

struct SonifyApp {
    shared: SharedState,
    viz_history: Arc<Mutex<Vec<(f32, f32, f32, f32, bool)>>>,
    waveform_buf: Arc<parking_lot::Mutex<Vec<f32>>>,
    recording: WavRecorder,
    loop_export: LoopExportPending,
    bifurc_data: Arc<Mutex<Vec<(f32, f32)>>>,
}

impl eframe::App for SonifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let points = self.viz_history.lock().clone();
        draw_ui(ctx, &self.shared, &points, &self.waveform_buf, &self.recording,
                &self.loop_export, &self.bifurc_data);
        ctx.request_repaint_after(Duration::from_millis(33)); // ~30 fps UI
    }
}

// ---------------------------------------------------------------------------
// Simulation thread
// ---------------------------------------------------------------------------

fn sim_thread(
    shared: SharedState,
    viz: Arc<Mutex<Vec<(f32, f32, f32, f32, bool)>>>,
    tx: crossbeam_channel::Sender<AudioParams>,
) {
    let initial_config = shared.lock().config.clone();
    let mut system = build_system(&initial_config);
    let mut mapper = build_mapper(&initial_config.sonification.mode);

    let control_rate_hz = 120.0f64;
    let control_period = Duration::from_secs_f64(1.0 / control_rate_hz);
    let mut next_tick = Instant::now();

    // Automation state
    let mut auto_snapshot_timer = 0u64;
    let auto_snapshot_interval = 12u64; // every ~100ms at 120Hz

    loop {
        let now = Instant::now();
        if now < next_tick {
            thread::sleep(next_tick - now);
        }
        next_tick += control_period;

        let (paused, mut config, sys_changed, mode_changed, bpm_sync, bpm, auto_recording, auto_playing) = {
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
            (p, c, sc, mc, bs, bm, ar, ap)
        };

        if paused { continue; }
        if sys_changed  { system = build_system(&config); }
        if mode_changed { mapper = build_mapper(&config.sonification.mode); }

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
                    if st.arr_loop {
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
                // Check if system or mode changed vs current config before overriding
                let (cur_sys, cur_mode) = {
                    let st = shared.lock();
                    (st.config.system.name.clone(), st.config.sonification.mode.clone())
                };
                if new_config.system.name != cur_sys {
                    shared.lock().system_changed = true;
                }
                if new_config.sonification.mode != cur_mode {
                    shared.lock().mode_changed = true;
                }
                config = new_config;
            }
        }

        // Integrate enough steps to cover one control period at the configured speed
        let steps = ((config.system.speed / control_rate_hz) / config.system.dt)
            .round() as usize;
        for _ in 0..steps.clamp(1, 10_000) {
            system.step(config.system.dt);
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
                let prev_z = vh.last().map(|p| p.2).unwrap_or(0.0);
                let mean_z_approx = 25.0f32;
                let is_crossing = prev_z < mean_z_approx && z >= mean_z_approx;
                is_poincare_crossing = is_crossing;
                vh.push((state[0] as f32, state[1] as f32, z, speed_norm, is_crossing));
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

        // Update shared state from simulation
        {
            let mut st = shared.lock();
            st.chaos_level = params.chaos_level;
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

        let _ = tx.try_send(params);
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
        _                 => Box::new(Lorenz::new(config.lorenz.sigma, config.lorenz.rho, config.lorenz.beta)),
    }
}

fn build_mapper(mode: &str) -> Box<dyn Sonification> {
    match mode {
        "orbital"  => Box::new(OrbitalResonance::new()),
        "granular" => Box::new(GranularMapping::new()),
        "spectral" => Box::new(SpectralMapping::new()),
        "fm"       => Box::new(FmMapping::new()),
        _          => Box::new(DirectMapping::new()),
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
