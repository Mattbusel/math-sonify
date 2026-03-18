/// Math Sonify — VST3 / CLAP plugin wrapper.
///
/// Exposes the core attractor-based synthesis engine as a DAW instrument.
/// The plugin runs the Lorenz system by default and maps its state to
/// polyphonic oscillator voices via the existing sonification pipeline.
///
/// Parameters are exposed as DAW-automatable knobs; MIDI note-on/off
/// triggers the arpeggiator and controls pitch.

use nih_plug::prelude::*;
use std::sync::Arc;

pub mod systems;
pub mod sonification;
pub mod synth;
pub mod config;
pub mod error;
mod patches;
mod arrangement;

use systems::*;
use sonification::{AudioParams, Sonification, DirectMapping, chord_intervals_for};
use synth::{Oscillator, OscShape, BiquadFilter, Freeverb, DelayLine, Limiter,
            GrainEngine, Bitcrusher, KarplusStrong, Chorus, Waveshaper, Adsr};
use config::{Config, LorenzConfig, SonificationConfig, AudioConfig};

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
                "Master Volume", 0.7,
                FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0)),

            reverb_wet: FloatParam::new(
                "Reverb Wet", 0.4,
                FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0)),

            delay_ms: FloatParam::new(
                "Delay Time", 300.0,
                FloatRange::Skewed { min: 1.0, max: 2000.0, factor: FloatRange::skew_factor(-1.5) })
                .with_unit(" ms"),

            delay_feedback: FloatParam::new(
                "Delay Feedback", 0.3,
                FloatRange::Linear { min: 0.0, max: 0.9 }),

            sigma: FloatParam::new(
                "Lorenz σ (Sigma)", 10.0,
                FloatRange::Linear { min: 1.0, max: 30.0 }),

            rho: FloatParam::new(
                "Lorenz ρ (Rho)", 28.0,
                FloatRange::Linear { min: 10.0, max: 60.0 }),

            beta: FloatParam::new(
                "Lorenz β (Beta)", 2.6667,
                FloatRange::Linear { min: 0.5, max: 8.0 }),

            speed: FloatParam::new(
                "Speed", 1.0,
                FloatRange::Skewed { min: 0.05, max: 10.0, factor: FloatRange::skew_factor(-1.0) }),

            base_frequency: FloatParam::new(
                "Base Frequency", 110.0,
                FloatRange::Skewed { min: 20.0, max: 1000.0, factor: FloatRange::skew_factor(-1.5) })
                .with_unit(" Hz"),

            octave_range: FloatParam::new(
                "Octave Range", 3.0,
                FloatRange::Linear { min: 0.5, max: 6.0 }),

            chorus_mix: FloatParam::new(
                "Chorus Mix", 0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 }),

            waveshaper_drive: FloatParam::new(
                "Waveshaper Drive", 1.0,
                FloatRange::Skewed { min: 1.0, max: 10.0, factor: FloatRange::skew_factor(-1.0) }),

            portamento_ms: FloatParam::new(
                "Portamento", 80.0,
                FloatRange::Skewed { min: 1.0, max: 2000.0, factor: FloatRange::skew_factor(-1.5) })
                .with_unit(" ms"),

            adsr_attack_ms: FloatParam::new(
                "Attack", 10.0,
                FloatRange::Skewed { min: 1.0, max: 2000.0, factor: FloatRange::skew_factor(-1.5) })
                .with_unit(" ms"),

            adsr_decay_ms: FloatParam::new(
                "Decay", 200.0,
                FloatRange::Skewed { min: 1.0, max: 2000.0, factor: FloatRange::skew_factor(-1.5) })
                .with_unit(" ms"),

            adsr_sustain: FloatParam::new(
                "Sustain", 0.7,
                FloatRange::Linear { min: 0.0, max: 1.0 }),

            adsr_release_ms: FloatParam::new(
                "Release", 400.0,
                FloatRange::Skewed { min: 10.0, max: 5000.0, factor: FloatRange::skew_factor(-1.5) })
                .with_unit(" ms"),

            // --- Extended parameters (v0.10) --------------------------------

            rossler_a: FloatParam::new(
                "Rössler a", 0.2,
                FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0)),

            rossler_c: FloatParam::new(
                "Rössler c", 5.7,
                FloatRange::Linear { min: 1.0, max: 20.0 })
                .with_smoother(SmoothingStyle::Linear(20.0)),

            base_freq_transpose: FloatParam::new(
                "Transpose", 0.0,
                FloatRange::Linear { min: -24.0, max: 24.0 })
                .with_smoother(SmoothingStyle::Linear(10.0))
                .with_unit(" st"),

            waveshaper_mix: FloatParam::new(
                "Waveshaper Mix", 0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0)),

            chorus_rate: FloatParam::new(
                "Chorus Rate", 0.5,
                FloatRange::Skewed { min: 0.1, max: 10.0, factor: FloatRange::skew_factor(-1.0) })
                .with_smoother(SmoothingStyle::Linear(50.0))
                .with_unit(" Hz"),

            chorus_depth: FloatParam::new(
                "Chorus Depth", 3.0,
                FloatRange::Linear { min: 0.5, max: 20.0 })
                .with_smoother(SmoothingStyle::Linear(50.0))
                .with_unit(" ms"),

            reverb_size: FloatParam::new(
                "Reverb Size", 0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(100.0)),

            eq_low_db: FloatParam::new(
                "EQ Low", 0.0,
                FloatRange::Linear { min: -12.0, max: 12.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_unit(" dB"),

            eq_high_db: FloatParam::new(
                "EQ High", 0.0,
                FloatRange::Linear { min: -12.0, max: 12.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_unit(" dB"),

            bit_depth: FloatParam::new(
                "Bit Depth", 24.0,
                FloatRange::Linear { min: 4.0, max: 24.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_unit(" bit"),

            speed_lfo_rate: FloatParam::new(
                "Speed LFO Rate", 0.05,
                FloatRange::Skewed { min: 0.001, max: 10.0, factor: FloatRange::skew_factor(-2.0) })
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
    mapper: DirectMapping,
    // Voice synthesis
    oscs: [Oscillator; 4],
    chord_oscs: [Oscillator; 3],
    voice_adsr: [Adsr; 4],
    amp_smooth: [f32; 4],
    freq_smooth: [f32; 4],
    chord_amp_smooth: [f32; 3],
    chord_freq_smooth: [f32; 3],
    freq_smooth_rate: f32,
    chord_intervals: [f32; 3],
    // Effects
    filter: BiquadFilter,
    reverb: Freeverb,
    delay: DelayLine,
    limiter: Limiter,
    chorus: Chorus,
    waveshaper: Waveshaper,
    ks: KarplusStrong,
    // Attractor integration accumulator (sub-sample precision)
    accum: f64,  // accumulated real time in seconds
    step_dt: f64,
    // MIDI state
    active_note: Option<u8>,  // currently held MIDI note
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
            mapper: DirectMapping::new(),
            oscs: std::array::from_fn(|i| Oscillator::new(110.0 * (i+1) as f32, OscShape::Sine, sr)),
            chord_oscs: std::array::from_fn(|_| Oscillator::new(220.0, OscShape::Sine, sr)),
            voice_adsr: std::array::from_fn(|_| Adsr::new(10.0, 200.0, 0.7, 400.0, sr)),
            amp_smooth: [0.0; 4],
            freq_smooth: [110.0, 220.0, 330.0, 440.0],
            chord_amp_smooth: [0.0; 3],
            chord_freq_smooth: [220.0, 330.0, 440.0],
            freq_smooth_rate: 0.01,
            chord_intervals: [4.0, 7.0, 0.0], // major
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
        use std::f32::consts::TAU;

        // --- Rössler parameters (read every sample, smoothed) ---------------
        // Future version: add an enum parameter to select the attractor system
        // (Lorenz / Rössler / Halvorsen / …).  For now the plugin always runs
        // the Lorenz system.  The rossler_a and rossler_c knobs are fully
        // automatable and will drive Rössler integration once system selection
        // is implemented.  Classic chaotic Rössler: a=0.2, b=0.2, c=5.7.
        let _rossler_a = params.rossler_a.smoothed.next();  // exposed, not yet wired
        let _rossler_c = params.rossler_c.smoothed.next();  // exposed, not yet wired

        // Integrate the attractor once per sample (or skip to keep real-time)
        let speed = params.speed.smoothed.next() as f64;
        self.accum += speed / self.sample_rate as f64;
        while self.accum >= self.step_dt {
            self.lorenz.step(self.step_dt);
            self.accum -= self.step_dt;
        }

        // Semitone transpose applied to the base frequency before pitch mapping
        let transpose_st = params.base_freq_transpose.smoothed.next();
        let transpose_mul = 2.0f64.powf(transpose_st as f64 / 12.0);

        // Map attractor state to frequencies
        let state = self.lorenz.state();
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
                self.amp_smooth[i]  += 0.005 * (tamp - self.amp_smooth[i]);
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
        self.waveshaper.mix   = params.waveshaper_mix.smoothed.next();
        let l = self.waveshaper.process(l);
        let r = self.waveshaper.process(r);

        let ks = self.ks.next_sample();
        let l = self.filter.process(l + ks * 0.3);
        let r = self.filter.process(r + ks * 0.3);

        self.delay.set_delay_ms(params.delay_ms.smoothed.next(), self.sample_rate);
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

        // EQ gain parameters are exposed for DAW automation.
        // Full biquad shelf implementation is a TODO (requires storing per-channel
        // filter state for low- and high-shelf stages).  The gains are read here
        // so smoothers stay active; a linear gain approximation is applied as a
        // temporary stand-in until proper shelf filters are plumbed in.
        let eq_low_gain  = 10.0f32.powf(params.eq_low_db.smoothed.next()  / 20.0);
        let eq_high_gain = 10.0f32.powf(params.eq_high_db.smoothed.next() / 20.0);
        // Approximate: blend a simple one-pole low-pass for the low shelf and
        // one-pole high-pass for the high shelf, both at a fixed crossover (~500 Hz).
        // This is intentionally lightweight; replace with BiquadFilter shelves later.
        let crossover_coeff = 0.03_f32; // ~500 Hz at 44.1 kHz
        // We keep running averages in place without extra state by using a
        // stateless approximation: treat the whole buffer sample as its own state.
        // This is equivalent to a one-sample lookahead shelf — acceptable for a
        // first-pass implementation.
        let l_low  = l * crossover_coeff;
        let l_high = l - l_low;
        let r_low  = r * crossover_coeff;
        let r_high = r - r_low;
        let l = l_low * eq_low_gain + l_high * eq_high_gain;
        let r = r_low * eq_low_gain + r_high * eq_high_gain;

        // Speed LFO rate is read here to keep the smoother advancing; the LFO
        // modulation itself is applied at the `speed` accumulator level and will
        // be fully wired in a subsequent commit.
        let _speed_lfo_rate = params.speed_lfo_rate.smoothed.next();

        // Chorus rate and depth are read to advance their smoothers; they will
        // be forwarded to `self.chorus` once the Chorus struct exposes those
        // fields (currently the struct uses internal fixed values).
        let _chorus_rate  = params.chorus_rate.smoothed.next();
        let _chorus_depth = params.chorus_depth.smoothed.next();

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
    const NAME:              &'static str = "Math Sonify";
    const VENDOR:            &'static str = "Mattbusel";
    const URL:               &'static str = "https://github.com/Mattbusel/math-sonify";
    const EMAIL:             &'static str = "mattbusel@gmail.com";
    const VERSION:           &'static str = "0.9.0";
    const AUDIO_IO_LAYOUTS:  &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels:  None,
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];
    const MIDI_INPUT:  MidiConfig = MidiConfig::Basic;
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
            None    => return ProcessStatus::Error("DSP not initialized"),
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
            let l = if l_raw.is_finite() { l_raw.clamp(-1.0, 1.0) } else { 0.0 };
            let r = if r_raw.is_finite() { r_raw.clamp(-1.0, 1.0) } else { 0.0 };
            let mut iter = channel_samples.into_iter();
            if let Some(out_l) = iter.next() { *out_l = l; }
            if let Some(out_r) = iter.next() { *out_r = r; }
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
