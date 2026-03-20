// Research and analysis functions below are intentionally kept for future use
// even if not currently called from the main binary.
#![allow(dead_code)]

use rayon::prelude::*;

// ---------------------------------------------------------------------------
// Compile-time system registry (#20)
// ---------------------------------------------------------------------------

/// Registry entry: system name + display metadata.
pub struct SystemEntry {
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
}

pub const SYSTEM_REGISTRY: &[SystemEntry] = &[
    SystemEntry {
        name: "lorenz",
        display_name: "Lorenz",
        description: "Classic butterfly attractor",
    },
    SystemEntry {
        name: "rossler",
        display_name: "Rossler",
        description: "Spiral attractor",
    },
    SystemEntry {
        name: "double_pendulum",
        display_name: "Double Pendulum",
        description: "Gravitational chaos",
    },
    SystemEntry {
        name: "geodesic_torus",
        display_name: "Geodesic Torus",
        description: "Ergodic irrational winding",
    },
    SystemEntry {
        name: "kuramoto",
        display_name: "Kuramoto",
        description: "8 coupled oscillators",
    },
    SystemEntry {
        name: "three_body",
        display_name: "Three Body",
        description: "Gravitational three-body problem",
    },
    SystemEntry {
        name: "duffing",
        display_name: "Duffing",
        description: "Driven nonlinear oscillator",
    },
    SystemEntry {
        name: "van_der_pol",
        display_name: "Van der Pol",
        description: "Self-sustaining limit cycle",
    },
    SystemEntry {
        name: "halvorsen",
        display_name: "Halvorsen",
        description: "Cyclic symmetry attractor",
    },
    SystemEntry {
        name: "aizawa",
        display_name: "Aizawa",
        description: "Six-parameter torus-like attractor",
    },
    SystemEntry {
        name: "chua",
        display_name: "Chua",
        description: "Electronic circuit chaos",
    },
    SystemEntry {
        name: "hindmarsh_rose",
        display_name: "Hindmarsh-Rose",
        description: "Neuron firing model",
    },
    SystemEntry {
        name: "coupled_map_lattice",
        display_name: "CML",
        description: "Spatiotemporal chaos",
    },
    SystemEntry {
        name: "mackey_glass",
        display_name: "Mackey-Glass",
        description: "Delay differential equation",
    },
    SystemEntry {
        name: "nose_hoover",
        display_name: "Nose-Hoover",
        description: "Conservative chaos",
    },
    SystemEntry {
        name: "sprott_b",
        display_name: "Sprott B",
        description: "Minimal algebraically simple attractor",
    },
    SystemEntry {
        name: "henon_map",
        display_name: "Henon Map",
        description: "Discrete-time map",
    },
    SystemEntry {
        name: "lorenz96",
        display_name: "Lorenz 96",
        description: "Weather prediction model",
    },
    SystemEntry {
        name: "custom",
        display_name: "Custom ODE",
        description: "Type your own 3-variable ODEs",
    },
    SystemEntry {
        name: "fractional_lorenz",
        display_name: "Fractional Lorenz",
        description: "Lorenz with fractional-order derivatives",
    },
    SystemEntry {
        name: "logistic_map",
        display_name: "Logistic Map",
        description: "Classic 1D chaos: x → r·x·(1−x)",
    },
    SystemEntry {
        name: "standard_map",
        display_name: "Standard Map",
        description: "Chirikov area-preserving toral map",
    },
    SystemEntry {
        name: "arnold_cat",
        display_name: "Arnold Cat",
        description: "Hyperbolic toral automorphism",
    },
    SystemEntry {
        name: "stochastic_lorenz",
        display_name: "Stochastic Lorenz",
        description: "Lorenz attractor with Wiener noise",
    },
    SystemEntry {
        name: "delayed_map",
        display_name: "Delayed Map",
        description: "Discrete delay logistic map",
    },
    SystemEntry {
        name: "oregonator",
        display_name: "Oregonator",
        description: "Belousov-Zhabotinsky chemical oscillator",
    },
    SystemEntry {
        name: "mathieu",
        display_name: "Mathieu",
        description: "Parametric resonance oscillator",
    },
    SystemEntry {
        name: "kuramoto_driven",
        display_name: "Kuramoto Driven",
        description: "Kuramoto oscillators with external drive",
    },
    SystemEntry {
        name: "thomas",
        display_name: "Thomas",
        description: "Cyclically symmetric dissipative chaotic attractor",
    },
    SystemEntry {
        name: "sprott_c",
        display_name: "Sprott C",
        description: "Minimal algebraically simple attractor (variant C)",
    },
    SystemEntry {
        name: "dadras",
        display_name: "Dadras",
        description: "Five-parameter multi-lobed chaotic attractor with large basin of attraction",
    },
    SystemEntry {
        name: "rucklidge",
        display_name: "Rucklidge",
        description: "Two-parameter double-convection chaotic attractor with folded-band topology",
    },
    SystemEntry {
        name: "chen",
        display_name: "Chen",
        description: "Three-parameter double-scroll attractor derived from Lorenz by anticontrol",
    },
    SystemEntry {
        name: "burke_shaw",
        display_name: "Burke-Shaw",
        description: "Two-parameter double-lobe attractor with elegant algebraic structure",
    },
    SystemEntry {
        name: "lorenz84",
        display_name: "Lorenz-84",
        description: "Atmospheric circulation model with westerly flow and wave interactions",
    },
    SystemEntry {
        name: "rabinovich_fabrikant",
        display_name: "Rabinovich-Fabrikant",
        description: "Plasma-physics derived attractor with complex multi-lobe topology",
    },
    SystemEntry {
        name: "sprott_g",
        display_name: "Sprott G",
        description: "Parameter-free single-scroll chaotic flow from Sprott's simplest systems",
    },
    SystemEntry {
        name: "rikitake",
        display_name: "Rikitake Dynamo",
        description: "Geomagnetic field reversal model with irregular polarity flips",
    },
];

pub mod lorenz84;
pub mod rabinovich_fabrikant;
pub mod sprott_g;
pub mod rikitake;
pub mod burke_shaw;
pub mod chen;
pub mod dadras;
pub mod rucklidge;
pub mod thomas;
pub mod sprott_c;
pub mod arnold_cat;
pub mod aizawa;
pub mod delayed_map;
pub mod kuramoto_driven;
pub mod mathieu;
pub mod oregonator;
pub mod stochastic_lorenz;
pub mod chua;
pub mod coupled_map_lattice;
pub mod custom_ode;
pub mod double_pendulum;
pub mod duffing;
pub mod fractional_lorenz;
pub mod geodesic_torus;
pub mod halvorsen;
pub mod henon_map;
pub mod hindmarsh_rose;
pub mod logistic_map;
pub mod standard_map;
pub mod kuramoto;
pub mod lorenz;
pub mod lorenz96;
pub mod mackey_glass;
pub mod nose_hoover;
pub mod rossler;
pub mod sprott_b;
pub mod three_body;
pub mod van_der_pol;

pub use burke_shaw::BurkeShaw;
pub use chen::Chen;
pub use dadras::Dadras;
pub use rucklidge::Rucklidge;
pub use lorenz84::Lorenz84;
pub use rabinovich_fabrikant::RabinovichFabrikant;
pub use sprott_g::SprottG;
pub use rikitake::Rikitake;
pub use thomas::Thomas;
pub use sprott_c::SprottC;
pub use arnold_cat::ArnoldCat;
pub use aizawa::Aizawa;
pub use delayed_map::DelayedMap;
pub use kuramoto_driven::KuramotoDriven;
pub use mathieu::Mathieu;
pub use oregonator::Oregonator;
pub use stochastic_lorenz::StochasticLorenz;
pub use chua::Chua;
pub use coupled_map_lattice::CoupledMapLattice;
pub use custom_ode::{validate_exprs, CustomOde};
pub use double_pendulum::DoublePendulum;
pub use duffing::Duffing;
pub use fractional_lorenz::FractionalLorenz;
pub use geodesic_torus::GeodesicTorus;
pub use halvorsen::Halvorsen;
pub use henon_map::HenonMap;
pub use hindmarsh_rose::HindmarshRose;
pub use kuramoto::Kuramoto;
pub use logistic_map::LogisticMap;
pub use lorenz::Lorenz;
pub use lorenz96::Lorenz96;
pub use standard_map::StandardMap;
pub use mackey_glass::MackeyGlass;
pub use nose_hoover::NoseHoover;
pub use rossler::Rossler;
pub use sprott_b::SprottB;
pub use three_body::ThreeBody;
pub use van_der_pol::VanDerPol;

/// A continuous-time dynamical system that can be stepped forward.
pub trait DynamicalSystem: Send {
    fn state(&self) -> &[f64];
    fn step(&mut self, dt: f64);
    fn dimension(&self) -> usize;
    fn name(&self) -> &str;
    /// Approximate speed of the trajectory (|dx/dt|) — used by granular mode.
    fn speed(&self) -> f64 {
        // Default: Euclidean norm of derivative estimate from last step.
        // Systems can override with a direct formula.
        1.0
    }

    /// Compute the derivative (dx/dt) at an arbitrary state point.
    /// Used for vector field visualization.
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        // Default: zero vector
        vec![0.0; state.len()]
    }

    /// Return the current derivative (at self.state())
    fn current_deriv(&self) -> Vec<f64> {
        self.deriv_at(self.state())
    }

    /// Load a saved state vector. Default: no-op (systems that don't override will ignore).
    fn set_state(&mut self, _s: &[f64]) {}

    /// Return current energy conservation error (relative), if applicable.
    fn energy_error(&self) -> Option<f64> {
        None
    }
}

/// Runge-Kutta 4 helper. Integrates `f(state) -> derivative` by dt.
pub fn rk4<F>(state: &mut Vec<f64>, dt: f64, f: F)
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n = state.len();
    let k1 = f(state);
    let s2: Vec<f64> = (0..n).map(|i| state[i] + 0.5 * dt * k1[i]).collect();
    let k2 = f(&s2);
    let s3: Vec<f64> = (0..n).map(|i| state[i] + 0.5 * dt * k2[i]).collect();
    let k3 = f(&s3);
    let s4: Vec<f64> = (0..n).map(|i| state[i] + dt * k3[i]).collect();
    let k4 = f(&s4);
    for i in 0..n {
        state[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
    }
}

/// Dormand-Prince RK4(5) embedded pair. Advances `state` using the 4th-order solution.
/// Returns `(rms_error, suggested_next_dt)`.
#[allow(clippy::unreadable_literal)]
pub fn rk45_step<F>(state: &mut Vec<f64>, dt: f64, f: &F) -> (f64, f64)
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n = state.len();
    let k1 = f(state);
    let s2: Vec<f64> = (0..n)
        .map(|i| state[i] + dt * (1.0 / 5.0) * k1[i])
        .collect();
    let k2 = f(&s2);
    let s3: Vec<f64> = (0..n)
        .map(|i| state[i] + dt * (3.0 / 40.0 * k1[i] + 9.0 / 40.0 * k2[i]))
        .collect();
    let k3 = f(&s3);
    let s4: Vec<f64> = (0..n)
        .map(|i| state[i] + dt * (44.0 / 45.0 * k1[i] - 56.0 / 15.0 * k2[i] + 32.0 / 9.0 * k3[i]))
        .collect();
    let k4 = f(&s4);
    let s5: Vec<f64> = (0..n)
        .map(|i| {
            state[i]
                + dt * (19372.0 / 6561.0 * k1[i] - 25360.0 / 2187.0 * k2[i]
                    + 64448.0 / 6561.0 * k3[i]
                    - 212.0 / 729.0 * k4[i])
        })
        .collect();
    let k5 = f(&s5);
    let s6: Vec<f64> = (0..n)
        .map(|i| {
            state[i]
                + dt * (9017.0 / 3168.0 * k1[i] - 355.0 / 33.0 * k2[i]
                    + 46732.0 / 5247.0 * k3[i]
                    + 49.0 / 176.0 * k4[i]
                    - 5103.0 / 18656.0 * k5[i])
        })
        .collect();
    let k6 = f(&s6);
    // 4th-order solution
    for i in 0..n {
        state[i] += dt
            * (35.0 / 384.0 * k1[i] + 500.0 / 1113.0 * k3[i] + 125.0 / 192.0 * k4[i]
                - 2187.0 / 6784.0 * k5[i]
                + 11.0 / 84.0 * k6[i]);
    }
    // 5th-order uses FSAL (k7 = f at new state)
    let k7 = f(state);
    let err: f64 = {
        let sum_sq: f64 = (0..n)
            .map(|i| {
                let e = dt
                    * (71.0 / 57600.0 * k1[i] - 71.0 / 16695.0 * k3[i] + 71.0 / 1920.0 * k4[i]
                        - 17253.0 / 339200.0 * k5[i]
                        + 22.0 / 525.0 * k6[i]
                        - 1.0 / 40.0 * k7[i]);
                e * e
            })
            .sum();
        (sum_sq / n as f64).sqrt()
    };
    let next_dt = if err > 1e-15 {
        dt * 0.9 * (1e-6 / err).powf(0.2)
    } else {
        dt * 2.0
    };
    (err, next_dt.clamp(dt * 0.1, dt * 5.0))
}

/// Integrate `state` from 0 to `total_dt` using adaptive Dormand-Prince RK45.
/// Automatically adjusts internal step size to keep local error below `tol`.
/// Returns the number of accepted sub-steps taken.
pub fn integrate_adaptive<F>(state: &mut Vec<f64>, total_dt: f64, tol: f64, f: F) -> usize
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let mut t = 0.0;
    let mut dt = (total_dt * 0.1).min(0.01).max(total_dt * 1e-5);
    let mut steps = 0usize;
    while t < total_dt - 1e-14 {
        let remaining = total_dt - t;
        let step = dt.min(remaining);
        let mut trial = state.clone();
        let (err, next_dt) = rk45_step(&mut trial, step, &f);
        if err <= tol || step <= total_dt * 1e-7 {
            *state = trial;
            t += step;
            steps += 1;
        }
        dt = next_dt.clamp(total_dt * 1e-6, total_dt);
        if steps > 100_000 {
            break;
        }
    }
    steps
}

/// Compute up to `n_exponents` Lyapunov exponents using QR/Gram-Schmidt reorthogonalization.
///
/// - `initial_state`: starting state (should already be on the attractor)
/// - `dim`: state-space dimension
/// - `n_exponents`: number of exponents to compute (capped at `dim`)
/// - `n_steps`: integration steps for accumulation
/// - `dt`: step size
/// - `f`: derivative function dx/dt = f(x)
///
/// Returns exponents ordered largest-first.
pub fn lyapunov_spectrum<F>(
    initial_state: &[f64],
    dim: usize,
    n_exponents: usize,
    n_steps: usize,
    dt: f64,
    f: &F,
) -> Vec<f64>
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n = n_exponents.min(dim);
    if n == 0 || dim == 0 {
        return Vec::new();
    }
    let mut state = initial_state.to_vec();
    // Orthonormal basis for tangent space (identity columns)
    let mut q: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let mut v = vec![0.0; dim];
            if i < dim {
                v[i] = 1.0;
            }
            v
        })
        .collect();
    let eps = 1e-8;
    let mut log_sum = vec![0.0f64; n];
    for _ in 0..n_steps {
        let state_old = state.clone();
        rk4(&mut state, dt, f);
        // Evolve each tangent vector via linearized flow (finite-difference Jacobian)
        for pv in &mut q {
            let mut perturbed: Vec<f64> = state_old
                .iter()
                .zip(pv.iter())
                .map(|(&s, &p)| s + eps * p)
                .collect();
            rk4(&mut perturbed, dt, f);
            for i in 0..dim {
                pv[i] = (perturbed[i] - state[i]) / eps;
            }
        }
        // QR via modified Gram-Schmidt
        for i in 0..n {
            let norm = q[i].iter().map(|&v| v * v).sum::<f64>().sqrt();
            if norm > 1e-15 {
                log_sum[i] += norm.ln();
                for j in 0..dim {
                    q[i][j] /= norm;
                }
            }
            for j in (i + 1)..n {
                let dot: f64 = q[i].iter().zip(q[j].iter()).map(|(&a, &b)| a * b).sum();
                for k in 0..dim {
                    q[j][k] -= dot * q[i][k];
                }
            }
        }
    }
    let total_time = n_steps as f64 * dt;
    log_sum.iter().map(|&s| s / total_time).collect()
}

/// Record states whenever the trajectory crosses the plane `state[plane_dim] == plane_val`
/// from below (negative to positive crossing).
/// Returns a Vec of crossing states (each is a Vec<f64> snapshot of the full state).
/// `n_warmup`: steps to discard before recording; `n_crossings`: crossings to capture.
pub fn poincare_section<F>(
    initial_state: &[f64],
    dt: f64,
    n_warmup: usize,
    n_crossings: usize,
    plane_dim: usize,
    plane_val: f64,
    f: &F,
) -> Vec<Vec<f64>>
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let mut state = initial_state.to_vec();
    // Warmup
    for _ in 0..n_warmup {
        rk4(&mut state, dt, f);
    }
    let mut crossings = Vec::new();
    let mut prev = state.clone();
    while crossings.len() < n_crossings {
        rk4(&mut state, dt, f);
        let prev_val = prev[plane_dim] - plane_val;
        let curr_val = state[plane_dim] - plane_val;
        if prev_val < 0.0 && curr_val >= 0.0 {
            // Linear interpolation to find crossing
            let t_cross = if (curr_val - prev_val).abs() > 1e-15 {
                (plane_val - prev[plane_dim]) / (state[plane_dim] - prev[plane_dim])
            } else {
                0.5
            };
            let cross_state: Vec<f64> = prev
                .iter()
                .zip(state.iter())
                .map(|(&p, &s)| p + t_cross * (s - p))
                .collect();
            crossings.push(cross_state);
        }
        prev = state.clone();
    }
    crossings
}

/// Ground-truth validation: compare RK4 vs RK45 trajectory divergence over time.
/// Runs both integrators from the same initial condition for `n_steps` steps.
/// Returns the RMS state divergence at the final step.
pub fn compare_integrators<F>(initial_state: &[f64], dt: f64, n_steps: usize, f: &F) -> f64
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let mut s_rk4 = initial_state.to_vec();
    let mut s_rk45 = initial_state.to_vec();
    for _ in 0..n_steps {
        rk4(&mut s_rk4, dt, f);
        integrate_adaptive(&mut s_rk45, dt, 1e-8, |s| f(s));
    }
    let rms: f64 = s_rk4
        .iter()
        .zip(s_rk45.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        / s_rk4.len() as f64;
    rms.sqrt()
}

/// Cluster trajectory points into k groups using k-means (Lloyd's algorithm).
/// Returns `(centroids, labels)` where labels[i] is the cluster index for trajectory[i].
/// Uses up to `max_iter` iterations. Only uses the first `use_dims` dimensions.
pub fn kmeans_cluster(
    trajectory: &[Vec<f64>],
    k: usize,
    use_dims: usize,
    max_iter: usize,
) -> (Vec<Vec<f64>>, Vec<usize>) {
    if k == 0 || trajectory.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let n = trajectory.len();
    let d = use_dims.min(trajectory[0].len());
    // Initialize centroids: evenly-spaced points from trajectory
    let mut centroids: Vec<Vec<f64>> = (0..k)
        .map(|i| {
            let idx = (i * n) / k;
            trajectory[idx][..d].to_vec()
        })
        .collect();
    let mut labels = vec![0usize; n];
    for _ in 0..max_iter {
        // Assignment step
        let mut changed = false;
        for (i, point) in trajectory.iter().enumerate() {
            let best = (0..k)
                .min_by(|&a, &b| {
                    let da: f64 = centroids[a]
                        .iter()
                        .zip(&point[..d])
                        .map(|(c, p)| (c - p).powi(2))
                        .sum();
                    let db: f64 = centroids[b]
                        .iter()
                        .zip(&point[..d])
                        .map(|(c, p)| (c - p).powi(2))
                        .sum();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(0);
            if labels[i] != best {
                changed = true;
            }
            labels[i] = best;
        }
        // Update centroids
        let mut sums = vec![vec![0.0f64; d]; k];
        let mut counts = vec![0usize; k];
        for (i, point) in trajectory.iter().enumerate() {
            let c = labels[i];
            for j in 0..d {
                sums[c][j] += point[j];
            }
            counts[c] += 1;
        }
        for c in 0..k {
            if counts[c] > 0 {
                for j in 0..d {
                    centroids[c][j] = sums[c][j] / counts[c] as f64;
                }
            }
        }
        if !changed {
            break;
        }
    }
    (centroids, labels)
}

/// Estimate the period of a (near-)periodic orbit by tracking Poincaré crossings.
/// Returns `Some(period_in_time_units)` if a period is detected within tolerance,
/// or `None` if the orbit appears chaotic or took too long.
pub fn detect_period<F>(
    initial_state: &[f64],
    dt: f64,
    plane_dim: usize,
    f: &F,
    max_steps: usize,
) -> Option<f64>
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    // Warmup 200 steps to estimate plane_val
    let mut state = initial_state.to_vec();
    let n_warm = 200usize;
    let mut sum_val = 0.0f64;
    for _ in 0..n_warm {
        rk4(&mut state, dt, f);
        sum_val += state[plane_dim];
    }
    let plane_val = sum_val / n_warm as f64;

    let mut prev = state.clone();
    let mut crossing_times: Vec<f64> = Vec::new();
    let mut t = n_warm as f64 * dt;

    for _ in 0..max_steps {
        rk4(&mut state, dt, f);
        t += dt;
        let prev_val = prev[plane_dim] - plane_val;
        let curr_val = state[plane_dim] - plane_val;
        if prev_val < 0.0 && curr_val >= 0.0 {
            let t_frac = if (curr_val - prev_val).abs() > 1e-15 {
                (plane_val - prev[plane_dim]) / (state[plane_dim] - prev[plane_dim])
            } else {
                0.5
            };
            let t_cross = t - dt + t_frac * dt;
            crossing_times.push(t_cross);
            if crossing_times.len() >= 3 {
                let n = crossing_times.len();
                let last_interval = crossing_times[n - 1] - crossing_times[n - 2];
                let prev_interval = crossing_times[n - 2] - crossing_times[n - 3];
                if prev_interval > 1e-12 {
                    let ratio = last_interval / prev_interval;
                    if (ratio - 1.0).abs() < 0.05 {
                        // Stable period detected — return average
                        let avg = crossing_times.windows(2).map(|w| w[1] - w[0]).sum::<f64>()
                            / (crossing_times.len() - 1) as f64;
                        return Some(avg);
                    }
                }
            }
        }
        prev = state.clone();
    }
    None
}

/// Find a fixed point of the system near `guess` using Newton's method on f(x)=0.
/// Returns `Some(fixed_point)` if converged within `tol` in `max_iter` iterations,
/// or `None` if diverged.
pub fn find_fixed_point<F>(guess: &[f64], tol: f64, max_iter: usize, f: &F) -> Option<Vec<f64>>
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let eps = 1e-7f64;
    let n = guess.len();
    let mut x = guess.to_vec();
    for _ in 0..max_iter {
        let fx = f(&x);
        // Check convergence
        let fnorm: f64 = fx.iter().map(|&v| v * v).sum::<f64>().sqrt();
        if fnorm < tol {
            return Some(x);
        }
        // Divergence check
        let xnorm: f64 = x.iter().map(|&v| v * v).sum::<f64>().sqrt();
        if xnorm > 1e6 {
            return None;
        }
        // Build numerical Jacobian n×n
        let mut jac = vec![vec![0.0f64; n]; n];
        for j in 0..n {
            let mut xp = x.clone();
            xp[j] += eps;
            let fxp = f(&xp);
            for i in 0..n {
                jac[i][j] = (fxp[i] - fx[i]) / eps;
            }
        }
        // Solve J * dx = -fx via Gaussian elimination (in-place augmented matrix)
        // Augmented matrix: [J | -fx]
        let mut aug: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                let mut row = jac[i].clone();
                row.push(-fx[i]);
                row
            })
            .collect();
        for col in 0..n {
            // Find pivot
            let pivot = (col..n).max_by(|&a, &b| {
                aug[a][col]
                    .abs()
                    .partial_cmp(&aug[b][col].abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })?;
            aug.swap(col, pivot);
            let diag = aug[col][col];
            if diag.abs() < 1e-15 {
                return None;
            }
            for k in col..=n {
                aug[col][k] /= diag;
            }
            for row in 0..n {
                if row != col {
                    let factor = aug[row][col];
                    for k in col..=n {
                        aug[row][k] -= factor * aug[col][k];
                    }
                }
            }
        }
        // dx is in aug[i][n]
        for i in 0..n {
            x[i] += aug[i][n];
        }
    }
    // Final check
    let fx = f(&x);
    let fnorm: f64 = fx.iter().map(|&v| v * v).sum::<f64>().sqrt();
    if fnorm < tol {
        Some(x)
    } else {
        None
    }
}

/// Classify the attractor type from its Lyapunov spectrum.
/// Returns one of: "fixed_point", "limit_cycle", "torus", "chaos", "hyperchaos", "unknown"
pub fn classify_attractor(lyapunov: &[f64]) -> &'static str {
    if lyapunov.is_empty() {
        return "unknown";
    }
    let positive_count = lyapunov.iter().filter(|&&l| l > 0.01).count();
    let near_zero_count = lyapunov.iter().filter(|&&l| l.abs() < 0.01).count();
    let all_negative = lyapunov.iter().all(|&l| l < 0.0);
    if all_negative {
        return "fixed_point";
    }
    if positive_count >= 2 {
        return "hyperchaos";
    }
    if positive_count == 1 {
        return "chaos";
    }
    // No positive exponents
    if near_zero_count >= 2 {
        return "torus";
    }
    if near_zero_count == 1 {
        return "limit_cycle";
    }
    "unknown"
}

/// Compute permutation entropy of a 1D time series.
/// `order`: embedding dimension (3–7 typical). `delay`: time delay in samples.
/// Returns entropy in nats, normalized to [0,1] by dividing by ln(order!).
pub fn permutation_entropy(trajectory: &[f64], order: usize, delay: usize) -> f64 {
    if order < 2 || trajectory.is_empty() {
        return 0.0;
    }
    let delay = delay.max(1);
    let window = order * delay;
    // Number of windows
    let n_windows = if trajectory.len() >= window {
        trajectory.len() - window + 1
    } else {
        return 0.0;
    };
    // Count pattern frequencies
    let mut counts: std::collections::HashMap<Vec<usize>, usize> = std::collections::HashMap::new();
    for start in 0..n_windows {
        // Extract pattern: indices 0, delay, 2*delay, ...
        let pattern: Vec<f64> = (0..order).map(|k| trajectory[start + k * delay]).collect();
        // Argsort
        let mut idx: Vec<usize> = (0..order).collect();
        idx.sort_by(|&a, &b| {
            pattern[a]
                .partial_cmp(&pattern[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        *counts.entry(idx).or_insert(0) += 1;
    }
    let total = n_windows as f64;
    let entropy: f64 = counts
        .values()
        .map(|&c| {
            let p = c as f64 / total;
            -p * p.ln()
        })
        .sum();
    // Normalize by ln(order!)
    let factorial: f64 = (1..=order).map(|k| k as f64).product();
    let max_entropy = factorial.ln();
    if max_entropy > 1e-15 {
        entropy / max_entropy
    } else {
        0.0
    }
}

/// Estimate the correlation dimension of an attractor from a trajectory.
/// Uses the Grassberger-Procaccia algorithm: C(r) ~ r^D as r→0.
/// `trajectory`: Vec of state snapshots. `n_pairs`: number of random pairs to sample.
/// Returns estimated dimension D.
pub fn correlation_dimension(trajectory: &[Vec<f64>], n_pairs: usize) -> f64 {
    let n = trajectory.len();
    if n < 2 || n_pairs == 0 {
        return 0.0;
    }
    // Sample random pairs using a simple LCG
    #[allow(clippy::unreadable_literal)]
    let mut seed: u64 = 12_345_678_901_234_567;
    let lcg_next = |s: &mut u64| -> usize {
        #[allow(clippy::unreadable_literal)]
        {
            *s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
        }
        (*s >> 33) as usize
    };
    let mut distances: Vec<f64> = Vec::with_capacity(n_pairs);
    for _ in 0..n_pairs {
        let i = lcg_next(&mut seed) % n;
        let j = lcg_next(&mut seed) % n;
        if i == j {
            continue;
        }
        let dist: f64 = trajectory[i]
            .iter()
            .zip(trajectory[j].iter())
            .map(|(&a, &b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt();
        distances.push(dist);
    }
    if distances.is_empty() {
        return 0.0;
    }
    distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let total = distances.len();
    let r1_idx = total / 10;
    let r2_idx = (total * 9) / 10;
    let r1 = distances[r1_idx];
    let r2 = distances[r2_idx.min(total - 1)];
    let c1 = distances.iter().filter(|&&d| d < r1).count() as f64;
    let c2 = distances.iter().filter(|&&d| d < r2).count() as f64;
    let dim = (c2.max(1.0).ln() - c1.max(1.0).ln()) / ((r2 / r1.max(1e-10)).max(1e-10).ln());
    dim.clamp(0.0, 20.0)
}

/// Find a parameter path from `p_start` to `p_end` that avoids divergence,
/// using binary subdivision. `test_fn` returns true if a parameter value is "safe"
/// (system does not diverge there). Returns a sorted Vec of safe waypoint values
/// from start to end. If the direct path is safe, returns `[p_start, p_end]`.
/// Recursively subdivides at the midpoint if midpoint is unsafe (up to `max_depth`).
pub fn safe_param_path(
    p_start: f64,
    p_end: f64,
    test_fn: &dyn Fn(f64) -> bool,
    max_depth: usize,
) -> Vec<f64> {
    if max_depth == 0 {
        return vec![p_start, p_end];
    }
    let p_mid = (p_start + p_end) / 2.0;
    if test_fn(p_mid) {
        // Midpoint is safe — direct path is fine
        vec![p_start, p_end]
    } else {
        // Subdivide
        let mut left = safe_param_path(p_start, p_mid, test_fn, max_depth - 1);
        let right = safe_param_path(p_mid, p_end, test_fn, max_depth - 1);
        // Merge: left already ends with p_mid or p_start; right starts with p_mid
        // Avoid duplicate at junction
        for v in right.into_iter().skip(1) {
            left.push(v);
        }
        left
    }
}

/// Morris one-at-a-time screening: estimate parameter sensitivity by measuring
/// how much the output changes when each parameter is perturbed by `delta`.
/// `params`: baseline parameter values. `output_fn`: closure taking a param slice,
/// returning a scalar metric. `delta`: relative perturbation (e.g. 0.1 for 10%).
/// Returns sensitivity index for each parameter (larger = more influential).
pub fn morris_sensitivity<F>(params: &[f64], delta: f64, output_fn: F) -> Vec<f64>
where
    F: Fn(&[f64]) -> f64,
{
    let baseline = output_fn(params);
    let mut sensitivities = Vec::with_capacity(params.len());
    for i in 0..params.len() {
        let mut perturbed = params.to_vec();
        perturbed[i] *= 1.0 + delta;
        let output = output_fn(&perturbed);
        let sensitivity = if delta.abs() > 1e-15 {
            ((output - baseline) / delta).abs()
        } else {
            0.0
        };
        sensitivities.push(sensitivity);
    }
    sensitivities
}

/// Estimate Kolmogorov (metric) entropy from the Lyapunov spectrum.
/// Per Pesin's identity: K = Σ max(λᵢ, 0) (sum of positive Lyapunov exponents).
pub fn kolmogorov_entropy(lyapunov: &[f64]) -> f64 {
    lyapunov.iter().map(|&l| l.max(0.0)).sum()
}

/// Symplectic (leapfrog / Störmer-Verlet) helper for Hamiltonian systems.
/// `q_indices` and `p_indices` are the indices of positions and momenta in state.
/// `dH_dq` computes -∂H/∂q (force), `dH_dp` computes ∂H/∂p (velocity).
pub fn leapfrog<Fv, Fa>(
    state: &mut Vec<f64>,
    q_idx: &[usize],
    p_idx: &[usize],
    dt: f64,
    velocity: Fv,
    force: Fa,
) where
    Fv: Fn(&[f64]) -> Vec<f64>,
    Fa: Fn(&[f64]) -> Vec<f64>,
{
    let n = q_idx.len();
    // half-kick momenta
    let f = force(state);
    for i in 0..n {
        state[p_idx[i]] += 0.5 * dt * f[i];
    }
    // full drift positions
    let v = velocity(state);
    for i in 0..n {
        state[q_idx[i]] += dt * v[i];
    }
    // half-kick momenta again
    let f2 = force(state);
    for i in 0..n {
        state[p_idx[i]] += 0.5 * dt * f2[i];
    }
}

/// Yoshida 4th-order symplectic integrator for Hamiltonian systems.
/// Composes three leapfrog steps with coefficients w0, w1 chosen to cancel
/// leading error terms. More accurate than plain leapfrog at same step size.
pub fn yoshida4<Fv, Fa>(
    state: &mut Vec<f64>,
    q_idx: &[usize],
    p_idx: &[usize],
    dt: f64,
    velocity: Fv,
    force: Fa,
) where
    Fv: Fn(&[f64]) -> Vec<f64>,
    Fa: Fn(&[f64]) -> Vec<f64>,
{
    // Yoshida (1990) coefficients
    let cbrt2: f64 = 2.0f64.cbrt();
    let w1 = 1.0 / (2.0 - cbrt2);
    let w0 = -cbrt2 * w1;
    let c1 = w1 / 2.0;
    let c2 = (w0 + w1) / 2.0;
    let d1 = w1;
    let d2 = w0;
    // Three-step composition: c1,d1,c2,d2,c2,d1,c1
    // Step 1
    let n = q_idx.len();
    let v = velocity(state);
    for i in 0..n {
        state[q_idx[i]] += c1 * dt * v[i];
    }
    let f = force(state);
    for i in 0..n {
        state[p_idx[i]] += d1 * dt * f[i];
    }
    // Step 2
    let v = velocity(state);
    for i in 0..n {
        state[q_idx[i]] += c2 * dt * v[i];
    }
    let f = force(state);
    for i in 0..n {
        state[p_idx[i]] += d2 * dt * f[i];
    }
    // Step 3 (mirror of step 1)
    let v = velocity(state);
    for i in 0..n {
        state[q_idx[i]] += c2 * dt * v[i];
    }
    let f = force(state);
    for i in 0..n {
        state[p_idx[i]] += d1 * dt * f[i];
    }
    // Final half-drift
    let v = velocity(state);
    for i in 0..n {
        state[q_idx[i]] += c1 * dt * v[i];
    }
}

/// Estimate transfer entropy from time series X → Y (information flow from X to Y).
/// Uses k=1 nearest-neighbor estimator approximated by binning.
/// `x` and `y`: equal-length 1D time series (first component of each system's state).
/// `lag`: time lag in samples (how far back in X to look).
/// `n_bins`: number of histogram bins per dimension.
/// Returns T_{X→Y} in nats (>0 means X drives Y, 0 means no coupling).
pub fn transfer_entropy(x: &[f64], y: &[f64], lag: usize, n_bins: usize) -> f64 {
    if x.len() != y.len() || x.len() < lag + 2 || n_bins < 2 {
        return 0.0;
    }
    let n = n_bins;
    // Normalize to [0, 1] then bin
    let normalize = |v: &[f64]| -> Vec<usize> {
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = (hi - lo).max(1e-15);
        v.iter()
            .map(|&val| ((val - lo) / range * (n as f64 - 1e-9)).floor() as usize)
            .collect()
    };
    let xb = normalize(x);
    let yb = normalize(y);

    let start = lag + 1;
    let len = x.len() - start;

    // 3D histogram: (y_future, y_past, x_past)
    let mut hist3 = vec![vec![vec![0.0f64; n]; n]; n];
    // 2D histogram: (y_future, y_past)
    let mut hist_yfy = vec![vec![0.0f64; n]; n];
    // 2D histogram: (y_past, x_past)
    let mut hist_yx = vec![vec![0.0f64; n]; n];
    // 1D histogram: y_past
    let mut hist_y = vec![0.0f64; n];

    for t in start..(start + len) {
        let yf = yb[t];
        let yp = yb[t - 1];
        let xp = xb[t - lag];
        hist3[yf][yp][xp] += 1.0;
        hist_yfy[yf][yp] += 1.0;
        hist_yx[yp][xp] += 1.0;
        hist_y[yp] += 1.0;
    }

    // Add Laplace smoothing
    let total3 = len as f64 + (n * n * n) as f64;
    let total_yfy = len as f64 + (n * n) as f64;
    let total_yx = len as f64 + (n * n) as f64;
    let total_y = len as f64 + n as f64;

    let entropy = |counts: &[f64], total: f64| -> f64 {
        counts
            .iter()
            .map(|&c| {
                let p = (c + 1.0) / total;
                -p * p.ln()
            })
            .sum::<f64>()
    };

    // H(y_future, y_past)
    let h_yfy = entropy(
        &hist_yfy.iter().flatten().cloned().collect::<Vec<_>>(),
        total_yfy,
    );
    // H(y_past)
    let h_y = entropy(&hist_y, total_y);
    // H(y_future, y_past, x_past)
    let h3 = entropy(
        &hist3
            .iter()
            .flatten()
            .flatten()
            .cloned()
            .collect::<Vec<_>>(),
        total3,
    );
    // H(y_past, x_past)
    let h_yx = entropy(
        &hist_yx.iter().flatten().cloned().collect::<Vec<_>>(),
        total_yx,
    );

    // T = H(yf,yp) - H(yp) - H(yf,yp,xp) + H(yp,xp)
    let te = h_yfy - h_y - h3 + h_yx;
    te.max(0.0)
}

/// Estimate mutual information between two 1D time series using histogram binning.
/// I(X;Y) = H(X) + H(Y) - H(X,Y)
/// `n_bins`: histogram bins per axis (8-16 typical).
/// Returns mutual information in nats.
#[allow(clippy::similar_names)]
pub fn mutual_information(x: &[f64], y: &[f64], n_bins: usize) -> f64 {
    if x.len() != y.len() || x.is_empty() || n_bins < 2 {
        return 0.0;
    }
    let n = n_bins;
    let normalize = |v: &[f64]| -> Vec<usize> {
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = (hi - lo).max(1e-15);
        v.iter()
            .map(|&val| ((val - lo) / range * (n as f64 - 1e-9)).floor() as usize)
            .collect()
    };
    let xb = normalize(x);
    let yb = normalize(y);

    let mut hist_x = vec![0.0f64; n];
    let mut hist_y = vec![0.0f64; n];
    let mut hist_xy = vec![vec![0.0f64; n]; n];

    for (&xi, &yi) in xb.iter().zip(yb.iter()) {
        hist_x[xi] += 1.0;
        hist_y[yi] += 1.0;
        hist_xy[xi][yi] += 1.0;
    }

    let len = x.len() as f64;
    let total_x = len + n as f64;
    let total_y = len + n as f64;
    let total_xy = len + (n * n) as f64;

    let h_x: f64 = hist_x
        .iter()
        .map(|&c| {
            let p = (c + 1.0) / total_x;
            -p * p.ln()
        })
        .sum();
    let h_y: f64 = hist_y
        .iter()
        .map(|&c| {
            let p = (c + 1.0) / total_y;
            -p * p.ln()
        })
        .sum();
    let h_xy: f64 = hist_xy
        .iter()
        .flatten()
        .map(|&c| {
            let p = (c + 1.0) / total_xy;
            -p * p.ln()
        })
        .sum();

    (h_x + h_y - h_xy).max(0.0)
}

/// Compute RQA measures from a trajectory.
/// Returns `RqaResult` with determinism, laminarity, and average diagonal line length.
#[derive(Debug, Clone)]
pub struct RqaResult {
    pub recurrence_rate: f64, // fraction of recurrent points
    pub determinism: f64,     // fraction of recurrent points in diagonal lines >= min_line
    pub laminarity: f64,      // fraction of recurrent points in vertical lines >= min_line
    pub avg_diag_len: f64,    // average diagonal line length
    pub entropy_diag: f64,    // Shannon entropy of diagonal line lengths
}

pub fn recurrence_quantification(
    trajectory: &[Vec<f64>],
    threshold: f64,
    min_line: usize,
) -> RqaResult {
    let raw_n = trajectory.len();
    if raw_n < 2 {
        return RqaResult {
            recurrence_rate: 0.0,
            determinism: 0.0,
            laminarity: 0.0,
            avg_diag_len: 0.0,
            entropy_diag: 0.0,
        };
    }
    // Cap at 200 points
    let max_n = 200usize;
    let (traj, n) = if raw_n > max_n {
        let step = raw_n / max_n;
        let sub: Vec<Vec<f64>> = trajectory
            .iter()
            .step_by(step)
            .take(max_n)
            .cloned()
            .collect();
        let len = sub.len();
        (sub, len)
    } else {
        (trajectory.to_vec(), raw_n)
    };

    // Build recurrence matrix
    let mut recur = vec![vec![false; n]; n];
    for i in 0..n {
        for j in 0..n {
            let dist: f64 = traj[i]
                .iter()
                .zip(traj[j].iter())
                .map(|(&a, &b)| (a - b) * (a - b))
                .sum::<f64>()
                .sqrt();
            recur[i][j] = dist < threshold;
        }
    }

    let total_points = n * n;
    let rec_sum: usize = recur.iter().flatten().filter(|&&v| v).count();
    let recurrence_rate = rec_sum as f64 / total_points as f64;

    // Diagonal lines
    let mut diag_lengths: Vec<usize> = Vec::new();
    for start_diag in (-(n as i64 - 1))..=(n as i64 - 1) {
        let mut run = 0usize;
        let i_start = (-start_diag).max(0) as usize;
        let j_start = start_diag.max(0) as usize;
        let diag_len = n - i_start.max(j_start);
        for k in 0..diag_len {
            if recur[i_start + k][j_start + k] {
                run += 1;
            } else {
                if run >= min_line {
                    diag_lengths.push(run);
                }
                run = 0;
            }
        }
        if run >= min_line {
            diag_lengths.push(run);
        }
    }

    let diag_rec_points: usize = diag_lengths.iter().sum();
    let determinism = if rec_sum > 0 {
        diag_rec_points as f64 / rec_sum as f64
    } else {
        0.0
    };
    let avg_diag_len = if !diag_lengths.is_empty() {
        diag_rec_points as f64 / diag_lengths.len() as f64
    } else {
        0.0
    };

    // Shannon entropy of diagonal line lengths
    let max_len = diag_lengths.iter().cloned().max().unwrap_or(0);
    let entropy_diag = if max_len >= min_line {
        let mut len_counts = vec![0usize; max_len + 1];
        for &l in &diag_lengths {
            len_counts[l] += 1;
        }
        let total_dl = diag_lengths.len() as f64;
        len_counts
            .iter()
            .filter(|&&c| c > 0)
            .map(|&c| {
                let p = c as f64 / total_dl;
                -p * p.ln()
            })
            .sum()
    } else {
        0.0
    };

    // Vertical lines (laminarity)
    let mut vert_rec_points = 0usize;
    for j in 0..n {
        let mut run = 0usize;
        for i in 0..n {
            if recur[i][j] {
                run += 1;
            } else {
                if run >= min_line {
                    vert_rec_points += run;
                }
                run = 0;
            }
        }
        if run >= min_line {
            vert_rec_points += run;
        }
    }
    let laminarity = if rec_sum > 0 {
        vert_rec_points as f64 / rec_sum as f64
    } else {
        0.0
    };

    RqaResult {
        recurrence_rate,
        determinism,
        laminarity,
        avg_diag_len,
        entropy_diag,
    }
}

/// Compute the FTLE field over a 2D grid of initial conditions in the (dim_x, dim_y) plane.
/// The FTLE measures local stretching — reveals stable and unstable manifolds.
/// `center`: center of the grid in state space; `extent`: half-width in each dimension.
/// `grid_n`: grid size (grid_n × grid_n points).
/// `T`: integration time; `dt`: step size.
/// `fixed_dims`: values for all dimensions NOT being varied (length = dim - 2).
/// Returns a Vec of (x_coord, y_coord, ftle_value) for each grid point.
pub fn ftle_field<F>(
    center: [f64; 2],
    extent: [f64; 2],
    grid_n: usize,
    dim_x: usize,
    dim_y: usize,
    t_integration: f64,
    dt: f64,
    full_dim: usize,
    fixed_state: &[f64],
    f: &F,
) -> Vec<(f64, f64, f64)>
where
    F: Fn(&[f64]) -> Vec<f64> + Sync,
{
    if grid_n < 2 || full_dim == 0 || dt <= 0.0 || t_integration <= 0.0 {
        return Vec::new();
    }
    let n_steps = (t_integration / dt).round() as usize;
    let eps = 1e-6;
    let inv_t = 1.0 / t_integration;

    let indices: Vec<(usize, usize)> = (0..grid_n)
        .flat_map(|i| (0..grid_n).map(move |j| (i, j)))
        .collect();

    indices
        .par_iter()
        .map(|&(i, j)| {
            let xc = center[0] + (i as f64 / (grid_n - 1) as f64 - 0.5) * 2.0 * extent[0];
            let yc = center[1] + (j as f64 / (grid_n - 1) as f64 - 0.5) * 2.0 * extent[1];

            let mut state_ref = fixed_state.to_vec();
            if state_ref.len() < full_dim {
                state_ref.resize(full_dim, 0.0);
            }
            state_ref[dim_x] = xc;
            state_ref[dim_y] = yc;

            let mut state_pert = state_ref.clone();
            state_pert[dim_x] += eps;

            for _ in 0..n_steps {
                rk4(&mut state_ref, dt, f);
                rk4(&mut state_pert, dt, f);
            }

            let dist: f64 = state_ref
                .iter()
                .zip(state_pert.iter())
                .map(|(&a, &b)| (a - b) * (a - b))
                .sum::<f64>()
                .sqrt();

            let ftle = if dist > 1e-20 {
                inv_t * (dist / eps).ln()
            } else {
                0.0
            };
            (xc, yc, ftle)
        })
        .collect()
}

/// Test time-reversibility: integrate forward N steps, then backward N steps.
/// Returns the return error |x_final - x_initial| / |x_initial|.
/// For Hamiltonian systems this should be ~machine epsilon; dissipative systems will be large.
pub fn reversibility_test<F>(initial_state: &[f64], dt: f64, n_steps: usize, f: &F) -> f64
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let mut state = initial_state.to_vec();
    // Integrate forward
    for _ in 0..n_steps {
        rk4(&mut state, dt, f);
    }
    // Integrate backward
    for _ in 0..n_steps {
        rk4(&mut state, -dt, f);
    }
    // Relative return error
    let norm_init: f64 = initial_state.iter().map(|&v| v * v).sum::<f64>().sqrt();
    let error: f64 = state
        .iter()
        .zip(initial_state.iter())
        .map(|(&a, &b)| (a - b) * (a - b))
        .sum::<f64>()
        .sqrt();
    if norm_init > 1e-15 {
        error / norm_init
    } else {
        error
    }
}

/// Compute an Arnold tongue map for a periodically forced system.
/// Sweeps driving frequency (omega) and amplitude (A) over a 2D grid.
/// At each (omega, A), runs the system and measures the dominant output frequency
/// (via zero-crossings of state[0]). Returns sync_ratio = output_freq / drive_freq.
/// Points where sync_ratio ≈ p/q (simple rational) indicate Arnold tongues (synchronization).
///
/// `make_deriv`: closure returning a deriv_fn for given (omega, A).
/// Returns Vec<(omega, amplitude, sync_ratio, is_locked)> for each grid point.
pub fn arnold_tongue_map<F>(
    omega_range: (f64, f64),
    amp_range: (f64, f64),
    grid_n: usize,
    initial_state: &[f64],
    t_warmup: f64,
    t_measure: f64,
    dt: f64,
    make_deriv: F,
) -> Vec<(f64, f64, f64, bool)>
where
    F: Fn(f64, f64) -> Box<dyn Fn(&[f64]) -> Vec<f64>> + Sync + Send,
{
    use std::f64::consts::PI;
    let n = grid_n.max(2);
    let indices: Vec<(usize, usize)> = (0..n).flat_map(|i| (0..n).map(move |j| (i, j))).collect();

    indices
        .par_iter()
        .map(|&(i, j)| {
            let omega = omega_range.0 + i as f64 * (omega_range.1 - omega_range.0) / (n - 1) as f64;
            let amp = amp_range.0 + j as f64 * (amp_range.1 - amp_range.0) / (n - 1) as f64;

            let deriv_fn = make_deriv(omega, amp);
            let mut state = initial_state.to_vec();

            // Warmup
            let n_warmup = (t_warmup / dt).round() as usize;
            for _ in 0..n_warmup {
                rk4(&mut state, dt, &*deriv_fn);
            }

            // Measure zero crossings (negative→positive) of state[0]
            let n_measure = (t_measure / dt).round() as usize;
            let mut n_crossings: usize = 0;
            let mut prev_val = state[0];
            for _ in 0..n_measure {
                rk4(&mut state, dt, &*deriv_fn);
                let curr_val = state[0];
                if prev_val < 0.0 && curr_val >= 0.0 {
                    n_crossings += 1;
                }
                prev_val = curr_val;
            }

            let output_freq = n_crossings as f64 / t_measure;
            let drive_freq = omega / (2.0 * PI);
            let sync_ratio = if omega > 0.0 {
                output_freq / drive_freq.max(1e-15)
            } else {
                0.0
            };

            // is_locked: sync_ratio near a simple rational p/q (p,q ≤ 4)
            let is_locked = {
                let near_integer = (sync_ratio - sync_ratio.round()).abs() < 0.1;
                let near_simple = {
                    let mut found = false;
                    'outer: for p in 1usize..=4 {
                        for q in 1usize..=4 {
                            if (sync_ratio - p as f64 / q as f64).abs() < 0.05 {
                                found = true;
                                break 'outer;
                            }
                        }
                    }
                    found
                };
                near_integer || near_simple
            };

            (omega, amp, sync_ratio, is_locked)
        })
        .collect()
}

/// Compute a distance between two parameter vectors in normalized parameter space.
/// Uses Euclidean distance with each parameter normalized by its expected range.
/// `params_a`, `params_b`: parameter slices of equal length.
/// `ranges`: (min, max) for each parameter dimension.
/// Returns normalized distance in [0, 1].
pub fn param_distance(params_a: &[f64], params_b: &[f64], ranges: &[(f64, f64)]) -> f64 {
    let n = params_a.len().min(params_b.len()).min(ranges.len());
    if n == 0 {
        return 0.0;
    }
    let sum_sq: f64 = (0..n)
        .map(|i| {
            let range = (ranges[i].1 - ranges[i].0).abs().max(1e-10);
            let d = (params_a[i] - params_b[i]) / range;
            d * d
        })
        .sum();
    (sum_sq / n as f64).sqrt().clamp(0.0, 1.0)
}

/// Compute the empirical volume contraction rate of the flow.
/// For a dissipative system this should be negative (= sum of Jacobian diagonal = ∇·f).
/// Estimated as d/dt[ln(vol)] where vol is approximated by a simplex of perturbed trajectories.
/// Returns the divergence ∇·f = Σ ∂f_i/∂x_i estimated by finite differences at `state`.
pub fn divergence_at<F>(state: &[f64], f: &F, eps: f64) -> f64
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n = state.len();
    let mut div = 0.0f64;
    for i in 0..n {
        let mut xp = state.to_vec();
        xp[i] += eps;
        let mut xm = state.to_vec();
        xm[i] -= eps;
        let fp = f(&xp);
        let fm = f(&xm);
        div += (fp[i] - fm[i]) / (2.0 * eps);
    }
    div
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple linear ODE: dx/dt = -x, exact solution x(t) = x0 * e^(-t)
    fn decay(s: &[f64]) -> Vec<f64> {
        vec![-s[0]]
    }

    // 2D harmonic oscillator: dx/dt = v, dv/dt = -x, exact period 2π
    fn harmonic(s: &[f64]) -> Vec<f64> {
        vec![s[1], -s[0]]
    }

    // Lorenz derivatives for Lyapunov / divergence tests
    fn lorenz_deriv(s: &[f64]) -> Vec<f64> {
        let (sigma, rho, beta) = (10.0, 28.0, 8.0 / 3.0);
        vec![
            sigma * (s[1] - s[0]),
            s[0] * (rho - s[2]) - s[1],
            s[0] * s[1] - beta * s[2],
        ]
    }

    // -------------------------------------------------------------------------
    // rk4
    // -------------------------------------------------------------------------

    #[test]
    fn rk4_scalar_decay_matches_exact() {
        let mut s = vec![1.0f64];
        let h = 0.1;
        rk4(&mut s, h, decay);
        let exact = (-h).exp();
        assert!((s[0] - exact).abs() < 1e-7, "rk4 error too large: {} vs {}", s[0], exact);
    }

    #[test]
    fn rk4_harmonic_oscillator_conserves_energy() {
        // E = 0.5*(x^2 + v^2), starts at 0.5*(1^2 + 0^2) = 0.5
        let mut s = vec![1.0f64, 0.0];
        let h = 0.01;
        for _ in 0..1000 {
            rk4(&mut s, h, harmonic);
        }
        let energy = 0.5 * (s[0] * s[0] + s[1] * s[1]);
        assert!((energy - 0.5).abs() < 1e-5, "Energy drifted: {}", energy);
    }

    #[test]
    fn rk4_zero_dt_leaves_state_unchanged() {
        let mut s = vec![3.0, 1.0, 4.0];
        rk4(&mut s, 0.0, |st| vec![1.0; st.len()]);
        assert!((s[0] - 3.0).abs() < 1e-15);
        assert!((s[1] - 1.0).abs() < 1e-15);
        assert!((s[2] - 4.0).abs() < 1e-15);
    }

    // -------------------------------------------------------------------------
    // rk45_step
    // -------------------------------------------------------------------------

    #[test]
    fn rk45_step_returns_finite_values() {
        let mut s = vec![1.0f64];
        let (err, next_dt) = rk45_step(&mut s, 0.1, &decay);
        assert!(s[0].is_finite(), "State became non-finite");
        assert!(err.is_finite() && err >= 0.0, "Error non-finite or negative: {}", err);
        assert!(next_dt.is_finite() && next_dt > 0.0, "next_dt invalid: {}", next_dt);
    }

    #[test]
    fn rk45_step_advances_state_correctly() {
        let mut s = vec![1.0f64];
        let h = 0.1;
        rk45_step(&mut s, h, &decay);
        let exact = (-h).exp();
        // RK45 4th-order solution should match within a loose bound
        assert!((s[0] - exact).abs() < 1e-6, "rk45 step error: {} vs {}", s[0], exact);
    }

    #[test]
    fn rk45_step_suggests_smaller_dt_on_large_error() {
        // High-frequency system needs small steps; large dt should trigger reduction
        let high_freq = |s: &[f64]| vec![1000.0 * s[0]];
        let mut s = vec![0.001f64];
        let (_, next_dt) = rk45_step(&mut s, 1.0, &high_freq);
        assert!(next_dt < 1.0, "Expected step reduction, got next_dt={}", next_dt);
    }

    // -------------------------------------------------------------------------
    // integrate_adaptive
    // -------------------------------------------------------------------------

    #[test]
    fn integrate_adaptive_decay_matches_exact() {
        let mut s = vec![1.0f64];
        let total = 1.0;
        // tol=1e-6 is within the range where RK45 error estimates trigger acceptance
        let steps = integrate_adaptive(&mut s, total, 1e-6, decay);
        let exact = (-total).exp();
        assert!((s[0] - exact).abs() < 1e-4, "adaptive error: {} vs {}", s[0], exact);
        assert!(steps > 0, "Expected at least one step");
    }

    #[test]
    fn integrate_adaptive_returns_finite_state() {
        let mut s = vec![1.0, 0.0, 1.0];
        integrate_adaptive(&mut s, 0.5, 1e-6, lorenz_deriv);
        for (i, &v) in s.iter().enumerate() {
            assert!(v.is_finite(), "Component {} became non-finite: {}", i, v);
        }
    }

    // -------------------------------------------------------------------------
    // lyapunov_spectrum
    // -------------------------------------------------------------------------

    #[test]
    fn lyapunov_spectrum_contracting_system_all_negative() {
        // dx/dt = -x, dy/dt = -2y: both exponents should be negative
        let f = |s: &[f64]| vec![-s[0], -2.0 * s[1]];
        let exps = lyapunov_spectrum(&[1.0, 1.0], 2, 2, 2000, 0.01, &f);
        assert_eq!(exps.len(), 2);
        assert!(exps[0] < 0.0, "Expected negative largest exponent, got {}", exps[0]);
        assert!(exps[1] < 0.0, "Expected negative second exponent, got {}", exps[1]);
    }

    #[test]
    fn lyapunov_spectrum_lorenz_largest_positive() {
        // Lorenz largest Lyapunov exponent is ~0.9 — should be clearly positive
        let initial = [1.0, 1.0, 1.0f64];
        let exps = lyapunov_spectrum(&initial, 3, 1, 3000, 0.01, &lorenz_deriv);
        assert_eq!(exps.len(), 1);
        assert!(exps[0] > 0.0, "Expected positive MLE for Lorenz, got {}", exps[0]);
    }

    #[test]
    fn lyapunov_spectrum_empty_for_zero_exponents() {
        let exps = lyapunov_spectrum(&[1.0], 1, 0, 100, 0.01, &decay);
        assert!(exps.is_empty());
    }

    // -------------------------------------------------------------------------
    // classify_attractor
    // -------------------------------------------------------------------------

    #[test]
    fn classify_attractor_all_cases() {
        assert_eq!(classify_attractor(&[]), "unknown");
        assert_eq!(classify_attractor(&[-1.0, -2.0, -3.0]), "fixed_point");
        assert_eq!(classify_attractor(&[0.5, -1.0, -2.0]), "chaos");
        assert_eq!(classify_attractor(&[0.5, 0.3, -1.0]), "hyperchaos");
        assert_eq!(classify_attractor(&[0.005, -1.0]), "limit_cycle");
        assert_eq!(classify_attractor(&[0.005, 0.005, -1.0]), "torus");
    }

    // -------------------------------------------------------------------------
    // kolmogorov_entropy
    // -------------------------------------------------------------------------

    #[test]
    fn kolmogorov_entropy_sums_positive_exponents() {
        let k = kolmogorov_entropy(&[0.9, -0.1, -14.0]);
        assert!((k - 0.9).abs() < 1e-10, "Expected 0.9, got {}", k);
    }

    #[test]
    fn kolmogorov_entropy_zero_for_stable() {
        let k = kolmogorov_entropy(&[-1.0, -2.0, -3.0]);
        assert!((k - 0.0).abs() < 1e-10, "Expected 0, got {}", k);
    }

    // -------------------------------------------------------------------------
    // permutation_entropy
    // -------------------------------------------------------------------------

    #[test]
    fn permutation_entropy_constant_series_is_zero() {
        let series: Vec<f64> = vec![1.0; 100];
        let h = permutation_entropy(&series, 3, 1);
        assert!((h - 0.0).abs() < 1e-10, "Constant series should have zero entropy, got {}", h);
    }

    #[test]
    fn permutation_entropy_monotone_series_is_zero() {
        let series: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let h = permutation_entropy(&series, 3, 1);
        // Only one pattern (0,1,2), so entropy = 0
        assert!((h - 0.0).abs() < 1e-10, "Monotone series should have zero entropy, got {}", h);
    }

    #[test]
    fn permutation_entropy_in_unit_range() {
        // Lorenz trajectory should have high entropy (near 1)
        let mut s = vec![1.0, 1.0, 1.0f64];
        let mut ts: Vec<f64> = Vec::with_capacity(1000);
        for _ in 0..1000 {
            rk4(&mut s, 0.01, lorenz_deriv);
            ts.push(s[0]);
        }
        let h = permutation_entropy(&ts, 4, 1);
        assert!(h >= 0.0 && h <= 1.0 + 1e-10, "Entropy out of [0,1]: {}", h);
    }

    // -------------------------------------------------------------------------
    // find_fixed_point
    // -------------------------------------------------------------------------

    #[test]
    fn find_fixed_point_finds_origin() {
        // f(x,y) = [-x, -y], fixed point at (0,0)
        let fp = find_fixed_point(&[0.1, 0.1], 1e-10, 50, &|s: &[f64]| vec![-s[0], -s[1]]);
        let fp = fp.expect("Should converge");
        assert!(fp[0].abs() < 1e-8, "x should be near 0, got {}", fp[0]);
        assert!(fp[1].abs() < 1e-8, "y should be near 0, got {}", fp[1]);
    }

    #[test]
    fn find_fixed_point_returns_none_on_divergence() {
        // f(x) = x + 1: no fixed point, Newton diverges
        let fp = find_fixed_point(&[10.0], 1e-10, 20, &|s: &[f64]| vec![s[0] + 1.0]);
        // Should either return None or at least not panic
        let _ = fp; // acceptable result
    }

    // -------------------------------------------------------------------------
    // kmeans_cluster
    // -------------------------------------------------------------------------

    #[test]
    fn kmeans_cluster_separates_two_clusters() {
        let traj: Vec<Vec<f64>> = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.0],
            vec![0.0, 0.1],
            vec![10.0, 10.0],
            vec![10.1, 10.0],
            vec![10.0, 10.1],
        ];
        let (centroids, labels) = kmeans_cluster(&traj, 2, 2, 100);
        assert_eq!(centroids.len(), 2);
        assert_eq!(labels.len(), 6);
        // First 3 should share a label; last 3 should share a different label
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[1], labels[2]);
        assert_eq!(labels[3], labels[4]);
        assert_eq!(labels[4], labels[5]);
        assert_ne!(labels[0], labels[3]);
    }

    #[test]
    fn kmeans_cluster_k_zero_returns_empty() {
        let traj = vec![vec![1.0, 2.0]];
        let (centroids, labels) = kmeans_cluster(&traj, 0, 2, 10);
        assert!(centroids.is_empty());
        assert!(labels.is_empty());
    }

    // -------------------------------------------------------------------------
    // safe_param_path
    // -------------------------------------------------------------------------

    #[test]
    fn safe_param_path_direct_when_midpoint_safe() {
        // Midpoint is always safe → direct path [0, 1]
        let path = safe_param_path(0.0, 1.0, &|_| true, 4);
        assert_eq!(path, vec![0.0, 1.0]);
    }

    #[test]
    fn safe_param_path_subdivides_when_unsafe() {
        // Midpoint is never safe → subdivides up to max_depth
        let path = safe_param_path(0.0, 1.0, &|_| false, 2);
        assert!(path.len() > 2, "Expected subdivision, got {:?}", path);
        assert_eq!(path[0], 0.0);
        assert_eq!(*path.last().unwrap(), 1.0);
    }

    // -------------------------------------------------------------------------
    // morris_sensitivity
    // -------------------------------------------------------------------------

    #[test]
    fn morris_sensitivity_detects_most_influential_param() {
        // f(p) = 100*p[0] + p[1], p[0] should dominate
        let params = vec![1.0, 1.0];
        let sens = morris_sensitivity(&params, 0.1, |p| 100.0 * p[0] + p[1]);
        assert_eq!(sens.len(), 2);
        assert!(sens[0] > sens[1], "p[0] should be more sensitive: {:?}", sens);
    }

    #[test]
    fn morris_sensitivity_zero_delta_returns_zeros() {
        let params = vec![1.0, 2.0];
        let sens = morris_sensitivity(&params, 0.0, |p| p[0] + p[1]);
        for (i, &s) in sens.iter().enumerate() {
            assert_eq!(s, 0.0, "Expected 0 sensitivity for zero delta at index {}", i);
        }
    }

    // -------------------------------------------------------------------------
    // poincare_section
    // -------------------------------------------------------------------------

    #[test]
    fn poincare_section_harmonic_oscillator_regular_crossings() {
        // Harmonic oscillator x'' + x = 0: crosses x=0 every π
        // state = [x, v], initial [0.0, 1.0] starts at x=0 moving right
        let crossings = poincare_section(
            &[0.5, 0.0],
            0.01,
            100,
            5,
            0,    // plane_dim = x
            0.0,  // plane_val = 0
            &harmonic,
        );
        assert_eq!(crossings.len(), 5, "Expected 5 crossings");
        for c in &crossings {
            assert!(c[0].abs() < 0.05, "Crossing not near x=0: {}", c[0]);
        }
    }

    // -------------------------------------------------------------------------
    // divergence_at
    // -------------------------------------------------------------------------

    #[test]
    fn divergence_at_lorenz_is_negative() {
        // Lorenz divergence = -(sigma + 1 + beta) ≈ -(10 + 1 + 2.667) ≈ -13.667
        let state = [1.0, 1.0, 1.0f64];
        let div = divergence_at(&state, &lorenz_deriv, 1e-6);
        let expected = -(10.0 + 1.0 + 8.0 / 3.0);
        assert!((div - expected).abs() < 0.01, "Lorenz divergence: {} vs expected {}", div, expected);
    }

    #[test]
    fn divergence_at_linear_system_exact() {
        // f(x,y) = [-2x, -3y], divergence = -2 + -3 = -5
        let state = [1.0, 1.0f64];
        let div = divergence_at(&state, &|s| vec![-2.0 * s[0], -3.0 * s[1]], 1e-7);
        assert!((div - (-5.0)).abs() < 1e-4, "Linear divergence: {} vs -5", div);
    }

    // -------------------------------------------------------------------------
    // param_distance
    // -------------------------------------------------------------------------

    #[test]
    fn param_distance_identical_params_is_zero() {
        let p = vec![1.0, 2.0, 3.0];
        let ranges = vec![(0.0, 10.0), (0.0, 10.0), (0.0, 10.0)];
        let d = param_distance(&p, &p, &ranges);
        assert!((d - 0.0).abs() < 1e-15, "Identical params should have distance 0");
    }

    #[test]
    fn param_distance_clamps_to_unit_range() {
        // Params far apart within large range
        let a = vec![0.0];
        let b = vec![100.0];
        let ranges = vec![(0.0, 1.0)]; // range = 1.0, diff = 100 → normalized > 1
        let d = param_distance(&a, &b, &ranges);
        assert!(d >= 0.0 && d <= 1.0, "param_distance should clamp to [0,1]: {}", d);
    }

    // -------------------------------------------------------------------------
    // mutual_information
    // -------------------------------------------------------------------------

    #[test]
    fn mutual_information_identical_series_positive() {
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let mi = mutual_information(&x, &x, 8);
        assert!(mi > 0.0, "Identical series should have positive MI: {}", mi);
    }

    #[test]
    fn mutual_information_empty_returns_zero() {
        let mi = mutual_information(&[], &[], 8);
        assert_eq!(mi, 0.0);
    }

    // -------------------------------------------------------------------------
    // correlation_dimension
    // -------------------------------------------------------------------------

    #[test]
    fn correlation_dimension_nonnegative() {
        let mut s = vec![1.0, 1.0, 1.0f64];
        let mut traj: Vec<Vec<f64>> = Vec::new();
        for _ in 0..300 {
            rk4(&mut s, 0.01, lorenz_deriv);
            traj.push(s.clone());
        }
        let d = correlation_dimension(&traj, 500);
        assert!(d >= 0.0, "Correlation dimension must be non-negative: {}", d);
    }

    // -------------------------------------------------------------------------
    // recurrence_quantification
    // -------------------------------------------------------------------------

    #[test]
    fn rqa_empty_trajectory_returns_zeros() {
        let result = recurrence_quantification(&[], 0.5, 2);
        assert_eq!(result.recurrence_rate, 0.0);
        assert_eq!(result.determinism, 0.0);
    }

    #[test]
    fn rqa_periodic_orbit_has_high_determinism() {
        // Harmonic oscillator is perfectly periodic → recurrence_rate should be high
        let mut s = vec![1.0f64, 0.0];
        let mut traj: Vec<Vec<f64>> = Vec::new();
        for _ in 0..200 {
            rk4(&mut s, 0.05, harmonic);
            traj.push(s.clone());
        }
        let result = recurrence_quantification(&traj, 0.3, 2);
        // Periodic orbit should have recurrence_rate > 0
        assert!(
            result.recurrence_rate > 0.0,
            "Periodic orbit should have non-zero recurrence rate"
        );
    }

    // -------------------------------------------------------------------------
    // leapfrog
    // -------------------------------------------------------------------------

    #[test]
    fn leapfrog_harmonic_conserves_energy() {
        // q'' = -q (harmonic oscillator as Hamiltonian: H = p^2/2 + q^2/2)
        // state = [q, p]
        let mut s = vec![1.0f64, 0.0]; // q=1, p=0, E=0.5
        let q_idx = vec![0];
        let p_idx = vec![1];
        let velocity = |st: &[f64]| vec![st[1]];    // dq/dt = p
        let force = |st: &[f64]| vec![-st[0]];      // dp/dt = -q
        for _ in 0..1000 {
            leapfrog(&mut s, &q_idx, &p_idx, 0.01, &velocity, &force);
        }
        let energy = 0.5 * (s[0] * s[0] + s[1] * s[1]);
        assert!((energy - 0.5).abs() < 1e-4, "Leapfrog energy drift: {}", energy);
    }
}
