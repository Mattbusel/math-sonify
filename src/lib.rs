//! Math Sonify core library: dynamical systems, synthesis, sonification and effects.
//!
//! The desktop app (`src/main.rs`) and the VST3/CLAP plugin (`plugin/`) are both built
//! on these modules.

mod arrangement;
pub mod audio_driven;
pub mod config;
pub mod error;
pub mod hindmarsh_rose;
pub mod patches;
pub mod rossler;
pub mod sonification;
pub mod spectrum_analyzer;
pub mod synth;
pub mod systems;
pub mod vanderpol;
pub mod synthesis;
pub mod midi;
pub mod randomizer;
pub mod duffing;
pub mod zoo;
pub mod effects;
pub mod blend;
pub mod scale_mapper;
pub mod euclidean;
pub mod tuning;
pub mod markov_music;
pub mod signal_processing;
pub mod harmony_system;
pub mod rhythm_quantizer;
pub mod generative_counterpoint;
pub mod spectral_morph;
pub mod arpeggiator;
pub mod stochastic_composer;
pub mod binaural_beats;
pub mod sequencer;
pub mod chord_progression;
pub mod sonification_pipeline;
pub mod musical_analysis;
pub mod microtonal;
pub mod algorithmic_composer;
pub mod motif_composer;
pub mod tuning_system;
pub mod scale_mapper_v2;
pub mod live_input;
pub mod melody_generator;
pub mod dynamic_processor;
pub mod pitch_detector;
pub mod audio_analyzer;
pub mod wavetable_synth;
pub mod spatial_audio;
pub mod chord_voicing;
pub mod rhythm_engine;
pub mod midi_sequencer;
pub mod envelope_generator;
pub mod granular_synth;
pub mod score_renderer;
pub mod synthesis_engine;
pub mod music_theory;
pub mod audio_pipeline;
pub mod bifurcation;
pub mod collab;
pub mod collaboration;
pub mod composer;
pub mod fractal;
pub mod midi_export;
pub mod network;
pub mod osc_sync;
pub mod preset_gallery;
pub mod preset_interpolation;
pub mod recorder;

/// Every Rust example in the README is compiled by `cargo test --doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}
