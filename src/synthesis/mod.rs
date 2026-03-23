//! Higher-level synthesis instruments built on top of the `synth` DSP primitives.
//!
//! This module groups "instrument-level" synthesis modes that consume the full
//! dynamical system state vector and produce audio samples.  Lower-level DSP
//! building blocks (oscillators, filters, reverb, etc.) live in `crate::synth`.

pub mod physical;

pub use physical::{build_physical_synth, PhysicalMode, PhysicalSynth, PluckedString, TubeResonator};
