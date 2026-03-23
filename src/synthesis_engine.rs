//! Unified audio synthesis pipeline and graph.

use std::collections::HashMap;
use std::f64::consts::PI;

/// Waveform types for the oscillator node.
#[derive(Debug, Clone)]
pub enum WaveformType {
    Sine,
    Square,
    Sawtooth,
    Triangle,
    Noise { seed: u64 },
}

/// ADSR envelope parameters (times in milliseconds, sustain as 0..1 gain).
#[derive(Debug, Clone)]
pub struct AdsrParams {
    pub attack_ms: f64,
    pub decay_ms: f64,
    pub sustain: f64,
    pub release_ms: f64,
}

impl Default for AdsrParams {
    fn default() -> Self {
        AdsrParams {
            attack_ms: 10.0,
            decay_ms: 100.0,
            sustain: 0.7,
            release_ms: 200.0,
        }
    }
}

/// Biquad filter types.
#[derive(Debug, Clone)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

/// A node in the audio graph.
#[derive(Debug, Clone)]
pub enum AudioNode {
    Oscillator { freq: f64, waveform: WaveformType },
    Envelope(AdsrParams),
    Filter { cutoff: f64, resonance: f64, filter_type: FilterType },
    Gain(f64),
    /// Mixer: sum of specified input node indices.
    Mixer(Vec<usize>),
    Output,
}

/// Node identifier type alias.
pub type NodeId = usize;

/// A directed audio processing graph.
#[derive(Debug, Clone, Default)]
pub struct AudioGraph {
    pub nodes: Vec<(NodeId, AudioNode)>,
    /// Directed edges: (from, to).
    pub edges: Vec<(NodeId, NodeId)>,
    pub next_id: NodeId,
}

impl AudioGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node to the graph and return its ID.
    pub fn add_node(&mut self, node: AudioNode) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push((id, node));
        id
    }

    /// Connect `from` → `to`.
    pub fn connect(&mut self, from: NodeId, to: NodeId) {
        self.edges.push((from, to));
    }

    /// Remove a node (and all its edges).
    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.retain(|(nid, _)| *nid != id);
        self.edges.retain(|(f, t)| *f != id && *t != id);
    }

    /// Kahn's topological sort; returns node IDs in processing order.
    pub fn topological_order(&self) -> Vec<NodeId> {
        let node_ids: Vec<NodeId> = self.nodes.iter().map(|(id, _)| *id).collect();
        let mut in_degree: HashMap<NodeId, usize> = node_ids.iter().map(|&id| (id, 0)).collect();

        for &(_, to) in &self.edges {
            *in_degree.entry(to).or_insert(0) += 1;
        }

        let mut queue: Vec<NodeId> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();
        queue.sort_unstable();

        let mut order: Vec<NodeId> = Vec::new();
        while !queue.is_empty() {
            queue.sort_unstable();
            let node = queue.remove(0);
            order.push(node);
            for &(f, t) in &self.edges {
                if f == node {
                    let deg = in_degree.entry(t).or_insert(1);
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push(t);
                    }
                }
            }
        }
        order
    }
}

/// The main synthesis engine.
pub struct SynthesisEngine {
    pub graph: AudioGraph,
    pub sample_rate: f64,
    pub buffer_size: usize,
    pub node_buffers: HashMap<NodeId, Vec<f64>>,
}

impl SynthesisEngine {
    pub fn new(sample_rate: f64, buffer_size: usize) -> Self {
        SynthesisEngine {
            graph: AudioGraph::new(),
            sample_rate,
            buffer_size,
            node_buffers: HashMap::new(),
        }
    }

    /// Process a single node, returning a reference to its output buffer.
    pub fn process_node(&mut self, id: NodeId, time_s: f64, gate: bool) -> Vec<f64> {
        // Find the node
        let node = self.graph.nodes.iter().find(|(nid, _)| *nid == id).map(|(_, n)| n.clone());
        let node = match node {
            Some(n) => n,
            None => return vec![0.0; self.buffer_size],
        };

        let num = self.buffer_size;
        let sr = self.sample_rate;

        let buf = match &node {
            AudioNode::Oscillator { freq, waveform } => {
                Self::oscillator_samples(*freq, waveform, num, time_s, sr)
            }
            AudioNode::Envelope(params) => {
                let input = self.get_input_buffer(id);
                Self::apply_envelope(&input, params, gate, None, sr)
            }
            AudioNode::Filter { cutoff, resonance, filter_type } => {
                let input = self.get_input_buffer(id);
                Self::apply_filter(&input, *cutoff, *resonance, filter_type, sr)
            }
            AudioNode::Gain(gain) => {
                let input = self.get_input_buffer(id);
                input.iter().map(|&s| s * gain).collect()
            }
            AudioNode::Mixer(indices) => {
                let mut out = vec![0.0f64; num];
                for &idx in indices {
                    if let Some(buf) = self.node_buffers.get(&idx) {
                        for (o, &s) in out.iter_mut().zip(buf.iter()) {
                            *o += s;
                        }
                    }
                }
                out
            }
            AudioNode::Output => {
                self.get_input_buffer(id)
            }
        };

        self.node_buffers.insert(id, buf.clone());
        buf
    }

    /// Render `num_samples` samples by traversing the graph in topological order.
    pub fn render(&mut self, num_samples: usize, gate: bool) -> Vec<f64> {
        let order = self.graph.topological_order();
        let time_s = 0.0f64;

        for id in order {
            let buf = self.process_node(id, time_s, gate);
            self.node_buffers.insert(id, buf);
        }

        // Output node or last node
        let output_id = self.graph.nodes.iter()
            .find(|(_, n)| matches!(n, AudioNode::Output))
            .map(|(id, _)| *id)
            .or_else(|| self.graph.nodes.last().map(|(id, _)| *id));

        if let Some(id) = output_id {
            if let Some(buf) = self.node_buffers.get(&id) {
                let out_len = num_samples.min(buf.len());
                return buf[..out_len].to_vec();
            }
        }

        vec![0.0; num_samples]
    }

    /// Generate oscillator samples.
    pub fn oscillator_samples(
        freq: f64,
        waveform: &WaveformType,
        num: usize,
        time_s: f64,
        sr: f64,
    ) -> Vec<f64> {
        let mut samples = Vec::with_capacity(num);
        let mut rng_state: u64 = if let WaveformType::Noise { seed } = waveform { *seed } else { 12345 };

        for i in 0..num {
            let t = time_s + i as f64 / sr;
            let phase = (t * freq).fract();
            let sample = match waveform {
                WaveformType::Sine => (2.0 * PI * phase).sin(),
                WaveformType::Square => if phase < 0.5 { 1.0 } else { -1.0 },
                WaveformType::Sawtooth => 2.0 * phase - 1.0,
                WaveformType::Triangle => {
                    if phase < 0.5 {
                        4.0 * phase - 1.0
                    } else {
                        3.0 - 4.0 * phase
                    }
                }
                WaveformType::Noise { .. } => {
                    // xorshift64
                    rng_state ^= rng_state << 13;
                    rng_state ^= rng_state >> 7;
                    rng_state ^= rng_state << 17;
                    (rng_state as i64 as f64) / (i64::MAX as f64)
                }
            };
            samples.push(sample);
        }
        samples
    }

    /// Apply a biquad filter to samples.
    pub fn apply_filter(
        samples: &[f64],
        cutoff: f64,
        resonance: f64,
        filter_type: &FilterType,
        sr: f64,
    ) -> Vec<f64> {
        // Biquad filter coefficients
        let omega = 2.0 * PI * cutoff / sr;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let q = resonance.max(0.001);
        let alpha = sin_omega / (2.0 * q);

        let (b0, b1, b2, a0, a1, a2) = match filter_type {
            FilterType::LowPass => {
                let b0 = (1.0 - cos_omega) / 2.0;
                let b1 = 1.0 - cos_omega;
                let b2 = (1.0 - cos_omega) / 2.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighPass => {
                let b0 = (1.0 + cos_omega) / 2.0;
                let b1 = -(1.0 + cos_omega);
                let b2 = (1.0 + cos_omega) / 2.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::BandPass => {
                let b0 = sin_omega / 2.0;
                let b1 = 0.0;
                let b2 = -sin_omega / 2.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        let mut out = Vec::with_capacity(samples.len());
        let mut x1 = 0.0f64;
        let mut x2 = 0.0f64;
        let mut y1 = 0.0f64;
        let mut y2 = 0.0f64;

        for &x0 in samples {
            let y0 = (b0 / a0) * x0 + (b1 / a0) * x1 + (b2 / a0) * x2
                - (a1 / a0) * y1 - (a2 / a0) * y2;
            out.push(y0);
            x2 = x1;
            x1 = x0;
            y2 = y1;
            y1 = y0;
        }

        out
    }

    /// Apply an ADSR envelope to samples.
    pub fn apply_envelope(
        samples: &[f64],
        params: &AdsrParams,
        gate_on: bool,
        gate_off_at: Option<f64>,
        sr: f64,
    ) -> Vec<f64> {
        let attack_samples = (params.attack_ms / 1000.0 * sr) as usize;
        let decay_samples = (params.decay_ms / 1000.0 * sr) as usize;
        let release_samples = (params.release_ms / 1000.0 * sr) as usize;
        let sustain = params.sustain;

        let gate_off_sample = gate_off_at.map(|t| (t * sr) as usize);

        samples.iter().enumerate().map(|(i, &s)| {
            let env = if !gate_on {
                // Release phase
                let elapsed = gate_off_sample.map(|off| i.saturating_sub(off)).unwrap_or(i);
                if release_samples > 0 {
                    let t = elapsed as f64 / release_samples as f64;
                    sustain * (1.0 - t.min(1.0))
                } else {
                    0.0
                }
            } else if i < attack_samples {
                if attack_samples > 0 { i as f64 / attack_samples as f64 } else { 1.0 }
            } else if i < attack_samples + decay_samples {
                let t = (i - attack_samples) as f64 / decay_samples.max(1) as f64;
                1.0 - t * (1.0 - sustain)
            } else {
                sustain
            };
            s * env
        }).collect()
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn get_input_buffer(&self, id: NodeId) -> Vec<f64> {
        // Find nodes that feed into `id`
        let inputs: Vec<NodeId> = self.graph.edges.iter()
            .filter(|(_, to)| *to == id)
            .map(|(from, _)| *from)
            .collect();

        let mut out = vec![0.0f64; self.buffer_size];
        for input_id in inputs {
            if let Some(buf) = self.node_buffers.get(&input_id) {
                for (o, &s) in out.iter_mut().zip(buf.iter()) {
                    *o += s;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oscillator_sine_length() {
        let samples = SynthesisEngine::oscillator_samples(440.0, &WaveformType::Sine, 128, 0.0, 44100.0);
        assert_eq!(samples.len(), 128);
    }

    #[test]
    fn test_oscillator_values_bounded() {
        let samples = SynthesisEngine::oscillator_samples(440.0, &WaveformType::Sine, 512, 0.0, 44100.0);
        for s in samples {
            assert!(s >= -1.0 && s <= 1.0, "sample out of bounds: {}", s);
        }
    }

    #[test]
    fn test_apply_filter_length() {
        let input: Vec<f64> = (0..256).map(|i| (i as f64 * 0.01).sin()).collect();
        let out = SynthesisEngine::apply_filter(&input, 1000.0, 0.7, &FilterType::LowPass, 44100.0);
        assert_eq!(out.len(), 256);
    }

    #[test]
    fn test_apply_envelope_gate_on() {
        let input = vec![1.0f64; 1024];
        let params = AdsrParams { attack_ms: 10.0, decay_ms: 50.0, sustain: 0.8, release_ms: 100.0 };
        let out = SynthesisEngine::apply_envelope(&input, &params, true, None, 44100.0);
        assert_eq!(out.len(), 1024);
        // First sample should be close to 0 (attack start)
        assert!(out[0] < 0.1);
    }

    #[test]
    fn test_audio_graph_add_connect() {
        let mut graph = AudioGraph::new();
        let osc = graph.add_node(AudioNode::Oscillator { freq: 440.0, waveform: WaveformType::Sine });
        let out = graph.add_node(AudioNode::Output);
        graph.connect(osc, out);
        assert_eq!(graph.edges.len(), 1);
    }

    #[test]
    fn test_topological_order() {
        let mut graph = AudioGraph::new();
        let a = graph.add_node(AudioNode::Oscillator { freq: 440.0, waveform: WaveformType::Sine });
        let b = graph.add_node(AudioNode::Gain(0.5));
        let c = graph.add_node(AudioNode::Output);
        graph.connect(a, b);
        graph.connect(b, c);
        let order = graph.topological_order();
        let pos_a = order.iter().position(|&x| x == a).unwrap();
        let pos_b = order.iter().position(|&x| x == b).unwrap();
        let pos_c = order.iter().position(|&x| x == c).unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_engine_render() {
        let mut engine = SynthesisEngine::new(44100.0, 256);
        let osc = engine.graph.add_node(AudioNode::Oscillator { freq: 440.0, waveform: WaveformType::Sine });
        let out = engine.graph.add_node(AudioNode::Output);
        engine.graph.connect(osc, out);
        let buf = engine.render(256, true);
        assert_eq!(buf.len(), 256);
    }
}
