//! Simulated real-time data input for sonification.
//!
//! Provides several data source generators (sine, random LCG, ramp, custom)
//! and a `DataProcessor` that converts generated samples to sonification events.

use crate::scale_mapper_v2::{DataSonificationEvent, ScaleMapper, map_series};
use std::sync::mpsc;

// ---------------------------------------------------------------------------
// DataSource
// ---------------------------------------------------------------------------

/// Describes the source of generated samples.
#[derive(Debug, Clone)]
pub enum DataSource {
    /// Sinusoidal wave.
    Sine { freq: f64, amplitude: f64 },
    /// Pseudo-random noise via a linear congruential generator.
    Random { seed: u64 },
    /// Linear ramp from `start` to `end` over `steps` samples, then wraps.
    Ramp { start: f64, end: f64, steps: usize },
    /// Replay a fixed sequence, cycling when exhausted.
    Custom(Vec<f64>),
}

// ---------------------------------------------------------------------------
// LiveInputConfig
// ---------------------------------------------------------------------------

/// Configuration for a [`LiveInputGenerator`].
#[derive(Debug, Clone)]
pub struct LiveInputConfig {
    pub source: DataSource,
    pub sample_rate_hz: f64,
    pub buffer_size: usize,
}

// ---------------------------------------------------------------------------
// LiveInputGenerator
// ---------------------------------------------------------------------------

/// Generates a stream of `f64` samples from a [`DataSource`].
pub struct LiveInputGenerator {
    pub config: LiveInputConfig,
    /// Current position within the sequence (for Ramp / Custom).
    pub position: usize,
    /// LCG state for `DataSource::Random`.
    pub lcg_state: u64,
}

// LCG constants (Knuth)
const LCG_A: u64 = 6_364_136_223_846_793_005;
const LCG_C: u64 = 1_442_695_040_888_963_407;

impl LiveInputGenerator {
    pub fn new(config: LiveInputConfig) -> Self {
        let lcg_state = match &config.source {
            DataSource::Random { seed } => *seed,
            _ => 0,
        };
        Self {
            config,
            position: 0,
            lcg_state,
        }
    }

    /// Advance the LCG and return a value in `[-1.0, 1.0]`.
    fn lcg_next(&mut self) -> f64 {
        self.lcg_state = self.lcg_state.wrapping_mul(LCG_A).wrapping_add(LCG_C);
        // Map to [-1.0, 1.0]
        let unsigned = (self.lcg_state >> 33) as f64; // 31-bit value
        unsigned / (0x7FFF_FFFFu64 as f64) - 1.0
    }

    /// Return the next generated sample.
    pub fn next_sample(&mut self) -> f64 {
        match &self.config.source.clone() {
            DataSource::Sine { freq, amplitude } => {
                let t = self.position as f64 / self.config.sample_rate_hz;
                self.position = self.position.wrapping_add(1);
                amplitude * (2.0 * std::f64::consts::PI * freq * t).sin()
            }
            DataSource::Random { .. } => self.lcg_next(),
            DataSource::Ramp { start, end, steps } => {
                let steps = (*steps).max(2);
                let pos = self.position % steps;
                self.position += 1;
                start + (end - start) * (pos as f64 / (steps - 1) as f64)
            }
            DataSource::Custom(data) => {
                if data.is_empty() {
                    return 0.0;
                }
                let v = data[self.position % data.len()];
                self.position += 1;
                v
            }
        }
    }

    /// Fill `buf` with generated samples.
    pub fn fill_buffer(&mut self, buf: &mut [f64]) {
        for slot in buf.iter_mut() {
            *slot = self.next_sample();
        }
    }

    /// Send `num_buffers` buffers of samples through a synchronous `mpsc` channel.
    pub fn stream_to_channel(
        &mut self,
        tx: &mpsc::Sender<Vec<f64>>,
        num_buffers: usize,
    ) {
        let size = self.config.buffer_size;
        for _ in 0..num_buffers {
            let mut buf = vec![0.0f64; size];
            self.fill_buffer(&mut buf);
            if tx.send(buf).is_err() {
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DataProcessor
// ---------------------------------------------------------------------------

/// Combines a [`LiveInputGenerator`] with a [`ScaleMapper`] to produce
/// sonification events from generated data.
pub struct DataProcessor {
    pub generator: LiveInputGenerator,
    pub mapper: ScaleMapper,
    pub output: Vec<DataSonificationEvent>,
}

impl DataProcessor {
    pub fn new(generator: LiveInputGenerator, mapper: ScaleMapper) -> Self {
        Self {
            generator,
            mapper,
            output: Vec::new(),
        }
    }

    /// Convert a pre-collected buffer of samples to events.
    pub fn process_buffer(&mut self, buf: &[f64]) -> Vec<DataSonificationEvent> {
        map_series(buf, &self.mapper)
    }

    /// Generate `num_samples` samples and process them all at once.
    pub fn run_sync(&mut self, num_samples: usize) -> Vec<DataSonificationEvent> {
        let mut buf = vec![0.0f64; num_samples];
        self.generator.fill_buffer(&mut buf);
        let events = self.process_buffer(&buf);
        self.output.extend(events.clone());
        events
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale_mapper_v2::MusicalScale;

    fn sine_gen(freq: f64, amplitude: f64) -> LiveInputGenerator {
        LiveInputGenerator::new(LiveInputConfig {
            source: DataSource::Sine { freq, amplitude },
            sample_rate_hz: 44100.0,
            buffer_size: 256,
        })
    }

    #[test]
    fn sine_values_within_amplitude() {
        let amp = 2.5;
        let mut gen = sine_gen(440.0, amp);
        for _ in 0..1000 {
            let v = gen.next_sample();
            assert!(
                v >= -amp - 1e-9 && v <= amp + 1e-9,
                "sine sample {} out of range [-{}, {}]",
                v, amp, amp
            );
        }
    }

    #[test]
    fn ramp_is_monotone_increasing() {
        let mut gen = LiveInputGenerator::new(LiveInputConfig {
            source: DataSource::Ramp { start: 0.0, end: 10.0, steps: 100 },
            sample_rate_hz: 44100.0,
            buffer_size: 100,
        });
        let mut prev = gen.next_sample();
        for _ in 1..100 {
            let cur = gen.next_sample();
            assert!(cur >= prev, "ramp not monotone: {} < {}", cur, prev);
            prev = cur;
        }
    }

    #[test]
    fn buffer_fill_correct_length() {
        let mut gen = sine_gen(220.0, 1.0);
        let mut buf = vec![0.0f64; 512];
        gen.fill_buffer(&mut buf);
        assert_eq!(buf.len(), 512);
        // All values should be finite.
        assert!(buf.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn random_source_varies() {
        let mut gen = LiveInputGenerator::new(LiveInputConfig {
            source: DataSource::Random { seed: 42 },
            sample_rate_hz: 44100.0,
            buffer_size: 64,
        });
        let a = gen.next_sample();
        let b = gen.next_sample();
        // Two consecutive LCG samples should differ.
        assert_ne!(a, b);
    }

    #[test]
    fn run_sync_produces_events() {
        let gen = LiveInputGenerator::new(LiveInputConfig {
            source: DataSource::Ramp { start: 0.0, end: 1.0, steps: 10 },
            sample_rate_hz: 44100.0,
            buffer_size: 10,
        });
        let mapper = ScaleMapper::new(MusicalScale::Major, 60, 2);
        let mut proc = DataProcessor::new(gen, mapper);
        let events = proc.run_sync(10);
        assert_eq!(events.len(), 10);
    }
}
