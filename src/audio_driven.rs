//! Audio-driven ODE morphing — reverse sonification pipeline.
//!
//! Instead of the normal forward direction (ODE state → audio parameters),
//! this module reads *incoming* audio from the default input device, extracts
//! perceptual features from it, and uses those features to continuously adjust
//! the ODE parameters of the running attractor.  The result is a closed loop:
//! the environment drives the mathematics, which in turn drives the audio.
//!
//! # Architecture
//!
//! ```text
//! Microphone / line-in (cpal default input)
//!     |
//!     | PCM frames (f32, mono-mixed)
//!     v
//! AudioInputAnalyzer
//!     |  per-frame (configurable hop size):
//!     |    RMS  (loudness proxy)
//!     |    Spectral centroid (brightness / pitch proxy)
//!     |    Spectral flux (onset / transient strength)
//!     |    8-band energy bins
//!     v
//! AudioFeatures  (sent via crossbeam channel every hop)
//!     |
//!     v
//! AudioOdeBridge
//!     |  mapping rules (configurable):
//!     |    rms      → Lorenz σ   (louder = more chaos)
//!     |    centroid → Lorenz ρ   (brighter = higher ρ)
//!     |    flux     → system switch threshold
//!     v
//! ODE parameter patch (sent to the simulation thread)
//! ```
//!
//! # Dual mode
//!
//! [`DualMode`] wraps both the forward pipeline (ODE → audio output) and the
//! reverse pipeline (audio input → ODE parameters) and can run them
//! concurrently.  In `ForwardOnly` mode only the ODE → audio path is active;
//! in `ReverseOnly` the ODE is entirely driven by the microphone; in `Both`
//! the two paths run simultaneously — the ODE modulates synthesis while the
//! environment simultaneously tweaks its own parameters.
//!
//! # Usage
//!
//! ```rust,no_run
//! use math_sonify::audio_driven::{AudioInputAnalyzer, AudioOdeBridge, BridgeConfig};
//! use crossbeam_channel::unbounded;
//! use std::sync::{atomic::AtomicBool, Arc};
//!
//! let (feature_tx, feature_rx) = unbounded();
//! let (patch_tx, patch_rx) = unbounded();
//! let mut analyzer = AudioInputAnalyzer::new(BridgeConfig::default(), feature_tx);
//! let bridge = AudioOdeBridge::from_channels(BridgeConfig::default(), feature_rx, patch_tx);
//! std::thread::spawn(move || bridge.run_background(Arc::new(AtomicBool::new(false))));
//!
//! // Feed input samples from the audio callback:
//! analyzer.feed(0.0);
//!
//! // In the simulation loop:
//! for patch in patch_rx.try_iter() {
//!     // Apply patch.sigma, patch.rho, … to the ODE params
//! }
//! ```

#![allow(dead_code)]

use crossbeam_channel::{Receiver, Sender};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

// ── Public feature-extracted snapshot ────────────────────────────────────────

/// Per-frame audio features extracted by [`AudioInputAnalyzer`].
///
/// All values are normalised to `[0.0, 1.0]` relative to configurable
/// reference maxima so that the bridge mapping stays parameter-agnostic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioFeatures {
    /// Root-mean-square amplitude of the current frame (loudness proxy).
    pub rms: f32,
    /// Spectral centroid normalised by the Nyquist frequency (brightness proxy).
    pub centroid: f32,
    /// Spectral flux — L1 difference between successive magnitude spectra,
    /// normalised to `[0, 1]` by the configured max-flux reference.
    pub flux: f32,
    /// Eight equal-width energy bands from 0 Hz to Nyquist, each in `[0, 1]`.
    pub bands: [f32; 8],
}

impl Default for AudioFeatures {
    fn default() -> Self {
        Self {
            rms: 0.0,
            centroid: 0.5,
            flux: 0.0,
            bands: [0.0; 8],
        }
    }
}

// ── ODE parameter patch ───────────────────────────────────────────────────────

/// A set of ODE parameter overrides derived from the current audio features.
///
/// Only fields that have meaningfully changed (beyond `min_delta`) are
/// included; the receiving simulation thread can apply them selectively.
#[derive(Debug, Clone, PartialEq)]
pub struct OdePatch {
    /// Suggested Lorenz σ value (None = no change).
    pub sigma: Option<f64>,
    /// Suggested Lorenz ρ value (None = no change).
    pub rho: Option<f64>,
    /// Suggested Lorenz β value (None = no change).
    pub beta: Option<f64>,
    /// Whether the audio flux exceeded the system-switch threshold this frame.
    pub trigger_system_switch: bool,
    /// The raw features that generated this patch, for downstream diagnostics.
    pub features: AudioFeatures,
}

impl OdePatch {
    /// Returns `true` if the patch requests any ODE parameter change.
    pub fn is_empty(&self) -> bool {
        self.sigma.is_none()
            && self.rho.is_none()
            && self.beta.is_none()
            && !self.trigger_system_switch
    }
}

// ── Mapping configuration ─────────────────────────────────────────────────────

/// Configures how audio features map to ODE parameters.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    // ── σ (sigma) mapping: rms → [sigma_min, sigma_max] ──
    /// Lorenz σ at silence (rms = 0).
    pub sigma_min: f64,
    /// Lorenz σ at full loudness (rms = 1).
    pub sigma_max: f64,

    // ── ρ (rho) mapping: centroid → [rho_min, rho_max] ──
    /// Lorenz ρ when the spectrum is dark (centroid = 0).
    pub rho_min: f64,
    /// Lorenz ρ when the spectrum is bright (centroid = 1).
    pub rho_max: f64,

    // ── β (beta) mapping: band[3] energy ──
    /// Lorenz β at zero mid-band energy.
    pub beta_min: f64,
    /// Lorenz β at full mid-band energy.
    pub beta_max: f64,

    // ── System switching ──
    /// Flux threshold above which a system-switch event is emitted.
    pub flux_switch_threshold: f32,

    // ── Change suppression ──
    /// Minimum change in σ before a new patch is emitted (reduces noise).
    pub sigma_min_delta: f64,
    /// Minimum change in ρ before a new patch is emitted.
    pub rho_min_delta: f64,

    // ── Feature extraction ──
    /// FFT frame size (must be a power of two).
    pub fft_size: usize,
    /// Hop between successive analysis frames (in samples).
    pub hop_size: usize,
    /// Reference maximum RMS for normalisation (typical peak ≈ 0.3).
    pub rms_ref: f32,
    /// Reference maximum flux for normalisation.
    pub flux_ref: f32,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            sigma_min: 5.0,
            sigma_max: 30.0,
            rho_min: 15.0,
            rho_max: 60.0,
            beta_min: 1.5,
            beta_max: 4.0,
            flux_switch_threshold: 0.6,
            sigma_min_delta: 0.25,
            rho_min_delta: 0.5,
            fft_size: 1024,
            hop_size: 512,
            rms_ref: 0.3,
            flux_ref: 50.0,
        }
    }
}

// ── Audio feature extractor ───────────────────────────────────────────────────

/// Accumulates raw PCM samples and emits [`AudioFeatures`] on every hop.
///
/// This is a pure DSP object with no I/O; it is driven by a caller that
/// feeds samples one at a time via [`feed`].
pub struct AudioInputAnalyzer {
    config: BridgeConfig,
    /// Ring buffer for the current FFT frame.
    frame_buf: Vec<f32>,
    /// Write head in `frame_buf`.
    write_pos: usize,
    /// Sample count since the last hop.
    hop_counter: usize,
    /// Magnitude spectrum from the previous frame (for flux calculation).
    prev_magnitudes: Vec<f32>,
    /// Hann window coefficients.
    window: Vec<f32>,
    /// Output channel — pushed to every hop.
    tx: Sender<AudioFeatures>,
}

impl AudioInputAnalyzer {
    /// Create a new analyzer that will push features to `tx` every hop.
    pub fn new(config: BridgeConfig, tx: Sender<AudioFeatures>) -> Self {
        let n = config.fft_size;
        let window: Vec<f32> = (0..n)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos())
            })
            .collect();
        Self {
            frame_buf: vec![0.0; n],
            write_pos: 0,
            hop_counter: 0,
            prev_magnitudes: vec![0.0; n / 2 + 1],
            window,
            config,
            tx,
        }
    }

    /// Feed a single mono sample into the analyzer.
    ///
    /// When `hop_size` samples have accumulated since the last analysis,
    /// an FFT frame is computed and a [`AudioFeatures`] value is sent to
    /// the output channel.
    pub fn feed(&mut self, sample: f32) {
        let n = self.config.fft_size;
        self.frame_buf[self.write_pos % n] = sample;
        self.write_pos += 1;
        self.hop_counter += 1;

        if self.hop_counter >= self.config.hop_size {
            self.hop_counter = 0;
            self.analyze();
        }
    }

    /// Run analysis on the current frame and push features to the channel.
    fn analyze(&mut self) {
        let n = self.config.fft_size;

        // Build the windowed frame in order (accounting for ring-buffer wrap).
        let mut windowed: Vec<f32> = (0..n)
            .map(|i| {
                let buf_idx = (self.write_pos + i) % n;
                self.frame_buf[buf_idx] * self.window[i]
            })
            .collect();

        // In-place Cooley-Tukey FFT (radix-2 DIT, no external dependency).
        cooley_tukey_fft(&mut windowed);

        // Magnitude spectrum (first n/2+1 bins).
        let half = n / 2 + 1;
        let magnitudes: Vec<f32> = (0..half)
            .map(|k| {
                // `windowed` has been transformed in-place; real/imag are
                // interleaved by the in-place transform: real[k], imag[k].
                // For the simple magnitude we read from `windowed` treating
                // indices 0..n as [Re0, Im0, Re1, Im1, …].
                let re = windowed[2 * k % n];
                let im = if 2 * k + 1 < n { windowed[2 * k + 1] } else { 0.0 };
                (re * re + im * im).sqrt()
            })
            .collect();

        // RMS of the time-domain frame.
        let rms_raw = {
            let sum_sq: f32 = self.frame_buf.iter().map(|&s| s * s).sum();
            (sum_sq / n as f32).sqrt()
        };
        let rms = (rms_raw / self.config.rms_ref).clamp(0.0, 1.0);

        // Spectral centroid (frequency-weighted mean of magnitude).
        let total_mag: f32 = magnitudes.iter().sum();
        let centroid_raw = if total_mag > 1e-9 {
            magnitudes
                .iter()
                .enumerate()
                .map(|(k, &m)| k as f32 * m)
                .sum::<f32>()
                / total_mag
        } else {
            half as f32 / 2.0
        };
        let centroid = (centroid_raw / half as f32).clamp(0.0, 1.0);

        // Spectral flux (L1 difference from previous frame).
        let flux_raw: f32 = magnitudes
            .iter()
            .zip(self.prev_magnitudes.iter())
            .map(|(&a, &b)| (a - b).abs())
            .sum();
        let flux = (flux_raw / self.config.flux_ref).clamp(0.0, 1.0);

        // 8-band energies.
        let band_size = half / 8;
        let mut bands = [0.0f32; 8];
        for (b, band) in bands.iter_mut().enumerate() {
            let start = b * band_size;
            let end = ((b + 1) * band_size).min(half);
            let energy: f32 = magnitudes[start..end].iter().map(|&m| m * m).sum();
            let peak = if end > start {
                magnitudes[start..end].iter().cloned().fold(0.0f32, f32::max)
            } else {
                1.0
            };
            *band = if peak > 1e-9 {
                (energy / peak / (end - start) as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }

        self.prev_magnitudes.clone_from_slice(&magnitudes);

        let _ = self.tx.send(AudioFeatures {
            rms,
            centroid,
            flux,
            bands,
        });
    }
}

// ── Bridge: features → ODE patches ───────────────────────────────────────────

/// Maps a stream of [`AudioFeatures`] to [`OdePatch`] values for the ODE solver.
///
/// `AudioOdeBridge` owns a receive end of the feature channel and pushes
/// patches into a separate channel consumed by the simulation thread.
pub struct AudioOdeBridge {
    config: BridgeConfig,
    /// Receive incoming audio features.
    feature_rx: Receiver<AudioFeatures>,
    /// Emit ODE parameter patches.
    patch_tx: Sender<OdePatch>,
    /// Last emitted σ value (for delta suppression).
    last_sigma: f64,
    /// Last emitted ρ value (for delta suppression).
    last_rho: f64,
    /// Last emitted β value (for delta suppression).
    last_beta: f64,
}

impl AudioOdeBridge {
    /// Create a bridge from the given feature receiver to the patch sender.
    pub fn from_channels(
        config: BridgeConfig,
        feature_rx: Receiver<AudioFeatures>,
        patch_tx: Sender<OdePatch>,
    ) -> Self {
        let sigma_init = (config.sigma_min + config.sigma_max) / 2.0;
        let rho_init = (config.rho_min + config.rho_max) / 2.0;
        let beta_init = (config.beta_min + config.beta_max) / 2.0;
        Self {
            config,
            feature_rx,
            patch_tx,
            last_sigma: sigma_init,
            last_rho: rho_init,
            last_beta: beta_init,
        }
    }

    /// Process one [`AudioFeatures`] frame and optionally emit a patch.
    pub fn process_frame(&mut self, f: AudioFeatures) {
        let cfg = &self.config;

        // σ: linear interpolation rms → [sigma_min, sigma_max].
        let sigma = cfg.sigma_min + (cfg.sigma_max - cfg.sigma_min) * f.rms as f64;
        // ρ: centroid → [rho_min, rho_max].
        let rho = cfg.rho_min + (cfg.rho_max - cfg.rho_min) * f.centroid as f64;
        // β: mid-band (band[3]) energy → [beta_min, beta_max].
        let beta = cfg.beta_min + (cfg.beta_max - cfg.beta_min) * f.bands[3] as f64;

        let sigma_changed = (sigma - self.last_sigma).abs() >= cfg.sigma_min_delta;
        let rho_changed = (rho - self.last_rho).abs() >= cfg.rho_min_delta;
        let beta_changed = (beta - self.last_beta).abs() >= 0.05;
        let flux_triggered = f.flux >= cfg.flux_switch_threshold;

        if sigma_changed || rho_changed || beta_changed || flux_triggered {
            if sigma_changed {
                self.last_sigma = sigma;
            }
            if rho_changed {
                self.last_rho = rho;
            }
            if beta_changed {
                self.last_beta = beta;
            }

            let patch = OdePatch {
                sigma: if sigma_changed { Some(sigma) } else { None },
                rho: if rho_changed { Some(rho) } else { None },
                beta: if beta_changed { Some(beta) } else { None },
                trigger_system_switch: flux_triggered,
                features: f,
            };

            let _ = self.patch_tx.send(patch);
        }
    }

    /// Drain all pending features from the channel and process them.
    ///
    /// Call this from the simulation thread's tick function.
    pub fn drain(&mut self) {
        while let Ok(f) = self.feature_rx.try_recv() {
            self.process_frame(f);
        }
    }

    /// Spawn a background thread that loops over the feature channel.
    ///
    /// This variant is for use when the bridge runs in its own thread.
    pub fn run_background(mut self, stop: Arc<AtomicBool>) {
        thread::Builder::new()
            .name("audio-ode-bridge".into())
            .spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if let Ok(f) = self.feature_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                        self.process_frame(f);
                    }
                }
            })
            .expect("audio-ode-bridge thread");
    }
}

// ── Dual mode ─────────────────────────────────────────────────────────────────

/// Selects which direction(s) of the audio–ODE coupling are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualModeKind {
    /// Classic: ODE state drives audio synthesis only.
    ForwardOnly,
    /// Reverse: incoming audio drives ODE parameters only.
    ReverseOnly,
    /// Both pipelines are active simultaneously.
    Both,
}

impl Default for DualModeKind {
    fn default() -> Self {
        Self::ForwardOnly
    }
}

/// Top-level controller for the dual-mode audio–ODE coupling.
///
/// Holds configuration for both the forward synthesis path and the reverse
/// audio-input path, and tracks which direction(s) are currently enabled.
pub struct DualMode {
    /// Currently active mode.
    pub kind: DualModeKind,
    /// Configuration for the reverse (audio-input → ODE) path.
    pub bridge_config: BridgeConfig,
    /// Stop flag for the background thread (if running).
    stop: Arc<AtomicBool>,
}

impl DualMode {
    /// Create a new dual-mode controller, initially in `ForwardOnly` mode.
    pub fn new(bridge_config: BridgeConfig) -> Self {
        Self {
            kind: DualModeKind::ForwardOnly,
            bridge_config,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Switch to a new mode.
    pub fn set_mode(&mut self, kind: DualModeKind) {
        self.kind = kind;
        log::info!("[audio_driven] mode changed to {:?}", kind);
    }

    /// Returns `true` if the reverse (audio-input) path should be active.
    pub fn reverse_active(&self) -> bool {
        matches!(self.kind, DualModeKind::ReverseOnly | DualModeKind::Both)
    }

    /// Returns `true` if the forward (ODE → audio) path should be active.
    pub fn forward_active(&self) -> bool {
        matches!(self.kind, DualModeKind::ForwardOnly | DualModeKind::Both)
    }

    /// Build and wire an [`AudioInputAnalyzer`] + [`AudioOdeBridge`] pair,
    /// returning the [`Receiver<OdePatch>`] for the simulation thread and a
    /// stop flag that can be used to terminate the bridge thread.
    ///
    /// The caller is responsible for feeding audio samples to the
    /// returned [`AudioInputAnalyzer`], or for using cpal to do so.
    pub fn build_reverse_pipeline(
        &self,
    ) -> (AudioInputAnalyzer, Receiver<OdePatch>, Arc<AtomicBool>) {
        let (feat_tx, feat_rx) = crossbeam_channel::unbounded::<AudioFeatures>();
        let (patch_tx, patch_rx) = crossbeam_channel::unbounded::<OdePatch>();
        let stop = Arc::clone(&self.stop);

        let analyzer = AudioInputAnalyzer::new(self.bridge_config.clone(), feat_tx);
        let bridge =
            AudioOdeBridge::from_channels(self.bridge_config.clone(), feat_rx, patch_tx);
        bridge.run_background(Arc::clone(&stop));

        (analyzer, patch_rx, stop)
    }

    /// Signal the background bridge thread to stop.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

// ── Minimal in-place Cooley-Tukey FFT (no external dependency) ───────────────

/// In-place radix-2 Cooley-Tukey FFT.
///
/// `buf` must have an even length (power of two is optimal). After the call,
/// `buf[2k]` and `buf[2k+1]` contain the real and imaginary parts of bin `k`.
/// For inputs shorter than 2 bins the function is a no-op.
fn cooley_tukey_fft(buf: &mut Vec<f32>) {
    let n = buf.len();
    if n < 2 {
        return;
    }

    // Bit-reversal permutation on pairs (real, imag) packed as flat array.
    // We treat the input as n/2 complex numbers stored as [Re0, Im0, Re1, Im1, ...].
    // For a real-valued signal Im_k = 0 initially.
    // Expand to complex pairs first.
    let half = n / 2;
    // Re-interpret: make a complex vector of length `half` treating even indices
    // as real and odd indices as imaginary (input is real so all Im = 0).
    // We rebuild `buf` as interleaved complex from the real-only input.
    // Since buf was already real (single-channel samples), set imaginary parts to 0.
    // We work with a separate complex buffer to keep things clear.
    let mut c: Vec<(f32, f32)> = buf[..half]
        .iter()
        .map(|&re| (re, 0.0))
        .collect();

    // Bit-reversal.
    let bits = (half as f32).log2() as usize;
    for i in 0..half {
        let j = bit_reverse(i, bits);
        if i < j {
            c.swap(i, j);
        }
    }

    // Cooley-Tukey butterfly stages.
    let mut len = 2usize;
    while len <= half {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let w_re = ang.cos();
        let w_im = ang.sin();
        let mut k = 0;
        while k < half {
            let (mut wr, mut wi) = (1.0f32, 0.0f32);
            for j in 0..len / 2 {
                let (ur, ui) = c[k + j];
                let vr = wr * c[k + j + len / 2].0 - wi * c[k + j + len / 2].1;
                let vi = wr * c[k + j + len / 2].1 + wi * c[k + j + len / 2].0;
                c[k + j] = (ur + vr, ui + vi);
                c[k + j + len / 2] = (ur - vr, ui - vi);
                let new_wr = wr * w_re - wi * w_im;
                wi = wr * w_im + wi * w_re;
                wr = new_wr;
            }
            k += len;
        }
        len *= 2;
    }

    // Write back as interleaved (re, im) into buf.
    for (i, (re, im)) in c.iter().enumerate() {
        if 2 * i < n {
            buf[2 * i] = *re;
        }
        if 2 * i + 1 < n {
            buf[2 * i + 1] = *im;
        }
    }
}

/// Reverse the lowest `bits` bits of `x`.
fn bit_reverse(x: usize, bits: usize) -> usize {
    let mut result = 0usize;
    let mut v = x;
    for _ in 0..bits {
        result = (result << 1) | (v & 1);
        v >>= 1;
    }
    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_channels() -> (Sender<AudioFeatures>, Receiver<AudioFeatures>) {
        crossbeam_channel::unbounded()
    }

    #[test]
    fn audio_features_default() {
        let f = AudioFeatures::default();
        assert_eq!(f.rms, 0.0);
        assert_eq!(f.bands, [0.0; 8]);
    }

    #[test]
    fn analyzer_feeds_without_panic() {
        let (tx, _rx) = make_channels();
        let cfg = BridgeConfig {
            fft_size: 16,
            hop_size: 8,
            ..Default::default()
        };
        let mut analyzer = AudioInputAnalyzer::new(cfg, tx);
        for i in 0..32 {
            analyzer.feed((i as f32 * 0.01).sin());
        }
    }

    #[test]
    fn bridge_maps_rms_to_sigma_range() {
        let (feat_tx, feat_rx) = crossbeam_channel::unbounded::<AudioFeatures>();
        let (patch_tx, patch_rx) = crossbeam_channel::unbounded::<OdePatch>();
        let cfg = BridgeConfig::default();
        let sigma_min = cfg.sigma_min;
        let sigma_max = cfg.sigma_max;

        let mut bridge = AudioOdeBridge::from_channels(cfg, feat_rx, patch_tx);

        // Feed a max-loudness frame.
        let _ = feat_tx.send(AudioFeatures {
            rms: 1.0,
            centroid: 0.5,
            flux: 0.0,
            bands: [0.0; 8],
        });
        bridge.drain();

        if let Ok(patch) = patch_rx.try_recv() {
            if let Some(sigma) = patch.sigma {
                assert!(
                    sigma >= sigma_min && sigma <= sigma_max + 1e-6,
                    "sigma {sigma} out of range [{sigma_min}, {sigma_max}]"
                );
            }
        }
    }

    #[test]
    fn bridge_triggers_switch_on_high_flux() {
        let (feat_tx, feat_rx) = crossbeam_channel::unbounded::<AudioFeatures>();
        let (patch_tx, patch_rx) = crossbeam_channel::unbounded::<OdePatch>();
        let cfg = BridgeConfig {
            flux_switch_threshold: 0.5,
            ..Default::default()
        };

        let mut bridge = AudioOdeBridge::from_channels(cfg, feat_rx, patch_tx);

        let _ = feat_tx.send(AudioFeatures {
            rms: 0.0,
            centroid: 0.5,
            flux: 0.9,
            bands: [0.0; 8],
        });
        bridge.drain();

        let patch = patch_rx.try_recv().expect("patch should be emitted");
        assert!(patch.trigger_system_switch, "high flux should trigger switch");
    }

    #[test]
    fn dual_mode_forward_active_by_default() {
        let mode = DualMode::new(BridgeConfig::default());
        assert!(mode.forward_active());
        assert!(!mode.reverse_active());
    }

    #[test]
    fn dual_mode_both() {
        let mut mode = DualMode::new(BridgeConfig::default());
        mode.set_mode(DualModeKind::Both);
        assert!(mode.forward_active());
        assert!(mode.reverse_active());
    }

    #[test]
    fn bit_reverse_identity_for_zero() {
        assert_eq!(bit_reverse(0, 8), 0);
    }

    #[test]
    fn ode_patch_is_empty_when_no_changes() {
        let p = OdePatch {
            sigma: None,
            rho: None,
            beta: None,
            trigger_system_switch: false,
            features: AudioFeatures::default(),
        };
        assert!(p.is_empty());
    }
}
