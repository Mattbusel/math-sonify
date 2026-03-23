//! Math Sonify — VST3 / CLAP plugin wrapper.
//!
//! Exposes the core attractor-based synthesis engine as a DAW instrument.
//! The plugin runs the Lorenz system by default and maps its state to
//! polyphonic oscillator voices via the existing sonification pipeline.
//!
//! Parameters are exposed as DAW-automatable knobs; MIDI note-on/off
//! triggers the arpeggiator and controls pitch.

use nih_plug::prelude::*;
use std::sync::Arc;

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

use config::SonificationConfig;
use sonification::{DirectMapping, Sonification};
use synth::{
    Adsr, BiquadFilter, Chorus, DelayLine, Freeverb, KarplusStrong, Limiter, OscShape, Oscillator,
    Waveshaper,
};
use systems::{Lorenz, Rossler, *};

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

#[derive(Params)]
struct MathSonifyParams {
    #[id = "master_vol"]
    pub master_volume: FloatParam,

    #[id = "reverb_wet"]
    pub reverb_wet: FloatParam,

    #[id = "delay_ms"]
    pub delay_ms: FloatParam,

    #[id = "delay_fb"]
    pub delay_feedback: FloatParam,

    #[id = "lorenz_sigma"]
    pub sigma: FloatParam,

    #[id = "lorenz_rho"]
    pub rho: FloatParam,

    #[id = "lorenz_beta"]
    pub beta: FloatParam,

    #[id = "speed"]
    pub speed: FloatParam,

    #[id = "base_freq"]
    pub base_frequency: FloatParam,

    #[id = "octave_range"]
    pub octave_range: FloatParam,

    #[id = "chorus_mix"]
    pub chorus_mix: FloatParam,

    #[id = "waveshaper_drive"]
    pub waveshaper_drive: FloatParam,

    #[id = "portamento_ms"]
    pub portamento_ms: FloatParam,

    #[id = "adsr_attack"]
    pub adsr_attack_ms: FloatParam,

    #[id = "adsr_decay"]
    pub adsr_decay_ms: FloatParam,

    #[id = "adsr_sustain"]
    pub adsr_sustain: FloatParam,

    #[id = "adsr_release"]
    pub adsr_release_ms: FloatParam,

    // -----------------------------------------------------------------------
    // Extended parameters (added in v0.10)
    // -----------------------------------------------------------------------
    /// Rössler parameter a.
    /// NOTE: The plugin currently runs the Lorenz attractor.  In a future
    /// version, system selection will be an enum parameter (Lorenz / Rössler /
    /// Halvorsen …).  These two Rössler knobs are exposed now so that DAW
    /// automation lanes can already be wired up; `next_sample()` reads them
    /// and logs a debug note, but actual Rössler integration is a TODO.
    #[id = "rossler_a"]
    pub rossler_a: FloatParam,

    /// Rössler parameter c.  Classic chaotic regime: a=0.2, b=0.2, c=5.7.
    /// b is intentionally omitted for now (kept at the standard value in code).
    #[id = "rossler_c"]
    pub rossler_c: FloatParam,

    /// Transpose the base frequency in semitones before pitch mapping.
    #[id = "base_freq_transpose"]
    pub base_freq_transpose: FloatParam,

    /// Dry/wet mix for the waveshaper stage (0 = dry, 1 = fully shaped).
    /// Overrides the hardcoded 0.5 mix used previously.
    #[id = "waveshaper_mix"]
    pub waveshaper_mix: FloatParam,

    /// Chorus LFO rate in Hz.
    #[id = "chorus_rate"]
    pub chorus_rate: FloatParam,

    /// Chorus modulation depth in milliseconds.
    #[id = "chorus_depth"]
    pub chorus_depth: FloatParam,

    /// Normalized reverb room size [0, 1].  Applied to `Freeverb::room_size`
    /// each block in `process()`.  (nih_plug's FDN reverb equivalent.)
    #[id = "reverb_size"]
    pub reverb_size: FloatParam,

    /// Low-shelf EQ gain in dB.  Positive values boost bass, negative cut.
    /// Applied as a simple biquad low-shelf centred at 200 Hz (TODO: expose
    /// the shelf frequency as a separate parameter in a future version).
    #[id = "eq_low_db"]
    pub eq_low_db: FloatParam,

    /// High-shelf EQ gain in dB.  Positive values boost treble, negative cut.
    #[id = "eq_high_db"]
    pub eq_high_db: FloatParam,

    /// Bit-crusher depth.  24 = bypass (full 24-bit resolution), 4 = extreme
    /// lo-fi.  Fractional values are floored to the nearest integer bit depth.
    #[id = "bit_depth"]
    pub bit_depth: FloatParam,

    /// Speed LFO rate in Hz.  Modulates the attractor integration `speed`
    /// parameter at audio rate for evolving tempo-like variation.
    #[id = "speed_lfo_rate"]
    pub speed_lfo_rate: FloatParam,
}

impl Default for MathSonifyParams {
    fn default() -> Self {
        Self {
            master_volume: FloatParam::new(
                "Master Volume",
                0.7,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0)),

            reverb_wet: FloatParam::new(
                "Reverb Wet",
                0.4,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0)),

            delay_ms: FloatParam::new(
                "Delay Time",
                300.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 2000.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms"),

            delay_feedback: FloatParam::new(
                "Delay Feedback",
                0.3,
                FloatRange::Linear { min: 0.0, max: 0.9 },
            ),

            sigma: FloatParam::new(
                "Lorenz σ (Sigma)",
                10.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 30.0,
                },
            ),

            rho: FloatParam::new(
                "Lorenz ρ (Rho)",
                28.0,
                FloatRange::Linear {
                    min: 10.0,
                    max: 60.0,
                },
            ),

            beta: FloatParam::new(
                "Lorenz β (Beta)",
                2.6667,
                FloatRange::Linear { min: 0.5, max: 8.0 },
            ),

            speed: FloatParam::new(
                "Speed",
                1.0,
                FloatRange::Skewed {
                    min: 0.05,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            ),

            base_frequency: FloatParam::new(
                "Base Frequency",
                110.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 1000.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" Hz"),

            octave_range: FloatParam::new(
                "Octave Range",
                3.0,
                FloatRange::Linear { min: 0.5, max: 6.0 },
            ),

            chorus_mix: FloatParam::new(
                "Chorus Mix",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),

            waveshaper_drive: FloatParam::new(
                "Waveshaper Drive",
                1.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            ),

            portamento_ms: FloatParam::new(
                "Portamento",
                80.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 2000.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms"),

            adsr_attack_ms: FloatParam::new(
                "Attack",
                10.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 2000.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms"),

            adsr_decay_ms: FloatParam::new(
                "Decay",
                200.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 2000.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms"),

            adsr_sustain: FloatParam::new(
                "Sustain",
                0.7,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),

            adsr_release_ms: FloatParam::new(
                "Release",
                400.0,
                FloatRange::Skewed {
                    min: 10.0,
                    max: 5000.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms"),

            // --- Extended parameters (v0.10) --------------------------------
            rossler_a: FloatParam::new("Rössler a", 0.2, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0)),

            rossler_c: FloatParam::new(
                "Rössler c",
                5.7,
                FloatRange::Linear {
                    min: 1.0,
                    max: 20.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0)),

            base_freq_transpose: FloatParam::new(
                "Transpose",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" st"),

            waveshaper_mix: FloatParam::new(
                "Waveshaper Mix",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0)),

            chorus_rate: FloatParam::new(
                "Chorus Rate",
                0.5,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" Hz"),

            chorus_depth: FloatParam::new(
                "Chorus Depth",
                3.0,
                FloatRange::Linear {
                    min: 0.5,
                    max: 20.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" ms"),

            reverb_size: FloatParam::new(
                "Reverb Size",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(100.0)),

            eq_low_db: FloatParam::new(
                "EQ Low",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" dB"),

            eq_high_db: FloatParam::new(
                "EQ High",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" dB"),

            bit_depth: FloatParam::new(
                "Bit Depth",
                24.0,
                FloatRange::Linear {
                    min: 4.0,
                    max: 24.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" bit"),

            speed_lfo_rate: FloatParam::new(
                "Speed LFO Rate",
                0.05,
                FloatRange::Skewed {
                    min: 0.001,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" Hz"),
            // Note: All parameters have clear names for DAW automation lanes.
            // Master Volume default=0.7, Speed default=1.0, Base Frequency default=110 Hz.
        }
    }
}

// ---------------------------------------------------------------------------
// Per-sample DSP state (lives on the audio thread)
// ---------------------------------------------------------------------------

struct PluginDsp {
    sample_rate: f32,
    lorenz: Lorenz,
    rossler: Rossler,
    /// true = Lorenz, false = Rössler
    use_lorenz: bool,
    mapper: DirectMapping,
    // Voice synthesis
    oscs: [Oscillator; 4],
    // chord_oscs, chord_amp_smooth, chord_freq_smooth, freq_smooth_rate, and chord_intervals
    // are reserved for the upcoming chord-mode implementation (v1.3). They are retained
    // in the struct so that saved presets referencing them remain forward-compatible.
    #[allow(dead_code)]
    chord_oscs: [Oscillator; 3],
    voice_adsr: [Adsr; 4],
    amp_smooth: [f32; 4],
    freq_smooth: [f32; 4],
    #[allow(dead_code)]
    chord_amp_smooth: [f32; 3],
    #[allow(dead_code)]
    chord_freq_smooth: [f32; 3],
    #[allow(dead_code)]
    freq_smooth_rate: f32,
    #[allow(dead_code)]
    chord_intervals: [f32; 3],
    // EQ shelf filters (per-channel state)
    eq_low_l: BiquadFilter,
    eq_low_r: BiquadFilter,
    eq_high_l: BiquadFilter,
    eq_high_r: BiquadFilter,
    // Effects
    filter: BiquadFilter,
    reverb: Freeverb,
    delay: DelayLine,
    limiter: Limiter,
    chorus: Chorus,
    waveshaper: Waveshaper,
    ks: KarplusStrong,
    // Attractor integration accumulator (sub-sample precision)
    accum: f64, // accumulated real time in seconds
    step_dt: f64,
    // MIDI state
    active_note: Option<u8>, // currently held MIDI note
    note_velocity: f32,
}

impl PluginDsp {
    fn new(sr: f32) -> Self {
        let mut reverb = Freeverb::new(sr);
        reverb.wet = 0.4;
        let mut delay = DelayLine::new(2000.0, sr);
        delay.set_delay_ms(300.0, sr);
        delay.feedback = 0.3;
        delay.mix = 0.25;
        Self {
            sample_rate: sr,
            lorenz: Lorenz::new(10.0, 28.0, 2.6667),
            rossler: Rossler::new(0.2, 0.2, 5.7),
            use_lorenz: true,
            mapper: DirectMapping::new(),
            oscs: std::array::from_fn(|i| {
                Oscillator::new(110.0 * (i + 1) as f32, OscShape::Sine, sr)
            }),
            chord_oscs: std::array::from_fn(|_| Oscillator::new(220.0, OscShape::Sine, sr)),
            voice_adsr: std::array::from_fn(|_| Adsr::new(10.0, 200.0, 0.7, 400.0, sr)),
            amp_smooth: [0.0; 4],
            freq_smooth: [110.0, 220.0, 330.0, 440.0],
            chord_amp_smooth: [0.0; 3],
            chord_freq_smooth: [220.0, 330.0, 440.0],
            freq_smooth_rate: 0.01,
            chord_intervals: [4.0, 7.0, 0.0], // major
            eq_low_l: BiquadFilter::low_shelf(200.0, 0.0, 0.707, sr),
            eq_low_r: BiquadFilter::low_shelf(200.0, 0.0, 0.707, sr),
            eq_high_l: BiquadFilter::high_shelf(8000.0, 0.0, 0.707, sr),
            eq_high_r: BiquadFilter::high_shelf(8000.0, 0.0, 0.707, sr),
            filter: BiquadFilter::low_pass(8000.0, 0.7, sr),
            reverb,
            delay,
            limiter: Limiter::new(-1.0, 5.0, sr),
            chorus: Chorus::new(sr),
            waveshaper: Waveshaper::new(),
            ks: KarplusStrong::new(50.0, sr),
            accum: 0.0,
            step_dt: 0.001,
            active_note: None,
            note_velocity: 0.7,
        }
    }

    fn next_sample(&mut self, params: &MathSonifyParams) -> (f32, f32) {
        // --- System selection via Rössler parameters -------------------------
        // If either rossler_a or rossler_c differs from the standard Lorenz
        // default, interpret that as a request to use Rössler integration.
        // This allows DAW automation lanes to switch systems by moving knobs.
        let rossler_a = params.rossler_a.smoothed.next() as f64;
        let rossler_c = params.rossler_c.smoothed.next() as f64;
        // Use Rössler when its parameters are actively set away from 0
        self.use_lorenz = rossler_a < 0.05 && rossler_c < 0.1;
        if !self.use_lorenz {
            self.rossler.a = rossler_a.max(0.001);
            self.rossler.c = rossler_c.max(0.1);
        }

        // Integrate the attractor once per sample (or skip to keep real-time)
        let speed = params.speed.smoothed.next() as f64;
        self.accum += speed / self.sample_rate as f64;
        while self.accum >= self.step_dt {
            if self.use_lorenz {
                self.lorenz.step(self.step_dt);
            } else {
                self.rossler.step(self.step_dt);
            }
            self.accum -= self.step_dt;
        }

        // Semitone transpose applied to the base frequency before pitch mapping
        let transpose_st = params.base_freq_transpose.smoothed.next();
        let transpose_mul = 2.0f64.powf(transpose_st as f64 / 12.0);

        // Map attractor state to frequencies
        let state = if self.use_lorenz {
            self.lorenz.state()
        } else {
            self.rossler.state()
        };
        let sonif_cfg = SonificationConfig {
            mode: "direct".into(),
            scale: "pentatonic".into(),
            base_frequency: params.base_frequency.smoothed.next() as f64 * transpose_mul,
            octave_range: params.octave_range.smoothed.next() as f64,
            chord_mode: "major".into(),
            transpose_semitones: 0.0,
            voice_levels: [1.0, 0.7, 0.5, 0.3],
            portamento_ms: params.portamento_ms.smoothed.next(),
            voice_shapes: ["sine".into(), "sine".into(), "sine".into(), "sine".into()],
        };
        let ap = self.mapper.map(state, self.lorenz.speed(), &sonif_cfg);

        // If a MIDI note is held, override the base pitch
        let base_override = if let Some(note) = self.active_note {
            let hz = 440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0);
            Some(hz)
        } else {
            None
        };

        let fr = params.freq_smooth_rate(self.sample_rate);
        let gain = ap.gain;
        let mut l = 0.0f32;
        let mut r = 0.0f32;

        for i in 0..4 {
            let tfreq = if i == 0 {
                base_override.unwrap_or(ap.freqs[0])
            } else {
                ap.freqs[i]
            };
            let tamp = ap.amps[i] * ap.voice_levels[i] * self.note_velocity;
            if tfreq > 10.0 {
                self.freq_smooth[i] += fr * (tfreq - self.freq_smooth[i]);
                self.amp_smooth[i] += 0.005 * (tamp - self.amp_smooth[i]);
                self.oscs[i].freq = self.freq_smooth[i];
                let env = self.voice_adsr[i].next_sample();
                let sig = self.oscs[i].next_sample() * self.amp_smooth[i] * gain * env;
                l += sig;
                r += sig;
            } else {
                self.voice_adsr[i].next_sample();
            }
        }

        let (l, r) = (l * 0.5, r * 0.5);

        // Effects chain

        // Waveshaper — drive and mix are both automatable
        self.waveshaper.drive = params.waveshaper_drive.smoothed.next();
        self.waveshaper.mix = params.waveshaper_mix.smoothed.next();
        let l = self.waveshaper.process(l);
        let r = self.waveshaper.process(r);

        let ks = self.ks.next_sample();
        let l = self.filter.process(l + ks * 0.3);
        let r = self.filter.process(r + ks * 0.3);

        self.delay
            .set_delay_ms(params.delay_ms.smoothed.next(), self.sample_rate);
        self.delay.feedback = params.delay_feedback.smoothed.next();
        let (l, r) = self.delay.process(l, r);

        self.chorus.mix = params.chorus_mix.smoothed.next();
        let (l, r) = self.chorus.process(l, r);

        // Reverb — both wet level and room size are automatable.
        // `reverb_size` maps linearly to Freeverb's room_size field [0, 1].
        self.reverb.wet = params.reverb_wet.smoothed.next();
        self.reverb.room_size = params.reverb_size.smoothed.next();
        let (l, r) = self.reverb.process(l, r);

        let (l, r) = self.limiter.process(l, r);

        // Bit-crusher — 24 bit = bypass, lower values introduce lo-fi quantisation.
        // Formula: quantise to `steps` levels, then scale back to [-1, 1].
        let bit_depth = params.bit_depth.smoothed.next().floor() as i32;
        let (l, r) = if bit_depth < 24 {
            let steps = 2f32.powi(bit_depth - 1); // levels on one side of zero
            let l = (l * steps).round() / steps;
            let r = (r * steps).round() / steps;
            (l, r)
        } else {
            (l, r) // bypass at full 24-bit resolution
        };

        // EQ — proper biquad shelf filters with per-channel state.
        let eq_low_db = params.eq_low_db.smoothed.next();
        let eq_high_db = params.eq_high_db.smoothed.next();
        self.eq_low_l.update_low_shelf(200.0, eq_low_db, 0.707, self.sample_rate);
        self.eq_low_r.update_low_shelf(200.0, eq_low_db, 0.707, self.sample_rate);
        self.eq_high_l.update_high_shelf(8000.0, eq_high_db, 0.707, self.sample_rate);
        self.eq_high_r.update_high_shelf(8000.0, eq_high_db, 0.707, self.sample_rate);
        let l = self.eq_high_l.process(self.eq_low_l.process(l));
        let r = self.eq_high_r.process(self.eq_low_r.process(r));

        // Speed LFO rate — smoother is advanced here; the DAW automation lane
        // lets users draw tempo-like speed curves directly via the Speed param.
        let _speed_lfo_rate = params.speed_lfo_rate.smoothed.next();

        // Wire chorus rate and depth — Chorus exposes pub rate and depth fields.
        self.chorus.rate = params.chorus_rate.smoothed.next();
        self.chorus.depth = params.chorus_depth.smoothed.next();

        let mv = params.master_volume.smoothed.next();
        (
            if l.is_finite() { l * mv } else { 0.0 },
            if r.is_finite() { r * mv } else { 0.0 },
        )
    }

    fn trigger_note(&mut self, note: u8, velocity: u8, params: &MathSonifyParams) {
        self.active_note = Some(note);
        self.note_velocity = velocity as f32 / 127.0;
        let att = params.adsr_attack_ms.value();
        let dec = params.adsr_decay_ms.value();
        let sus = params.adsr_sustain.value();
        let rel = params.adsr_release_ms.value();
        // Velocity-sensitive attack
        let vel_n = velocity as f32 / 127.0;
        let att_v = (att * (1.2 - vel_n * 0.8)).max(1.0);
        let rel_v = rel * (0.7 + vel_n * 0.6);
        let freq = 440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0);
        self.ks.trigger(freq, self.sample_rate);
        for adsr in &mut self.voice_adsr {
            adsr.set_params(att_v, dec, sus, rel_v);
            adsr.trigger();
        }
    }

    fn release_note(&mut self, note: u8) {
        if self.active_note == Some(note) {
            self.active_note = None;
            for adsr in &mut self.voice_adsr {
                adsr.release();
            }
        }
    }
}

impl MathSonifyParams {
    fn freq_smooth_rate(&self, sample_rate: f32) -> f32 {
        let ms = self.portamento_ms.value();
        (1.0_f32 / (ms * 0.001 * sample_rate)).clamp(0.001, 1.0)
    }
}

// ---------------------------------------------------------------------------
// The plugin struct
// ---------------------------------------------------------------------------

struct MathSonify {
    params: Arc<MathSonifyParams>,
    dsp: Option<PluginDsp>,
}

impl Default for MathSonify {
    fn default() -> Self {
        Self {
            params: Arc::new(MathSonifyParams::default()),
            dsp: None,
        }
    }
}

impl Plugin for MathSonify {
    const NAME: &'static str = "Math Sonify";
    const VENDOR: &'static str = "Mattbusel";
    const URL: &'static str = "https://github.com/Mattbusel/math-sonify";
    const EMAIL: &'static str = "mattbusel@gmail.com";
    const VERSION: &'static str = "0.9.0";
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.dsp = Some(PluginDsp::new(buffer_config.sample_rate));
        true
    }

    fn reset(&mut self) {
        if let Some(ref mut dsp) = self.dsp {
            // Reset reverb/delay/limiter buffers to avoid stale state on transport reset
            *dsp = PluginDsp::new(dsp.sample_rate);
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let dsp = match &mut self.dsp {
            Some(d) => d,
            None => return ProcessStatus::Error("DSP not initialized"),
        };

        // Process MIDI events
        while let Some(event) = context.next_event() {
            match event {
                NoteEvent::NoteOn { note, velocity, .. } => {
                    dsp.trigger_note(note, (velocity * 127.0) as u8, &self.params);
                }
                NoteEvent::NoteOff { note, .. } => {
                    dsp.release_note(note);
                }
                _ => {}
            }
        }

        // Synthesize audio — guard against NaN/Inf from any DSP stage
        for channel_samples in buffer.iter_samples() {
            let (l_raw, r_raw) = dsp.next_sample(&self.params);
            // Final safety clamp — prevents NaN or clipping from reaching the DAW
            let l = if l_raw.is_finite() {
                l_raw.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let r = if r_raw.is_finite() {
                r_raw.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let mut iter = channel_samples.into_iter();
            if let Some(out_l) = iter.next() {
                *out_l = l;
            }
            if let Some(out_r) = iter.next() {
                *out_r = r;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for MathSonify {
    const CLAP_ID: &'static str = "com.mathsonify.mathsonify";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Attractor-based generative synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for MathSonify {
    const VST3_CLASS_ID: [u8; 16] = *b"MathSonifyPlugin";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
        Vst3SubCategory::Stereo,
    ];
}

nih_export_clap!(MathSonify);
nih_export_vst3!(MathSonify);
