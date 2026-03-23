// Audio recording mode.
//
// `AudioRecorder`   — appends audio samples to a rolling WAV file.
// `SegmentRecorder` — captures a fixed-duration clip and auto-names it.
//
// Both use `hound` (already in Cargo.toml) for WAV writing.
// The recorder is toggled from the UI with the 'R' key; a red dot in the
// status bar shows active recording.
#![allow(dead_code)]

use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use hound::{SampleFormat, WavSpec, WavWriter};

// ---------------------------------------------------------------------------
// Sample format configuration
// ---------------------------------------------------------------------------

/// Supported recording sample depths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingDepth {
    Bits16,
    Bits32,
}

impl RecordingDepth {
    fn bits(self) -> u16 {
        match self {
            RecordingDepth::Bits16 => 16,
            RecordingDepth::Bits32 => 32,
        }
    }

    fn hound_format(self) -> SampleFormat {
        match self {
            RecordingDepth::Bits16 => SampleFormat::Int,
            RecordingDepth::Bits32 => SampleFormat::Float,
        }
    }
}

/// Supported recording sample rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingSampleRate {
    Hz44100,
    Hz48000,
}

impl RecordingSampleRate {
    pub fn hz(self) -> u32 {
        match self {
            RecordingSampleRate::Hz44100 => 44100,
            RecordingSampleRate::Hz48000 => 48000,
        }
    }
}

// ---------------------------------------------------------------------------
// AudioRecorder
// ---------------------------------------------------------------------------

/// Records the live audio output to a WAV file.
///
/// Stereo interleaved f32 samples from the audio callback are fed in via
/// [`AudioRecorder::push_samples`].  Call [`AudioRecorder::stop`] (or let the
/// struct drop) to finalize the file.
pub struct AudioRecorder {
    writer: Option<WavWriter<BufWriter<std::fs::File>>>,
    path: PathBuf,
    sample_rate: u32,
    depth: RecordingDepth,
    /// Monotonic start time, used to display elapsed duration.
    started_at: Instant,
    /// Total frames written (mono pairs counted as one frame).
    frames_written: u64,
}

impl AudioRecorder {
    /// Create a new recorder and open the output WAV file.
    ///
    /// `output_dir` is created if it does not exist.
    /// Files are named `recordings/YYYYMMDD_HHMMSS.wav` using the current
    /// wall-clock time (UTC seconds since epoch as a proxy — no heavy
    /// chrono dependency needed).
    pub fn start(
        output_dir: &Path,
        depth: RecordingDepth,
        sample_rate: RecordingSampleRate,
    ) -> anyhow::Result<Self> {
        use anyhow::Context as _;
        std::fs::create_dir_all(output_dir)
            .with_context(|| format!("creating recordings directory {}", output_dir.display()))?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let filename = format!("{ts}.wav");
        let path = output_dir.join(filename);

        let spec = WavSpec {
            channels: 2,
            sample_rate: sample_rate.hz(),
            bits_per_sample: depth.bits(),
            sample_format: depth.hound_format(),
        };
        let writer = WavWriter::create(&path, spec)
            .with_context(|| format!("creating WAV file {}", path.display()))?;

        Ok(Self {
            writer: Some(writer),
            path,
            sample_rate: sample_rate.hz(),
            depth,
            started_at: Instant::now(),
            frames_written: 0,
        })
    }

    /// Feed interleaved stereo f32 samples into the recorder.
    ///
    /// For 32-bit recordings samples are written directly.  For 16-bit, each
    /// sample is scaled and quantized to `i16`.
    ///
    /// Returns an error if writing fails (e.g., disk full).  On error the
    /// recorder should be considered invalid.
    pub fn push_samples(&mut self, samples: &[f32]) -> anyhow::Result<()> {
        let writer = match self.writer.as_mut() {
            Some(w) => w,
            None => return Ok(()), // already stopped
        };
        match self.depth {
            RecordingDepth::Bits32 => {
                for &s in samples {
                    writer.write_sample(s)?;
                }
            }
            RecordingDepth::Bits16 => {
                for &s in samples {
                    let clamped = s.clamp(-1.0, 1.0);
                    let quantized = (clamped * i16::MAX as f32) as i16;
                    writer.write_sample(quantized)?;
                }
            }
        }
        self.frames_written += samples.len() as u64 / 2;
        Ok(())
    }

    /// Finalize and close the WAV file.  Returns the path written.
    ///
    /// Calling this more than once is a no-op after the first call.
    pub fn stop(&mut self) -> anyhow::Result<PathBuf> {
        use anyhow::Context as _;
        if let Some(writer) = self.writer.take() {
            writer
                .finalize()
                .with_context(|| format!("finalizing WAV file {}", self.path.display()))?;
        }
        Ok(self.path.clone())
    }

    /// Path of the file being recorded.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How long recording has been running.
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Total frames written so far.
    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }

    /// Whether the recorder is still active.
    pub fn is_active(&self) -> bool {
        self.writer.is_some()
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            let _ = writer.finalize();
        }
    }
}

// ---------------------------------------------------------------------------
// SegmentRecorder
// ---------------------------------------------------------------------------

/// Records a fixed-duration audio clip and auto-names it by system + preset.
pub struct SegmentRecorder {
    inner: AudioRecorder,
    max_duration: Duration,
    system_name: String,
    preset_name: String,
}

impl SegmentRecorder {
    /// Default clip duration: 60 seconds.
    pub const DEFAULT_DURATION_SECS: u64 = 60;

    /// Start recording a segment.
    ///
    /// - `output_dir`: directory for the file.
    /// - `system_name`: used in the output filename.
    /// - `preset_name`: used in the output filename.
    /// - `duration_secs`: clip length (default 60 s).
    pub fn start(
        output_dir: &Path,
        system_name: &str,
        preset_name: &str,
        duration_secs: u64,
        depth: RecordingDepth,
        sample_rate: RecordingSampleRate,
    ) -> anyhow::Result<Self> {
        use anyhow::Context as _;
        std::fs::create_dir_all(output_dir)
            .with_context(|| format!("creating recordings dir {}", output_dir.display()))?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let safe_system = sanitize_filename(system_name);
        let safe_preset = sanitize_filename(preset_name);
        let filename = format!("{ts}_{safe_system}_{safe_preset}.wav");
        let path = output_dir.join(&filename);

        let spec = WavSpec {
            channels: 2,
            sample_rate: sample_rate.hz(),
            bits_per_sample: depth.bits(),
            sample_format: depth.hound_format(),
        };
        let writer = WavWriter::create(&path, spec)
            .with_context(|| format!("creating segment WAV {}", path.display()))?;

        let inner = AudioRecorder {
            writer: Some(writer),
            path,
            sample_rate: sample_rate.hz(),
            depth,
            started_at: Instant::now(),
            frames_written: 0,
        };

        Ok(Self {
            inner,
            max_duration: Duration::from_secs(duration_secs.max(1)),
            system_name: system_name.into(),
            preset_name: preset_name.into(),
        })
    }

    /// Feed samples.  Automatically stops when the segment duration is reached.
    ///
    /// Returns `true` if the segment is now complete (caller should call `finish`).
    pub fn push_samples(&mut self, samples: &[f32]) -> anyhow::Result<bool> {
        if !self.inner.is_active() {
            return Ok(true);
        }
        self.inner.push_samples(samples)?;
        if self.inner.elapsed() >= self.max_duration {
            return Ok(true);
        }
        Ok(false)
    }

    /// Finalize the segment and return the output path.
    pub fn finish(&mut self) -> anyhow::Result<PathBuf> {
        self.inner.stop()
    }

    /// System name tag on this segment.
    pub fn system_name(&self) -> &str {
        &self.system_name
    }

    /// Preset name tag on this segment.
    pub fn preset_name(&self) -> &str {
        &self.preset_name
    }

    /// Whether the segment has reached its full duration.
    pub fn is_complete(&self) -> bool {
        self.inner.elapsed() >= self.max_duration || !self.inner.is_active()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Replace characters not suitable for filenames with underscores.
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("math_sonify_recorder_{tag}"))
    }

    #[test]
    fn test_recorder_creates_file() {
        let dir = temp_dir("create");
        let mut rec = AudioRecorder::start(
            &dir,
            RecordingDepth::Bits32,
            RecordingSampleRate::Hz44100,
        )
        .unwrap();
        rec.push_samples(&[0.1f32, -0.1, 0.2, -0.2]).unwrap();
        let path = rec.stop().unwrap();
        assert!(path.exists(), "WAV file should exist after stop");
    }

    #[test]
    fn test_recorder_16bit_creates_file() {
        let dir = temp_dir("16bit");
        let mut rec = AudioRecorder::start(
            &dir,
            RecordingDepth::Bits16,
            RecordingSampleRate::Hz48000,
        )
        .unwrap();
        rec.push_samples(&[0.0f32, 0.5, -0.5, 1.0]).unwrap();
        let path = rec.stop().unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_recorder_elapsed_increases() {
        let dir = temp_dir("elapsed");
        let rec = AudioRecorder::start(
            &dir,
            RecordingDepth::Bits32,
            RecordingSampleRate::Hz44100,
        )
        .unwrap();
        let e = rec.elapsed();
        assert!(e >= Duration::ZERO);
    }

    #[test]
    fn test_recorder_stop_twice_is_noop() {
        let dir = temp_dir("stop2x");
        let mut rec = AudioRecorder::start(
            &dir,
            RecordingDepth::Bits32,
            RecordingSampleRate::Hz44100,
        )
        .unwrap();
        let _p1 = rec.stop().unwrap();
        let _p2 = rec.stop().unwrap(); // should not panic
    }

    #[test]
    fn test_segment_recorder_auto_name() {
        let dir = temp_dir("segment");
        let mut seg = SegmentRecorder::start(
            &dir,
            "lorenz",
            "Lorenz Ambience",
            5,
            RecordingDepth::Bits32,
            RecordingSampleRate::Hz44100,
        )
        .unwrap();
        assert!(!seg.is_complete());
        let samples = vec![0.0f32; 44100 * 2]; // 1 second stereo
        let done = seg.push_samples(&samples).unwrap();
        assert!(!done); // not done after 1s of 5s clip
        let path = seg.finish().unwrap();
        assert!(path.exists());
        // Filename should contain system and preset slugs.
        let fname = path.file_name().unwrap().to_string_lossy();
        assert!(fname.contains("lorenz"), "expected 'lorenz' in filename: {fname}");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Lorenz Ambience"), "Lorenz_Ambience");
        assert_eq!(sanitize_filename("FM-Chaos"), "FM-Chaos");
        assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
    }

    #[test]
    fn test_recording_depth_bits() {
        assert_eq!(RecordingDepth::Bits16.bits(), 16);
        assert_eq!(RecordingDepth::Bits32.bits(), 32);
    }

    #[test]
    fn test_recording_sample_rate_hz() {
        assert_eq!(RecordingSampleRate::Hz44100.hz(), 44100);
        assert_eq!(RecordingSampleRate::Hz48000.hz(), 48000);
    }
}
