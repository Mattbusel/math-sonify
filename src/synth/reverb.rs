/// Freeverb — Jezar's classic algorithmic reverb.
///
/// 8 parallel comb filters feed 4 series allpass filters in a stereo
/// configuration.  The allpass filters decorrelate the left and right
/// channels by using delay-line lengths that differ by 23 samples.
///
/// This reverb is kept on the master bus as a low-CPU hall/room effect.
/// For a denser, more diffuse tail see [`FdnReverb`](crate::synth::FdnReverb).

const NUM_COMBS: usize = 8;
const NUM_ALLPASS: usize = 4;

// Tuning constants (samples at 44100 Hz)
const COMB_TUNING_L: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const COMB_TUNING_R: [usize; 8] = [1116+23, 1188+23, 1277+23, 1356+23, 1422+23, 1491+23, 1557+23, 1617+23];
const ALLPASS_TUNING_L: [usize; 4] = [556, 441, 341, 225];
const ALLPASS_TUNING_R: [usize; 4] = [556+23, 441+23, 341+23, 225+23];

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
        Self { buf: vec![0.0; size], pos: 0, feedback: 0.84, damp1: 0.2, damp2: 0.8, filter_store: 0.0 }
    }

    fn set_damp(&mut self, d: f32) { self.damp1 = d; self.damp2 = 1.0 - d; }
    fn set_feedback(&mut self, f: f32) { self.feedback = f; }

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
    fn new(size: usize) -> Self { Self { buf: vec![0.0; size], pos: 0 } }

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
        for c in &mut self.combs_l { c.set_feedback(r); }
        for c in &mut self.combs_r { c.set_feedback(r); }
    }

    /// Set the high-frequency damping coefficient for each comb filter.
    ///
    /// Higher values roll off highs faster, simulating absorption by air and surfaces.
    pub fn set_damp(&mut self, d: f32) {
        self.damp = d;
        for c in &mut self.combs_l { c.set_damp(d); }
        for c in &mut self.combs_r { c.set_damp(d); }
    }

    /// Process one stereo sample pair. Returns (left, right).
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let input_l = if input_l.is_finite() { input_l } else { 0.0 };
        let input_r = if input_r.is_finite() { input_r } else { 0.0 };
        let mono_in = (input_l + input_r) * (0.015 * self.wet.clamp(0.1, 1.0) / 0.4); // scale proportional to wet level
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        for c in &mut self.combs_l { out_l += c.process(mono_in); }
        for c in &mut self.combs_r { out_r += c.process(mono_in); }
        for a in &mut self.allpass_l { out_l = a.process(out_l); }
        for a in &mut self.allpass_r { out_r = a.process(out_r); }
        // Sanitize: if reverb buffers are corrupted, reset them
        if !out_l.is_finite() || !out_r.is_finite() {
            for c in &mut self.combs_l { c.buf.iter_mut().for_each(|x| *x = 0.0); c.filter_store = 0.0; }
            for c in &mut self.combs_r { c.buf.iter_mut().for_each(|x| *x = 0.0); c.filter_store = 0.0; }
            for a in &mut self.allpass_l { a.buf.iter_mut().for_each(|x| *x = 0.0); }
            for a in &mut self.allpass_r { a.buf.iter_mut().for_each(|x| *x = 0.0); }
            return (input_l * (1.0 - self.wet), input_r * (1.0 - self.wet));
        }
        let dry = 1.0 - self.wet;
        (input_l * dry + out_l * self.wet, input_r * dry + out_r * self.wet)
    }
}
