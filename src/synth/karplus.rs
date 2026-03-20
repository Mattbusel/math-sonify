//! Karplus-Strong plucked string synthesis.
//!
//! Improvements over the naive two-point average:
//!
//! 1. **First-order IIR loop filter** — replaces `(a+b)*0.5`.  The coefficient
//!    `b` (0 = bright steel, 0.5 = classical guitar, 0.9 = bass/cello dark) lets
//!    the simulation model different string/body characteristics.
//!
//! 2. **Allpass stretch** — a first-order allpass in the delay loop adds slight
//!    inharmonicity (stiffness), giving the characteristic piano-like pitch
//!    "bloom" where upper partials go slightly sharp.  Set to 0 for an ideal
//!    string.
//!
//! 3. **Fractional delay** — integer rounding of delay length causes pitch
//!    quantisation errors; linear interpolation gives exact intonation at all
//!    frequencies.

pub struct KarplusStrong {
    buf: Vec<f32>,
    write: usize,
    pub decay: f32,      // per-loop gain (0 < decay < 1)
    pub brightness: f32, // IIR coeff: 0 = bright, 0.5 = balanced, 0.85 = dark
    pub stretch: f32,    // allpass coefficient for stiffness (0 = none)
    pub active: bool,
    pub volume: f32,
    length_f: f32, // fractional delay line length (samples)
    // Filter states
    lp_state: f32,
    ap_state: f32,
}

impl KarplusStrong {
    /// Create a new Karplus-Strong synthesizer.
    ///
    /// # Parameters
    /// - `max_freq_hz`: Minimum expected note frequency (sets the buffer size).
    ///   Lower values require a larger delay line; 20 Hz covers the full audio range.
    /// - `sample_rate`: Audio sample rate in Hz.
    pub fn new(max_freq_hz: f32, sample_rate: f32) -> Self {
        let max_len = (sample_rate / max_freq_hz) as usize + 4;
        Self {
            buf: vec![0.0; max_len],
            write: 0,
            decay: 0.996,
            brightness: 0.45,
            stretch: 0.06,
            active: false,
            volume: 0.5,
            length_f: (sample_rate / 220.0).clamp(2.0, max_len as f32 - 2.0),
            lp_state: 0.0,
            ap_state: 0.0,
        }
    }

    /// Trigger a new note at the given frequency.
    pub fn trigger(&mut self, freq: f32, sample_rate: f32) {
        self.length_f = (sample_rate / freq.max(20.0)).clamp(2.0, self.buf.len() as f32 - 2.0);
        let len = self.length_f as usize;
        // Seed with high-resolution nanoseconds so two voices triggered at the same
        // time (or same write position) produce genuinely different excitations.
        let ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(self.write as u64 + 1);
        let mut rng = self.write as u64 ^ ns ^ 0xDEAD_BEEF_CAFE_BABE;
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        for i in 0..len {
            rng = rng
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            // >> 33 yields 31 bits; divide by 2^31 for unbiased [-1, 1) range.
            // Dividing by u32::MAX (≈ 2^32) would cap the max at ~0.5, producing
            // DC-biased initial excitation that clicks at note onset.
            self.buf[i] = (rng >> 33) as f32 / (1u64 << 31) as f32 * 2.0 - 1.0;
        }
        for i in len..self.buf.len() {
            self.buf[i] = 0.0;
        }
        // Start the write pointer AFTER the noise so the first next_sample() read
        // lands on buf[0] (the noise) rather than the empty tail.  With write=0,
        // the first read would be from buf[buf.len()-len] which is in the cleared
        // region, producing silence and immediately tripping the inactivity gate.
        self.write = len % self.buf.len();
        self.lp_state = 0.0;
        self.ap_state = 0.0;
        self.active = true;
    }

    /// Advance the string model by one sample and return the output.
    ///
    /// Returns 0.0 when the string has decayed to inaudible levels.
    pub fn next_sample(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        let len = self.buf.len();
        let delay = self.length_f;

        // Fractional read with linear interpolation
        let d0 = delay as usize;
        let frac = delay - d0 as f32;
        let i0 = (self.write + len - d0.min(len - 1)) % len;
        let i1 = (self.write + len - (d0 + 1).min(len - 1)) % len;
        let read = self.buf[i0] * (1.0 - frac) + self.buf[i1] * frac;

        // IIR lowpass loop filter
        let b = self.brightness;
        self.lp_state = (1.0 - b) * read + b * self.lp_state;

        // Allpass dispersion (stiffness)
        let c = self.stretch;
        let ap_out = c * (self.lp_state - self.ap_state) + self.ap_state;
        self.ap_state = self.lp_state;

        // Write back with damping
        let fed = ap_out * self.decay;
        self.buf[self.write] = fed;
        self.write = (self.write + 1) % len;

        // Silence detection — avoid running forever at inaudible levels
        if fed.abs() < 1e-6 {
            self.active = false;
        }

        read * self.volume
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_karplus_silent_before_trigger() {
        let mut ks = KarplusStrong::new(20.0, SR);
        assert!(!ks.active, "Should be inactive before trigger");
        let s = ks.next_sample();
        assert_eq!(s, 0.0, "Should be silent before trigger");
    }

    #[test]
    fn test_karplus_produces_output_after_trigger() {
        let mut ks = KarplusStrong::new(20.0, SR);
        ks.trigger(440.0, SR);
        assert!(ks.active, "Should be active after trigger");
        let mut max_abs = 0.0_f32;
        for _ in 0..4410 {
            let s = ks.next_sample();
            max_abs = max_abs.max(s.abs());
        }
        assert!(max_abs > 0.0, "Triggered string should produce output");
    }

    #[test]
    fn test_karplus_output_finite() {
        let mut ks = KarplusStrong::new(20.0, SR);
        ks.trigger(440.0, SR);
        for i in 0..22050 {
            let s = ks.next_sample();
            assert!(s.is_finite(), "Output non-finite at sample {}", i);
        }
    }

    #[test]
    fn test_karplus_decays_to_silence() {
        // With default decay=0.996 at 440 Hz (~100 sample delay), the string needs
        // ~3450 loop passes to reach 1e-6, or about 345000 samples (~7.8 s).
        // Run 10 seconds to cover the full decay with margin.
        let mut ks = KarplusStrong::new(20.0, SR);
        ks.trigger(440.0, SR);
        for _ in 0..SR as usize * 10 {
            ks.next_sample();
        }
        assert!(!ks.active, "String should decay to silence");
    }

    #[test]
    fn test_karplus_higher_freq_shorter_delay() {
        // Higher frequency → shorter delay line length
        let mut ks_low = KarplusStrong::new(20.0, SR);
        ks_low.trigger(220.0, SR);
        let len_low = ks_low.length_f;

        let mut ks_high = KarplusStrong::new(20.0, SR);
        ks_high.trigger(880.0, SR);
        let len_high = ks_high.length_f;

        assert!(
            len_high < len_low,
            "Higher frequency should give shorter delay: 220={}, 880={}",
            len_low,
            len_high
        );
    }

    #[test]
    fn test_karplus_low_decay_fades_faster() {
        // Lower decay coefficient → string fades faster
        let mut ks_fast = KarplusStrong::new(20.0, SR);
        ks_fast.decay = 0.9; // fast decay
        ks_fast.trigger(440.0, SR);
        let mut rms_fast = 0.0_f32;
        for _ in 0..4410 {
            let s = ks_fast.next_sample();
            rms_fast += s * s;
        }

        let mut ks_slow = KarplusStrong::new(20.0, SR);
        ks_slow.decay = 0.999; // slow decay
        ks_slow.trigger(440.0, SR);
        let mut rms_slow = 0.0_f32;
        for _ in 0..4410 {
            let s = ks_slow.next_sample();
            rms_slow += s * s;
        }

        assert!(
            rms_fast < rms_slow,
            "Lower decay should produce lower RMS: fast={}, slow={}",
            rms_fast,
            rms_slow
        );
    }

    #[test]
    fn test_karplus_retrigger_reactivates() {
        let mut ks = KarplusStrong::new(20.0, SR);
        ks.trigger(440.0, SR);
        // Let it fully decay
        for _ in 0..SR as usize * 10 {
            ks.next_sample();
        }
        assert!(!ks.active, "Should be inactive after full decay");
        // Retrigger at a different frequency
        ks.trigger(880.0, SR);
        assert!(ks.active, "Should be active after retrigger");
        let mut max_abs = 0.0_f32;
        for _ in 0..4410 {
            max_abs = max_abs.max(ks.next_sample().abs());
        }
        assert!(max_abs > 0.0, "Retriggered string should produce output");
    }
}
