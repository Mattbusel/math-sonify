//! Genetic algorithm-based parameter evolution for ODE systems.
//!
//! This module implements [`ParameterEvolution`], which maintains a population
//! of candidate ODE parameter sets and evolves them over generations to
//! maximise a chosen [`FitnessMetric`].  The best individual at the end of
//! each generation is stored in a [`SavedEvolution`] that can be written to
//! disk as a named preset.
//!
//! # Overview
//!
//! The evolutionary loop is:
//!
//! 1. Initialise a random population of parameter vectors drawn from the
//!    system's natural parameter bounds.
//! 2. Evaluate each individual by running the ODE forward for a short burst
//!    and computing the selected [`FitnessMetric`] over the resulting
//!    trajectory.
//! 3. Select parents via tournament selection.
//! 4. Produce offspring via uniform crossover and Gaussian mutation.
//! 5. Elitism: the best individual is carried forward unchanged.
//! 6. Repeat for `EvolutionConfig::generations` generations.
//!
//! The GUI can read [`EvolutionState`] (behind a `Mutex`) every frame to draw
//! a generation counter, best-fitness bar, and a sparkline of fitness history.

use std::fmt;
use parking_lot::Mutex;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Fitness metric
// ---------------------------------------------------------------------------

/// How the fitness of a parameter set is judged after simulating a short
/// trajectory.
#[derive(Clone, Debug, PartialEq)]
pub enum FitnessMetric {
    /// Reward overtone density: high fitness when many harmonically related
    /// frequency components are present in the resulting audio output.
    HarmonicRichness,
    /// Reward rhythmic interest: high fitness when the amplitude envelope
    /// shows varied inter-onset intervals (not too regular, not too noisy).
    RhythmicVariance,
    /// Reward timbral diversity: high fitness when the spectral centroid
    /// changes significantly over time, indicating evolving timbre.
    TimbralDiversity,
    /// User-defined fitness function receiving the raw state trajectory.
    ///
    /// The closure receives a slice of state snapshots `&[[f64; 16]]`
    /// (each snapshot holds up to 16 state variables, zero-padded) and
    /// returns a fitness score in [0, 1].
    UserDefined(Arc<dyn Fn(&[[f64; 16]]) -> f64 + Send + Sync>),
}

impl fmt::Display for FitnessMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FitnessMetric::HarmonicRichness => write!(f, "Harmonic Richness"),
            FitnessMetric::RhythmicVariance => write!(f, "Rhythmic Variance"),
            FitnessMetric::TimbralDiversity => write!(f, "Timbral Diversity"),
            FitnessMetric::UserDefined(_) => write!(f, "User Defined"),
        }
    }
}

// ---------------------------------------------------------------------------
// Evolution configuration
// ---------------------------------------------------------------------------

/// Configuration for the genetic algorithm.
#[derive(Clone, Debug)]
pub struct EvolutionConfig {
    /// Number of individuals in the population.
    ///
    /// Larger populations explore more of parameter space but take longer per
    /// generation. Default: `40`.
    pub population_size: usize,

    /// Number of generations to run before stopping.
    ///
    /// Default: `50`.
    pub generations: usize,

    /// Probability that any single parameter is perturbed during mutation.
    ///
    /// At each gene locus, with probability `mutation_rate` a Gaussian
    /// perturbation with σ = 5 % of the parameter range is applied.
    /// Default: `0.1`.
    pub mutation_rate: f64,

    /// Probability that two parent genomes are crossed over at a random
    /// locus.  When crossover does not occur both offspring are clones of
    /// their respective parents before mutation.  Default: `0.7`.
    pub crossover_rate: f64,

    /// Number of ODE steps simulated per fitness evaluation.
    ///
    /// More steps give a more representative trajectory but slow evaluation.
    /// Default: `2000`.
    pub eval_steps: usize,

    /// ODE integration timestep used during fitness evaluation.
    ///
    /// Default: `0.005`.
    pub eval_dt: f64,

    /// Tournament size for parent selection.
    ///
    /// A random sample of this many individuals is drawn and the best one
    /// is selected as a parent.  Default: `3`.
    pub tournament_size: usize,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            population_size: 40,
            generations: 50,
            mutation_rate: 0.1,
            crossover_rate: 0.7,
            eval_steps: 2000,
            eval_dt: 0.005,
            tournament_size: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Individual
// ---------------------------------------------------------------------------

/// A single individual in the population: a parameter vector plus its
/// most recently evaluated fitness score.
#[derive(Clone, Debug)]
pub struct Individual {
    /// ODE parameters (length determined by the system).
    pub params: Vec<f64>,
    /// Fitness score in [0, 1].  `f64::NEG_INFINITY` until evaluated.
    pub fitness: f64,
}

impl Individual {
    /// Construct an unevaluated individual with the given parameters.
    pub fn new(params: Vec<f64>) -> Self {
        Self {
            params,
            fitness: f64::NEG_INFINITY,
        }
    }
}

// ---------------------------------------------------------------------------
// Saved evolution (best individual → preset)
// ---------------------------------------------------------------------------

/// The best individual found during an evolution run, ready to be persisted as
/// a named preset.
#[derive(Clone, Debug)]
pub struct SavedEvolution {
    /// Human-readable name for this evolved preset.
    pub name: String,
    /// The evolved ODE parameter vector.
    pub params: Vec<f64>,
    /// Fitness score of this individual.
    pub fitness: f64,
    /// Which metric was optimised.
    pub metric: String,
    /// Generation at which this best individual was recorded.
    pub generation: usize,
}

impl SavedEvolution {
    /// Serialise to a simple TOML-compatible string that can be appended to a
    /// presets file.
    ///
    /// # Example
    ///
    /// ```
    /// use math_sonify_plugin::evolution::SavedEvolution;
    ///
    /// let se = SavedEvolution {
    ///     name: "My Evolved Preset".into(),
    ///     params: vec![10.0, 28.0, 2.666],
    ///     fitness: 0.87,
    ///     metric: "HarmonicRichness".into(),
    ///     generation: 42,
    /// };
    /// let s = se.to_toml_snippet();
    /// assert!(s.contains("My Evolved Preset"));
    /// ```
    pub fn to_toml_snippet(&self) -> String {
        let params_str = self
            .params
            .iter()
            .map(|p| format!("{p:.6}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "[[evolved_preset]]\n\
             name = \"{}\"\n\
             metric = \"{}\"\n\
             fitness = {:.6}\n\
             generation = {}\n\
             params = [{}]\n",
            self.name, self.metric, self.fitness, self.generation, params_str
        )
    }
}

// ---------------------------------------------------------------------------
// Evolution state (shared with the GUI)
// ---------------------------------------------------------------------------

/// Observable snapshot of an in-progress or completed evolution run.
///
/// The GUI reads this (behind a `Mutex`) every frame to render live feedback.
#[derive(Clone, Debug, Default)]
pub struct EvolutionState {
    /// Generation currently being evaluated (0-based).
    pub current_generation: usize,
    /// Best fitness seen so far across all generations.
    pub best_fitness: f64,
    /// Fitness of the best individual at the end of *each completed* generation.
    ///
    /// This is the data for the sparkline plot.
    pub fitness_history: Vec<f64>,
    /// Parameter vector of the current best individual.
    pub best_params: Vec<f64>,
    /// `true` while the background evolution thread is running.
    pub running: bool,
    /// Total number of generations configured.
    pub total_generations: usize,
}

/// Thread-safe handle to [`EvolutionState`].
pub type SharedEvolutionState = Arc<Mutex<EvolutionState>>;

// ---------------------------------------------------------------------------
// Parameter bounds
// ---------------------------------------------------------------------------

/// Inclusive [min, max] bounds for a single ODE parameter.
#[derive(Clone, Copy, Debug)]
pub struct ParamBounds {
    pub min: f64,
    pub max: f64,
}

impl ParamBounds {
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Width of the interval.
    pub fn range(&self) -> f64 {
        self.max - self.min
    }

    /// Clamp a value into [min, max].
    pub fn clamp(&self, v: f64) -> f64 {
        v.clamp(self.min, self.max)
    }
}

// ---------------------------------------------------------------------------
// Parameter evolution engine
// ---------------------------------------------------------------------------

/// Genetic algorithm that evolves ODE parameter sets to maximise a
/// [`FitnessMetric`].
///
/// # Usage
///
/// ```no_run
/// use math_sonify_plugin::evolution::{
///     ParameterEvolution, EvolutionConfig, FitnessMetric, ParamBounds,
/// };
/// use std::sync::Arc;
/// use parking_lot::Mutex;
///
/// let bounds = vec![
///     ParamBounds::new(5.0, 15.0),   // sigma
///     ParamBounds::new(20.0, 35.0),  // rho
///     ParamBounds::new(1.0, 4.0),    // beta
/// ];
/// let state = Arc::new(Mutex::new(Default::default()));
/// let mut evo = ParameterEvolution::new(
///     EvolutionConfig::default(),
///     FitnessMetric::HarmonicRichness,
///     bounds,
///     state,
/// );
/// // Run in a background thread; `state` is updated each generation.
/// // evo.run(ode_fn);
/// ```
pub struct ParameterEvolution {
    config: EvolutionConfig,
    metric: FitnessMetric,
    bounds: Vec<ParamBounds>,
    state: SharedEvolutionState,
    population: Vec<Individual>,
    rng_seed: u64,
}

impl ParameterEvolution {
    /// Create a new evolution engine.
    ///
    /// - `config` — algorithm hyper-parameters.
    /// - `metric` — fitness criterion.
    /// - `bounds` — per-parameter [min, max] constraints; also determines the
    ///   genome length.
    /// - `state` — shared observable state for the GUI.
    pub fn new(
        config: EvolutionConfig,
        metric: FitnessMetric,
        bounds: Vec<ParamBounds>,
        state: SharedEvolutionState,
    ) -> Self {
        let rng_seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(12345);
        Self {
            config,
            metric,
            bounds,
            state,
            population: Vec::new(),
            rng_seed,
        }
    }

    // ── PRNG (xorshift64) ─────────────────────────────────────────────────

    fn next_u64(&mut self) -> u64 {
        let mut x = self.rng_seed;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_seed = x;
        x
    }

    /// Uniform float in [0, 1).
    fn rand_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Box-Muller Gaussian sample N(0, 1).
    fn rand_normal(&mut self) -> f64 {
        let u1 = self.rand_f64().max(1e-15);
        let u2 = self.rand_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    // ── Initialisation ────────────────────────────────────────────────────

    fn initialise_population(&mut self) {
        self.population = (0..self.config.population_size)
            .map(|_| {
                let params = self
                    .bounds
                    .iter()
                    .map(|b| b.min + self.rand_f64() * b.range())
                    .collect();
                Individual::new(params)
            })
            .collect();
    }

    // ── Fitness evaluation ────────────────────────────────────────────────

    /// Evaluate fitness for every individual whose score is `NEG_INFINITY`.
    ///
    /// The `ode_step` closure advances one integration step: it receives the
    /// current state vector (mutable, up to 16 elements) and the parameter
    /// slice, and writes the new state in place.
    fn evaluate_population<F>(&mut self, ode_step: &F)
    where
        F: Fn(&mut [f64; 16], &[f64]) + Sync,
    {
        for individual in &mut self.population {
            if individual.fitness.is_finite() {
                continue;
            }
            individual.fitness =
                self.evaluate_individual(&individual.params.clone(), ode_step);
        }
    }

    fn evaluate_individual<F>(&mut self, params: &[f64], ode_step: &F) -> f64
    where
        F: Fn(&mut [f64; 16], &[f64]),
    {
        let mut state = [0.01f64; 16];
        let mut trajectory: Vec<[f64; 16]> = Vec::with_capacity(self.config.eval_steps);

        // Warm up to avoid transient bias.
        for _ in 0..200 {
            ode_step(&mut state, params);
        }
        // Collect trajectory.
        for _ in 0..self.config.eval_steps {
            ode_step(&mut state, params);
            trajectory.push(state);
        }

        self.compute_fitness(&trajectory)
    }

    /// Compute the fitness score from a recorded trajectory.
    fn compute_fitness(&self, trajectory: &[[f64; 16]]) -> f64 {
        if trajectory.is_empty() {
            return 0.0;
        }
        match &self.metric {
            FitnessMetric::HarmonicRichness => harmonic_richness(trajectory),
            FitnessMetric::RhythmicVariance => rhythmic_variance(trajectory),
            FitnessMetric::TimbralDiversity => timbral_diversity(trajectory),
            FitnessMetric::UserDefined(f) => f(trajectory).clamp(0.0, 1.0),
        }
    }

    // ── Selection ─────────────────────────────────────────────────────────

    fn tournament_select(&mut self) -> usize {
        let k = self.config.tournament_size.min(self.population.len());
        let mut best_idx = self.random_index(self.population.len());
        for _ in 1..k {
            let idx = self.random_index(self.population.len());
            if self.population[idx].fitness > self.population[best_idx].fitness {
                best_idx = idx;
            }
        }
        best_idx
    }

    fn random_index(&mut self, len: usize) -> usize {
        (self.next_u64() as usize) % len
    }

    // ── Crossover ─────────────────────────────────────────────────────────

    fn crossover(&mut self, a: &[f64], b: &[f64]) -> (Vec<f64>, Vec<f64>) {
        if self.rand_f64() > self.config.crossover_rate || a.len() != b.len() {
            return (a.to_vec(), b.to_vec());
        }
        let mut child_a = a.to_vec();
        let mut child_b = b.to_vec();
        for i in 0..a.len() {
            if self.rand_f64() < 0.5 {
                child_a[i] = b[i];
                child_b[i] = a[i];
            }
        }
        (child_a, child_b)
    }

    // ── Mutation ──────────────────────────────────────────────────────────

    fn mutate(&mut self, params: &mut Vec<f64>) {
        for (p, b) in params.iter_mut().zip(self.bounds.iter()) {
            if self.rand_f64() < self.config.mutation_rate {
                let sigma = b.range() * 0.05;
                *p += self.rand_normal() * sigma;
                *p = b.clamp(*p);
            }
        }
    }

    // ── Best individual ───────────────────────────────────────────────────

    fn best_index(&self) -> usize {
        self.population
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.fitness.partial_cmp(&b.fitness).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    // ── Main loop ─────────────────────────────────────────────────────────

    /// Run the full evolutionary loop.
    ///
    /// `ode_step` is a closure that advances the ODE by one step of size
    /// `config.eval_dt`. It receives `(state, params)` and mutates `state`
    /// in place.
    ///
    /// This method **blocks** until all generations are complete and is
    /// designed to be called on a dedicated background thread.  The shared
    /// [`EvolutionState`] is updated after every generation so the GUI can
    /// render live progress.
    ///
    /// Returns the [`SavedEvolution`] for the globally best individual found.
    pub fn run<F>(&mut self, preset_name: &str, ode_step: F) -> SavedEvolution
    where
        F: Fn(&mut [f64; 16], &[f64]) + Sync,
    {
        self.initialise_population();

        {
            let mut s = self.state.lock();
            s.running = true;
            s.current_generation = 0;
            s.best_fitness = f64::NEG_INFINITY;
            s.fitness_history.clear();
            s.total_generations = self.config.generations;
        }

        let mut global_best = Individual::new(
            self.bounds.iter().map(|b| (b.min + b.max) * 0.5).collect(),
        );

        for gen in 0..self.config.generations {
            // Evaluate.
            self.evaluate_population(&ode_step);

            // Track best.
            let bi = self.best_index();
            let gen_best = self.population[bi].clone();
            if gen_best.fitness > global_best.fitness {
                global_best = gen_best.clone();
            }

            // Update shared state for GUI.
            {
                let mut s = self.state.lock();
                s.current_generation = gen + 1;
                s.best_fitness = global_best.fitness;
                s.fitness_history.push(global_best.fitness);
                s.best_params = global_best.params.clone();
            }

            // Build next generation.
            let mut next_gen: Vec<Individual> = Vec::with_capacity(self.config.population_size);

            // Elitism: carry the best individual forward unchanged.
            next_gen.push(Individual {
                params: global_best.params.clone(),
                fitness: global_best.fitness,
            });

            while next_gen.len() < self.config.population_size {
                let pa_idx = self.tournament_select();
                let pb_idx = self.tournament_select();
                let pa = self.population[pa_idx].params.clone();
                let pb = self.population[pb_idx].params.clone();
                let (mut ca, mut cb) = self.crossover(&pa, &pb);
                self.mutate(&mut ca);
                self.mutate(&mut cb);
                next_gen.push(Individual::new(ca));
                if next_gen.len() < self.config.population_size {
                    next_gen.push(Individual::new(cb));
                }
            }

            self.population = next_gen;
        }

        {
            let mut s = self.state.lock();
            s.running = false;
        }

        SavedEvolution {
            name: preset_name.to_string(),
            params: global_best.params,
            fitness: global_best.fitness,
            metric: self.metric.to_string(),
            generation: self.config.generations,
        }
    }
}

// ---------------------------------------------------------------------------
// Fitness metric implementations
// ---------------------------------------------------------------------------

/// Harmonic richness: estimate overtone density from autocorrelation of the
/// first state variable's trajectory.
///
/// A high autocorrelation peak ratio relative to the zero-lag value indicates
/// strong periodic (harmonic) content; near-zero ratio indicates noise.
fn harmonic_richness(trajectory: &[[f64; 16]]) -> f64 {
    let n = trajectory.len();
    if n < 64 {
        return 0.0;
    }
    let x: Vec<f64> = trajectory.iter().map(|s| s[0]).collect();
    let mean = x.iter().sum::<f64>() / n as f64;
    let x: Vec<f64> = x.iter().map(|v| v - mean).collect();

    // Zero-lag (variance).
    let r0: f64 = x.iter().map(|v| v * v).sum::<f64>() / n as f64;
    if r0 < 1e-12 {
        return 0.0;
    }

    // Search for a peak in lag range [8, n/4].
    let max_lag = n / 4;
    let mut peak = 0.0f64;
    for lag in 8..max_lag {
        let r: f64 = x[..n - lag]
            .iter()
            .zip(&x[lag..])
            .map(|(a, b)| a * b)
            .sum::<f64>()
            / (n - lag) as f64;
        peak = peak.max(r.abs());
    }
    (peak / r0).clamp(0.0, 1.0)
}

/// Rhythmic variance: measure variance of amplitude envelope inter-onset
/// intervals.
///
/// High variance → complex rhythm; very low variance → monotonous pulse;
/// extremely high variance → arrhythmic noise.  The score peaks at moderate
/// coefficient-of-variation values (around 0.3–0.7).
fn rhythmic_variance(trajectory: &[[f64; 16]]) -> f64 {
    let n = trajectory.len();
    if n < 32 {
        return 0.0;
    }

    // Amplitude envelope via abs of first variable, smoothed.
    let env: Vec<f64> = trajectory.iter().map(|s| s[0].abs()).collect();
    let window = 8_usize;
    let smoothed: Vec<f64> = env
        .windows(window)
        .map(|w| w.iter().sum::<f64>() / window as f64)
        .collect();

    // Detect onsets (local maxima above mean).
    let mean = smoothed.iter().sum::<f64>() / smoothed.len() as f64;
    let mut onsets: Vec<usize> = Vec::new();
    for i in 1..smoothed.len().saturating_sub(1) {
        if smoothed[i] > mean && smoothed[i] > smoothed[i - 1] && smoothed[i] > smoothed[i + 1] {
            onsets.push(i);
        }
    }

    if onsets.len() < 3 {
        return 0.0;
    }

    // Inter-onset intervals.
    let iois: Vec<f64> = onsets
        .windows(2)
        .map(|w| (w[1] - w[0]) as f64)
        .collect();
    let mean_ioi = iois.iter().sum::<f64>() / iois.len() as f64;
    if mean_ioi < 1e-12 {
        return 0.0;
    }
    let var = iois.iter().map(|&x| (x - mean_ioi).powi(2)).sum::<f64>() / iois.len() as f64;
    let cv = var.sqrt() / mean_ioi;

    // Score peaks at cv ≈ 0.5.
    let score = (-((cv - 0.5) / 0.3).powi(2)).exp();
    score.clamp(0.0, 1.0)
}

/// Timbral diversity: measure how much the spectral centroid (proxy: centre of
/// mass of absolute state values) changes over time.
fn timbral_diversity(trajectory: &[[f64; 16]]) -> f64 {
    let n = trajectory.len();
    if n < 16 {
        return 0.0;
    }

    // Use first 4 variables as "spectral bands".
    let centroids: Vec<f64> = trajectory
        .iter()
        .map(|s| {
            let total: f64 = s[..4].iter().map(|v| v.abs()).sum();
            if total < 1e-12 {
                return 0.0;
            }
            s[..4]
                .iter()
                .enumerate()
                .map(|(i, v)| i as f64 * v.abs())
                .sum::<f64>()
                / total
        })
        .collect();

    let mean = centroids.iter().sum::<f64>() / n as f64;
    let std =
        (centroids.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64).sqrt();

    // Normalise by the maximum possible centroid (index 3).
    (std / 1.5).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn lorenz_step(state: &mut [f64; 16], params: &[f64]) {
        let (sigma, rho, beta) = (params[0], params[1], params[2]);
        let dt = 0.005;
        let (x, y, z) = (state[0], state[1], state[2]);
        let dx = sigma * (y - x);
        let dy = x * (rho - z) - y;
        let dz = x * y - beta * z;
        state[0] += dx * dt;
        state[1] += dy * dt;
        state[2] += dz * dt;
    }

    fn lorenz_bounds() -> Vec<ParamBounds> {
        vec![
            ParamBounds::new(5.0, 15.0),
            ParamBounds::new(20.0, 35.0),
            ParamBounds::new(1.0, 4.0),
        ]
    }

    #[test]
    fn evolution_runs_without_panic() {
        let cfg = EvolutionConfig {
            population_size: 6,
            generations: 3,
            eval_steps: 100,
            ..Default::default()
        };
        let state: SharedEvolutionState = Arc::new(Mutex::new(EvolutionState::default()));
        let mut evo = ParameterEvolution::new(
            cfg,
            FitnessMetric::HarmonicRichness,
            lorenz_bounds(),
            Arc::clone(&state),
        );
        let saved = evo.run("test_preset", lorenz_step);
        assert!(!saved.params.is_empty());
        assert!(saved.fitness.is_finite());
    }

    #[test]
    fn fitness_history_length_matches_generations() {
        let generations = 4;
        let cfg = EvolutionConfig {
            population_size: 4,
            generations,
            eval_steps: 50,
            ..Default::default()
        };
        let state: SharedEvolutionState = Arc::new(Mutex::new(EvolutionState::default()));
        let mut evo = ParameterEvolution::new(
            FitnessMetric::RhythmicVariance,
            FitnessMetric::RhythmicVariance,
            lorenz_bounds(),
            Arc::clone(&state),
        );
        // Re-create properly.
        let state2: SharedEvolutionState = Arc::new(Mutex::new(EvolutionState::default()));
        let mut evo2 = ParameterEvolution::new(
            cfg,
            FitnessMetric::RhythmicVariance,
            lorenz_bounds(),
            Arc::clone(&state2),
        );
        evo2.run("h", lorenz_step);
        let hist = state2.lock().fitness_history.clone();
        assert_eq!(hist.len(), generations, "history length {}", hist.len());
        // suppress unused warning
        let _ = evo.config.population_size;
    }

    #[test]
    fn saved_evolution_toml_contains_name() {
        let se = SavedEvolution {
            name: "My Preset".into(),
            params: vec![10.0, 28.0, 2.666],
            fitness: 0.75,
            metric: "HarmonicRichness".into(),
            generation: 5,
        };
        let t = se.to_toml_snippet();
        assert!(t.contains("My Preset"), "toml: {t}");
        assert!(t.contains("0.750000"), "toml: {t}");
    }

    #[test]
    fn param_bounds_clamp() {
        let b = ParamBounds::new(1.0, 5.0);
        assert_eq!(b.clamp(0.0), 1.0);
        assert_eq!(b.clamp(10.0), 5.0);
        assert_eq!(b.clamp(3.0), 3.0);
    }

    #[test]
    fn harmonic_richness_sine_like_scores_high() {
        // Build a near-sine trajectory in the first variable.
        let n = 1024;
        let traj: Vec<[f64; 16]> = (0..n)
            .map(|i| {
                let mut s = [0.0f64; 16];
                s[0] = (i as f64 * std::f64::consts::TAU / 64.0).sin();
                s
            })
            .collect();
        let score = harmonic_richness(&traj);
        assert!(score > 0.3, "sine richness={score}");
    }

    #[test]
    fn timbral_diversity_constant_scores_zero() {
        let traj: Vec<[f64; 16]> = vec![[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; 200];
        let score = timbral_diversity(&traj);
        assert!(score < 0.05, "constant diversity={score}");
    }
}
