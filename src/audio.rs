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
    Oscillator, OscShape, BiquadFilter, Freeverb, DelayLine, Limiter, GrainEngine,
};

pub type WavRecorder = Arc<parking_lot::Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>;

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
        // Initialize with config values; thereafter updated dynamically via AudioParams
        let synth_state = Arc::new(Mutex::new(SynthState::new(sr, reverb_wet, delay_ms, delay_feedback, waveform, recording)));
        // Store master_volume in the SynthState initial value
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
    partial_phases: [f32; 32],
    amp_smooth: [f32; 4],
    freq_smooth: [f32; 4],
    chord_amp_smooth: [f32; 3],
    chord_freq_smooth: [f32; 3],
    freq_smooth_rate: f32,
    chord_intervals: [f32; 3],
    pub waveform: Arc<Mutex<Vec<f32>>>,
    pub recording: WavRecorder,
}

impl SynthState {
    fn new(sample_rate: f32, reverb_wet: f32, delay_ms: f32, delay_feedback: f32, waveform: Arc<Mutex<Vec<f32>>>, recording: WavRecorder) -> Self {
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
            partial_phases: [0.0; 32],
            amp_smooth: [0.0; 4],
            freq_smooth: [220.0, 440.0, 660.0, 880.0],
            chord_amp_smooth: [0.0; 3],
            chord_freq_smooth: [330.0, 440.0, 550.0],
            freq_smooth_rate: 0.01,
            chord_intervals: [0.0; 3],
            waveform,
            recording,
        }
    }

    fn update_params(&mut self, params: AudioParams) {
        self.filter.update_lp(params.filter_cutoff, params.filter_q, self.sample_rate);
        self.grains.spawn_rate = params.grain_spawn_rate;
        self.grains.base_freq = params.grain_base_freq;
        self.grains.freq_spread = params.grain_freq_spread;
        self.freq_smooth_rate = (1.0 / (params.portamento_ms.max(1.0) * 0.001 * self.sample_rate)).clamp(0.001, 1.0);
        self.chord_intervals = params.chord_intervals;
        // Route dynamic audio parameters from config through AudioParams
        self.master_volume = params.master_volume;
        self.reverb.wet = params.reverb_wet;
        self.delay.feedback = params.delay_feedback;
        self.delay.set_delay_ms(params.delay_ms, self.sample_rate);
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
        };

        let lf = self.filter.process(l);
        let rf = self.filter.process(r);

        let (ld, rd) = self.delay.process(lf, rf);
        let (lrev, rrev) = self.reverb.process(ld, rd);
        let (lo, ro) = self.limiter.process(lrev, rrev);

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

        (lo, ro)
    }

    /// Simple polyphonic sine voices (Direct / Orbital modes).
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
                // Drive oscillator even when silent to keep phase consistent
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
}
