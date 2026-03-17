/// Lookahead brickwall limiter.
/// Uses a simple peak follower with fast attack and slow release.
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

    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let l = if l.is_finite() { l.clamp(-10.0, 10.0) } else { 0.0 };
        let r = if r.is_finite() { r.clamp(-10.0, 10.0) } else { 0.0 };

        // Peak detection — reset envelope if it has gone NaN/inf
        if !self.envelope.is_finite() { self.envelope = 0.0; }
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
        // Fast attack (0.001 coeff), slow release (0.0001 coeff)
        let coeff = if target_gain < self.gain_smooth { 0.001 } else { 0.0001 };
        self.gain_smooth += coeff * (target_gain - self.gain_smooth);
        (dl * self.gain_smooth, dr * self.gain_smooth)
    }
}
