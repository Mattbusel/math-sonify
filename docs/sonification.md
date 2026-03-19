# Sonification Modes Reference

Sonification is the bridge between the dynamical system and the audio engine. Each mode implements the `Sonification` trait:

```rust
pub trait Sonification {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams;
}
```

`state` is the current system state vector, `speed` is the instantaneous trajectory speed (‖Δstate/dt‖), and `config` carries the musical parameters (scale, base frequency, etc.).

The returned `AudioParams` struct drives the `LayerSynth` audio engine every control-rate tick (120 Hz).

---

## AudioParams Fields (selected)

| Field | Type | Description |
|-------|------|-------------|
| `freqs[4]` | `[f32; 4]` | Voice frequencies in Hz |
| `amps[4]` | `[f32; 4]` | Voice amplitudes [0, 1] |
| `pans[4]` | `[f32; 4]` | Voice pan positions [−1, 1] |
| `filter_cutoff` | `f32` | Biquad cutoff in Hz |
| `filter_q` | `f32` | Biquad resonance |
| `grain_spawn_rate` | `f32` | Grains per second |
| `grain_base_freq` | `f32` | Grain center frequency Hz |
| `grain_freq_spread` | `f32` | Grain detune in semitones |
| `partials[32]` | `[f32; 32]` | Additive partial amplitudes |
| `partials_base_freq` | `f32` | Additive fundamental Hz |
| `fm_carrier_freq` | `f32` | FM carrier frequency Hz |
| `fm_mod_ratio` | `f32` | FM modulator/carrier ratio |
| `fm_mod_index` | `f32` | FM modulation depth |
| `chaos_level` | `f32` | Estimated chaos [0, 1] |
| `mode` | `SonifMode` | Active synthesis mode enum |

---

## Scale Quantization

All pitch-based modes optionally quantize frequencies to a musical scale via `quantize_to_scale(t, base_hz, octave_range, scale)`, which maps a normalized value t ∈ [0, 1] to the nearest scale degree over the configured octave span.

| Scale | Notes per octave | Character |
|-------|-----------------|-----------|
| `pentatonic` | 5 | Consonant, always pleasant |
| `chromatic` | 12 | Full chromatic; can be dissonant |
| `just` | 12 | Pure integer ratios; clear harmonics |
| `microtonal` | 24 (quarter-tones) | Fine pitch shading |
| `edo19` | 19 | Excellent minor thirds |
| `edo31` | 31 | Rich just approximations |
| `edo24` | 24 | Quarter-tone chromatic |
| `whole_tone` | 6 | Dreamy, floating |
| `phrygian` | 7 | Dark, Spanish-sounding |
| `lydian` | 7 | Bright, otherworldly |

---

## Mode 1: Direct Mapping
**File:** `src/sonification/direct.rs`
**Enum:** `SonifMode::Direct`

The simplest mapping: each state dimension drives one voice.

**Algorithm:**
1. Normalize each state variable using an exponential moving-average min/max tracker (α = 0.001 — adjusts over ~1000 samples to the attractor's natural range).
2. Map normalized value → frequency via `quantize_to_scale`.
3. Amplitude = `0.5 + 0.5·t` — always audible, louder when near extremes.
4. Pan = linear map of normalized state to [−1, 1].
5. Filter cutoff driven by the last available state dimension (300–4000 Hz).

**Voices:** up to 4, one per state dimension.
**Gain:** 0.25
**Best for:** Understanding what the attractor is doing; clear one-to-one correspondence between mathematics and pitch.

---

## Mode 2: FM Synthesis
**File:** `src/sonification/fm.rs`
**Enum:** `SonifMode::FM`

Uses frequency modulation where the attractor drives carrier pitch and modulation index.

**Algorithm:**
1. Carrier frequency: state[0] tanh-normalized, scale-quantized.
2. Modulator ratio: state[1] mapped to [1.0, 7.0] — integer ratios give harmonic tones, non-integer gives bells/metallic.
3. Modulation index: derived from `speed`, weighted by chaos estimate (‖state[0:3]‖/50), clamped to [0.1, 20.0].
4. High index → dense sidebands (bright/harsh); low index → near-pure sine (calm).

**Gain:** 0.5
**Best for:** Systems with rich speed variation (Lorenz, Duffing). Index sweeps from pure to screaming with chaos level.

---

## Mode 3: Orbital Resonance
**File:** `src/sonification/orbital.rs`
**Enum:** `SonifMode::Orbital`

Treats the trajectory as a rotating body; uses angular velocity for pitch.

**Algorithm:**
1. Compute instantaneous angle: θ = atan2(state[1], state[0]).
2. Track angular velocity dθ/dt with phase unwrapping.
3. Smooth fundamental over ~400 ms (EMA α = 0.005).
4. Generate up to 4 partials: `fₙ = f₁ · n^(1 + stretch·0.35)` where stretch ∈ [0, 1].
5. Harmonic profile: interpolate between **string** (harmonic, 1/n amplitude falloff) and **bell** (inharmonic, chaos-modulated) based on a Lyapunov-like divergence estimate.
6. Filter Q widens with chaos (0.5 → 2.5 resonance).

**Voices:** 4 partials + 1 sub-octave voice (from state[2] if available).
**Gain:** 0.15–0.35 (speed-scaled)
**Best for:** Lorenz, Rössler, Halvorsen — systems with coherent orbital motion that transitions to chaos. Sound quality shifts from string-like to bell-like as chaos increases.

---

## Mode 4: Granular
**File:** `src/sonification/granular.rs`
**Enum:** `SonifMode::Granular`

Maps trajectory speed and state to granular synthesis parameters.

**Algorithm:**
1. `grain_spawn_rate` = speed clamped to [5, 200] grains/sec.
2. `grain_base_freq` = state[0] min/max normalized → scale-quantized Hz.
3. `grain_freq_spread` = state[1] normalized → [0, 2.0] semitones of random detuning per grain.
4. `chaos_level` = speed / 200.0.

**Gain:** 0.4
**Best for:** Mackey-Glass, CML, Kuramoto — systems that spend time near slow orbits interrupted by fast bursts. Dense grain clouds during high-speed bursts; sparse during calm periods.

---

## Mode 5: Spectral
**File:** `src/sonification/spectral.rs`
**Enum:** `SonifMode::Spectral`

Computes a real DFT over the recent trajectory history and maps the spectrum to additive synthesis partials.

**Algorithm:**
1. Maintain a 64-point ring buffer of trajectory history (all state dimensions summed).
2. Compute a 32-bin real DFT over the buffer using a cosine/sine recurrence for efficiency.
3. Apply a −12 dB/oct natural spectral slope: `rolloff[k] = 1.0 / (1.0 + (k as f32)^1.5 · 0.08)`.
4. Smooth each partial amplitude with EMA (α = 0.05) to prevent clicks from rapid changes.
5. Feed into `partials[32]` for additive synthesis.

**Base frequency:** From `config.base_frequency` (default 220 Hz).
**Gain:** 0.30
**Best for:** Systems with interesting spectral content (Coupled Map Lattice, Lorenz 96). The harmonic balance shifts as the attractor changes geometry — rich overtones during chaos, pure tones during order.

---

## Mode 6: Vocal / Formant
**File:** `src/sonification/vocal.rs`
**Enum:** `SonifMode::Vocal`

Maps the trajectory through a vowel space defined by six vowel formants.

**Vowel formants (Hz):**
| Vowel | F1 | F2 | F3 |
|-------|----|----|----|
| /a/ | 800 | 1200 | 2500 |
| /e/ | 400 | 2000 | 2600 |
| /i/ | 300 | 2300 | 3000 |
| /o/ | 500 | 900 | 2500 |
| /u/ | 300 | 800 | 2300 |
| /æ/ | 700 | 1700 | 2600 |

**Algorithm:**
1. `state[0]` cycles through the vowel array (position modulo 6 with smooth interpolation).
2. `state[1]` controls interpolation blend rate.
3. Three band-pass resonators at (F1, F2, F3) form the vowel color; amplitudes 0.8, 0.5, 0.3.
4. Breathiness = EMA-smoothed speed with floor 0.08 — the attractor always "breathes" slightly.
5. Filter cutoff = F1, Q = 5.0 for resonant vowel character.

**Gain:** 0.3
**Best for:** Systems that wander slowly through state space (Nose-Hoover, Van der Pol). The sound morphs through recognizable vowel shapes as the trajectory evolves — uncanny, almost biological.

---

## Chord Modes

All modes support harmonic chord stacking on top of the primary voice:

| Mode | Intervals | Description |
|------|-----------|-------------|
| `none` | — | Single voice only |
| `power` | +7 semitones | Power chord (5th) |
| `octave` | +12 semitones | Octave doubling |
| `major` | +4, +7 | Major triad |
| `minor` | +3, +7 | Minor triad |
| `sus2` | +2, +7 | Suspended 2nd |
| `dom7` | +4, +7, +10 | Dominant 7th |

---

## Portamento

All frequency changes pass through a portamento smoother:
- Time constant: `config.portamento_ms` (default 80 ms, range 1–5000 ms).
- Implemented as a one-pole IIR: `freq += coeff · (target - freq)` per sample.
- At 80 ms: smooth pitch glides; at 1 ms: nearly instant; at 1000+ ms: extreme slow glissando.
