//! DSP building blocks used by the audio engine.
//!
//! Each module is a self-contained unit processor that accepts `f32` audio
//! samples and exposes a `process` (or `next_sample`) method.  All processors
//! are real-time safe: no heap allocation after construction, no blocking I/O,
//! no `unwrap` / `panic!` on the hot path.
//!
//! # Signal chain (per layer)
//!
//! ```text
//! Oscillator(s) --> ADSR --> Waveshaper --> Bitcrusher
//!                                                    \
//!                                                     --> Master bus:
//!                                                     BiquadFilter (LP) --> DelayLine
//!                                                     --> Chorus --> Freeverb --> Limiter
//! ```

// Synth primitives are used via dynamic dispatch from the audio engine;
// the compiler can't always see through the call graph, hence these suppressions.
#![allow(dead_code)]

pub mod bitcrusher;
pub mod chorus;
pub mod delay;
pub mod envelope;
pub mod eq;
pub mod fdn_reverb;
pub mod filter;
pub mod grain;
pub mod karplus;
pub mod limiter;
pub mod oscillator;
pub mod reverb;
pub mod resonator;
pub mod waveguide;
pub mod waveshaper;

pub use filter::BiquadFilter;
pub use oscillator::{OscShape, Oscillator};
// Freeverb is used by the plugin lib (src/plugin.rs) via this re-export;
// the standalone binary uses FdnReverb instead, so the compiler warns here
// when building the binary target.
pub use bitcrusher::Bitcrusher;
pub use chorus::Chorus;
pub use delay::DelayLine;
pub use envelope::Adsr;
pub use eq::ThreeBandEq;
pub use fdn_reverb::FdnReverb;
pub use grain::GrainEngine;
pub use karplus::KarplusStrong;
pub use limiter::Limiter;
#[allow(unused_imports)]
pub use resonator::ResonatorBank;
#[allow(unused_imports)]
pub use reverb::Freeverb;
pub use waveguide::WaveguideString;
pub use waveshaper::Waveshaper;
