/// Three-band parametric EQ using biquad shelf and peak filters.
///
/// Band layout:
///   - Low shelf  : 200 Hz
///   - Mid peak   : configurable (default 1000 Hz)
///   - High shelf : 6000 Hz
///
/// All gains in dB (±12 dB range). At 0 dB each band is transparent.

use std::f32::consts::TAU;

/// A single biquad section (direct form II transposed).
#[derive(Clone)]
struct Biquad {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    z1: f32, z2: f32,
}

impl Biquad {
    fn unity() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, z1: 0.0, z2: 0.0 }
    }

    /// Low shelf filter (Audio EQ Cookbook, Robert Bristow-Johnson).
    fn low_shelf(freq_hz: f32, gain_db: f32, sample_rate: f32) -> Self {
        let a = 10.0f32.powf(gain_db / 40.0); // sqrt of linear gain
        let w0 = TAU * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let s = 1.0; // shelf slope (1.0 = maximally flat)
        let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();
        let b0 =       a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt());
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 =       a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * alpha * a.sqrt());
        let a0 =             (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt();
        let a1 = -2.0 *     ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 =             (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * alpha * a.sqrt();
        Self { b0: b0/a0, b1: b1/a0, b2: b2/a0, a1: a1/a0, a2: a2/a0, z1: 0.0, z2: 0.0 }
    }

    /// High shelf filter.
    fn high_shelf(freq_hz: f32, gain_db: f32, sample_rate: f32) -> Self {
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = TAU * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let s = 1.0;
        let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();
        let b0 =       a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt());
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 =       a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * alpha * a.sqrt());
        let a0 =             (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt();
        let a1 =  2.0 *     ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 =             (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * alpha * a.sqrt();
        Self { b0: b0/a0, b1: b1/a0, b2: b2/a0, a1: a1/a0, a2: a2/a0, z1: 0.0, z2: 0.0 }
    }

    /// Peaking EQ filter.
    fn peak(freq_hz: f32, gain_db: f32, q: f32, sample_rate: f32) -> Self {
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = TAU * freq_hz / sample_rate;
        let alpha = w0.sin() / (2.0 * q.max(0.1));
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * w0.cos();
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha / a;
        Self { b0: b0/a0, b1: b1/a0, b2: b2/a0, a1: a1/a0, a2: a2/a0, z1: 0.0, z2: 0.0 }
    }

    #[inline(always)]
    fn process(&mut self, x: f32) -> f32 {
        let x = if x.is_finite() { x } else { 0.0 };
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        if y.is_finite() { y } else { self.z1 = 0.0; self.z2 = 0.0; 0.0 }
    }
}

pub struct ThreeBandEq {
    low_shelf_l: Biquad,
    low_shelf_r: Biquad,
    mid_peak_l: Biquad,
    mid_peak_r: Biquad,
    high_shelf_l: Biquad,
    high_shelf_r: Biquad,
    pub low_gain_db: f32,   // ±12 dB
    pub mid_gain_db: f32,
    pub high_gain_db: f32,
    pub mid_freq: f32,      // default 1000 Hz
    sample_rate: f32,
}

impl ThreeBandEq {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            low_shelf_l: Biquad::unity(),
            low_shelf_r: Biquad::unity(),
            mid_peak_l: Biquad::unity(),
            mid_peak_r: Biquad::unity(),
            high_shelf_l: Biquad::unity(),
            high_shelf_r: Biquad::unity(),
            low_gain_db: 0.0,
            mid_gain_db: 0.0,
            high_gain_db: 0.0,
            mid_freq: 1000.0,
            sample_rate,
        }
    }

    /// Rebuild all biquad coefficients from current gain settings.
    pub fn update(&mut self) {
        let sr = self.sample_rate;
        let low = Biquad::low_shelf(200.0, self.low_gain_db.clamp(-12.0, 12.0), sr);
        // Q=2.5: ~2/3 octave bandwidth — surgical enough to sculpt specific
        // resonances without sounding mushy (Q=1.0 ≈ 2 octaves was too wide).
        let mid = Biquad::peak(self.mid_freq.clamp(200.0, sr * 0.45), self.mid_gain_db.clamp(-12.0, 12.0), 2.5, sr);
        let high = Biquad::high_shelf(6000.0, self.high_gain_db.clamp(-12.0, 12.0), sr);
        self.low_shelf_l = low.clone();
        self.low_shelf_r = low;
        self.mid_peak_l = mid.clone();
        self.mid_peak_r = mid;
        self.high_shelf_l = high.clone();
        self.high_shelf_r = high;
    }

    /// Process one stereo sample pair.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let l = self.high_shelf_l.process(self.mid_peak_l.process(self.low_shelf_l.process(l)));
        let r = self.high_shelf_r.process(self.mid_peak_r.process(self.low_shelf_r.process(r)));
        (l, r)
    }
}
