use super::DynamicalSystem;

/// Geodesic flow on a flat torus T² with major radius R and tube radius r.
/// State: [φ, θ, φ̇, θ̇] — angles and angular velocities.
/// On the flat torus (R→∞ limit), geodesics are straight lines — winding number
/// determines periodicity. On the embedded torus with metric, we solve the
/// geodesic equations from the Christoffel symbols.
///
/// The embedded metric is: ds² = (R + r·cos θ)²dφ² + r²dθ²
/// Geodesic equations (derived from Euler-Lagrange):
///   φ̈ = -2(r·sin θ / (R + r·cos θ)) · φ̇ · θ̇
///   θ̈ = (R + r·cos θ)·sin θ / r · φ̇²
pub struct GeodesicTorus {
    state: Vec<f64>,
    pub big_r: f64,
    pub small_r: f64,
    speed: f64,
}

impl GeodesicTorus {
    /// `big_r`: distance from tube center to torus center.
    /// `small_r`: tube radius.
    /// Initial velocity with components (dphi, dtheta) determines winding number.
    pub fn new(big_r: f64, small_r: f64) -> Self {
        // Winding number ≈ dphi/dtheta — use golden ratio for ergodic flow
        let phi_dot = 1.0;
        let theta_dot = 1.0 / 1.618_033_988_749; // irrational → ergodic
        Self {
            state: vec![0.0, 0.0, phi_dot, theta_dot],
            big_r,
            small_r,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], big_r: f64, small_r: f64) -> Vec<f64> {
        let (phi, theta, dphi, dtheta) = (s[0], s[1], s[2], s[3]);
        let factor = big_r + small_r * theta.cos();
        let ddphi = -2.0 * (small_r * theta.sin() / factor.max(1e-10)) * dphi * dtheta;
        let ddtheta = factor * theta.sin() / small_r.max(1e-10) * dphi * dphi;
        vec![dphi, dtheta, ddphi, ddtheta]
    }
}

impl DynamicalSystem for GeodesicTorus {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 4 }
    fn name(&self) -> &str { "Geodesic Torus" }
    fn speed(&self) -> f64 { self.speed }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> { Self::deriv(state, self.big_r, self.small_r) }

    fn step(&mut self, dt: f64) {
        let (big_r, small_r) = (self.big_r, self.small_r);
        let prev = self.state.clone();
        super::rk4(&mut self.state, dt, |s| Self::deriv(s, big_r, small_r));
        // Wrap angles to [0, 2π)
        self.state[0] = self.state[0].rem_euclid(std::f64::consts::TAU);
        self.state[1] = self.state[1].rem_euclid(std::f64::consts::TAU);
        let ds: f64 = self.state.iter().zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
        self.speed = ds / dt;
    }
}
