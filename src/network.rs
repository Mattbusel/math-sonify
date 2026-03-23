//! Network of Coupled Oscillators — N oscillators on arbitrary graph topologies.
//!
//! This module implements a flexible framework for networks of coupled
//! dynamical systems:
//!
//! - [`NetworkTopology`] — graph structure (Ring, Star, Small-World, Erdos-Rényi, Full).
//! - [`CouplingGraph`] — weighted adjacency matrix constructed from a [`NetworkTopology`].
//! - [`KuramotoNetwork`] — Kuramoto phase oscillators on a graph; generalises
//!   [`crate::systems::Kuramoto`] to arbitrary topology.
//! - [`StuartLandauNetwork`] — Stuart–Landau (normal-form Hopf) complex
//!   amplitude oscillators; exhibits amplitude death and oscillation revival.
//! - [`OscillatorNetwork`] — unified wrapper exposing up to 16 oscillators as
//!   separate audio voices.
//! - [`NetworkState`] — snapshot of all oscillator outputs, ready for broadcast
//!   to the audio system.
//!
//! Each oscillator maps to one audio voice through a configurable
//! frequency / amplitude mapping.  The audio system receives a
//! [`NetworkState`] once per control-rate tick.

#![allow(dead_code)]

use std::f64::consts::TAU;

// ---------------------------------------------------------------------------
// Network topology
// ---------------------------------------------------------------------------

/// Graph topology for the oscillator network.
#[derive(Debug, Clone)]
pub enum NetworkTopology {
    /// Each node connected to its two nearest neighbours (circular).
    Ring,
    /// One central hub connected to all leaves; leaves connect only to hub.
    StarGraph,
    /// Watts–Strogatz small-world: start from ring, rewire each edge with
    /// probability `p`.  At `p=0` it is a ring; at `p=1` it is random.
    SmallWorld { rewire_prob: f64 },
    /// Erdos–Rényi random graph: each pair of nodes connected with
    /// probability `p`.
    RandomErdos { connect_prob: f64 },
    /// Every node connected to every other node with equal weight.
    FullyConnected,
}

impl NetworkTopology {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ring => "Ring",
            Self::StarGraph => "Star",
            Self::SmallWorld { .. } => "Small World",
            Self::RandomErdos { .. } => "Erdos-Renyi",
            Self::FullyConnected => "Fully Connected",
        }
    }
}

// ---------------------------------------------------------------------------
// Coupling graph (adjacency matrix)
// ---------------------------------------------------------------------------

/// Weighted directed adjacency matrix for N nodes.
///
/// `weight[i][j]` is the coupling weight from node j to node i.
/// 0.0 means no connection.
#[derive(Debug, Clone)]
pub struct CouplingGraph {
    pub n: usize,
    /// Flattened row-major N×N weight matrix.
    weight: Vec<f64>,
}

impl CouplingGraph {
    /// Create a new graph with all edge weights zero.
    pub fn new(n: usize) -> Self {
        Self { n, weight: vec![0.0; n * n] }
    }

    /// Get the weight from j to i.
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.weight.get(i * self.n + j).copied().unwrap_or(0.0)
    }

    /// Set the weight from j to i.
    pub fn set(&mut self, i: usize, j: usize, w: f64) {
        if i < self.n && j < self.n {
            self.weight[i * self.n + j] = w;
        }
    }

    /// Return the degree (out-degree sum) of node i.
    pub fn degree(&self, i: usize) -> f64 {
        (0..self.n).map(|j| self.get(i, j)).sum()
    }

    /// Build a graph from a [`NetworkTopology`].
    ///
    /// * `n` — number of nodes (capped at 16).
    /// * `coupling` — global coupling weight applied to each edge.
    /// * `seed` — deterministic pseudo-random seed for stochastic topologies.
    pub fn from_topology(topology: &NetworkTopology, n: usize, coupling: f64, seed: u64) -> Self {
        let n = n.clamp(2, 16);
        let mut g = Self::new(n);

        match topology {
            NetworkTopology::Ring => {
                for i in 0..n {
                    let left = (i + n - 1) % n;
                    let right = (i + 1) % n;
                    g.set(i, left, coupling);
                    g.set(i, right, coupling);
                }
            }
            NetworkTopology::StarGraph => {
                // Node 0 is the hub.
                for leaf in 1..n {
                    g.set(0, leaf, coupling); // hub receives from leaves
                    g.set(leaf, 0, coupling); // leaves receive from hub
                }
            }
            NetworkTopology::SmallWorld { rewire_prob } => {
                // Start with a ring.
                for i in 0..n {
                    let right = (i + 1) % n;
                    g.set(i, right, coupling);
                    g.set(right, i, coupling);
                }
                // Rewire with probability p.
                let mut rng = SimpleRng::new(seed);
                for i in 0..n {
                    let j = (i + 1) % n;
                    if rng.next_f64() < *rewire_prob {
                        // Remove edge i-j.
                        g.set(i, j, 0.0);
                        g.set(j, i, 0.0);
                        // Add edge i-k for a random k != i, != j.
                        let k = {
                            let mut k = rng.next_usize(n);
                            while k == i || k == j { k = rng.next_usize(n); }
                            k
                        };
                        g.set(i, k, coupling);
                        g.set(k, i, coupling);
                    }
                }
            }
            NetworkTopology::RandomErdos { connect_prob } => {
                let mut rng = SimpleRng::new(seed);
                for i in 0..n {
                    for j in (i + 1)..n {
                        if rng.next_f64() < *connect_prob {
                            g.set(i, j, coupling);
                            g.set(j, i, coupling);
                        }
                    }
                }
            }
            NetworkTopology::FullyConnected => {
                for i in 0..n {
                    for j in 0..n {
                        if i != j {
                            g.set(i, j, coupling);
                        }
                    }
                }
            }
        }
        g
    }

    /// Count the number of non-zero edges.
    pub fn edge_count(&self) -> usize {
        self.weight.iter().filter(|&&w| w != 0.0).count()
    }
}

// ---------------------------------------------------------------------------
// Kuramoto network
// ---------------------------------------------------------------------------

/// Kuramoto model on an arbitrary graph topology.
///
/// Each oscillator i evolves according to:
///
///   dθᵢ/dt = ωᵢ + Σⱼ Kᵢⱼ sin(θⱼ − θᵢ)
///
/// where Kᵢⱼ is the coupling weight from j to i in the [`CouplingGraph`].
#[derive(Debug, Clone)]
pub struct KuramotoNetwork {
    /// Phase of each oscillator (radians, wrapped to [0, 2π)).
    pub phases: Vec<f64>,
    /// Natural frequency of each oscillator (rad/s).
    pub natural_frequencies: Vec<f64>,
    /// Coupling graph.
    pub graph: CouplingGraph,
    /// Global coupling strength multiplier.
    pub coupling: f64,
    /// Order parameter r ∈ [0, 1].
    pub order_parameter: f64,
    /// Mean phase of the ensemble.
    pub mean_phase: f64,
}

impl KuramotoNetwork {
    /// Create a Kuramoto network.
    ///
    /// * `n` — number of oscillators (1–16).
    /// * `topology` — graph structure.
    /// * `coupling` — global coupling weight.
    /// * `freq_spread` — spread of natural frequencies (Hz); frequencies are
    ///   spaced linearly from 1.0 − freq_spread/2 to 1.0 + freq_spread/2.
    pub fn new(n: usize, topology: &NetworkTopology, coupling: f64, freq_spread: f64, seed: u64) -> Self {
        let n = n.clamp(1, 16);
        let phases: Vec<f64> = (0..n)
            .map(|i| TAU * i as f64 / n as f64)
            .collect();
        let natural_frequencies: Vec<f64> = (0..n)
            .map(|i| {
                if n == 1 {
                    1.0
                } else {
                    1.0 - freq_spread / 2.0 + freq_spread * i as f64 / (n - 1) as f64
                }
            })
            .collect();
        let graph = CouplingGraph::from_topology(topology, n, coupling, seed);
        Self {
            phases,
            natural_frequencies,
            graph,
            coupling,
            order_parameter: 0.0,
            mean_phase: 0.0,
        }
    }

    /// Step the network forward by `dt` seconds using Euler integration.
    pub fn step(&mut self, dt: f64) {
        let n = self.phases.len();
        let mut dphase = vec![0.0f64; n];

        for i in 0..n {
            let mut coupling_sum = 0.0f64;
            for j in 0..n {
                let w = self.graph.get(i, j);
                if w != 0.0 {
                    coupling_sum += w * (self.phases[j] - self.phases[i]).sin();
                }
            }
            dphase[i] = self.natural_frequencies[i] * TAU + coupling_sum;
        }

        for (phi, dphi) in self.phases.iter_mut().zip(dphase.iter()) {
            *phi = (*phi + dt * dphi).rem_euclid(TAU);
        }

        self.update_order_parameter();
    }

    fn update_order_parameter(&mut self) {
        let n = self.phases.len() as f64;
        let (sin_sum, cos_sum): (f64, f64) = self
            .phases
            .iter()
            .fold((0.0, 0.0), |(s, c), &ph| (s + ph.sin(), c + ph.cos()));
        self.order_parameter = (sin_sum.powi(2) + cos_sum.powi(2)).sqrt() / n;
        self.mean_phase = cos_sum.atan2(sin_sum);
    }

    /// Map each oscillator's phase to an audio frequency (Hz).
    ///
    /// * `base_freq` — frequency at phase 0 (Hz).
    /// * `freq_range` — frequency span (Hz); phase 2π → `base_freq + freq_range`.
    pub fn oscillator_frequencies(&self, base_freq: f64, freq_range: f64) -> Vec<f64> {
        self.phases
            .iter()
            .map(|&ph| base_freq + ph / TAU * freq_range)
            .collect()
    }

    /// Return oscillator phases as amplitudes ∈ [0, 1] for audio voice levels.
    pub fn oscillator_amplitudes(&self) -> Vec<f64> {
        self.phases.iter().map(|&ph| (ph.sin() + 1.0) * 0.5).collect()
    }

    /// Set the coupling graph to a new topology.
    pub fn set_topology(&mut self, topology: &NetworkTopology, seed: u64) {
        self.graph = CouplingGraph::from_topology(topology, self.phases.len(), self.coupling, seed);
    }
}

// ---------------------------------------------------------------------------
// Stuart–Landau network
// ---------------------------------------------------------------------------

/// Stuart–Landau (supercritical Hopf normal form) oscillator network.
///
/// Each node has a complex amplitude Aᵢ = Xᵢ + iYᵢ and evolves according to:
///
///   dAᵢ/dt = (μᵢ + i·ωᵢ − |Aᵢ|²)·Aᵢ + Σⱼ Kᵢⱼ·Aⱼ
///
/// * μᵢ > 0: oscillation (limit cycle with amplitude √μᵢ).
/// * μᵢ < 0: stable fixed point at origin.
/// * With diffusive coupling and heterogeneous μ, the network can exhibit
///   **amplitude death** (all oscillators settle to zero) or
///   **oscillation revival** when coupling is increased.
#[derive(Debug, Clone)]
pub struct StuartLandauNetwork {
    /// Real part of each complex amplitude.
    pub x: Vec<f64>,
    /// Imaginary part of each complex amplitude.
    pub y: Vec<f64>,
    /// Bifurcation parameter μᵢ (positive = oscillating, negative = damped).
    pub mu: Vec<f64>,
    /// Natural frequency of each node (rad/s).
    pub omega: Vec<f64>,
    /// Coupling graph.
    pub graph: CouplingGraph,
    /// Instantaneous amplitude |Aᵢ| of each oscillator.
    pub amplitudes: Vec<f64>,
    /// Whether the network is in amplitude-death state.
    pub amplitude_death: bool,
    /// Death threshold: if max amplitude < this value → amplitude death.
    pub death_threshold: f64,
}

impl StuartLandauNetwork {
    /// Create a Stuart–Landau network.
    ///
    /// * `n` — number of nodes (1–16).
    /// * `topology` — graph structure.
    /// * `coupling` — edge coupling weight.
    /// * `mu_spread` — μ values spread uniformly around 0.5 by ±mu_spread.
    /// * `omega_spread` — natural frequency spread (rad/s).
    pub fn new(
        n: usize,
        topology: &NetworkTopology,
        coupling: f64,
        mu_spread: f64,
        omega_spread: f64,
        seed: u64,
    ) -> Self {
        let n = n.clamp(1, 16);
        let mut rng = SimpleRng::new(seed ^ 0xDEAD_BEEF);
        let x: Vec<f64> = (0..n).map(|_| rng.next_f64() * 0.1).collect();
        let y: Vec<f64> = (0..n).map(|_| rng.next_f64() * 0.1).collect();
        let mu: Vec<f64> = (0..n)
            .map(|i| {
                let t = if n > 1 { i as f64 / (n - 1) as f64 } else { 0.5 };
                0.5 - mu_spread + 2.0 * mu_spread * t
            })
            .collect();
        let omega: Vec<f64> = (0..n)
            .map(|i| {
                let t = if n > 1 { i as f64 / (n - 1) as f64 } else { 0.5 };
                TAU * (1.0 - omega_spread / 2.0 + omega_spread * t)
            })
            .collect();
        let graph = CouplingGraph::from_topology(topology, n, coupling, seed);
        let amplitudes = vec![0.0f64; n];
        Self { x, y, mu, omega, graph, amplitudes, amplitude_death: false, death_threshold: 1e-4 }
    }

    /// Step the network forward by `dt` seconds using Euler integration.
    pub fn step(&mut self, dt: f64) {
        let n = self.x.len();
        let mut dx = vec![0.0f64; n];
        let mut dy = vec![0.0f64; n];

        for i in 0..n {
            let amp_sq = self.x[i].powi(2) + self.y[i].powi(2);
            let mu_i = self.mu[i];
            let om_i = self.omega[i];

            // Intrinsic Stuart–Landau dynamics.
            let fx = (mu_i - amp_sq) * self.x[i] - om_i * self.y[i];
            let fy = (mu_i - amp_sq) * self.y[i] + om_i * self.x[i];

            // Coupling: sum over neighbours (diffusive, real part only for simplicity).
            let mut cx = 0.0f64;
            let mut cy = 0.0f64;
            for j in 0..n {
                let w = self.graph.get(i, j);
                if w != 0.0 {
                    cx += w * (self.x[j] - self.x[i]);
                    cy += w * (self.y[j] - self.y[i]);
                }
            }

            dx[i] = fx + cx;
            dy[i] = fy + cy;
        }

        for i in 0..n {
            self.x[i] += dt * dx[i];
            self.y[i] += dt * dy[i];
            // Clamp to prevent blow-up (rare but possible with large coupling).
            self.x[i] = self.x[i].clamp(-100.0, 100.0);
            self.y[i] = self.y[i].clamp(-100.0, 100.0);
            self.amplitudes[i] = (self.x[i].powi(2) + self.y[i].powi(2)).sqrt();
        }

        let max_amp = self.amplitudes.iter().cloned().fold(0.0f64, f64::max);
        self.amplitude_death = max_amp < self.death_threshold;
    }

    /// Return normalised amplitude of each oscillator ∈ [0, 1].
    ///
    /// The maximum observed amplitude is used for normalisation.
    pub fn normalised_amplitudes(&self) -> Vec<f64> {
        let max = self.amplitudes.iter().cloned().fold(f64::EPSILON, f64::max);
        self.amplitudes.iter().map(|&a| (a / max).clamp(0.0, 1.0)).collect()
    }

    /// Return the instantaneous phase of each oscillator (atan2(y, x)) in [−π, π].
    pub fn phases(&self) -> Vec<f64> {
        self.x
            .iter()
            .zip(self.y.iter())
            .map(|(&xi, &yi)| yi.atan2(xi))
            .collect()
    }

    /// Set μᵢ for all nodes (e.g. to trigger amplitude death by going negative).
    pub fn set_mu_uniform(&mut self, mu: f64) {
        for v in &mut self.mu {
            *v = mu;
        }
    }

    /// Return audio-ready frequency for oscillator i.
    ///
    /// Maps the oscillator's phase velocity to an audio frequency.
    /// Base frequency + omega contribution.
    pub fn audio_frequency(&self, i: usize, base_freq_hz: f64) -> f64 {
        let om = self.omega.get(i).copied().unwrap_or(TAU);
        base_freq_hz + om / TAU * 10.0  // omega in Hz, scaled
    }
}

// ---------------------------------------------------------------------------
// Oscillator network — unified voice API
// ---------------------------------------------------------------------------

/// Oscillator model variant.
#[derive(Debug, Clone, PartialEq)]
pub enum OscillatorModel {
    Kuramoto,
    StuartLandau,
}

/// Snapshot of network state for one control-rate tick.
///
/// Consumed by the audio thread to set per-voice frequencies and amplitudes.
#[derive(Debug, Clone)]
pub struct NetworkState {
    /// Number of oscillators.
    pub n: usize,
    /// Audio frequency for each voice (Hz).
    pub frequencies: Vec<f64>,
    /// Amplitude (0–1) for each voice.
    pub amplitudes: Vec<f64>,
    /// Phase of each oscillator (radians).
    pub phases: Vec<f64>,
    /// Global order parameter (Kuramoto r or mean amplitude).
    pub order_parameter: f64,
    /// True if the network is in amplitude death.
    pub amplitude_death: bool,
    /// Model type tag.
    pub model: OscillatorModel,
    /// Number of active (non-silent) voices.
    pub active_voices: usize,
}

impl NetworkState {
    /// Return an amplitude-sorted list of (frequency, amplitude) pairs for
    /// polyphonic audio voice assignment (loudest first).
    pub fn sorted_voices(&self) -> Vec<(f64, f64)> {
        let mut pairs: Vec<(f64, f64)> = self
            .frequencies
            .iter()
            .zip(self.amplitudes.iter())
            .map(|(&f, &a)| (f, a))
            .collect();
        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        pairs
    }
}

/// Unified wrapper around a Kuramoto or Stuart–Landau oscillator network.
///
/// Exposes a simple polling interface: call [`OscillatorNetwork::step`] at
/// the control rate and read [`OscillatorNetwork::state`] to obtain per-voice
/// parameters.
pub struct OscillatorNetwork {
    kuramoto: Option<KuramotoNetwork>,
    stuart_landau: Option<StuartLandauNetwork>,
    model: OscillatorModel,
    /// Base audio frequency for the lowest-frequency voice (Hz).
    pub base_freq_hz: f64,
    /// Total audio frequency span across the network (Hz).
    pub freq_range_hz: f64,
    pub n: usize,
    cached_state: NetworkState,
}

impl OscillatorNetwork {
    /// Create a Kuramoto oscillator network.
    pub fn kuramoto(
        n: usize,
        topology: &NetworkTopology,
        coupling: f64,
        freq_spread: f64,
        base_freq_hz: f64,
        freq_range_hz: f64,
        seed: u64,
    ) -> Self {
        let n = n.clamp(1, 16);
        let net = KuramotoNetwork::new(n, topology, coupling, freq_spread, seed);
        let cached_state = Self::build_kuramoto_state(&net, base_freq_hz, freq_range_hz);
        Self {
            kuramoto: Some(net),
            stuart_landau: None,
            model: OscillatorModel::Kuramoto,
            base_freq_hz,
            freq_range_hz,
            n,
            cached_state,
        }
    }

    /// Create a Stuart–Landau oscillator network.
    pub fn stuart_landau(
        n: usize,
        topology: &NetworkTopology,
        coupling: f64,
        mu_spread: f64,
        omega_spread: f64,
        base_freq_hz: f64,
        freq_range_hz: f64,
        seed: u64,
    ) -> Self {
        let n = n.clamp(1, 16);
        let net = StuartLandauNetwork::new(n, topology, coupling, mu_spread, omega_spread, seed);
        let cached_state = Self::build_sl_state(&net, base_freq_hz);
        Self {
            kuramoto: None,
            stuart_landau: Some(net),
            model: OscillatorModel::StuartLandau,
            base_freq_hz,
            freq_range_hz,
            n,
            cached_state,
        }
    }

    /// Advance the network by `dt` seconds.
    pub fn step(&mut self, dt: f64) {
        match self.model {
            OscillatorModel::Kuramoto => {
                if let Some(ref mut net) = self.kuramoto {
                    net.step(dt);
                    self.cached_state = Self::build_kuramoto_state(net, self.base_freq_hz, self.freq_range_hz);
                }
            }
            OscillatorModel::StuartLandau => {
                if let Some(ref mut net) = self.stuart_landau {
                    net.step(dt);
                    self.cached_state = Self::build_sl_state(net, self.base_freq_hz);
                }
            }
        }
    }

    /// Return the latest [`NetworkState`] (updated on each call to `step`).
    pub fn state(&self) -> &NetworkState {
        &self.cached_state
    }

    /// Change the network topology at runtime.
    pub fn set_topology(&mut self, topology: &NetworkTopology, seed: u64) {
        match self.model {
            OscillatorModel::Kuramoto => {
                if let Some(ref mut net) = self.kuramoto {
                    net.set_topology(topology, seed);
                }
            }
            OscillatorModel::StuartLandau => {
                if let Some(ref mut net) = self.stuart_landau {
                    net.graph = CouplingGraph::from_topology(topology, net.x.len(), net.graph.get(0, 1).max(1e-6), seed);
                }
            }
        }
    }

    /// Set coupling strength (rebuilds graph).
    pub fn set_coupling(&mut self, coupling: f64, topology: &NetworkTopology, seed: u64) {
        match self.model {
            OscillatorModel::Kuramoto => {
                if let Some(ref mut net) = self.kuramoto {
                    net.coupling = coupling;
                    net.graph = CouplingGraph::from_topology(topology, net.phases.len(), coupling, seed);
                }
            }
            OscillatorModel::StuartLandau => {
                if let Some(ref mut net) = self.stuart_landau {
                    net.graph = CouplingGraph::from_topology(topology, net.x.len(), coupling, seed);
                }
            }
        }
    }

    // ---- private state builders -------------------------------------------

    fn build_kuramoto_state(net: &KuramotoNetwork, base_freq: f64, range: f64) -> NetworkState {
        let n = net.phases.len();
        let frequencies = net.oscillator_frequencies(base_freq, range);
        let amplitudes = net.oscillator_amplitudes();
        let phases = net.phases.clone();
        let active_voices = amplitudes.iter().filter(|&&a| a > 0.01).count();
        NetworkState {
            n,
            frequencies,
            amplitudes,
            phases,
            order_parameter: net.order_parameter,
            amplitude_death: false,
            model: OscillatorModel::Kuramoto,
            active_voices,
        }
    }

    fn build_sl_state(net: &StuartLandauNetwork, base_freq: f64) -> NetworkState {
        let n = net.x.len();
        let frequencies: Vec<f64> = (0..n).map(|i| net.audio_frequency(i, base_freq)).collect();
        let amplitudes = net.normalised_amplitudes();
        let phases = net.phases();
        let active_voices = amplitudes.iter().filter(|&&a| a > 0.01).count();
        NetworkState {
            n,
            frequencies,
            amplitudes,
            phases,
            order_parameter: amplitudes.iter().sum::<f64>() / n as f64,
            amplitude_death: net.amplitude_death,
            model: OscillatorModel::StuartLandau,
            active_voices,
        }
    }
}

// ---------------------------------------------------------------------------
// Simple deterministic PRNG (xorshift64) — avoids rand crate dependency
// ---------------------------------------------------------------------------

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn next_usize(&mut self, n: usize) -> usize {
        if n == 0 { return 0; }
        (self.next_u64() as usize) % n
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- CouplingGraph ------------------------------------------------------

    #[test]
    fn ring_graph_degree_is_two() {
        let g = CouplingGraph::from_topology(&NetworkTopology::Ring, 6, 1.0, 0);
        for i in 0..6 {
            assert!((g.degree(i) - 2.0).abs() < 1e-10, "node {i} degree should be 2, got {}", g.degree(i));
        }
    }

    #[test]
    fn fully_connected_edge_count() {
        let n = 4;
        let g = CouplingGraph::from_topology(&NetworkTopology::FullyConnected, n, 1.0, 0);
        assert_eq!(g.edge_count(), n * (n - 1));
    }

    #[test]
    fn star_hub_has_n_minus_one_connections() {
        let n = 5;
        let g = CouplingGraph::from_topology(&NetworkTopology::StarGraph, n, 1.0, 0);
        // Hub (node 0) should have n-1 in-edges.
        let hub_degree: f64 = (1..n).map(|j| g.get(0, j)).sum();
        assert!((hub_degree - (n as f64 - 1.0)).abs() < 1e-10);
    }

    #[test]
    fn coupling_graph_set_get_roundtrip() {
        let mut g = CouplingGraph::new(4);
        g.set(1, 2, 0.75);
        assert!((g.get(1, 2) - 0.75).abs() < 1e-15);
    }

    #[test]
    fn small_world_has_correct_node_count() {
        let g = CouplingGraph::from_topology(&NetworkTopology::SmallWorld { rewire_prob: 0.3 }, 8, 1.0, 42);
        assert_eq!(g.n, 8);
    }

    #[test]
    fn erdos_renyi_p0_is_empty() {
        let g = CouplingGraph::from_topology(&NetworkTopology::RandomErdos { connect_prob: 0.0 }, 6, 1.0, 1);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn erdos_renyi_p1_is_fully_connected() {
        let n = 5;
        let g = CouplingGraph::from_topology(&NetworkTopology::RandomErdos { connect_prob: 1.0 }, n, 1.0, 2);
        assert_eq!(g.edge_count(), n * (n - 1));
    }

    // ---- KuramotoNetwork ----------------------------------------------------

    #[test]
    fn kuramoto_phases_stay_in_range() {
        let mut net = KuramotoNetwork::new(8, &NetworkTopology::Ring, 1.0, 0.5, 0);
        for _ in 0..500 {
            net.step(0.01);
        }
        for &ph in &net.phases {
            assert!(ph >= 0.0 && ph < TAU, "phase out of range: {ph}");
        }
    }

    #[test]
    fn kuramoto_order_parameter_in_range() {
        let mut net = KuramotoNetwork::new(6, &NetworkTopology::FullyConnected, 5.0, 0.1, 0);
        for _ in 0..500 {
            net.step(0.01);
        }
        assert!(net.order_parameter >= 0.0 && net.order_parameter <= 1.0 + 1e-9);
    }

    #[test]
    fn kuramoto_high_coupling_synchronizes() {
        let mut net = KuramotoNetwork::new(8, &NetworkTopology::FullyConnected, 20.0, 0.2, 1);
        for _ in 0..2000 {
            net.step(0.01);
        }
        assert!(net.order_parameter > 0.8, "high coupling should synchronize: r={}", net.order_parameter);
    }

    #[test]
    fn kuramoto_oscillator_frequencies_count() {
        let net = KuramotoNetwork::new(4, &NetworkTopology::Ring, 1.0, 0.5, 0);
        let freqs = net.oscillator_frequencies(440.0, 100.0);
        assert_eq!(freqs.len(), 4);
        for &f in &freqs {
            assert!(f >= 440.0 && f <= 540.0, "freq {f} out of expected range");
        }
    }

    #[test]
    fn kuramoto_amplitudes_in_range() {
        let net = KuramotoNetwork::new(6, &NetworkTopology::Ring, 1.0, 0.3, 0);
        for &a in net.oscillator_amplitudes().iter() {
            assert!(a >= 0.0 && a <= 1.0, "amplitude {a} out of [0,1]");
        }
    }

    // ---- StuartLandauNetwork ------------------------------------------------

    #[test]
    fn stuart_landau_amplitudes_non_negative() {
        let mut net = StuartLandauNetwork::new(4, &NetworkTopology::Ring, 0.5, 0.2, 0.1, 0);
        for _ in 0..200 {
            net.step(0.005);
        }
        for &a in &net.amplitudes {
            assert!(a >= 0.0, "amplitude should be non-negative: {a}");
        }
    }

    #[test]
    fn stuart_landau_amplitude_death_at_negative_mu() {
        let mut net = StuartLandauNetwork::new(4, &NetworkTopology::FullyConnected, 0.1, 0.1, 0.1, 99);
        net.set_mu_uniform(-2.0);
        for _ in 0..5000 {
            net.step(0.005);
        }
        assert!(net.amplitude_death, "should reach amplitude death with negative mu");
    }

    #[test]
    fn stuart_landau_phases_are_finite() {
        let mut net = StuartLandauNetwork::new(4, &NetworkTopology::StarGraph, 0.3, 0.3, 0.5, 5);
        for _ in 0..100 {
            net.step(0.01);
        }
        for ph in net.phases() {
            assert!(ph.is_finite(), "phase {ph} is not finite");
        }
    }

    #[test]
    fn stuart_landau_normalised_amplitudes_in_range() {
        let mut net = StuartLandauNetwork::new(6, &NetworkTopology::Ring, 0.5, 0.5, 0.3, 7);
        for _ in 0..200 {
            net.step(0.01);
        }
        for &a in net.normalised_amplitudes().iter() {
            assert!(a >= 0.0 && a <= 1.0 + 1e-10, "normalised amplitude {a} out of [0,1]");
        }
    }

    // ---- OscillatorNetwork --------------------------------------------------

    #[test]
    fn oscillator_network_kuramoto_state_size() {
        let mut net = OscillatorNetwork::kuramoto(6, &NetworkTopology::Ring, 1.0, 0.3, 220.0, 220.0, 0);
        net.step(0.01);
        let st = net.state();
        assert_eq!(st.n, 6);
        assert_eq!(st.frequencies.len(), 6);
        assert_eq!(st.amplitudes.len(), 6);
    }

    #[test]
    fn oscillator_network_sl_state_amplitudes_non_negative() {
        let mut net = OscillatorNetwork::stuart_landau(
            4, &NetworkTopology::FullyConnected, 0.5, 0.3, 0.5, 110.0, 110.0, 42,
        );
        for _ in 0..50 {
            net.step(0.01);
        }
        let st = net.state();
        for &a in &st.amplitudes {
            assert!(a >= 0.0);
        }
    }

    #[test]
    fn oscillator_network_max_16_voices() {
        let net = OscillatorNetwork::kuramoto(
            32, // request 32, should clamp to 16
            &NetworkTopology::FullyConnected,
            1.0,
            0.5,
            220.0,
            440.0,
            0,
        );
        assert_eq!(net.n, 16);
        assert_eq!(net.state().n, 16);
    }

    #[test]
    fn network_state_sorted_voices_descending() {
        let mut net = OscillatorNetwork::kuramoto(4, &NetworkTopology::Ring, 2.0, 0.5, 220.0, 440.0, 1);
        for _ in 0..100 {
            net.step(0.01);
        }
        let voices = net.state().sorted_voices();
        for pair in voices.windows(2) {
            assert!(pair[0].1 >= pair[1].1, "sorted voices should be descending by amplitude");
        }
    }

    #[test]
    fn oscillator_network_set_coupling_does_not_panic() {
        let mut net = OscillatorNetwork::kuramoto(4, &NetworkTopology::Ring, 1.0, 0.5, 220.0, 220.0, 0);
        net.set_coupling(3.0, &NetworkTopology::FullyConnected, 7);
        net.step(0.01);
    }

    // ---- SimpleRng ----------------------------------------------------------

    #[test]
    fn simple_rng_f64_in_range() {
        let mut rng = SimpleRng::new(12345);
        for _ in 0..1000 {
            let v = rng.next_f64();
            assert!(v >= 0.0 && v < 1.0, "rng value {v} out of [0,1)");
        }
    }

    #[test]
    fn simple_rng_usize_in_range() {
        let mut rng = SimpleRng::new(99);
        for _ in 0..1000 {
            let v = rng.next_usize(7);
            assert!(v < 7, "rng usize {v} >= 7");
        }
    }

    // ---- NetworkTopology labels ---------------------------------------------

    #[test]
    fn topology_labels_non_empty() {
        let topologies: Vec<NetworkTopology> = vec![
            NetworkTopology::Ring,
            NetworkTopology::StarGraph,
            NetworkTopology::SmallWorld { rewire_prob: 0.2 },
            NetworkTopology::RandomErdos { connect_prob: 0.5 },
            NetworkTopology::FullyConnected,
        ];
        for t in &topologies {
            assert!(!t.label().is_empty());
        }
    }
}
