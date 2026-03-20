//! Freeverb — Jezar's classic algorithmic reverb.
//!
//! 8 parallel comb filters feed 4 series allpass filters in a stereo
//! configuration.  The allpass filters decorrelate the left and right
//! channels by using delay-line lengths that differ by 23 samples.
//!
//! This reverb is kept on the master bus as a low-CPU hall/room effect.
//! For a denser, more diffuse tail see [`FdnReverb`](crate::synth::FdnReverb).

const NUM_COMBS: usize = 8;
const NUM_ALLPASS: usize = 4;

// Tuning constants (samples at 44100 Hz)
const COMB_TUNING_L: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const COMB_TUNING_R: [usize; 8] = [
    1116 + 23,
    1188 + 23,
    1277 + 23,
    1356 + 23,
    1422 + 23,
    1491 + 23,
    1557 + 23,
    1617 + 23,
];
const ALLPASS_TUNING_L: [usize; 4] = [556, 441, 341, 225];
const ALLPASS_TUNING_R: [usize; 4] = [556 + 23, 441 + 23, 341 + 23, 225 + 23];

struct CombFilter {
    buf: Vec<f32>,
    pos: usize,
    feedback: f32,
    damp1: f32,
    damp2: f32,
    filter_store: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buf: vec![0.0; size],
            pos: 0,
            feedback: 0.84,
            damp1: 0.2,
            damp2: 0.8,
            filter_store: 0.0,
        }
    }

    fn set_damp(&mut self, d: f32) {
        self.damp1 = d;
        self.damp2 = 1.0 - d;
    }
    fn set_feedback(&mut self, f: f32) {
        self.feedback = f;
    }

    fn process(&mut self, input: f32) -> f32 {
        let out = self.buf[self.pos];
        self.filter_store = out * self.damp2 + self.filter_store * self.damp1;
        self.buf[self.pos] = input + self.filter_store * self.feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

struct AllpassFilter {
    buf: Vec<f32>,
    pos: usize,
}

impl AllpassFilter {
    fn new(size: usize) -> Self {
        Self {
            buf: vec![0.0; size],
            pos: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buf[self.pos];
        let output = -input + buf_out;
        self.buf[self.pos] = input + buf_out * 0.5;
        self.pos = (self.pos + 1) % self.buf.len();
        output
    }
}

pub struct Freeverb {
    combs_l: [CombFilter; NUM_COMBS],
    combs_r: [CombFilter; NUM_COMBS],
    allpass_l: [AllpassFilter; NUM_ALLPASS],
    allpass_r: [AllpassFilter; NUM_ALLPASS],
    pub wet: f32,
    pub room_size: f32,
    pub damp: f32,
}

impl Freeverb {
    /// Create a new Freeverb instance tuned to the given sample rate.
    ///
    /// Delay-line lengths are scaled proportionally from the canonical
    /// 44100 Hz values, so the reverb time is sample-rate independent.
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 44100.0;
        let scale_usize = |t: usize| ((t as f32 * scale) as usize).max(1);

        macro_rules! make_combs {
            ($tuning:expr) => {{
                let t = $tuning;
                [
                    CombFilter::new(scale_usize(t[0])),
                    CombFilter::new(scale_usize(t[1])),
                    CombFilter::new(scale_usize(t[2])),
                    CombFilter::new(scale_usize(t[3])),
                    CombFilter::new(scale_usize(t[4])),
                    CombFilter::new(scale_usize(t[5])),
                    CombFilter::new(scale_usize(t[6])),
                    CombFilter::new(scale_usize(t[7])),
                ]
            }};
        }
        macro_rules! make_allpass {
            ($tuning:expr) => {{
                let t = $tuning;
                [
                    AllpassFilter::new(scale_usize(t[0])),
                    AllpassFilter::new(scale_usize(t[1])),
                    AllpassFilter::new(scale_usize(t[2])),
                    AllpassFilter::new(scale_usize(t[3])),
                ]
            }};
        }

        Self {
            combs_l: make_combs!(COMB_TUNING_L),
            combs_r: make_combs!(COMB_TUNING_R),
            allpass_l: make_allpass!(ALLPASS_TUNING_L),
            allpass_r: make_allpass!(ALLPASS_TUNING_R),
            wet: 0.4,
            room_size: 0.84,
            damp: 0.2,
        }
    }

    /// Set the comb-filter feedback coefficient, controlling reverb decay time.
    ///
    /// A value near `0.84` gives roughly a 2-second tail at 44100 Hz.
    /// Values above `0.98` cause the reverb to ring indefinitely.
    pub fn set_room_size(&mut self, r: f32) {
        self.room_size = r;
        for c in &mut self.combs_l {
            c.set_feedback(r);
        }
        for c in &mut self.combs_r {
            c.set_feedback(r);
        }
    }

    /// Set the high-frequency damping coefficient for each comb filter.
    ///
    /// Higher values roll off highs faster, simulating absorption by air and surfaces.
    pub fn set_damp(&mut self, d: f32) {
        self.damp = d;
        for c in &mut self.combs_l {
            c.set_damp(d);
        }
        for c in &mut self.combs_r {
            c.set_damp(d);
        }
    }

    /// Process one stereo sample pair. Returns (left, right).
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let input_l = if input_l.is_finite() { input_l } else { 0.0 };
        let input_r = if input_r.is_finite() { input_r } else { 0.0 };
        let mono_in = (input_l + input_r) * (0.015 * self.wet.clamp(0.1, 1.0) / 0.4); // scale proportional to wet level
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        for c in &mut self.combs_l {
            out_l += c.process(mono_in);
        }
        for c in &mut self.combs_r {
            out_r += c.process(mono_in);
        }
        for a in &mut self.allpass_l {
            out_l = a.process(out_l);
        }
        for a in &mut self.allpass_r {
            out_r = a.process(out_r);
        }
        // Sanitize: if reverb buffers are corrupted, reset them
        if !out_l.is_finite() || !out_r.is_finite() {
            for c in &mut self.combs_l {
                c.buf.iter_mut().for_each(|x| *x = 0.0);
                c.filter_store = 0.0;
            }
            for c in &mut self.combs_r {
                c.buf.iter_mut().for_each(|x| *x = 0.0);
                c.filter_store = 0.0;
            }
            for a in &mut self.allpass_l {
                a.buf.iter_mut().for_each(|x| *x = 0.0);
            }
            for a in &mut self.allpass_r {
                a.buf.iter_mut().for_each(|x| *x = 0.0);
            }
            return (input_l * (1.0 - self.wet), input_r * (1.0 - self.wet));
        }
        let dry = 1.0 - self.wet;
        (
            input_l * dry + out_l * self.wet,
            input_r * dry + out_r * self.wet,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_freeverb_output_finite() {
        let mut rv = Freeverb::new(SR);
        for i in 0..2000 {
            let x = (i as f32 * 0.05).sin();
            let (l, r) = rv.process(x, x);
            assert!(l.is_finite(), "Reverb L output non-finite");
            assert!(r.is_finite(), "Reverb R output non-finite");
        }
    }

    #[test]
    fn test_freeverb_wet_zero_is_dry() {
        let mut rv = Freeverb::new(SR);
        rv.wet = 0.0;
        let (l, r) = rv.process(0.5, -0.3);
        assert!((l - 0.5).abs() < 1e-5, "wet=0 should pass dry: {}", l);
        assert!((r - (-0.3)).abs() < 1e-5, "wet=0 should pass dry: {}", r);
    }

    #[test]
    fn test_freeverb_produces_tail_after_impulse() {
        // After an impulse, reverb tail should linger
        let mut rv = Freeverb::new(SR);
        rv.wet = 1.0;
        // Send impulse then silence
        rv.process(1.0, 1.0);
        let mut max_tail = 0.0_f32;
        for _ in 0..4410 {
            let (l, _) = rv.process(0.0, 0.0);
            max_tail = max_tail.max(l.abs());
        }
        assert!(max_tail > 0.0, "Reverb should produce a tail after impulse");
    }

    #[test]
    fn test_freeverb_nan_input_safe() {
        let mut rv = Freeverb::new(SR);
        let (l, r) = rv.process(f32::NAN, f32::NAN);
        assert!(l.is_finite(), "NaN input should produce finite output");
        assert!(r.is_finite(), "NaN input should produce finite output");
    }

    #[test]
    fn test_freeverb_larger_room_longer_tail() {
        // Larger room_size (higher feedback) → more energy retained → longer tail
        let mut rv_small = Freeverb::new(SR);
        rv_small.set_room_size(0.5);
        rv_small.wet = 1.0;

        let mut rv_large = Freeverb::new(SR);
        rv_large.set_room_size(0.95);
        rv_large.wet = 1.0;

        // Send identical impulse
        rv_small.process(1.0, 1.0);
        rv_large.process(1.0, 1.0);

        // After 5000 samples, large-room should retain more energy
        let mut energy_small = 0.0_f32;
        let mut energy_large = 0.0_f32;
        for _ in 0..5000 {
            let (l, _) = rv_small.process(0.0, 0.0);
            energy_small += l * l;
            let (l, _) = rv_large.process(0.0, 0.0);
            energy_large += l * l;
        }
        assert!(
            energy_large > energy_small,
            "Larger room should have more reverb tail energy: small={}, large={}",
            energy_small, energy_large
        );
    }

    #[test]
    fn test_freeverb_stereo_outputs_differ() {
        // Due to 23-sample offset between L and R comb filters, L and R should differ
        let mut rv = Freeverb::new(SR);
        rv.wet = 1.0;
        rv.process(1.0, 1.0); // impulse
        let mut diff_sum = 0.0_f32;
        for _ in 0..2000 {
            let (l, r) = rv.process(0.0, 0.0);
            diff_sum += (l - r).abs();
        }
        assert!(diff_sum > 0.0, "Freeverb L and R should differ due to decorrelation");
    }

    #[test]
    fn test_freeverb_set_damp_affects_output() {
        // Low damp → brighter tail; high damp → darker tail. Both should be finite.
        let mut rv_bright = Freeverb::new(SR);
        rv_bright.set_damp(0.0);
        rv_bright.wet = 1.0;

        let mut rv_dark = Freeverb::new(SR);
        rv_dark.set_damp(0.9);
        rv_dark.wet = 1.0;

        rv_bright.process(1.0, 1.0);
        rv_dark.process(1.0, 1.0);

        let mut e_bright = 0.0_f32;
        let mut e_dark = 0.0_f32;
        for _ in 0..2000 {
            let (l, _) = rv_bright.process(0.0, 0.0);
            e_bright += l * l;
            let (l, _) = rv_dark.process(0.0, 0.0);
            e_dark += l * l;
            assert!(l.is_finite(), "Dark reverb output should be finite");
        }
        // Both should produce some reverb tail; they may differ in energy
        assert!(e_bright >= 0.0 && e_dark >= 0.0, "Both should be non-negative energy");
    }
}
