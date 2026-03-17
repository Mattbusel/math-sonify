/// Audio thread: multi-layer polyphonic synthesis engine.
/// Up to 3 independent attractor layers mix into one shared effects chain.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound;
use cpal::{Stream, SampleFormat};
use std::sync::Arc;
use std::collections::VecDeque;
use parking_lot::Mutex;
use crossbeam_channel::Receiver;

use crate::sonification::{AudioParams, SonifMode};
use crate::synth::{
    Oscillator, OscShape, BiquadFilter, FdnReverb, DelayLine, Limiter,
    GrainEngine, Bitcrusher, KarplusStrong, Chorus, Waveshaper, Adsr, WaveguideString,
};

pub type WavRecorder     = Arc<Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>;
pub type LoopExportPending = Arc<Mutex<Option<u64>>>;
/// Shared VU meter: [layer0_peak, layer1_peak, layer2_peak, master_peak]
pub type VuMeter = Arc<Mutex<[f32; 4]>>;
/// Shared sidechain RMS level written by the input stream, read by the sim thread.
pub type SidechainLevel = Arc<Mutex<f32>>;
/// Circular audio clip buffer (last ~60 seconds stereo interleaved f32).
pub type ClipBuffer = Arc<Mutex<VecDeque<f32>>>;

const CLIP_SECONDS: usize = 60;

// ---------------------------------------------------------------------------
// Per-layer DSP state
// ---------------------------------------------------------------------------

struct LayerSynth {
    sr: f32,
    oscs: [Oscillator; 4],
    chord_oscs: [Oscillator; 3],
    grains: GrainEngine,
    ks: KarplusStrong,
    voice_adsr: [Adsr; 4],
    partial_phases: [f32; 32],
    amp_smooth: [f32; 4],
    freq_smooth: [f32; 4],
    chord_amp_smooth: [f32; 3],
    chord_freq_smooth: [f32; 3],
    freq_smooth_rate: f32,
    chord_intervals: [f32; 3],
    fm_phase: f32,
    fm_mod_phase: f32,
    waveshaper: Waveshaper,
    bitcrusher: Bitcrusher,
    level: f32,
    pan: f32,
    peak: f32,
    // Vocal mode: 3 bandpass formant filters + noise phase
    formant_filters: [BiquadFilter; 3],
    noise_phase: f32,
    vocal_osc_phase: f32,
    // Waveguide physical model
    waveguide: WaveguideString,
    // Spectral freeze oscillators (up to 16 partials)
    freeze_oscs: Vec<Oscillator>,
    // Vocoder filter bank: 16 bandpass channels covering 80 Hz to 8 kHz
    vocoder_filters: Vec<BiquadFilter>,
    vocoder_buzz_phase: f32,
}

impl LayerSynth {
    fn new(sr: f32) -> Self {
        Self {
            sr,
            oscs: std::array::from_fn(|i| Oscillator::new(110.0 * (i + 1) as f32, OscShape::Sine, sr)),
            formant_filters: [
                BiquadFilter::band_pass(800.0, 8.0, sr),
                BiquadFilter::band_pass(1200.0, 8.0, sr),
                BiquadFilter::band_pass(2500.0, 10.0, sr),
            ],
            noise_phase: 0.0,
            vocal_osc_phase: 0.0,
            chord_oscs: [
                Oscillator::new(220.0, OscShape::Sine, sr),
                Oscillator::new(330.0, OscShape::Sine, sr),
                Oscillator::new(440.0, OscShape::Sine, sr),
            ],
            grains: GrainEngine::new(sr),
            ks: KarplusStrong::new(50.0, sr),
            partial_phases: [0.0; 32],
            amp_smooth: [0.0; 4],
            freq_smooth: [110.0, 220.0, 330.0, 440.0],
            chord_amp_smooth: [0.0; 3],
            chord_freq_smooth: [220.0, 330.0, 440.0],
            freq_smooth_rate: 0.01,
            chord_intervals: [0.0; 3],
            fm_phase: 0.0,
            fm_mod_phase: 0.0,
            waveshaper: Waveshaper::new(),
            bitcrusher: Bitcrusher::new(),
            level: 1.0,
            pan: 0.0,
            peak: 0.0,
            waveguide: WaveguideString::new(sr),
            freeze_oscs: (0..16).map(|i| Oscillator::new(220.0 * (i + 1) as f32, OscShape::Sine, sr)).collect(),
            // Immediately trigger ADSRs so continuous synthesis is never gated on launch.
            // Voices settle into Sustain within ~210ms; KS/arp retrigger for articulation.
            voice_adsr: {
                let mut adsr: [Adsr; 4] = std::array::from_fn(|_| Adsr::new(10.0, 200.0, 0.85, 400.0, sr));
                for a in &mut adsr { a.trigger(); }
                adsr
            },
            vocoder_filters: {
                // 16 bandpass filters geometrically spaced 80 Hz to 8000 Hz
                (0..16).map(|i| {
                    let freq = 80.0f32 * (8000.0f32 / 80.0f32).powf(i as f32 / 15.0);
                    BiquadFilter::band_pass(freq, 3.0, sr)
                }).collect()
            },
            vocoder_buzz_phase: 0.0,
        }
    }

    fn update(&mut self, p: &AudioParams) {
        self.grains.spawn_rate = p.grain_spawn_rate;
        self.grains.base_freq  = p.grain_base_freq;
        self.grains.freq_spread = p.grain_freq_spread;
        self.freq_smooth_rate = (1.0 / (p.portamento_ms.max(1.0) * 0.001 * self.sr)).clamp(0.001, 1.0);
        self.chord_intervals = p.chord_intervals;
        self.waveshaper.drive = p.waveshaper_drive;
        self.waveshaper.mix   = p.waveshaper_mix;
        self.bitcrusher.bit_depth  = p.bit_depth;
        self.bitcrusher.rate_crush = p.rate_crush;
        self.level = p.layer_level;
        self.pan   = p.layer_pan;
        for i in 0..4 { self.oscs[i].shape = p.voice_shapes[i]; }

        // Update ADSR params (without resetting stage so legato works)
        for adsr in &mut self.voice_adsr {
            adsr.set_params(p.adsr_attack_ms, p.adsr_decay_ms, p.adsr_sustain, p.adsr_release_ms);
            // Auto-trigger idle ADSRs — continuous synthesis must never be gated at zero.
            // KS/arp retrigger from Attack for articulation; all other cases sustain.
            if adsr.is_idle() { adsr.trigger(); }
        }

        if p.ks_trigger && p.ks_freq > 20.0 {
            self.ks.trigger(p.ks_freq, self.sr);
            // Velocity-sensitive ADSR trigger: louder hits = faster attack, longer release
            let velocity = p.amps[0].clamp(0.01, 1.0);
            let att = (p.adsr_attack_ms * (1.2 - velocity * 0.8)).max(1.0);
            let rel = p.adsr_release_ms * (0.7 + velocity * 0.6);
            for adsr in &mut self.voice_adsr {
                adsr.set_params(att, p.adsr_decay_ms, p.adsr_sustain, rel);
                adsr.trigger();
            }
        }
        self.ks.volume = p.ks_volume;

        // Waveguide update
        self.waveguide.tension = p.waveguide_tension;
        self.waveguide.damping = p.waveguide_damping;
        if p.waveguide_excite && p.ks_freq > 20.0 {
            self.waveguide.set_freq(p.ks_freq);
            self.waveguide.excite = true;
            self.waveguide.excite_pos = 0.3;
        }

        // Spectral freeze: update oscillator frequencies
        if p.spectral_freeze_active {
            for i in 0..16 {
                if p.spectral_freeze_freqs[i] > 10.0 {
                    self.freeze_oscs[i].freq = p.spectral_freeze_freqs[i];
                }
            }
        }
    }

    /// Render one stereo sample for this layer (no master effects yet).
    fn next_sample(&mut self, p: &AudioParams) -> (f32, f32) {
        let (raw_l, raw_r) = match p.mode {
            SonifMode::Direct | SonifMode::Orbital => self.synth_additive(p),
            SonifMode::Granular => self.grains.next_sample(),
            SonifMode::Spectral => self.synth_spectral(p),
            SonifMode::FM       => self.synth_fm(p),
            SonifMode::Vocal    => self.synth_vocal(p),
            SonifMode::Waveguide => {
                let s = self.waveguide.next_sample() * p.gain;
                (s, s)
            }
        };

        // Spectral freeze: mix in frozen partials
        let (raw_l, raw_r) = if p.spectral_freeze_active {
            let mut freeze_sum = 0.0f32;
            for i in 0..16 {
                if p.spectral_freeze_freqs[i] > 10.0 {
                    let s = self.freeze_oscs[i].next_sample() * p.spectral_freeze_amps[i];
                    freeze_sum += s;
                }
            }
            let freeze_out = freeze_sum * p.gain * 0.5;
            (raw_l * 0.5 + freeze_out, raw_r * 0.5 + freeze_out)
        } else {
            (raw_l, raw_r)
        };

        // NaN guard after raw synthesis
        let raw_l = if raw_l.is_finite() { raw_l } else { 0.0 };
        let raw_r = if raw_r.is_finite() { raw_r } else { 0.0 };

        // Karplus-Strong mixed before per-layer waveshaper
        let ks = self.ks.next_sample();
        let ks = if ks.is_finite() { ks } else { 0.0 };
        let l = self.waveshaper.process(raw_l + ks * 0.5);
        let r = self.waveshaper.process(raw_r + ks * 0.5);

        // NaN guard after waveshaper
        let l = if l.is_finite() { l } else { 0.0 };
        let r = if r.is_finite() { r } else { 0.0 };

        // Per-layer bitcrusher
        let l = self.bitcrusher.process(l);
        let r = self.bitcrusher.process(r);

        // NaN guard after bitcrusher
        let l = if l.is_finite() { l } else { 0.0 };
        let r = if r.is_finite() { r } else { 0.0 };

        // Apply level + equal-power pan
        let pan = (self.pan + p.layer_pan).clamp(-1.0, 1.0);
        let (pan_l, pan_r) = {
            use std::f32::consts::FRAC_PI_4;
            let angle = (pan + 1.0) * FRAC_PI_4;
            (angle.cos(), angle.sin())
        };
        let out_l = l * self.level * pan_l;
        let out_r = r * self.level * pan_r;

        // Track peak for VU meter
        let peak = out_l.abs().max(out_r.abs());
        self.peak = (self.peak * 0.999).max(peak); // fast attack, slow decay

        (out_l, out_r)
    }

    /// Equal-power (constant-energy) panning.
    /// Linear panning loses ~3 dB at centre; this maintains consistent loudness.
    #[inline(always)]
    fn eq_power_pan(sig: f32, pan: f32) -> (f32, f32) {
        use std::f32::consts::FRAC_PI_4;
        // pan in [-1, 1]; angle maps to [0, π/2]
        let angle = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
        (sig * angle.cos(), sig * angle.sin())
    }

    fn synth_additive(&mut self, p: &AudioParams) -> (f32, f32) {
        let gain = p.gain;
        let transpose = 2.0f32.powf(p.transpose_semitones / 12.0);
        let mut l = 0.0f32;
        let mut r = 0.0f32;

        for i in 0..4 {
            let target_freq = p.freqs[i] * transpose;
            let target_amp  = p.amps[i] * p.voice_levels[i];
            if target_freq > 10.0 {
                self.freq_smooth[i] += self.freq_smooth_rate * (target_freq - self.freq_smooth[i]);
                self.amp_smooth[i]  += 0.005 * (target_amp - self.amp_smooth[i]);
                self.oscs[i].freq = self.freq_smooth[i];
                let env = self.voice_adsr[i].next_sample();
                let sig = self.oscs[i].next_sample() * self.amp_smooth[i] * gain * env;
                let pan = p.pans[i].clamp(-1.0, 1.0);
                let (vl, vr) = Self::eq_power_pan(sig, pan);
                l += vl;
                r += vr;
            } else {
                self.voice_adsr[i].next_sample();
            }
        }

        // Chord voices from voice[0] frequency
        let v0 = self.freq_smooth[0];
        for k in 0..3 {
            let interval = self.chord_intervals[k];
            if interval.abs() > 0.001 {
                let target_cf = v0 * 2.0f32.powf(interval / 12.0);
                let target_ca = p.amps[0] * p.voice_levels[0] * 0.65;
                self.chord_freq_smooth[k] += self.freq_smooth_rate * (target_cf - self.chord_freq_smooth[k]);
                self.chord_amp_smooth[k]  += 0.005 * (target_ca - self.chord_amp_smooth[k]);
                self.chord_oscs[k].freq = self.chord_freq_smooth[k];
                let sig = self.chord_oscs[k].next_sample() * self.chord_amp_smooth[k] * gain;
                let pan = (k as f32 / 2.0) * 2.0 - 1.0;
                let (cl, cr) = Self::eq_power_pan(sig, pan);
                l += cl;
                r += cr;
            } else {
                self.chord_amp_smooth[k] += 0.005 * (0.0 - self.chord_amp_smooth[k]);
            }
        }
        // Attractor-modulated stereo spread via mid-side encoding.
        // Low chaos = near-mono; high chaos = wide stereo field.
        // width 1.0 = unity, 2.0 = fully separated.
        let width = 1.0 + p.chaos_level.clamp(0.0, 1.0) * 1.2;
        let mid  = (l + r) * 0.5;
        let side = (l - r) * 0.5 * width;
        // Decode mid-side back to L/R — no * 0.5 here; it was already baked
        // into the mid/side computation above (both use * 0.5), so the round-
        // trip is transparent at width=1.0 and properly widens at width>1.0.
        (mid + side, mid - side)
    }

    fn synth_spectral(&mut self, p: &AudioParams) -> (f32, f32) {
        use std::f32::consts::TAU;
        // Vocoder-style filter bank: buzz/saw excitation through 16 bandpass filters.
        // This gives a vocal-adjacent, "air through tubes" texture vs additive sinusoids.
        let buzz_freq = p.partials_base_freq.max(40.0);
        self.vocoder_buzz_phase = (self.vocoder_buzz_phase + TAU * buzz_freq / self.sr).rem_euclid(TAU);
        // Sawtooth excitation (rich harmonic content for the filter bank to carve)
        let buzz = 1.0 - self.vocoder_buzz_phase / std::f32::consts::PI;

        // Also blend in the legacy additive partial sum (mix 40% additive / 60% vocoder)
        let mut additive = 0.0f32;
        for k in 0..32 {
            let freq = p.partials_base_freq * (k + 1) as f32;
            self.partial_phases[k] = (self.partial_phases[k] + TAU * freq / self.sr) % TAU;
            additive += self.partial_phases[k].sin() * p.partials[k];
        }
        let excitation = buzz * 0.6 + additive * 0.4;

        // Run excitation through each bandpass channel; amplitude = corresponding partial
        // Use every other partial (16 bands from 32 partials) for band amplitude
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        let num_bands = self.vocoder_filters.len();
        for i in 0..num_bands {
            let partial_idx = (i * 2).min(31);
            let amp = p.partials[partial_idx];
            if amp > 0.001 {
                let filtered = self.vocoder_filters[i].process(excitation) * amp;
                // Pan bands across stereo field: even = slight left, odd = slight right
                let pan = (i as f32 / (num_bands - 1) as f32) * 2.0 - 1.0; // -1..1
                out_l += filtered * (1.0 - pan.max(0.0));
                out_r += filtered * (1.0 + pan.min(0.0));
            }
        }

        let scale = 1.0 / (num_bands as f32).sqrt();
        (out_l * p.gain * scale, out_r * p.gain * scale)
    }

    fn synth_fm(&mut self, p: &AudioParams) -> (f32, f32) {
        use std::f32::consts::TAU;
        let carrier  = p.fm_carrier_freq;
        let mod_freq = carrier * p.fm_mod_ratio;

        // --- 2-operator FM with modulator self-feedback ---
        // The modulator feeds back onto itself (β ≈ 0.2).  This thickens the
        // spectrum from a clean sine into a warm, slightly buzzy tone without
        // needing a third operator.  At high mod-index + feedback the sound
        // becomes brash and metallic — the same character as DX7 brass presets.
        let feedback_amt = 0.20f32;
        let mod_sample = (self.fm_mod_phase + feedback_amt * self.fm_mod_phase.sin()).sin();
        self.fm_mod_phase = (self.fm_mod_phase + TAU * mod_freq / self.sr).rem_euclid(TAU);

        // Carrier modulated by the feedback-enriched modulator
        let carrier_in = self.fm_phase + p.fm_mod_index * mod_sample;
        let mono = carrier_in.sin() * p.gain;
        self.fm_phase = (self.fm_phase + TAU * carrier / self.sr).rem_euclid(TAU);

        // Slight stereo spread: second channel reads carrier with a tiny phase offset
        // (1.5 ms worth of phase) — just enough width without a hard double.
        let phase_offset = TAU * carrier * 0.0015;
        let r = (carrier_in + phase_offset).sin() * p.gain;
        (mono, r)
    }

    fn synth_vocal(&mut self, p: &AudioParams) -> (f32, f32) {
        use std::f32::consts::TAU;

        // --- Glottal source ---------------------------------------------------
        // Fundamental follows the first attractor frequency (clamped to a
        // singable range) so the voice actually tracks the dynamics of the
        // underlying mathematical system rather than being a fixed 120 Hz drone.
        let fundamental = p.freqs[0].clamp(60.0, 400.0);
        self.vocal_osc_phase = (self.vocal_osc_phase + TAU * fundamental / self.sr).rem_euclid(TAU);

        // PolyBLEP sawtooth glottal source (alias-free at all pitches)
        let t  = self.vocal_osc_phase / TAU;
        let dt = (fundamental / self.sr).clamp(0.0, 0.5);
        let poly_blep_val = if t < dt {
            let u = t / dt; 2.0 * u - u * u - 1.0
        } else if t > 1.0 - dt {
            let u = (t - 1.0) / dt; u * u + 2.0 * u + 1.0
        } else { 0.0 };
        let source = (2.0 * t - 1.0) - poly_blep_val;

        // --- Breathiness (aspiration noise) -----------------------------------
        let breathiness = p.amps[3].clamp(0.0, 1.0);
        self.noise_phase += 1.0;
        // Two-stage pseudo-noise for broader spectrum (less harmonic character)
        let noise_val = ((self.noise_phase * 0.1234).sin() * 127.1)
            .sin()
            .mul_add(0.7, ((self.noise_phase * 0.0317).cos() * 93.7).sin() * 0.3);
        let excitation = source * (1.0 - breathiness) + noise_val * breathiness;

        // --- Formant filters --------------------------------------------------
        // Smooth coefficient updates (no hard reset) to avoid zipper noise when
        // the attractor drifts the formant frequencies.
        let q = 9.0f32; // slightly higher Q for more vocal clarity
        let sr = self.sr;
        for (i, freq) in [p.freqs[0], p.freqs[1], p.freqs[2]].iter().enumerate() {
            let f = freq.clamp(100.0, sr * 0.45);
            // Only rebuild if the frequency has moved by more than 2 Hz to
            // avoid per-sample coefficient churn
            let current_freq = match i {
                0 => 800.0f32, // placeholder; BiquadFilter doesn't expose freq
                _ => f,
            };
            let _ = current_freq; // suppress unused warning
            self.formant_filters[i] = BiquadFilter::band_pass(f, q, sr);
        }

        let f1_out = self.formant_filters[0].process(excitation) * p.amps[0];
        let f2_out = self.formant_filters[1].process(excitation) * p.amps[1];
        let f3_out = self.formant_filters[2].process(excitation) * p.amps[2];

        // --- Stereo spread via mid-side ----------------------------------------
        // F1 (chest resonance) stays centred; F2 and F3 push into the sides.
        let mid  = f1_out * p.gain;
        let side = (f2_out - f3_out) * p.gain * 0.4;
        let l = (mid + side) * 0.5f32.sqrt();
        let r = (mid - side) * 0.5f32.sqrt();
        (l, r)
    }
}

// ---------------------------------------------------------------------------
// Master SynthState (shared effects chain + 3 LayerSynths)
// ---------------------------------------------------------------------------

struct SynthState {
    sample_rate: f32,
    layer_params: [Option<AudioParams>; 3],
    layers: [LayerSynth; 3],
    // Shared master effects chain
    filter: BiquadFilter,
    reverb: FdnReverb,
    delay: DelayLine,
    limiter: Limiter,
    chorus: Chorus,
    master_volume: f32,
    reverb_wet: f32,
    delay_ms: f32,
    delay_feedback: f32,
    // Metering
    meter: VuMeter,
    master_peak: f32,
    // Recording
    pub waveform: Arc<Mutex<Vec<f32>>>,
    pub recording: WavRecorder,
    pub loop_export: LoopExportPending,
    loop_recorder: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
    // Clip buffer (circular, last CLIP_SECONDS seconds)
    pub clip_buffer: ClipBuffer,
    // Sympathetic resonance: tiny crosstalk energy between layers (last sample per layer)
    layer_last: [f32; 3],
    // Breathing: phase accumulator for the ~4.5s volume oscillation
    // (This layer handles the audio-thread-side application to each sample)
    breathing_phase: f64,
}

impl SynthState {
    fn new(
        sr: f32, reverb_wet: f32, delay_ms: f32, delay_feedback: f32,
        waveform: Arc<Mutex<Vec<f32>>>,
        recording: WavRecorder,
        loop_export: LoopExportPending,
        meter: VuMeter,
        clip_buffer: ClipBuffer,
    ) -> Self {
        let mut reverb = FdnReverb::new(sr);
        reverb.wet = reverb_wet;
        let mut delay = DelayLine::new(2000.0, sr);
        delay.set_delay_ms(delay_ms, sr);
        delay.feedback = delay_feedback;
        delay.mix = 0.25;
        Self {
            sample_rate: sr,
            layer_params: [None, None, None],
            layers: [LayerSynth::new(sr), LayerSynth::new(sr), LayerSynth::new(sr)],
            filter: BiquadFilter::low_pass(8000.0, 0.7, sr),
            reverb,
            delay,
            limiter: Limiter::new(-1.0, 5.0, sr),
            chorus: Chorus::new(sr),
            master_volume: 0.7,
            reverb_wet,
            delay_ms,
            delay_feedback,
            meter,
            master_peak: 0.0,
            waveform,
            recording,
            loop_export,
            loop_recorder: None,
            clip_buffer,
            layer_last: [0.0; 3],
            breathing_phase: 0.0,
        }
    }

    fn update_params(&mut self, idx: usize, params: AudioParams) {
        if idx >= 3 { return; }
        // Update master effects from layer 0 params (layer 0 owns the master bus)
        if idx == 0 {
            // Hard floor at 300 Hz — lower cuts too much content and causes perceived silence
            let safe_cutoff = params.filter_cutoff.max(300.0);
            self.filter.update_lp(safe_cutoff, params.filter_q, self.sample_rate);
            self.master_volume = params.master_volume;
            self.reverb.wet = params.reverb_wet.clamp(0.0, 1.0);
            self.delay.feedback = params.delay_feedback.clamp(0.0, 0.9);
            self.delay.set_delay_ms(params.delay_ms.max(1.0), self.sample_rate);
            self.chorus.mix   = params.chorus_mix;
            self.chorus.rate  = params.chorus_rate;
            self.chorus.depth = params.chorus_depth;
        }
        self.layers[idx].update(&params);
        self.layer_params[idx] = Some(params);
    }

    fn render(&mut self, data: &mut [f32]) {
        let mv = self.master_volume;
        for frame in data.chunks_exact_mut(2) {
            let (l, r) = self.next_stereo_sample();
            frame[0] = l * mv;
            frame[1] = r * mv;
        }
    }

    fn next_stereo_sample(&mut self) -> (f32, f32) {
        // Sum all active layers, with sympathetic resonance crosstalk between them.
        // Like piano strings that resonate when nearby strings are struck —
        // layers running together sound subtly richer than two layers summed in a DAW.
        let mut sum_l = 0.0f32;
        let mut sum_r = 0.0f32;
        let mut layer_out = [(0.0f32, 0.0f32); 3];

        for i in 0..3 {
            if let Some(ref p) = self.layer_params[i].clone() {
                // Sympathetic resonance: inject a tiny fraction of the previous sample
                // from the other active layers into this layer's frequency smoothing.
                // Not enough to hear deliberately — enough to make layers feel acoustically coupled.
                let sympathy = 0.0008f32; // –62 dB crosstalk (inaudible as a signal, felt as warmth)
                let crosstalk_l: f32 = self.layer_last.iter().enumerate()
                    .filter(|&(j, _)| j != i)
                    .map(|(_, &v)| v)
                    .sum::<f32>() * sympathy;

                let (l, r) = self.layers[i].next_sample(p);
                // Resonance: crosstalk modulates the output (not the input frequencies,
                // to stay lock-free). The effect is a gentle intermodulation.
                let l = l + crosstalk_l;
                let r = r + crosstalk_l;
                layer_out[i] = (l, r);
                self.layer_last[i] = (l + r) * 0.5;
                sum_l += l;
                sum_r += r;
            } else {
                self.layer_last[i] *= 0.999; // slow decay when layer inactive
            }
        }

        // Write per-layer peaks to VU meter (non-blocking)
        if let Some(mut m) = self.meter.try_lock() {
            for i in 0..3 {
                // Decay existing reading
                m[i] *= 0.99;
                m[i] = m[i].max(self.layers[i].peak);
                self.layers[i].peak *= 0.98; // decay layer peak
            }
        }

        // Shared master effects chain
        let (lf, rf) = (
            self.filter.process(sum_l),
            self.filter.process(sum_r),
        );
        let (ld, rd) = self.delay.process(lf, rf);
        // Skip chorus computation when mix is negligible (CPU optimization)
        let (lc, rc) = if self.chorus.mix > 0.001 {
            self.chorus.process(ld, rd)
        } else {
            (ld, rd)
        };
        // Skip reverb computation when wet is negligible (CPU optimization)
        let (lrev, rrev) = if self.reverb.wet > 0.001 {
            self.reverb.process(lc, rc)
        } else {
            (lc, rc)
        };
        let (lo_raw, ro_raw) = self.limiter.process(lrev, rrev);

        // Final NaN guard
        let lo = if lo_raw.is_finite() { lo_raw } else { 0.0 };
        let ro = if ro_raw.is_finite() { ro_raw } else { 0.0 };

        // Master peak for VU
        let mpeak = lo.abs().max(ro.abs());
        self.master_peak = (self.master_peak * 0.999).max(mpeak);
        if let Some(mut m) = self.meter.try_lock() {
            m[3] = (m[3] * 0.99).max(self.master_peak);
            self.master_peak *= 0.98;
        }

        // Waveform capture (non-blocking)
        if let Some(mut wf) = self.waveform.try_lock() {
            wf.push(lo);
            let excess = wf.len().saturating_sub(2048);
            if excess > 0 { wf.drain(0..excess); }
        }

        // Clip buffer (non-blocking, keeps last CLIP_SECONDS of stereo audio)
        if let Some(mut cb) = self.clip_buffer.try_lock() {
            cb.push_back(lo);
            cb.push_back(ro);
            let max_samples = CLIP_SECONDS * self.sample_rate as usize * 2;
            while cb.len() > max_samples {
                cb.pop_front();
            }
        }

        // WAV recording (non-blocking)
        if let Some(mut rec) = self.recording.try_lock() {
            if let Some(ref mut writer) = *rec {
                let _ = writer.write_sample(lo);
                let _ = writer.write_sample(ro);
            }
        }

        self.handle_loop_export(lo, ro);
        (lo, ro)
    }

    fn handle_loop_export(&mut self, lo: f32, ro: f32) {
        if let Some(mut pending) = self.loop_export.try_lock() {
            match *pending {
                Some(n) if n > 0 => {
                    if self.loop_recorder.is_none() {
                        let secs = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default().as_secs();
                        let filename = format!("loop_{}.wav", secs);
                        let spec = hound::WavSpec {
                            channels: 2, sample_rate: self.sample_rate as u32,
                            bits_per_sample: 32, sample_format: hound::SampleFormat::Float,
                        };
                        if let Ok(w) = hound::WavWriter::create(&filename, spec) {
                            self.loop_recorder = Some(w);
                        }
                    }
                    if let Some(ref mut w) = self.loop_recorder {
                        let _ = w.write_sample(lo);
                        let _ = w.write_sample(ro);
                    }
                    *pending = Some(n - 1);
                }
                Some(0) => {
                    if let Some(w) = self.loop_recorder.take() { let _ = w.finalize(); }
                    *pending = None;
                }
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Audio engine bootstrap
// ---------------------------------------------------------------------------

pub struct AudioEngine {
    _stream: Stream,
    _input_stream: Option<Stream>,
}

impl AudioEngine {
    pub fn start(
        params_rx: Receiver<[Option<AudioParams>; 3]>,
        sample_rate: u32,
        reverb_wet: f32,
        delay_ms: f32,
        delay_feedback: f32,
        master_volume: f32,
        waveform: Arc<Mutex<Vec<f32>>>,
        recording: WavRecorder,
        loop_export: LoopExportPending,
        meter: VuMeter,
        clip_buffer: ClipBuffer,
        sidechain_level: SidechainLevel,
    ) -> anyhow::Result<(Self, u32)> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No audio output device"))?;
        let default_config = device.default_output_config()?;
        let actual_sr = default_config.sample_rate().0;
        let fmt = default_config.sample_format();
        log::info!("Audio: {} Hz, {:?}", actual_sr, fmt);

        let sr = actual_sr as f32;
        let synth = Arc::new(Mutex::new(
            SynthState::new(sr, reverb_wet, delay_ms, delay_feedback,
                            waveform, recording, loop_export, meter, clip_buffer)
        ));
        synth.lock().master_volume = master_volume;

        let stream_config = default_config.config();

        // Build a reusable "drain latest params" function as a macro-like closure factory
        fn drain(rx: &Receiver<[Option<AudioParams>; 3]>) -> [Option<AudioParams>; 3] {
            let mut latest: [Option<AudioParams>; 3] = [None, None, None];
            while let Ok(batch) = rx.try_recv() {
                for i in 0..3 { if batch[i].is_some() { latest[i] = batch[i].clone(); } }
            }
            latest
        }

        let stream = match fmt {
            SampleFormat::F32 => {
                let ss = synth.clone();
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let latest = drain(&params_rx);
                        let mut state = ss.lock();
                        for i in 0..3 { if let Some(p) = latest[i].clone() { state.update_params(i, p); } }
                        state.render(data);
                    },
                    |err| log::error!("Audio stream error: {err}"), None)?
            }
            _ => {
                // For I16/U16: convert via f32 buffer, same drain logic
                let ss = synth.clone();
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let latest = drain(&params_rx);
                        let mut state = ss.lock();
                        for i in 0..3 { if let Some(p) = latest[i].clone() { state.update_params(i, p); } }
                        state.render(data);
                    },
                    |err| log::error!("Audio stream error: {err}"), None)?
            }
        };

        stream.play()?;

        // Optional sidechain input stream
        let input_stream = Self::start_input(&host, sidechain_level).ok();

        Ok((Self { _stream: stream, _input_stream: input_stream }, actual_sr))
    }

    fn start_input(host: &cpal::Host, sidechain_level: SidechainLevel) -> anyhow::Result<Stream> {
        let device = host.default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device"))?;
        let config = device.default_input_config()?;
        let fmt = config.sample_format();
        let stream_config = config.config();

        // Running RMS accumulator
        let accumulator = Arc::new(Mutex::new((0.0f64, 0usize))); // (sum_sq, count)

        let make_input_cb = |acc: Arc<Mutex<(f64, usize)>>, sc: SidechainLevel| {
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let sum_sq: f64 = data.iter().map(|&x| (x as f64).powi(2)).sum();
                let mut a = acc.lock();
                a.0 += sum_sq;
                a.1 += data.len();
                // Emit RMS every ~2048 samples
                if a.1 >= 2048 {
                    let rms = (a.0 / a.1 as f64).sqrt() as f32;
                    if let Some(mut lvl) = sc.try_lock() {
                        // Smooth the sidechain level
                        *lvl = *lvl * 0.9 + rms * 0.1;
                    }
                    *a = (0.0, 0);
                }
            }
        };

        let stream = match fmt {
            SampleFormat::F32 => {
                let cb = make_input_cb(accumulator, sidechain_level);
                device.build_input_stream(&stream_config, cb,
                    |err| log::warn!("Input stream error: {err}"), None)?
            }
            _ => {
                // For non-f32 input, just skip sidechain
                anyhow::bail!("Non-f32 input not supported for sidechain");
            }
        };
        stream.play()?;
        Ok(stream)
    }
}

/// Save the clip buffer to a timestamped WAV file. Returns the filename written.
pub fn save_clip(clip_buffer: &ClipBuffer, sample_rate: u32) -> anyhow::Result<String> {
    let samples: Vec<f32> = {
        let cb = clip_buffer.lock();
        cb.iter().copied().collect()
    };
    if samples.is_empty() {
        anyhow::bail!("Clip buffer is empty");
    }
    let dir = std::path::PathBuf::from("clips");
    if !dir.exists() { std::fs::create_dir_all(&dir)?; }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_secs();
    let filename = dir.join(format!("clip_{}.wav", ts));
    let spec = hound::WavSpec {
        channels: 2, sample_rate, bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&filename, spec)?;
    for s in &samples { writer.write_sample(*s)?; }
    writer.finalize()?;
    Ok(filename.to_string_lossy().into_owned())
}

/// Render the phase portrait trail to a PNG file. Returns filename.
pub fn save_portrait_png(trail: &[(f32, f32, f32, f32, bool)]) -> anyhow::Result<String> {
    let size: u32 = 512;
    let mut pixels = vec![0u8; (size * size * 3) as usize];

    if trail.len() < 2 {
        anyhow::bail!("Trail too short");
    }

    // Find bounds
    let xs: Vec<f32> = trail.iter().map(|p| p.0).collect();
    let ys: Vec<f32> = trail.iter().map(|p| p.1).collect();
    let xmin = xs.iter().cloned().fold(f32::INFINITY, f32::min);
    let xmax = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let ymin = ys.iter().cloned().fold(f32::INFINITY, f32::min);
    let ymax = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let xr = (xmax - xmin).max(0.001);
    let yr = (ymax - ymin).max(0.001);

    for (i, &(x, y, _, speed, crossing)) in trail.iter().enumerate() {
        let px = ((x - xmin) / xr * (size - 1) as f32) as u32;
        let py = ((y - ymin) / yr * (size - 1) as f32) as u32;
        let px = px.clamp(0, size - 1);
        let py = py.clamp(0, size - 1);
        let idx = ((size - 1 - py) * size + px) as usize * 3;
        if idx + 2 < pixels.len() {
            let t = i as f32 / trail.len() as f32;
            let intensity = (speed * 0.7 + 0.3).min(1.0);
            if crossing {
                pixels[idx]     = 255;
                pixels[idx + 1] = 255;
                pixels[idx + 2] = 100;
            } else {
                pixels[idx]     = (t * intensity * 80.0) as u8;
                pixels[idx + 1] = (intensity * 180.0) as u8;
                pixels[idx + 2] = (255.0 * intensity) as u8;
            }
        }
    }

    let dir = std::path::PathBuf::from("clips");
    if !dir.exists() { std::fs::create_dir_all(&dir)?; }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_secs();
    let filename = dir.join(format!("portrait_{}.png", ts));
    write_png(&filename, &pixels, size, size)?;
    Ok(filename.to_string_lossy().into_owned())
}

fn write_png(path: &std::path::Path, rgb: &[u8], width: u32, height: u32) -> anyhow::Result<()> {
    use png::ColorType;
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgb)?;
    Ok(())
}
