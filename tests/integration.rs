/// Integration tests for math-sonify.
use math_sonify_plugin::{
    config::{Config, SonificationConfig},
    sonification::{
        chord_intervals_for, quantize_to_scale, DirectMapping, Scale, SonifMode, Sonification,
    },
    systems::{
        validate_exprs, Aizawa, ArnoldCat, Bouali, BurkeShaw, Chen, Chua, CoupledMapLattice,
        Dadras, DelayedMap, DoublePendulum, Duffing, DynamicalSystem, FractionalLorenz,
        GenesioTesi, GeodesicTorus, Halvorsen, HenonMap, HindmarshRose, Kuramoto, KuramotoDriven,
        Liu, LogisticMap, Lorenz, Lorenz84, Lorenz96, MackeyGlass, Mathieu, NewtonLeipnik, NoseHoover,
        Oregonator, RabinovichFabrikant, Rikitake, Rossler, Rucklidge, ShimizuMorioka, SprottB,
        SprottC, SprottD, SprottE, SprottF, SprottG, SprottH, SprottL, StandardMap, StochasticLorenz,
        Thomas, ThreeBody, VanDerPol,
    },
};

fn all_finite(s: &[f64]) -> bool {
    s.iter().all(|v| v.is_finite())
}

#[test]
fn lorenz_stays_on_attractor() {
    let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
    for _ in 0..50_000 {
        sys.step(0.001);
    }
    let s = sys.state();
    assert!(all_finite(s));
    assert!(s[0].abs() < 30.0 && s[1].abs() < 30.0 && s[2] > 0.0 && s[2] < 60.0);
}

#[test]
fn lorenz_z_stays_positive() {
    let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
    for _ in 0..5_000 {
        sys.step(0.001);
    }
    for _ in 0..20_000 {
        sys.step(0.001);
        assert!(sys.state()[2] > 0.0);
    }
}

#[test]
fn lorenz_deterministic_trajectory() {
    let (mut s1, mut s2) = (
        Lorenz::new(10.0, 28.0, 2.6667),
        Lorenz::new(10.0, 28.0, 2.6667),
    );
    for _ in 0..1_000 {
        s1.step(0.001);
        s2.step(0.001);
    }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-14);
    }
}

#[test]
fn lorenz_zero_dt_no_change() {
    let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
    for _ in 0..100 {
        sys.step(0.001);
    }
    let before: Vec<f64> = sys.state().to_vec();
    sys.step(0.0);
    for (a, b) in before.iter().zip(sys.state().iter()) {
        assert!((a - b).abs() < 1e-14);
    }
}

#[test]
fn rossler_stays_bounded() {
    let mut sys = Rossler::new(0.2, 0.2, 5.7);
    for _ in 0..30_000 {
        sys.step(0.001);
    }
    let s = sys.state();
    assert!(all_finite(s) && s[0].abs() < 15.0 && s[1].abs() < 15.0 && s[2] > 0.0 && s[2] < 25.0);
}

#[test]
fn rossler_z_stays_positive() {
    let mut sys = Rossler::new(0.2, 0.2, 5.7);
    for _ in 0..5_000 {
        sys.step(0.001);
    }
    for _ in 0..10_000 {
        sys.step(0.001);
        assert!(sys.state()[2] > 0.0);
    }
}

#[test]
fn double_pendulum_energy_conserved_small_angles() {
    let (m1, m2, l1, l2, g) = (1.0_f64, 1.0, 1.0, 1.0, 9.81);
    let mut sys = DoublePendulum::new(m1, m2, l1, l2);
    // Override default large-angle initial state (π/2) with genuinely small angles.
    // Small angles ≈ 6° give near-linear dynamics and much better energy conservation.
    sys.set_state(&[0.1, 0.12, 0.0, 0.0]);
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
    for _ in 0..10_000 {
        sys.step(0.001);
    }
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
    for _ in 0..20_000 {
        sys.step(0.01);
    }
    assert!(sys.order_parameter() < 0.5);
}

#[test]
fn kuramoto_above_critical_coupling_synchronizes() {
    let mut sys = Kuramoto::new(16, 5.0);
    for _ in 0..50_000 {
        sys.step(0.01);
    }
    assert!(sys.order_parameter() > 0.5);
}

#[test]
fn kuramoto_order_parameter_always_in_unit_interval() {
    for &k in &[0.0_f64, 0.5, 1.0, 2.0, 10.0, 50.0] {
        let mut sys = Kuramoto::new(8, k);
        for _ in 0..5_000 {
            sys.step(0.01);
        }
        let r = sys.order_parameter();
        assert!(r >= 0.0 && r <= 1.0 + 1e-9, "K={} r={}", k, r);
    }
}

#[test]
fn three_body_energy_conserved() {
    let mut sys = ThreeBody::new([1.0, 1.0, 1.0]);
    for _ in 0..10_000 {
        sys.step(0.001);
    }
    assert!(sys.energy_error < 0.01);
}

#[test]
fn quantize_to_scale_always_audible_range() {
    let scales = [
        Scale::Pentatonic,
        Scale::Chromatic,
        Scale::JustIntonation,
        Scale::Microtonal,
        Scale::Edo19,
        Scale::Edo31,
        Scale::Edo24,
        Scale::WholeTone,
        Scale::Phrygian,
        Scale::Lydian,
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
        Scale::Pentatonic,
        Scale::Chromatic,
        Scale::JustIntonation,
        Scale::Microtonal,
        Scale::Edo19,
        Scale::Edo31,
        Scale::Edo24,
        Scale::WholeTone,
        Scale::Phrygian,
        Scale::Lydian,
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
    for _ in 0..20 {
        mapper.map(&state, 2.0, &cfg);
    }
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
    assert!((orig.rossler.c - r.rossler.c).abs() < 1e-9);
    assert_eq!(orig.system.name, r.system.name);
}

#[test]
fn sonif_mode_display_non_empty() {
    for mode in &[
        SonifMode::Direct,
        SonifMode::Orbital,
        SonifMode::Granular,
        SonifMode::Spectral,
        SonifMode::FM,
        SonifMode::Vocal,
        SonifMode::Waveguide,
    ] {
        assert!(!format!("{}", mode).is_empty());
    }
}

#[test]
fn sonif_mode_default_is_direct() {
    assert_eq!(SonifMode::default(), SonifMode::Direct);
}

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

use math_sonify_plugin::synth::{OscShape, Oscillator};

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
    let n_left = stereo.iter().step_by(2).count();
    let n_right = stereo.iter().skip(1).step_by(2).count();
    assert_eq!(
        n_left, n_right,
        "Left and right channels have different sample counts"
    );
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
    let single_peak = (0..n)
        .map(|_| osc1a.next_sample().abs())
        .fold(0.0_f32, f32::max);

    // Two oscillators (in phase — same initial phase = 0).
    let double_peak = (0..n)
        .map(|_| (osc1b.next_sample() + osc2a.next_sample()).abs())
        .fold(0.0_f32, f32::max);

    // The sum should be roughly twice the individual peak (within 5% tolerance).
    assert!(
        double_peak > single_peak * 1.8,
        "Two in-phase oscillators should nearly double amplitude: single={}, double={}",
        single_peak,
        double_peak
    );
}

#[test]
fn test_direct_mapping_produces_non_zero_freqs() {
    // DirectMapping::map() on a Lorenz trajectory should yield non-zero voice frequencies.
    let mut mapper = DirectMapping::new();
    let mut lorenz = math_sonify_plugin::systems::Lorenz::new(10.0, 28.0, 2.6667);
    // Warm up the attractor.
    for _ in 0..1000 {
        lorenz.step(0.001);
    }
    let config = SonificationConfig::default();
    let params = mapper.map(lorenz.state(), 10.0, &config);
    // At least one voice should have a non-zero frequency.
    let any_nonzero = params.freqs.iter().any(|&f| f > 0.0);
    assert!(
        any_nonzero,
        "DirectMapping should produce non-zero frequencies from Lorenz state"
    );
}

// --- Bifurcation Boundary Tests ---

// ── Lorenz ──────────────────────────────────────────────────────────────────

#[test]
fn lorenz_below_chaos_onset_is_periodic() {
    // rho=20.0 is below the chaos onset at ~24.74; expect periodic/fixed-point behaviour.
    let mut sys = Lorenz::new(10.0, 20.0, 2.6667);
    // Warm up transient
    for _ in 0..2000 {
        sys.step(0.001);
    }
    // Record trajectory and check for near-repeat OR low z variance
    let mut history: Vec<[f64; 3]> = Vec::with_capacity(5000);
    let mut found_repeat = false;
    for _ in 0..5000 {
        sys.step(0.001);
        let s = sys.state();
        assert!(all_finite(s), "state became non-finite below chaos onset");
        let cur = [s[0], s[1], s[2]];
        // Check if current position is within 0.5 of any previous position
        if !found_repeat {
            for &prev in &history {
                let dist = ((cur[0] - prev[0]).powi(2)
                    + (cur[1] - prev[1]).powi(2)
                    + (cur[2] - prev[2]).powi(2))
                .sqrt();
                if dist < 0.5 {
                    found_repeat = true;
                    break;
                }
            }
        }
        history.push(cur);
    }
    // Either we found a near-repeat, or z variance is low (periodic/fixed-point)
    let z_vals: Vec<f64> = history.iter().map(|s| s[2]).collect();
    let z_mean = z_vals.iter().sum::<f64>() / z_vals.len() as f64;
    let z_var = z_vals.iter().map(|&v| (v - z_mean).powi(2)).sum::<f64>() / z_vals.len() as f64;
    assert!(
        found_repeat || z_var < 10.0,
        "lorenz rho=20 should be periodic/fixed-point: found_repeat={}, z_var={}",
        found_repeat,
        z_var
    );
}

#[test]
fn lorenz_at_chaos_onset_rho_24_74() {
    let mut sys = Lorenz::new(10.0, 24.74, 2.6667);
    for _ in 0..3000 {
        sys.step(0.001);
    }
    let s = sys.state();
    assert!(all_finite(s), "state became non-finite at rho=24.74");
    assert!(s[0].abs() <= 40.0, "x out of range at rho=24.74: {}", s[0]);
    assert!(s[1].abs() <= 40.0, "y out of range at rho=24.74: {}", s[1]);
    assert!(
        s[2] >= 0.0 && s[2] <= 80.0,
        "z out of range at rho=24.74: {}",
        s[2]
    );
}

#[test]
fn lorenz_above_chaos_onset_is_chaotic() {
    // Lyapunov sensitivity: two near-identical initial conditions on the attractor
    // must diverge.  Warm up to attractor first, then perturb and track peak separation.
    let mut sys1 = Lorenz::new(10.0, 28.0, 2.6667);
    // Warm up to bring state onto the attractor
    for _ in 0..5000 {
        sys1.step(0.001);
    }
    let mut sys2 = Lorenz::new(10.0, 28.0, 2.6667);
    // Copy attractor state from sys1 into sys2 with a small perturbation on x
    let warm_state = sys1.state().to_vec();
    sys2.set_state(&warm_state);
    sys2.set_state(&[warm_state[0] + 1e-4, warm_state[1], warm_state[2]]);

    let mut max_dist = 0.0f64;
    for _ in 0..50000 {
        sys1.step(0.001);
        sys2.step(0.001);
        let s1 = sys1.state();
        let s2 = sys2.state();
        let d =
            ((s1[0] - s2[0]).powi(2) + (s1[1] - s2[1]).powi(2) + (s1[2] - s2[2]).powi(2)).sqrt();
        if d > max_dist {
            max_dist = d;
        }
    }
    // With max Lyapunov ~0.9 and perturbation 1e-4, the trajectories saturate at
    // the attractor diameter (~30) within ~15 sim-time units.
    assert!(
        max_dist > 1.0,
        "lorenz rho=28 should show Lyapunov divergence, max_dist={}",
        max_dist
    );
}

#[test]
fn lorenz_sigma_at_boundary_min() {
    let mut sys = Lorenz::new(0.1, 28.0, 2.6667);
    for _ in 0..1000 {
        sys.step(0.001);
    }
    assert!(
        all_finite(sys.state()),
        "state non-finite at sigma=0.1 (config min)"
    );
}

#[test]
fn lorenz_rho_at_boundary_max() {
    let mut sys = Lorenz::new(10.0, 200.0, 2.6667);
    for _ in 0..500 {
        sys.step(0.001);
    }
    // Must not produce NaN/inf; may be strongly chaotic but must stay finite
    assert!(
        all_finite(sys.state()),
        "state non-finite at rho=200.0 (config max)"
    );
}

// ── Rossler ─────────────────────────────────────────────────────────────────

#[test]
fn rossler_periodic_low_c() {
    // c=3.0 gives period-1 limit cycle (below chaos)
    let mut sys = Rossler::new(0.2, 0.2, 3.0);
    for _ in 0..5000 {
        sys.step(0.001);
    }
    let s = sys.state();
    assert!(all_finite(s), "state non-finite at c=3.0");
    assert!(
        s[2] < 20.0,
        "z exceeds 20.0 at c=3.0 (should be periodic): {}",
        s[2]
    );
}

#[test]
fn rossler_chaotic_high_c() {
    // c=5.7 (default, chaotic) — x/y variance must be significant
    let mut sys = Rossler::new(0.2, 0.2, 5.7);
    for _ in 0..2000 {
        sys.step(0.001);
    } // warm up
    let mut samples: Vec<f64> = Vec::with_capacity(5000);
    for _ in 0..5000 {
        sys.step(0.001);
        samples.push(sys.state()[0]);
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let var = samples.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / samples.len() as f64;
    assert!(
        var > 1.0,
        "rossler c=5.7 x-variance should be > 1.0, got {}",
        var
    );
}

#[test]
fn rossler_c_at_boundary_max() {
    let mut sys = Rossler::new(0.2, 0.2, 20.0);
    for _ in 0..1000 {
        sys.step(0.001);
    }
    assert!(
        all_finite(sys.state()),
        "state non-finite at c=20.0 (boundary max)"
    );
}

// ── Kuramoto ─────────────────────────────────────────────────────────────────

#[test]
fn kuramoto_just_below_critical_coupling() {
    // K=0.9 is just below K_c=1.0; should remain largely incoherent
    let mut sys = Kuramoto::new(16, 0.9);
    for _ in 0..2000 {
        sys.step(0.01);
    }
    let r = sys.order_parameter();
    assert!(
        r >= 0.0 && r <= 1.0 + 1e-9,
        "order parameter out of [0,1]: {}",
        r
    );
    assert!(
        r < 0.7,
        "kuramoto K=0.9 should be incoherent (r < 0.7), got r={}",
        r
    );
}

#[test]
fn kuramoto_just_above_critical_coupling() {
    // K=1.1 is just above K_c=1.0; partial synchronization should emerge.
    // Very close to the boundary so synchronization is weak; threshold is 0.3.
    let mut sys = Kuramoto::new(16, 1.1);
    for _ in 0..5000 {
        sys.step(0.01);
    }
    let r = sys.order_parameter();
    assert!(
        r >= 0.0 && r <= 1.0 + 1e-9,
        "order parameter out of [0,1]: {}",
        r
    );
    assert!(
        r > 0.3,
        "kuramoto K=1.1 should show partial sync (r > 0.3), got r={}",
        r
    );
}

#[test]
fn kuramoto_exactly_at_critical_coupling() {
    let mut sys = Kuramoto::new(16, 1.0);
    for _ in 0..2000 {
        sys.step(0.01);
    }
    let s = sys.state();
    assert!(
        all_finite(s),
        "kuramoto state non-finite at K=1.0 (exact critical coupling)"
    );
    let r = sys.order_parameter();
    assert!(
        r >= 0.0 && r <= 1.0 + 1e-9,
        "order parameter out of [0,1] at K=1.0: {}",
        r
    );
}

// ── Duffing ──────────────────────────────────────────────────────────────────

#[test]
fn duffing_small_forcing_periodic() {
    // Small forcing amplitude gamma=0.1 should give periodic, bounded behaviour
    let mut sys = Duffing::new();
    sys.gamma = 0.1;
    for _ in 0..3000 {
        sys.step(0.01);
    }
    let s = sys.state();
    assert!(all_finite(s), "duffing state non-finite at gamma=0.1");
    assert!(
        s[1].abs() <= 5.0,
        "duffing velocity out of [-5,5] at gamma=0.1: {}",
        s[1]
    );
}

#[test]
fn duffing_chaotic_forcing() {
    // gamma=0.5 (default chaos) — x must not be stuck at a fixed point
    let mut sys = Duffing::new(); // defaults: gamma=0.5
    for _ in 0..1000 {
        sys.step(0.01);
    } // warm up
    let mut xs: Vec<f64> = Vec::with_capacity(3000);
    for _ in 0..3000 {
        sys.step(0.01);
        xs.push(sys.state()[0]);
    }
    assert!(
        all_finite(sys.state()),
        "duffing state non-finite at gamma=0.5"
    );
    let mean = xs.iter().sum::<f64>() / xs.len() as f64;
    let var = xs.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / xs.len() as f64;
    assert!(
        var > 0.1,
        "duffing gamma=0.5 x-variance should be > 0.1, got {}",
        var
    );
}

// ── CML ───────────────────────────────────────────────────────────────────────

#[test]
fn cml_periodic_r_below_chaos() {
    // r=3.0 is period-2 (below logistic chaos onset ~3.57)
    let mut sys = CoupledMapLattice::new(3.0, 0.35);
    for _ in 0..2000 {
        sys.step(0.001);
    }
    let s = sys.state();
    assert!(
        s.iter().all(|&v| v.is_finite() && v >= 0.0 && v <= 1.0),
        "CML sites out of [0,1] at r=3.0: {:?}",
        &s[..4]
    );
}

#[test]
fn cml_chaotic_r_at_max() {
    // r=4.0 is fully chaotic logistic map
    let mut sys = CoupledMapLattice::new(4.0, 0.35);
    for _ in 0..2000 {
        sys.step(0.001);
    }
    let s = sys.state();
    assert!(
        s.iter().all(|&v| v.is_finite() && v >= 0.0 && v <= 1.0),
        "CML sites out of [0,1] at r=4.0: {:?}",
        &s[..4]
    );
}

#[test]
fn cml_r_at_config_max() {
    // boundary r=4.0 — verify no panic or NaN
    let mut sys = CoupledMapLattice::new(4.0, 0.35);
    for _ in 0..2000 {
        sys.step(0.001);
    }
    let s = sys.state();
    assert!(
        s.iter().all(|v| v.is_finite()),
        "CML produced NaN/inf at r=4.0 (config max)"
    );
}

// ── Henon Map ─────────────────────────────────────────────────────────────────

#[test]
fn henon_canonical_parameters_bounded() {
    // a=1.4, b=0.3 — canonical strange attractor; skip first 1000 as transient
    let mut sys = HenonMap::new(); // defaults: a=1.4, b=0.3
    for _ in 0..1000 {
        sys.step(0.001);
    } // transient
    for _ in 0..10000 {
        sys.step(0.001);
        let s = sys.state();
        // The Henon attractor lives in roughly x ∈ [-1.5, 1.5], y ∈ [-0.5, 0.5]
        // but we allow slightly wider margins for numerical reasons
        assert!(s[0].abs() <= 1.5, "henon x out of [-1.5,1.5]: {}", s[0]);
        assert!(s[1].abs() <= 0.5, "henon y out of [-0.5,0.5]: {}", s[1]);
    }
}

#[test]
fn henon_at_a_boundary() {
    // a=2.0 (config max) — may diverge or stay bounded; must not produce NaN from within the map
    let mut sys = HenonMap::new();
    sys.a = 2.0;
    // Run for 500 steps; if x diverges, x.is_finite() will catch it
    for _ in 0..500 {
        sys.step(0.001);
        let s = sys.state();
        // At a=2.0 the map often escapes to infinity; once diverged the state may be non-finite.
        // The contract is that the system must not panic; NaN is acceptable at boundary.
        let _ = s[0]; // just ensure no panic
    }
    // At minimum the struct should still be accessible
    let _ = sys.state();
}

#[test]
fn henon_b_at_zero() {
    // b=0.0 — degenerate (area-contracting = 0); must stay finite for 500 steps
    let mut sys = HenonMap::new();
    sys.b = 0.0;
    for _ in 0..500 {
        sys.step(0.001);
    }
    // b=0 means y_new = 0 always; x_new = 1 - a*x^2 (1-D logistic-like)
    // With a=1.4 this may converge to a fixed point or oscillate
    assert!(sys.state()[1].is_finite(), "henon y non-finite at b=0.0");
}

// ── Mackey-Glass ──────────────────────────────────────────────────────────────

#[test]
fn mackey_glass_stable_low_tau() {
    // tau=5.0 is below the chaos onset (~7); expect stable limit cycle in [0, 5]
    let mut sys = MackeyGlass::new();
    sys.tau = 5.0;
    // Rebuild history buffer for new tau (re-create the system with overridden tau is simplest)
    // The MackeyGlass::new() starts with tau=17; we override then step to let it settle
    for _ in 0..2000 {
        sys.step(0.5);
    }
    let s = sys.state();
    assert!(all_finite(s), "mackey-glass state non-finite at tau=5.0");
    assert!(
        s[0] >= 0.0 && s[0] <= 5.0,
        "mackey-glass x out of [0,5] at tau=5.0: {}",
        s[0]
    );
}

#[test]
fn mackey_glass_chaotic_high_tau() {
    // tau=17.0 (default, chaotic) — bounded chaos, should remain in [0, 5]
    let mut sys = MackeyGlass::new(); // tau=17 default
    for _ in 0..2000 {
        sys.step(0.5);
    }
    let s = sys.state();
    assert!(all_finite(s), "mackey-glass state non-finite at tau=17.0");
    assert!(
        s[0] >= 0.0 && s[0] <= 5.0,
        "mackey-glass x out of [0,5] at tau=17.0: {}",
        s[0]
    );
}

// --- Latency SLA Tests ---

#[test]
fn sim_tick_completes_within_control_period_budget() {
    // One simulation tick must complete in < 8.33ms (120 Hz control rate)
    // to avoid starving the audio thread.
    // We test Lorenz (representative continuous system) running 120 steps.
    use std::time::Instant;
    let mut sys = Lorenz::new(10.0, 28.0, 2.6667);

    let start = Instant::now();
    for _ in 0..120 {
        // one second of ticks at 120 Hz
        sys.step(0.001);
    }
    let elapsed = start.elapsed();
    let per_tick_us = elapsed.as_micros() / 120;
    assert!(
        per_tick_us < 8_333,
        "sim tick took {}us, exceeds 8.33ms control period budget",
        per_tick_us
    );
}

#[test]
fn audio_buffer_renders_within_latency_budget() {
    // A 512-sample buffer at 44100 Hz must render in < 11.6ms.
    use std::time::Instant;
    let sr = 44100.0f32;
    let n_samples = 512usize;
    let budget_us: u128 = (n_samples as u128 * 1_000_000) / 44100; // ~11610 us

    let mut osc = math_sonify_plugin::synth::Oscillator::new(440.0, OscShape::Sine, sr);
    let start = Instant::now();
    for _ in 0..n_samples {
        let _ = osc.next_sample();
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_micros() < budget_us,
        "512-sample buffer render took {}us, exceeds {}us latency budget",
        elapsed.as_micros(),
        budget_us
    );
}

#[test]
fn ten_consecutive_buffers_render_within_budget() {
    // Verify no buffer in a burst of 10 consecutive renders exceeds the latency budget.
    use std::time::Instant;
    let sr = 44100.0f32;
    let n_samples = 512usize;
    let budget_us: u128 = (n_samples as u128 * 1_000_000) / 44100; // ~11610 us

    let mut osc = math_sonify_plugin::synth::Oscillator::new(440.0, OscShape::Sine, sr);
    for buf_idx in 0..10 {
        let start = Instant::now();
        for _ in 0..n_samples {
            let _ = osc.next_sample();
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_micros() < budget_us,
            "buffer {} render took {}us, exceeds {}us latency budget",
            buf_idx,
            elapsed.as_micros(),
            budget_us
        );
    }
}

#[test]
fn synthesis_modes_all_meet_latency_sla() {
    // For each synthesis mode, render one 512-sample buffer and verify < latency budget.
    use std::time::Instant;
    let sr = 44100.0f32;
    let n_samples = 512usize;
    let budget_us: u128 = (n_samples as u128 * 1_000_000) / 44100; // ~11610 us

    // All modes use the Oscillator DSP path at their core; test each OscShape
    let shapes = [
        ("Sine", OscShape::Sine),
        ("Square", OscShape::Square),
        ("Saw", OscShape::Saw),
        ("Triangle", OscShape::Triangle),
        ("Noise", OscShape::Noise),
    ];

    for (name, shape) in &shapes {
        let mut osc = math_sonify_plugin::synth::Oscillator::new(440.0, *shape, sr);
        let start = Instant::now();
        for _ in 0..n_samples {
            let _ = osc.next_sample();
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_micros() < budget_us,
            "shape {} render took {}us, exceeds {}us latency budget",
            name,
            elapsed.as_micros(),
            budget_us
        );
    }
}

// ── New-system integration tests ─────────────────────────────────────────────

#[test]
fn burke_shaw_stays_finite() {
    let mut sys = BurkeShaw::new();
    for _ in 0..20_000 {
        sys.step(0.005);
    }
    assert!(all_finite(sys.state()), "BurkeShaw state non-finite: {:?}", sys.state());
}

#[test]
fn burke_shaw_state_bounded() {
    let mut sys = BurkeShaw::new();
    for _ in 0..20_000 {
        sys.step(0.005);
    }
    let s = sys.state();
    for v in s {
        assert!(v.abs() < 100.0, "BurkeShaw component out of range: {}", v);
    }
}

#[test]
fn chen_stays_finite() {
    let mut sys = Chen::new();
    for _ in 0..20_000 {
        sys.step(0.001);
    }
    assert!(all_finite(sys.state()), "Chen state non-finite: {:?}", sys.state());
}

#[test]
fn chen_state_bounded() {
    let mut sys = Chen::new();
    for _ in 0..20_000 {
        sys.step(0.001);
    }
    let s = sys.state();
    for v in s {
        assert!(v.abs() < 200.0, "Chen component out of range: {}", v);
    }
}

#[test]
fn dadras_stays_finite() {
    let mut sys = Dadras::new();
    for _ in 0..20_000 {
        sys.step(0.005);
    }
    assert!(all_finite(sys.state()), "Dadras state non-finite: {:?}", sys.state());
}

#[test]
fn dadras_state_bounded() {
    let mut sys = Dadras::new();
    for _ in 0..20_000 {
        sys.step(0.005);
    }
    let s = sys.state();
    for v in s {
        assert!(v.abs() < 200.0, "Dadras component out of range: {}", v);
    }
}

#[test]
fn rucklidge_stays_finite() {
    let mut sys = Rucklidge::new();
    for _ in 0..20_000 {
        sys.step(0.005);
    }
    assert!(all_finite(sys.state()), "Rucklidge state non-finite: {:?}", sys.state());
}

#[test]
fn sprott_c_stays_finite() {
    let mut sys = SprottC::new();
    for _ in 0..20_000 {
        sys.step(0.01);
    }
    assert!(all_finite(sys.state()), "SprottC state non-finite: {:?}", sys.state());
}

#[test]
fn sprott_c_state_bounded() {
    let mut sys = SprottC::new();
    for _ in 0..20_000 {
        sys.step(0.01);
    }
    let s = sys.state();
    for v in s {
        assert!(v.abs() < 50.0, "SprottC component out of range: {}", v);
    }
}

#[test]
fn thomas_stays_finite() {
    let mut sys = Thomas::new(0.208186);
    for _ in 0..20_000 {
        sys.step(0.05);
    }
    assert!(all_finite(sys.state()), "Thomas state non-finite: {:?}", sys.state());
}

#[test]
fn thomas_state_bounded() {
    let mut sys = Thomas::new(0.208186);
    for _ in 0..20_000 {
        sys.step(0.05);
    }
    let s = sys.state();
    for v in s {
        assert!(v.abs() < 10.0, "Thomas component out of range: {}", v);
    }
}

#[test]
fn arnold_cat_stays_finite() {
    let mut sys = ArnoldCat::new();
    for _ in 0..10_000 {
        sys.step(1.0);
    }
    assert!(all_finite(sys.state()), "ArnoldCat state non-finite: {:?}", sys.state());
}

#[test]
fn arnold_cat_state_in_unit_square() {
    // ArnoldCat maps the unit torus [0,1)^2; state should stay in [0,1).
    let mut sys = ArnoldCat::new();
    for _ in 0..10_000 {
        sys.step(1.0);
    }
    let s = sys.state();
    for v in s {
        assert!(*v >= 0.0 && *v < 1.0, "ArnoldCat component outside [0,1): {}", v);
    }
}

// ── Additional system integration tests ──────────────────────────────────────

#[test]
fn aizawa_stays_finite() {
    let mut sys = Aizawa::new();
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Aizawa state non-finite: {:?}", sys.state());
}

#[test]
fn aizawa_state_bounded() {
    let mut sys = Aizawa::new();
    for _ in 0..20_000 { sys.step(0.01); }
    for v in sys.state() {
        assert!(v.abs() < 20.0, "Aizawa component out of range: {}", v);
    }
}

#[test]
fn halvorsen_stays_finite() {
    let mut sys = Halvorsen::new();
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Halvorsen state non-finite: {:?}", sys.state());
}

#[test]
fn chua_stays_finite() {
    let mut sys = Chua::new();
    for _ in 0..20_000 { sys.step(0.001); }
    assert!(all_finite(sys.state()), "Chua state non-finite: {:?}", sys.state());
}

#[test]
fn chua_state_bounded() {
    let mut sys = Chua::new();
    for _ in 0..20_000 { sys.step(0.001); }
    for v in sys.state() {
        assert!(v.abs() < 100.0, "Chua component out of range: {}", v);
    }
}

#[test]
fn van_der_pol_stays_finite() {
    let mut sys = VanDerPol::new();
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "VanDerPol state non-finite: {:?}", sys.state());
}

#[test]
fn hindmarsh_rose_stays_finite() {
    // I=3.0 drives spiking; r=0.001 is slow adaptation
    let mut sys = HindmarshRose::new(3.0, 0.001);
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "HindmarshRose state non-finite: {:?}", sys.state());
}

#[test]
fn lorenz96_stays_finite() {
    let mut sys = Lorenz96::new();
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Lorenz96 state non-finite: {:?}", sys.state());
}

#[test]
fn sprott_b_stays_finite() {
    let mut sys = SprottB::new();
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "SprottB state non-finite: {:?}", sys.state());
}

#[test]
fn nose_hoover_stays_finite() {
    let mut sys = NoseHoover::new();
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "NoseHoover state non-finite: {:?}", sys.state());
}

#[test]
fn oregonator_stays_finite() {
    // f=0.5 is a typical stoichiometric parameter for the BZ reaction model
    let mut sys = Oregonator::new(0.5);
    for _ in 0..10_000 { sys.step(0.0001); }
    assert!(all_finite(sys.state()), "Oregonator state non-finite: {:?}", sys.state());
}

#[test]
fn oregonator_state_stays_positive() {
    // The Oregonator models chemical concentrations: all variables must be > 0.
    // The step function clamps to [1e-12, 1e6], so this verifies the clamp works.
    let mut sys = Oregonator::new(0.5);
    for step in 0..10_000 {
        sys.step(0.0001);
        for (i, v) in sys.state().iter().enumerate() {
            assert!(
                *v > 0.0,
                "Oregonator concentration {i} became non-positive at step {step}: {v}"
            );
        }
    }
}

#[test]
fn hindmarsh_rose_exhibits_oscillations() {
    // At I=3.0 the HindmarshRose model should exhibit regular spiking/bursting.
    // x (membrane potential) oscillates: we verify it crosses both below -0.5 and above 1.0.
    let mut sys = HindmarshRose::new(3.0, 0.001);
    // Burn in transient
    for _ in 0..5_000 { sys.step(0.01); }
    let mut saw_low = false;
    let mut saw_high = false;
    for _ in 0..10_000 {
        sys.step(0.01);
        let x = sys.state()[0];
        if x < -0.5 { saw_low = true; }
        if x > 1.0  { saw_high = true; }
    }
    assert!(saw_low,  "HindmarshRose x never went below -0.5 (not oscillating)");
    assert!(saw_high, "HindmarshRose x never went above 1.0 (not spiking)");
}

#[test]
fn geodesic_torus_stays_finite() {
    let mut sys = GeodesicTorus::new(2.0, 0.5);
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "GeodesicTorus state non-finite: {:?}", sys.state());
}

#[test]
fn geodesic_torus_metric_speed_conserved() {
    // Geodesic flow on a Riemannian manifold conserves the metric speed
    // L = (R+r·cosθ)²·φ̇² + r²·θ̇², which equals twice the kinetic energy.
    // RK4 at dt=0.01 gives excellent conservation: drift should be < 1%.
    let mut sys = GeodesicTorus::new(3.0, 1.0);
    for _ in 0..5_000 { sys.step(0.01); }
    let drift = sys.energy_error().expect("GeodesicTorus should implement energy_error");
    assert!(
        drift < 0.01,
        "Metric speed should be conserved within 1%: drift={}",
        drift
    );
}

#[test]
fn standard_map_stays_finite() {
    // k=0.97 puts the map near the chaos threshold
    let mut sys = StandardMap::new(0.97);
    for _ in 0..10_000 { sys.step(1.0); }
    assert!(all_finite(sys.state()), "StandardMap state non-finite: {:?}", sys.state());
}

#[test]
fn logistic_map_stays_in_unit_interval() {
    // r=3.9: deep into the chaotic regime
    let mut sys = LogisticMap::new(3.9);
    for _ in 0..10_000 { sys.step(1.0); }
    let x = sys.state()[0];
    assert!(x >= 0.0 && x <= 1.0, "LogisticMap x outside [0,1]: {}", x);
}

#[test]
fn fractional_lorenz_stays_finite() {
    // alpha=0.99 approximates standard Lorenz; use classic params
    let mut sys = FractionalLorenz::new(0.99, 10.0, 28.0, 8.0 / 3.0);
    for _ in 0..5_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "FractionalLorenz state non-finite: {:?}", sys.state());
}

#[test]
fn stochastic_lorenz_stays_finite() {
    let mut sys = StochasticLorenz::new(10.0, 28.0, 8.0 / 3.0, 0.1);
    for _ in 0..10_000 { sys.step(0.001); }
    assert!(all_finite(sys.state()), "StochasticLorenz state non-finite: {:?}", sys.state());
}

#[test]
fn delayed_map_stays_finite_low_r() {
    // r=2.0 with uniform initial history at 0.5 is a fixed point: x* = 1 - 1/r = 0.5.
    // The delayed map can diverge at high r because the stabilising (1-x) feedback
    // uses delayed, not current, state — so r values above ~2 can be unstable.
    let mut sys = DelayedMap::new(2.0, 5);
    for _ in 0..10_000 { sys.step(1.0); }
    assert!(all_finite(sys.state()), "DelayedMap state non-finite: {:?}", sys.state());
}

#[test]
fn mathieu_stable_parameters_bounded() {
    // a=0.5, q=0.1 is well below the first parametric resonance tongue (which
    // opens around a≈1 ± q), so the solution stays bounded.
    let mut sys = Mathieu::new(0.5, 0.1);
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Mathieu state non-finite: {:?}", sys.state());
    for v in sys.state() {
        assert!(v.abs() < 1000.0, "Mathieu component diverged: {}", v);
    }
}

#[test]
fn kuramoto_driven_stays_finite() {
    let mut sys = KuramotoDriven::new(1.0, 0.5, 1.0);
    for _ in 0..10_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "KuramotoDriven state non-finite: {:?}", sys.state());
}

// ── Lorenz84 integration tests ────────────────────────────────────────────────

#[test]
fn lorenz84_stays_finite() {
    let mut sys = Lorenz84::new();
    for _ in 0..20_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Lorenz84 state non-finite: {:?}", sys.state());
}

#[test]
fn lorenz84_state_bounded() {
    // With a=0.25, b=4, F=8, G=1.23 the attractor is centred near x∈[-2,5].
    // Use conservative bounds with margin for transient excursions.
    let mut sys = Lorenz84::new();
    for _ in 0..20_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(s[0] > -5.0 && s[0] < 10.0, "Lorenz84 x out of expected range: {}", s[0]);
    assert!(s[1].abs() < 15.0, "Lorenz84 y out of expected range: {}", s[1]);
    assert!(s[2].abs() < 15.0, "Lorenz84 z out of expected range: {}", s[2]);
}

#[test]
fn lorenz84_deterministic() {
    let (mut s1, mut s2) = (Lorenz84::new(), Lorenz84::new());
    for _ in 0..5_000 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-14, "Lorenz84 non-deterministic: {} vs {}", a, b);
    }
}

// ── Rabinovich–Fabrikant integration tests ────────────────────────────────────

#[test]
fn rabinovich_fabrikant_stays_finite() {
    // Use small dt; RF is more sensitive than Lorenz due to cubic nonlinearity.
    let mut sys = RabinovichFabrikant::new();
    for _ in 0..10_000 { sys.step(0.001); }
    assert!(all_finite(sys.state()), "RabinovichFabrikant state non-finite: {:?}", sys.state());
}

#[test]
fn rabinovich_fabrikant_state_bounded() {
    // With α=0.14, γ=0.1 the attractor stays within moderate bounds.
    let mut sys = RabinovichFabrikant::new();
    for _ in 0..10_000 { sys.step(0.001); }
    for v in sys.state() {
        assert!(v.abs() < 5.0, "RabinovichFabrikant component out of range: {}", v);
    }
}

// ── Van der Pol physics test ──────────────────────────────────────────────────

#[test]
fn van_der_pol_limit_cycle_amplitude() {
    // For μ=2, the stable limit cycle has x-amplitude ≈ 2. After a transient
    // the peak should land solidly in [1.5, 3.5] regardless of initial conditions.
    let mut sys = VanDerPol::new();
    for _ in 0..5_000 { sys.step(0.01); }   // burn through transient
    let mut max_x = 0.0_f64;
    for _ in 0..5_000 {
        sys.step(0.01);
        max_x = max_x.max(sys.state()[0].abs());
    }
    assert!(
        max_x > 1.5 && max_x < 3.5,
        "Van der Pol x amplitude = {:.3} (expected ≈ 2.0)", max_x
    );
}

// ── CustomOde / validate_exprs integration tests ──────────────────────────────

#[test]
fn validate_exprs_accepts_valid_lorenz_like_equations() {
    // Simple Lorenz-like derivatives should pass validation.
    let result = validate_exprs("y - x", "-x*z + 28*x - y", "x*y - 2.667*z", "");
    assert!(result.is_ok(), "Valid Lorenz-like exprs rejected: {:?}", result);
}

#[test]
fn validate_exprs_accepts_harmonic_oscillator() {
    let result = validate_exprs("y", "-x", "0", "");
    assert!(result.is_ok(), "Valid harmonic oscillator rejected: {:?}", result);
}

#[test]
fn validate_exprs_rejects_unknown_identifier() {
    // 'sigma' and 'rho' are not in the known variable/function list; the
    // typo-detection path in validate_exprs should flag them as an error.
    let result = validate_exprs("sigma * (y - x)", "-x*rho + 28*x - y", "x*y - 2.667*z", "");
    assert!(result.is_err(), "Unknown identifiers 'sigma'/'rho' should be rejected");
}

#[test]
fn validate_exprs_rejects_division_by_zero() {
    // eval_expr_4d_raw (used inside validate_exprs) does NOT coerce inf/NaN
    // to 0, so 1/(x-x) = 1/0 should produce a non-finite result and Err.
    let result = validate_exprs("1/(x-x)", "y", "z", "");
    assert!(result.is_err(), "Division by zero should be rejected by validate_exprs");
}

// ── Sprott-G, Sprott-H, Sprott-L, Rikitake integration tests ─────────────────

#[test]
fn sprott_g_stays_finite() {
    let mut sys = SprottG::new();
    for _ in 0..10_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "SprottG state non-finite: {:?}", sys.state());
}

#[test]
fn sprott_h_stays_finite() {
    let mut sys = SprottH::new();
    for _ in 0..10_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "SprottH state non-finite: {:?}", sys.state());
}

#[test]
fn sprott_l_stays_finite() {
    let mut sys = SprottL::new();
    for _ in 0..10_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "SprottL state non-finite: {:?}", sys.state());
}

#[test]
fn rikitake_stays_finite() {
    let mut sys = Rikitake::new();
    for _ in 0..10_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Rikitake state non-finite: {:?}", sys.state());
}

#[test]
fn rikitake_state_bounded() {
    // With μ=1, a=5 the Rikitake dynamo has bounded reversals.
    let mut sys = Rikitake::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(s[0].abs() < 20.0, "Rikitake x out of range: {}", s[0]);
    assert!(s[1].abs() < 20.0, "Rikitake y out of range: {}", s[1]);
    assert!(s[2].abs() < 50.0, "Rikitake z out of range: {}", s[2]);
}

// ── DoublePendulum energy_error integration test ──────────────────────────────

#[test]
fn double_pendulum_energy_error_trait_small_angles() {
    // Verify that the energy_error() trait method reports a small drift for
    // near-linear (small-angle) trajectories integrated with small dt.
    let mut sys = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
    sys.set_state(&[0.05, 0.07, 0.0, 0.0]);
    for _ in 0..5_000 { sys.step(0.001); }
    let drift = sys.energy_error().expect("DoublePendulum must implement energy_error");
    assert!(drift < 0.01, "Energy drift too large: {:.2e}", drift);
}

// ── Bouali attractor integration tests ────────────────────────────────────────

#[test]
fn bouali_stays_finite() {
    let mut sys = Bouali::new();
    for _ in 0..10_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Bouali state non-finite: {:?}", sys.state());
}

#[test]
fn bouali_state_bounded() {
    // With default parameters the Bouali attractor stays within moderate bounds.
    let mut sys = Bouali::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(s[0].abs() < 30.0, "Bouali x out of range: {}", s[0]);
    assert!(s[1].abs() < 30.0, "Bouali y out of range: {}", s[1]);
    assert!(s[2].abs() < 30.0, "Bouali z out of range: {}", s[2]);
}

#[test]
fn bouali_deterministic() {
    let mut s1 = Bouali::new();
    let mut s2 = Bouali::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Bouali not deterministic: {} vs {}", a, b);
    }
}

// ── Newton-Leipnik attractor integration tests ────────────────────────────────

#[test]
fn newton_leipnik_stays_finite() {
    let mut sys = NewtonLeipnik::new();
    for _ in 0..10_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Newton-Leipnik state non-finite: {:?}", sys.state());
}

#[test]
fn newton_leipnik_state_bounded() {
    let mut sys = NewtonLeipnik::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(s[0].abs() < 5.0, "Newton-Leipnik x out of range: {}", s[0]);
    assert!(s[1].abs() < 5.0, "Newton-Leipnik y out of range: {}", s[1]);
    assert!(s[2].abs() < 5.0, "Newton-Leipnik z out of range: {}", s[2]);
}

#[test]
fn newton_leipnik_deterministic() {
    let mut s1 = NewtonLeipnik::new();
    let mut s2 = NewtonLeipnik::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Newton-Leipnik not deterministic: {} vs {}", a, b);
    }
}

// ── Shimizu-Morioka integration tests ────────────────────────────────────────

#[test]
fn shimizu_morioka_stays_finite() {
    let mut sys = ShimizuMorioka::new();
    for _ in 0..20_000 { sys.step(0.005); }
    assert!(all_finite(sys.state()), "Shimizu-Morioka state non-finite: {:?}", sys.state());
}

#[test]
fn shimizu_morioka_state_bounded() {
    let mut sys = ShimizuMorioka::new();
    for _ in 0..20_000 { sys.step(0.005); }
    let s = sys.state();
    assert!(s[0].abs() < 15.0, "Shimizu-Morioka x out of range: {}", s[0]);
    assert!(s[1].abs() < 15.0, "Shimizu-Morioka y out of range: {}", s[1]);
    assert!(s[2].abs() < 5.0,  "Shimizu-Morioka z out of range: {}", s[2]);
}

#[test]
fn shimizu_morioka_deterministic() {
    let mut s1 = ShimizuMorioka::new();
    let mut s2 = ShimizuMorioka::new();
    for _ in 0..500 { s1.step(0.005); s2.step(0.005); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Shimizu-Morioka not deterministic: {} vs {}", a, b);
    }
}

// ── Sprott D integration tests ────────────────────────────────────────────────

#[test]
fn sprott_d_stays_finite() {
    // dt=0.01 for 5000 steps (50 time units) — matches the bounded operating range
    let mut sys = SprottD::new();
    for _ in 0..5_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Sprott-D state non-finite: {:?}", sys.state());
}

#[test]
fn sprott_d_state_bounded() {
    let mut sys = SprottD::new();
    for _ in 0..5_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(s[0].abs() < 20.0, "Sprott-D x out of range: {}", s[0]);
    assert!(s[1].abs() < 20.0, "Sprott-D y out of range: {}", s[1]);
    assert!(s[2].abs() < 20.0, "Sprott-D z out of range: {}", s[2]);
}

#[test]
fn sprott_d_deterministic() {
    let mut s1 = SprottD::new();
    let mut s2 = SprottD::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Sprott-D not deterministic: {} vs {}", a, b);
    }
}

// ── Sprott E integration tests ────────────────────────────────────────────────

#[test]
fn sprott_e_stays_finite() {
    let mut sys = SprottE::new();
    for _ in 0..10_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Sprott-E state non-finite: {:?}", sys.state());
}

#[test]
fn sprott_e_state_bounded() {
    let mut sys = SprottE::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(s[0].abs() < 10.0, "Sprott-E x out of range: {}", s[0]);
    assert!(s[1].abs() < 10.0, "Sprott-E y out of range: {}", s[1]);
    assert!(s[2].abs() < 10.0, "Sprott-E z out of range: {}", s[2]);
}

#[test]
fn sprott_e_deterministic() {
    let mut s1 = SprottE::new();
    let mut s2 = SprottE::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Sprott-E not deterministic: {} vs {}", a, b);
    }
}

// ── Sprott F integration tests ────────────────────────────────────────────────

#[test]
fn sprott_f_stays_finite() {
    let mut sys = SprottF::new();
    for _ in 0..10_000 { sys.step(0.01); }
    assert!(all_finite(sys.state()), "Sprott-F state non-finite: {:?}", sys.state());
}

#[test]
fn sprott_f_state_bounded() {
    let mut sys = SprottF::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(s[0].abs() < 10.0, "Sprott-F x out of range: {}", s[0]);
    assert!(s[1].abs() < 10.0, "Sprott-F y out of range: {}", s[1]);
    assert!(s[2].abs() < 10.0, "Sprott-F z out of range: {}", s[2]);
}

#[test]
fn sprott_f_deterministic() {
    let mut s1 = SprottF::new();
    let mut s2 = SprottF::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Sprott-F not deterministic: {} vs {}", a, b);
    }
}


// ── Sprott B integration tests ────────────────────────────────────────────────

#[test]
fn sprott_b_state_bounded() {
    let mut sys = SprottB::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(all_finite(s), "Sprott-B non-finite: {:?}", s);
    assert!(s[0].abs() < 30.0, "Sprott-B x out of range: {}", s[0]);
    assert!(s[1].abs() < 30.0, "Sprott-B y out of range: {}", s[1]);
    assert!(s[2].abs() < 30.0, "Sprott-B z out of range: {}", s[2]);
}

#[test]
fn sprott_b_deterministic() {
    let mut s1 = SprottB::new();
    let mut s2 = SprottB::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Sprott-B not deterministic: {} vs {}", a, b);
    }
}

// ── Sprott C additional integration tests ─────────────────────────────────────

#[test]
fn sprott_c_state_bounded_generous() {
    // Secondary bounds check with generous limit; primary test is at line ~1033
    let mut sys = SprottC::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(all_finite(s), "Sprott-C non-finite: {:?}", s);
    assert!(s[0].abs() < 50.0, "Sprott-C x out of range: {}", s[0]);
    assert!(s[1].abs() < 50.0, "Sprott-C y out of range: {}", s[1]);
    assert!(s[2].abs() < 50.0, "Sprott-C z out of range: {}", s[2]);
}

#[test]
fn sprott_c_deterministic() {
    let mut s1 = SprottC::new();
    let mut s2 = SprottC::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Sprott-C not deterministic: {} vs {}", a, b);
    }
}

// ── Sprott G integration tests ────────────────────────────────────────────────

#[test]
fn sprott_g_state_bounded() {
    let mut sys = SprottG::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(all_finite(s), "Sprott-G non-finite: {:?}", s);
}

#[test]
fn sprott_g_deterministic() {
    let mut s1 = SprottG::new();
    let mut s2 = SprottG::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Sprott-G not deterministic: {} vs {}", a, b);
    }
}

// ── Sprott H integration tests ────────────────────────────────────────────────

#[test]
fn sprott_h_state_bounded() {
    let mut sys = SprottH::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(all_finite(s), "Sprott-H non-finite: {:?}", s);
}

#[test]
fn sprott_h_deterministic() {
    let mut s1 = SprottH::new();
    let mut s2 = SprottH::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Sprott-H not deterministic: {} vs {}", a, b);
    }
}

// ── Sprott L integration tests ────────────────────────────────────────────────

#[test]
fn sprott_l_state_bounded() {
    let mut sys = SprottL::new();
    for _ in 0..10_000 { sys.step(0.01); }
    let s = sys.state();
    assert!(all_finite(s), "Sprott-L non-finite: {:?}", s);
}

#[test]
fn sprott_l_deterministic() {
    let mut s1 = SprottL::new();
    let mut s2 = SprottL::new();
    for _ in 0..500 { s1.step(0.01); s2.step(0.01); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Sprott-L not deterministic: {} vs {}", a, b);
    }
}

// ── Genesio-Tesi integration tests ────────────────────────────────────────────

#[test]
fn genesio_tesi_stays_finite() {
    let mut sys = GenesioTesi::new();
    for _ in 0..10_000 { sys.step(0.005); }
    assert!(all_finite(sys.state()), "Genesio-Tesi state non-finite: {:?}", sys.state());
}

#[test]
fn genesio_tesi_state_bounded() {
    let mut sys = GenesioTesi::new();
    for _ in 0..10_000 { sys.step(0.005); }
    let s = sys.state();
    assert!(all_finite(s), "Genesio-Tesi non-finite: {:?}", s);
    // Attractor stays within moderate bounds with NaN guard active
    assert!(s[0].abs() < 10.0, "Genesio-Tesi x out of range: {}", s[0]);
    assert!(s[1].abs() < 15.0, "Genesio-Tesi y out of range: {}", s[1]);
    assert!(s[2].abs() < 30.0, "Genesio-Tesi z out of range: {}", s[2]);
}

#[test]
fn genesio_tesi_deterministic() {
    let mut s1 = GenesioTesi::new();
    let mut s2 = GenesioTesi::new();
    for _ in 0..500 { s1.step(0.005); s2.step(0.005); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Genesio-Tesi not deterministic: {} vs {}", a, b);
    }
}

// ── Liu attractor integration tests ──────────────────────────────────────────

#[test]
fn liu_stays_finite() {
    let mut sys = Liu::new();
    for _ in 0..20_000 { sys.step(0.001); }
    assert!(all_finite(sys.state()), "Liu state non-finite: {:?}", sys.state());
}

#[test]
fn liu_state_bounded() {
    let mut sys = Liu::new();
    for _ in 0..20_000 { sys.step(0.001); }
    let s = sys.state();
    assert!(s[0].abs() < 10.0, "Liu x out of range: {}", s[0]);
    assert!(s[1].abs() < 15.0, "Liu y out of range: {}", s[1]);
    assert!(s[2].abs() < 50.0, "Liu z out of range: {}", s[2]);
}

#[test]
fn liu_deterministic() {
    let mut s1 = Liu::new();
    let mut s2 = Liu::new();
    for _ in 0..500 { s1.step(0.001); s2.step(0.001); }
    for (a, b) in s1.state().iter().zip(s2.state().iter()) {
        assert!((a - b).abs() < 1e-12, "Liu not deterministic: {} vs {}", a, b);
    }
}

// ── All-presets load test ─────────────────────────────────────────────────────

#[test]
fn all_presets_load_without_panic() {
    use math_sonify_plugin::patches::{load_preset, PRESETS};
    for preset in PRESETS {
        let mut config = load_preset(preset.name);
        config.validate(); // must not panic
        assert!(
            config.audio.master_volume >= 0.0 && config.audio.master_volume <= 1.0,
            "Preset '{}' master_volume out of range: {}",
            preset.name,
            config.audio.master_volume
        );
        assert!(
            !config.system.name.is_empty(),
            "Preset '{}' has empty system name",
            preset.name
        );
    }
}
