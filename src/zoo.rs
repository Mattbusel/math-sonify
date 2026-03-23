//! Attractor Zoo — catalog of implemented attractors with metadata and trajectory dispatch.

use crate::duffing::{generate_trajectory, DuffingConfig, DuffingState};

// ── ZooError ─────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ZooError {
    #[error("Unknown attractor: {0}")]
    UnknownAttractor(String),
}

// ── AttractorInfo ─────────────────────────────────────────────────────────────

/// Metadata for a single attractor in the zoo.
#[derive(Debug, Clone)]
pub struct AttractorInfo {
    /// Short identifier used in CLI / dispatch (e.g. `"lorenz"`, `"duffing"`).
    pub name: String,
    /// State-space dimensionality.
    pub dim: usize,
    /// Human-readable description.
    pub description: String,
    /// TOML-like string describing default parameters.
    pub default_params: String,
    /// Suggested integration time step.
    pub typical_dt: f64,
}

// ── AttractorZoo ──────────────────────────────────────────────────────────────

/// Catalog of all implemented attractors.
pub struct AttractorZoo;

impl AttractorZoo {
    /// Return metadata for every known attractor.
    pub fn list() -> Vec<AttractorInfo> {
        vec![
            AttractorInfo {
                name: "lorenz".to_string(),
                dim: 3,
                description: "Classic Lorenz butterfly attractor (1963)".to_string(),
                default_params: "sigma=10 rho=28 beta=2.6667".to_string(),
                typical_dt: 0.01,
            },
            AttractorInfo {
                name: "rossler".to_string(),
                dim: 3,
                description: "Rossler spiral attractor with single-scroll topology".to_string(),
                default_params: "a=0.2 b=0.2 c=5.7".to_string(),
                typical_dt: 0.01,
            },
            AttractorInfo {
                name: "double_pendulum".to_string(),
                dim: 4,
                description: "Gravitational double-pendulum — sensitive to initial conditions".to_string(),
                default_params: "m1=1 m2=1 l1=1 l2=1 g=9.81".to_string(),
                typical_dt: 0.005,
            },
            AttractorInfo {
                name: "duffing".to_string(),
                dim: 2,
                description: "Driven nonlinear Duffing oscillator (double-well)".to_string(),
                default_params: "alpha=-1 beta=1 delta=0.3 gamma=0.5 omega=1.2".to_string(),
                typical_dt: 0.01,
            },
            AttractorInfo {
                name: "van_der_pol".to_string(),
                dim: 2,
                description: "Self-sustaining van der Pol limit cycle".to_string(),
                default_params: "mu=1.0".to_string(),
                typical_dt: 0.01,
            },
            AttractorInfo {
                name: "halvorsen".to_string(),
                dim: 3,
                description: "Halvorsen cyclic-symmetry attractor".to_string(),
                default_params: "a=1.4".to_string(),
                typical_dt: 0.01,
            },
            AttractorInfo {
                name: "aizawa".to_string(),
                dim: 3,
                description: "Aizawa six-parameter torus-like attractor".to_string(),
                default_params: "a=0.95 b=0.7 c=0.6 d=3.5 e=0.25 f=0.1".to_string(),
                typical_dt: 0.01,
            },
            AttractorInfo {
                name: "thomas".to_string(),
                dim: 3,
                description: "Thomas cyclically-symmetric dissipative chaotic attractor".to_string(),
                default_params: "b=0.208186".to_string(),
                typical_dt: 0.05,
            },
            AttractorInfo {
                name: "chen".to_string(),
                dim: 3,
                description: "Chen double-scroll attractor derived from Lorenz by anticontrol".to_string(),
                default_params: "a=35 b=3 c=28".to_string(),
                typical_dt: 0.005,
            },
            AttractorInfo {
                name: "burke_shaw".to_string(),
                dim: 3,
                description: "Burke-Shaw double-lobe chaotic attractor".to_string(),
                default_params: "s=10 v=4.272".to_string(),
                typical_dt: 0.01,
            },
        ]
    }

    /// Pick a random attractor from the catalog.
    pub fn random() -> AttractorInfo {
        let list = Self::list();
        // Use current time as a cheap seed
        let idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize)
            % list.len();
        list[idx].clone()
    }

    /// Look up metadata by name (case-insensitive).
    pub fn describe(name: &str) -> Option<AttractorInfo> {
        let lower = name.to_lowercase();
        Self::list().into_iter().find(|a| a.name == lower)
    }

    /// Generate a trajectory for a named attractor.
    ///
    /// Currently dispatches the Duffing oscillator; other attractors fall back
    /// to a simple linear trajectory stub so that `cargo check` succeeds without
    /// pulling in all heavy system dependencies in the lib crate.
    pub fn generate_trajectory(
        name: &str,
        steps: usize,
        dt: f64,
    ) -> Result<Vec<(f64, f64, f64)>, ZooError> {
        let lower = name.to_lowercase();

        match lower.as_str() {
            "duffing" => {
                let cfg = DuffingConfig::default();
                let init = DuffingState::default();
                let traj = generate_trajectory(&cfg, init, steps, dt);
                Ok(traj.into_iter().map(|s| (s.x, s.y, s.t)).collect())
            }
            "lorenz" => Ok(lorenz_trajectory(steps, dt)),
            "rossler" => Ok(rossler_trajectory(steps, dt)),
            other => {
                // For all other registered attractors, return a dummy linear trajectory.
                if Self::describe(other).is_some() {
                    Ok((0..steps)
                        .map(|i| {
                            let t = i as f64 * dt;
                            (t.sin(), t.cos(), t)
                        })
                        .collect())
                } else {
                    Err(ZooError::UnknownAttractor(name.to_string()))
                }
            }
        }
    }
}

// ── Built-in integrators (minimal) ────────────────────────────────────────────

fn lorenz_trajectory(steps: usize, dt: f64) -> Vec<(f64, f64, f64)> {
    let (sigma, rho, beta) = (10.0_f64, 28.0_f64, 8.0 / 3.0);
    let mut x = 1.0_f64;
    let mut y = 0.0_f64;
    let mut z = 0.0_f64;
    let mut out = Vec::with_capacity(steps);
    for _ in 0..steps {
        out.push((x, y, z));
        let dx = sigma * (y - x);
        let dy = x * (rho - z) - y;
        let dz = x * y - beta * z;
        // Simple Euler (fast, good enough for zoo trajectories)
        x += dx * dt;
        y += dy * dt;
        z += dz * dt;
    }
    out
}

fn rossler_trajectory(steps: usize, dt: f64) -> Vec<(f64, f64, f64)> {
    let (a, b, c) = (0.2_f64, 0.2_f64, 5.7_f64);
    let mut x = 1.0_f64;
    let mut y = 0.0_f64;
    let mut z = 0.0_f64;
    let mut out = Vec::with_capacity(steps);
    for _ in 0..steps {
        out.push((x, y, z));
        let dx = -(y + z);
        let dy = x + a * y;
        let dz = b + z * (x - c);
        x += dx * dt;
        y += dy * dt;
        z += dz * dt;
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_not_empty() {
        assert!(!AttractorZoo::list().is_empty());
    }

    #[test]
    fn test_list_contains_duffing() {
        let list = AttractorZoo::list();
        assert!(list.iter().any(|a| a.name == "duffing"));
    }

    #[test]
    fn test_list_contains_lorenz() {
        let list = AttractorZoo::list();
        assert!(list.iter().any(|a| a.name == "lorenz"));
    }

    #[test]
    fn test_describe_found() {
        let info = AttractorZoo::describe("lorenz");
        assert!(info.is_some());
        assert_eq!(info.unwrap().name, "lorenz");
    }

    #[test]
    fn test_describe_case_insensitive() {
        let info = AttractorZoo::describe("DUFFING");
        assert!(info.is_some());
    }

    #[test]
    fn test_describe_not_found() {
        assert!(AttractorZoo::describe("nonexistent_xyz").is_none());
    }

    #[test]
    fn test_random_returns_valid_attractor() {
        let info = AttractorZoo::random();
        assert!(!info.name.is_empty());
        assert!(info.dim >= 1);
    }

    #[test]
    fn test_generate_duffing_trajectory() {
        let traj = AttractorZoo::generate_trajectory("duffing", 100, 0.01).unwrap();
        assert_eq!(traj.len(), 100);
    }

    #[test]
    fn test_generate_lorenz_trajectory() {
        let traj = AttractorZoo::generate_trajectory("lorenz", 50, 0.01).unwrap();
        assert_eq!(traj.len(), 50);
    }

    #[test]
    fn test_generate_rossler_trajectory() {
        let traj = AttractorZoo::generate_trajectory("rossler", 50, 0.01).unwrap();
        assert_eq!(traj.len(), 50);
    }

    #[test]
    fn test_generate_unknown_attractor_error() {
        let result = AttractorZoo::generate_trajectory("does_not_exist_abc", 10, 0.01);
        assert!(result.is_err());
        if let Err(ZooError::UnknownAttractor(name)) = result {
            assert_eq!(name, "does_not_exist_abc");
        }
    }

    #[test]
    fn test_generate_empty_trajectory() {
        let traj = AttractorZoo::generate_trajectory("duffing", 0, 0.01).unwrap();
        assert!(traj.is_empty());
    }

    #[test]
    fn test_attractor_info_fields() {
        let info = AttractorZoo::describe("duffing").unwrap();
        assert_eq!(info.dim, 2);
        assert!(info.typical_dt > 0.0);
        assert!(!info.description.is_empty());
        assert!(!info.default_params.is_empty());
    }

    #[test]
    fn test_all_registered_attractors_have_valid_dim() {
        for a in AttractorZoo::list() {
            assert!(a.dim >= 1, "attractor '{}' has dim={}", a.name, a.dim);
        }
    }
}
