# Synthesis Primitives Reference

All synth modules live under `src/synth/`. They are stateful structs initialized once per audio engine startup and driven sample-by-sample in the audio callback (`src/audio.rs`).

---

## Oscillator
**File:** `src/synth/oscillator.rs`

Band-limited wavetable oscillator with PolyBLEP anti-aliasing.

```rust
pub struct Oscillator { /* ... */ }

impl Oscillator {
    pub fn new(freq: f32, shape: OscShape, sample_rate: f32) -> Self
    pub fn next_sample(&mut self) -> f32
    pub fn set_freq(&mut self, freq: f32)
    pub fn set_shape(&mut self, shape: OscShape)
}
```

### OscShape variants
| Variant | Description |
|---------|-------------|
| `Sine` | Pure sinusoid; no aliasing |
| `Sawtooth` | Rising ramp; PolyBLEP-corrected at wraparound |
| `Square` | 50% duty; PolyBLEP at both transitions |
| `Triangle` | Integrated PolyBLEP square; −12 dB/oct rolloff |
| `Noise` | White noise via xorshift64 |

**PolyBLEP** correction is applied at phase discontinuities using normalized phase t ∈ [0, 1) and dt = freq/sample_rate. Reduces aliasing by ~40 dB compared to naive waveforms.

---

## SmoothParam
**File:** `src/synth/oscillator.rs`

Eliminates zipper noise from sudden parameter changes by applying a one-pole IIR to the target value.

```rust
pub struct SmoothParam { /* ... */ }

impl SmoothParam {
    pub fn new(initial: f32, smoothing_ms: f32, sample_rate: f32) -> Self
    pub fn set(&mut self, target: f32)
    pub fn next(&mut self) -> f32
}
```

Converges to within 0.1% of target in `smoothing_ms` milliseconds.

---

## ADSR Envelope
**File:** `src/synth/envelope.rs`

Four-stage envelope generator. Attack is linear; decay and release use exponential curves.

```rust
pub struct Adsr { /* ... */ }

impl Adsr {
    pub fn new(attack_ms: f32, decay_ms: f32, sustain: f32, release_ms: f32, sr: f32) -> Self
    pub fn trigger(&mut self)           // begin attack
    pub fn release(&mut self)           // begin release
    pub fn is_idle(&self) -> bool       // true when fully silent
    pub fn next_sample(&mut self) -> f32
    pub fn update_params(&mut self, attack_ms: f32, decay_ms: f32, sustain: f32, release_ms: f32, sr: f32)
}
```

| Parameter | Description |
|-----------|-------------|
| `attack_ms` | Linear ramp from 0 → 1 |
| `decay_ms` | Exponential fall from 1 → `sustain` |
| `sustain` | Hold level [0, 1] |
| `release_ms` | Exponential fall from `sustain` → 0 |

Exponential coefficient: `coeff = exp(−ln(1000) / samples)` — reaches 0.1% of target in the specified time.

---

## Biquad Filter
**File:** `src/synth/filter.rs`

Transposed direct form II biquad with low-pass and band-pass modes.

```rust
pub struct BiquadFilter { /* ... */ }

impl BiquadFilter {
    pub fn low_pass(cutoff_hz: f32, q: f32, sample_rate: f32) -> Self
    pub fn band_pass(center_hz: f32, q: f32, sample_rate: f32) -> Self
    pub fn process(&mut self, x: f32) -> f32
    pub fn update_lp(&mut self, cutoff_hz: f32, q: f32, sample_rate: f32)
    pub fn update_bp(&mut self, center_hz: f32, q: f32, sample_rate: f32)
    pub fn reset_if_nan(&mut self)      // sanitize state if NaN/Inf
}
```

| Parameter | Description |
|-----------|-------------|
| `cutoff_hz` / `center_hz` | −3 dB point or resonant center |
| `q` | Quality factor (0.707 = Butterworth flatness) |

State variables z1, z2 are sanitized if NaN/Inf to prevent audio blowup.

---

## Karplus-Strong Plucked String
**File:** `src/synth/karplus.rs`

Physical model of a plucked string using a delay-line feedback loop.

```rust
pub struct KarplusStrong { /* ... */ }

impl KarplusStrong {
    pub fn new(max_freq_hz: f32, sample_rate: f32) -> Self
    pub fn trigger(&mut self, freq: f32, sample_rate: f32)
    pub fn next_sample(&mut self) -> f32

    pub decay: f32,      // per-loop gain; 0 < decay < 1
    pub brightness: f32, // IIR coeff: 0 = bright, 0.5 = balanced, 0.85 = dark
    pub stretch: f32,    // allpass stiffness coefficient
    pub active: bool,
    pub volume: f32,
}
```

**Loop chain:** fractional-delay read → IIR lowpass → allpass dispersion → gain feedback.
**Fractional delay:** linear interpolation for exact intonation at all frequencies.
**Auto-silence:** stops when peak < 1e-6 to avoid running forever.

---

## Digital Waveguide String
**File:** `src/synth/waveguide.rs`

Two-directional traveling-wave string model with frequency and brightness control.

```rust
pub struct WaveguideString { /* ... */ }

impl WaveguideString {
    pub fn new(sample_rate: f32) -> Self
    pub fn set_freq(&mut self, hz: f32)
    pub fn next_sample(&mut self) -> f32

    pub tension: f32,    // [0, 1] → frequency ×[0.25, 4.0] (unity at 0.5)
    pub damping: f32,    // feedback gain; 0.995 ≈ guitar-like decay
    pub brightness: f32, // IIR loop filter: 0 = bright, 0.9 = dark
    pub dispersion: f32, // allpass stiffness: 0 = ideal, 0.3 = piano-like
    pub excite: bool,    // set true to inject new noise burst
    pub excite_pos: f32, // [0, 1] position along string where burst is injected
}
```

**Loop chain:** fractional read (fwd + bck) → IIR lowpass → allpass → damping → write back.
**Buffer:** fixed `MAX_DELAY = 4096` samples; supports fundamentals down to ~10 Hz at 44.1 kHz.

---

## Grain Engine
**File:** `src/synth/grain.rs`

Granular synthesis: up to 96 simultaneous Hann-windowed sine grains.

```rust
pub struct GrainEngine { /* ... */ }

impl GrainEngine {
    pub fn new(sample_rate: f32) -> Self
    pub fn next_sample(&mut self) -> (f32, f32)  // stereo (L, R)
    pub fn set_base_freq(&mut self, hz: f32)

    pub spawn_rate: f32,    // grains per second [1, 300]
    pub base_freq: f32,     // center frequency in Hz
    pub freq_spread: f32,   // random detune in semitones
}
```

**Per-grain:** random phase, random detuning, random pan, random duration (40–220 ms), Hann window.
**Occasional harmonic shifts:** 25% octave down, 15% perfect fifth up, 60% unison.
**Output normalization:** `0.6 / √active_grains` — correct RMS loudness for incoherent grains.

---

## FDN Reverb
**File:** `src/synth/fdn_reverb.rs`

8-channel Feedback Delay Network reverb with pre-delay and LFO-modulated delay lines.

```rust
pub struct FdnReverb { /* ... */ }

impl FdnReverb {
    pub fn new(sample_rate: f32) -> Self
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32)

    pub feedback: f32,  // loop gain; 0.88 default
    pub damping: f32,   // first-order LP in each channel; 0.25 default
    pub wet: f32,       // wet/dry mix; 0.4 default
}
```

**Pre-delay:** 10 ms before entering the FDN (separates dry from tail).
**Delay lengths:** 8 coprime delays scaled from [1559, 3761] samples at 44.1 kHz.
**LFO modulation:** individual sinusoidal offset per channel (< 2 Hz, < 5 samples depth) — breaks metallic resonances.
**Diffusion:** Walsh-Hadamard transform (WHT8) — energy-preserving, scatter pattern.
**True stereo:** L feeds even channels, R feeds odd channels.
**Safety:** NaN guard; clamps output to [−10, 10] per channel per write.

---

## Reverb (Freeverb)
**File:** `src/synth/reverb.rs`

Classic Schroeder/Freeverb reverberator: comb filters + allpass diffusers.

```rust
pub struct Reverb { /* ... */ }

impl Reverb {
    pub fn new(sample_rate: f32) -> Self
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32)
    pub fn set_room_size(&mut self, size: f32)
    pub fn set_damping(&mut self, damp: f32)
    pub fn set_wet(&mut self, wet: f32)
}
```

---

## Delay Line
**File:** `src/synth/delay.rs`

Stereo feedback delay with configurable time and feedback.

```rust
pub struct DelayLine { /* ... */ }

impl DelayLine {
    pub fn new(max_ms: f32, sample_rate: f32) -> Self
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32)

    pub delay_ms: f32,
    pub feedback: f32,
    pub wet: f32,
}
```

---

## Chorus
**File:** `src/synth/chorus.rs`

Multi-tap modulated delay for stereo thickening.

```rust
pub struct Chorus { /* ... */ }

impl Chorus {
    pub fn new(sample_rate: f32) -> Self
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32)

    pub rate: f32,   // LFO frequency in Hz
    pub depth: f32,  // modulation depth in samples
    pub mix: f32,    // wet level
}
```

---

## Three-Band EQ
**File:** `src/synth/eq.rs`

Low-shelf, parametric mid, and high-shelf biquad EQ.

```rust
pub struct ThreeBandEq { /* ... */ }

impl ThreeBandEq {
    pub fn new(sample_rate: f32) -> Self
    pub fn process(&mut self, x: f32) -> f32
    pub fn update(&mut self, low_gain_db: f32, mid_gain_db: f32, high_gain_db: f32, sample_rate: f32)
}
```

---

## Bitcrusher
**File:** `src/synth/bitcrusher.rs`

Sample-and-hold bit depth and sample-rate reduction.

```rust
pub struct Bitcrusher { /* ... */ }

impl Bitcrusher {
    pub fn new(seed: u64) -> Self
    pub fn process(&mut self, x: f32) -> f32

    pub bit_depth: f32,   // effective bits [1, 32]; 16 = CD quality
    pub rate_crush: f32,  // rate reduction [0, 1]; 0 = bypass
}
```

**Dither:** TPDF noise added before quantization to prevent granulation artifacts.
**Decorrelated seeds:** each audio layer gets a unique seed via `new_with_index(sr, layer_idx)` so stereo dither noise doesn't produce audible beating.

---

## Waveshaper
**File:** `src/synth/waveshaper.rs`

Soft-clipping saturation via a tanh-like nonlinearity.

```rust
pub struct Waveshaper { /* ... */ }

impl Waveshaper {
    pub fn new() -> Self
    pub fn process(&mut self, x: f32) -> f32

    pub drive: f32,  // gain before clipping [1, 100]
    pub mix: f32,    // wet/dry [0, 1]
}
```

**Compensation:** output is divided by `sqrt(drive)` to approximately normalize perceived loudness.
**DC blocking:** first-order high-pass (100 Hz) removes DC offset introduced by asymmetric driving.

---

## Limiter
**File:** `src/synth/limiter.rs`

Lookahead peak limiter to prevent clipping at the master output.

```rust
pub struct Limiter { /* ... */ }

impl Limiter {
    pub fn new(sample_rate: f32) -> Self
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32)

    pub threshold: f32,  // peak ceiling [0, 1]; default 0.95
}
```

**Lookahead:** 1-sample delay buffer separates detection from processing (avoids transient overshoot).
**Gain smoothing:** fast attack (0.001 coeff), slow release (0.0001 coeff) — minimizes pumping artifacts.
**NaN guard:** envelope resets to 0 if non-finite.

---

## Effects Signal Chain

The per-layer and master effects are applied in this order inside `audio.rs`:

```
Oscillators / Grains / Karplus / Waveguide
    → per-voice ADSR
    → Biquad Filter (cutoff modulated by sonification)
    → Three-Band EQ
    → Waveshaper (saturation)
    → Bitcrusher
    → per-layer mix
↓
Layer sum  ×3 layers
    → Chorus
    → Delay
    → FDN Reverb
    → Limiter
    → master volume
    → output
```
