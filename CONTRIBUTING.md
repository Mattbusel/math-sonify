# Contributing to math-sonify

Thank you for your interest in contributing. This document covers the most common extension points.

---

## Code structure overview

```
src/
├── main.rs              # Entry point: thread setup, sim loop, build_system()
├── config.rs            # Config struct (TOML-backed) — one sub-struct per system
├── patches.rs           # Named presets (PRESETS constant)
├── audio.rs             # Audio thread, DSP engine, shared type aliases
├── ui.rs                # egui UI, AppState
├── arrangement.rs       # Scene/arrangement interpolation
├── presets.rs           # Preset application helpers
├── lib.rs               # core library (shared by app and plugin)
├── systems/             # Dynamical systems
│   ├── mod.rs           # DynamicalSystem trait, rk4/rk45 helpers, re-exports
│   ├── lorenz.rs
│   └── ...              # One file per system
├── sonification/        # State → AudioParams mappers
│   ├── mod.rs           # AudioParams, SonifMode, Sonification trait
│   ├── direct.rs
│   └── ...              # One file per mode
└── synth/               # DSP building blocks
    ├── mod.rs
    ├── oscillator.rs
    └── ...              # One file per module
```

---

## How to add a new dynamical system

### 1. Implement `DynamicalSystem`

Create `src/systems/my_system.rs`:

```rust
use crate::systems::{DynamicalSystem, rk4};

pub struct MySystem {
    state: Vec<f64>,
    // system parameters
    pub param_a: f64,
}

impl MySystem {
    pub fn new(param_a: f64) -> Self {
        Self { state: vec![1.0, 0.0, 0.0], param_a }
    }

    fn deriv(state: &[f64], param_a: f64) -> Vec<f64> {
        // dx/dt = ...
        vec![/* ... */]
    }
}

impl DynamicalSystem for MySystem {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 3 }
    fn name(&self) -> &str { "my_system" }

    fn step(&mut self, dt: f64) {
        let a = self.param_a;
        rk4(&mut self.state, dt, |s| Self::deriv(s, a));
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.param_a)
    }
}
```

Use `rk4` for dissipative systems and `leapfrog`/`yoshida4` for Hamiltonian ones. Override `speed()` if the granular sonification mode should respond to trajectory velocity. Override `energy_error()` for conservative systems that should display a conservation error.

### 2. Register in `systems/mod.rs`

```rust
pub mod my_system;
pub use my_system::MySystem;
```

### 3. Add a Config struct in `config.rs`

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct MySystemConfig {
    pub param_a: f64,
}
impl Default for MySystemConfig {
    fn default() -> Self { Self { param_a: 1.0 } }
}
```

Add the field to the top-level `Config` struct and its `Default` impl, following the pattern of every existing system (e.g. `lorenz: LorenzConfig`).

### 4. Add to `build_system()` in `main.rs`

Find the `build_system` function and add a match arm:

```rust
"my_system" => Box::new(MySystem::new(config.my_system.param_a)),
```

### 5. Add a preset in `patches.rs`

```rust
Preset {
    name: "My Preset Name",
    description: "One evocative sentence about what this sounds like.",
    color: Color32::from_rgb(100, 200, 80),
    category: "Experimental",
},
```

Presets are display-only metadata. Patch application logic (mapping a preset name to concrete Config values) lives in `presets.rs` — add a corresponding arm there if you want the preset to change parameters when selected.

---

## How to add a new sonification mode

### 1. Create `src/sonification/my_mode.rs`

```rust
use crate::sonification::{AudioParams, Sonification};
use crate::config::SonificationConfig;

pub struct MyMapping { /* internal state */ }

impl MyMapping {
    pub fn new() -> Self { Self {} }
}

impl Sonification for MyMapping {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let mut p = AudioParams::default();
        p.mode = crate::sonification::SonifMode::MyMode;
        // populate p.freqs, p.amps, etc. from state
        p
    }
}
```

### 2. Add variant to `SonifMode` in `sonification/mod.rs`

```rust
pub enum SonifMode { Direct, Orbital, Granular, Spectral, FM, Vocal, Waveguide, MyMode }
```

Update the `Display` impl and the `From<&str>` impl in `config.rs`.

### 3. Register in `sonification/mod.rs`

```rust
pub mod my_mode;
pub use my_mode::MyMapping;
```

### 4. Wire into the sim thread in `main.rs`

In `sim_thread`, find where `DirectMapping`, `OrbitalResonance`, etc. are constructed and add your mode. Also add a rendering branch in `audio.rs` where the audio thread switches on `params.mode`.

---

## How to add a new synth module

Create `src/synth/my_module.rs` with a plain struct that takes `sample_rate: f32` in its constructor and exposes a `process(&mut self, input: f32) -> f32` (or stereo equivalent).

Register in `synth/mod.rs`:

```rust
pub mod my_module;
pub use my_module::MyModule;
```

Instantiate in `LayerSynth` inside `audio.rs` and call it in the audio callback. Expose control parameters through `AudioParams` fields (add them to the struct and its `Default` impl in `sonification/mod.rs`).

---

## Build instructions

### Standalone application

```sh
cargo build --release
```

The binary is at `target/release/math-sonify[.exe]`. Copy alongside `config.toml` for distribution.

### VST3 / CLAP plugin

```sh
cargo build --release -p math-sonify-plugin
```

The compiled `.dll` / `.so` / `.dylib` is the plugin binary. Use [cargo-nih-plug](https://github.com/robbert-vdh/nih-plug) or nih-plug's own bundler for proper VST3/CLAP bundle layout:

```sh
cargo xtask bundle math_sonify --release
```

### Development build

```sh
cargo build          # opt-level 1 for faster compile
cargo run            # runs the standalone app
```

### Lints and tests

```sh
cargo clippy --all-targets
cargo test
```

---

## Conventions

- Keep each dynamical system in its own file; do not add system logic to `main.rs`.
- `AudioParams` is `Clone` and crosses a bounded crossbeam channel from sim to audio — keep it `Send` and allocation-free (no heap fields).
- UI mutations go through `AppState` under a `parking_lot::Mutex`; use `try_lock` in hot paths.
- Prefer `f64` for simulation state and `f32` for audio.
