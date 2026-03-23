# math-sonify

[![CI](https://github.com/Mattbusel/math-sonify/actions/workflows/ci.yml/badge.svg)](https://github.com/Mattbusel/math-sonify/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

**Math sonification** is the practice of mapping the evolving state of a mathematical system directly to audio synthesis parameters so that the structure of the mathematics becomes audible. math-sonify runs differential equations continuously in real time and routes every variable of their state vector into oscillator frequencies, grain densities, FM modulation indices, formant positions, or waveguide string parameters. The result is not a preset synthesizer with math-themed names: the Lorenz attractor is actually integrating, the Kuramoto coupling constant is live, the Three-Body gravitational problem is stepped forward at 120 Hz.

---

## What is math sonification?

Classical sonification maps data to sound after the fact (load a CSV, play it back). Math sonification is generative and continuous: the sound is the running computation. There is no playback cursor; the audio is produced by the physics as it happens.

This makes the technique useful for:
- Auditory exploration of dynamical system behavior (period doubling, chaos onset, synchronization).
- Generative music where the mathematical constraints of the attractor act as a compositional structure.
- Live performance: parameter changes propagate to audio within one control-rate frame (8 ms at 120 Hz).

---

## Download

**Windows pre-built binary:** download `math-sonify.exe` from the [latest GitHub release](https://github.com/Mattbusel/math-sonify/releases/latest). No install required — double-click and audio starts immediately.

---

## Architecture

```
ODE Solver (120 Hz, sim thread)
    |
    |  53 dynamical systems — Lorenz, Rossler, Duffing, Kuramoto, Three-Body,
    |  Hyperchaos (4D), Finance, WINDMI, Liu, Genesio-Tesi, Shimizu-Morioka, ...
    |  RK4 integration per configured dt
    |
    v
Parameter Morphing (arrangement layer)
    |
    |  Scene arranger linearly interpolates all numeric config fields
    |  between named snapshots; string fields switch at midpoint
    |
    v
Sonification Mapper (sim thread, 120 Hz)
    |
    |  DirectMapping    -- state quantized to musical scale -> oscillator freqs
    |  OrbitalResonance -- angular velocity + Lyapunov exponent drive pitch
    |  GranularMapping  -- trajectory speed -> grain density and pitch
    |  SpectralMapping  -- state -> 32-partial additive envelope
    |  FmMapping        -- attractor drives carrier/modulator ratio and index
    |  AmMapping        -- amplitude modulation driven by state variables
    |  VocalMapping     -- state interpolates between vowel formant positions
    |  Waveguide        -- Karplus-Strong string with chaotic modulation
    |  Resonator        -- modal resonator bank driven by attractor
    |
    v  [crossbeam bounded channel, try_recv in audio callback]
    v
Audio Synthesis (audio thread, 44100 / 48000 Hz)
    |
    |  Per-layer DSP:
    |    Oscillator(s) [PolyBLEP anti-aliased] --> ADSR --> Waveshaper --> Bitcrusher
    |
    |  Master bus (shared across up to 4 layers):
    |    3-Band EQ --> LP BiquadFilter --> Stereo DelayLine --> Chorus
    |    --> FDN Reverb (8-channel, modulated) --> Lookahead Limiter
    |
    v
DAW (VST3 / CLAP plugin) or Desktop (standalone cpal output)
```

**Thread safety:** the sim thread and audio thread communicate through a bounded `crossbeam-channel` of capacity 16. The audio callback calls `try_recv` and renders silence on a miss, so it is never blocked. The UI thread reads shared state through `parking_lot::Mutex` on the control rate.

---

## Supported mathematical systems (53)

| System | Dim | Type | Notes |
|--------|-----|------|-------|
| Lorenz | 3 | chaos | Classic butterfly attractor; chaos onset near rho=24.74 |
| Rossler | 3 | chaos | Spiral attractor; period-doubling as c increases |
| Double Pendulum | 4 | chaos | Lagrangian mechanics (θ1, θ2, p1, p2); leapfrog integrator |
| Geodesic Torus | 4 | quasi-periodic | Ergodic irrational winding on a flat torus |
| Kuramoto | N | sync | N coupled oscillators; synchronization at critical K |
| Three-Body | 12 | chaos | Newtonian gravity, 3 point masses in 2D; figure-8 ICs |
| Duffing | 2 | chaos | Driven nonlinear oscillator; period-doubling cascade |
| Van der Pol | 2 | limit cycle | Self-sustaining limit cycle; relaxation oscillations |
| Halvorsen | 3 | chaos | Dense cyclic-symmetry spiral attractor |
| Aizawa | 3 | chaos | Six-parameter torus-like attractor |
| Chua | 3 | chaos | Piecewise-linear double-scroll circuit |
| Hindmarsh-Rose | 3 | chaos | Neuron firing model; bursting and spiking |
| Lorenz-96 | N | chaos | Weather model; spatiotemporal chaos at F > 8 |
| Mackey-Glass | DDE | chaos | Delay differential equation; history-dependent |
| Nose-Hoover | 3 | chaos | Thermostatted Hamiltonian; conservative chaos |
| Coupled Map Lattice | N | chaos | Logistic map on a 1D lattice with diffusive coupling |
| Henon Map | 2 | chaos | Discrete map; fractal strange attractor (dim ≈ 1.26) |
| Custom ODE | 3–4 | user | User-defined equations via text input |
| Fractional Lorenz | 3 | chaos | Lorenz with derivative order alpha in (0.5, 1.0] |
| Logistic Map | 1 | chaos | Period-doubling route to chaos; bifurcation diagram classic |
| Standard Map | 2 | chaos | Area-preserving Chirikov map; KAM tori to global chaos |
| Arnold Cat | 2 | chaos | Ergodic linear torus map; hyperbolic fixed point |
| Stochastic Lorenz | 3 | chaos | Lorenz with additive Wiener noise per axis |
| Delayed Map | 1 | chaos | Logistic map with discrete delay tau |
| Oregonator | 3 | oscillation | Belousov-Zhabotinsky chemical reaction oscillator |
| Mathieu | 2 | parametric | Parametric resonance; stability tongues in a/q space |
| Kuramoto-Driven | N | sync | Kuramoto + external sinusoidal drive on first oscillator |
| Thomas | 3 | chaos | Conservative symmetric attractor; b≈0.208 chaos boundary |
| Lorenz-84 | 3 | chaos | Low-order atmospheric circulation model |
| Dadras | 3 | chaos | Five-parameter attractor with rich bifurcation structure |
| Rucklidge | 3 | chaos | Double-scroll from a convection model |
| Chen | 3 | chaos | Lorenz-family; denser scroll than standard Lorenz |
| Burke-Shaw | 3 | chaos | Two-scroll; sigma/rho parameterization |
| Rabinovich-Fabrikant | 3 | chaos | Plasma wave instability model |
| Rikitake | 3 | chaos | Two coupled dynamos; geomagnetic reversal model |
| Bouali | 3 | chaos | Slow-manifold attractor; a/s parameterization |
| Newton-Leipnik | 3 | chaos | Two coupled rigid bodies; two coexisting attractors |
| Sprott B | 3 | chaos | Minimal 5-term polynomial system |
| Sprott C | 3 | chaos | Minimal polynomial; single quadratic term |
| Sprott D (Case I) | 3 | chaos | y² instability with −1.1z dissipation |
| Sprott E | 3 | chaos | Minimal chaos from a yz product |
| Sprott F | 3 | chaos | Slow-spiral; x² drives z |
| Sprott G | 3 | chaos | Linear + quadratic; minimal form |
| Sprott H | 3 | chaos | Single xz product nonlinearity |
| Sprott K | 3 | chaos | xy product; one of Sprott's simplest forms |
| Sprott L | 3 | chaos | Bounded strange attractor; yz coupling |
| Shimizu-Morioka | 3 | chaos | Two-scroll; x²-driven z destabilizes y |
| Genesio-Tesi | 3 | chaos | Jerk circuit: one x² term is all the chaos needed |
| Liu | 3 | chaos | Single-band scroll; y² and xz/xy cross-coupling |
| WINDMI | 3 | chaos | Ionospheric substorm model; exponential nonlinearity |
| Finance | 3 | chaos | Macroeconomic chaos: interest rate, investment, price |
| Hyperchaos (Chen-Li) | 4 | hyperchaos | Two positive Lyapunov exponents; richer than ordinary chaos |

---

## Sonification modes (9)

| Mode | How math maps to audio |
|------|------------------------|
| Direct | State variables quantized to configured scale → oscillator frequencies. Amplitude tracks normalized magnitude. |
| Orbital | State interpreted as polar coordinates. Angular velocity drives pitch; Lyapunov exponent modulates inharmonicity. |
| Granular | Trajectory speed controls grain spawn rate (0–50 grains/sec). Position in state space sets grain frequency. |
| Spectral | 32 additive partials. Each partial amplitude derived from a normalized component of the state vector. |
| FM | Two-operator FM synthesis. Carrier tracks first state variable; modulator ratio and index driven by remaining variables. |
| AM | Amplitude modulation. Carrier frequency from state; AM depth and rate driven by trajectory speed. |
| Vocal | State coordinates mapped to vowel formant positions (F1/F2). Trajectory wanders through /a/ /e/ /i/ /o/ /u/. |
| Waveguide | Karplus-Strong string model. Tension and damping modulated by the attractor in real time. |
| Resonator | Modal resonator bank. Attractor state excites a set of tuned resonant modes. |

---

## Musical scales (20)

| Scale | Intervals (semitones) |
|-------|-----------------------|
| Pentatonic | 0, 2, 4, 7, 9 |
| Natural Minor (Aeolian) | 0, 2, 3, 5, 7, 8, 10 |
| Harmonic Minor | 0, 2, 3, 5, 7, 8, 11 |
| Dorian | 0, 2, 3, 5, 7, 9, 10 |
| Phrygian | 0, 1, 3, 5, 7, 8, 10 |
| Lydian | 0, 2, 4, 6, 7, 9, 11 |
| Mixolydian | 0, 2, 4, 5, 7, 9, 10 |
| Locrian | 0, 1, 3, 5, 6, 8, 10 |
| Whole Tone | 0, 2, 4, 6, 8, 10 |
| Blues | 0, 3, 5, 6, 7, 10 |
| Hirajoshi | 0, 2, 3, 7, 8 |
| Hungarian Minor | 0, 2, 3, 6, 7, 8, 11 |
| Octatonic (dim.) | 0, 2, 3, 5, 6, 8, 9, 11 |
| Chromatic | all 12 semitones |
| Just Intonation | pure-ratio tuning |
| Microtonal | 24 equal divisions per octave |
| EDO-19 | 19 equal divisions of the octave |
| EDO-31 | 31 equal divisions of the octave |
| EDO-24 | 24-TET (quarter-tones) |
| Harmonic Series | 16 partials of a fundamental |

---

## Presets

math-sonify ships with ~40 named presets organized into four moods:

- **Atmospheric** — Midnight Approach, Breathing Galaxy, Aurora Borealis, Deep Hypnosis, Cathedral Organ, Substorm, and more.
- **Rhythmic** — Frozen Machinery, The Phase Transition, Clockwork Insect, Industrial Heartbeat, Velocity Band, and more.
- **Experimental** — Neon Labyrinth, Dissociation, Jerk Circuit, Invisible Hand, Hyperchaos Engine, and more.
- **Melodic** — Glass Harp, Electric Kelp, The Butterfly's Aria, Solar Wind, and more.

The **AUTO** arrangement generator picks 6 presets from a mood pool, scatters system parameters into varied dynamical regimes, randomizes synthesis settings, and builds an 8-scene timeline with morphs as the main musical event.

---

## Audio output

math-sonify outputs **32-bit IEEE float stereo PCM** at the system default sample rate (44100 or 48000 Hz).

| Export method | Details |
|---------------|---------|
| Clip save (`S`) | Last 60 seconds → 32-bit float WAV in `clips/` |
| Loop export | Current loop region → WAV |
| Headless render | `--headless --duration 60 --output clip.wav` — no display required |

---

## Installation

### Pre-built binary (Windows)

Download `math-sonify.exe` from the [latest release](https://github.com/Mattbusel/math-sonify/releases/latest) and run it. No dependencies, no install.

### From source

Requires [Rust](https://rustup.rs/) 1.75+ and a working audio output device.

```bash
git clone https://github.com/Mattbusel/math-sonify
cd math-sonify
cargo run --release
```

### Build the VST3 / CLAP plugin

```bash
cargo build --release --lib
```

Copy the output to your DAW plugin folder:

| Platform | File | Destination |
|----------|------|-------------|
| Windows  | `math_sonify_plugin.dll` | `C:\Program Files\Common Files\VST3\` |
| Linux    | `libmath_sonify_plugin.so` | `~/.vst3/` |
| macOS    | `libmath_sonify_plugin.dylib` | `~/Library/Audio/Plug-Ins/VST3/` |

---

## Getting started

Audio starts immediately on launch using the system default output device. The Lorenz attractor runs in Direct mode with a pentatonic scale.

The GUI has four panels:

- **Attractor** — system selector, parameter sliders, Lyapunov display, and phase-portrait visualisation.
- **Sound** — sonification mode picker, scale selector, master volume, reverb/delay/EQ controls.
- **Timeline** — arrangement editor with named scenes and morph durations.
- **Evolution** — genetic algorithm engine; select a fitness metric, configure the population, press Run, and watch parameters evolve toward musically richer output.

### Headless WAV export

```bash
cargo run --release -- --headless --duration 60 --output clip.wav
```

### MIDI export

The MIDI recorder runs alongside normal audio output.  Configure it in `config.toml`:

```toml
[midi_export]
# Duration to capture in seconds (1–600)
duration_secs = 30
# BPM for the output file
tempo_bpm = 120
# Musical scale applied to quantize pitches
scale = "pentatonic"
# Root note (MIDI number; 57 = A3 = 440 Hz root)
scale_root = 57
# Output path (relative to CWD)
output_path = "export.mid"
```

Or export from the GUI: **File → Export MIDI…** opens a dialog where you set the duration and press Export.  The file is written to `clips/` by default.

### Parameter evolution example

Evolving Lorenz parameters to maximise harmonic richness:

```bash
# In the GUI: Evolution panel → Metric: Harmonic Richness
#             Population: 40  Generations: 50
#             Click "Run" — the sparkline updates every generation.
#             When finished, click "Save as Preset" to name and store the result.
```

From code:

```rust
use math_sonify_plugin::evolution::{
    EvolutionConfig, FitnessMetric, ParameterEvolution, ParamBounds,
};
use std::sync::Arc;
use parking_lot::Mutex;

let bounds = vec![
    ParamBounds::new(5.0, 15.0),   // sigma
    ParamBounds::new(20.0, 35.0),  // rho
    ParamBounds::new(1.0, 4.0),    // beta
];
let state = Arc::new(Mutex::new(Default::default()));
let mut evo = ParameterEvolution::new(
    EvolutionConfig { population_size: 40, generations: 50, ..Default::default() },
    FitnessMetric::HarmonicRichness,
    bounds,
    Arc::clone(&state),
);

// ode_step advances state by eval_dt using RK4.
let saved = evo.run("My Evolved Lorenz", |state, params| {
    let dt = 0.005;
    let dx = params[0] * (state[1] - state[0]);
    let dy = state[0] * (params[1] - state[2]) - state[1];
    let dz = state[0] * state[1] - params[2] * state[2];
    state[0] += dx * dt;
    state[1] += dy * dt;
    state[2] += dz * dt;
});

println!("Best fitness: {:.4}", saved.fitness);
println!("Best params: {:?}", saved.params);
// Write to presets file:
std::fs::write("evolved.toml", saved.to_toml_snippet()).unwrap();
```

---

## Configuration

The application reads `config.toml` from the current working directory at startup. The file is watched with `notify`; edits take effect without restarting.

```toml
[system]
name  = "lorenz"   # see full system list above
dt    = 0.001      # ODE integration time step (clamped 0.0001..0.1)
speed = 1.0        # simulation speed multiplier (0..100)

[lorenz]
sigma = 10.0
rho   = 28.0
beta  = 2.6667

[rossler]
a = 0.2
b = 0.2
c = 5.7

[hyperchaos]
a = 35.0
b = 3.0
c = 28.0
d = -7.0     # must be negative

[windmi]
a = 0.9
b = 2.5

[finance]
a = 3.0
b = 0.1
c = 1.0

[kuramoto]
n_oscillators = 8
coupling      = 1.5

[duffing]
delta = 0.3
alpha = -1.0
beta  = 1.0
gamma = 0.5
omega = 1.2

[van_der_pol]
mu = 2.0

[audio]
sample_rate      = 44100
buffer_size      = 512
reverb_wet       = 0.4
delay_ms         = 300.0
delay_feedback   = 0.3
master_volume    = 0.7
bit_depth        = 16.0    # 1..32 (32 = bypass bitcrusher)
rate_crush       = 0.0     # 0..1 (0 = bypass)
chorus_mix       = 0.0
chorus_rate      = 0.5     # Hz
chorus_depth     = 3.0     # ms
waveshaper_drive = 1.0
waveshaper_mix   = 0.0

[sonification]
mode                = "direct"
# Modes: direct | orbital | granular | spectral | fm | am | vocal | waveguide | resonator
scale               = "pentatonic"
# Scales: pentatonic | natural_minor | harmonic_minor | dorian | phrygian | lydian
#         | mixolydian | locrian | whole_tone | blues | hirajoshi | hungarian_minor
#         | octatonic | chromatic | just_intonation | microtonal | edo19 | edo31 | edo24
#         | harmonic_series
base_frequency      = 220.0
octave_range        = 3.0
transpose_semitones = 0.0
chord_mode          = "none"
# Chord modes: none | major | minor | power | sus2 | octave | dom7 | open_fifth | cluster
portamento_ms       = 80.0
voice_levels        = [1.0, 0.8, 0.6, 0.4]
voice_shapes        = ["sine", "sine", "sine", "sine"]
# Shapes: sine | saw | square | triangle | noise

[viz]
trail_length = 800
projection   = "xy"   # xy | xz | yz | 3d
glow         = true
theme        = "neon" # neon | amber | ice | mono
```

---

## GUI

Five top-level tabs:

- **SYNTH** — system selector, parameter sliders, sonification mode, scale, effects chain, randomize.
- **MIXER** — per-layer volume/pan/ADSR, master effects (EQ, delay, chorus, reverb), VU meters, WAV export.
- **ARRANGE** — scene timeline, morph time controls, AUTO arrangement generator with mood selection.
- **MATH VIEW** — live phase portrait (XY/XZ/YZ/3D), bifurcation diagram, custom ODE text input, state readout.
- **WAVEFORM** — oscilloscope and spectrum analyzer.

Performance mode (`F`) switches to fullscreen phase portrait only.

---

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| `F` | Toggle fullscreen performance mode |
| `Space` | Pause / resume simulation |
| `R` | Reset attractor to default initial condition |
| `S` | Save clip (last 60 seconds as WAV) |
| `Ctrl+S` | Save current configuration to `config.toml` |
| `1` – `7` | Switch sonification mode |
| `<` / `>` | Previous / next dynamical system |
| `Up` / `Down` | Increase / decrease simulation speed by 10% |
| `E` | Toggle Evolve (autonomous parameter wandering) |
| `A` | Toggle AUTO arrangement playback |
| `P` | Play / stop scene arranger |
| `Escape` | Exit fullscreen |

---

## Technical architecture

### Threading model

```
Main thread (GUI, 60 Hz)
  └─ egui / eframe render loop
  └─ reads SharedState via parking_lot::Mutex

Simulation thread (120 Hz)
  └─ ODE RK4 integration
  └─ Evolution engine (ParameterEvolution) — when running
  └─ Sonification mapping → AudioParams
  └─ sends AudioParams via crossbeam bounded channel (cap 16)

Audio thread (cpal callback, 44100 / 48000 Hz)
  └─ try_recv AudioParams (renders silence on miss — never blocks)
  └─ DSP chain: Oscillator → ADSR → Waveshaper → Bitcrusher
  └─ Master bus: EQ → BiquadFilter → Delay → Chorus → Reverb → Limiter
  └─ MIDI recorder (MidiRecorder) — records frames when enabled
```

### Parameter evolution engine

`ParameterEvolution` runs a genetic algorithm on a population of ODE parameter
vectors.  Each individual is evaluated by running the ODE for `eval_steps`
frames and computing one of four fitness metrics:

| Metric | Signal | Implementation |
|--------|--------|----------------|
| Harmonic Richness | Autocorrelation peak ratio | Measures periodic content strength |
| Rhythmic Variance | Coefficient of variation of inter-onset intervals | Rewards complex but not chaotic rhythm |
| Timbral Diversity | Std-dev of spectral centroid over time | Rewards evolving timbre |
| User Defined | Arbitrary closure `&[[f64; 16]] → f64` | Full user control |

Selection is tournament-based (default k=3).  Crossover is uniform (per-gene coin
flip).  Mutation applies Gaussian perturbations (σ = 5% of range) at each locus
with probability `mutation_rate`.  Elitism preserves the best individual each
generation.  The result is a `SavedEvolution` with parameters and fitness score,
serializable as a TOML preset snippet.

### MIDI export pipeline

```
AudioParams (120 Hz) → MidiRecorder.push(pitch, velocity)
                              |
                     MidiFrame ring buffer
                              |
                     export_midi() → midly SMF type-0 .mid file
```

`coords_to_midi` maps the normalised attractor x-coordinate to the nearest
in-scale MIDI note over the configured octave range. Velocity is derived from
the y-magnitude. Consecutive frames with the same pitch are merged into a
single held note. The `midly` crate handles SMF serialization.

### DSP pipeline (per audio frame)

```
ODE state → Sonification mapper (120 Hz, sim thread)
                 ↓  AudioParams via bounded channel
            Audio callback (sample rate Hz)
                 ↓
         Oscillator (PolyBLEP anti-aliased, up to 4 voices)
                 ↓
         ADSR envelope (per voice)
                 ↓
         Waveshaper (soft-clip tanh, configurable drive)
                 ↓
         Bitcrusher (rate/bit reduction)
                 ↓
         3-band EQ (shelving + peaking)
                 ↓
         BiquadFilter (LP/HP/BP, configurable cutoff and Q)
                 ↓
         Stereo delay line (ping-pong)
                 ↓
         Chorus (LFO-modulated delay lines)
                 ↓
         FDN Reverb (8-channel feedback delay network, modulated)
                 ↓
         Lookahead limiter (4 ms lookahead, brickwall -0.3 dBFS)
                 ↓
         cpal output (32-bit float stereo)
```

## Building and testing

```bash
# Run all unit and integration tests (~1650 tests, no display required)
cargo test --lib --tests

# Release binary
cargo build --release --bin math-sonify

# Release plugin
cargo build --release --lib

# Documentation
cargo doc --no-deps --open
```

The test suite covers: ODE solver accuracy (attractor bounds, energy conservation, synchronization thresholds), scale quantization, polyphony, config parsing and clamping, scene arranger timeline consistency, oscillator amplitude bounds, ADSR envelope behavior, all-presets load/validate, lerp_config correctness for every system, and bifurcation parameter sweeps.

---

## Troubleshooting

**No audio / device not found**
- math-sonify uses `cpal::default_host().default_output_device()`. Ensure a device is selected in OS audio settings.
- Windows exclusive mode: close any application holding the device exclusively.
- Linux ALSA: `sudo apt install libasound2-dev`, add user to `audio` group.
- Sample rate mismatch: set `sample_rate = 48000` in `config.toml`.

**High CPU usage**
- Increase `buffer_size` to 1024 or 2048.
- Disable Evolve mode when not in use.
- For Three-Body and Lorenz-96, reduce `system.speed`.

**Distorted audio**
- Lower `audio.master_volume`.
- Set `waveshaper_drive = 1.0` and `waveshaper_mix = 0.0`.

**Phase portrait blank**
- Wait 2–3 seconds for the trail to build after startup or after pressing `R`.

**Config not loading**
- math-sonify looks for `config.toml` in the **current working directory**.

**VST3/CLAP not appearing**
- Copy to the correct system folder and trigger a plugin rescan in your DAW.
- The plugin requires `cargo build --release --lib`, not `--bin`.

---

## Contributing

1. Fork and create a feature branch.
2. Run `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings`.
3. Add tests for new public API (unit tests in the module, integration tests in `tests/integration.rs`).
4. Open a pull request. CI (fmt, clippy, test, doc, release build) must pass.

Code style: no `unsafe` without comment, no `.unwrap()` in `src/` outside tests, audio thread must be real-time safe (no heap allocation, no blocking I/O).

---

## License

MIT. See [LICENSE](LICENSE).

---

Built with [Rust](https://www.rust-lang.org), [cpal](https://github.com/RustAudio/cpal), [egui](https://github.com/emilk/egui), [nih-plug](https://github.com/robbert-vdh/nih-plug), [crossbeam](https://github.com/crossbeam-rs/crossbeam), [parking_lot](https://github.com/Amanieu/parking_lot), [hound](https://github.com/ruuda/hound), [rayon](https://github.com/rayon-rs/rayon), [tracing](https://github.com/tokio-rs/tracing).
