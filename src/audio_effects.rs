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

// ── New effect building blocks (Round 27) ─────────────────────────────────────

/// A simple circular delay buffer.
pub struct DelayLine {
    pub buffer: Vec<f64>,
    pub write_pos: usize,
    pub max_size: usize,
}

impl DelayLine {
    pub fn new(max_delay_samples: usize) -> Self {
        let max_size = max_delay_samples.max(1);
        DelayLine {
            buffer: vec![0.0; max_size],
            write_pos: 0,
            max_size,
        }
    }

    /// Write `input` at the current write position and read back `delay_samples` ago.
    pub fn push_and_read(&mut self, input: f64, delay_samples: usize) -> f64 {
        let delay_samples = delay_samples.min(self.max_size - 1);
        self.buffer[self.write_pos] = input;
        let read_pos = (self.write_pos + self.max_size - delay_samples) % self.max_size;
        let out = self.buffer[read_pos];
        self.write_pos = (self.write_pos + 1) % self.max_size;
        out
    }

    /// Read a sample `delay_samples` behind the current write head without advancing.
    pub fn read_at(&self, delay_samples: usize) -> f64 {
        let delay_samples = delay_samples.min(self.max_size - 1);
        let read_pos = (self.write_pos + self.max_size - delay_samples - 1) % self.max_size;
        self.buffer[read_pos]
    }
}

/// Flanger effect (very short modulated delay with feedback).
pub struct FlangerEffect {
    pub delay_ms: f64,
    pub depth_ms: f64,
    pub rate_hz: f64,
    pub feedback: f64,
    pub mix: f64,
    delay_line: DelayLine,
    phase: f64,
    last_out: f64,
}

impl FlangerEffect {
    pub fn new(delay_ms: f64, depth_ms: f64, rate_hz: f64, feedback: f64, mix: f64) -> Self {
        FlangerEffect {
            delay_ms,
            depth_ms,
            rate_hz,
            feedback,
            mix,
            delay_line: DelayLine::new(4096),
            phase: 0.0,
            last_out: 0.0,
        }
    }

    pub fn process(&mut self, input: &[f64], sample_rate: f64) -> Vec<f64> {
        let base_delay = (self.delay_ms * 0.001 * sample_rate) as usize;
        let depth_samp = (self.depth_ms * 0.001 * sample_rate) as f64;
        let lfo_inc = self.rate_hz / sample_rate;

        input.iter().map(|&x| {
            let lfo = (self.phase * 2.0 * std::f64::consts::PI).sin();
            let delay = (base_delay as f64 + depth_samp * lfo) as usize;
            let delayed = self.delay_line.push_and_read(x + self.last_out * self.feedback, delay.max(1));
            self.last_out = delayed;
            self.phase = (self.phase + lfo_inc).rem_euclid(1.0);
            x * (1.0 - self.mix) + delayed * self.mix
        }).collect()
    }
}

/// Echo effect: single-tap delay with feedback.
pub struct EchoEffect {
    pub delay_ms: f64,
    pub feedback: f64,
    pub mix: f64,
    delay_line: DelayLine,
}

impl EchoEffect {
    pub fn new(delay_ms: f64, feedback: f64, mix: f64) -> Self {
        EchoEffect {
            delay_ms,
            feedback,
            mix,
            delay_line: DelayLine::new(192000),
        }
    }

    pub fn process(&mut self, input: &[f64], sample_rate: f64) -> Vec<f64> {
        let delay_samples = (self.delay_ms * 0.001 * sample_rate) as usize;
        let delay_samples = delay_samples.max(1);
        let fb = self.feedback.clamp(0.0, 0.99);

        input.iter().map(|&x| {
            let delayed = self.delay_line.push_and_read(x, delay_samples);
            // Feed echo back into delay line
            let write_idx = (self.delay_line.write_pos + self.delay_line.max_size - 1) % self.delay_line.max_size;
            self.delay_line.buffer[write_idx] += delayed * fb;
            x * (1.0 - self.mix) + delayed * self.mix
        }).collect()
    }
}

/// New-API chorus effect with multi-voice support and `&mut self`.
pub struct MultiChorusEffect {
    pub delay_ms: f64,
    pub depth_ms: f64,
    pub rate_hz: f64,
    pub mix: f64,
    pub num_voices: u32,
}

impl MultiChorusEffect {
    pub fn new(delay_ms: f64, depth_ms: f64, rate_hz: f64, mix: f64, num_voices: u32) -> Self {
        MultiChorusEffect { delay_ms, depth_ms, rate_hz, mix, num_voices }
    }

    pub fn lfo(time_s: f64, rate_hz: f64, phase: f64) -> f64 {
        (2.0 * std::f64::consts::PI * (rate_hz * time_s + phase)).sin()
    }

    pub fn process(&mut self, input: &[f64], sample_rate: f64, time_offset: f64) -> Vec<f64> {
        let base_delay = (self.delay_ms * 0.001 * sample_rate) as usize;
        let depth_samp = self.depth_ms * 0.001 * sample_rate;
        let n_voices = self.num_voices.max(1);
        let buf_len = (base_delay + depth_samp as usize + 4).max(8);
        let mut voices: Vec<Vec<f64>> = Vec::new();

        for v in 0..n_voices {
            let phase = v as f64 / n_voices as f64;
            let mut buf = vec![0.0f64; buf_len];
            let mut write = 0usize;
            let voice_out: Vec<f64> = input.iter().enumerate().map(|(i, &x)| {
                buf[write % buf_len] = x;
                let t = time_offset + i as f64 / sample_rate;
                let lfo = Self::lfo(t, self.rate_hz, phase);
                let delay = base_delay as f64 + depth_samp * lfo;
                let di = delay.floor() as usize;
                let frac = delay - delay.floor();
                let i0 = (write + buf_len - di) % buf_len;
                let i1 = (write + buf_len - di - 1) % buf_len;
                let out = buf[i0] * (1.0 - frac) + buf[i1] * frac;
                write += 1;
                out
            }).collect();
            voices.push(voice_out);
        }

        // Mix voices
        input.iter().enumerate().map(|(i, &x)| {
            let wet: f64 = voices.iter().map(|v| v[i]).sum::<f64>() / n_voices as f64;
            x * (1.0 - self.mix) + wet * self.mix
        }).collect()
    }
}

/// New Schroeder reverb with explicit `&mut self` process method.
pub struct NewReverbEffect {
    pub room_size: f64,
    pub damping: f64,
    pub wet: f64,
    pub dry: f64,
    comb_bufs: [Vec<f64>; 4],
    comb_pos: [usize; 4],
    comb_last: [f64; 4],
    ap_bufs: [Vec<f64>; 2],
    ap_pos: [usize; 2],
}

impl NewReverbEffect {
    /// Comb filter delay lengths at 44100 Hz (scaled by sample_rate / 44100 at runtime).
    const COMB_DELAYS_44K: [usize; 4] = [1116, 1188, 1277, 1356];
    const AP_DELAYS_44K: [usize; 2] = [225, 556];

    pub fn new(room_size: f64, damping: f64, wet: f64, dry: f64) -> Self {
        let make_buf = |d: usize| vec![0.0f64; d.max(1)];
        NewReverbEffect {
            room_size,
            damping,
            wet,
            dry,
            comb_bufs: [
                make_buf(Self::COMB_DELAYS_44K[0]),
                make_buf(Self::COMB_DELAYS_44K[1]),
                make_buf(Self::COMB_DELAYS_44K[2]),
                make_buf(Self::COMB_DELAYS_44K[3]),
            ],
            comb_pos: [0; 4],
            comb_last: [0.0; 4],
            ap_bufs: [
                make_buf(Self::AP_DELAYS_44K[0]),
                make_buf(Self::AP_DELAYS_44K[1]),
            ],
            ap_pos: [0; 2],
        }
    }

    pub fn comb_filter(
        input: f64,
        buffer: &mut Vec<f64>,
        pos: &mut usize,
        delay: usize,
        feedback: f64,
        damping: f64,
    ) -> f64 {
        let len = buffer.len().max(1);
        let idx = *pos % len;
        let out = buffer[idx];
        let filtered = out * (1.0 - damping);
        buffer[idx] = input + filtered * feedback;
        *pos = (idx + 1) % len;
        let _ = delay;
        out
    }

    pub fn all_pass(
        input: f64,
        buffer: &mut Vec<f64>,
        pos: &mut usize,
        _delay: usize,
    ) -> f64 {
        let len = buffer.len().max(1);
        let idx = *pos % len;
        let delayed = buffer[idx];
        let out = -input + delayed;
        buffer[idx] = input + delayed * 0.5;
        *pos = (idx + 1) % len;
        out
    }

    pub fn process(&mut self, input: &[f64], _sample_rate: f64) -> Vec<f64> {
        let scale = self.room_size.clamp(0.1, 1.0);
        let fb = 0.84 * scale;
        let damp = self.damping.clamp(0.0, 1.0);

        input.iter().map(|&x| {
            // 4 parallel comb filters
            let mut comb_sum = 0.0;
            for k in 0..4 {
                let delay = self.comb_bufs[k].len();
                let out = self.comb_bufs[k][self.comb_pos[k] % delay];
                self.comb_last[k] = out * (1.0 - damp);
                self.comb_bufs[k][self.comb_pos[k] % delay] = x + self.comb_last[k] * fb;
                self.comb_pos[k] = (self.comb_pos[k] + 1) % delay;
                comb_sum += out;
            }
            comb_sum /= 4.0;

            // 2 series all-pass filters
            let mut ap = comb_sum;
            for k in 0..2 {
                let delay = self.ap_bufs[k].len();
                let delayed = self.ap_bufs[k][self.ap_pos[k] % delay];
                let out = -ap + delayed;
                self.ap_bufs[k][self.ap_pos[k] % delay] = ap + delayed * 0.5;
                self.ap_pos[k] = (self.ap_pos[k] + 1) % delay;
                ap = out;
            }

            x * self.dry + ap * self.wet
        }).collect()
    }
}

/// A trait for mutable effects (Round 27 variant).
pub trait MutableEffect: Send {
    fn process(&mut self, input: &[f64], sample_rate: f64) -> Vec<f64>;
    fn name(&self) -> &str;
}

impl MutableEffect for MultiChorusEffect {
    fn process(&mut self, input: &[f64], sample_rate: f64) -> Vec<f64> {
        self.process(input, sample_rate, 0.0)
    }
    fn name(&self) -> &str { "multi_chorus" }
}

impl MutableEffect for EchoEffect {
    fn process(&mut self, input: &[f64], sample_rate: f64) -> Vec<f64> {
        self.process(input, sample_rate)
    }
    fn name(&self) -> &str { "echo" }
}

impl MutableEffect for NewReverbEffect {
    fn process(&mut self, input: &[f64], sample_rate: f64) -> Vec<f64> {
        self.process(input, sample_rate)
    }
    fn name(&self) -> &str { "reverb" }
}

impl MutableEffect for FlangerEffect {
    fn process(&mut self, input: &[f64], sample_rate: f64) -> Vec<f64> {
        self.process(input, sample_rate)
    }
    fn name(&self) -> &str { "flanger" }
}

/// Mutable effects chain (Round 27).
pub struct EffectsChain {
    effects: Vec<Box<dyn MutableEffect>>,
}

impl EffectsChain {
    pub fn new() -> Self {
        EffectsChain { effects: Vec::new() }
    }

    pub fn add_chorus(&mut self, params: MultiChorusEffect) {
        self.effects.push(Box::new(params));
    }

    pub fn add_echo(&mut self, params: EchoEffect) {
        self.effects.push(Box::new(params));
    }

    pub fn add_reverb(&mut self, params: NewReverbEffect) {
        self.effects.push(Box::new(params));
    }

    pub fn process_chain(&mut self, input: &[f64], sample_rate: f64) -> Vec<f64> {
        let mut signal = input.to_vec();
        for effect in self.effects.iter_mut() {
            signal = effect.process(&signal, sample_rate);
        }
        signal
    }
}

impl Default for EffectsChain {
    fn default() -> Self { Self::new() }
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
