//! Audio effects processing: reverb, delay, chorus, distortion.
//!
//! Each effect implements the [`Effect`] trait. Effects can be composed into
//! an [`EffectChain`] that processes audio sequentially.

// ── Effect trait ──────────────────────────────────────────────────────────────

/// Common interface for all audio effects.
pub trait Effect {
    /// Process a block of mono audio samples and return the effected block.
    fn process(&self, input: &[f64], sample_rate: u32) -> Vec<f64>;
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Mix a dry and wet signal by `wet_amount` ∈ [0, 1].
pub fn mix(dry: &[f64], wet: &[f64], wet_amount: f64) -> Vec<f64> {
    let len = dry.len().min(wet.len());
    (0..len)
        .map(|i| dry[i] * (1.0 - wet_amount) + wet[i] * wet_amount)
        .collect()
}

// ── DelayEffect ───────────────────────────────────────────────────────────────

/// Comb-filter delay line.
///
/// * `delay_ms`  — delay time in milliseconds (≥ 0).
/// * `feedback`  — feedback coefficient ∈ [0, 1).
/// * `wet`       — wet/dry mix ∈ [0, 1].
#[derive(Debug, Clone)]
pub struct DelayEffect {
    pub delay_ms: f64,
    pub feedback: f64,
    pub wet: f64,
}

impl Default for DelayEffect {
    fn default() -> Self {
        Self { delay_ms: 250.0, feedback: 0.4, wet: 0.5 }
    }
}

impl Effect for DelayEffect {
    fn process(&self, input: &[f64], sample_rate: u32) -> Vec<f64> {
        let delay_samples = ((self.delay_ms / 1000.0) * sample_rate as f64).round() as usize;
        let delay_samples = delay_samples.max(1);
        let feedback = self.feedback.clamp(0.0, 0.99);

        let mut line = vec![0.0f64; delay_samples];
        let mut wet_out = Vec::with_capacity(input.len());
        let mut write = 0usize;

        for &x in input {
            let delayed = line[write % delay_samples];
            let new_val = x + feedback * delayed;
            line[write % delay_samples] = new_val;
            write += 1;
            wet_out.push(delayed);
        }

        mix(input, &wet_out, self.wet.clamp(0.0, 1.0))
    }
}

// ── ReverbEffect ──────────────────────────────────────────────────────────────

/// Simplified Schroeder reverb: 4 comb filters in parallel + 2 allpass filters in series.
///
/// * `room_size` — ∈ [0, 1]; scales comb filter delay lengths.
/// * `damping`   — ∈ [0, 1]; high-frequency damping coefficient.
/// * `wet`       — wet/dry mix ∈ [0, 1].
#[derive(Debug, Clone)]
pub struct ReverbEffect {
    pub room_size: f64,
    pub damping: f64,
    pub wet: f64,
}

impl Default for ReverbEffect {
    fn default() -> Self {
        Self { room_size: 0.5, damping: 0.5, wet: 0.3 }
    }
}

struct CombFilter {
    buf: Vec<f64>,
    pos: usize,
    feedback: f64,
    damp: f64,
    last: f64,
}

impl CombFilter {
    fn new(size: usize, feedback: f64, damp: f64) -> Self {
        Self { buf: vec![0.0; size.max(1)], pos: 0, feedback, damp, last: 0.0 }
    }

    fn process(&mut self, input: f64) -> f64 {
        let out = self.buf[self.pos];
        self.last = out * (1.0 - self.damp) + self.last * self.damp;
        self.buf[self.pos] = input + self.last * self.feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

struct AllpassFilter {
    buf: Vec<f64>,
    pos: usize,
    feedback: f64,
}

impl AllpassFilter {
    fn new(size: usize, feedback: f64) -> Self {
        Self { buf: vec![0.0; size.max(1)], pos: 0, feedback }
    }

    fn process(&mut self, input: f64) -> f64 {
        let delayed = self.buf[self.pos];
        let out = -input + delayed;
        self.buf[self.pos] = input + delayed * self.feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

impl Effect for ReverbEffect {
    fn process(&self, input: &[f64], sample_rate: u32) -> Vec<f64> {
        let sr = sample_rate as f64;
        let scale = self.room_size.clamp(0.1, 1.0);
        let damp = self.damping.clamp(0.0, 1.0);
        let fb = 0.84 * scale;

        // Comb filter delay lengths (samples) — classic Schroeder values scaled.
        let comb_sizes: [usize; 4] = [
            ((0.0297 * scale + 0.02) * sr) as usize,
            ((0.0371 * scale + 0.02) * sr) as usize,
            ((0.0411 * scale + 0.02) * sr) as usize,
            ((0.0437 * scale + 0.02) * sr) as usize,
        ];
        let allpass_sizes: [usize; 2] = [
            ((0.005 * sr) as usize).max(1),
            ((0.0017 * sr) as usize).max(1),
        ];

        let mut combs: Vec<CombFilter> = comb_sizes
            .iter()
            .map(|&s| CombFilter::new(s, fb, damp))
            .collect();
        let mut allpasses: Vec<AllpassFilter> = allpass_sizes
            .iter()
            .map(|&s| AllpassFilter::new(s, 0.5))
            .collect();

        let wet_out: Vec<f64> = input
            .iter()
            .map(|&x| {
                // Sum parallel comb filters.
                let comb_sum: f64 = combs.iter_mut().map(|c| c.process(x)).sum::<f64>() / 4.0;
                // Series allpass.
                let mut ap = comb_sum;
                for a in &mut allpasses {
                    ap = a.process(ap);
                }
                ap
            })
            .collect();

        mix(input, &wet_out, self.wet.clamp(0.0, 1.0))
    }
}

// ── ChorusEffect ──────────────────────────────────────────────────────────────

/// LFO-modulated delay (chorus / flanger).
///
/// * `rate_hz`  — LFO rate in Hz.
/// * `depth_ms` — maximum delay modulation depth in milliseconds.
/// * `wet`      — wet/dry mix ∈ [0, 1].
#[derive(Debug, Clone)]
pub struct ChorusEffect {
    pub rate_hz: f64,
    pub depth_ms: f64,
    pub wet: f64,
}

impl Default for ChorusEffect {
    fn default() -> Self {
        Self { rate_hz: 1.5, depth_ms: 7.0, wet: 0.5 }
    }
}

impl Effect for ChorusEffect {
    fn process(&self, input: &[f64], sample_rate: u32) -> Vec<f64> {
        let sr = sample_rate as f64;
        let base_delay_samples = (0.010 * sr) as usize; // 10 ms base delay
        let depth_samples = (self.depth_ms / 1000.0 * sr).max(0.0);
        let buf_len = (base_delay_samples + depth_samples as usize + 2).max(4);

        let mut buf = vec![0.0f64; buf_len];
        let mut write = 0usize;
        let lfo_inc = self.rate_hz / sr;
        let mut phase = 0.0f64;

        let wet_out: Vec<f64> = input
            .iter()
            .map(|&x| {
                buf[write % buf_len] = x;
                let lfo = (phase * std::f64::consts::TAU).sin();
                let delay = base_delay_samples as f64 + depth_samples * lfo;
                let delay_i = delay.floor() as usize;
                let frac = delay - delay.floor();
                let idx0 = (write + buf_len - delay_i) % buf_len;
                let idx1 = (write + buf_len - delay_i - 1) % buf_len;
                let out = buf[idx0] * (1.0 - frac) + buf[idx1] * frac;
                write = (write + 1) % buf_len;
                phase = (phase + lfo_inc).rem_euclid(1.0);
                out
            })
            .collect();

        mix(input, &wet_out, self.wet.clamp(0.0, 1.0))
    }
}

// ── DistortionEffect ──────────────────────────────────────────────────────────

/// Soft-clip distortion: `tanh(drive * x) * tone`.
///
/// * `drive` — gain before clipping (≥ 1.0 for audible distortion).
/// * `tone`  — output level scale ∈ (0, 1].
#[derive(Debug, Clone)]
pub struct DistortionEffect {
    pub drive: f64,
    pub tone: f64,
}

impl Default for DistortionEffect {
    fn default() -> Self {
        Self { drive: 3.0, tone: 0.7 }
    }
}

impl Effect for DistortionEffect {
    fn process(&self, input: &[f64], _sample_rate: u32) -> Vec<f64> {
        let drive = self.drive.max(0.001);
        let tone = self.tone.clamp(0.0, 1.0);
        input.iter().map(|&x| (drive * x).tanh() * tone).collect()
    }
}

// ── EffectChain ───────────────────────────────────────────────────────────────

/// An ordered chain of effects applied sequentially.
#[derive(Default)]
pub struct EffectChain {
    effects: Vec<Box<dyn Effect>>,
}

impl EffectChain {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an effect to the end of the chain.
    pub fn add(&mut self, effect: Box<dyn Effect>) {
        self.effects.push(effect);
    }

    /// Process audio through all effects in order.
    pub fn process(&self, input: &[f64], sample_rate: u32) -> Vec<f64> {
        let mut signal = input.to_vec();
        for effect in &self.effects {
            signal = effect.process(&signal, sample_rate);
        }
        signal
    }

    /// Number of effects in the chain.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 44_100;

    fn sine_block(freq: f64, len: usize) -> Vec<f64> {
        (0..len)
            .map(|i| (i as f64 * freq / SR as f64 * std::f64::consts::TAU).sin() * 0.5)
            .collect()
    }

    #[test]
    fn delay_output_length_matches() {
        let effect = DelayEffect::default();
        let input = sine_block(440.0, 1024);
        let out = effect.process(&input, SR);
        assert_eq!(out.len(), input.len());
    }

    #[test]
    fn delay_output_bounded() {
        let effect = DelayEffect { delay_ms: 50.0, feedback: 0.5, wet: 0.5 };
        let input = sine_block(440.0, 2048);
        let out = effect.process(&input, SR);
        assert!(out.iter().all(|&x| x.abs() < 2.0), "delay output out of bounds");
    }

    #[test]
    fn reverb_output_length_matches() {
        let effect = ReverbEffect::default();
        let input = sine_block(220.0, 4096);
        let out = effect.process(&input, SR);
        assert_eq!(out.len(), input.len());
    }

    #[test]
    fn reverb_output_non_empty() {
        let effect = ReverbEffect { room_size: 0.7, damping: 0.3, wet: 0.5 };
        let input = sine_block(440.0, 2048);
        let out = effect.process(&input, SR);
        assert!(!out.is_empty());
        assert!(out.iter().any(|&x| x.abs() > 1e-9), "reverb output is all zeros");
    }

    #[test]
    fn chorus_output_length_matches() {
        let effect = ChorusEffect::default();
        let input = sine_block(330.0, 2048);
        let out = effect.process(&input, SR);
        assert_eq!(out.len(), input.len());
    }

    #[test]
    fn chorus_output_bounded() {
        let effect = ChorusEffect { rate_hz: 2.0, depth_ms: 5.0, wet: 0.5 };
        let input = sine_block(440.0, 4096);
        let out = effect.process(&input, SR);
        assert!(out.iter().all(|&x| x.abs() <= 2.0));
    }

    #[test]
    fn distortion_output_length_matches() {
        let effect = DistortionEffect::default();
        let input = sine_block(440.0, 512);
        let out = effect.process(&input, SR);
        assert_eq!(out.len(), input.len());
    }

    #[test]
    fn distortion_soft_clip() {
        let effect = DistortionEffect { drive: 10.0, tone: 1.0 };
        let input: Vec<f64> = vec![-5.0, -1.0, 0.0, 1.0, 5.0];
        let out = effect.process(&input, SR);
        // tanh saturates to ≤ 1.0
        assert!(out.iter().all(|&x| x.abs() <= 1.01));
    }

    #[test]
    fn effect_chain_processes_all() {
        let mut chain = EffectChain::new();
        chain.add(Box::new(DelayEffect::default()));
        chain.add(Box::new(DistortionEffect::default()));
        assert_eq!(chain.len(), 2);
        let input = sine_block(440.0, 1024);
        let out = chain.process(&input, SR);
        assert_eq!(out.len(), input.len());
    }

    #[test]
    fn effect_chain_empty_passthrough() {
        let chain = EffectChain::new();
        let input: Vec<f64> = vec![0.1, 0.2, 0.3];
        let out = chain.process(&input, SR);
        assert_eq!(out, input);
    }

    #[test]
    fn mix_blend() {
        let dry = vec![1.0, 1.0];
        let wet = vec![0.0, 0.0];
        let out = mix(&dry, &wet, 0.5);
        assert_eq!(out, vec![0.5, 0.5]);
    }

    #[test]
    fn mix_full_wet() {
        let dry = vec![1.0, 1.0];
        let wet = vec![2.0, 2.0];
        let out = mix(&dry, &wet, 1.0);
        assert_eq!(out, vec![2.0, 2.0]);
    }
}
