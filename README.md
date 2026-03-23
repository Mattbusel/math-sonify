# math-sonify

[![CI](https://github.com/Mattbusel/math-sonify/actions/workflows/ci.yml/badge.svg)](https://github.com/Mattbusel/math-sonify/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

math-sonify is a real-time generative audio engine that runs mathematical dynamical systems — differential equations, maps, and coupled oscillators — and routes every variable of their evolving state directly into audio synthesis parameters. The Lorenz attractor is actually integrating at 120 Hz; the Kuramoto coupling constant is live; the Three-Body gravitational problem advances at each control frame. The result is not a preset synthesiser with math-themed names: the mathematics _is_ the music, and every parameter change propagates to sound within 8 ms.

---

## Feature highlights

1. **53 dynamical systems** — Lorenz, Rossler, Double Pendulum, Kuramoto, Three-Body, Hyperchaos (Chen-Li), WINDMI, Finance, all Sprott cases, Tinkerbell map, and more (full list below).
2. **9 sonification modes** — Direct, Orbital, Granular, Spectral, FM, AM, Vocal, Waveguide, Resonator.
3. **20 musical scales** — Pentatonic through Microtonal, EDO-19/24/31, Harmonic Series, Just Intonation.
4. **MIDI export** — trajectory-to-MIDI conversion; outputs Standard MIDI Files (SMF) importable into any DAW.
5. **Preset gallery** — 16+ named presets with mood tags, complexity ratings, favorites, and a discovery mode that surfaces less-played entries.
6. **Collaborative session mode** — real-time multi-user parameter control via a WebSocket server with per-participant colour highlights, conflict resolution, and full session replay log.
7. **Audio-driven ODE morphing** — reverse the sonification pipeline: incoming microphone audio extracts features (RMS, spectral centroid, flux, 8-band energy) and maps them to ODE parameters in real time. Can run simultaneously with the forward synthesis path (dual mode).
8. **Lyapunov exponent tracker** — real-time estimation of the maximal Lyapunov exponent; displayed in the MATH VIEW tab.
9. **FFT spectral overlay** — live FFT spectrum superimposed on the phase portrait and the WAVEFORM tab.
10. **Scene arranger** — 8-scene timeline with smooth parameter morphs; AUTO generator builds full arrangements from a mood pool.
11. **VST3 / CLAP plugin** — load inside Ableton, FL Studio, Logic Pro, Reaper, and any other NIH-plug-compatible DAW.
12. **Headless render** — `--headless --duration 60 --output clip.wav` with no display required.
13. **Live config reload** — edit `config.toml` while the engine runs; changes take effect without restart.

---

## 5-minute quickstart

### Pre-built binary (Windows)

Download `math-sonify.exe` from the [latest release](https://github.com/Mattbusel/math-sonify/releases/latest) and double-click it. Audio starts immediately on the system default output device.

### Build from source

Requires [Rust](https://rustup.rs/) 1.75+ and a working audio output device.

```bash
git clone https://github.com/Mattbusel/math-sonify
cd math-sonify
cargo run --release
```

### Headless export (no GUI)

```bash
cargo run --release -- --headless --duration 60 --output clip.wav
```

---

## Architecture

```
ODE Solver (120 Hz, sim thread)
    |
    |  53 dynamical systems -- Lorenz, Rossler, Duffing, Kuramoto, Three-Body,
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
| Double Pendulum | 4 | chaos | Lagrangian mechanics (theta1, theta2, p1, p2); leapfrog integrator |
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
| Henon Map | 2 | chaos | Discrete map; fractal strange attractor (dim ~1.26) |
| Custom ODE | 3-4 | user | User-defined equations via text input |
| Fractional Lorenz | 3 | chaos | Lorenz with derivative order alpha in (0.5, 1.0] |
| Logistic Map | 1 | chaos | Period-doubling route to chaos; bifurcation diagram classic |
| Standard Map | 2 | chaos | Area-preserving Chirikov map; KAM tori to global chaos |
| Arnold Cat | 2 | chaos | Ergodic linear torus map; hyperbolic fixed point |
| Stochastic Lorenz | 3 | chaos | Lorenz with additive Wiener noise per axis |
| Delayed Map | 1 | chaos | Logistic map with discrete delay tau |
| Oregonator | 3 | oscillation | Belousov-Zhabotinsky chemical reaction oscillator |
| Mathieu | 2 | parametric | Parametric resonance; stability tongues in a/q space |
| Kuramoto-Driven | N | sync | Kuramoto + external sinusoidal drive on first oscillator |
| Thomas | 3 | chaos | Conservative symmetric attractor; b~0.208 chaos boundary |
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
| Sprott D (Case I) | 3 | chaos | y^2 instability with -1.1z dissipation |
| Sprott E | 3 | chaos | Minimal chaos from a yz product |
| Sprott F | 3 | chaos | Slow-spiral; x^2 drives z |
| Sprott G | 3 | chaos | Linear + quadratic; minimal form |
| Sprott H | 3 | chaos | Single xz product nonlinearity |
| Sprott K | 3 | chaos | xy product; one of Sprott's simplest forms |
| Sprott L | 3 | chaos | Bounded strange attractor; yz coupling |
| Shimizu-Morioka | 3 | chaos | Two-scroll; x^2-driven z destabilizes y |
| Genesio-Tesi | 3 | chaos | Jerk circuit: one x^2 term is all the chaos needed |
| Liu | 3 | chaos | Single-band scroll; y^2 and xz/xy cross-coupling |
| WINDMI | 3 | chaos | Ionospheric substorm model; exponential nonlinearity |
| Finance | 3 | chaos | Macroeconomic chaos: interest rate, investment, price |
| Hyperchaos (Chen-Li) | 4 | hyperchaos | Two positive Lyapunov exponents; richer than ordinary chaos |
| Tinkerbell | 2 | chaos | Complex-plane map; orbit traps and fractal basins |

---

## Sonification modes (9)

| Mode | How math maps to audio |
|------|------------------------|
| Direct | State variables quantized to configured scale -> oscillator frequencies. Amplitude tracks normalized magnitude. |
| Orbital | State interpreted as polar coordinates. Angular velocity drives pitch; Lyapunov exponent modulates inharmonicity. |
| Granular | Trajectory speed controls grain spawn rate (0-50 grains/sec). Position in state space sets grain frequency. |
| Spectral | 32 additive partials. Each partial amplitude derived from a normalized component of the state vector. |
| FM | Two-operator FM synthesis. Carrier tracks first state variable; modulator ratio and index driven by remaining variables. |
| AM | Amplitude modulation. Carrier frequency from state; AM depth and rate driven by trajectory speed. |
| Vocal | State coordinates mapped to vowel formant positions (F1/F2). Trajectory wanders through /a/ /e/ /i/ /o/ /u/. |
| Waveguide | Karplus-Strong string model. Tension and damping modulated by the attractor in real time. |
| Resonator | Modal resonator bank. Attractor state excites a set of tuned resonant modes. |

---

## Generative Composition Engine (`src/composer.rs`)

The composition engine (`ComposerEngine`) turns the running attractor into a structured musical piece in real time, without any pre-written score.

### Musical forms

| Form | Description |
|------|-------------|
| ABA | Statement (A), contrast (B), varied return (A'). Section boundaries follow basin changes. |
| Theme & Variations | Theme derived from the initial attractor basin; each variation alters a different synthesis parameter. |
| Rondo | Refrain (A) alternates with episodes driven by bifurcation events. |
| Through-Composed | Linear succession of sections with no repeats; topology-driven. |
| Stochastic | Section boundaries placed by a pseudo-random walk seeded from the attractor's own bit-mixing. |

### Components

| Component | Role |
|-----------|------|
| `MotifGenerator` | Slides a window over recent pitches; finds the most-repeated sub-sequence as the current motif. |
| `HarmonicProgression` | Maps phase-space regions to chord progressions: Classical (I–IV–V–I), Jazz Turnaround (ii7–V7–Imaj7–VI7), Modal (scale-derived triads), Jazz Extended (9th/11th/13th tensions added proportional to chaos level). |
| `RhythmicQuantizer` | Snaps the continuous ODE output to a rhythmic grid. Supports 4/4, 3/4, 5/4, 6/8, and 7/8 time signatures with configurable sub-division. |
| `CompositionExporter` | Builds a 3-track MIDI SMF (melody, harmony, bass) and writes it to disk via the existing MIDI infrastructure. |

### MIDI export example

```rust
use math_sonify::composer::{ComposerEngine, MusicalForm, ProgressionStyle, TimeSig};

let mut engine = ComposerEngine::new(
    MusicalForm::Aba,
    ProgressionStyle::JazzTurnaround,
    TimeSig::FOUR_FOUR,
    120.0,   // BPM
    120.0,   // control rate Hz
    8,       // section length in bars
);

// Tick the engine each sim step:
let frame = engine.tick(&state, melody_pitch, velocity, chaos_level, lyapunov);
// frame.chord, frame.motif, frame.section_idx, frame.bar_position, …

// Export when done:
engine.export_midi("composition.mid", 120.0).ok();
```

---

## Fractal Dimension Analyzer (`src/fractal.rs`)

The fractal analyzer characterises the geometric and dynamical structure of the attractor in real time. All metrics are displayed in the **Math View** tab.

### Algorithms

| Algorithm | Struct | What it computes |
|-----------|--------|-----------------|
| Box-counting | `BoxCounting` | D₀ (Minkowski–Bouligand dimension). 2-D projection; slope of log N(ε) vs log(1/ε). Lorenz ≈ 2.05, Hénon ≈ 1.26. |
| Correlation dimension | `CorrelationDimension` | D₂ (Grassberger–Procaccia). Counts point pairs within distance r; slope of log C(r) vs log r. |
| Full Lyapunov spectrum | `LyapunovSpectrum` | All N exponents via QR/Gram–Schmidt on the tangent bundle. Kaplan–Yorke dimension D_KY = j + Σλᵢ/|λⱼ|. |

### `AttractorCharacterization` struct

```rust
pub struct AttractorCharacterization {
    pub fractal_dim: f64,                    // Box-counting D₀
    pub correlation_dim: f64,                // Grassberger-Procaccia D₂
    pub lyapunov_spectrum: Vec<f64>,         // Full spectrum λ₁ ≥ λ₂ ≥ ... ≥ λₙ
    pub kaplan_yorke_dim: f64,               // D_KY from the spectrum
    pub kolmogorov_entropy: f64,             // hKS = Σ positive λᵢ
    pub phase_space_volume_contraction: f64, // Σ all λᵢ (< 0 for dissipative)
    pub attractor_type: AttractorType,       // FixedPoint / LimitCycle / StrangeAttractor / Hyperchaos
    pub sample_size: usize,
    pub last_updated_ticks: u64,
}
```

`AttractorType` is derived automatically from the spectrum sign pattern:

| Type | Criterion |
|------|-----------|
| Fixed Point | All λᵢ < 0 |
| Limit Cycle | Exactly one zero exponent, rest negative |
| Quasi-Periodic (T²) | Two zero exponents |
| Strange Attractor | Exactly one positive exponent |
| Hyperchaos | Two or more positive exponents |

### Usage

```rust
use math_sonify::fractal::FractalAnalyzer;

let mut analyzer = FractalAnalyzer::new(3); // 3-D system

// Called periodically in the sim thread:
let ch = analyzer.analyze(&trajectory, &lorenz_deriv, dt, current_tick);
println!("{}", analyzer.lyapunov_spectrum().summary());
// λ = [+0.9053, -0.0001, -14.572]  D_KY=2.062  hKS=0.9053  div=-13.667
```

---

## Network of Coupled Oscillators (`src/network.rs`)

`OscillatorNetwork` places up to **16 coupled oscillators** on an arbitrary graph topology, with each oscillator mapped to a separate audio voice.

### Network topologies

| Topology | Description |
|----------|-------------|
| `Ring` | Each node connected to its two nearest neighbours (circular). |
| `StarGraph` | Central hub connected to all leaves; leaves only connect to the hub. |
| `SmallWorld(p)` | Watts–Strogatz: start from a ring and rewire each edge with probability p. |
| `RandomErdos(p)` | Each pair connected independently with probability p. |
| `FullyConnected` | All-to-all coupling; mean-field limit of the Kuramoto model. |

### Oscillator models

**Kuramoto network** — phase oscillators on an arbitrary graph:

```
dθᵢ/dt = ωᵢ + Σⱼ Kᵢⱼ sin(θⱼ − θᵢ)
```

Each oscillator's phase maps directly to an audio frequency. The order parameter r ∈ [0, 1] measures synchronisation.

**Stuart–Landau network** — complex-amplitude oscillators (normal form of the Hopf bifurcation):

```
dAᵢ/dt = (μᵢ + iωᵢ − |Aᵢ|²)·Aᵢ + Σⱼ Kᵢⱼ·Aⱼ
```

With diffusive coupling and heterogeneous μ, the network exhibits:
- **Amplitude death** (μ < 0 and coupling pushes all voices to zero).
- **Oscillation revival** (coupling restores oscillations suppressed by individual μ < 0).

### Audio voice mapping

Each tick, `OscillatorNetwork::state()` returns a `NetworkState`:

```rust
pub struct NetworkState {
    pub n: usize,
    pub frequencies: Vec<f64>,   // per-voice audio frequency (Hz)
    pub amplitudes: Vec<f64>,    // per-voice amplitude [0, 1]
    pub phases: Vec<f64>,        // oscillator phase (radians)
    pub order_parameter: f64,    // Kuramoto r or mean amplitude
    pub amplitude_death: bool,   // true if network collapsed to zero
    pub active_voices: usize,    // number of non-silent voices
}
```

`NetworkState::sorted_voices()` returns `(frequency, amplitude)` pairs sorted by amplitude, ready for polyphonic voice assignment.

### Example

```rust
use math_sonify::network::{OscillatorNetwork, NetworkTopology};

// 8 Kuramoto oscillators in a small-world graph.
let mut net = OscillatorNetwork::kuramoto(
    8,
    &NetworkTopology::SmallWorld { rewire_prob: 0.15 },
    2.0,   // coupling K
    0.4,   // natural frequency spread
    220.0, // base audio frequency (Hz)
    440.0, // frequency range (Hz)
    42,    // seed
);

// Sim loop (120 Hz):
net.step(1.0 / 120.0);
let st = net.state();
// Route st.frequencies[i] and st.amplitudes[i] to audio voice i.
println!("r = {:.3}  voices = {}", st.order_parameter, st.active_voices);
```

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

## MIDI export guide

math-sonify can export attractor trajectories to Standard MIDI Files (SMF format 0) that import cleanly into Ableton Live, FL Studio, Logic Pro, Reaper, and any other DAW.

### Mapping

| Attractor coordinate | MIDI parameter |
|---|---|
| X | Note pitch -- quantised to the selected scale |
| Y | Velocity (64-127) |
| Z | Note duration (16th note to whole note, exponentially scaled) |
| Simulation speed | BPM written into the file tempo event |

### From the GUI

1. Open the **MIXER** tab.
2. Click **Record MIDI** to start capturing. The status bar shows the frame count.
3. Click **Stop + Export** to choose a filename and write the `.mid` file.

### From Rust code

```rust
use math_sonify_plugin::midi_export::{MidiExporter, SCALE_PENTATONIC_C4};

// trajectory is a Vec<(f64, f64, f64)> collected from the ODE solver
let exporter = MidiExporter::new();   // 480 ticks per quarter note
let track = exporter.trajectory_to_track(
    "Lorenz Take 1",
    &trajectory,
    SCALE_PENTATONIC_C4,
    120.0,   // BPM
);
exporter.export_to_file(&[track], "lorenz_take1.mid")?;
```

Multiple tracks can be passed to `export_smf` / `export_to_file`; they are merged into the single track required by SMF format 0, with each track's notes placed on a different MIDI channel (0-15) for DAW separation.

### Headless export

```bash
cargo run --release -- --headless --duration 30 --output take.wav --export-midi take.mid
```

---

## Preset gallery guide

math-sonify ships with 16 named presets organised in a browsable in-memory catalogue. Each preset carries:

- **System** -- which dynamical system it uses.
- **Mood tags** -- `atmospheric`, `rhythmic`, `experimental`, `meditative`, `melodic`, `percussive`, `drone`, `eerie`, `evolving`, `hypnotic`, `minimalist`, `complex`, `electronic`, `energetic`.
- **BPM range** -- the tempo window in which the preset sounds best.
- **Complexity** -- 1 (minimal) to 5 (dense).

### In the GUI

The **SYNTH** tab has a **Presets** panel with:

- A scrollable list filtered by mood or system.
- A search box for partial name/description match.
- A heart icon to toggle favorites.
- A **Discover** button that picks a random preset weighted toward entries you have played least.

### From Rust code

```rust
use math_sonify_plugin::preset_gallery::PresetGallery;

let mut gallery = PresetGallery::with_builtin_presets();

// Filter by mood
let drones = gallery.by_mood("drone");

// Search
let results = gallery.search("butterfly");

// Random discovery (weighted by inverse play count)
if let Some(preset) = gallery.random_discovery() {
    println!("Try: {} ({})", preset.name, preset.system);
    gallery.record_play(&preset.name.clone());
}

// Favorites
gallery.toggle_favorite("Lorenz Ambience");
let favs = gallery.favorites();
```

### Built-in presets

| Name | System | Moods | Complexity |
|------|--------|-------|-----------|
| Lorenz Ambience | Lorenz | atmospheric, meditative, melodic | 2 |
| Pendulum Rhythm | Double Pendulum | rhythmic, percussive, energetic | 3 |
| Torus Drone | Geodesic Torus | atmospheric, meditative, drone | 2 |
| Kuramoto Sync | Kuramoto | experimental, evolving, hypnotic | 3 |
| Three-Body Jazz | Three-Body | melodic, rhythmic, complex | 4 |
| Rossler Drift | Rossler | atmospheric, melodic, meditative | 2 |
| FM Chaos | Lorenz | experimental, electronic, energetic | 4 |
| Pendulum Meditation | Double Pendulum | meditative, atmospheric, drone | 2 |
| Thomas Labyrinth | Thomas | atmospheric, experimental, eerie | 3 |
| Neural Burst | Hindmarsh-Rose | rhythmic, percussive, experimental | 4 |
| Chemical Wave | Oregonator | atmospheric, evolving, hypnotic | 3 |
| Sprott Minimal | Sprott E | experimental, electronic, minimalist | 2 |
| Substorm Pulse | WINDMI | rhythmic, atmospheric, electronic | 3 |
| Market Collapse | Finance | experimental, eerie, complex | 4 |
| Hyperdimensional | Hyperchaos | experimental, complex, electronic | 5 |
| Magyar Trance | Dadras | meditative, melodic, atmospheric | 3 |

---

## Collaborative session guide

math-sonify includes two complementary collaboration features: a low-level protocol (`collaboration.rs`) for sharing attractor state between performers, and a full-featured **Collaborative Session** server (`collab.rs`) for real-time multi-user parameter editing.

### Collaborative Session server (`collab.rs`)

The session server is a raw-TCP WebSocket-style server that accepts JSON connections from any number of participants. Each participant can claim ownership of specific ODE parameters, edit them in real time, and all changes propagate immediately to every other connected client.

**Key types:**

| Type | Role |
|------|------|
| `CollabServer` | TCP listener; spawns a thread per client |
| `SessionEvent` | Events emitted to the simulation thread (`ParamChanged`, `ClientJoined`, `ClientLeft`) |
| `SharedSynthState` | ODE parameters + sonification mode + scale, wrapped in `Arc<RwLock<>>` for lock-free reads |
| `ParticipantCursor` | Each participant has a unique colour highlight on the parameter they are currently editing |
| `SessionLog` | Full ordered history of every parameter change for post-session replay |

**Wire protocol** (newline-delimited JSON):

```json
// Client -> Server
{ "claim":   ["rho", "sigma"] }
{ "set":     { "rho": 28.5 } }
{ "release": ["rho"] }

// Server -> Client
{ "welcome":     { "client_id": 3 } }
{ "update":      { "rho": 28.5, "owner": 3 } }
{ "error":       "parameter 'rho' is owned by client 1" }
{ "peer_joined": { "client_id": 4, "total": 2 } }
{ "peer_left":   { "client_id": 4, "total": 1 } }
```

**Conflict resolution:** last-write-wins per parameter. A participant must first `claim` a parameter; attempts to `set` an unclaimed or foreign-owned parameter are rejected with an `error` message.

**Starting the server:**

```rust
use math_sonify_plugin::collab::{CollabServer, SessionEvent};
use crossbeam_channel::unbounded;

let (tx, rx) = unbounded::<SessionEvent>();
let server = CollabServer::new("127.0.0.1:9001", tx).unwrap();
server.run_background();

// In the simulation thread:
for event in rx.try_iter() {
    match event {
        SessionEvent::ParamChanged { name, value, .. } => { /* apply to ODE */ }
        _ => {}
    }
}
```

Connect any WebSocket client (browser, `websocat`, Python `websockets`) to `ws://127.0.0.1:9001` to join the session.

### Legacy performance protocol (`collaboration.rs`)

math-sonify also includes a JSON-based collaborative performance protocol that lets multiple performers share attractor state in real time. Any transport layer (WebSocket, UDP, OSC) can carry the messages; the module itself only handles serialisation and session logic.

### Concepts

- **Session** -- a named room (e.g. `"concert-2026-03-22"`) that holds up to 8 performers.
- **Performer** -- identified by a unique string ID, carries live `(x, y, z)` attractor coordinates, BPM, volume, and an RGB colour.
- **Messages** -- typed JSON objects: `JoinSession`, `LeaveSession`, `StateUpdate`, `ParameterSync`, `ChatMessage`, `KickOff`.

### Example flow

```rust
use math_sonify_plugin::collaboration::{CollaborationClient, PerformerState};

// Create local performer
let performer = PerformerState::new("alice-01", "Alice");
let mut client = CollaborationClient::new(performer);

// Join
let join_msg = client.join_message("my-session");
let json = CollaborationClient::serialize_message(&join_msg);
// ... send json over your WebSocket / UDP socket ...

// Each sim tick: push current attractor state
let update = client.push_xyz(lorenz_x, lorenz_y, lorenz_z);
let json = CollaborationClient::serialize_message(&update);
// ... send json ...

// Receive a message
let incoming_json = r#"{"type":"KickOff","session_id":"my-session","bpm":128.0}"#;
let msg = CollaborationClient::deserialize_message(incoming_json).unwrap();
```

### Server-side session tracking

```rust
use math_sonify_plugin::collaboration::CollaborationSession;

let mut session = CollaborationSession::new("my-session");

// On receive JoinSession
session.join(performer_state)?;

// On receive StateUpdate
session.update_state(performer_state);

// Broadcast mean-attractor coordinates to new joiners
let sync_msg = session.broadcast_message();
```

### Message reference

```json
{ "type": "JoinSession",   "session_id": "room1", "performer": { ... } }
{ "type": "LeaveSession",  "performer_id": "alice-01" }
{ "type": "StateUpdate",   "performer": { "x": 1.2, "y": -3.4, "z": 0.8, ... } }
{ "type": "ParameterSync", "params": { "rho": 28.0, "sigma": 10.0 } }
{ "type": "ChatMessage",   "performer_id": "alice-01", "text": "raising sigma now" }
{ "type": "KickOff",       "session_id": "room1", "bpm": 128.0 }
```

---

## Presets

math-sonify ships with ~40 named presets organised into four moods:

- **Atmospheric** -- Midnight Approach, Breathing Galaxy, Aurora Borealis, Deep Hypnosis, Cathedral Organ, Substorm, and more.
- **Rhythmic** -- Frozen Machinery, The Phase Transition, Clockwork Insect, Industrial Heartbeat, Velocity Band, and more.
- **Experimental** -- Neon Labyrinth, Dissociation, Jerk Circuit, Invisible Hand, Hyperchaos Engine, and more.
- **Melodic** -- Glass Harp, Electric Kelp, The Butterfly's Aria, Solar Wind, and more.

The **AUTO** arrangement generator picks 6 presets from a mood pool, scatters system parameters into varied dynamical regimes, randomises synthesis settings, and builds an 8-scene timeline with morphs as the main musical event.

---

## Audio output

math-sonify outputs 32-bit IEEE float stereo PCM at the system default sample rate (44100 or 48000 Hz).

| Export method | Details |
|---------------|---------|
| Clip save (`S`) | Last 60 seconds -> 32-bit float WAV in `clips/` |
| Loop export | Current loop region -> WAV |
| MIDI export | Trajectory -> SMF `.mid` importable into any DAW |
| Headless render | `--headless --duration 60 --output clip.wav` -- no display required |

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

After copying, trigger a plugin rescan in your DAW (**Options > Plug-in Manager** in Ableton; **Plug-in Database > Rescan** in FL Studio).

---

## VST/CLAP plugin setup

1. Run `cargo build --release --lib`.
2. Locate the output file in `target/release/`.
3. Copy to the system VST3 folder for your platform (table above).
4. Open your DAW and trigger a plugin rescan.
5. Search for "math-sonify" in the plugin browser.
6. The plugin exposes all system parameters as automatable VST3 parameters.
7. MIDI output from the plugin can be routed to any instrument track.

---

## GUI

Five top-level tabs:

- **SYNTH** -- system selector, parameter sliders, sonification mode, scale, effects chain, randomize, preset browser.
- **MIXER** -- per-layer volume/pan/ADSR, master effects (EQ, delay, chorus, reverb), VU meters, WAV export, MIDI record/export.
- **ARRANGE** -- scene timeline, morph time controls, AUTO arrangement generator with mood selection.
- **MATH VIEW** -- live phase portrait (XY/XZ/YZ/3D), bifurcation diagram, custom ODE text input, state readout.
- **WAVEFORM** -- oscilloscope and spectrum analyzer.

Performance mode (`F`) switches to fullscreen phase portrait only.

---

## Keyboard shortcut reference

| Key | Action |
|-----|--------|
| `F` | Toggle fullscreen performance mode |
| `Space` | Pause / resume simulation |
| `R` | Reset attractor to default initial condition |
| `S` | Save clip (last 60 seconds as WAV) |
| `Ctrl+S` | Save current configuration to `config.toml` |
| `1` -- `7` | Switch sonification mode |
| `<` / `>` | Previous / next dynamical system |
| `Up` / `Down` | Increase / decrease simulation speed by 10% |
| `E` | Toggle Evolve (autonomous parameter wandering) |
| `A` | Toggle AUTO arrangement playback |
| `P` | Play / stop scene arranger |
| `M` | Toggle MIDI record |
| `Escape` | Exit fullscreen |

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

## Audio-driven ODE morphing guide

In addition to the classic forward pipeline (ODE state → audio), math-sonify can reverse the flow: use **incoming microphone audio** to continuously modify ODE parameters in real time.

### How it works

```
Microphone / line-in (cpal default input)
    |
    v  per-frame (configurable hop size, default 512 samples)
AudioInputAnalyzer
    |  computes:
    |    RMS        → Lorenz σ  (louder = more chaos, σ ∈ [5, 30])
    |    Centroid   → Lorenz ρ  (brighter spectrum = higher ρ, ρ ∈ [15, 60])
    |    Flux       → system switch trigger (transients trigger attractor changes)
    |    8-band energy → Lorenz β (mid-band energy → β ∈ [1.5, 4.0])
    v
AudioFeatures { rms, centroid, flux, bands: [f32; 8] }
    |
    v
AudioOdeBridge  (delta suppression: only emits patch when change exceeds threshold)
    |
    v
OdePatch → simulation thread (applies sigma/rho/beta overrides)
```

### Dual mode

`DualMode` lets both pipelines run simultaneously:

| Mode | Description |
|------|-------------|
| `ForwardOnly` | Classic: ODE state drives audio synthesis (default) |
| `ReverseOnly` | Microphone input drives ODE parameters only |
| `Both` | Both paths active simultaneously — environment modulates the attractor which modulates the sound which feeds back into the environment |

### Usage

```rust
use math_sonify_plugin::audio_driven::{AudioOdeBridge, BridgeConfig, DualMode, DualModeKind};
use crossbeam_channel::unbounded;

// Build the reverse pipeline
let dual = DualMode::new(BridgeConfig::default());
let (mut analyzer, patch_rx, _stop) = dual.build_reverse_pipeline();

// Feed audio samples from cpal input callback:
// analyzer.feed(sample);  // called per sample in the cpal callback

// In the simulation thread:
for patch in patch_rx.try_iter() {
    if let Some(sigma) = patch.sigma { ode_params.sigma = sigma; }
    if let Some(rho)   = patch.rho   { ode_params.rho   = rho;   }
    if patch.trigger_system_switch   { /* switch to next attractor */ }
}
```

### Configuration (`BridgeConfig`)

| Field | Default | Description |
|-------|---------|-------------|
| `sigma_min` / `sigma_max` | 5.0 / 30.0 | Lorenz σ range |
| `rho_min` / `rho_max` | 15.0 / 60.0 | Lorenz ρ range |
| `beta_min` / `beta_max` | 1.5 / 4.0 | Lorenz β range |
| `flux_switch_threshold` | 0.6 | Flux above this triggers a system switch |
| `fft_size` | 1024 | FFT frame size (power of two) |
| `hop_size` | 512 | Samples between analysis frames |

---

## Mathematical background

### Dynamical systems and attractors

A **dynamical system** is a set of differential equations `dx/dt = f(x)` or a map `x_{n+1} = f(x_n)`. The long-term behaviour of trajectories in phase space determines the system's character:

- **Fixed point** — all trajectories converge to a single point (stable equilibrium).
- **Limit cycle** — trajectories converge to a closed loop (periodic oscillation).
- **Quasi-periodic** — trajectories wind around a torus; the ratio of frequencies is irrational.
- **Chaotic attractor (strange attractor)** — trajectories are bounded but never repeat; nearby trajectories diverge exponentially (sensitive dependence on initial conditions).

### Lyapunov exponents

The **maximal Lyapunov exponent** λ₁ quantifies the average rate of exponential divergence of nearby trajectories:

```
||δx(t)|| ≈ e^{λ₁ t} ||δx(0)||
```

- λ₁ < 0: stable fixed point or limit cycle.
- λ₁ = 0: quasi-periodic or at a bifurcation boundary.
- λ₁ > 0: chaotic. The Lorenz system at standard parameters has λ₁ ≈ 0.906.

math-sonify estimates λ₁ using a standard rescaling algorithm: a shadow trajectory is integrated alongside the main one, the separation is measured every N steps, its logarithm is accumulated, and the separation is rescaled. This runs at `LYAP_INTERVAL_TICKS` (every 2 seconds of sim time).

### FFT spectral analysis

The FFT module (`spectrum.rs`) computes a 1024-point Hann-windowed FFT of the synthesised audio output and displays:

- Magnitude spectrum (dB scale, linear frequency axis).
- Spectral centroid (brightness indicator).
- Fundamental frequency estimate via parabolic interpolation on the magnitude peak.

The audio-driven morphing module uses the same FFT on the *input* signal to extract audio features.

### Chaos onset in the Lorenz system

The Lorenz system `(σ=10, β=8/3, ρ)` undergoes the following transitions as ρ increases:

| ρ range | Behaviour |
|---------|-----------|
| ρ < 1 | All trajectories converge to origin |
| 1 < ρ < 13.93 | Two stable fixed points (C+ and C−) |
| 13.93 < ρ < 24.06 | Unstable limit cycles; trajectories still attracted to C± |
| ρ > 24.74 | Strange attractor (chaos onset) — the classic butterfly |

---

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

The test suite covers: ODE solver accuracy (attractor bounds, energy conservation, synchronization thresholds), scale quantization, polyphony, config parsing and clamping, scene arranger timeline consistency, oscillator amplitude bounds, ADSR envelope behavior, all-presets load/validate, lerp_config correctness for every system, bifurcation parameter sweeps, MIDI frame recording and SMF export, preset gallery filtering and discovery, and collaboration session/client message round-trips.

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
- Wait 2-3 seconds for the trail to build after startup or after pressing `R`.

**Config not loading**
- math-sonify looks for `config.toml` in the **current working directory**.

**VST3/CLAP not appearing**
- Copy to the correct system folder and trigger a plugin rescan in your DAW.
- The plugin requires `cargo build --release --lib`, not `--bin`.

**MIDI export produces empty file**
- Start recording before the session (click **Record MIDI** in the MIXER tab), then export.
- Headless: pass `--export-midi output.mid` on the command line.

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

Built with [Rust](https://www.rust-lang.org), [cpal](https://github.com/RustAudio/cpal), [egui](https://github.com/emilk/egui), [nih-plug](https://github.com/robbert-vdh/nih-plug), [crossbeam](https://github.com/crossbeam-rs/crossbeam), [parking_lot](https://github.com/Amanieu/parking_lot), [hound](https://github.com/ruuda/hound), [rayon](https://github.com/rayon-rs/rayon), [tracing](https://github.com/tokio-rs/tracing), [serde](https://github.com/serde-rs/serde), [serde_json](https://github.com/serde-rs/json), [midly](https://github.com/nicholasgasior/midly).
