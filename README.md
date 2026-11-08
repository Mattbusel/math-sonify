# math-sonify

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) [![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

A synth played by chaos math, live.

53 dynamical systems. 1,800+ tests.

- Plays Lorenz, Kuramoto and Three-Body as sound
- Runs as a desktop app or VST3/CLAP plugin
- Renders WAV with no screen
- Exports MIDI for any DAW

## Install

```bash
cargo install math-sonify
```

## Use

```bash
cargo run --release -- --headless --duration 60 --output clip.wav
```

[Latest release](https://github.com/Mattbusel/math-sonify/releases/latest)

More: [docs/DETAILS.md](docs/DETAILS.md)

License: MIT
