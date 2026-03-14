use egui::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::sonification::chord_intervals_for;
use crate::patches::{PRESETS, load_preset, save_patch, list_patches, load_patch_file};
use crate::audio::{WavRecorder, LoopExportPending};
use crate::systems::*;
use hound;

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
            selected_preset: "Lorenz Ambience".into(),
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
    match theme {
        "vaporwave" => {
            visuals.window_fill = Color32::from_rgb(20, 10, 30);
            visuals.panel_fill = Color32::from_rgb(20, 10, 30);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(35, 15, 50);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(50, 20, 70);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(80, 30, 100);
            visuals.widgets.active.bg_fill = Color32::from_rgb(200, 50, 150);
            visuals.selection.bg_fill = Color32::from_rgb(180, 40, 130);
            visuals.override_text_color = Some(Color32::from_rgb(255, 180, 230));
        }
        "crt" => {
            visuals.window_fill = Color32::from_rgb(0, 0, 0);
            visuals.panel_fill = Color32::from_rgb(0, 0, 0);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(0, 10, 0);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(0, 20, 0);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(0, 40, 0);
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 150, 0);
            visuals.selection.bg_fill = Color32::from_rgb(0, 120, 0);
            visuals.override_text_color = Some(Color32::from_rgb(0, 255, 60));
        }
        "solar" => {
            visuals.window_fill = Color32::from_rgb(20, 10, 0);
            visuals.panel_fill = Color32::from_rgb(20, 10, 0);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(35, 18, 0);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(50, 28, 0);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(80, 45, 0);
            visuals.widgets.active.bg_fill = Color32::from_rgb(200, 120, 0);
            visuals.selection.bg_fill = Color32::from_rgb(180, 100, 0);
            visuals.override_text_color = Some(Color32::from_rgb(255, 210, 100));
        }
        _ => { // neon (default)
            visuals.window_fill = Color32::from_rgb(12, 12, 20);
            visuals.panel_fill = Color32::from_rgb(12, 12, 20);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(20, 20, 35);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(25, 25, 45);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(30, 30, 60);
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 120, 200);
            visuals.selection.bg_fill = Color32::from_rgb(0, 100, 180);
            visuals.override_text_color = Some(Color32::from_rgb(200, 210, 230));
        }
    }
    ctx.set_visuals(visuals);
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

    let mut st = state.lock();

    // Keyboard shortcuts
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
        if i.key_pressed(Key::Num1) { st.viz_tab = 0; }
        if i.key_pressed(Key::Num2) { st.viz_tab = 1; }
        if i.key_pressed(Key::Num3) { st.viz_tab = 2; }
        if i.key_pressed(Key::Num4) { st.viz_tab = 3; }
        if i.key_pressed(Key::Num5) { st.viz_tab = 4; }
    });

    SidePanel::left("controls").min_width(300.0).max_width(340.0).show(ctx, |ui| {
        ScrollArea::vertical().show(ui, |ui| {

            // ---- THEME ----
            ui.horizontal(|ui| {
                ui.label("Theme:");
                for t in &["neon", "vaporwave", "crt", "solar"] {
                    let selected = st.theme == *t;
                    let color = if selected { Color32::from_rgb(0, 150, 220) } else { Color32::from_rgb(40, 40, 70) };
                    if ui.add(Button::new(*t).fill(color).min_size(Vec2::new(60.0, 20.0))).clicked() {
                        st.theme = t.to_string();
                    }
                }
            });

            ui.separator();

            // ---- PRESETS ----
            CollapsingHeader::new("PRESETS").default_open(true).show(ui, |ui| {
                let preset_names: Vec<&str> = PRESETS.iter().map(|p| p.name).collect();
                let sel = st.selected_preset.clone();
                ComboBox::from_label("Preset")
                    .selected_text(&sel)
                    .show_ui(ui, |ui| {
                        for name in &preset_names {
                            if ui.selectable_label(sel == *name, *name).clicked() {
                                st.selected_preset = name.to_string();
                            }
                        }
                    });
                if ui.button("Load Preset").clicked() {
                    st.config = load_preset(&st.selected_preset.clone());
                    st.system_changed = true;
                    st.mode_changed = true;
                }
                if let Some(p) = PRESETS.iter().find(|p| p.name == st.selected_preset.as_str()) {
                    ui.label(egui::RichText::new(p.description).italics().color(Color32::from_rgb(140, 160, 200)));
                }
            });

            ui.separator();

            // ---- PATCHES ----
            CollapsingHeader::new("PATCHES").default_open(false).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut st.patch_name_input).desired_width(140.0).hint_text("Patch name..."));
                    if ui.button("Save").clicked() {
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
                    ui.label(RichText::new("No patches saved yet").color(Color32::from_rgb(100, 100, 120)).small());
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

            ui.separator();

            // ---- SYSTEM ----
            CollapsingHeader::new("SYSTEM").default_open(true).show(ui, |ui| {
                let systems = ["lorenz", "rossler", "double_pendulum", "geodesic_torus", "kuramoto", "three_body"];
                let current_sys = st.config.system.name.clone();
                ComboBox::from_label("System")
                    .selected_text(&current_sys)
                    .show_ui(ui, |ui| {
                        for s in &systems {
                            if ui.selectable_label(current_sys == *s, *s).clicked() {
                                st.config.system.name = s.to_string();
                                st.system_changed = true;
                            }
                        }
                    });

                ui.add(Slider::new(&mut st.config.system.dt, 0.0001..=0.01).text("dt").logarithmic(true));
                ui.add(Slider::new(&mut st.config.system.speed, 0.1..=10.0).text("Speed"));

                CollapsingHeader::new("System Parameters").default_open(false).show(ui, |ui| {
                    match st.config.system.name.as_str() {
                        "lorenz" => {
                            ui.add(Slider::new(&mut st.config.lorenz.sigma, 1.0..=20.0).text("σ"));
                            ui.add(Slider::new(&mut st.config.lorenz.rho,   10.0..=50.0).text("ρ"));
                            ui.add(Slider::new(&mut st.config.lorenz.beta,  0.5..=5.0).text("β"));
                        }
                        "rossler" => {
                            ui.add(Slider::new(&mut st.config.rossler.a, 0.01..=0.5).text("a"));
                            ui.add(Slider::new(&mut st.config.rossler.b, 0.01..=0.5).text("b"));
                            ui.add(Slider::new(&mut st.config.rossler.c, 1.0..=15.0).text("c"));
                        }
                        "double_pendulum" => {
                            ui.add(Slider::new(&mut st.config.double_pendulum.m1, 0.1..=5.0).text("m₁"));
                            ui.add(Slider::new(&mut st.config.double_pendulum.m2, 0.1..=5.0).text("m₂"));
                            ui.add(Slider::new(&mut st.config.double_pendulum.l1, 0.1..=3.0).text("l₁"));
                            ui.add(Slider::new(&mut st.config.double_pendulum.l2, 0.1..=3.0).text("l₂"));
                        }
                        "geodesic_torus" => {
                            ui.add(Slider::new(&mut st.config.geodesic_torus.big_r, 1.0..=8.0).text("R"));
                            ui.add(Slider::new(&mut st.config.geodesic_torus.r,     0.1..=3.0).text("r"));
                        }
                        "kuramoto" => {
                            ui.add(Slider::new(&mut st.config.kuramoto.coupling, 0.0..=5.0).text("K (coupling)"));
                        }
                        _ => {}
                    }
                });

                // Randomize button
                if ui.button("🎲 Randomize").clicked() {
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

                // LFO / Auto-Wander
                CollapsingHeader::new("Auto-Wander (LFO)").default_open(false).show(ui, |ui| {
                    ui.checkbox(&mut st.lfo_enabled, "Enable LFO");
                    ui.add(Slider::new(&mut st.lfo_rate, 0.01..=2.0).text("Rate Hz").logarithmic(true));
                    ui.add(Slider::new(&mut st.lfo_depth, 0.0..=1.0).text("Depth"));
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

            ui.separator();

            // ---- SONIFICATION ----
            CollapsingHeader::new("SONIFICATION").default_open(true).show(ui, |ui| {
                let modes = ["direct", "orbital", "granular", "spectral", "fm"];
                let current_mode = st.config.sonification.mode.clone();
                ui.horizontal_wrapped(|ui| {
                    for m in &modes {
                        let selected = current_mode == *m;
                        let color = if selected { Color32::from_rgb(0, 150, 220) } else { Color32::from_rgb(40, 40, 70) };
                        if ui.add(Button::new(*m).fill(color).min_size(Vec2::new(55.0, 22.0))).clicked() {
                            st.config.sonification.mode = m.to_string();
                            st.mode_changed = true;
                        }
                    }
                });

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

                ui.add(Slider::new(&mut st.config.sonification.base_frequency, 55.0..=880.0)
                    .text("Root Hz").logarithmic(true));
                ui.add(Slider::new(&mut st.config.sonification.octave_range, 1.0..=6.0).text("Octave Range"));
            });

            ui.separator();

            // ---- HARMONY ----
            CollapsingHeader::new("HARMONY").default_open(true).show(ui, |ui| {
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
                    });

                let mut ts = st.config.sonification.transpose_semitones as f64;
                if ui.add(Slider::new(&mut ts, -24.0..=24.0).text("Transpose st").step_by(1.0)).changed() {
                    st.config.sonification.transpose_semitones = ts as f32;
                }

                ui.add(Slider::new(&mut st.config.sonification.portamento_ms, 1.0..=1000.0)
                    .text("Portamento ms").logarithmic(true));

                ui.label("Voice Levels:");
                for i in 0..4 {
                    ui.add(Slider::new(&mut st.config.sonification.voice_levels[i], 0.0..=1.0)
                        .text(format!("Voice {}", i + 1)));
                }

                // Per-voice waveform shapes
                ui.add_space(4.0);
                ui.label("Voice Waveforms:");
                let shapes = ["sine", "triangle", "saw"];
                for i in 0..4 {
                    ui.horizontal(|ui| {
                        ui.label(format!("V{}:", i + 1));
                        let current_shape = st.config.sonification.voice_shapes[i].clone();
                        for s in &shapes {
                            let selected = current_shape == *s;
                            let color = if selected { Color32::from_rgb(0, 150, 220) } else { Color32::from_rgb(40, 40, 70) };
                            if ui.add(Button::new(*s).fill(color).min_size(Vec2::new(50.0, 18.0))).clicked() {
                                st.config.sonification.voice_shapes[i] = s.to_string();
                            }
                        }
                    });
                }
            });

            ui.separator();

            // ---- EFFECTS ----
            CollapsingHeader::new("EFFECTS").default_open(false).show(ui, |ui| {
                ui.add(Slider::new(&mut st.config.audio.reverb_wet, 0.0..=1.0).text("Reverb"));
                ui.add(Slider::new(&mut st.config.audio.delay_ms, 0.0..=1000.0).text("Delay ms"));
                ui.add(Slider::new(&mut st.config.audio.delay_feedback, 0.0..=0.95).text("Delay FB"));

                // BPM Sync
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add(Slider::new(&mut st.bpm, 60.0..=200.0).text("BPM"));
                    let sync_color = if st.bpm_sync { Color32::from_rgb(0, 180, 0) } else { Color32::from_rgb(60, 60, 60) };
                    if ui.add(Button::new("Sync").fill(sync_color).min_size(Vec2::new(44.0, 22.0))).clicked() {
                        st.bpm_sync = !st.bpm_sync;
                    }
                });
                if st.bpm_sync {
                    ui.label(RichText::new(format!("Delay: {:.0}ms  LFO: {:.2}Hz", 60000.0 / st.bpm, st.bpm / 60.0 * 0.25))
                        .small().color(Color32::from_rgb(100, 200, 100)));
                }

                // Bitcrusher
                ui.add_space(4.0);
                ui.label("Bitcrusher:");
                ui.add(Slider::new(&mut st.config.audio.bit_depth, 1.0..=16.0).text("Bit Depth"));
                ui.add(Slider::new(&mut st.config.audio.rate_crush, 0.0..=1.0).text("Rate Crush"));
            });

            ui.separator();

            // ---- MASTER ----
            CollapsingHeader::new("MASTER").default_open(true).show(ui, |ui| {
                ui.add(Slider::new(&mut st.config.audio.master_volume, 0.0..=1.0).text("Volume"));

                let pause_label = if st.paused { "▶  Resume" } else { "⏸  Pause" };
                let btn = Button::new(pause_label)
                    .fill(Color32::from_rgb(0, 90, 160))
                    .min_size(Vec2::new(200.0, 32.0));
                if ui.add(btn).clicked() {
                    st.paused = !st.paused;
                }

                ui.add_space(4.0);

                // Record WAV button
                let st_sample_rate = st.sample_rate;
                let is_recording = recording.try_lock().map(|r| r.is_some()).unwrap_or(false);
                let rec_label = if is_recording { "⏹  Stop Recording" } else { "⏺  Record WAV" };
                let rec_color = if is_recording { Color32::from_rgb(180, 30, 30) } else { Color32::from_rgb(30, 120, 30) };
                if ui.add(Button::new(rec_label).fill(rec_color).min_size(Vec2::new(200.0, 32.0))).clicked() {
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

                ui.add_space(4.0);

                // Loop Export
                ui.horizontal(|ui| {
                    ui.label("Bars:");
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
                    let export_label = if is_exporting { "Exporting..." } else { "Export Loop" };
                    let export_color = if is_exporting { Color32::from_rgb(180, 120, 0) } else { Color32::from_rgb(0, 100, 100) };
                    if !is_exporting && ui.add(Button::new(export_label).fill(export_color)).clicked() {
                        let bars = st.loop_bars;
                        let bpm = st.bpm;
                        let sr = st.sample_rate;
                        // total samples = bars * beats_per_bar(4) * seconds_per_beat * sample_rate
                        let total_samples = ((bars as f64 * 4.0 * 60.0 / bpm as f64) * sr as f64) as u64;
                        if let Some(mut lock) = loop_export.try_lock() {
                            *lock = Some(total_samples);
                        }
                    }
                });

                ui.add_space(4.0);

                // Automation
                CollapsingHeader::new("Automate").default_open(false).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let rec_color = if st.auto_recording { Color32::from_rgb(180, 30, 30) } else { Color32::from_rgb(60, 40, 40) };
                        if ui.add(Button::new("⏺ Rec").fill(rec_color).min_size(Vec2::new(50.0, 24.0))).clicked() {
                            st.auto_recording = !st.auto_recording;
                            if st.auto_recording {
                                st.auto_playing = false;
                                st.auto_events.clear();
                                st.auto_start_time = Instant::now();
                            }
                        }
                        let play_color = if st.auto_playing { Color32::from_rgb(0, 150, 0) } else { Color32::from_rgb(40, 60, 40) };
                        if ui.add(Button::new("▶ Play").fill(play_color).min_size(Vec2::new(50.0, 24.0))).clicked() {
                            st.auto_playing = !st.auto_playing;
                            if st.auto_playing {
                                st.auto_recording = false;
                                st.auto_play_pos = 0;
                                st.auto_start_time = Instant::now();
                            }
                        }
                        if ui.button("■ Stop").clicked() {
                            st.auto_recording = false;
                            st.auto_playing = false;
                        }
                    });
                    ui.checkbox(&mut st.auto_loop, "Loop playback");
                    ui.label(RichText::new(format!("{} events recorded", st.auto_events.len())).small()
                        .color(Color32::from_rgb(100, 150, 200)));
                    if st.auto_recording {
                        ui.label(RichText::new("● Recording...").color(Color32::from_rgb(255, 80, 80)));
                    }
                    if st.auto_playing {
                        ui.label(RichText::new("► Playing back").color(Color32::from_rgb(80, 255, 80)));
                    }
                });

                ui.add_space(4.0);

                // Save Config button
                if ui.add(Button::new("💾  Save Config").fill(Color32::from_rgb(40, 60, 40)).min_size(Vec2::new(200.0, 28.0))).clicked() {
                    let toml_str = toml::to_string_pretty(&st.config).unwrap_or_default();
                    let _ = std::fs::write("config.toml", toml_str);
                }

                ui.add_space(4.0);

                // MIDI toggle
                ui.checkbox(&mut st.midi_enabled, "MIDI Output");
                if st.midi_enabled {
                    ui.label(RichText::new("Sending to first MIDI port").small().color(Color32::from_rgb(100, 200, 100)));
                }

                ui.add_space(8.0);
                let chaos = st.chaos_level;
                let chaos_color = lerp_color(
                    Color32::from_rgb(0, 80, 180),
                    Color32::from_rgb(220, 40, 40),
                    chaos,
                );
                ui.add(
                    ProgressBar::new(chaos)
                        .text(format!("Chaos  {:.0}%", chaos * 100.0))
                        .fill(chaos_color)
                );

                ui.add_space(4.0);
                ui.label(RichText::new("Space: pause  |  ↑↓: volume  |  ←→: speed  |  1-5: tabs")
                    .small()
                    .color(Color32::from_rgb(90, 100, 130)));
            });
        });
    });

    // ---- CENTRAL PANEL: Visualization ----
    CentralPanel::default().show(ctx, |ui| {
        // Tab bar
        ui.horizontal(|ui| {
            let tabs = ["Phase Portrait", "Waveform", "Note Map", "Math View", "Bifurcation"];
            let mut viz_tab = st.viz_tab;
            for (i, name) in tabs.iter().enumerate() {
                let selected = viz_tab == i;
                let color = if selected { Color32::from_rgb(0, 150, 220) } else { Color32::from_rgb(60, 60, 90) };
                let btn = Button::new(*name).fill(color).min_size(Vec2::new(110.0, 28.0));
                if ui.add(btn).clicked() {
                    viz_tab = i;
                }
            }
            st.viz_tab = viz_tab;
        });

        // Trail length slider + projection buttons for Phase Portrait tab
        let viz_tab = st.viz_tab;
        if viz_tab == 0 {
            ui.horizontal(|ui| {
                ui.label("Trail:");
                ui.add(Slider::new(&mut st.config.viz.trail_length, 100..=2000).text("pts"));
                ui.separator();
                ui.label("Projection:");
                for (i, label) in ["XY", "XZ", "YZ"].iter().enumerate() {
                    let selected = st.viz_projection == i;
                    let color = if selected { Color32::from_rgb(0, 150, 220) } else { Color32::from_rgb(40, 40, 70) };
                    if ui.add(Button::new(*label).fill(color).min_size(Vec2::new(36.0, 22.0))).clicked() {
                        st.viz_projection = i;
                    }
                }
            });
        }

        // Bifurcation controls
        if viz_tab == 4 {
            ui.horizontal(|ui| {
                let params = ["rho", "sigma", "coupling", "c"];
                let current_bp = st.bifurc_param.clone();
                ComboBox::from_id_source("bifurc_param")
                    .selected_text(&current_bp)
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        for p in &params {
                            if ui.selectable_label(current_bp == *p, *p).clicked() {
                                st.bifurc_param = p.to_string();
                            }
                        }
                    });
                let computing = st.bifurc_computing;
                let compute_color = if computing { Color32::from_rgb(100, 60, 0) } else { Color32::from_rgb(0, 80, 120) };
                if !computing && ui.add(Button::new("Compute").fill(compute_color)).clicked() {
                    st.bifurc_computing = true;
                    // Start computation in background thread
                    let param = st.bifurc_param.clone();
                    let sys_name = st.config.system.name.clone();
                    let lorenz_cfg = st.config.lorenz.clone();
                    let rossler_cfg = st.config.rossler.clone();
                    let kuramoto_cfg = st.config.kuramoto.clone();
                    let bifurc_data_clone = bifurc_data.clone();
                    let state_clone = state.clone();
                    std::thread::spawn(move || {
                        let mut result = Vec::new();
                        let steps = 200usize;
                        let (pmin, pmax) = param_range(&param);
                        for i in 0..steps {
                            let pval = pmin + (pmax - pmin) * i as f64 / (steps - 1) as f64;
                            let mut sys: Box<dyn DynamicalSystem> = build_bifurc_system(&sys_name, &param, pval, &lorenz_cfg, &rossler_cfg, &kuramoto_cfg);
                            // Warmup
                            for _ in 0..2000 { sys.step(0.005); }
                            // Record
                            for _ in 0..100 {
                                sys.step(0.005);
                                let state_v = sys.state();
                                if !state_v.is_empty() {
                                    result.push((pval as f32, state_v[0] as f32));
                                }
                            }
                        }
                        *bifurc_data_clone.lock() = result;
                        state_clone.lock().bifurc_computing = false;
                    });
                }
                if computing {
                    ui.label(RichText::new("Computing...").color(Color32::from_rgb(255, 200, 0)));
                }
            });
        }

        ui.separator();

        let projection = st.viz_projection;
        let system_name = st.config.system.name.clone();
        let mode_name = st.config.sonification.mode.clone();
        let freqs = [
            st.config.sonification.base_frequency as f32,
            st.config.sonification.base_frequency as f32 * 2.0,
            st.config.sonification.base_frequency as f32 * 3.0,
            st.config.sonification.base_frequency as f32 * 4.0,
        ];
        let voice_levels = st.config.sonification.voice_levels;
        let chord_intervals = chord_intervals_for(&st.config.sonification.chord_mode);
        let current_state = st.current_state.clone();
        let current_deriv = st.current_deriv.clone();
        let chaos_level = st.chaos_level;
        let order_param = st.order_param;
        let kuramoto_phases = st.kuramoto_phases.clone();
        drop(st); // release lock before painting

        match viz_tab {
            0 => draw_phase_portrait(ui, viz_points, &system_name, &mode_name, &current_state, &current_deriv, projection),
            1 => draw_waveform(ui, waveform),
            2 => draw_note_map(ui, &freqs, &voice_levels, &chord_intervals),
            3 => draw_math_view(ui, &system_name, &current_state, &current_deriv, chaos_level, order_param, &kuramoto_phases),
            4 => draw_bifurc_diagram(ui, bifurc_data),
            _ => {}
        }
    });
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

fn draw_bifurc_diagram(ui: &mut Ui, bifurc_data: &Arc<Mutex<Vec<(f32, f32)>>>) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let data = if let Some(d) = bifurc_data.try_lock() { d.clone() } else { return; };

    if data.is_empty() {
        painter.text(rect.center(), Align2::CENTER_CENTER,
            "Click 'Compute' to generate bifurcation diagram",
            FontId::proportional(16.0), Color32::from_rgb(80, 80, 120));
        return;
    }

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
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

    // Extract projected coordinates
    let proj_pts: Vec<(f32, f32, f32, bool)> = points.iter().map(|&(x, y, z, s, c)| {
        let (pa, pb) = match projection {
            1 => (x, z), // XZ
            2 => (y, z), // YZ
            _ => (x, y), // XY
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

    // Draw trail
    for (idx, w) in proj_pts.windows(2).enumerate() {
        let (x0, y0, s0, _) = w[0];
        let (x1, y1, _, _) = w[1];
        let p0 = to_screen(x0, y0);
        let p1 = to_screen(x1, y1);
        let recency = idx as f32 / n as f32;
        let alpha = (recency * 255.0) as u8;
        let col = speed_color(s0);
        let col_a = Color32::from_rgba_premultiplied(
            (col.r() as f32 * recency) as u8,
            (col.g() as f32 * recency) as u8,
            (col.b() as f32 * recency) as u8,
            alpha,
        );
        let glow_col = Color32::from_rgba_premultiplied(
            (col.r() as f32 * recency * 0.3) as u8,
            (col.g() as f32 * recency * 0.3) as u8,
            (col.b() as f32 * recency * 0.3) as u8,
            (alpha as f32 * 0.3) as u8,
        );
        painter.line_segment([p0, p1], Stroke::new(4.0, glow_col));
        painter.line_segment([p0, p1], Stroke::new(1.5, col_a));
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

    // Corner labels
    painter.text(
        rect.left_top() + Vec2::new(8.0, 8.0),
        Align2::LEFT_TOP,
        format!("System: {}", system_name),
        FontId::proportional(12.0),
        Color32::from_rgb(120, 140, 180),
    );
    painter.text(
        rect.left_top() + Vec2::new(8.0, 24.0),
        Align2::LEFT_TOP,
        format!("Mode: {}", mode_name),
        FontId::proportional(12.0),
        Color32::from_rgb(100, 120, 160),
    );

    // Projection label
    let proj_label = match projection { 1 => "XZ Projection", 2 => "YZ Projection", _ => "XY Projection" };
    painter.text(
        rect.left_top() + Vec2::new(8.0, 40.0),
        Align2::LEFT_TOP,
        proj_label,
        FontId::proportional(11.0),
        Color32::from_rgb(80, 100, 160),
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

    // State values
    if !current_state.is_empty() {
        let var_names = dim_names(system_name);
        for (i, (&val, name)) in current_state.iter().zip(var_names.iter()).enumerate().take(6) {
            let text = format!("{} = {:+.3}", name, val);
            let pos = rect.right_top() + Vec2::new(-8.0, 8.0 + i as f32 * 14.0);
            painter.text(pos, Align2::RIGHT_TOP, text, FontId::monospace(10.0), Color32::from_rgba_premultiplied(100, 200, 100, 200));
        }
    }
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
    painter.line_segment(
        [Pos2::new(rect.left(), cy), Pos2::new(rect.right(), cy)],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(40, 60, 40, 100)),
    );

    let n = samples.len();
    let w = rect.width();

    let neon_green = Color32::from_rgb(0, 255, 100);

    let pts: Vec<Pos2> = samples.iter().enumerate().map(|(i, &s)| {
        let x = rect.left() + (i as f32 / n as f32) * w;
        let y = cy - s.clamp(-1.0, 1.0) * (rect.height() * 0.45);
        Pos2::new(x, y)
    }).collect();

    for w in pts.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(1.5, neon_green));
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
        "lorenz" => "ẋ = σ(y-x)\nẏ = x(ρ-z)-y\nż = xy-βz",
        "rossler" => "ẋ = -y-z\nẏ = x+ay\nż = b+z(x-c)",
        "double_pendulum" => "θ̈₁ = f(θ₁,θ₂,ω₁,ω₂)\nθ̈₂ = g(θ₁,θ₂,ω₁,ω₂)",
        "geodesic_torus" => "φ̈ = -2(r sinθ/(R+r cosθ))φ̇θ̇\nθ̈ = (R+r cosθ)sinθ/r · φ̇²",
        "kuramoto" => "θ̇ᵢ = ωᵢ + K/N Σⱼ sin(θⱼ-θᵢ)",
        "three_body" => "r̈ᵢ = G Σⱼ mⱼ(rⱼ-rᵢ)/|rⱼ-rᵢ|³",
        _ => "",
    }
}

fn equation_lines(system: &str) -> Vec<&'static str> {
    match system {
        "lorenz" => vec!["ẋ = σ(y - x)", "ẏ = x(ρ - z) - y", "ż = xy - βz"],
        "rossler" => vec!["ẋ = -y - z", "ẏ = x + ay", "ż = b + z(x - c)"],
        "double_pendulum" => vec![
            "θ̈₁ = (p₁l₂ - p₂l₁cosΔ) / (l₁²l₂M·det)",
            "θ̈₂ = ((m₁+m₂)l₁p₂ - m₂l₂p₁cosΔ) / (m₂l₁l₂²M·det)",
            "ṗ₁ = -(m₁+m₂)gl₁sinθ₁ - θ̇₁θ̇₂l₁l₂m₂sinΔ",
            "ṗ₂ = -m₂gl₂sinθ₂ + θ̇₁θ̇₂l₁l₂m₂sinΔ",
        ],
        "geodesic_torus" => vec![
            "φ̈ = -2·r·sinθ/(R+r·cosθ) · φ̇θ̇",
            "θ̈ = (R+r·cosθ)·sinθ/r · φ̇²",
            "ds² = (R+r·cosθ)²dφ² + r²dθ²",
        ],
        "kuramoto" => vec![
            "θ̇ᵢ = ωᵢ + K/N · Σⱼsin(θⱼ-θᵢ)",
            "r·e^(iψ) = 1/N · Σⱼe^(iθⱼ)",
            "Critical: Kc = 2γ (Lorentzian width γ)",
        ],
        "three_body" => vec![
            "r̈₁ = G·m₂(r₂-r₁)/|r₂-r₁|³ + G·m₃(r₃-r₁)/|r₃-r₁|³",
            "r̈₂ = G·m₁(r₁-r₂)/|r₁-r₂|³ + G·m₃(r₃-r₂)/|r₃-r₂|³",
            "r̈₃ = G·m₁(r₁-r₃)/|r₁-r₃|³ + G·m₂(r₂-r₃)/|r₂-r₃|³",
        ],
        _ => vec!["No equations available"],
    }
}

fn dim_names(system: &str) -> &'static [&'static str] {
    match system {
        "lorenz" | "rossler" => &["x", "y", "z"],
        "double_pendulum" => &["θ₁", "θ₂", "ω₁", "ω₂"],
        "geodesic_torus" => &["φ", "θ", "φ̇", "θ̇"],
        "kuramoto" => &["θ₁", "θ₂", "θ₃", "θ₄"],
        "three_body" => &["x₁", "y₁", "x₂", "y₂", "x₃", "y₃"],
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
) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let mid_x = rect.center().x;

    let mut y = rect.top() + 20.0;
    let x = rect.left() + 20.0;

    painter.text(Pos2::new(x, y), Align2::LEFT_TOP,
        format!("System: {}", system_name),
        FontId::proportional(18.0),
        Color32::from_rgb(100, 180, 255));
    y += 30.0;

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
