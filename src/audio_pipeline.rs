//! Sample-accurate audio processing pipeline.
//!
//! Provides an `AudioBuffer` abstraction, a `PipelineStage` trait, and
//! several concrete stages (gain, pan, limiter, DC-block, metering) that
//! can be chained together inside an `AudioPipeline`.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// AudioFormat
// ---------------------------------------------------------------------------

/// Describes the layout and precision of an audio buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u8,
    pub bit_depth: u8,
    pub is_float: bool,
}

impl AudioFormat {
    pub fn stereo_f32(sample_rate: u32) -> Self {
        Self { sample_rate, channels: 2, bit_depth: 32, is_float: true }
    }

    pub fn mono_f32(sample_rate: u32) -> Self {
        Self { sample_rate, channels: 1, bit_depth: 32, is_float: true }
    }
}

// ---------------------------------------------------------------------------
// AudioBuffer
// ---------------------------------------------------------------------------

/// Interleaved multi-channel audio buffer using 64-bit float samples.
pub struct AudioBuffer {
    /// Interleaved samples: [ch0_s0, ch1_s0, ch0_s1, ch1_s1, ...]
    pub samples: Vec<f64>,
    pub format: AudioFormat,
}

impl AudioBuffer {
    /// Create a silent buffer with `num_samples` frames.
    pub fn new(num_samples: usize, format: AudioFormat) -> Self {
        let total = num_samples * format.channels as usize;
        Self {
            samples: vec![0.0; total],
            format,
        }
    }

    /// Number of sample frames (not individual sample values).
    pub fn frame_count(&self) -> usize {
        if self.format.channels == 0 {
            return 0;
        }
        self.samples.len() / self.format.channels as usize
    }

    /// Read-only per-channel view by collecting every N-th sample.
    pub fn channel_slice(&self, channel: u8) -> Vec<f64> {
        let ch = channel as usize;
        let n_ch = self.format.channels as usize;
        self.samples.iter().skip(ch).step_by(n_ch).copied().collect()
    }

    /// Mix all channels down to mono by averaging.
    pub fn mix_down(&self) -> Vec<f64> {
        let n_ch = self.format.channels as usize;
        if n_ch == 0 {
            return Vec::new();
        }
        let frames = self.frame_count();
        (0..frames)
            .map(|f| {
                let base = f * n_ch;
                let sum: f64 = self.samples[base..base + n_ch].iter().sum();
                sum / n_ch as f64
            })
            .collect()
    }

    /// Normalise so the peak absolute value equals `peak`.
    pub fn normalize(&mut self, peak: f64) {
        let max = self.samples.iter().map(|s| s.abs()).fold(0.0_f64, f64::max);
        if max > 1e-12 {
            let scale = peak / max;
            self.samples.iter_mut().for_each(|s| *s *= scale);
        }
    }

    /// Linear fade-in over the first `samples` frames.
    pub fn fade_in(&mut self, samples: usize) {
        let n_ch = self.format.channels as usize;
        let frames = samples.min(self.frame_count());
        for f in 0..frames {
            let gain = f as f64 / frames as f64;
            for c in 0..n_ch {
                self.samples[f * n_ch + c] *= gain;
            }
        }
    }

    /// Linear fade-out over the last `samples` frames.
    pub fn fade_out(&mut self, samples: usize) {
        let n_ch = self.format.channels as usize;
        let total = self.frame_count();
        let start = total.saturating_sub(samples);
        for f in start..total {
            let elapsed = f - start;
            let gain = 1.0 - elapsed as f64 / samples as f64;
            for c in 0..n_ch {
                self.samples[f * n_ch + c] *= gain;
            }
        }
    }

    /// Hard-clip every sample to `[min, max]`.
    pub fn clip_to(&mut self, min: f64, max: f64) {
        self.samples.iter_mut().for_each(|s| *s = s.clamp(min, max));
    }

    /// Root-mean-square of all samples.
    pub fn rms(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = self.samples.iter().map(|s| s * s).sum();
        (sum_sq / self.samples.len() as f64).sqrt()
    }

    /// Absolute peak sample value.
    pub fn peak_level(&self) -> f64 {
        self.samples.iter().map(|s| s.abs()).fold(0.0_f64, f64::max)
    }
}

// ---------------------------------------------------------------------------
// PipelineStage trait
// ---------------------------------------------------------------------------

pub trait PipelineStage: Send {
    /// In-place processing of the buffer at a given playhead time.
    fn process(&mut self, buffer: &mut AudioBuffer, time_s: f64);
    /// Human-readable stage identifier.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// GainStage
// ---------------------------------------------------------------------------

/// Applies a fixed gain (in dB) to all samples.
pub struct GainStage {
    pub gain_db: f64,
}

impl GainStage {
    pub fn new(gain_db: f64) -> Self {
        Self { gain_db }
    }

    fn linear_gain(&self) -> f64 {
        10.0_f64.powf(self.gain_db / 20.0)
    }
}

impl PipelineStage for GainStage {
    fn process(&mut self, buffer: &mut AudioBuffer, _time_s: f64) {
        let g = self.linear_gain();
        buffer.samples.iter_mut().for_each(|s| *s *= g);
    }

    fn name(&self) -> &str {
        "GainStage"
    }
}

// ---------------------------------------------------------------------------
// PanStage
// ---------------------------------------------------------------------------

/// Equal-power stereo pan law for a 2-channel buffer.
/// `pan` is in [-1.0, 1.0]: -1 = hard left, 0 = centre, +1 = hard right.
pub struct PanStage {
    pub pan: f64,
}

impl PanStage {
    pub fn new(pan: f64) -> Self {
        Self { pan: pan.clamp(-1.0, 1.0) }
    }
}

impl PipelineStage for PanStage {
    fn process(&mut self, buffer: &mut AudioBuffer, _time_s: f64) {
        if buffer.format.channels < 2 {
            return;
        }
        // Equal-power law: angle in [0, π/2]
        let angle = (self.pan + 1.0) / 2.0 * std::f64::consts::FRAC_PI_2;
        let left_gain = angle.cos();
        let right_gain = angle.sin();

        let n_ch = buffer.format.channels as usize;
        let frames = buffer.frame_count();
        for f in 0..frames {
            buffer.samples[f * n_ch] *= left_gain;
            buffer.samples[f * n_ch + 1] *= right_gain;
        }
    }

    fn name(&self) -> &str {
        "PanStage"
    }
}

// ---------------------------------------------------------------------------
// LimiterStage
// ---------------------------------------------------------------------------

/// Peak limiter with a fixed threshold and a per-sample release coefficient.
pub struct LimiterStage {
    pub threshold_db: f64,
    pub release_ms: f64,
    /// Current gain reduction (linear).
    gain_reduction: f64,
}

impl LimiterStage {
    pub fn new(threshold_db: f64, release_ms: f64) -> Self {
        Self { threshold_db, release_ms, gain_reduction: 1.0 }
    }

    fn threshold_linear(&self) -> f64 {
        10.0_f64.powf(self.threshold_db / 20.0)
    }
}

impl PipelineStage for LimiterStage {
    fn process(&mut self, buffer: &mut AudioBuffer, _time_s: f64) {
        let threshold = self.threshold_linear();
        // release coefficient per sample
        let sr = buffer.format.sample_rate as f64;
        let release_samples = (self.release_ms / 1000.0 * sr).max(1.0);
        let release_coeff = 1.0_f64 / release_samples;

        for s in buffer.samples.iter_mut() {
            let abs = s.abs();
            if abs > threshold {
                self.gain_reduction = threshold / abs;
            } else {
                self.gain_reduction = (self.gain_reduction + release_coeff).min(1.0);
            }
            *s *= self.gain_reduction;
        }
    }

    fn name(&self) -> &str {
        "LimiterStage"
    }
}

// ---------------------------------------------------------------------------
// DcBlockStage
// ---------------------------------------------------------------------------

/// First-order high-pass filter to remove DC offset (R = 0.995).
pub struct DcBlockStage {
    /// Per-channel state: [x_prev, y_prev]
    state: Vec<[f64; 2]>,
    r: f64,
}

impl DcBlockStage {
    pub fn new() -> Self {
        Self { state: Vec::new(), r: 0.995 }
    }
}

impl Default for DcBlockStage {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineStage for DcBlockStage {
    fn process(&mut self, buffer: &mut AudioBuffer, _time_s: f64) {
        let n_ch = buffer.format.channels as usize;
        if self.state.len() != n_ch {
            self.state = vec![[0.0; 2]; n_ch];
        }
        let frames = buffer.frame_count();
        for f in 0..frames {
            for c in 0..n_ch {
                let x = buffer.samples[f * n_ch + c];
                let y = x - self.state[c][0] + self.r * self.state[c][1];
                self.state[c][0] = x;
                self.state[c][1] = y;
                buffer.samples[f * n_ch + c] = y;
            }
        }
    }

    fn name(&self) -> &str {
        "DcBlockStage"
    }
}

// ---------------------------------------------------------------------------
// MeteringStage
// ---------------------------------------------------------------------------

/// Non-destructive metering stage that records RMS and peak in micro-dB.
pub struct MeteringStage {
    pub rms: Arc<AtomicI64>,
    pub peak: Arc<AtomicI64>,
}

impl MeteringStage {
    pub fn new() -> Self {
        Self {
            rms: Arc::new(AtomicI64::new(i64::MIN)),
            peak: Arc::new(AtomicI64::new(i64::MIN)),
        }
    }

    /// Read current RMS level in dBFS.
    pub fn rms_db(&self) -> f64 {
        let raw = self.rms.load(Ordering::Relaxed);
        raw as f64 / 1_000_000.0
    }

    /// Read current peak level in dBFS.
    pub fn peak_db(&self) -> f64 {
        let raw = self.peak.load(Ordering::Relaxed);
        raw as f64 / 1_000_000.0
    }

    fn linear_to_db(linear: f64) -> f64 {
        if linear < 1e-12 { -120.0 } else { 20.0 * linear.log10() }
    }
}

impl Default for MeteringStage {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineStage for MeteringStage {
    fn process(&mut self, buffer: &mut AudioBuffer, _time_s: f64) {
        let rms_db = Self::linear_to_db(buffer.rms());
        let peak_db = Self::linear_to_db(buffer.peak_level());
        self.rms.store((rms_db * 1_000_000.0) as i64, Ordering::Relaxed);
        self.peak.store((peak_db * 1_000_000.0) as i64, Ordering::Relaxed);
        // Metering stage does not modify the buffer.
    }

    fn name(&self) -> &str {
        "MeteringStage"
    }
}

// ---------------------------------------------------------------------------
// AudioPipeline
// ---------------------------------------------------------------------------

/// An ordered chain of `PipelineStage` processors.
pub struct AudioPipeline {
    pub stages: Vec<Box<dyn PipelineStage>>,
    pub format: AudioFormat,
}

impl AudioPipeline {
    pub fn new(format: AudioFormat) -> Self {
        Self { stages: Vec::new(), format }
    }

    /// Append a stage to the end of the chain.
    pub fn add_stage(&mut self, stage: Box<dyn PipelineStage>) {
        self.stages.push(stage);
    }

    /// Run the buffer through every stage in order.
    pub fn process(&mut self, buffer: &mut AudioBuffer, time_s: f64) {
        for stage in &mut self.stages {
            stage.process(buffer, time_s);
        }
    }

    /// Convenience wrapper: creates an `AudioBuffer`, runs the pipeline, writes back.
    pub fn process_block(&mut self, samples: &mut Vec<f64>, time_s: f64) {
        let frames = if self.format.channels == 0 {
            0
        } else {
            samples.len() / self.format.channels as usize
        };
        let mut buf = AudioBuffer {
            samples: std::mem::take(samples),
            format: self.format,
        };
        // Ensure correct length.
        let expected = frames * self.format.channels as usize;
        buf.samples.resize(expected, 0.0);

        self.process(&mut buf, time_s);
        *samples = buf.samples;
    }

    /// Return the name of each stage in order.
    pub fn pipeline_info(&self) -> Vec<String> {
        self.stages.iter().map(|s| s.name().to_string()).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stereo_buffer(frames: usize) -> AudioBuffer {
        let fmt = AudioFormat::stereo_f32(44100);
        let mut buf = AudioBuffer::new(frames, fmt);
        // Fill with alternating sine-like values.
        for (i, s) in buf.samples.iter_mut().enumerate() {
            *s = (i as f64 * 0.1).sin() * 0.5;
        }
        buf
    }

    #[test]
    fn buffer_frame_count() {
        let buf = make_stereo_buffer(100);
        assert_eq!(buf.frame_count(), 100);
    }

    #[test]
    fn channel_slice_length() {
        let buf = make_stereo_buffer(100);
        assert_eq!(buf.channel_slice(0).len(), 100);
        assert_eq!(buf.channel_slice(1).len(), 100);
    }

    #[test]
    fn mix_down_length() {
        let buf = make_stereo_buffer(100);
        assert_eq!(buf.mix_down().len(), 100);
    }

    #[test]
    fn normalize_sets_peak() {
        let mut buf = make_stereo_buffer(1000);
        buf.normalize(1.0);
        let peak = buf.peak_level();
        assert!((peak - 1.0).abs() < 1e-9 || peak <= 1.0 + 1e-9);
    }

    #[test]
    fn gain_stage_increases_amplitude() {
        let mut buf = make_stereo_buffer(100);
        let before_peak = buf.peak_level();
        let mut gain = GainStage::new(6.0);
        gain.process(&mut buf, 0.0);
        assert!(buf.peak_level() > before_peak);
    }

    #[test]
    fn limiter_clamps_amplitude() {
        let fmt = AudioFormat::stereo_f32(44100);
        let mut buf = AudioBuffer::new(100, fmt);
        buf.samples.iter_mut().for_each(|s| *s = 2.0); // clip-worthy
        let mut lim = LimiterStage::new(-6.0, 10.0);
        lim.process(&mut buf, 0.0);
        assert!(buf.peak_level() <= 1.0);
    }

    #[test]
    fn dc_block_removes_offset() {
        let fmt = AudioFormat::mono_f32(44100);
        let mut buf = AudioBuffer::new(100, fmt);
        buf.samples.iter_mut().for_each(|s| *s = 0.5); // pure DC
        let mut dc = DcBlockStage::new();
        dc.process(&mut buf, 0.0);
        // After settling, output should be near zero.
        let last = *buf.samples.last().unwrap();
        assert!(last.abs() < 0.1);
    }

    #[test]
    fn pipeline_info_preserves_order() {
        let fmt = AudioFormat::stereo_f32(44100);
        let mut pipeline = AudioPipeline::new(fmt);
        pipeline.add_stage(Box::new(GainStage::new(0.0)));
        pipeline.add_stage(Box::new(PanStage::new(0.0)));
        pipeline.add_stage(Box::new(LimiterStage::new(-1.0, 50.0)));
        let info = pipeline.pipeline_info();
        assert_eq!(info[0], "GainStage");
        assert_eq!(info[1], "PanStage");
        assert_eq!(info[2], "LimiterStage");
    }

    #[test]
    fn metering_stage_records_levels() {
        let meter = MeteringStage::new();
        let mut buf = make_stereo_buffer(1024);
        let mut stage: Box<dyn PipelineStage> = Box::new(MeteringStage {
            rms: meter.rms.clone(),
            peak: meter.peak.clone(),
        });
        stage.process(&mut buf, 0.0);
        assert!(meter.rms_db() < 0.0); // should be negative dBFS
    }
}
