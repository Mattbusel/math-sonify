#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 24 { return; }

    // Interpret bytes as f64 parameters
    let make_f64 = |b: &[u8]| -> f64 {
        let arr: [u8; 8] = b[..8].try_into().unwrap();
        let v = f64::from_le_bytes(arr);
        if v.is_finite() { v.clamp(-1000.0, 1000.0) } else { 1.0 }
    };
    let p1 = make_f64(&data[0..8]);
    let p2 = make_f64(&data[8..16]);
    let p3 = make_f64(&data[16..24]);

    // Fuzz Lorenz: verify it never produces non-finite state values
    use math_sonify_plugin::systems::Lorenz;
    let mut sys = Lorenz::new(
        p1.abs().clamp(0.1, 100.0),
        p2.abs().clamp(0.1, 200.0),
        p3.abs().clamp(0.01, 20.0),
    );
    for _ in 0..10_000 {
        sys.step(0.001);
        let s = sys.state();
        // Must not produce NaN or Inf
        assert!(
            s.iter().all(|v| v.is_finite()),
            "Lorenz produced non-finite state: {:?}",
            s
        );
    }
});
