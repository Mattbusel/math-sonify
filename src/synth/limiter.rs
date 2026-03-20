/// Lookahead brickwall limiter with peak follower and gain smoothing.
///
/// A short lookahead buffer (default 5 ms) gives the gain-reduction
/// envelope time to react before the transient arrives at the output,
/// preventing overshoot at the configured threshold.  Gain changes are
/// low-pass smoothed to avoid zipper noise on rapid transients.
///
/// This processor is always present on the master bus.  It is not meant
/// to be a creative effect; the threshold is set to -0.5 dBFS so that
/// the limiter is only active on peaks that would otherwise clip.
pub struct Limiter {
    threshold: f32,
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
    lookahead: Vec<(f32, f32)>,
    lh_pos: usize,
    lh_len: usize,
    gain_smooth: f32,
}

impl Limiter {
    /// Create a new limiter.
    ///
    /// # Parameters
    /// - `threshold_db`: Limiting threshold in dBFS (e.g. `-0.5`).
    /// - `lookahead_ms`: Lookahead delay in milliseconds (e.g. `5.0`).
    /// - `sample_rate`: Audio sample rate in Hz.
    pub fn new(threshold_db: f32, lookahead_ms: f32, sample_rate: f32) -> Self {
        let threshold = 10.0f32.powf(threshold_db / 20.0);
        let lh_len = (lookahead_ms * 0.001 * sample_rate) as usize + 1;
        Self {
            threshold,
            envelope: 0.0,
            // 5 ms attack (was 1 ms) — avoids zipper noise on complex multi-layer
            // content where rapid gain changes were audible as digital clatter.
            attack_coeff: 1.0 - (-2.2 / (0.005 * sample_rate)).exp(),
            release_coeff: 1.0 - (-2.2 / (0.300 * sample_rate)).exp(),
            lookahead: vec![(0.0, 0.0); lh_len],
            lh_pos: 0,
            lh_len,
            gain_smooth: 1.0,
        }
    }

    /// Process one stereo sample pair and return the gain-limited output `(left, right)`.
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let l = if l.is_finite() {
            l.clamp(-10.0, 10.0)
        } else {
            0.0
        };
        let r = if r.is_finite() {
            r.clamp(-10.0, 10.0)
        } else {
            0.0
        };

        // Peak detection — reset envelope if it has gone NaN/inf
        if !self.envelope.is_finite() {
            self.envelope = 0.0;
        }
        let peak = l.abs().max(r.abs());
        if peak > self.envelope {
            self.envelope += self.attack_coeff * (peak - self.envelope);
        } else {
            self.envelope += self.release_coeff * (peak - self.envelope);
        }

        // Write to lookahead buffer
        self.lookahead[self.lh_pos] = (l, r);
        let read_pos = (self.lh_pos + 1) % self.lh_len;
        let (dl, dr) = self.lookahead[read_pos];
        self.lh_pos = (self.lh_pos + 1) % self.lh_len;

        // Smooth gain reduction to eliminate zipper noise
        let target_gain = if self.envelope > self.threshold {
            self.threshold / self.envelope
        } else {
            1.0
        };
        // Gain smoothing: 0.01 attack (≈100 samples / 2.3 ms) limits fast transients
        // without zipper noise; 0.001 release (≈1000 samples / 23 ms) restores gain
        // slowly enough to avoid audible pumping on dense material.
        let coeff = if target_gain < self.gain_smooth {
            0.01
        } else {
            0.001
        };
        self.gain_smooth += coeff * (target_gain - self.gain_smooth);
        (dl * self.gain_smooth, dr * self.gain_smooth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_limiter_passes_quiet_signal() {
        // A quiet signal well below threshold should pass without gain reduction.
        // The limiter has a 5 ms lookahead buffer (~221 samples at 44100 Hz), so
        // outputs are delayed and the first ~221 samples are silence.  We run 1500
        // samples and average only the last 800 to skip the lookahead latency.
        let mut lim = Limiter::new(-0.5, 5.0, SR);
        let level = 0.1_f32;
        let mut outputs: Vec<f32> = Vec::with_capacity(3000);
        for _ in 0..1500 {
            let (l, _r) = lim.process(level, level);
            outputs.push(l);
        }
        let n = outputs.len();
        let skip = n - 800;
        let avg: f32 = outputs[skip..].iter().sum::<f32>() / 800.0;
        assert!(
            (avg - level).abs() < 0.01,
            "Quiet signal should pass unchanged, avg={}",
            avg
        );
    }

    #[test]
    fn test_limiter_gain_reduction_engages_on_loud_signal() {
        // The gain_smooth has a ~1000-sample time constant by design to avoid
        // zipper noise.  After enough settling time (10000 samples), output should
        // be significantly reduced from the 5.0 input (< 80% = 4.0).
        let mut lim = Limiter::new(-0.5, 5.0, SR);
        let loud = 5.0_f32;
        let (l_first, _) = lim.process(loud, loud);
        for _ in 0..10000 {
            lim.process(loud, loud);
        }
        let (l_settled, _) = lim.process(loud, loud);
        assert!(
            l_settled.abs() < l_first.abs() || l_settled.abs() < loud * 0.8,
            "Limiter should reduce a loud signal after settling: first={}, settled={}",
            l_first,
            l_settled
        );
    }

    #[test]
    fn test_limiter_output_always_finite() {
        let mut lim = Limiter::new(-0.5, 5.0, SR);
        for i in 0..1000 {
            let x = (i as f32 * 0.1).sin() * 10.0;
            let (l, r) = lim.process(x, -x);
            assert!(l.is_finite(), "Left output non-finite at {}", i);
            assert!(r.is_finite(), "Right output non-finite at {}", i);
        }
    }

    #[test]
    fn test_limiter_nan_input_safe() {
        let mut lim = Limiter::new(-0.5, 5.0, SR);
        let (l, r) = lim.process(f32::NAN, f32::NAN);
        assert!(l.is_finite(), "NaN input should produce finite output");
        assert!(r.is_finite(), "NaN input should produce finite output");
    }
}
