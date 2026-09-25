# Math Sonify — Changelog

## [1.4.0] - 2026-09-25

### Added

- Prebuilt downloads: a release workflow builds the desktop app for Windows (x86_64),
  macOS (Apple Silicon and Intel) and Linux (x86_64) on every `v*` tag and attaches
  them, with `SHA256SUMS.txt`, to the GitHub Release.
- `--help` and `--version` flags.
- `cargo install math-sonify` support: the VST3/CLAP plugin moved into its own
  unpublished workspace crate (`plugin/`) because nih-plug is not on crates.io.

### Changed

- The core library is now `math_sonify` (`src/lib.rs`); the plugin crate builds
  `math_sonify_plugin` with `cargo build --release -p math-sonify-plugin`.
- CI builds and tests the whole workspace on Linux, Windows and macOS and installs the
  Linux audio and windowing libraries it needs.

### Fixed

- Test suite compiles again (`evolution` and `preset_gallery` tests).
- `collab`: `set` messages dropped their last parameter because the parser matched the
  outer closing brace.
- `beat_tracker`: tempo estimation always returned 60 BPM; it now picks the dominant
  inter-onset interval.
- `randomizer`: the chaos check ran too briefly for any Lorenz trajectory to pass.
- Broken doc examples in `audio_driven` and `duffing`, and four tests with wrong
  expectations (arpeggiator LFO, DC blocker, spectrum DC bin, Rule 90 symmetry).

## [Unreleased] — Production-ready pass (2026-03-18)

### Fixed

- Removed all compiler warnings from both the binary and lib targets:
  - Unused imports (`AudioParams`, `chord_intervals_for`, `Bitcrusher`, `GrainEngine`,
    `AudioConfig`, `Config`, `LorenzConfig`, `std::f32::consts::TAU`) in `src/plugin.rs`.
  - `#[allow(dead_code)]` with explanatory comments added to `PluginDsp` fields reserved
    for the upcoming chord-mode feature (`chord_oscs`, `chord_amp_smooth`, etc.).
  - `reverb::Freeverb` re-export in `src/synth/mod.rs` annotated with `#[allow(unused_imports)]`
    and a comment explaining the plugin-only usage.
  - Unused variable `noise_val` in `src/main.rs` prefixed with `_`.
  - Dead field `poincare_z_prev` in `src/ui.rs` annotated with `#[allow(dead_code)]`.
  - Dead function `system_internal_name` in `src/ui.rs` annotated with `#[allow(dead_code)]`.

### Added

- Comprehensive unit tests (`src/synth/oscillator.rs`): sine at phase zero = 0,
  quarter-period peak near 1, square wave ±1 midpoint, sawtooth range, higher-frequency
  shorter period, amplitude scaling.
- Comprehensive unit tests (`src/synth/envelope.rs`): idle = 0, attack rises,
  decay falls to sustain, sustain is constant, release falls to 0, zero-duration
  stages do not panic.
- Comprehensive unit tests (`src/synth/filter.rs`): low-pass passes DC,
  low-pass attenuates high frequencies, band-pass peaks at center, all outputs finite,
  NaN input cleared safely.
- Unit tests (`src/error.rs`): all variants display non-empty strings, `From<io::Error>`
  produces `IoError`, display contains the original message.
- Public helpers `map_to_frequency` and `map_to_amplitude` in `src/sonification/direct.rs`
  with unit tests covering audible range, unit range, and monotonicity.
- DSP integration tests appended to `tests/integration.rs`: 1-second audio buffer is
  non-zero, stereo channels equal, two in-phase oscillators double amplitude,
  `DirectMapping` on Lorenz trajectory yields non-zero frequencies.
- CI workflow (`.github/workflows/ci.yml`) updated to run `cargo check`, `cargo test --lib`
  (lib-only to avoid audio device dependency), `cargo clippy -- -D warnings`, and
  `cargo fmt --check` with dependency caching.

## [1.2.0] - 2026-03-18

### Added

- `tracing` + `tracing-subscriber` instrumentation throughout the engine:
  - `main()`: startup span with version, loaded system name, and dt.
  - `AudioEngine::start`: logs sample rate and sample format at `info` level.
  - `save_clip`: logs exported file path and sample count at `info` level.
  - Subscribers respect `RUST_LOG` environment variable (default `info`).
- `[lints.clippy]` table in `Cargo.toml`: `unwrap_used`, `expect_used`, `panic`,
  `indexing_slicing` promoted to `warn`; false-positive DSP lints suppressed via
  `allow`.
- `[profile.release]` extended: `codegen-units = 1`, `strip = "debuginfo"`,
  `panic = "abort"` for smaller and faster release builds.
- `[profile.bench]` added for reproducible benchmarking builds.
- `exclude` list in `[package]` to keep `cargo package` output clean.
- `tracing` and `tracing-subscriber` added to `[dependencies]`.
- CI workflow (`.github/workflows/ci.yml`) rewritten with seven jobs:
  - `fmt`: `cargo fmt --all -- --check`
  - `clippy`: `cargo clippy --lib --tests -- -D warnings` on Ubuntu/Windows matrix
  - `test`: `cargo test --lib --tests` on Ubuntu/Windows matrix
  - `doc`: `cargo doc --no-deps --lib` with `RUSTDOCFLAGS="-D warnings"`
  - `build-release`: release binary + library on Ubuntu/Windows matrix with artifact upload
  - `audit`: `cargo audit` for known CVEs
  - `fuzz`: 30-second `cargo fuzz run fuzz_systems` smoke test on nightly
- Additional `///` doc comments on all public items previously lacking them:
  `BiquadFilter`, `Freeverb`, `DelayLine`, `Limiter`, `Chorus`, `Adsr`,
  `Waveshaper`, `ThreeBandEq`, `KarplusStrong`, `WaveguideString`,
  `SnippetPlayback`, `AudioEngine`, `Preset`, `load_preset`.
- New integration tests in `tests/integration.rs` (13 additional test functions):
  - `lorenz_deterministic_trajectory`, `lorenz_zero_dt_no_change`
  - `quantize_to_scale_t_zero_equals_base`, `quantize_to_scale_all_scales_finite_positive`
  - `polyphony_all_voices_finite_and_non_negative`
  - `config_default_is_already_valid`, `config_round_trip_lossless`
  - `sonif_mode_display_non_empty`, `sonif_mode_default_is_direct`
  - `chord_intervals_major_and_minor`, `chord_intervals_dom7_three_notes`,
    `chord_intervals_unknown_returns_zeros`
- README rewritten: what math sonification is, architecture diagram, supported
  systems table with equations, sonification mode reference table, audio output
  formats section, quickstart, full configuration reference, troubleshooting,
  contributing guidelines.

### Changed

- `Cargo.toml`: version bumped to 1.2.0; `authors` field includes email;
  `exclude` list added.

## [1.1.0] - 2026-03-17

### Added

- `tests/integration.rs`: External integration test suite exercising the public
  library API. Covers:
  - Lorenz attractor bounding box (`lorenz_stays_on_attractor`, `lorenz_z_stays_positive`)
  - Rossler attractor boundedness
  - Double pendulum Hamiltonian energy conservation (< 2% drift over 10 000 steps)
  - Kuramoto synchronization transition: incoherence below K_c, synchronization above
  - Three-body Hamiltonian energy conservation (leapfrog, < 1% drift)
  - Scale quantization producing audible-range and valid MIDI-range frequencies for all 10 scales
  - Polyphony limits: exactly 4 voice slots, zero-amp for voices beyond state dimension
  - Config: empty TOML parses to defaults, out-of-range values are clamped, unknown
    fields are silently ignored
- `src/tests.rs` — 22 additional unit tests appended:
  - `lorenz_stays_on_attractor` (50 000-step bounding-box test, the one referenced in README)
  - `lorenz_z_stays_positive`
  - `rossler_stays_bounded_30000_steps`
  - `double_pendulum_energy_conserved_small_angles` (Yoshida symplectic integrator)
  - `double_pendulum_state_stays_finite_and_bounded`
  - `kuramoto_below_critical_coupling_incoherent`
  - `kuramoto_above_critical_coupling_synchronizes`
  - `kuramoto_order_parameter_in_unit_interval`
  - `three_body_energy_conserved`
  - `scale_quantization_produces_valid_midi_notes`
  - `polyphony_limit_four_voices_max`
  - `polyphony_default_voice_levels_descending`
- `src/plugin.rs`: promoted `systems`, `sonification`, `synth`, and `config` modules
  from `mod` to `pub mod` so that `tests/integration.rs` can access the public API.

## [1.0.0] - 2026-03-17

### Added

- `src/error.rs`: `SonifyError` enum using `thiserror`, covering `AudioDeviceError`,
  `OdeIntegrationError`, `ConfigError`, `PluginError`, `RenderError`, and `IoError`.
  `SonifyResult<T>` convenience alias. The audio callback and plugin `process()`
  never panic; errors are logged and produce silence.
- Expanded test suite in `src/tests.rs`:
  - ODE accuracy: Lorenz attractor bounded after 50 000 steps, Rossler near-period
    closure, Duffing bounded trajectory, Kuramoto synchronization above/below
    critical coupling, Three-Body Hamiltonian conservation (leapfrog, error < 1%).
  - Parameter morphing: intermediate values, quarter-point interpolation, master
    volume floor enforced at all t values.
  - Audio synthesis: oscillator amplitude bounds for all five waveform shapes,
    SmoothParam zero stability, SmoothParam convergence rate, sample-rate
    independence via zero-crossing count.
  - Config: all sections parse from a full TOML string; out-of-range values are
    clamped to defaults.
  - Scene arranger: zero duration with no active scenes, None past end, monotonic
    morph fraction, single-scene never in morph state.
- CI workflow (`.github/workflows/ci.yml`): four jobs (`fmt`, `clippy`, `test`,
  `build-release`), each on `ubuntu-latest` and `windows-latest`. GUI tests
  excluded via `--skip`.
- `///` doc comments on all public items lacking them: `Scene`, `lerp_config`,
  `chord_intervals_for`, `SonifMode`, `Scale`, `Sonification`, `Lorenz`,
  `Rossler`, `Duffing`, `Kuramoto`, `ThreeBody`.
- Module-level doc comment on `sonification/mod.rs` with mode reference table.

### Changed

- `Cargo.toml`: version 0.1.0 to 1.0.0; added `thiserror = "1"` dependency.
- README rewritten: architecture ASCII diagram, supported systems with equations,
  quickstart (standalone, headless, plugin build), full configuration reference,
  GUI layout description. No em dashes, no emojis.
- CI workflow updated from Ubuntu-only to matrix on Ubuntu and Windows.

---

## v0.9.1 — The Polish
*The boring work that separates a project from a product.*

Every control now has a tooltip. Every preset is audible at launch. The AUTO button shows what it's doing in real time. The fractional Lorenz system no longer crashes. The VST identifies itself correctly to your DAW.

---

## v0.9 — The Attractor Speaks
*Everything on the roadmap, all at once.*

**The attractor speaks.** New vocal synthesis mode maps the trajectory to vowel formant positions. The chaotic system wanders through vowel space — /a/ /e/ /i/ /o/ /u/ — producing evolving textures that sound human and inhuman simultaneously. Stack with chorus and reverb for ghostly choir textures from pure math.

**Physical modeling.** Waveguide synthesis where the attractor modulates the string's tension, damping, and length in real-time. A violin string whose tension is chaotic. A tube whose bore is being reshaped by a three-body orbit. Sounds that are acoustically realistic in texture but physically impossible in behavior.

**Fractional calculus.** The Lorenz system with derivative order α ∈ [0.5, 1.0]. Below 1.0, the system depends on its entire history. This is genuinely active research territory — the sonic character of fractional-order dynamics has barely been studied.

**Type any equation.** Write your own ODE system in three text fields. Any attractor from any paper, sonified in seconds. A mathematician publishes a new system — you type it, you hear it.

**Two attractors in conversation.** Coupled systems let two attractors co-evolve. The Rössler's x-output modulates the Lorenz's ρ in real-time. The interaction produces dynamics neither system contains alone.

**Live looper.** Record, loop, layer. Four tracks. Build a full arrangement in real time, on stage, from nothing.

**The piece is non-deterministic.** Probabilistic scene transitions let you assign weights to each scene. The structure of the piece becomes random at the macro level while remaining deterministic within each moment.

**MIDI in.** Connect a keyboard. Map velocity to ρ, pitch to coupling, aftertouch to FM index. Navigate a chaotic parameter space with muscle memory you already have.

**Red-blue glasses.** Anaglyph 3D phase portrait rendering. The attractor becomes a volumetric object floating in front of you.

---

## v0.8 — Simple
*Someone who has never heard of a Lorenz attractor should be able to make something beautiful in under a minute.*

Four macro knobs replace thirty parameters. Chaos. Space. Rhythm. Warmth. Each maps to the underlying physics in ways that always sound musical.

The AUTO button generates a full 8-scene arrangement matched to your mood and starts playing it. One click.

Evolve mode runs a random walk on all four macros simultaneously. Leave it running. In ten minutes you'll have sounds you couldn't have designed manually.

---

## v0.7.2 — Multi-Layer + Plugin
*Three attractors running simultaneously. Math Sonify as a DAW instrument.*

Multi-layer polyphony: up to three independent attractor systems with independent mix, pan, and ADSR. All three feed one shared effects chain.

VST3/CLAP plugin: 17 automatable parameters. Full MIDI note-on/off with velocity-sensitive envelopes.

30 presets across 6 evocative categories. Save Clip: one button captures the last 60 seconds of audio as WAV plus a 512×512 phase portrait PNG.

---

## v0.1 — It Makes Sound
*Differential equations running in real time, mapped to audio.*

The Lorenz attractor. Pentatonic scale. RK4 integration at 120Hz. The butterfly spiral sounds like itself.
