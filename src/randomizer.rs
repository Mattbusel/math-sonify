//! Attractor parameter randomizer.
//!
//! Generates interesting random parameter sets for each attractor type and
//! optionally validates them against trajectory-based constraints (boundedness,
//! rough chaoticity).

use std::collections::HashMap;

// ── ParamConstraint ───────────────────────────────────────────────────────────

/// Constraint applied when generating random attractor parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamConstraint {
    /// Accept any parameters, no validation.
    Any,
    /// Accept only parameters whose short trajectory stays within ±`bound`
    /// for all coordinates.
    Bounded { bound: f64 },
    /// Prefer parameters that look chaotic: at least one coordinate grows
    /// slightly in a short test run (positive finite-time Lyapunov proxy).
    Chaotic,
}

// ── RandomConfig ──────────────────────────────────────────────────────────────

/// Configuration for the parameter randomizer.
#[derive(Debug, Clone)]
pub struct RandomConfig {
    /// Optional RNG seed for reproducibility. If `None`, a time-based seed is
    /// used.
    pub seed: Option<u64>,
    /// Constraint to apply when generating parameters.
    pub constraint: ParamConstraint,
}

impl Default for RandomConfig {
    fn default() -> Self {
        Self {
            seed: None,
            constraint: ParamConstraint::Any,
        }
    }
}

// ── Inline LCG RNG ────────────────────────────────────────────────────────────
// A minimal linear-congruential generator so we don't need to pull `rand` in
// tests — `rand` is already a project dependency, but we keep this self-
// contained so the module is usable in any context.

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(1) }
    }

    fn next_u64(&mut self) -> u64 {
        // Knuth MMIX multiplier / increment
        self.state = self.state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    /// Return a value in [lo, hi].
    fn gen_range(&mut self, lo: f64, hi: f64) -> f64 {
        let t = (self.next_u64() >> 11) as f64 / (u64::MAX >> 11) as f64;
        lo + t * (hi - lo)
    }
}

fn make_lcg(config: &RandomConfig) -> Lcg {
    let seed = config.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42)
    });
    Lcg::new(seed)
}

// ── Simple integrators for constraint checking ────────────────────────────────

/// Run a Lorenz step using Euler integration (fast, for constraint checking).
fn lorenz_step(s: &mut [f64; 3], sigma: f64, rho: f64, beta: f64, dt: f64) {
    let dx = sigma * (s[1] - s[0]);
    let dy = s[0] * (rho - s[2]) - s[1];
    let dz = s[0] * s[1] - beta * s[2];
    s[0] += dx * dt;
    s[1] += dy * dt;
    s[2] += dz * dt;
}

fn rossler_step(s: &mut [f64; 3], a: f64, b: f64, c: f64, dt: f64) {
    let dx = -s[1] - s[2];
    let dy = s[0] + a * s[1];
    let dz = b + s[2] * (s[0] - c);
    s[0] += dx * dt;
    s[1] += dy * dt;
    s[2] += dz * dt;
}

fn van_der_pol_step(s: &mut [f64; 2], mu: f64, omega: f64, dt: f64) {
    let dx = s[1];
    let dy = mu * (1.0 - s[0] * s[0]) * s[1] - omega * omega * s[0];
    s[0] += dx * dt;
    s[1] += dy * dt;
}

fn hindmarsh_rose_step(s: &mut [f64; 3], a: f64, b: f64, c: f64, d: f64, r: f64, s_param: f64, x_r: f64, i: f64, dt: f64) {
    let dx = s[1] - a * s[0] * s[0] * s[0] + b * s[0] * s[0] - s[2] + i;
    let dy = c - d * s[0] * s[0] - s[1];
    let dz = r * (s_param * (s[0] - x_r) - s[2]);
    s[0] += dx * dt;
    s[1] += dy * dt;
    s[2] += dz * dt;
}

// ── Constraint validation ─────────────────────────────────────────────────────

fn check_bounded_lorenz(sigma: f64, rho: f64, beta: f64, bound: f64) -> bool {
    let mut s = [1.0f64, 0.1, 0.1];
    let dt = 0.01;
    for _ in 0..1000 {
        lorenz_step(&mut s, sigma, rho, beta, dt);
        if s.iter().any(|v| v.abs() > bound || !v.is_finite()) {
            return false;
        }
    }
    true
}

fn check_bounded_rossler(a: f64, b: f64, c: f64, bound: f64) -> bool {
    let mut s = [0.1f64, 0.1, 0.1];
    let dt = 0.01;
    for _ in 0..1000 {
        rossler_step(&mut s, a, b, c, dt);
        if s.iter().any(|v| v.abs() > bound || !v.is_finite()) {
            return false;
        }
    }
    true
}

fn check_bounded_van_der_pol(mu: f64, omega: f64, bound: f64) -> bool {
    let mut s = [0.1f64, 0.1];
    let dt = 0.01;
    for _ in 0..1000 {
        van_der_pol_step(&mut s, mu, omega, dt);
        if s.iter().any(|v| v.abs() > bound || !v.is_finite()) {
            return false;
        }
    }
    true
}

fn check_bounded_hr(a: f64, b: f64, c: f64, d: f64, r: f64, s_p: f64, x_r: f64, ii: f64, bound: f64) -> bool {
    let mut s = [-1.6f64, -9.0, 1.0];
    let dt = 0.01;
    for _ in 0..1000 {
        hindmarsh_rose_step(&mut s, a, b, c, d, r, s_p, x_r, ii, dt);
        if s.iter().any(|v| v.abs() > bound || !v.is_finite()) {
            return false;
        }
    }
    true
}

fn check_chaotic_lorenz(sigma: f64, rho: f64, beta: f64) -> bool {
    // Proxy: run two slightly offset trajectories and check if they diverge.
    let mut s1 = [1.0f64, 0.0, 0.0];
    let mut s2 = [1.001f64, 0.0, 0.0];
    let dt = 0.01;
    for _ in 0..500 {
        lorenz_step(&mut s1, sigma, rho, beta, dt);
        lorenz_step(&mut s2, sigma, rho, beta, dt);
    }
    let d = ((s1[0] - s2[0]).powi(2) + (s1[1] - s2[1]).powi(2) + (s1[2] - s2[2]).powi(2)).sqrt();
    d > 1.0 && s1.iter().all(|v| v.is_finite())
}

// ── Config types (mirrors config.rs, but standalone here) ────────────────────

/// Randomized Lorenz parameters.
#[derive(Debug, Clone)]
pub struct LorenzConfig {
    pub sigma: f64,
    pub rho: f64,
    pub beta: f64,
}

/// Randomized Rössler parameters.
#[derive(Debug, Clone)]
pub struct RosslerConfig {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

/// Randomized Van der Pol parameters.
#[derive(Debug, Clone)]
pub struct VanDerPolConfig {
    pub mu: f64,
    pub omega: f64,
}

/// Randomized Hindmarsh-Rose neuron model parameters.
#[derive(Debug, Clone)]
pub struct HindmarshRoseConfig {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub r: f64,
    pub s: f64,
    pub x_r: f64,
    pub i: f64,
}

// ── ParameterRandomizer ───────────────────────────────────────────────────────

/// Generates interesting random parameter sets for attractor systems.
pub struct ParameterRandomizer;

impl ParameterRandomizer {
    /// Generate a random [`LorenzConfig`] using the provided config.
    ///
    /// - σ ∈ [5, 15], ρ ∈ [20, 35], β ∈ [1, 4]
    /// - At most `max_tries` random draws are made to satisfy the constraint.
    pub fn random_lorenz(config: &RandomConfig) -> LorenzConfig {
        let mut rng = make_lcg(config);
        let bound = if config.constraint == (ParamConstraint::Bounded { bound: 50.0 }) {
            50.0
        } else {
            200.0
        };
        for _ in 0..64 {
            let sigma = rng.gen_range(5.0, 15.0);
            let rho = rng.gen_range(20.0, 35.0);
            let beta = rng.gen_range(1.0, 4.0);
            let ok = match &config.constraint {
                ParamConstraint::Any => true,
                ParamConstraint::Bounded { bound: b } => check_bounded_lorenz(sigma, rho, beta, *b),
                ParamConstraint::Chaotic => check_chaotic_lorenz(sigma, rho, beta),
            };
            if ok {
                return LorenzConfig { sigma, rho, beta };
            }
            let _ = bound;
        }
        // Fallback to well-known chaotic parameters
        LorenzConfig { sigma: 10.0, rho: 28.0, beta: 2.6667 }
    }

    /// Generate a random [`RosslerConfig`].
    ///
    /// - a ∈ [0.1, 0.4], b ∈ [0.1, 0.4], c ∈ [4.0, 10.0]
    pub fn random_rossler(config: &RandomConfig) -> RosslerConfig {
        let mut rng = make_lcg(config);
        for _ in 0..64 {
            let a = rng.gen_range(0.1, 0.4);
            let b = rng.gen_range(0.1, 0.4);
            let c = rng.gen_range(4.0, 10.0);
            let ok = match &config.constraint {
                ParamConstraint::Any => true,
                ParamConstraint::Bounded { bound: bnd } => check_bounded_rossler(a, b, c, *bnd),
                ParamConstraint::Chaotic => {
                    // Rössler is almost always chaotic in this range; just check it's finite.
                    check_bounded_rossler(a, b, c, 1000.0)
                }
            };
            if ok {
                return RosslerConfig { a, b, c };
            }
        }
        RosslerConfig { a: 0.2, b: 0.2, c: 5.7 }
    }

    /// Generate a random [`VanDerPolConfig`].
    ///
    /// - μ ∈ [0.5, 5.0], ω ∈ [0.5, 2.0]
    pub fn random_van_der_pol(config: &RandomConfig) -> VanDerPolConfig {
        let mut rng = make_lcg(config);
        for _ in 0..64 {
            let mu = rng.gen_range(0.5, 5.0);
            let omega = rng.gen_range(0.5, 2.0);
            let ok = match &config.constraint {
                ParamConstraint::Any => true,
                ParamConstraint::Bounded { bound: bnd } => check_bounded_van_der_pol(mu, omega, *bnd),
                ParamConstraint::Chaotic => check_bounded_van_der_pol(mu, omega, 500.0),
            };
            if ok {
                return VanDerPolConfig { mu, omega };
            }
        }
        VanDerPolConfig { mu: 1.0, omega: 1.0 }
    }

    /// Generate a random [`HindmarshRoseConfig`].
    ///
    /// Uses standard physiological ranges:
    /// - a=1, b ∈ [2,4], c=-3, d=5, r ∈ [0.001, 0.01], s ∈ [3,5], x_r=-1.6, I ∈ [1,5]
    pub fn random_hindmarsh_rose(config: &RandomConfig) -> HindmarshRoseConfig {
        let mut rng = make_lcg(config);
        for _ in 0..64 {
            let b = rng.gen_range(2.0, 4.0);
            let r = rng.gen_range(0.001, 0.01);
            let s = rng.gen_range(3.0, 5.0);
            let i = rng.gen_range(1.0, 5.0);
            let ok = match &config.constraint {
                ParamConstraint::Any => true,
                ParamConstraint::Bounded { bound: bnd } => {
                    check_bounded_hr(1.0, b, -3.0, 5.0, r, s, -1.6, i, *bnd)
                }
                ParamConstraint::Chaotic => {
                    check_bounded_hr(1.0, b, -3.0, 5.0, r, s, -1.6, i, 500.0)
                }
            };
            if ok {
                return HindmarshRoseConfig { a: 1.0, b, c: -3.0, d: 5.0, r, s, x_r: -1.6, i };
            }
        }
        HindmarshRoseConfig { a: 1.0, b: 3.0, c: -3.0, d: 5.0, r: 0.006, s: 4.0, x_r: -1.6, i: 3.0 }
    }

    /// Generate one random config for each supported attractor.
    ///
    /// Returns a map from attractor name to a string description of the params.
    pub fn random_all(config: &RandomConfig) -> HashMap<String, String> {
        let lorenz = Self::random_lorenz(config);
        let rossler = Self::random_rossler(config);
        let vdp = Self::random_van_der_pol(config);
        let hr = Self::random_hindmarsh_rose(config);

        let mut map = HashMap::new();
        map.insert(
            "lorenz".to_string(),
            format!("sigma={:.3} rho={:.3} beta={:.3}", lorenz.sigma, lorenz.rho, lorenz.beta),
        );
        map.insert(
            "rossler".to_string(),
            format!("a={:.3} b={:.3} c={:.3}", rossler.a, rossler.b, rossler.c),
        );
        map.insert(
            "van_der_pol".to_string(),
            format!("mu={:.3} omega={:.3}", vdp.mu, vdp.omega),
        );
        map.insert(
            "hindmarsh_rose".to_string(),
            format!("a={:.3} b={:.3} r={:.4} s={:.3} i={:.3}", hr.a, hr.b, hr.r, hr.s, hr.i),
        );
        map
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn any_cfg(seed: u64) -> RandomConfig {
        RandomConfig { seed: Some(seed), constraint: ParamConstraint::Any }
    }

    fn bounded_cfg(seed: u64) -> RandomConfig {
        RandomConfig { seed: Some(seed), constraint: ParamConstraint::Bounded { bound: 50.0 } }
    }

    fn chaotic_cfg(seed: u64) -> RandomConfig {
        RandomConfig { seed: Some(seed), constraint: ParamConstraint::Chaotic }
    }

    // 1. Lorenz sigma in range
    #[test]
    fn lorenz_sigma_in_range() {
        let cfg = any_cfg(1);
        let p = ParameterRandomizer::random_lorenz(&cfg);
        assert!(p.sigma >= 5.0 && p.sigma <= 15.0, "sigma={}", p.sigma);
    }

    // 2. Lorenz rho in range
    #[test]
    fn lorenz_rho_in_range() {
        let cfg = any_cfg(2);
        let p = ParameterRandomizer::random_lorenz(&cfg);
        assert!(p.rho >= 20.0 && p.rho <= 35.0, "rho={}", p.rho);
    }

    // 3. Lorenz beta in range
    #[test]
    fn lorenz_beta_in_range() {
        let cfg = any_cfg(3);
        let p = ParameterRandomizer::random_lorenz(&cfg);
        assert!(p.beta >= 1.0 && p.beta <= 4.0, "beta={}", p.beta);
    }

    // 4. Bounded constraint: Lorenz trajectory stays within ±50
    #[test]
    fn lorenz_bounded_trajectory() {
        let cfg = bounded_cfg(4);
        let p = ParameterRandomizer::random_lorenz(&cfg);
        assert!(check_bounded_lorenz(p.sigma, p.rho, p.beta, 50.0),
            "trajectory exceeded bound");
    }

    // 5. Chaotic constraint: Lorenz trajectories diverge
    #[test]
    fn lorenz_chaotic() {
        let cfg = chaotic_cfg(5);
        let p = ParameterRandomizer::random_lorenz(&cfg);
        assert!(check_chaotic_lorenz(p.sigma, p.rho, p.beta),
            "expected chaotic trajectory");
    }

    // 6. Rossler a in range
    #[test]
    fn rossler_a_in_range() {
        let cfg = any_cfg(6);
        let p = ParameterRandomizer::random_rossler(&cfg);
        assert!(p.a >= 0.1 && p.a <= 0.4, "a={}", p.a);
    }

    // 7. Rossler b in range
    #[test]
    fn rossler_b_in_range() {
        let cfg = any_cfg(7);
        let p = ParameterRandomizer::random_rossler(&cfg);
        assert!(p.b >= 0.1 && p.b <= 0.4, "b={}", p.b);
    }

    // 8. Rossler c in range
    #[test]
    fn rossler_c_in_range() {
        let cfg = any_cfg(8);
        let p = ParameterRandomizer::random_rossler(&cfg);
        assert!(p.c >= 4.0 && p.c <= 10.0, "c={}", p.c);
    }

    // 9. Van der Pol mu in range
    #[test]
    fn van_der_pol_mu_in_range() {
        let cfg = any_cfg(9);
        let p = ParameterRandomizer::random_van_der_pol(&cfg);
        assert!(p.mu >= 0.5 && p.mu <= 5.0, "mu={}", p.mu);
    }

    // 10. Van der Pol omega in range
    #[test]
    fn van_der_pol_omega_in_range() {
        let cfg = any_cfg(10);
        let p = ParameterRandomizer::random_van_der_pol(&cfg);
        assert!(p.omega >= 0.5 && p.omega <= 2.0, "omega={}", p.omega);
    }

    // 11. Bounded Van der Pol trajectory stays within ±50
    #[test]
    fn van_der_pol_bounded() {
        let cfg = bounded_cfg(11);
        let p = ParameterRandomizer::random_van_der_pol(&cfg);
        assert!(check_bounded_van_der_pol(p.mu, p.omega, 50.0));
    }

    // 12. Hindmarsh-Rose returns valid params
    #[test]
    fn hindmarsh_rose_valid() {
        let cfg = any_cfg(12);
        let p = ParameterRandomizer::random_hindmarsh_rose(&cfg);
        assert!(p.a.is_finite());
        assert!(p.b >= 2.0 && p.b <= 4.0, "b={}", p.b);
        assert!(p.r >= 0.001 && p.r <= 0.01, "r={}", p.r);
    }

    // 13. Hindmarsh-Rose bounded constraint
    #[test]
    fn hindmarsh_rose_bounded() {
        let cfg = bounded_cfg(13);
        let p = ParameterRandomizer::random_hindmarsh_rose(&cfg);
        assert!(check_bounded_hr(p.a, p.b, p.c, p.d, p.r, p.s, p.x_r, p.i, 50.0));
    }

    // 14. random_all returns all four attractor keys
    #[test]
    fn random_all_keys() {
        let cfg = any_cfg(14);
        let map = ParameterRandomizer::random_all(&cfg);
        assert!(map.contains_key("lorenz"));
        assert!(map.contains_key("rossler"));
        assert!(map.contains_key("van_der_pol"));
        assert!(map.contains_key("hindmarsh_rose"));
    }

    // 15. random_all values are non-empty strings
    #[test]
    fn random_all_values_nonempty() {
        let cfg = any_cfg(15);
        let map = ParameterRandomizer::random_all(&cfg);
        for (k, v) in &map {
            assert!(!v.is_empty(), "empty value for key {}", k);
        }
    }

    // 16. Seeded randomizer is deterministic
    #[test]
    fn seeded_deterministic() {
        let cfg = any_cfg(999);
        let p1 = ParameterRandomizer::random_lorenz(&cfg);
        let p2 = ParameterRandomizer::random_lorenz(&cfg);
        assert!((p1.sigma - p2.sigma).abs() < 1e-12);
        assert!((p1.rho - p2.rho).abs() < 1e-12);
        assert!((p1.beta - p2.beta).abs() < 1e-12);
    }

    // 17. Different seeds produce different params (with high probability)
    #[test]
    fn different_seeds_different_params() {
        let p1 = ParameterRandomizer::random_lorenz(&any_cfg(1));
        let p2 = ParameterRandomizer::random_lorenz(&any_cfg(2));
        let same = (p1.sigma - p2.sigma).abs() < 1e-12
            && (p1.rho - p2.rho).abs() < 1e-12
            && (p1.beta - p2.beta).abs() < 1e-12;
        assert!(!same, "different seeds produced identical params");
    }

    // 18. check_bounded_lorenz: canonical params are bounded at ±50?
    //     (Actually Lorenz canonical params can reach ~30 for x, fine)
    #[test]
    fn canonical_lorenz_bounded_check() {
        assert!(check_bounded_lorenz(10.0, 28.0, 2.6667, 100.0));
    }

    // 19. VLQ: check a large VLQ value for Lorenz step sanity
    #[test]
    fn lorenz_step_smoke() {
        let mut s = [1.0f64, 0.0, 0.0];
        lorenz_step(&mut s, 10.0, 28.0, 2.6667, 0.001);
        assert!(s.iter().all(|v| v.is_finite()));
    }

    // 20. Rossler step smoke test
    #[test]
    fn rossler_step_smoke() {
        let mut s = [0.1f64, 0.1, 0.1];
        rossler_step(&mut s, 0.2, 0.2, 5.7, 0.01);
        assert!(s.iter().all(|v| v.is_finite()));
    }
}
