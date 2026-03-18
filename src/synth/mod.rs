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

pub mod oscillator;
pub mod filter;
pub mod reverb;
pub mod fdn_reverb;
pub mod delay;
pub mod envelope;
pub mod limiter;
pub mod grain;
pub mod bitcrusher;
pub mod karplus;
pub mod chorus;
pub mod waveshaper;
pub mod waveguide;
pub mod eq;

pub use oscillator::{Oscillator, OscShape};
pub use filter::BiquadFilter;
// Freeverb is used by the plugin lib (src/plugin.rs) via this re-export;
// the standalone binary uses FdnReverb instead, so the compiler warns here
// when building the binary target.
#[allow(unused_imports)]
pub use reverb::Freeverb;
pub use fdn_reverb::FdnReverb;
pub use delay::DelayLine;
pub use envelope::Adsr;
pub use limiter::Limiter;
pub use grain::GrainEngine;
pub use bitcrusher::Bitcrusher;
pub use karplus::KarplusStrong;
pub use chorus::Chorus;
pub use waveshaper::Waveshaper;
pub use waveguide::WaveguideString;
pub use eq::ThreeBandEq;
