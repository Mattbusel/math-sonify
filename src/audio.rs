/// Audio thread: consumes AudioParams from a lock-free ring buffer,
/// synthesizes stereo PCM, applies effects chain, outputs via cpal.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound;
use cpal::{Stream, SampleFormat};
use std::sync::Arc;
use parking_lot::Mutex;
use crossbeam_channel::Receiver;

use crate::sonification::{AudioParams, SonifMode};
use crate::synth::{
    Oscillator, OscShape, BiquadFilter, Freeverb, DelayLine, Limiter, GrainEngine, Bitcrusher,
    KarplusStrong, Chorus, Waveshaper,
};

pub type WavRecorder = Arc<parking_lot::Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>;
/// When Some(n), audio thread records n more samples then finalizes.
pub type LoopExportPending = Arc<parking_lot::Mutex<Option<u64>>>;

pub struct AudioEngine {
    _stream: Stream,
}

impl AudioEngine {
    pub fn start(
        params_rx: Receiver<AudioParams>,
        sample_rate: u32,
        reverb_wet: f32,
        delay_ms: f32,
        delay_feedback: f32,
        master_volume: f32,
        waveform: Arc<Mutex<Vec<f32>>>,
        recording: WavRecorder,
        loop_export: LoopExportPending,
    ) -> anyhow::Result<(Self, u32)> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No audio output device"))?;

        // Use the device's default config and read back the actual sample rate.
        let default_config = device.default_output_config()?;
        let actual_sr = default_config.sample_rate().0;
        let fmt = default_config.sample_format();
        log::info!("Audio: {} Hz, {:?}", actual_sr, fmt);

        let sr = actual_sr as f32;
        let synth_state = Arc::new(Mutex::new(SynthState::new(sr, reverb_wet, delay_ms, delay_feedback, waveform, recording, loop_export)));
        synth_state.lock().master_volume = master_volume;
        let synth_state_clone = synth_state.clone();
        let stream_config = default_config.config();

        let stream = match fmt {
            SampleFormat::F32 => {
                let ss = synth_state_clone.clone();
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let params = { let mut l = None; while let Ok(p) = params_rx.try_recv() { l = Some(p); } l };
                        let mut state = ss.lock();
                        if let Some(p) = params { state.update_params(p); }
                        state.render(data);
                    },
                    |err| log::error!("Audio stream error: {err}"),
                    None,
                )?
            }
            SampleFormat::I16 => {
                let ss = synth_state_clone.clone();
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        let params = { let mut l = None; while let Ok(p) = params_rx.try_recv() { l = Some(p); } l };
                        let mut state = ss.lock();
                        if let Some(p) = params { state.update_params(p); }
                        let mut buf = vec![0.0f32; data.len()];
                        state.render(&mut buf);
                        for (d, s) in data.iter_mut().zip(buf.iter()) {
                            *d = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        }
                    },
                    |err| log::error!("Audio stream error: {err}"),
                    None,
                )?
            }
            SampleFormat::U16 => {
                let ss = synth_state_clone.clone();
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        let params = { let mut l = None; while let Ok(p) = params_rx.try_recv() { l = Some(p); } l };
                        let mut state = ss.lock();
                        if let Some(p) = params { state.update_params(p); }
                        let mut buf = vec![0.0f32; data.len()];
                        state.render(&mut buf);
                        for (d, s) in data.iter_mut().zip(buf.iter()) {
                            *d = ((s.clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32) as u16;
                        }
                    },
                    |err| log::error!("Audio stream error: {err}"),
                    None,
                )?
            }
            _ => anyhow::bail!("Unsupported audio sample format: {:?}", fmt),
        };

        stream.play()?;
        Ok((Self { _stream: stream }, actual_sr))
    }
}

/// All mutable DSP state lives here, owned exclusively by the audio callback.
struct SynthState {
    sample_rate: f32,
    params: AudioParams,
    master_volume: f32,
    oscs: [Oscillator; 4],
    chord_oscs: [Oscillator; 3],
    filter: BiquadFilter,
    reverb: Freeverb,
    delay: DelayLine,
    limiter: Limiter,
    grains: GrainEngine,
    bitcrusher: Bitcrusher,
    ks: KarplusStrong,
    chorus: Chorus,
    waveshaper: Waveshaper,
    partial_phases: [f32; 32],
    amp_smooth: [f32; 4],
    freq_smooth: [f32; 4],
    chord_amp_smooth: [f32; 3],
    chord_freq_smooth: [f32; 3],
    freq_smooth_rate: f32,
    chord_intervals: [f32; 3],
    fm_phase: f32,
    fm_mod_phase: f32,
    pub waveform: Arc<Mutex<Vec<f32>>>,
    pub recording: WavRecorder,
    pub loop_export: LoopExportPending,
    loop_recorder: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
}

impl SynthState {
    fn new(sample_rate: f32, reverb_wet: f32, delay_ms: f32, delay_feedback: f32,
           waveform: Arc<Mutex<Vec<f32>>>, recording: WavRecorder, loop_export: LoopExportPending) -> Self {
        let mut reverb = Freeverb::new(sample_rate);
        reverb.wet = reverb_wet;
        let mut delay = DelayLine::new(2000.0, sample_rate);
        delay.set_delay_ms(delay_ms, sample_rate);
        delay.feedback = delay_feedback;
        delay.mix = 0.25;

        Self {
            sample_rate,
            master_volume: 0.7,
            params: AudioParams::default(),
            oscs: std::array::from_fn(|i| {
                Oscillator::new(220.0 * (i + 1) as f32, OscShape::Sine, sample_rate)
            }),
            chord_oscs: [
                Oscillator::new(330.0, OscShape::Sine, sample_rate),
                Oscillator::new(440.0, OscShape::Sine, sample_rate),
                Oscillator::new(550.0, OscShape::Sine, sample_rate),
            ],
            filter: BiquadFilter::low_pass(2000.0, 0.7, sample_rate),
            reverb,
            delay,
            limiter: Limiter::new(-1.0, 5.0, sample_rate),
            grains: GrainEngine::new(sample_rate),
            bitcrusher: Bitcrusher::new(),
            ks: KarplusStrong::new(50.0, sample_rate),
            chorus: Chorus::new(sample_rate),
            waveshaper: Waveshaper::new(),
            partial_phases: [0.0; 32],
            amp_smooth: [0.0; 4],
            freq_smooth: [220.0, 440.0, 660.0, 880.0],
            chord_amp_smooth: [0.0; 3],
            chord_freq_smooth: [330.0, 440.0, 550.0],
            freq_smooth_rate: 0.01,
            chord_intervals: [0.0; 3],
            fm_phase: 0.0,
            fm_mod_phase: 0.0,
            waveform,
            recording,
            loop_export,
            loop_recorder: None,
        }
    }

    fn update_params(&mut self, params: AudioParams) {
        self.filter.update_lp(params.filter_cutoff, params.filter_q, self.sample_rate);
        self.grains.spawn_rate = params.grain_spawn_rate;
        self.grains.base_freq = params.grain_base_freq;
        self.grains.freq_spread = params.grain_freq_spread;
        self.freq_smooth_rate = (1.0 / (params.portamento_ms.max(1.0) * 0.001 * self.sample_rate)).clamp(0.001, 1.0);
        self.chord_intervals = params.chord_intervals;
        self.master_volume = params.master_volume;
        self.reverb.wet = params.reverb_wet.clamp(0.0, 1.0);
        self.delay.feedback = params.delay_feedback.clamp(0.0, 0.9); // cap < 1 to prevent runaway
        self.delay.set_delay_ms(params.delay_ms.max(1.0), self.sample_rate);
        // Bitcrusher
        self.bitcrusher.bit_depth = params.bit_depth;
        self.bitcrusher.rate_crush = params.rate_crush;
        // Karplus-Strong
        if params.ks_trigger && params.ks_freq > 20.0 {
            self.ks.trigger(params.ks_freq, self.sample_rate);
        }
        self.ks.volume = params.ks_volume;
        // Chorus
        self.chorus.mix = params.chorus_mix;
        self.chorus.rate = params.chorus_rate;
        self.chorus.depth = params.chorus_depth;
        // Waveshaper
        self.waveshaper.drive = params.waveshaper_drive;
        self.waveshaper.mix = params.waveshaper_mix;
        // Voice shapes
        for i in 0..4 {
            self.oscs[i].shape = params.voice_shapes[i];
        }
        self.params = params;
    }

    fn render(&mut self, data: &mut [f32]) {
        let master_vol = self.master_volume;
        let chunk = data.chunks_exact_mut(2);
        for frame in chunk {
            let (l, r) = self.next_stereo_sample();
            frame[0] = l * master_vol;
            frame[1] = r * master_vol;
        }
    }

    fn next_stereo_sample(&mut self) -> (f32, f32) {
        let (l, r) = match self.params.mode {
            SonifMode::Direct | SonifMode::Orbital => self.synth_additive_voices(),
            SonifMode::Granular => self.grains.next_sample(),
            SonifMode::Spectral => self.synth_spectral(),
            SonifMode::FM => self.synth_fm(),
        };

        // Waveshaper (before filter)
        let l = self.waveshaper.process(l);
        let r = self.waveshaper.process(r);

        // Karplus-Strong mixed in before filter
        let ks_sample = self.ks.next_sample();
        let lf = self.filter.process(l + ks_sample * 0.5);
        let rf = self.filter.process(r + ks_sample * 0.5);

        // Bitcrusher
        let lf = self.bitcrusher.process(lf);
        let rf = self.bitcrusher.process(rf);

        let (ld, rd) = self.delay.process(lf, rf);
        let (lc, rc) = self.chorus.process(ld, rd);
        let (lrev, rrev) = self.reverb.process(lc, rc);
        let (lo_raw, ro_raw) = self.limiter.process(lrev, rrev);
        // Final NaN/inf guard — any upstream corruption ends here, never reaches the driver
        let (lo, ro) = (
            if lo_raw.is_finite() { lo_raw } else { 0.0 },
            if ro_raw.is_finite() { ro_raw } else { 0.0 },
        );

        // Capture waveform non-blocking
        if let Some(mut wf) = self.waveform.try_lock() {
            wf.push(lo);
            let excess = wf.len().saturating_sub(2048);
            if excess > 0 { wf.drain(0..excess); }
        }

        // WAV recording (non-blocking)
        if let Some(mut rec) = self.recording.try_lock() {
            if let Some(ref mut writer) = *rec {
                let _ = writer.write_sample(lo);
                let _ = writer.write_sample(ro);
            }
        }

        // Loop export countdown (non-blocking check)
        self.handle_loop_export(lo, ro);

        (lo, ro)
    }

    fn handle_loop_export(&mut self, lo: f32, ro: f32) {
        // Check if a loop export has been requested
        if let Some(mut pending) = self.loop_export.try_lock() {
            match *pending {
                Some(n) if n > 0 => {
                    // We're actively loop-recording
                    if self.loop_recorder.is_none() {
                        // Start loop recorder
                        let secs = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let filename = format!("loop_{}.wav", secs);
                        let spec = hound::WavSpec {
                            channels: 2,
                            sample_rate: self.sample_rate as u32,
                            bits_per_sample: 32,
                            sample_format: hound::SampleFormat::Float,
                        };
                        if let Ok(writer) = hound::WavWriter::create(&filename, spec) {
                            self.loop_recorder = Some(writer);
                        }
                    }
                    if let Some(ref mut writer) = self.loop_recorder {
                        let _ = writer.write_sample(lo);
                        let _ = writer.write_sample(ro);
                    }
                    *pending = Some(n - 1);
                }
                Some(0) => {
                    // Done, finalize
                    if let Some(writer) = self.loop_recorder.take() {
                        let _ = writer.finalize();
                    }
                    *pending = None;
                }
                _ => {}
            }
        }
    }

    /// Simple polyphonic voices (Direct / Orbital modes).
    fn synth_additive_voices(&mut self) -> (f32, f32) {
        let gain = self.params.gain;
        let transpose_ratio = 2.0f32.powf(self.params.transpose_semitones / 12.0);
        let mut l = 0.0f32;
        let mut r = 0.0f32;

        for i in 0..4 {
            let target_freq = self.params.freqs[i] * transpose_ratio;
            let target_amp  = self.params.amps[i] * self.params.voice_levels[i];
            if target_freq > 10.0 {
                self.freq_smooth[i] += self.freq_smooth_rate * (target_freq - self.freq_smooth[i]);
                self.amp_smooth[i]  += 0.005 * (target_amp - self.amp_smooth[i]);
                self.oscs[i].freq = self.freq_smooth[i];
                let sig = self.oscs[i].next_sample() * self.amp_smooth[i] * gain;
                let pan = self.params.pans[i].clamp(-1.0, 1.0);
                l += sig * (1.0 - pan.max(0.0));
                r += sig * (1.0 + pan.min(0.0));
            }
        }

        // Chord voices derived from voice[0]
        let voice0_freq = self.freq_smooth[0];
        for k in 0..3 {
            let interval = self.chord_intervals[k];
            if interval.abs() > 0.001 {
                let target_chord_freq = voice0_freq * 2.0f32.powf(interval / 12.0);
                let target_chord_amp = self.params.amps[0] * self.params.voice_levels[0] * 0.7;
                self.chord_freq_smooth[k] += self.freq_smooth_rate * (target_chord_freq - self.chord_freq_smooth[k]);
                self.chord_amp_smooth[k]  += 0.005 * (target_chord_amp - self.chord_amp_smooth[k]);
                self.chord_oscs[k].freq = self.chord_freq_smooth[k];
                let sig = self.chord_oscs[k].next_sample() * self.chord_amp_smooth[k] * gain;
                let pan = (k as f32 / 2.0) * 2.0 - 1.0;
                l += sig * (1.0 - pan.max(0.0));
                r += sig * (1.0 + pan.min(0.0));
            } else {
                self.chord_amp_smooth[k] += 0.005 * (0.0 - self.chord_amp_smooth[k]);
            }
        }

        (l * 0.5, r * 0.5)
    }

    /// Additive synthesis from spectral partials.
    fn synth_spectral(&mut self) -> (f32, f32) {
        use std::f32::consts::TAU;
        let base = self.params.partials_base_freq;
        let gain = self.params.gain;
        let mut out = 0.0f32;

        for k in 0..32 {
            let freq = base * (k + 1) as f32;
            self.partial_phases[k] =
                (self.partial_phases[k] + TAU * freq / self.sample_rate) % TAU;
            out += self.partial_phases[k].sin() * self.params.partials[k];
        }
        let mono = out * gain;
        (mono, mono)
    }

    /// 2-operator FM synthesis.
    fn synth_fm(&mut self) -> (f32, f32) {
        use std::f32::consts::TAU;
        let carrier_freq = self.params.fm_carrier_freq;
        let mod_freq = carrier_freq * self.params.fm_mod_ratio;
        let mod_index = self.params.fm_mod_index;
        let gain = self.params.gain;

        // Advance modulator phase
        self.fm_mod_phase = (self.fm_mod_phase + TAU * mod_freq / self.sample_rate).rem_euclid(TAU);
        // PM-style FM: output = sin(carrier_phase + mod_index * sin(mod_phase))
        self.fm_phase = (self.fm_phase + TAU * carrier_freq / self.sample_rate).rem_euclid(TAU);

        let out = (self.fm_phase + mod_index * self.fm_mod_phase.sin()).sin() * gain;
        (out, out)
    }
}
