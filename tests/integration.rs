/// Integration tests for math-sonify.
///
/// These tests exercise the public library API end-to-end — the same interface
/// that external crates (or the binary) consume.  They run in a separate
/// compilation unit so they can only use `pub` items, mirroring a real
/// downstream consumer.
///
/// Run with:
///   cargo test --tests -- --test-thread=1
///
/// Individual groups can be filtered:
///   cargo test --tests lorenz_stays_on_attractor
///   cargo test --tests energy_conservation
///   cargo test --tests resonance
///   cargo test --tests polyphony

use math_sonify_plugin::{
    config::{Config, SonificationConfig},
    systems::{DynamicalSystem, Lorenz, Rossler, DoublePendulum, Kuramoto, ThreeBody},
    sonification::{
        Scale, AudioParams, DirectMapping, Sonification, quantize_to_scale,
    },
};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn all_finite(s: &[f64]) -> bool {
    s.iter().all(|v| v.is_finite())
}

// ===========================================================================
// 1. Lorenz attractor: trajectory stays within the known attractor bounds
// ===========================================================================

/// After 50 000 steps with canonical parameters (sigma=10, rho=28, beta=8/3),
/// every state component must remain inside the empirically established
/// attractor bounding box: |x| < 30, |y| < 30, 0 < z < 60.
#[test]
fn lorenz_stays_on_attractor() {
    let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
    for _ in 0..50_000 {
        sys.step(0.001);
    }
    let s = sys.state();
    assert!(all_finite(s), "State contains NaN/Inf: {:?}", s);
    assert!(s[0].abs() < 30.0, "x out of attractor bounds: {}", s[0]);
    assert!(s[1].abs() < 30.0, "y out of attractor bounds: {}", s[1]);
    assert!(s[2] > 0.0 && s[2] < 60.0, "z out of attractor bounds: {}", s[2]);
}

/// After transient decay the Lorenz z-component must remain strictly positive.
#[test]
fn lorenz_z_stays_positive() {
    let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
    for _ in 0..5_000 { sys.step(0.001); } // burn-in to reach attractor
    for _ in 0..20_000 {
        sys.step(0.001);
        let z = sys.state()[2];
        assert!(z > 0.0, "Lorenz z became non-positive ({}); attractor property violated", z);
    }
}

// ===========================================================================
// 2. Rossler: periodicity / boundedness properties
// ===========================================================================

/// With a=0.2, b=0.2, c=5.7 the trajectory must stay within
/// |x|, |y| < 15, 0 < z < 25.
#[test]
fn rossler_stays_bounded() {
    let mut sys = Rossler::new(0.2, 0.2, 5.7);
    for _ in 0..30_000 { sys.step(0.001); }
    let s = sys.state();
    assert!(all_finite(s), "Rossler state is non-finite: {:?}", s);
    assert!(s[0].abs() < 15.0, "Rossler x out of bounds: {}", s[0]);
    assert!(s[1].abs() < 15.0, "Rossler y out of bounds: {}", s[1]);
    assert!(s[2] > 0.0 && s[2] < 25.0, "Rossler z out of bounds: {}", s[2]);
}

/// The z-component stays positive in the near-periodic Rossler orbit.
#[test]
fn rossler_z_stays_positive() {
    let mut sys = Rossler::new(0.2, 0.2, 5.7);
    for _ in 0..5_000 { sys.step(0.001); }
    for _ in 0..10_000 {
        sys.step(0.001);
        let z = sys.state()[2];
        assert!(z > 0.0, "Rossler z became non-positive: {}", z);
    }
}

// ===========================================================================
// 3. Double pendulum: energy conservation at small angles
// ===========================================================================

/// With the Yoshida 4th-order symplectic integrator, the total mechanical
/// energy of the double pendulum started from small angles should drift by
/// less than 1% over 10 000 steps (dt = 0.001).
#[test]
fn double_pendulum_energy_conserved_small_angles() {
    let m1 = 1.0_f64;
    let m2 = 1.0_f64;
    let l1 = 1.0_f64;
    let l2 = 1.0_f64;
    let g  = 9.81_f64;

    let mut sys = DoublePendulum::new(m1, m2, l1, l2);
    // Near vertical-down: θ1=θ2=0.1 rad, momenta zero
    sys.set_state(&[0.1_f64, 0.1, 0.0, 0.0]);

    // Exact Hamiltonian for the double pendulum in canonical coordinates
    let hamiltonian = |s: &[f64]| -> f64 {
        let (th1, th2, p1, p2) = (s[0], s[1], s[2], s[3]);
        let delta = th2 - th1;
        let denom = (m1 + m2 - m2 * delta.cos().powi(2)).max(1e-12);
        let t = (
            (m1 + m2) * l2.powi(2) * p1.powi(2)
            + m2 * l1.powi(2) * p2.powi(2)
            - 2.0 * m2 * l1 * l2 * p1 * p2 * delta.cos()
        ) / (2.0 * m1 * m2 * l1.powi(2) * l2.powi(2) * denom);
        let v = -(m1 + m2) * g * l1 * th1.cos() - m2 * g * l2 * th2.cos();
        t + v
    };

    let e0 = hamiltonian(sys.state());
    for _ in 0..10_000 { sys.step(0.001); }
    let e1 = hamiltonian(sys.state());
    let rel_error = ((e1 - e0) / e0.abs()).abs();
    assert!(
        rel_error < 0.01,
        "Energy drift too large: {:.6} -> {:.6} (rel error {:.4})",
        e0, e1, rel_error
    );
}

/// At small angles the double pendulum stays near the equilibrium.
#[test]
fn double_pendulum_small_angle_stays_bounded() {
    let mut sys = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
    sys.set_state(&[0.05_f64, 0.05, 0.0, 0.0]);
    for _ in 0..10_000 {
        sys.step(0.001);
        let s = sys.state();
        assert!(all_finite(s), "State went non-finite: {:?}", s);
        assert!(s[0].abs() < 0.5, "theta1 escaped small-angle regime: {}", s[0]);
        assert!(s[1].abs() < 0.5, "theta2 escaped small-angle regime: {}", s[1]);
    }
}

// ===========================================================================
// 4. Coupled oscillators (Kuramoto): resonance / synchronization
// ===========================================================================

/// K_c = 2*gamma = 1.0 for the Lorentzian frequency distribution used.
/// Below K_c the order parameter r should stay low (incoherent).
#[test]
fn kuramoto_below_critical_coupling_stays_incoherent() {
    let mut sys = Kuramoto::new(16, 0.1); // K=0.1, far below K_c=1.0
    for _ in 0..20_000 { sys.step(0.01); }
    let r = sys.order_parameter();
    assert!(r < 0.5, "Expected incoherence below K_c, got r={:.4}", r);
}

/// Well above K_c the order parameter should approach 1.
#[test]
fn kuramoto_above_critical_coupling_synchronizes() {
    let mut sys = Kuramoto::new(16, 5.0); // K=5.0, well above K_c=1.0
    for _ in 0..50_000 { sys.step(0.01); }
    let r = sys.order_parameter();
    assert!(r > 0.5, "Expected synchronization above K_c, got r={:.4}", r);
}

/// The order parameter must always lie in [0, 1].
#[test]
fn kuramoto_order_parameter_always_in_unit_interval() {
    for &k in &[0.0_f64, 0.5, 1.0, 2.0, 10.0, 50.0] {
        let mut sys = Kuramoto::new(8, k);
        for _ in 0..5_000 { sys.step(0.01); }
        let r = sys.order_parameter();
        assert!(r >= 0.0 && r <= 1.0 + 1e-9,
            "Order parameter out of [0,1] at K={}: {}", k, r);
    }
}

// ===========================================================================
// 5. Three-Body: energy conservation with leapfrog integrator
// ===========================================================================

/// The three-body leapfrog integrator is symplectic; energy should be
/// conserved to within 1% over 10 000 steps at dt=0.001.
#[test]
fn three_body_energy_conserved() {
    let mut sys = ThreeBody::new([1.0, 1.0, 1.0]);
    for _ in 0..10_000 { sys.step(0.001); }
    let err = sys.energy_error;
    assert!(err < 0.01, "Three-body energy error exceeded 1%: {:.4}", err);
}

// ===========================================================================
// 6. Audio mapping: frequencies stay in valid MIDI / audible range
// ===========================================================================

/// All quantized frequencies must fall within the audible range [20, 22050] Hz.
#[test]
fn quantize_to_scale_always_audible_range() {
    let base = 220.0_f32;
    let oct  = 4.0_f32;
    for &scale in &[
        Scale::Pentatonic, Scale::Chromatic, Scale::JustIntonation,
        Scale::Microtonal, Scale::Edo19, Scale::Edo31, Scale::Edo24,
        Scale::WholeTone,  Scale::Phrygian, Scale::Lydian,
    ] {
        for i in 0..=200 {
            let t = i as f32 / 200.0;
            let f = quantize_to_scale(t, base, oct, scale);
            assert!(
                f >= 20.0 && f <= 22_050.0,
                "Scale {:?} t={:.3} produced freq outside audible range: {:.2} Hz",
                scale, t, f
            );
        }
    }
}

/// Frequencies must correspond to MIDI notes in [0, 127].
#[test]
fn quantize_to_scale_produces_valid_midi_range() {
    let base = 110.0_f32; // A2
    let oct  = 3.0_f32;
    for &scale in &[Scale::Pentatonic, Scale::Chromatic, Scale::Lydian] {
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let f = quantize_to_scale(t, base, oct, scale);
            let midi = 69.0_f32 + 12.0 * (f / 440.0).log2();
            assert!(
                midi >= 0.0 && midi <= 127.0,
                "Scale {:?} t={:.3}: freq {:.2} Hz -> MIDI {:.1} out of range",
                scale, t, f, midi
            );
        }
    }
}

// ===========================================================================
// 7. Polyphony limits: AudioParams voice array bounds
// ===========================================================================

/// DirectMapping must produce exactly 4 voice slots; voices beyond the state
/// dimension must have amplitude 0.
#[test]
fn polyphony_limit_at_most_four_voices() {
    let mut mapper = DirectMapping::new();
    let cfg = SonificationConfig::default();

    let state_3d = vec![1.2_f64, -3.1, 14.7];
    let params = mapper.map(&state_3d, 5.0, &cfg);
    assert_eq!(params.freqs.len(), 4, "AudioParams must have exactly 4 frequency slots");
    assert_eq!(params.amps.len(),  4, "AudioParams must have exactly 4 amplitude slots");

    // 1D state: only voice 0 active; voices 1..3 must be zero-amplitude
    let state_1d = vec![0.5_f64];
    let params_1d = mapper.map(&state_1d, 1.0, &cfg);
    assert_eq!(params_1d.amps[1], 0.0, "Voice 1 should be 0 for 1-D state: {}", params_1d.amps[1]);
    assert_eq!(params_1d.amps[2], 0.0, "Voice 2 should be 0 for 1-D state: {}", params_1d.amps[2]);
    assert_eq!(params_1d.amps[3], 0.0, "Voice 3 should be 0 for 1-D state: {}", params_1d.amps[3]);
}

/// Default voice_levels must be in descending order.
#[test]
fn polyphony_voice_levels_descending() {
    let cfg = SonificationConfig::default();
    let vl = cfg.voice_levels;
    assert!(vl[0] >= vl[1], "voice_levels[0] < voice_levels[1]");
    assert!(vl[1] >= vl[2], "voice_levels[1] < voice_levels[2]");
    assert!(vl[2] >= vl[3], "voice_levels[2] < voice_levels[3]");
}

/// All voice frequencies and amplitudes must be finite and non-negative.
#[test]
fn polyphony_all_voices_finite_and_non_negative() {
    let mut mapper = DirectMapping::new();
    let cfg = SonificationConfig::default();
    let state = vec![5.0_f64, -10.0, 3.14, 0.5];
    for _ in 0..20 { mapper.map(&state, 2.0, &cfg); } // warm-up normalizer
    let params = mapper.map(&state, 2.0, &cfg);
    for i in 0..4 {
        assert!(params.freqs[i].is_finite() && params.freqs[i] >= 0.0,
            "Voice {} freq invalid: {}", i, params.freqs[i]);
        assert!(params.amps[i].is_finite() && params.amps[i] >= 0.0,
            "Voice {} amp invalid: {}", i, params.amps[i]);
    }
}

// ===========================================================================
// 8. Config parsing: invalid configs produce clear errors / sensible defaults
// ===========================================================================

/// An empty TOML string should parse to defaults without error.
#[test]
fn config_empty_toml_parses_to_defaults() {
    let cfg: Config = toml::from_str("").expect("Empty TOML should parse to defaults");
    let defaults = Config::default();
    assert_eq!(cfg.lorenz.sigma, defaults.lorenz.sigma);
    assert_eq!(cfg.audio.sample_rate, defaults.audio.sample_rate);
}

/// Out-of-range values must be clamped by validate().
#[test]
fn config_out_of_range_values_clamped() {
    let toml_src = r#"
        [lorenz]
        sigma = 99999.0
        rho   = -100.0
        beta  = 0.0

        [audio]
        sample_rate    = 1234
        reverb_wet     = 99.0
        delay_feedback = 5.0
        master_volume  = -1.0
    "#;
    let mut cfg: Config = toml::from_str(toml_src).expect("Should parse with wild values");
    cfg.validate();

    assert!(cfg.lorenz.sigma <= 100.0,   "sigma not clamped: {}", cfg.lorenz.sigma);
    assert!(cfg.lorenz.rho >= 0.1,       "rho not clamped: {}", cfg.lorenz.rho);
    assert!(cfg.lorenz.beta >= 0.01,     "beta not clamped: {}", cfg.lorenz.beta);
    assert!(cfg.audio.reverb_wet <= 1.0, "reverb_wet not clamped: {}", cfg.audio.reverb_wet);
    assert!(cfg.audio.delay_feedback <= 0.99, "delay_feedback not clamped: {}", cfg.audio.delay_feedback);
    assert!(cfg.audio.master_volume >= 0.0,   "master_volume not clamped: {}", cfg.audio.master_volume);
    assert!(
        cfg.audio.sample_rate == 44100 || cfg.audio.sample_rate == 48000,
        "invalid sample_rate not reset: {}", cfg.audio.sample_rate
    );
}

/// Unknown TOML sections must be silently ignored (no deny_unknown_fields).
#[test]
fn config_unknown_fields_ignored() {
    let toml_src = r#"
        [unknown_section]
        foo = "bar"

        [lorenz]
        sigma = 12.0
    "#;
    let result: Result<Config, _> = toml::from_str(toml_src);
    assert!(result.is_ok(), "Unknown TOML section caused error: {:?}", result.err());
    assert!((result.unwrap().lorenz.sigma - 12.0).abs() < 1e-9);
}
