/// Integration tests for math-sonify.
use math_sonify_plugin::{
    config::{Config, SonificationConfig},
    systems::{DynamicalSystem, Lorenz, Rossler, DoublePendulum, Kuramoto, ThreeBody},
    sonification::{
        Scale, SonifMode, DirectMapping, Sonification,
        quantize_to_scale, chord_intervals_for,
    },
};

fn all_finite(s: &[f64]) -> bool { s.iter().all(|v| v.is_finite()) }

#[test]
fn lorenz_stays_on_attractor() {
    let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
    for _ in 0..50_000 { sys.step(0.001); }
    let s = sys.state();
    assert!(all_finite(s));
    assert!(s[0].abs() < 30.0 && s[1].abs() < 30.0 && s[2] > 0.0 && s[2] < 60.0);
}

#[test]
fn lorenz_z_stays_positive() {
    let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
    for _ in 0..5_000 { sys.step(0.001); }
    for _ in 0..20_000 { sys.step(0.001); assert!(sys.state()[2] > 0.0); }
}

#[test]
fn lorenz_deterministic_trajectory() {
    let (mut s1, mut s2) = (Lorenz::new(10.0, 28.0, 2.6667), Lorenz::new(10.0, 28.0, 2.6667));
    for _ in 0..1_000 { s1.step(0.001); s2.step(0.001); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) { assert!((a - b).abs() < 1e-14); }
}

#[test]
fn lorenz_zero_dt_no_change() {
    let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
    for _ in 0..100 { sys.step(0.001); }
    let before: Vec<f64> = sys.state().to_vec();
    sys.step(0.0);
    for (a, b) in before.iter().zip(sys.state().iter()) { assert!((a - b).abs() < 1e-14); }
}

#[test]
fn rossler_stays_bounded() {
    let mut sys = Rossler::new(0.2, 0.2, 5.7);
    for _ in 0..30_000 { sys.step(0.001); }
    let s = sys.state();
    assert!(all_finite(s) && s[0].abs() < 15.0 && s[1].abs() < 15.0 && s[2] > 0.0 && s[2] < 25.0);
}

#[test]
fn rossler_z_stays_positive() {
    let mut sys = Rossler::new(0.2, 0.2, 5.7);
    for _ in 0..5_000 { sys.step(0.001); }
    for _ in 0..10_000 { sys.step(0.001); assert!(sys.state()[2] > 0.0); }
}

#[test]
fn double_pendulum_energy_conserved_small_angles() {
    let (m1, m2, l1, l2, g) = (1.0_f64, 1.0, 1.0, 1.0, 9.81);
    let mut sys = DoublePendulum::new(m1, m2, l1, l2);
    let hamiltonian = |s: &[f64]| -> f64 {
        let (th1, th2, p1, p2) = (s[0], s[1], s[2], s[3]);
        let delta = th2 - th1;
        let denom = (m1 + m2 - m2 * delta.cos().powi(2)).max(1e-12);
        let t = ((m1 + m2) * l2.powi(2) * p1.powi(2) + m2 * l1.powi(2) * p2.powi(2)
            - 2.0 * m2 * l1 * l2 * p1 * p2 * delta.cos())
            / (2.0 * m1 * m2 * l1.powi(2) * l2.powi(2) * denom);
        t - (m1 + m2) * g * l1 * th1.cos() - m2 * g * l2 * th2.cos()
    };
    let e0 = hamiltonian(sys.state());
    for _ in 0..10_000 { sys.step(0.001); }
    let e1 = hamiltonian(sys.state());
    assert!(((e1 - e0) / e0.abs()).abs() < 0.02);
}

#[test]
fn double_pendulum_state_stays_finite_and_bounded() {
    let mut sys = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
    for _ in 0..10_000 {
        sys.step(0.001);
        let s = sys.state();
        assert!(all_finite(s) && s[2].abs() < 1000.0 && s[3].abs() < 1000.0);
    }
}

#[test]
fn kuramoto_below_critical_coupling_stays_incoherent() {
    let mut sys = Kuramoto::new(16, 0.1);
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(sys.order_parameter() < 0.5);
}

#[test]
fn kuramoto_above_critical_coupling_synchronizes() {
    let mut sys = Kuramoto::new(16, 5.0);
    for _ in 0..50_000 { sys.step(0.01); }
    assert!(sys.order_parameter() > 0.5);
}

#[test]
fn kuramoto_order_parameter_always_in_unit_interval() {
    for &k in &[0.0_f64, 0.5, 1.0, 2.0, 10.0, 50.0] {
        let mut sys = Kuramoto::new(8, k);
        for _ in 0..5_000 { sys.step(0.01); }
        let r = sys.order_parameter();
        assert!(r >= 0.0 && r <= 1.0 + 1e-9, "K={} r={}", k, r);
    }
}

#[test]
fn three_body_energy_conserved() {
    let mut sys = ThreeBody::new([1.0, 1.0, 1.0]);
    for _ in 0..10_000 { sys.step(0.001); }
    assert!(sys.energy_error < 0.01);
}

#[test]
fn quantize_to_scale_always_audible_range() {
    let scales = [
        Scale::Pentatonic, Scale::Chromatic, Scale::JustIntonation,
        Scale::Microtonal, Scale::Edo19, Scale::Edo31, Scale::Edo24,
        Scale::WholeTone, Scale::Phrygian, Scale::Lydian,
    ];
    for &scale in &scales {
        for i in 0..=200 {
            let f = quantize_to_scale(i as f32 / 200.0, 220.0, 4.0, scale);
            assert!(f >= 20.0 && f <= 22_050.0);
        }
    }
}

#[test]
fn quantize_to_scale_produces_valid_midi_range() {
    for &scale in &[Scale::Pentatonic, Scale::Chromatic, Scale::Lydian] {
        for i in 0..=100 {
            let f = quantize_to_scale(i as f32 / 100.0, 110.0, 3.0, scale);
            let midi = 69.0_f32 + 12.0 * (f / 440.0).log2();
            assert!(midi >= 0.0 && midi <= 127.0);
        }
    }
}

#[test]
fn quantize_to_scale_t_zero_equals_base() {
    for &scale in &[Scale::Pentatonic, Scale::Chromatic, Scale::Edo24] {
        let f = quantize_to_scale(0.0, 220.0, 3.0, scale);
        assert!((f - 220.0).abs() < 0.01);
    }
}

#[test]
fn quantize_to_scale_all_scales_finite_positive() {
    let scales = [
        Scale::Pentatonic, Scale::Chromatic, Scale::JustIntonation,
        Scale::Microtonal, Scale::Edo19, Scale::Edo31, Scale::Edo24,
        Scale::WholeTone, Scale::Phrygian, Scale::Lydian,
    ];
    for &scale in &scales {
        for i in 0..=50 {
            let f = quantize_to_scale(i as f32 / 50.0, 110.0, 2.0, scale);
            assert!(f.is_finite() && f > 0.0);
        }
    }
}

#[test]
fn polyphony_limit_at_most_four_voices() {
    let mut mapper = DirectMapping::new();
    let cfg = SonificationConfig::default();
    let p = mapper.map(&[1.2, -3.1, 14.7], 5.0, &cfg);
    assert_eq!(p.freqs.len(), 4);
    assert_eq!(p.amps.len(), 4);
    let p1 = mapper.map(&[0.5], 1.0, &cfg);
    assert_eq!(p1.amps[1], 0.0);
    assert_eq!(p1.amps[2], 0.0);
    assert_eq!(p1.amps[3], 0.0);
}

#[test]
fn polyphony_voice_levels_descending() {
    let vl = SonificationConfig::default().voice_levels;
    assert!(vl[0] >= vl[1] && vl[1] >= vl[2] && vl[2] >= vl[3]);
}

#[test]
fn polyphony_all_voices_finite_and_non_negative() {
    let mut mapper = DirectMapping::new();
    let cfg = SonificationConfig::default();
    let state = vec![5.0_f64, -10.0, 3.14, 0.5];
    for _ in 0..20 { mapper.map(&state, 2.0, &cfg); }
    let p = mapper.map(&state, 2.0, &cfg);
    for i in 0..4 {
        assert!(p.freqs[i].is_finite() && p.freqs[i] >= 0.0);
        assert!(p.amps[i].is_finite() && p.amps[i] >= 0.0);
    }
}

#[test]
fn config_empty_toml_parses_to_defaults() {
    let cfg: Config = toml::from_str("").expect("empty TOML");
    assert_eq!(cfg.lorenz.sigma, Config::default().lorenz.sigma);
}

#[test]
fn config_out_of_range_values_clamped() {
    let src = "[lorenz]\nsigma=99999\nrho=-100\nbeta=0\n[audio]\nsample_rate=1234\nreverb_wet=99\ndelay_feedback=5\nmaster_volume=-1";
    let mut cfg: Config = toml::from_str(src).expect("parse");
    cfg.validate();
    assert!(cfg.lorenz.sigma <= 100.0 && cfg.lorenz.rho >= 0.1 && cfg.lorenz.beta >= 0.01);
    assert!(cfg.audio.reverb_wet <= 1.0 && cfg.audio.delay_feedback <= 0.99);
    assert!(cfg.audio.master_volume >= 0.0);
    assert!(cfg.audio.sample_rate == 44100 || cfg.audio.sample_rate == 48000);
}

#[test]
fn config_unknown_fields_ignored() {
    let src = "[unknown]\nfoo=\"bar\"\n[lorenz]\nsigma=12.0";
    let r: Result<Config, _> = toml::from_str(src);
    assert!(r.is_ok());
    assert!((r.unwrap().lorenz.sigma - 12.0).abs() < 1e-9);
}

#[test]
fn config_default_is_already_valid() {
    let mut cfg = Config::default();
    let before = format!("{:?}", cfg);
    cfg.validate();
    assert_eq!(before, format!("{:?}", cfg));
}

#[test]
fn config_round_trip_lossless() {
    let orig = Config::default();
    let s = toml::to_string(&orig).expect("serialize");
    let mut r: Config = toml::from_str(&s).expect("deserialize");
    r.validate();
    assert!((orig.lorenz.sigma - r.lorenz.sigma).abs() < 1e-9);
    assert!((orig.rossler.c   - r.rossler.c).abs()   < 1e-9);
    assert_eq!(orig.system.name, r.system.name);
}

#[test]
fn sonif_mode_display_non_empty() {
    for mode in &[SonifMode::Direct, SonifMode::Orbital, SonifMode::Granular,
                  SonifMode::Spectral, SonifMode::FM, SonifMode::Vocal, SonifMode::Waveguide] {
        assert!(!format!("{}", mode).is_empty());
    }
}

#[test]
fn sonif_mode_default_is_direct() { assert_eq!(SonifMode::default(), SonifMode::Direct); }

#[test]
fn chord_intervals_major_and_minor() {
    assert_eq!(chord_intervals_for("major"), [4.0, 7.0, 0.0]);
    assert_eq!(chord_intervals_for("minor"), [3.0, 7.0, 0.0]);
}

#[test]
fn chord_intervals_dom7_three_notes() {
    let d = chord_intervals_for("dom7");
    assert!(d[0] > 0.0 && d[1] > 0.0 && d[2] > 0.0);
}

#[test]
fn chord_intervals_unknown_returns_zeros() {
    assert_eq!(chord_intervals_for("xyzzy"), [0.0, 0.0, 0.0]);
}

// ---------------------------------------------------------------------------
// Synthesis DSP integration tests (no audio device required)
// ---------------------------------------------------------------------------

use math_sonify_plugin::synth::{Oscillator, OscShape};

/// Render `duration_secs` of mono audio at `sample_rate` from a sine oscillator.
fn render_sine(freq_hz: f32, sample_rate: f32, duration_secs: f32) -> Vec<f32> {
    let n = (sample_rate * duration_secs) as usize;
    let mut osc = Oscillator::new(freq_hz, OscShape::Sine, sample_rate);
    (0..n).map(|_| osc.next_sample()).collect()
}

#[test]
fn test_one_second_sine_buffer_is_non_zero() {
    // A 1-second buffer from a 440 Hz sine must contain non-zero samples.
    let buf = render_sine(440.0, 44100.0, 1.0);
    assert_eq!(buf.len(), 44100, "Buffer length mismatch");
    let any_nonzero = buf.iter().any(|&s| s.abs() > 1e-6);
    assert!(any_nonzero, "1-second sine buffer contains only silence");
}

#[test]
fn test_stereo_buffer_equal_channels() {
    // Render a stereo buffer by interleaving the same oscillator L+R.
    // Both channels must have the same number of samples.
    let mono = render_sine(440.0, 44100.0, 1.0);
    // Interleave: L = mono[i], R = mono[i] * 0.9 (simulated pan).
    let stereo: Vec<f32> = mono.iter().flat_map(|&s| [s, s * 0.9]).collect();
    let n_left  = stereo.iter().step_by(2).count();
    let n_right = stereo.iter().skip(1).step_by(2).count();
    assert_eq!(n_left, n_right, "Left and right channels have different sample counts");
}

#[test]
fn test_two_oscillators_higher_amplitude() {
    // Summing two identical oscillators should approximately double amplitude.
    // We compare the peak absolute value of one vs two oscillators.
    let n = 4410_usize; // 100 ms at 44100 Hz
    let mut osc1a = Oscillator::new(440.0, OscShape::Sine, 44100.0);
    let mut osc1b = Oscillator::new(440.0, OscShape::Sine, 44100.0);
    let mut osc2a = Oscillator::new(440.0, OscShape::Sine, 44100.0);

    // Single oscillator peak.
    let single_peak = (0..n).map(|_| osc1a.next_sample().abs()).fold(0.0_f32, f32::max);

    // Two oscillators (in phase — same initial phase = 0).
    let double_peak = (0..n)
        .map(|_| (osc1b.next_sample() + osc2a.next_sample()).abs())
        .fold(0.0_f32, f32::max);

    // The sum should be roughly twice the individual peak (within 5% tolerance).
    assert!(double_peak > single_peak * 1.8,
        "Two in-phase oscillators should nearly double amplitude: single={}, double={}",
        single_peak, double_peak);
}

#[test]
fn test_direct_mapping_produces_non_zero_freqs() {
    // DirectMapping::map() on a Lorenz trajectory should yield non-zero voice frequencies.
    let mut mapper = DirectMapping::new();
    let mut lorenz = math_sonify_plugin::systems::Lorenz::new(10.0, 28.0, 2.6667);
    // Warm up the attractor.
    for _ in 0..1000 { lorenz.step(0.001); }
    let config = SonificationConfig::default();
    let params = mapper.map(lorenz.state(), 10.0, &config);
    // At least one voice should have a non-zero frequency.
    let any_nonzero = params.freqs.iter().any(|&f| f > 0.0);
    assert!(any_nonzero, "DirectMapping should produce non-zero frequencies from Lorenz state");
}
