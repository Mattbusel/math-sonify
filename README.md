# math-sonify

[![CI](https://github.com/Mattbusel/math-sonify/actions/workflows/ci.yml/badge.svg)](https://github.com/Mattbusel/math-sonify/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/math-sonify.svg)](https://crates.io/crates/math-sonify)

**Math sonification** is the practice of mapping the evolving state of a mathematical system directly to audio synthesis parameters so that the structure of the mathematics becomes audible. math-sonify runs differential equations continuously in real time and routes every variable of their state vector into oscillator frequencies, grain densities, FM modulation indices, formant positions, or waveguide string parameters. The result is not a preset synthesizer with math-themed names: the Lorenz attractor is actually integrating, the Kuramoto coupling constant is live, the Three-Body gravitational problem is stepped forward at 120 Hz.

---

## What is math sonification?

Classical sonification maps data to sound after the fact (load a CSV, play it back). Math sonification is generative and continuous: the sound is the running computation. There is no playback cursor; the audio is produced by the physics as it happens.

This makes the technique useful for:
- Auditory exploration of dynamical system behavior (period doubling, chaos onset, synchronization).
- Generative music where the mathematical constraints of the attractor act as a compositional structure.
- Live performance: parameter changes propagate to audio within one control-rate frame (8 ms at 120 Hz).

---

## Architecture

```
ODE Solver (120 Hz, sim thread)
    |
    |  Lorenz / Rossler / Duffing / Kuramoto / Three-Body / ...
    |  RK4 or leapfrog integration per configured dt
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
    |  DirectMapping   -- state quantized to musical scale -> oscillator freqs
    |  OrbitalResonance-- angular velocity + Lyapunov exponent drive pitch
    |  GranularMapping -- trajectory speed -> grain density and pitch
    |  SpectralMapping -- state -> 32-partial additive envelope
    |  FmMapping       -- attractor drives carrier/modulator ratio and index
    |  VocalMapping    -- state interpolates between vowel formant positions
    |  Waveguide       -- Karplus-Strong string with chaotic modulation
    |
    v  [crossbeam bounded channel, try_recv in audio callback]
    v
Audio Synthesis (audio thread, 44100 / 48000 Hz)
    |
    |  Per-layer DSP:
    |    Oscillator(s) [PolyBLEP anti-aliased] --> ADSR --> Waveshaper --> Bitcrusher
    |
    |  Master bus (shared across up to 3 layers):
    |    3-Band EQ --> LP BiquadFilter --> Stereo DelayLine --> Chorus
    |    --> FDN Reverb (8-channel, modulated) --> Lookahead Limiter
    |
    v
DAW (VST3 / CLAP plugin) or Desktop (standalone cpal output)
```

**Thread safety:** the sim thread and audio thread communicate through a bounded `crossbeam-channel` of capacity 16. The audio callback calls `try_recv` and renders silence on a miss, so it is never blocked. The UI thread reads shared state through `parking_lot::Mutex` on the control rate.

---

## Supported mathematical systems

| System | Dimension | Notes |
|--------|-----------|-------|
| Lorenz | 3 | Classic butterfly attractor; chaos onset near rho=24.74 |
| Rossler | 3 | Spiral attractor; period-doubling as c increases |
| Double Pendulum | 4 | Lagrangian mechanics (theta1, theta2, p1, p2); leapfrog |
| Geodesic Torus | 4 | Ergodic irrational winding on a flat torus |
| Kuramoto | N | N coupled oscillators; synchronization at critical K |
| Three-Body | 12 | Newtonian gravity, 3 point masses in 2D; figure-8 ICs |
| Duffing | 2 | Driven nonlinear oscillator; period-doubling cascade |
| Van der Pol | 2 | Self-sustaining limit cycle; relaxation oscillations |
| Halvorsen | 3 | Dense cyclic-symmetry spiral attractor |
| Aizawa | 3 | Six-parameter torus-like attractor |
| Chua | 3 | Piecewise-linear double-scroll circuit |
| Hindmarsh-Rose | 3 | Neuron firing model; bursting and spiking |
| Lorenz-96 | N | Weather model; spatiotemporal chaos at F > 8 |
| Mackey-Glass | DDE | Delay differential equation; history-dependent |
| Nose-Hoover | 3 | Thermostatted Hamiltonian; conservative chaos |
| Coupled Map Lattice | N | Logistic map on a 1D lattice with coupling |
| Henon Map | 2 | Discrete map; fractal strange attractor |
| Custom ODE | 3 | User-defined equations via text input |
| Fractional Lorenz | 3 | Lorenz with derivative order alpha in (0.5, 1.0] |

**Equations (abbreviated):**

```
Lorenz:      dx/dt = sigma*(y-x),  dy/dt = x*(rho-z)-y,  dz/dt = xy-beta*z
Rossler:     dx/dt = -y-z,         dy/dt = x+a*y,         dz/dt = b+z*(x-c)
Duffing:     dx/dt = v,  dv/dt = -delta*v - alpha*x - beta*x^3 + gamma*cos(phi)
Van der Pol: dx/dt = v,  dv/dt = mu*(1-x^2)*v - x
Kuramoto:    d(theta_i)/dt = omega_i + (K/N)*sum_j sin(theta_j - theta_i)
```

---

## Sonification modes

| Mode | How math maps to audio |
|------|------------------------|
| Direct | Each state variable is quantized to the configured scale, producing oscillator frequencies. Amplitude tracks the variable's normalized magnitude. |
| Orbital | State variables are interpreted as polar coordinates. Angular velocity drives pitch; Lyapunov exponent modulates inharmonicity. |
| Granular | Trajectory speed controls grain spawn rate (0..50 grains/sec). Position in state space sets grain frequency. Chaos level thickens the cloud. |
| Spectral | Up to 32 additive partials. Each partial amplitude is derived from a normalized component of the state vector, producing a continuously morphing spectrum. |
| FM | Two-operator frequency modulation. Carrier frequency tracks the first state variable; modulator-to-carrier ratio and FM index are driven by the remaining variables. |
| Vocal | State coordinates are mapped to vowel formant positions (F1/F2 pairs). The trajectory wanders through vowel space /a/ /e/ /i/ /o/ /u/. |
| Waveguide | A Karplus-Strong waveguide string model. Tension and damping are modulated by the attractor trajectory in real time. |

---

## Audio output formats

math-sonify outputs **32-bit IEEE float stereo PCM** on the real-time audio thread. It always uses the system default output device.

When exporting audio:
- **Clip save** (`S` key or WAV button): exports the last 60 seconds as 32-bit float stereo WAV to the `clips/` directory.
- **Loop export**: exports the current loop region as a WAV file.
- **Headless render** (`--headless --output file.wav`): renders to 16-bit or 32-bit WAV (configurable).

Supported sample rates: **44100 Hz** (default) and **48000 Hz** (set in `config.toml`). Other rates fall back to 44100 Hz.

The plugin (VST3/CLAP) exports audio at whatever sample rate the host requests.

---

## Installation

### From crates.io

Requires [Rust](https://rustup.rs/) 1.75 or later and a working audio output device.

```bash
cargo install math-sonify
math-sonify
```

### From source

```bash
git clone https://github.com/Mattbusel/math-sonify
cd math-sonify
cargo run --release
```

### Pre-built binaries

Download a pre-built executable from the [latest GitHub release](https://github.com/Mattbusel/math-sonify/releases/latest).

---

## Quickstart

### Standalone application

```bash
cargo run --release
```

Audio starts immediately using the system default output device at 44100 Hz. The Lorenz attractor runs in Direct mode with a pentatonic scale. Use the GUI to switch systems, adjust parameters, and configure effects.

### Headless mode (WAV export, no GUI)

```bash
cargo run --release -- --headless --duration 60 --output clip.wav
```

Renders 60 seconds of audio to `clip.wav` with no display required. Suitable for batch exports and server environments.

### Build the VST3 / CLAP plugin

```bash
cargo build --release --lib
```

The plugin shared library is written to `target/release/`. Copy it to your DAW plugin folder and rescan.

| Platform | File | Destination |
|----------|------|-------------|
| Windows  | `math_sonify_plugin.dll` | `C:\Program Files\Common Files\VST3\` |
| Linux    | `libmath_sonify_plugin.so` | `~/.vst3/` |
| macOS    | `libmath_sonify_plugin.dylib` | `~/Library/Audio/Plug-Ins/VST3/` |

---

## Configuration reference

The application reads `config.toml` from the current working directory at startup. All fields are optional; missing values use defaults. The file is watched with `notify`; edits take effect without restarting.

```toml
[system]
name  = "lorenz"  # active system: lorenz | rossler | double_pendulum | geodesic_torus
                  # | kuramoto | three_body | duffing | van_der_pol | halvorsen | aizawa
                  # | chua | hindmarsh_rose | lorenz96 | mackey_glass | nose_hoover
                  # | coupled_map_lattice | henon_map | custom_ode | fractional_lorenz
dt    = 0.001     # ODE integration time step (clamped 0.0001..0.1)
speed = 1.0       # simulation speed multiplier (0..100)

[lorenz]
sigma = 10.0
rho   = 28.0
beta  = 2.6667

[rossler]
a = 0.2
b = 0.2
c = 5.7

[kuramoto]
n_oscillators = 8
coupling      = 1.5   # K; synchronization threshold approx 1.0

[duffing]
delta = 0.3
alpha = -1.0
beta  = 1.0
gamma = 0.5
omega = 1.2

[van_der_pol]
mu = 2.0

[halvorsen]
a = 1.89

[chua]
alpha = 15.6
beta  = 28.0
m0    = -1.143
m1    = -0.714

[hindmarsh_rose]
current_i = 3.0  # external drive current (main control parameter)
r         = 0.006

[lorenz96]
f = 8.0  # forcing parameter; chaos for f > 8

[audio]
sample_rate      = 44100   # 44100 or 48000
buffer_size      = 512
reverb_wet       = 0.4     # 0..1
delay_ms         = 300.0   # 1..5000 ms
delay_feedback   = 0.3     # 0..0.99
master_volume    = 0.7     # 0..1
bit_depth        = 16.0    # 1..32 (32 = bypass bitcrusher)
rate_crush       = 0.0     # 0..1 (0 = bypass rate crusher)
chorus_mix       = 0.0     # 0..1
chorus_rate      = 0.5     # Hz
chorus_depth     = 3.0     # ms
waveshaper_drive = 1.0     # 0..100
waveshaper_mix   = 0.0     # 0..1

[sonification]
mode                = "direct"
# Modes: direct | orbital | granular | spectral | fm | vocal | waveguide
scale               = "pentatonic"
# Scales: pentatonic | chromatic | just_intonation | microtonal
#         | edo19 | edo31 | edo24 | whole_tone | phrygian | lydian
base_frequency      = 220.0         # Hz, root of the scale
octave_range        = 3.0           # number of octaves spanned
transpose_semitones = 0.0
chord_mode          = "none"
# Chord modes: none | major | minor | power | sus2 | octave | dom7
portamento_ms       = 80.0          # frequency glide time
voice_levels        = [1.0, 0.8, 0.6, 0.4]
voice_shapes        = ["sine", "sine", "sine", "sine"]
# Shapes: sine | saw | square | triangle | noise

[viz]
trail_length = 800    # number of points in the phase portrait trail
projection   = "xy"   # xy | xz | yz | 3d
glow         = true
theme        = "neon" # neon | amber | ice | mono
```

---

## Building and testing

```bash
# Run all unit and integration tests (no display required)
cargo test --lib --tests

# Run only attractor bound tests
cargo test --lib -- lorenz_stays_on_attractor

# Release build (binary)
cargo build --release --bin math-sonify

# Release build (VST3/CLAP plugin)
cargo build --release --lib

# Documentation
cargo doc --no-deps --open
```

The test suite covers: ODE solver accuracy (attractor bounds, energy conservation, synchronization thresholds), scale quantization, polyphony limits, config parsing and clamping, scene arranger timeline consistency, oscillator amplitude bounds, and ADSR envelope behavior.

---

## GUI layout

The application has five top-level tabs:

- **SYNTH** -- system selector, parameter sliders, sonification mode, scale, effects chain.
- **MIXER** -- per-layer volume/pan/ADSR, master effects (EQ, delay, chorus, reverb), VU meters, WAV export, clip save.
- **ARRANGE** -- scene timeline, morph time controls, AUTO arrangement generator, probability weights.
- **MATH VIEW** -- live phase portrait (XY/XZ/YZ/3D), bifurcation diagram, custom ODE text input, state readout.
- **WAVEFORM** -- oscilloscope and spectrum analyzer.

Performance mode (press `F`) switches to fullscreen phase portrait only.

---

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| `F` | Toggle fullscreen performance mode |
| `Space` | Pause / resume simulation |
| `R` | Reset attractor to default initial condition |
| `S` | Save clip (last 60 seconds as WAV + PNG) |
| `Ctrl+S` | Save current configuration to `config.toml` |
| `1` to `7` | Switch sonification mode (Direct, Orbital, Granular, Spectral, FM, Vocal, Waveguide) |
| `<` / `>` | Previous / next dynamical system |
| `Up` / `Down` | Increase / decrease simulation speed by 10% |
| `E` | Toggle Evolve (autonomous parameter wandering) |
| `A` | Toggle AUTO arrangement playback |
| `P` | Play / stop scene arranger |
| `Escape` | Exit fullscreen |

---

## Troubleshooting

### No audio / device not found

- math-sonify uses `cpal::default_host().default_output_device()`. Ensure a device is selected in OS audio settings.
- **Exclusive mode (Windows)**: close any application holding the device in exclusive mode (e.g., games, some audio interfaces).
- **ALSA errors (Linux)**: install `libasound2-dev` (`sudo apt install libasound2-dev`) and add your user to the `audio` group.
- **Sample rate mismatch**: set `sample_rate = 48000` in `config.toml` if you see an `AudioDeviceError`.

### High CPU usage

- Increase `buffer_size` to 1024 or 2048.
- Disable Evolve mode when not in use.
- For Three-Body and Lorenz96, reduce `system.speed`.

### Distorted audio

- Lower `audio.master_volume` (default 0.7).
- The lookahead limiter prevents true clipping; audible distortion usually means waveshaper drive is too high. Set `waveshaper_drive = 1.0` and `waveshaper_mix = 0.0`.

### Phase portrait is blank

- Wait 2-3 seconds after startup or after pressing `R` for the trail to build up.
- If the system diverges (NaN/Inf state), the engine resets automatically (logged as `OdeIntegrationError`).

### Config file not loading

- math-sonify looks for `config.toml` in the **current working directory**. Run the binary from the directory containing your config file.
- Parse errors are logged as warnings; invalid values are clamped to their valid range rather than rejected.

### VST3/CLAP plugin not appearing

- Ensure you copied the `.dll`/`.so` to the correct system VST3 folder and triggered a plugin rescan in your DAW.
- Some DAWs require a full restart after installing new plugins.
- The plugin must be built with `cargo build --release --lib`, not `--bin`.

---

## Contributing

1. Fork the repository and create a feature branch.
2. Run `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings` before pushing.
3. Add tests for any new public API (unit tests in the relevant module, integration tests in `tests/integration.rs`).
4. Open a pull request. The CI matrix (fmt, clippy, test, doc, release build, audit) must pass.

Code style notes:
- No `unsafe` except where unavoidable with a `#[allow(unsafe_code)]` comment explaining why.
- No `.unwrap()` or `.expect()` in non-test `src/` code; use `?` or explicit `Result` returns.
- Audio thread code must be real-time safe: no heap allocation, no blocking I/O, no `unwrap`.
- All public items must have `///` doc comments.

---

## License

MIT. See [LICENSE](LICENSE).

---

Built with [Rust](https://www.rust-lang.org), [cpal](https://github.com/RustAudio/cpal), [egui](https://github.com/emilk/egui), [nih-plug](https://github.com/robbert-vdh/nih-plug), [crossbeam](https://github.com/crossbeam-rs/crossbeam), [parking_lot](https://github.com/Amanieu/parking_lot), [hound](https://github.com/ruuda/hound), [tracing](https://github.com/tokio-rs/tracing).
