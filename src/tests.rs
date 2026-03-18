#[cfg(test)]
mod tests {
    use crate::config::{Config, load_config};
    use crate::systems::{DynamicalSystem, Lorenz, Rossler, Duffing, Kuramoto};
    use crate::sonification::{Scale, quantize_to_scale, chord_intervals_for};
    use crate::synth::oscillator::{Oscillator, OscShape};

    // -------------------------------------------------------------------------
    // Dynamical system integration tests
    // -------------------------------------------------------------------------

    fn all_finite(state: &[f64]) -> bool {
        state.iter().all(|v| v.is_finite())
    }

    #[test]
    fn lorenz_stays_finite_after_1000_steps() {
        let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
        for _ in 0..1000 {
            sys.step(0.001);
        }
        assert!(all_finite(sys.state()), "Lorenz state contains NaN/Inf: {:?}", sys.state());
    }

    #[test]
    fn rossler_stays_finite_after_1000_steps() {
        let mut sys = Rossler::new(0.2, 0.2, 5.7);
        for _ in 0..1000 {
            sys.step(0.001);
        }
        assert!(all_finite(sys.state()), "Rossler state contains NaN/Inf: {:?}", sys.state());
    }

    #[test]
    fn duffing_stays_finite_after_1000_steps() {
        let mut sys = Duffing::new();
        for _ in 0..1000 {
            sys.step(0.001);
        }
        assert!(all_finite(sys.state()), "Duffing state contains NaN/Inf: {:?}", sys.state());
    }

    #[test]
    fn kuramoto_stays_finite_after_1000_steps() {
        let mut sys = Kuramoto::new(8, 1.5);
        for _ in 0..1000 {
            sys.step(0.001);
        }
        assert!(all_finite(sys.state()), "Kuramoto state contains NaN/Inf: {:?}", sys.state());
    }

    // -------------------------------------------------------------------------
    // Scale quantization tests
    // -------------------------------------------------------------------------

    #[test]
    fn scale_quantization_pentatonic_in_valid_range() {
        let base = 220.0_f32;
        let octave_range = 3.0_f32;
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let freq = quantize_to_scale(t, base, octave_range, Scale::Pentatonic);
            assert!(freq.is_finite() && freq > 0.0,
                "Pentatonic quantize produced invalid freq {} at t={}", freq, t);
            // Should be within the expected octave range above base
            let max_freq = base * 2.0_f32.powf(octave_range);
            assert!(freq >= base * 0.99 && freq <= max_freq * 1.01,
                "Pentatonic freq {} out of expected range [{}, {}] at t={}", freq, base, max_freq, t);
        }
    }

    #[test]
    fn scale_quantization_chromatic_in_valid_range() {
        let base = 440.0_f32;
        let octave_range = 2.0_f32;
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let freq = quantize_to_scale(t, base, octave_range, Scale::Chromatic);
            assert!(freq.is_finite() && freq > 0.0,
                "Chromatic quantize produced invalid freq {} at t={}", freq, t);
        }
    }

    #[test]
    fn scale_quantization_boundaries() {
        // t=0.0 should return base frequency
        let base = 220.0_f32;
        let f0 = quantize_to_scale(0.0, base, 3.0, Scale::Pentatonic);
        assert!((f0 - base).abs() < 0.01, "t=0 should return base freq {}, got {}", base, f0);

        // t < 0 and t > 1 should be clamped (no panic, finite result)
        let f_neg = quantize_to_scale(-1.0, base, 3.0, Scale::Pentatonic);
        let f_over = quantize_to_scale(2.0, base, 3.0, Scale::Pentatonic);
        assert!(f_neg.is_finite(), "t<0 should produce finite freq, got {}", f_neg);
        assert!(f_over.is_finite(), "t>1 should produce finite freq, got {}", f_over);
    }

    // -------------------------------------------------------------------------
    // Config serialization round-trip
    // -------------------------------------------------------------------------

    #[test]
    fn config_default_roundtrips_toml() {
        let original = Config::default();
        let serialized = toml::to_string(&original).expect("Config::default() should serialize to TOML");
        let deserialized: Config = toml::from_str(&serialized)
            .expect("Serialized default config should parse back without errors");

        // Spot-check a few fields
        assert_eq!(deserialized.lorenz.sigma, original.lorenz.sigma);
        assert_eq!(deserialized.lorenz.rho,   original.lorenz.rho);
        assert_eq!(deserialized.lorenz.beta,  original.lorenz.beta);
        assert_eq!(deserialized.audio.sample_rate,   original.audio.sample_rate);
        assert_eq!(deserialized.audio.master_volume, original.audio.master_volume);
        assert_eq!(deserialized.system.dt,    original.system.dt);
        assert_eq!(deserialized.rossler.a,    original.rossler.a);
        assert_eq!(deserialized.sonification.base_frequency, original.sonification.base_frequency);
        assert_eq!(deserialized.sonification.octave_range,   original.sonification.octave_range);
    }

    // -------------------------------------------------------------------------
    // Config validation clamping
    // -------------------------------------------------------------------------

    #[test]
    fn validate_clamps_out_of_range_values() {
        let mut cfg = Config::default();

        // Push values well outside bounds
        cfg.system.dt            = -1.0;
        cfg.system.speed         = 9999.0;
        cfg.lorenz.sigma         = 0.0;
        cfg.lorenz.rho           = 500.0;
        cfg.lorenz.beta          = -5.0;
        cfg.rossler.a            = 999.0;
        cfg.rossler.b            = -1.0;
        cfg.rossler.c            = 999.0;
        cfg.audio.reverb_wet     = 5.0;
        cfg.audio.delay_ms       = 0.0;
        cfg.audio.delay_feedback = 2.0;
        cfg.audio.master_volume  = -0.5;
        cfg.audio.sample_rate    = 22050; // unsupported rate
        cfg.sonification.base_frequency = 0.0;
        cfg.sonification.octave_range   = 100.0;
        cfg.sonification.portamento_ms  = -100.0;

        cfg.validate();

        assert!(cfg.system.dt >= 0.0001 && cfg.system.dt <= 0.1,
            "dt not clamped: {}", cfg.system.dt);
        assert!(cfg.system.speed >= 0.0 && cfg.system.speed <= 100.0,
            "speed not clamped: {}", cfg.system.speed);
        assert!(cfg.lorenz.sigma >= 0.1 && cfg.lorenz.sigma <= 100.0,
            "lorenz.sigma not clamped: {}", cfg.lorenz.sigma);
        assert!(cfg.lorenz.rho >= 0.1 && cfg.lorenz.rho <= 200.0,
            "lorenz.rho not clamped: {}", cfg.lorenz.rho);
        assert!(cfg.lorenz.beta >= 0.01 && cfg.lorenz.beta <= 20.0,
            "lorenz.beta not clamped: {}", cfg.lorenz.beta);
        assert!(cfg.rossler.a >= 0.0 && cfg.rossler.a <= 20.0,
            "rossler.a not clamped: {}", cfg.rossler.a);
        assert!(cfg.rossler.b >= 0.0 && cfg.rossler.b <= 20.0,
            "rossler.b not clamped: {}", cfg.rossler.b);
        assert!(cfg.rossler.c >= 0.0 && cfg.rossler.c <= 20.0,
            "rossler.c not clamped: {}", cfg.rossler.c);
        assert!(cfg.audio.reverb_wet >= 0.0 && cfg.audio.reverb_wet <= 1.0,
            "reverb_wet not clamped: {}", cfg.audio.reverb_wet);
        assert!(cfg.audio.delay_ms >= 1.0 && cfg.audio.delay_ms <= 5000.0,
            "delay_ms not clamped: {}", cfg.audio.delay_ms);
        assert!(cfg.audio.delay_feedback >= 0.0 && cfg.audio.delay_feedback <= 0.99,
            "delay_feedback not clamped: {}", cfg.audio.delay_feedback);
        assert!(cfg.audio.master_volume >= 0.0 && cfg.audio.master_volume <= 1.0,
            "master_volume not clamped: {}", cfg.audio.master_volume);
        assert!(cfg.audio.sample_rate == 44100 || cfg.audio.sample_rate == 48000,
            "invalid sample_rate not reset: {}", cfg.audio.sample_rate);
        assert!(cfg.sonification.base_frequency >= 20.0 && cfg.sonification.base_frequency <= 2000.0,
            "base_frequency not clamped: {}", cfg.sonification.base_frequency);
        assert!(cfg.sonification.octave_range >= 0.1 && cfg.sonification.octave_range <= 8.0,
            "octave_range not clamped: {}", cfg.sonification.octave_range);
        assert!(cfg.sonification.portamento_ms >= 1.0 && cfg.sonification.portamento_ms <= 5000.0,
            "portamento_ms not clamped: {}", cfg.sonification.portamento_ms);
    }

    #[test]
    fn validate_leaves_valid_values_unchanged() {
        let original = Config::default();
        let mut cfg = original.clone();
        cfg.validate();

        // Defaults are within bounds — they should be unchanged
        assert_eq!(cfg.lorenz.sigma, original.lorenz.sigma);
        assert_eq!(cfg.lorenz.rho,   original.lorenz.rho);
        assert_eq!(cfg.lorenz.beta,  original.lorenz.beta);
        assert_eq!(cfg.audio.sample_rate, original.audio.sample_rate);
        assert_eq!(cfg.system.dt, original.system.dt);
    }

    // -------------------------------------------------------------------------
    // load_config with corrupted / missing file falls back to defaults
    // -------------------------------------------------------------------------

    #[test]
    fn load_config_corrupted_file_returns_defaults() {
        let dir = std::env::temp_dir();
        let path = dir.join("math_sonify_test_corrupted_config.toml");
        std::fs::write(&path, b"this is not valid toml ][[[")
            .expect("Should be able to write temp file");

        let cfg = load_config(&path);
        let defaults = Config::default();

        // After loading a corrupted file the result must equal defaults
        // (and pass validation, so defaults themselves must already be valid).
        assert_eq!(cfg.lorenz.sigma, defaults.lorenz.sigma);
        assert_eq!(cfg.audio.sample_rate, defaults.audio.sample_rate);
        assert_eq!(cfg.system.dt, defaults.system.dt);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_config_missing_file_returns_defaults() {
        let path = std::path::Path::new("/this/path/does/not/exist/config.toml");
        let cfg = load_config(path);
        let defaults = Config::default();
        assert_eq!(cfg.lorenz.sigma, defaults.lorenz.sigma);
        assert_eq!(cfg.system.dt,    defaults.system.dt);
    }

    // -------------------------------------------------------------------------
    // lerp_config tests (Item 16)
    // -------------------------------------------------------------------------

    #[test]
    fn lerp_config_at_t0_equals_a() {
        use crate::arrangement::lerp_config;
        let a = Config::default();
        let mut b = Config::default();
        b.lorenz.sigma = 20.0;
        b.audio.master_volume = 0.9;
        b.sonification.base_frequency = 880.0;
        b.system.name = "rossler".into();

        let result = lerp_config(&a, &b, 0.0);
        assert!((result.lorenz.sigma - a.lorenz.sigma).abs() < 1e-9);
        assert!((result.audio.master_volume - a.audio.master_volume).abs() < 1e-6);
        assert!((result.sonification.base_frequency - a.sonification.base_frequency).abs() < 1e-6);
        assert_eq!(result.system.name, a.system.name);
    }

    #[test]
    fn lerp_config_at_t1_equals_b() {
        use crate::arrangement::lerp_config;
        let a = Config::default();
        let mut b = Config::default();
        b.lorenz.sigma = 20.0;
        b.audio.master_volume = 0.9;
        b.sonification.base_frequency = 880.0;
        b.system.name = "rossler".into();

        let result = lerp_config(&a, &b, 1.0);
        assert!((result.lorenz.sigma - b.lorenz.sigma).abs() < 1e-9);
        assert!((result.audio.master_volume - b.audio.master_volume).abs() < 1e-6);
        assert!((result.sonification.base_frequency - b.sonification.base_frequency).abs() < 1e-6);
        assert_eq!(result.system.name, b.system.name);
    }

    #[test]
    fn lerp_config_at_t_half_is_midpoint() {
        use crate::arrangement::lerp_config;
        let mut a = Config::default();
        let mut b = Config::default();
        a.audio.master_volume = 0.4;
        b.audio.master_volume = 0.8;
        a.lorenz.sigma = 10.0;
        b.lorenz.sigma = 20.0;

        let result = lerp_config(&a, &b, 0.5);
        // master_volume: midpoint of 0.4 and 0.8 = 0.6, clamped to >= 0.45 -> 0.6
        assert!((result.audio.master_volume - 0.6).abs() < 1e-5,
            "Expected ~0.6, got {}", result.audio.master_volume);
        assert!((result.lorenz.sigma - 15.0).abs() < 1e-9,
            "Expected sigma=15.0, got {}", result.lorenz.sigma);
    }

    #[test]
    fn lerp_config_volume_floor_clamp() {
        use crate::arrangement::lerp_config;
        // Both configs have very low volume -> lerp should still respect the 0.45 floor
        let mut a = Config::default();
        let mut b = Config::default();
        a.audio.master_volume = 0.3;
        b.audio.master_volume = 0.3;
        let result = lerp_config(&a, &b, 0.5);
        assert!(result.audio.master_volume >= 0.45,
            "Volume below floor: {}", result.audio.master_volume);
    }

    #[test]
    fn lerp_config_string_switches_at_half() {
        use crate::arrangement::lerp_config;
        let mut a = Config::default();
        let mut b = Config::default();
        a.system.name = "lorenz".into();
        b.system.name = "rossler".into();

        // At t < 0.5 should be "lorenz"
        let r0 = lerp_config(&a, &b, 0.3);
        assert_eq!(r0.system.name, "lorenz");

        // At t >= 0.5 should be "rossler"
        let r1 = lerp_config(&a, &b, 0.7);
        assert_eq!(r1.system.name, "rossler");
    }

    #[test]
    fn lerp_config_clamp_t_outside_01() {
        use crate::arrangement::lerp_config;
        let mut a = Config::default();
        let mut b = Config::default();
        a.lorenz.rho = 10.0;
        b.lorenz.rho = 30.0;
        // t < 0 should behave like t=0
        let r_neg = lerp_config(&a, &b, -0.5);
        assert!((r_neg.lorenz.rho - 10.0).abs() < 1e-9);
        // t > 1 should behave like t=1
        let r_over = lerp_config(&a, &b, 1.5);
        assert!((r_over.lorenz.rho - 30.0).abs() < 1e-9);
    }

    #[test]
    fn total_duration_sums_active_scenes() {
        use crate::arrangement::{Scene, total_duration};
        let mut s1 = Scene::empty(0);
        s1.active = true; s1.hold_secs = 20.0; s1.morph_secs = 0.0; // first scene: no morph
        let mut s2 = Scene::empty(1);
        s2.active = true; s2.hold_secs = 15.0; s2.morph_secs = 10.0;
        let mut s3 = Scene::empty(2);
        s3.active = false; // inactive — should not contribute
        s3.hold_secs = 100.0; s3.morph_secs = 50.0;

        let scenes = vec![s1, s2, s3];
        let dur = total_duration(&scenes);
        // Only s1 and s2 are active. s1: 0+20=20, s2: 10+15=25 => total=45
        assert!((dur - 45.0).abs() < 1e-5, "Expected 45.0, got {}", dur);
    }

    #[test]
    fn scene_at_returns_correct_phase() {
        use crate::arrangement::{Scene, scene_at};
        let mut s1 = Scene::empty(0);
        s1.active = true; s1.hold_secs = 10.0; s1.morph_secs = 0.0;
        let mut s2 = Scene::empty(1);
        s2.active = true; s2.hold_secs = 20.0; s2.morph_secs = 5.0;

        let scenes = vec![s1, s2];

        // t=5 is in s1's hold (5/10 = 0.5 of hold)
        if let Some((idx, is_morph, _frac)) = scene_at(&scenes, 5.0) {
            assert_eq!(idx, 0);
            assert!(!is_morph, "Should be holding at scene 0");
        } else {
            panic!("scene_at returned None at t=5");
        }

        // t=12 is in s2's morph (2/5 = 0.4 of morph), which starts at t=10
        if let Some((idx, is_morph, frac)) = scene_at(&scenes, 12.0) {
            assert_eq!(idx, 1, "Expected scene index 1");
            assert!(is_morph, "Should be morphing into scene 1");
            assert!((frac - 0.4).abs() < 1e-5, "Expected frac=0.4, got {}", frac);
        } else {
            panic!("scene_at returned None at t=12");
        }

        // t=16 is in s2's hold (1/20 = 0.05 of hold), which starts at t=15
        if let Some((idx, is_morph, _frac)) = scene_at(&scenes, 16.0) {
            assert_eq!(idx, 1);
            assert!(!is_morph, "Should be holding at scene 1");
        } else {
            panic!("scene_at returned None at t=16");
        }

        // t=36 (past end) should return None
        assert!(scene_at(&scenes, 36.0).is_none(), "Should return None past end");
    }

    // -------------------------------------------------------------------------
    // Custom ODE parser tests (Item 17)
    // -------------------------------------------------------------------------

    #[test]
    fn ode_parser_basic_arithmetic() {
        use crate::systems::custom_ode::eval_expr;
        assert!((eval_expr("x + y", 1.0, 2.0, 0.0, 0.0) - 3.0).abs() < 1e-12);
        assert!((eval_expr("x - y", 5.0, 3.0, 0.0, 0.0) - 2.0).abs() < 1e-12);
        assert!((eval_expr("x * y", 3.0, 4.0, 0.0, 0.0) - 12.0).abs() < 1e-12);
        assert!((eval_expr("x / y", 9.0, 3.0, 0.0, 0.0) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn ode_parser_power() {
        use crate::systems::custom_ode::eval_expr;
        assert!((eval_expr("x^2", 3.0, 0.0, 0.0, 0.0) - 9.0).abs() < 1e-12);
        assert!((eval_expr("x^3", 2.0, 0.0, 0.0, 0.0) - 8.0).abs() < 1e-12);
    }

    #[test]
    fn ode_parser_sum_of_squares() {
        use crate::systems::custom_ode::eval_expr;
        // 3^2 + 4^2 + 0^2 = 25
        let result = eval_expr("x^2 + y^2 + z^2", 3.0, 4.0, 0.0, 0.0);
        assert!((result - 25.0).abs() < 1e-12);
    }

    #[test]
    fn ode_parser_sin_at_zero() {
        use crate::systems::custom_ode::eval_expr;
        let result = eval_expr("sin(x)", 0.0, 0.0, 0.0, 0.0);
        assert!(result.abs() < 1e-12, "sin(0) should be 0, got {}", result);
    }

    #[test]
    fn ode_parser_cos_at_zero() {
        use crate::systems::custom_ode::eval_expr;
        let result = eval_expr("cos(x)", 0.0, 0.0, 0.0, 0.0);
        assert!((result - 1.0).abs() < 1e-12, "cos(0) should be 1, got {}", result);
    }

    #[test]
    fn ode_parser_exp_at_zero() {
        use crate::systems::custom_ode::eval_expr;
        let result = eval_expr("exp(x)", 0.0, 0.0, 0.0, 0.0);
        assert!((result - 1.0).abs() < 1e-12, "exp(0) should be 1, got {}", result);
    }

    #[test]
    fn ode_parser_division_by_zero_returns_zero() {
        use crate::systems::custom_ode::eval_expr;
        // The parser guards division by near-zero values
        let result = eval_expr("1.0 / 0.0", 0.0, 0.0, 0.0, 0.0);
        assert!(result.is_finite(), "Division by zero should return finite value, got {}", result);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn ode_parser_lorenz_y_deriv() {
        use crate::systems::custom_ode::eval_expr;
        // Lorenz: dy/dt = x*(28.0 - z) - y   at (1,1,1) => 1*(27)-1 = 26
        let result = eval_expr("x * (28.0 - z) - y", 1.0, 1.0, 1.0, 0.0);
        assert!((result - 26.0).abs() < 1e-12, "Expected 26.0, got {}", result);
    }

    #[test]
    fn ode_parser_constants_pi_and_e() {
        use crate::systems::custom_ode::eval_expr;
        let pi = eval_expr("pi", 0.0, 0.0, 0.0, 0.0);
        assert!((pi - std::f64::consts::PI).abs() < 1e-12);
        let e = eval_expr("e", 0.0, 0.0, 0.0, 0.0);
        assert!((e - std::f64::consts::E).abs() < 1e-12);
    }

    #[test]
    fn ode_parser_unary_minus() {
        use crate::systems::custom_ode::eval_expr;
        assert!((eval_expr("-x", 5.0, 0.0, 0.0, 0.0) - (-5.0)).abs() < 1e-12);
        assert!((eval_expr("-x + y", 3.0, 1.0, 0.0, 0.0) - (-2.0)).abs() < 1e-12);
    }

    #[test]
    fn ode_parser_validate_lorenz_exprs() {
        use crate::systems::custom_ode::validate_exprs;
        let r = validate_exprs("10.0*(y-x)", "x*(28.0-z)-y", "x*y-2.667*z");
        assert!(r.is_ok(), "Valid Lorenz expressions should pass: {:?}", r);
    }

    #[test]
    fn ode_custom_ode_integration_stays_finite() {
        use crate::systems::{DynamicalSystem, custom_ode::CustomOde};
        let mut sys = CustomOde::new(
            "10.0*(y-x)".into(),
            "x*(28.0-z)-y".into(),
            "x*y-2.667*z".into(),
        );
        for _ in 0..100 {
            sys.step(0.001);
        }
        assert!(sys.state().iter().all(|v| v.is_finite()),
            "Custom ODE state went non-finite: {:?}", sys.state());
        let mag = sys.state().iter().map(|v| v * v).sum::<f64>().sqrt();
        assert!(mag < 1000.0, "Custom ODE magnitude too large: {}", mag);
    }

    // -------------------------------------------------------------------------
    // Additional scale quantization tests (Item 18)
    // -------------------------------------------------------------------------

    #[test]
    fn scale_quantization_t0_returns_base() {
        // t=0 always maps to the root (base frequency)
        for &scale in &[
            Scale::Pentatonic, Scale::Chromatic, Scale::JustIntonation, Scale::Microtonal,
            Scale::Edo19, Scale::Edo31, Scale::Edo24,
            Scale::WholeTone, Scale::Phrygian, Scale::Lydian,
        ] {
            let base = 220.0_f32;
            let f = quantize_to_scale(0.0, base, 3.0, scale);
            assert!((f - base).abs() < 0.01,
                "t=0 with {:?} should return base {}, got {}", scale, base, f);
        }
    }

    #[test]
    fn scale_quantization_microtonal_quarter_tones() {
        // Microtonal scale has 13 intervals per octave with 0.75 semitone steps.
        // The second degree (interval 1/13 of the way through a 1-octave range) should
        // be 0.75 semitones above base.
        let base = 220.0_f32;
        // With octave_range=1.0 and 13 intervals, t=1/13 maps to degree 1 (0.75 semitones)
        let t = 1.0 / 13.0;
        let f = quantize_to_scale(t, base, 1.0, Scale::Microtonal);
        let expected = base * 2.0_f32.powf(0.75 / 12.0);
        assert!((f - expected).abs() < 0.5,
            "Microtonal second degree: expected {:.2} Hz, got {:.2} Hz", expected, f);
    }

    #[test]
    fn scale_quantization_never_below_base() {
        // For all scales and valid t values, frequency should never be below base
        for &scale in &[Scale::Pentatonic, Scale::Chromatic, Scale::JustIntonation, Scale::Microtonal] {
            for i in 0..=100 {
                let t = i as f32 / 100.0;
                let f = quantize_to_scale(t, 220.0, 2.0, scale);
                assert!(f >= 219.9, "Freq below base at t={} with {:?}: {}", t, scale, f);
            }
        }
    }

    // ── EDO and modal scale tests (#3) ────────────────────────────────────────

    #[test]
    fn edo19_t0_returns_base() {
        let base = 440.0_f32;
        let f = quantize_to_scale(0.0, base, 1.0, Scale::Edo19);
        assert!((f - base).abs() < 0.01, "Edo19 t=0 expected base {}, got {}", base, f);
    }

    #[test]
    fn edo19_produces_finite_values() {
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let f = quantize_to_scale(t, 220.0, 2.0, Scale::Edo19);
            assert!(f.is_finite() && f >= 219.9, "Edo19 invalid freq {} at t={}", f, t);
        }
    }

    #[test]
    fn edo31_step_size_approximately_correct() {
        let base = 220.0_f32;
        let expected_semitones = 12.0_f32 / 31.0;
        let expected_freq = base * 2.0_f32.powf(expected_semitones / 12.0);
        let f = quantize_to_scale(1.0 / 31.0, base, 1.0, Scale::Edo31);
        assert!((f - expected_freq).abs() < 0.5,
            "Edo31 second degree: expected {:.2} Hz, got {:.2} Hz", expected_freq, f);
    }

    #[test]
    fn edo24_quarter_tone_step() {
        let base = 440.0_f32;
        let expected = base * 2.0_f32.powf(0.5 / 12.0);
        let f = quantize_to_scale(1.0 / 24.0, base, 1.0, Scale::Edo24);
        assert!((f - expected).abs() < 0.5,
            "Edo24 quarter-tone: expected {:.2} Hz, got {:.2} Hz", expected, f);
    }

    #[test]
    fn whole_tone_has_6_degrees() {
        let base = 220.0_f32;
        let expected_4th = base * 2.0_f32.powf(6.0 / 12.0);
        let f = quantize_to_scale(3.0 / 6.0, base, 1.0, Scale::WholeTone);
        assert!((f - expected_4th).abs() < 0.5,
            "WholeTone 4th degree: expected {:.2} Hz, got {:.2} Hz", expected_4th, f);
    }

    #[test]
    fn phrygian_second_degree_is_semitone() {
        let base = 220.0_f32;
        let expected = base * 2.0_f32.powf(1.0 / 12.0);
        let f = quantize_to_scale(1.0 / 7.0, base, 1.0, Scale::Phrygian);
        assert!((f - expected).abs() < 0.5,
            "Phrygian second degree: expected {:.2} Hz, got {:.2} Hz", expected, f);
    }

    #[test]
    fn lydian_fourth_degree_is_tritone() {
        let base = 220.0_f32;
        let expected = base * 2.0_f32.powf(6.0 / 12.0);
        let f = quantize_to_scale(3.0 / 7.0, base, 1.0, Scale::Lydian);
        assert!((f - expected).abs() < 0.5,
            "Lydian tritone: expected {:.2} Hz, got {:.2} Hz", expected, f);
    }

    #[test]
    fn scale_from_str_new_variants() {
        use crate::sonification::Scale;
        assert_eq!(Scale::from("edo19"),      Scale::Edo19);
        assert_eq!(Scale::from("edo31"),      Scale::Edo31);
        assert_eq!(Scale::from("edo24"),      Scale::Edo24);
        assert_eq!(Scale::from("whole_tone"), Scale::WholeTone);
        assert_eq!(Scale::from("phrygian"),   Scale::Phrygian);
        assert_eq!(Scale::from("lydian"),     Scale::Lydian);
        assert_eq!(Scale::from("unknown"),    Scale::Pentatonic);
    }

    #[test]
    fn chord_intervals_known_values() {
        let major = chord_intervals_for("major");
        assert_eq!(major, [4.0, 7.0, 0.0]);

        let minor = chord_intervals_for("minor");
        assert_eq!(minor, [3.0, 7.0, 0.0]);

        let dom7 = chord_intervals_for("dom7");
        assert_eq!(dom7, [4.0, 7.0, 10.0]);

        let power = chord_intervals_for("power");
        assert_eq!(power, [7.0, 12.0, 0.0]);

        let octave = chord_intervals_for("octave");
        assert_eq!(octave, [12.0, 24.0, 0.0]);
    }

    #[test]
    fn chord_intervals_unknown_returns_zeros() {
        let unknown = chord_intervals_for("diminished_eleventh_no_one_uses_this");
        assert_eq!(unknown, [0.0, 0.0, 0.0]);
    }

    // -------------------------------------------------------------------------
    // Item 18: Golden audio regression tests
    // -------------------------------------------------------------------------

    fn run_osc(freq: f32, shape: OscShape, n: usize) -> Vec<f32> {
        let mut osc = Oscillator::new(freq, shape, 44100.0);
        (0..n).map(|_| osc.next_sample()).collect()
    }

    #[test]
    fn golden_sine_440hz_first_sample() {
        let samples = run_osc(440.0, OscShape::Sine, 1);
        assert!(samples[0].abs() < 1e-6, "First sine sample should be 0.0, got {}", samples[0]);
    }

    #[test]
    fn golden_sine_440hz_quarter_period() {
        let samples = run_osc(440.0, OscShape::Sine, 26);
        let peak = samples[25];
        assert!(peak > 0.9, "Sine quarter-period sample should be near +1, got {}", peak);
    }

    #[test]
    fn golden_saw_first_sample_in_range() {
        let samples = run_osc(440.0, OscShape::Saw, 1);
        assert!(samples[0].is_finite() && samples[0].abs() <= 1.01,
            "Saw first sample out of range: {}", samples[0]);
    }

    #[test]
    fn golden_triangle_amplitude_bounded() {
        let samples = run_osc(220.0, OscShape::Triangle, 4410);
        let max_amp = samples.iter().cloned().fold(0.0f32, f32::max);
        let min_amp = samples.iter().cloned().fold(0.0f32, f32::min);
        assert!(max_amp <= 1.2, "Triangle amplitude exceeded +1.2: {}", max_amp);
        assert!(min_amp >= -1.2, "Triangle amplitude exceeded -1.2: {}", min_amp);
    }

    #[test]
    fn golden_square_amplitude_bounded() {
        let samples = run_osc(440.0, OscShape::Square, 4410);
        let max_amp = samples[1000..].iter().cloned().fold(0.0f32, f32::max);
        assert!(max_amp <= 1.1, "Square amplitude exceeded +1.1: {}", max_amp);
    }

    #[test]
    fn golden_noise_covers_both_signs() {
        let samples = run_osc(440.0, OscShape::Noise, 100);
        let has_pos = samples.iter().any(|&s| s > 0.1);
        let has_neg = samples.iter().any(|&s| s < -0.1);
        assert!(has_pos, "Noise should produce positive samples");
        assert!(has_neg, "Noise should produce negative samples");
    }

    #[test]
    fn golden_sine_deterministic_across_runs() {
        let a = run_osc(330.0, OscShape::Sine, 64);
        let b = run_osc(330.0, OscShape::Sine, 64);
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            assert_eq!(x.to_bits(), y.to_bits(),
                "Sine output not deterministic at sample {}: {} != {}", i, x, y);
        }
    }

    #[test]
    fn golden_lorenz_trajectory_deterministic() {
        let mut s1 = Lorenz::new(10.0, 28.0, 2.6667);
        let mut s2 = Lorenz::new(10.0, 28.0, 2.6667);
        for _ in 0..500 { s1.step(0.001); s2.step(0.001); }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "Lorenz trajectory not deterministic");
        }
    }

    // -------------------------------------------------------------------------
    // Item 19: Config hot-reload test coverage
    // -------------------------------------------------------------------------

    #[test]
    fn hot_reload_modified_field_is_picked_up() {
        let dir = std::env::temp_dir();
        let path = dir.join("math_sonify_test_hot_reload.toml");
        let mut cfg1 = Config::default();
        cfg1.lorenz.sigma = 12.5;
        std::fs::write(&path, toml::to_string(&cfg1).expect("serialize")).expect("write");
        let loaded1 = load_config(&path);
        assert!((loaded1.lorenz.sigma - 12.5).abs() < 1e-9,
            "Initial load should see sigma=12.5, got {}", loaded1.lorenz.sigma);
        let mut cfg2 = Config::default();
        cfg2.lorenz.sigma = 18.0;
        cfg2.audio.reverb_wet = 0.7;
        std::fs::write(&path, toml::to_string(&cfg2).expect("serialize")).expect("overwrite");
        let loaded2 = load_config(&path);
        assert!((loaded2.lorenz.sigma - 18.0).abs() < 1e-9,
            "After hot-reload should see sigma=18.0, got {}", loaded2.lorenz.sigma);
        assert!((loaded2.audio.reverb_wet - 0.7).abs() < 1e-5,
            "After hot-reload should see reverb_wet=0.7, got {}", loaded2.audio.reverb_wet);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn hot_reload_invalid_value_is_clamped() {
        let dir = std::env::temp_dir();
        let path = dir.join("math_sonify_test_hot_reload_clamp.toml");
        let mut cfg = Config::default();
        cfg.audio.reverb_wet = 5.0;
        std::fs::write(&path, toml::to_string(&cfg).expect("serialize")).expect("write");
        let loaded = load_config(&path);
        assert!(loaded.audio.reverb_wet <= 1.0,
            "reverb_wet should be clamped to <=1.0, got {}", loaded.audio.reverb_wet);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn hot_reload_preserves_unchanged_fields() {
        let dir = std::env::temp_dir();
        let path = dir.join("math_sonify_test_hot_reload_stable.toml");
        let cfg = Config::default();
        std::fs::write(&path, toml::to_string(&cfg).expect("serialize")).expect("write");
        let a = load_config(&path);
        let b = load_config(&path);
        assert_eq!(a.lorenz.sigma, b.lorenz.sigma);
        assert_eq!(a.system.dt, b.system.dt);
        assert_eq!(a.audio.sample_rate, b.audio.sample_rate);
        let _ = std::fs::remove_file(&path);
    }

    // -------------------------------------------------------------------------
    // DSP boundary, NaN, and zero-input tests
    // -------------------------------------------------------------------------

    /// All oscillator shapes must produce finite output at an extremely low frequency.
    #[test]
    fn oscillator_finite_output_low_frequency() {
        use crate::synth::oscillator::OscShape;
        let sr = 44100.0_f32;
        for shape in [OscShape::Sine, OscShape::Saw, OscShape::Square, OscShape::Triangle, OscShape::Noise] {
            let mut osc = Oscillator::new(0.001, shape, sr);
            for _ in 0..128 {
                let s = osc.next_sample();
                assert!(s.is_finite(),
                    "Oscillator {:?} at 0.001 Hz produced non-finite: {}", shape, s);
            }
        }
    }

    /// Oscillator at zero frequency must not produce NaN.
    #[test]
    fn oscillator_zero_frequency_no_nan() {
        let mut osc = Oscillator::new(0.0, OscShape::Sine, 44100.0);
        for _ in 0..64 {
            let s = osc.next_sample();
            assert!(s.is_finite(),
                "Zero-frequency sine produced non-finite: {}", s);
        }
    }

    /// Oscillator at near-Nyquist frequency must not produce NaN.
    #[test]
    fn oscillator_near_nyquist_no_nan() {
        let sr = 44100.0_f32;
        for shape in [OscShape::Saw, OscShape::Square, OscShape::Triangle] {
            let mut osc = Oscillator::new(sr * 0.499, shape, sr);
            for _ in 0..256 {
                let s = osc.next_sample();
                assert!(s.is_finite(),
                    "Near-Nyquist {:?} produced non-finite: {}", shape, s);
            }
        }
    }

    /// BiquadFilter must produce finite output after an NaN input.
    #[test]
    fn biquad_filter_recovers_from_nan_input() {
        use crate::synth::filter::BiquadFilter;
        let mut f = BiquadFilter::low_pass(1000.0, 0.707, 44100.0);
        let _ = f.process(f32::NAN);
        let out = f.process(0.5);
        assert!(out.is_finite(),
            "BiquadFilter did not recover after NaN input: {}", out);
    }

    /// BiquadFilter at extreme cutoff values must not produce NaN.
    #[test]
    fn biquad_filter_extreme_cutoff_no_nan() {
        use crate::synth::filter::BiquadFilter;
        let sr = 44100.0_f32;
        let mut f_low = BiquadFilter::low_pass(20.0, 0.707, sr);
        for _ in 0..64 {
            let out = f_low.process(0.5);
            assert!(out.is_finite(), "Low-cutoff filter produced non-finite: {}", out);
        }
        // High Q (near instability)
        let mut f_hq = BiquadFilter::low_pass(440.0, 20.0, sr);
        for _ in 0..64 {
            let out = f_hq.process(0.1);
            assert!(out.is_finite(), "High-Q filter produced non-finite: {}", out);
        }
    }

    /// ADSR level must remain in [0, 1] throughout a complete A/D/S/R cycle.
    #[test]
    fn adsr_level_stays_in_unit_range() {
        use crate::synth::envelope::Adsr;
        let sr = 44100.0_f32;
        let mut env = Adsr::new(10.0, 100.0, 0.7, 200.0, sr);
        env.trigger();
        for _ in 0..(sr as usize / 2) {
            let l = env.next_sample();
            assert!(l >= 0.0 && l <= 1.0 + 1e-4,
                "ADSR level out of [0,1] during hold: {}", l);
        }
        env.release();
        for _ in 0..(sr as usize / 4) {
            let l = env.next_sample();
            assert!(l >= 0.0 && l <= 1.0 + 1e-4,
                "ADSR level out of [0,1] during release: {}", l);
        }
    }

    /// Bitcrusher at 16-bit depth and zero rate_crush must be transparent.
    #[test]
    fn bitcrusher_bypass_transparent() {
        use crate::synth::bitcrusher::Bitcrusher;
        let mut bc = Bitcrusher::new();
        bc.bit_depth = 16.0;
        bc.rate_crush = 0.0;
        bc.dither = false;
        for &val in &[-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
            let out = bc.process(val);
            assert!((out - val).abs() < 1e-4,
                "Bitcrusher bypass changed {} to {}", val, out);
        }
    }

    /// Bitcrusher at 1-bit depth with no dither must produce at most 2 distinct output levels.
    #[test]
    fn bitcrusher_1bit_only_two_levels() {
        use crate::synth::bitcrusher::Bitcrusher;
        let mut bc = Bitcrusher::new();
        bc.bit_depth = 1.0;
        bc.rate_crush = 0.0;
        bc.dither = false;
        let levels: std::collections::HashSet<i32> = (-10..=10)
            .map(|i| {
                let out = bc.process(i as f32 * 0.1);
                (out * 100.0).round() as i32
            })
            .collect();
        assert!(levels.len() <= 2,
            "1-bit bitcrusher should produce at most 2 levels, got: {:?}", levels);
    }

    /// DelayLine with all-zero input must produce finite output.
    #[test]
    fn delay_line_zero_input_stays_finite() {
        use crate::synth::delay::DelayLine;
        let mut d = DelayLine::new(500.0, 44100.0);
        for _ in 0..4096 {
            let (l, r) = d.process(0.0, 0.0);
            assert!(l.is_finite() && r.is_finite(),
                "DelayLine zero-input produced non-finite: ({}, {})", l, r);
        }
    }

    /// Freeverb must produce finite output after NaN injection.
    #[test]
    fn freeverb_recovers_from_nan_input() {
        use crate::synth::reverb::Freeverb;
        let mut rv = Freeverb::new(44100.0);
        rv.wet = 0.5;
        for _ in 0..256 { rv.process(0.1, -0.1); }
        let _ = rv.process(f32::NAN, f32::NAN);
        for _ in 0..32 {
            let (l, r) = rv.process(0.0, 0.0);
            assert!(l.is_finite() && r.is_finite(),
                "Freeverb did not recover from NaN: ({}, {})", l, r);
        }
    }

    /// KarplusStrong at high frequency must produce finite samples.
    #[test]
    fn karplus_strong_high_frequency_finite() {
        use crate::synth::karplus::KarplusStrong;
        let sr = 44100.0_f32;
        let mut ks = KarplusStrong::new(20.0, sr);
        ks.trigger(4000.0, sr);
        for _ in 0..256 {
            let s = ks.next_sample();
            assert!(s.is_finite(),
                "KarplusStrong 4000 Hz produced non-finite: {}", s);
        }
    }

    /// KarplusStrong at minimum supported frequency (20 Hz) must not panic.
    #[test]
    fn karplus_strong_minimum_frequency_no_panic() {
        use crate::synth::karplus::KarplusStrong;
        let sr = 44100.0_f32;
        let mut ks = KarplusStrong::new(20.0, sr);
        ks.trigger(20.0, sr);
        for _ in 0..512 {
            let s = ks.next_sample();
            assert!(s.is_finite(),
                "KarplusStrong 20 Hz produced non-finite: {}", s);
        }
    }

    /// quantize_to_scale at t=1.0 must return a positive finite frequency within the octave range.
    #[test]
    fn quantize_to_scale_t_one_bounded() {
        let base = 220.0_f32;
        let oct = 3.0_f32;
        for scale in [Scale::Pentatonic, Scale::Chromatic, Scale::WholeTone, Scale::Phrygian, Scale::Lydian] {
            let f = quantize_to_scale(1.0, base, oct, scale);
            assert!(f.is_finite() && f > 0.0,
                "quantize_to_scale(1.0, {:?}) is not positive-finite: {}", scale, f);
            let max = base * 2.0_f32.powf(oct);
            assert!(f <= max * 1.01,
                "quantize_to_scale(1.0, {:?}) exceeds max {}: got {}", scale, max, f);
        }
    }

    /// chord_intervals_for must return non-negative finite semitone offsets for all modes.
    #[test]
    fn chord_intervals_all_modes_valid() {
        for mode in ["major", "minor", "power", "sus2", "octave", "dom7", "none", "unknown"] {
            let ivs = chord_intervals_for(mode);
            for iv in ivs {
                assert!(iv.is_finite() && iv >= 0.0,
                    "chord_intervals_for({}) contains invalid value: {}", mode, iv);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Remaining dynamical systems: step() stays finite after 1000 steps
    // -----------------------------------------------------------------------

    #[test]
    fn double_pendulum_stays_finite_after_1000_steps() {
        use crate::systems::DoublePendulum;
        let mut sys = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "DoublePendulum non-finite: {:?}", sys.state());
    }

    #[test]
    fn van_der_pol_stays_finite_after_1000_steps() {
        use crate::systems::VanDerPol;
        let mut sys = VanDerPol::new();
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "VanDerPol non-finite: {:?}", sys.state());
    }

    #[test]
    fn halvorsen_stays_finite_after_1000_steps() {
        use crate::systems::Halvorsen;
        let mut sys = Halvorsen::new();
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "Halvorsen non-finite: {:?}", sys.state());
    }

    #[test]
    fn aizawa_stays_finite_after_1000_steps() {
        use crate::systems::Aizawa;
        let mut sys = Aizawa::new();
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "Aizawa non-finite: {:?}", sys.state());
    }

    #[test]
    fn chua_stays_finite_after_1000_steps() {
        use crate::systems::Chua;
        let mut sys = Chua::new();
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "Chua non-finite: {:?}", sys.state());
    }

    #[test]
    fn henon_map_stays_finite_after_1000_steps() {
        use crate::systems::HenonMap;
        let mut sys = HenonMap::new();
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "HenonMap non-finite: {:?}", sys.state());
    }

    #[test]
    fn geodesic_torus_stays_finite_after_1000_steps() {
        use crate::systems::GeodesicTorus;
        let mut sys = GeodesicTorus::new(3.0, 1.0);
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "GeodesicTorus non-finite: {:?}", sys.state());
    }

    #[test]
    fn nose_hoover_stays_finite_after_1000_steps() {
        use crate::systems::NoseHoover;
        let mut sys = NoseHoover::new();
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "NoseHoover non-finite: {:?}", sys.state());
    }

    #[test]
    fn mackey_glass_stays_finite_after_1000_steps() {
        use crate::systems::MackeyGlass;
        let mut sys = MackeyGlass::new();
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "MackeyGlass non-finite: {:?}", sys.state());
    }

    #[test]
    fn lorenz96_stays_finite_after_1000_steps() {
        use crate::systems::Lorenz96;
        let mut sys = Lorenz96::new();
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "Lorenz96 non-finite: {:?}", sys.state());
    }

    #[test]
    fn coupled_map_lattice_stays_finite_after_1000_steps() {
        use crate::systems::CoupledMapLattice;
        let mut sys = CoupledMapLattice::new(3.9, 0.35);
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "CoupledMapLattice non-finite: {:?}", sys.state());
    }

    #[test]
    fn hindmarsh_rose_stays_finite_after_1000_steps() {
        use crate::systems::HindmarshRose;
        let mut sys = HindmarshRose::new(3.0, 0.006);
        for _ in 0..1000 { sys.step(0.001); }
        assert!(all_finite(sys.state()), "HindmarshRose non-finite: {:?}", sys.state());
    }

    // -----------------------------------------------------------------------
    // DirectMapping: output stays in expected ranges
    // -----------------------------------------------------------------------

    #[test]
    fn direct_mapping_freqs_are_finite_and_positive() {
        use crate::sonification::{DirectMapping, Sonification};
        use crate::config::SonificationConfig;
        let mut mapper = DirectMapping::new();
        let state = vec![1.0, -2.0, 0.5, 3.0];
        let cfg = SonificationConfig::default();
        let mut params = mapper.map(&state, 10.0, &cfg);
        for _ in 0..20 { params = mapper.map(&state, 10.0, &cfg); }
        for (i, &f) in params.freqs.iter().enumerate() {
            assert!(f.is_finite() && f > 0.0,
                "DirectMapping voice {} freq not positive-finite: {}", i, f);
        }
    }

    #[test]
    fn direct_mapping_amps_in_unit_interval() {
        use crate::sonification::{DirectMapping, Sonification};
        use crate::config::SonificationConfig;
        let mut mapper = DirectMapping::new();
        let state = vec![1.0, -2.0, 0.5];
        let cfg = SonificationConfig::default();
        for _ in 0..30 {
            let params = mapper.map(&state, 5.0, &cfg);
            for (i, &a) in params.amps.iter().enumerate() {
                assert!(a.is_finite() && a >= 0.0 && a <= 1.01,
                    "DirectMapping amp[{}] out of [0,1]: {}", i, a);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Audio config boundary conditions
    // -----------------------------------------------------------------------

    #[test]
    fn audio_config_validate_all_fields_within_bounds() {
        let mut cfg = Config::default();
        cfg.audio.reverb_wet = 999.0;
        cfg.audio.delay_ms = 0.0;
        cfg.audio.delay_feedback = -1.0;
        cfg.audio.master_volume = -10.0;
        cfg.audio.chorus_mix = 50.0;
        cfg.audio.chorus_rate = -1.0;
        cfg.audio.chorus_depth = 999.0;
        cfg.audio.waveshaper_drive = -5.0;
        cfg.audio.waveshaper_mix = 2.0;
        cfg.audio.rate_crush = -0.5;
        cfg.audio.bit_depth = 0.0;
        cfg.validate();
        assert!(cfg.audio.reverb_wet >= 0.0 && cfg.audio.reverb_wet <= 1.0);
        assert!(cfg.audio.delay_ms >= 1.0 && cfg.audio.delay_ms <= 5000.0);
        assert!(cfg.audio.delay_feedback >= 0.0 && cfg.audio.delay_feedback <= 0.99);
        assert!(cfg.audio.master_volume >= 0.0 && cfg.audio.master_volume <= 1.0);
        assert!(cfg.audio.chorus_mix >= 0.0 && cfg.audio.chorus_mix <= 1.0);
        assert!(cfg.audio.chorus_rate >= 0.01 && cfg.audio.chorus_rate <= 20.0);
        assert!(cfg.audio.chorus_depth >= 0.0 && cfg.audio.chorus_depth <= 50.0);
        assert!(cfg.audio.waveshaper_drive >= 0.0 && cfg.audio.waveshaper_drive <= 100.0);
        assert!(cfg.audio.waveshaper_mix >= 0.0 && cfg.audio.waveshaper_mix <= 1.0);
        assert!(cfg.audio.rate_crush >= 0.0 && cfg.audio.rate_crush <= 1.0);
        assert!(cfg.audio.bit_depth >= 1.0 && cfg.audio.bit_depth <= 32.0);
    }

    #[test]
    fn oscillator_zero_frequency_does_not_panic() {
        use crate::synth::oscillator::{Oscillator, OscShape};
        for &shape in &[OscShape::Sine, OscShape::Saw, OscShape::Square, OscShape::Triangle] {
            let mut osc = Oscillator::new(0.0, shape, 44100.0);
            for _ in 0..100 {
                let s = osc.next_sample();
                assert!(s.is_finite(), "Shape {:?} at freq=0 produced non-finite: {}", shape, s);
            }
        }
    }

    #[test]
    fn oscillator_nyquist_frequency_does_not_panic() {
        use crate::synth::oscillator::{Oscillator, OscShape};
        for &shape in &[OscShape::Sine, OscShape::Saw, OscShape::Square] {
            let mut osc = Oscillator::new(22050.0, shape, 44100.0);
            for _ in 0..100 {
                let s = osc.next_sample();
                assert!(s.is_finite(), "Shape {:?} at Nyquist produced non-finite: {}", shape, s);
            }
        }
    }

    // -----------------------------------------------------------------------
    // All system-specific configs survive TOML serialization round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn config_all_system_configs_roundtrip_toml() {
        let mut orig = Config::default();
        orig.lorenz.sigma = 12.5; orig.lorenz.rho = 32.0;
        orig.rossler.c = 7.5;
        orig.double_pendulum.m1 = 2.0;
        orig.duffing.omega = 1.3;
        orig.van_der_pol.mu = 3.5;
        orig.halvorsen.a = 1.75;
        orig.aizawa.d = 4.0;
        orig.chua.alpha = 18.0;
        orig.hindmarsh_rose.current_i = 2.5;
        orig.coupled_map_lattice.r = 3.7;
        orig.mackey_glass.tau = 25.0;
        orig.nose_hoover.a = 2.5;
        orig.henon_map.b = 0.25;
        orig.lorenz96.f = 10.0;
        let toml_str = toml::to_string(&orig).expect("serialize");
        let loaded: Config = toml::from_str(&toml_str).expect("deserialize");
        assert!((loaded.lorenz.sigma - orig.lorenz.sigma).abs() < 1e-9);
        assert!((loaded.rossler.c - orig.rossler.c).abs() < 1e-9);
        assert!((loaded.duffing.omega - orig.duffing.omega).abs() < 1e-9);
        assert!((loaded.halvorsen.a - orig.halvorsen.a).abs() < 1e-9);
        assert!((loaded.mackey_glass.tau - orig.mackey_glass.tau).abs() < 1e-9);
        assert!((loaded.lorenz96.f - orig.lorenz96.f).abs() < 1e-9);
    }

    // -----------------------------------------------------------------------
    // Lorenz attractor: trajectory confinement (canonical 50 000-step test)
    // -----------------------------------------------------------------------

    /// The canonical test referenced in README `cargo test -- lorenz_stays_on_attractor`.
    /// With sigma=10, rho=28, beta=8/3 the RK4 trajectory must stay within
    /// the known strange attractor bounding box: |x| < 30, |y| < 30, 0 < z < 60.
    #[test]
    fn lorenz_stays_on_attractor() {
        let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
        for _ in 0..50_000 {
            sys.step(0.001);
        }
        let s = sys.state();
        assert!(all_finite(s), "Lorenz state non-finite: {:?}", s);
        assert!(s[0].abs() < 30.0, "Lorenz x outside attractor bounds: {}", s[0]);
        assert!(s[1].abs() < 30.0, "Lorenz y outside attractor bounds: {}", s[1]);
        assert!(s[2] > 0.0 && s[2] < 60.0, "Lorenz z outside attractor bounds: {}", s[2]);
    }

    /// z remains strictly positive on the Lorenz attractor after burn-in.
    #[test]
    fn lorenz_z_stays_positive() {
        let mut sys = Lorenz::new(10.0, 28.0, 2.6667);
        for _ in 0..5_000 { sys.step(0.001); }
        for _ in 0..20_000 {
            sys.step(0.001);
            assert!(sys.state()[2] > 0.0,
                "Lorenz z became non-positive: {}", sys.state()[2]);
        }
    }

    // -----------------------------------------------------------------------
    // Rossler: boundedness (periodicity verification)
    // -----------------------------------------------------------------------

    /// Rossler trajectory must stay within |x|,|y| < 15, 0 < z < 25.
    #[test]
    fn rossler_stays_bounded_30000_steps() {
        let mut sys = Rossler::new(0.2, 0.2, 5.7);
        for _ in 0..30_000 { sys.step(0.001); }
        let s = sys.state();
        assert!(all_finite(s), "Rossler non-finite: {:?}", s);
        assert!(s[0].abs() < 15.0, "Rossler x out of bounds: {}", s[0]);
        assert!(s[1].abs() < 15.0, "Rossler y out of bounds: {}", s[1]);
        assert!(s[2] > 0.0 && s[2] < 25.0, "Rossler z out of bounds: {}", s[2]);
    }

    // -----------------------------------------------------------------------
    // Double pendulum: energy conservation at small angles
    // -----------------------------------------------------------------------

    /// At small angles (0.1 rad) the Yoshida 4th-order symplectic integrator
    /// must conserve the Hamiltonian to within 1% over 10 000 steps.
    #[test]
    fn double_pendulum_energy_conserved_small_angles() {
        use crate::systems::DoublePendulum;
        let (m1, m2, l1, l2, g) = (1.0_f64, 1.0, 1.0, 1.0, 9.81);
        let mut sys = DoublePendulum::new(m1, m2, l1, l2);
        sys.set_state(&[0.1_f64, 0.1, 0.0, 0.0]);

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
        let rel = ((e1 - e0) / e0.abs()).abs();
        assert!(rel < 0.01,
            "Energy drift too large: e0={:.6} e1={:.6} rel={:.4}", e0, e1, rel);
    }

    /// At small angles (0.05 rad) the pendulum must stay bounded near the equilibrium.
    #[test]
    fn double_pendulum_small_angle_stays_near_equilibrium() {
        use crate::systems::DoublePendulum;
        let mut sys = DoublePendulum::new(1.0, 1.0, 1.0, 1.0);
        sys.set_state(&[0.05_f64, 0.05, 0.0, 0.0]);
        for _ in 0..10_000 {
            sys.step(0.001);
            let s = sys.state();
            assert!(all_finite(s), "DP state non-finite: {:?}", s);
            assert!(s[0].abs() < 0.5, "theta1 escaped small-angle regime: {}", s[0]);
            assert!(s[1].abs() < 0.5, "theta2 escaped small-angle regime: {}", s[1]);
        }
    }

    // -----------------------------------------------------------------------
    // Kuramoto: resonance / synchronization transition
    // -----------------------------------------------------------------------

    /// K_c = 2*gamma = 1.0 for a Lorentzian with half-width gamma=0.5.
    /// Below K_c the order parameter r must remain < 0.5 (incoherent).
    #[test]
    fn kuramoto_below_critical_coupling_incoherent() {
        let mut sys = Kuramoto::new(16, 0.1);
        for _ in 0..20_000 { sys.step(0.01); }
        assert!(sys.order_parameter() < 0.5,
            "Expected incoherence below K_c, got r={:.4}", sys.order_parameter());
    }

    /// Well above K_c the order parameter must exceed 0.5 (synchronized).
    #[test]
    fn kuramoto_above_critical_coupling_synchronizes() {
        let mut sys = Kuramoto::new(16, 5.0);
        for _ in 0..50_000 { sys.step(0.01); }
        assert!(sys.order_parameter() > 0.5,
            "Expected synchronization above K_c, got r={:.4}", sys.order_parameter());
    }

    /// The order parameter must always lie in [0, 1].
    #[test]
    fn kuramoto_order_parameter_in_unit_interval() {
        for &k in &[0.0_f64, 0.5, 1.0, 2.0, 10.0] {
            let mut sys = Kuramoto::new(8, k);
            for _ in 0..5_000 { sys.step(0.01); }
            let r = sys.order_parameter();
            assert!(r >= 0.0 && r <= 1.0 + 1e-9,
                "Order parameter out of [0,1] at K={}: {}", k, r);
        }
    }

    // -----------------------------------------------------------------------
    // Three-Body: Hamiltonian energy conservation
    // -----------------------------------------------------------------------

    /// The three-body leapfrog integrator must conserve energy to < 1%
    /// over 10 000 steps at dt=0.001.
    #[test]
    fn three_body_energy_conserved() {
        use crate::systems::ThreeBody;
        let mut sys = ThreeBody::new([1.0, 1.0, 1.0]);
        for _ in 0..10_000 { sys.step(0.001); }
        let err = sys.energy_error;
        assert!(err < 0.01, "Three-body energy error > 1%: {:.4}", err);
    }

    // -----------------------------------------------------------------------
    // Audio mapping: valid MIDI range
    // -----------------------------------------------------------------------

    /// All quantized frequencies must map to MIDI notes in [0, 127].
    #[test]
    fn scale_quantization_produces_valid_midi_notes() {
        let base = 110.0_f32; // A2
        let oct  = 3.0_f32;
        for &scale in &[Scale::Pentatonic, Scale::Chromatic, Scale::Lydian, Scale::Phrygian] {
            for i in 0..=100 {
                let t = i as f32 / 100.0;
                let f = quantize_to_scale(t, base, oct, scale);
                let midi = 69.0_f32 + 12.0 * (f / 440.0).log2();
                assert!(midi >= 0.0 && midi <= 127.0,
                    "Scale {:?} t={:.3}: freq {:.2} -> MIDI {:.1} out of [0,127]",
                    scale, t, f, midi);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Polyphony limits
    // -----------------------------------------------------------------------

    /// AudioParams has exactly 4 voice slots; voices beyond the state
    /// dimension must be zero-amplitude.
    #[test]
    fn polyphony_limit_four_voices_max() {
        use crate::sonification::{DirectMapping, Sonification};
        use crate::config::SonificationConfig;
        let mut mapper = DirectMapping::new();
        let cfg = SonificationConfig::default();

        // 3D state — voice 3 must be zero
        let params = mapper.map(&[1.0_f64, -2.0, 0.5], 5.0, &cfg);
        assert_eq!(params.freqs.len(), 4, "Must have exactly 4 frequency slots");
        assert_eq!(params.amps.len(),  4, "Must have exactly 4 amplitude slots");
        assert_eq!(params.amps[3], 0.0, "Voice 3 amp should be 0 for 3-D state: {}", params.amps[3]);

        // 1D state — voices 1-3 must all be zero
        let p1 = mapper.map(&[0.5_f64], 1.0, &cfg);
        for i in 1..4 {
            assert_eq!(p1.amps[i], 0.0, "Voice {} amp not 0 for 1-D state: {}", i, p1.amps[i]);
        }
    }

    /// Default voice_levels must be in descending order (louder to quieter).
    #[test]
    fn polyphony_default_voice_levels_descending() {
        use crate::config::SonificationConfig;
        let vl = SonificationConfig::default().voice_levels;
        assert!(vl[0] >= vl[1], "voice_levels[0] < [1]");
        assert!(vl[1] >= vl[2], "voice_levels[1] < [2]");
        assert!(vl[2] >= vl[3], "voice_levels[2] < [3]");
    }
}
