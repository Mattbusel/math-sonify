//! Audio dynamics processing: compressor, limiter, expander.

// ---------------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CompressorConfig {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub makeup_gain_db: f64,
    pub knee_db: f64,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            makeup_gain_db: 6.0,
            knee_db: 6.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DynamicProcessor {
    pub config: CompressorConfig,
    pub envelope: f64,
    pub sample_rate: f64,
}

impl DynamicProcessor {
    pub fn new(config: CompressorConfig, sample_rate: f64) -> Self {
        Self {
            config,
            envelope: 0.0,
            sample_rate,
        }
    }

    /// Convert dB to linear amplitude.
    pub fn db_to_linear(db: f64) -> f64 {
        10_f64.powf(db / 20.0)
    }

    /// Convert linear amplitude to dB.
    pub fn linear_to_db(linear: f64) -> f64 {
        20.0 * (linear.abs() + 1e-10).log10()
    }

    /// Compute gain reduction (in dB, negative) using soft-knee compression.
    pub fn compute_gain_reduction(&self, input_db: f64) -> f64 {
        let threshold = self.config.threshold_db;
        let ratio = self.config.ratio;
        let knee = self.config.knee_db;
        let half_knee = knee / 2.0;

        let overshoot = input_db - threshold;

        if overshoot < -half_knee {
            // Below knee: no gain reduction
            0.0
        } else if overshoot < half_knee {
            // Within the soft knee
            let t = (overshoot + half_knee) / knee;
            // Smoothly interpolate from 0 to full compression using quadratic blend
            let effective_ratio = 1.0 + (ratio - 1.0) * t;
            -((overshoot + half_knee) * (1.0 - 1.0 / effective_ratio)) / 2.0
        } else {
            // Above knee: full compression
            overshoot * (1.0 - 1.0 / ratio)
        }
    }

    /// Process a single sample through envelope follower and gain reduction.
    pub fn process_sample(&mut self, sample: f64) -> f64 {
        let attack_coeff = (-1.0 / (self.config.attack_ms * 1e-3 * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.config.release_ms * 1e-3 * self.sample_rate)).exp();

        let level = sample.abs();
        // Envelope follower
        if level > self.envelope {
            self.envelope = attack_coeff * self.envelope + (1.0 - attack_coeff) * level;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * level;
        }

        let input_db = Self::linear_to_db(self.envelope);
        let gain_reduction_db = self.compute_gain_reduction(input_db);
        let makeup_db = self.config.makeup_gain_db;
        let total_gain_db = -gain_reduction_db + makeup_db;

        sample * Self::db_to_linear(total_gain_db)
    }

    /// Process a buffer of samples.
    pub fn process_buffer(&mut self, samples: &[f64]) -> Vec<f64> {
        samples.iter().map(|&s| self.process_sample(s)).collect()
    }
}

// ---------------------------------------------------------------------------
// Limiter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LimiterConfig {
    pub ceiling_db: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            ceiling_db: -1.0,
            attack_ms: 1.0,
            release_ms: 50.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Limiter {
    pub config: LimiterConfig,
    pub envelope: f64,
    pub sample_rate: f64,
}

impl Limiter {
    pub fn new(config: LimiterConfig, sample_rate: f64) -> Self {
        Self {
            config,
            envelope: 0.0,
            sample_rate,
        }
    }

    pub fn process_sample(&mut self, sample: f64) -> f64 {
        let attack_coeff = (-1.0 / (self.config.attack_ms * 1e-3 * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.config.release_ms * 1e-3 * self.sample_rate)).exp();

        let level = sample.abs();
        if level > self.envelope {
            self.envelope = attack_coeff * self.envelope + (1.0 - attack_coeff) * level;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * level;
        }

        let ceiling = DynamicProcessor::db_to_linear(self.config.ceiling_db);
        if self.envelope > ceiling {
            // Hard limit: infinite ratio above ceiling
            let gain = ceiling / self.envelope.max(1e-10);
            sample * gain
        } else {
            sample
        }
    }

    pub fn process_buffer(&mut self, samples: &[f64]) -> Vec<f64> {
        samples.iter().map(|&s| self.process_sample(s)).collect()
    }
}

// ---------------------------------------------------------------------------
// Expander
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ExpanderConfig {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
}

impl Default for ExpanderConfig {
    fn default() -> Self {
        Self {
            threshold_db: -40.0,
            ratio: 2.0,
            attack_ms: 10.0,
            release_ms: 100.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Expander {
    pub config: ExpanderConfig,
    pub envelope: f64,
    pub sample_rate: f64,
}

impl Expander {
    pub fn new(config: ExpanderConfig, sample_rate: f64) -> Self {
        Self {
            config,
            envelope: 0.0,
            sample_rate,
        }
    }

    pub fn process_sample(&mut self, sample: f64) -> f64 {
        let attack_coeff = (-1.0 / (self.config.attack_ms * 1e-3 * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.config.release_ms * 1e-3 * self.sample_rate)).exp();

        let level = sample.abs();
        if level > self.envelope {
            self.envelope = attack_coeff * self.envelope + (1.0 - attack_coeff) * level;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * level;
        }

        let input_db = DynamicProcessor::linear_to_db(self.envelope);
        let threshold = self.config.threshold_db;

        // Downward expansion: reduce gain when below threshold
        let gain_db = if input_db < threshold {
            // gain = ratio * (input_db - threshold), which is negative (attenuate)
            self.config.ratio * (input_db - threshold)
        } else {
            0.0
        };

        sample * DynamicProcessor::db_to_linear(gain_db)
    }

    pub fn process_buffer(&mut self, samples: &[f64]) -> Vec<f64> {
        samples.iter().map(|&s| self.process_sample(s)).collect()
    }
}

// ---------------------------------------------------------------------------
// Dynamics Chain
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct DynamicsChain {
    pub compressor: Option<DynamicProcessor>,
    pub limiter: Option<Limiter>,
    pub expander: Option<Expander>,
}

impl DynamicsChain {
    pub fn new() -> Self {
        Self {
            compressor: None,
            limiter: None,
            expander: None,
        }
    }

    pub fn with_compressor(mut self, compressor: DynamicProcessor) -> Self {
        self.compressor = Some(compressor);
        self
    }

    pub fn with_limiter(mut self, limiter: Limiter) -> Self {
        self.limiter = Some(limiter);
        self
    }

    pub fn with_expander(mut self, expander: Expander) -> Self {
        self.expander = Some(expander);
        self
    }

    /// Process the chain in order: expander → compressor → limiter.
    pub fn process_chain(&mut self, samples: &[f64]) -> Vec<f64> {
        let mut buf: Vec<f64> = samples.to_vec();

        if let Some(ref mut exp) = self.expander {
            buf = exp.process_buffer(&buf);
        }
        if let Some(ref mut comp) = self.compressor {
            buf = comp.process_buffer(&buf);
        }
        if let Some(ref mut lim) = self.limiter {
            buf = lim.process_buffer(&buf);
        }

        buf
    }
}

impl Default for DynamicsChain {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_linear_roundtrip() {
        for db in [-60.0f64, -20.0, -6.0, 0.0, 6.0, 20.0] {
            let linear = DynamicProcessor::db_to_linear(db);
            let back = DynamicProcessor::linear_to_db(linear);
            assert!(
                (back - db).abs() < 0.001,
                "Roundtrip failed for {}dB: got {}",
                db,
                back
            );
        }
    }

    #[test]
    fn test_compressor_reduces_gain_above_threshold() {
        let config = CompressorConfig {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 10.0,
            makeup_gain_db: 0.0,
            knee_db: 0.0,
        };
        let processor = DynamicProcessor::new(config, 44100.0);
        // A signal well above threshold should have negative gain reduction
        let gr = processor.compute_gain_reduction(-10.0); // 10dB above threshold
        assert!(gr > 0.0, "Expected positive gain reduction above threshold, got {}", gr);
    }

    #[test]
    fn test_limiter_caps_peaks() {
        let config = LimiterConfig {
            ceiling_db: -6.0,
            attack_ms: 0.1,
            release_ms: 10.0,
        };
        let mut limiter = Limiter::new(config, 44100.0);
        let ceiling = DynamicProcessor::db_to_linear(-6.0);
        // Feed a loud signal until envelope catches up
        let loud = vec![2.0f64; 1000];
        let output = limiter.process_buffer(&loud);
        // After settling, output should be near or below ceiling
        let last_out = output.last().unwrap().abs();
        assert!(
            last_out <= ceiling * 1.1,
            "Limiter should cap peaks near ceiling. Got {}",
            last_out
        );
    }

    #[test]
    fn test_expander_reduces_below_threshold() {
        let config = ExpanderConfig {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 10.0,
        };
        let mut expander = Expander::new(config, 44100.0);
        // A quiet signal below threshold should be attenuated
        let quiet = vec![0.001f64; 1000];
        let output = expander.process_buffer(&quiet);
        let last_out = output.last().unwrap().abs();
        assert!(
            last_out < 0.001,
            "Expander should attenuate signal below threshold. Got {}",
            last_out
        );
    }

    #[test]
    fn test_chain_with_all_three() {
        let comp = DynamicProcessor::new(CompressorConfig::default(), 44100.0);
        let lim = Limiter::new(LimiterConfig::default(), 44100.0);
        let exp = Expander::new(ExpanderConfig::default(), 44100.0);
        let mut chain = DynamicsChain::new()
            .with_compressor(comp)
            .with_limiter(lim)
            .with_expander(exp);
        let samples: Vec<f64> = (0..100).map(|i| (i as f64 * 0.1).sin()).collect();
        let output = chain.process_chain(&samples);
        assert_eq!(output.len(), samples.len());
    }

    #[test]
    fn test_process_buffer_same_length() {
        let mut comp = DynamicProcessor::new(CompressorConfig::default(), 44100.0);
        let input = vec![0.5f64; 256];
        let output = comp.process_buffer(&input);
        assert_eq!(output.len(), input.len());
    }
}
