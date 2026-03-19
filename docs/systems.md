# Dynamical Systems Reference

All systems implement the `DynamicalSystem` trait and are integrated by the simulation thread at 120 Hz. Each system exposes a `state()` slice, a `step(dt)` method, and metadata such as `speed()` and `name()`.

---

## Integration Methods

| Method | Used By | Notes |
|--------|---------|-------|
| RK4 (4th-order Runge-Kutta) | Most continuous systems | Generic accuracy/cost tradeoff |
| Yoshida 4th-order symplectic | Double Pendulum | Exact energy conservation |
| Velocity Verlet (leapfrog) | Three-Body | Symplectic, Hamiltonian tracking |
| Adams-Bashforth 2 + Adams-Moulton 2 | Mackey-Glass | Predictor-corrector for DDEs |
| Grünwald-Letnikov | Fractional Lorenz | Fractional derivative approximation |
| Discrete map | Henon Map, Coupled Map Lattice | No ODE; one call = one iteration |

---

## Systems

### Lorenz Attractor
**File:** `src/systems/lorenz.rs`
**Dimension:** 3

```
dx/dt = σ(y − x)
dy/dt = x(ρ − z) − y
dz/dt = xy − βz
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `sigma` (σ) | 10.0 | 0.1–100 | Prandtl number; controls spiral rate |
| `rho` (ρ) | 28.0 | 0.1–200 | Rayleigh number; chaos onset ≈ 24.74 |
| `beta` (β) | 8/3 ≈ 2.667 | 0.01–20 | Geometric dissipation factor |

**Initial state:** (1, 0, 0)
**Character:** Classic butterfly attractor; smooth spirals interrupted by chaotic wing-switching. The archetypical strange attractor.

---

### Rössler Attractor
**File:** `src/systems/rossler.rs`
**Dimension:** 3

```
dx/dt = −y − z
dy/dt = x + ay
dz/dt = b + z(x − c)
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `a` | 0.2 | 0–20 | y-feedback; chaos onset ≈ 0.398 |
| `b` | 0.2 | 0–20 | Additive constant |
| `c` | 5.7 | 0–20 | Shifts z-nullcline; canonical chaos at 5.7 |

**Initial state:** (1, 0, 0)
**Character:** Spiral attractor with period-doubling route to chaos. Simpler structure than Lorenz; frequently used for musical pitch mapping.

---

### Double Pendulum
**File:** `src/systems/double_pendulum.rs`
**Dimension:** 4 — [θ₁, θ₂, p₁, p₂] (angles + conjugate momenta)

Hamiltonian mechanics; equations are the canonical Hamilton equations for a planar double pendulum under gravity.

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `m1` | 1.0 | 0.01–100 | Mass of first bob (kg) |
| `m2` | 1.0 | 0.01–100 | Mass of second bob (kg) |
| `l1` | 1.0 | 0.01–100 | Length of first arm (m) |
| `l2` | 1.0 | 0.01–100 | Length of second arm (m) |

**Initial state:** θ₁ = θ₂ = π/2, momenta = 0
**Integration:** Yoshida 4th-order symplectic — energy is conserved to O(dt²) per step.
**Character:** Sudden, unpredictable flips; energy-conserving trajectory through complex angular space.

---

### Geodesic Torus
**File:** `src/systems/geodesic_torus.rs`
**Dimension:** 4 — [φ, θ, φ̇, θ̇]

Geodesic flow on the surface of a torus with metric ds² = (R + r·cos θ)²dφ² + r²dθ².

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `big_r` (R) | 3.0 | 0.1–100 | Distance from tube center to torus center |
| `small_r` (r) | 1.0 | 0.01–50 | Tube radius |

**Initial velocity:** Golden ratio (φ̇ ≈ 1.0, θ̇ ≈ 1/φ) for ergodic, non-repeating flow.
**Character:** Quasi-periodic winding. Not chaotic but richly patterned; produces complex rhythmic phasing.

---

### Kuramoto Model
**File:** `src/systems/kuramoto.rs`
**Dimension:** N (default 8)

```
dθᵢ/dt = ωᵢ + (K/N) · Σⱼ sin(θⱼ − θᵢ)
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `n_oscillators` | 8 | 2–256 | Number of coupled phase oscillators |
| `coupling` (K) | 1.5 | 0–50 | Coupling strength |

**Natural frequencies:** Lorentzian distribution (center 1.0, half-width 0.5).
**Phase transition:** Synchronization onset at Kc = 1.0.
**Observable:** Order parameter r = |Σ exp(iθⱼ)|/N ∈ [0, 1] — measures global coherence.
**Character:** Transitions from incoherence to collective synchrony as K crosses Kc.

---

### Three-Body Problem
**File:** `src/systems/three_body.rs`
**Dimension:** 12 — [x₁, y₁, x₂, y₂, x₃, y₃, vx₁, vy₁, vx₂, vy₂, vx₃, vy₃]

```
d²rᵢ/dt² = G · Σⱼ≠ᵢ mⱼ · (rⱼ − rᵢ) / |rⱼ − rᵢ|³
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `masses` | [1, 1, 1] | 0.01–1000 each | Gravitational masses |

**Default IC:** Figure-8 periodic orbit (Chenciner & Montgomery 2000).
**Softening:** 1e-3 floor prevents force singularity at close encounters.
**Integration:** Velocity Verlet (leapfrog) — symplectic; Hamiltonian error is tracked.
**Character:** Sensitive to initial conditions; sudden near-collisions produce dramatic high-speed bursts.

---

### Hindmarsh-Rose Neuron
**File:** `src/systems/hindmarsh_rose.rs`
**Dimension:** 3

```
dx/dt = y − ax³ + bx² + I − z
dy/dt = c − dx² − y
dz/dt = r(s(x − x_rest) − z)
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `current_i` (I) | 3.0 | −5–10 | External drive current; primary control |
| `r` | 0.006 | 1e-6–1.0 | Slow adaptation timescale |

Fixed: a=1, b=3, c=1, d=5, s=4, x_rest=−1.6 (canonical bursting parameters).
**State clamped:** x ∈ [−5, 5], y ∈ [−20, 20], z ∈ [−5, 5] to prevent divergence.
**Character:** Chaotic bursting; rapid spiking interrupted by quiet hyperpolarized periods. I > 3.5 produces complex bursting.

---

### Van der Pol Oscillator
**File:** `src/systems/van_der_pol.rs`
**Dimension:** 2

```
dx/dt = y
dy/dt = μ(1 − x²)y − x
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `mu` (μ) | 2.0 | 0–100 | Nonlinearity; large μ → relaxation oscillator |

**Character:** Stable limit cycle; not chaotic but useful for rhythmic patterns. Larger μ gives slow charge/fast discharge shape.

---

### Duffing Oscillator
**File:** `src/systems/duffing.rs`
**Dimension:** 3 — includes driving phase φ

```
dx/dt = v
dv/dt = −δv − αx − βx³ + γ cos(φ)
dφ/dt = ω
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `delta` (δ) | 0.3 | 0–10 | Damping coefficient |
| `alpha` (α) | −1.0 | −10–10 | Linear stiffness (α < 0 → double-well) |
| `beta` (β) | 1.0 | −10–10 | Nonlinear stiffness |
| `gamma` (γ) | 0.5 | 0–10 | Driving amplitude |
| `omega` (ω) | 1.2 | 0.001–100 | Driving frequency |

**Initial state:** (1, 0, 0) — starts in right potential well.
**Character:** Fractal basin boundaries; chaotic orbit switching between potential wells.

---

### Halvorsen Attractor
**File:** `src/systems/halvorsen.rs`
**Dimension:** 3

```
dx/dt = −ax − 4y − 4z − y²
dy/dt = −ay − 4z − 4x − z²
dz/dt = −az − 4x − 4y − x²
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `a` | 1.89 | 0–10 | Coupling parameter |

**Symmetry:** Cyclic under (x, y, z) → (y, z, x).
**Initial state:** (−5, 0, 0)
**Character:** Densely folded symmetric attractor; complex layered structure in all three planes.

---

### Aizawa Attractor
**File:** `src/systems/aizawa.rs`
**Dimension:** 3

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `a` | 0.95 | 0–5 | |
| `b` | 0.70 | 0–5 | |
| `c` | 0.60 | 0–5 | |
| `d` | 3.50 | 0–10 | |
| `e` | 0.25 | 0–5 | |
| `f` | 0.10 | 0–5 | |

**Character:** Toroidal surface with a slow polar wobble; delicate structure that is sensitive to all parameters.

---

### Chua's Circuit
**File:** `src/systems/chua.rs`
**Dimension:** 3

```
dx/dt = α(y − h(x))     where h(x) = m₁x + 0.5(m₀ − m₁)(|x+1| − |x−1|)
dy/dt = x − y + z
dz/dt = −βy
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `alpha` (α) | 15.6 | 0–100 | Feedback factor |
| `beta` (β) | 28.0 | 0–100 | Secondary parameter |
| `m0` | −1.143 | any | Outer slope of piecewise-linear resistor |
| `m1` | −0.714 | any | Inner slope |

**Character:** Double-scroll attractor; the canonical chaos demonstration circuit. Rich folded structure in x-y projection.

---

### Coupled Map Lattice
**File:** `src/systems/coupled_map_lattice.rs`
**Dimension:** 16 (sites), periodic boundary

```
xᵢ(t+1) = (1−ε)·f(xᵢ) + (ε/2)·(f(xᵢ₋₁) + f(xᵢ₊₁))
f(x) = r·x·(1−x)   (logistic map)
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `r` | 3.9 | 0–4 | Logistic growth (≥ 3.7 for chaos) |
| `eps` | 0.35 | 0–1 | Coupling (0 = independent, 1 = synchrony) |

**Character:** Spatially extended chaos. Site index maps to stereo position so interference patterns travel left-to-right. Ghostly, flickering texture.

---

### Mackey-Glass Delay DDE
**File:** `src/systems/mackey_glass.rs`
**Dimension:** 3 — [x(t), x(t−τ/3), x(t−2τ/3)]

```
dx/dt = β·x(t−τ) / (1 + x(t−τ)ⁿ) − γ·x
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `beta` (β) | 0.2 | 0–10 | Production rate |
| `gamma` (γ) | 0.1 | 0–10 | Decay rate |
| `tau` (τ) | 17.0 | 1–300 | Time delay — τ < 7 → limit cycle; τ > 17 → complex chaos |
| `n` | 10.0 | 1–20 | Nonlinearity exponent |

**History:** Ring buffer (buf_len ≈ τ/dt + 1) with Adams-Bashforth 2 / Adams-Moulton 2 corrector.
**Character:** Slow, smooth undulations punctuated by occasional sharp excursions. Feels almost biological.

---

### Nose-Hoover Thermostat
**File:** `src/systems/nose_hoover.rs`
**Dimension:** 3

```
dx/dt = y
dy/dt = −x + yz
dz/dt = a − y²
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `a` | 3.0 | 0.1–20 | Coupling to thermal reservoir |

**Initial state:** (0, 5, 0)
**Character:** Conservative chaos; no attractor but bounded ergodic motion on an energy shell.

---

### Sprott B
**File:** `src/systems/sprott_b.rs`
**Dimension:** 3

```
dx/dt = yz
dy/dt = x − y
dz/dt = 1 − xy
```

No free parameters.
**Initial state:** (0, 3, 0)
**Fractal dimension:** ≈ 2.04
**Character:** Minimal chaotic flow — only three terms, two of them quadratic. Clean, hypnotic orbit.

---

### Henon Map
**File:** `src/systems/henon_map.rs`
**Dimension:** 2 (+ dummy z for API compatibility)

```
xₙ₊₁ = 1 − a·xₙ² + yₙ
yₙ₊₁ = b·xₙ
```

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `a` | 1.4 | 0–2 | Nonlinearity; classical chaos at 1.4 |
| `b` | 0.3 | −1–1 | Area contraction |

**Attractor dimension:** ≈ 1.26
**Character:** Discrete map; fractal horseshoe structure. Sparse, clicking rhythmic texture.

---

### Lorenz 96
**File:** `src/systems/lorenz96.rs`
**Dimension:** Variable N (default 8), periodic boundary

```
dxᵢ/dt = (xᵢ₊₁ − xᵢ₋₂)·xᵢ₋₁ − xᵢ + Fᵢ
```

Supports homogeneous forcing (all Fᵢ = F) or heterogeneous (Fᵢ = f_mean + f_spread·sin(2πi/N)).

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `f` (F) | 8.0 | 0–50 | External forcing (F ≥ 8 → chaos) |

**Character:** Atmospheric weather-prediction model; collective wave-like dynamics propagating around the lattice ring.

---

### Fractional-Order Lorenz
**File:** `src/systems/fractional_lorenz.rs`
**Dimension:** 3

Grünwald-Letnikov fractional derivative approximation with memory length 64.
Equations are the Lorenz system with derivatives replaced by fractional-order Dᵅ.

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `alpha` | 1.0 | 0.7–1.5 | Fractional order (1.0 = classical Lorenz) |
| `sigma`, `rho`, `beta` | as Lorenz | as Lorenz | |

**Character:** At α < 1 the system has long-term memory; slower, smoother trajectory that retains influence of the distant past.
