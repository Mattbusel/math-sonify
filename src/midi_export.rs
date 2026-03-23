//! MIDI export for math-sonify.
//!
//! Two distinct APIs live in this module:
//!
//! ## 1. Live recorder (`MidiRecorder` / `MidiFrame` / `coords_to_midi` / `export_midi`)
//!
//! Records a time series of pitch/velocity values derived from the running
//! attractor state and exports them as a Type-0 MIDI file via the `midly` crate.
//! This is the path used by the GUI "Record MIDI" button.
//!
//! ## 2. Trajectory exporter (`MidiExporter` / `MidiNote` / `MidiTrack`)
//!
//! Converts a pre-computed `&[(f64, f64, f64)]` trajectory snapshot directly
//! to a Standard MIDI File (SMF) without requiring the live recorder.  Useful
//! for headless export and DAW round-tripping.
//!
//! ### Mapping strategy
//! | Attractor coordinate | MIDI parameter |
//! |---|---|
//! | X | Note pitch quantised to the supplied scale |
//! | Y | Velocity (64-127) |
//! | Z | Note duration (16th note to whole note) |
//! | `tempo_bpm` argument | BPM stored in the file tempo event |
//! | channel index | MIDI channel (0-15) |

use std::io::Write as IoWrite;

use midly::{
    num::{u15, u24, u28, u4, u7},
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
};

// ---- Live recorder (original API) ------------------------------------------

/// A single recorded parameter frame (captured at control rate).
#[derive(Clone, Debug)]
pub struct MidiFrame {
    /// MIDI pitch (0-127).
    pub pitch: u8,
    /// MIDI velocity (1-127).
    pub velocity: u8,
    /// Duration in ticks (one tick = 1 control-rate step in the recording).
    pub ticks: u32,
}

/// Record buffer: accumulates frames during recording.
#[derive(Default)]
pub struct MidiRecorder {
    pub frames: Vec<MidiFrame>,
    pub max_frames: usize,
}

impl MidiRecorder {
    pub fn new(max_frames: usize) -> Self {
        Self { frames: Vec::with_capacity(max_frames), max_frames }
    }

    /// Push a new (pitch, velocity) frame.  Returns `false` if capacity reached.
    pub fn push(&mut self, pitch: u8, velocity: u8) -> bool {
        if self.frames.len() >= self.max_frames {
            return false;
        }
        if let Some(last) = self.frames.last_mut() {
            if last.pitch == pitch {
                last.ticks = last.ticks.saturating_add(1);
                return true;
            }
        }
        self.frames.push(MidiFrame { pitch, velocity, ticks: 1 });
        true
    }

    pub fn is_full(&self) -> bool {
        self.frames.len() >= self.max_frames
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// Convert attractor coordinates to MIDI pitch + velocity.
///
/// * `x_norm` - normalised x in \[-1, 1\]
/// * `y_norm` - normalised y magnitude in \[0, 1\]
/// * `base_midi` - MIDI note of the lowest note (e.g. 48 = C3)
/// * `semitone_range` - number of semitones spanning the octave range
/// * `scale_offsets` - semitone offsets of the scale degrees
pub fn coords_to_midi(
    x_norm: f64,
    y_norm: f64,
    base_midi: u8,
    semitone_range: u8,
    scale_offsets: &[i32],
) -> (u8, u8) {
    let t = ((x_norm + 1.0) * 0.5).clamp(0.0, 1.0);
    let semitone_float = t * f64::from(semitone_range);
    let octave = (semitone_float / 12.0) as i32;
    let semitone_in_octave = (semitone_float as i32) % 12;
    let closest_offset = scale_offsets
        .iter()
        .min_by_key(|&&o| (o - semitone_in_octave).abs())
        .copied()
        .unwrap_or(0);
    let raw_pitch = i32::from(base_midi) + octave * 12 + closest_offset;
    let pitch = raw_pitch.clamp(0, 127) as u8;
    let vel = (20.0 + y_norm * 107.0).clamp(1.0, 127.0) as u8;
    (pitch, vel)
}

/// Scale degree semitone offsets for common scales.
pub fn scale_offsets(scale: &str) -> Vec<i32> {
    match scale {
        "pentatonic"                    => vec![0, 2, 4, 7, 9],
        "major"                         => vec![0, 2, 4, 5, 7, 9, 11],
        "minor" | "natural_minor"       => vec![0, 2, 3, 5, 7, 8, 10],
        "chromatic"                     => (0..12).collect(),
        "blues"                         => vec![0, 3, 5, 6, 7, 10],
        "whole_tone"                    => vec![0, 2, 4, 6, 8, 10],
        "diminished" | "octatonic"      => vec![0, 2, 3, 5, 6, 8, 9, 11],
        "dorian"                        => vec![0, 2, 3, 5, 7, 9, 10],
        "phrygian"                      => vec![0, 1, 3, 5, 7, 8, 10],
        "lydian"                        => vec![0, 2, 4, 6, 7, 9, 11],
        "mixolydian"                    => vec![0, 2, 4, 5, 7, 9, 10],
        "harmonic_minor"                => vec![0, 2, 3, 5, 7, 8, 11],
        "hungarian_minor"               => vec![0, 2, 3, 6, 7, 8, 11],
        "hirajoshi"                     => vec![0, 2, 3, 7, 8],
        _                               => vec![0, 2, 4, 7, 9],
    }
}

/// Write recorded frames to a Type-0 MIDI file at `path`.
///
/// * `ticks_per_beat` - MIDI ticks per quarter note
/// * `tempo_us` - microseconds per beat (500 000 = 120 BPM)
/// * `channel` - MIDI channel 0-15
pub fn export_midi(
    frames: &[MidiFrame],
    path: &str,
    ticks_per_beat: u16,
    tempo_us: u32,
    channel: u4,
) -> anyhow::Result<()> {
    if frames.is_empty() {
        anyhow::bail!("No MIDI frames to export");
    }

    let mut events: Vec<TrackEvent<'static>> = Vec::new();

    events.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(tempo_us))),
    });

    let mut abs_tick: u32 = 0;
    let mut prev_abs: u32 = 0;

    for frame in frames {
        let dur = frame.ticks.max(1);
        let note = u7::new(frame.pitch.clamp(0, 127));
        let vel  = u7::new(frame.velocity.clamp(1, 127));

        let delta_on = abs_tick.saturating_sub(prev_abs);
        events.push(TrackEvent {
            delta: u28::new(delta_on),
            kind: TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOn { key: note, vel },
            },
        });
        prev_abs = abs_tick;
        abs_tick += dur;

        let delta_off = abs_tick.saturating_sub(prev_abs);
        events.push(TrackEvent {
            delta: u28::new(delta_off),
            kind: TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOff { key: note, vel: u7::new(0) },
            },
        });
        prev_abs = abs_tick;
    }

    events.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });

    let smf = Smf {
        header: Header {
            format: Format::SingleTrack,
            timing: Timing::Metrical(u15::new(ticks_per_beat)),
        },
        tracks: vec![events],
    };

    let mut buf = Vec::new();
    smf.write_std(&mut buf).map_err(|e| anyhow::anyhow!("MIDI write error: {e}"))?;
    std::fs::write(path, &buf).map_err(|e| anyhow::anyhow!("File write error: {e}"))?;
    Ok(())
}

// ---- Trajectory exporter (new API) -----------------------------------------

/// A single MIDI note produced by the trajectory exporter.
#[derive(Debug, Clone)]
pub struct MidiNote {
    /// MIDI channel (0-15).
    pub channel: u8,
    /// MIDI pitch (0-127).
    pub pitch: u8,
    /// MIDI velocity (0-127).
    pub velocity: u8,
    /// Absolute start time in MIDI ticks.
    pub start_tick: u32,
    /// Duration in MIDI ticks.
    pub duration_ticks: u32,
}

/// A complete MIDI track produced by the trajectory exporter.
#[derive(Debug, Clone)]
pub struct MidiTrack {
    /// Human-readable track name embedded in the SMF as a meta-event.
    pub name: String,
    /// Notes in this track (in any order; sorted by `start_tick` on export).
    pub notes: Vec<MidiNote>,
    /// Tempo in beats per minute.
    pub tempo_bpm: f64,
    /// Time signature numerator (e.g. 4 for 4/4).
    pub time_sig_num: u8,
    /// Time signature denominator as a plain integer (e.g. 4 = quarter note).
    pub time_sig_denom: u8,
}

/// Converts mathematical attractor trajectories to Standard MIDI Files.
///
/// Each `(x, y, z)` attractor point maps to one MIDI note:
///
/// | Coordinate | MIDI parameter |
/// |---|---|
/// | X | Pitch - quantised to `scale_notes` |
/// | Y | Velocity (64-127) |
/// | Z | Duration (16th to whole note) |
///
/// The output is a single-track SMF format-0 file compatible with all DAWs.
pub struct MidiExporter {
    /// MIDI ticks per quarter note.  480 is the standard DAW default.
    pub ticks_per_quarter: u16,
}

/// Pentatonic scale starting on C4 (MIDI 60).
pub const SCALE_PENTATONIC_C4: &[u8] = &[60, 62, 64, 67, 69, 72, 74, 76];

/// Natural minor scale starting on A3 (MIDI 57).
pub const SCALE_MINOR_A3: &[u8] = &[57, 59, 60, 62, 64, 65, 67, 69];

/// Whole-tone scale starting on C4 (MIDI 60).
pub const SCALE_WHOLE_TONE_C4: &[u8] = &[60, 62, 64, 66, 68, 70, 72, 74];

/// Chromatic scale across two octaves from C4.
pub const SCALE_CHROMATIC_C4: &[u8] = &[
    60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71,
    72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83,
];

impl Default for MidiExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiExporter {
    /// Create a new exporter with 480 ticks per quarter note.
    pub fn new() -> Self {
        Self { ticks_per_quarter: 480 }
    }

    /// Map a trajectory of `(x, y, z)` attractor points to a [`MidiTrack`].
    ///
    /// * `name`        - track name embedded in the SMF.
    /// * `trajectory`  - slice of `(x, y, z)` points from the ODE solver.
    /// * `scale_notes` - MIDI pitch values to use (e.g. [`SCALE_PENTATONIC_C4`]).
    /// * `tempo_bpm`   - playback tempo embedded in the SMF tempo meta-event.
    ///
    /// Each point becomes one note; the playback cursor advances by the note's
    /// duration before the next note starts, giving a monophonic melody.
    pub fn trajectory_to_track(
        &self,
        name: &str,
        trajectory: &[(f64, f64, f64)],
        scale_notes: &[u8],
        tempo_bpm: f64,
    ) -> MidiTrack {
        if trajectory.is_empty() || scale_notes.is_empty() {
            return MidiTrack {
                name: name.to_string(),
                notes: Vec::new(),
                tempo_bpm,
                time_sig_num: 4,
                time_sig_denom: 4,
            };
        }

        let (x_min, x_max) = axis_bounds(trajectory, |p| p.0);
        let (y_min, y_max) = axis_bounds(trajectory, |p| p.1);
        let (z_min, z_max) = axis_bounds(trajectory, |p| p.2);
        let tpq = self.ticks_per_quarter;

        let mut notes = Vec::with_capacity(trajectory.len());
        let mut cursor: u32 = 0;

        for &(x, y, z) in trajectory {
            let pitch          = self.pitch_from_x(x, x_min, x_max, scale_notes);
            let velocity       = self.velocity_from_y(y, y_min, y_max);
            let duration_ticks = self.duration_from_z(z, z_min, z_max, tpq);

            notes.push(MidiNote { channel: 0, pitch, velocity, start_tick: cursor, duration_ticks });
            cursor = cursor.saturating_add(duration_ticks);
        }

        MidiTrack { name: name.to_string(), notes, tempo_bpm, time_sig_num: 4, time_sig_denom: 4 }
    }

    /// Serialise one or more [`MidiTrack`]s to a Standard MIDI File (SMF format 0).
    ///
    /// Multiple tracks are merged into the single track required by format 0;
    /// each note's channel is set to the track's index (clamped to 0-15).
    pub fn export_smf(&self, tracks: &[MidiTrack]) -> Vec<u8> {
        let mut out = Vec::new();

        let tempo_bpm = tracks.first().map(|t| t.tempo_bpm).unwrap_or(120.0);
        let tempo_us  = bpm_to_microseconds(tempo_bpm);

        // SMF header: "MThd" + length(6) + format(0) + ntracks(1) + tpq
        out.extend_from_slice(b"MThd");
        push_u32(&mut out, 6);
        push_u16(&mut out, 0); // format 0
        push_u16(&mut out, 1); // one merged track
        push_u16(&mut out, self.ticks_per_quarter);

        // Merge all track notes, tagging each with its source track's channel.
        let mut merged: Vec<MidiNote> = Vec::new();
        for (idx, track) in tracks.iter().enumerate() {
            let ch = (idx as u8).min(15);
            for note in &track.notes {
                merged.push(MidiNote { channel: ch, ..note.clone() });
            }
        }
        merged.sort_by(|a, b| a.start_tick.cmp(&b.start_tick).then(a.pitch.cmp(&b.pitch)));

        // Build track data.
        let mut track_buf = Vec::new();

        // Time signature meta (4/4 unless overridden).
        let ts_num    = tracks.first().map(|t| t.time_sig_num).unwrap_or(4);
        let ts_den_pw = tracks.first().map(|t| denom_to_power(t.time_sig_denom)).unwrap_or(2);
        write_var_len(0, &mut track_buf);
        track_buf.extend_from_slice(&[0xFF, 0x58, 0x04, ts_num, ts_den_pw, 24, 8]);

        // Tempo meta.
        write_var_len(0, &mut track_buf);
        track_buf.extend_from_slice(&[0xFF, 0x51, 0x03]);
        track_buf.push(((tempo_us >> 16) & 0xFF) as u8);
        track_buf.push(((tempo_us >>  8) & 0xFF) as u8);
        track_buf.push( (tempo_us        & 0xFF) as u8);

        // Expand notes into (tick, kind, channel, pitch, velocity) events.
        #[derive(Ord, PartialOrd, Eq, PartialEq)]
        struct Ev { tick: u32, kind_order: u8, ch: u8, pitch: u8, vel: u8 }

        let mut raw: Vec<Ev> = Vec::with_capacity(merged.len() * 2);
        for n in &merged {
            raw.push(Ev { tick: n.start_tick,                          kind_order: 0, ch: n.channel, pitch: n.pitch, vel: n.velocity });
            raw.push(Ev { tick: n.start_tick.saturating_add(n.duration_ticks), kind_order: 1, ch: n.channel, pitch: n.pitch, vel: 0 });
        }
        raw.sort();

        let mut prev_tick: u32 = 0;
        for ev in &raw {
            let delta  = ev.tick.saturating_sub(prev_tick);
            let status = if ev.kind_order == 0 { 0x90 | (ev.ch & 0x0F) } else { 0x80 | (ev.ch & 0x0F) };
            write_var_len(delta, &mut track_buf);
            track_buf.push(status);
            track_buf.push(ev.pitch.min(127));
            track_buf.push(ev.vel.min(127));
            prev_tick = ev.tick;
        }

        // End of track.
        write_var_len(0, &mut track_buf);
        track_buf.extend_from_slice(&[0xFF, 0x2F, 0x00]);

        // Track chunk: "MTrk" + length + data.
        out.extend_from_slice(b"MTrk");
        push_u32(&mut out, track_buf.len() as u32);
        out.extend_from_slice(&track_buf);
        out
    }

    /// Write the SMF bytes to a file at `path`.
    pub fn export_to_file(&self, tracks: &[MidiTrack], path: &str) -> std::io::Result<()> {
        let bytes = self.export_smf(tracks);
        let mut file = std::fs::File::create(path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    // ---- private helpers ---------------------------------------------------

    fn pitch_from_x(&self, x: f64, x_min: f64, x_max: f64, scale: &[u8]) -> u8 {
        let range = (x_max - x_min).abs();
        let t = if range < f64::EPSILON { 0.5 } else { ((x - x_min) / range).clamp(0.0, 1.0) };
        let idx = (t * (scale.len() - 1) as f64).round() as usize;
        scale[idx.min(scale.len() - 1)]
    }

    fn velocity_from_y(&self, y: f64, y_min: f64, y_max: f64) -> u8 {
        let range = (y_max - y_min).abs();
        let t = if range < f64::EPSILON { 0.5 } else { ((y - y_min) / range).clamp(0.0, 1.0) };
        (64.0 + t * 63.0).round() as u8
    }

    /// Map z to a duration between a 16th note and a whole note using
    /// exponential interpolation so short values stay granular.
    fn duration_from_z(&self, z: f64, z_min: f64, z_max: f64, tpq: u16) -> u32 {
        let range = (z_max - z_min).abs();
        let t = if range < f64::EPSILON { 0.5 } else { ((z - z_min) / range).clamp(0.0, 1.0) };
        let sixteenth = u32::from(tpq) / 4;  // e.g. 120 at 480 tpq
        let whole     = u32::from(tpq) * 4;  // e.g. 1920 at 480 tpq
        let log_min   = (sixteenth as f64).ln();
        let log_max   = (whole     as f64).ln();
        ((log_min + t * (log_max - log_min)).exp().round() as u32).max(1)
    }
}

// ---- Internal helpers -------------------------------------------------------

fn axis_bounds(traj: &[(f64, f64, f64)], f: impl Fn(&(f64, f64, f64)) -> f64) -> (f64, f64) {
    let mut mn = f64::MAX;
    let mut mx = f64::MIN;
    for p in traj {
        let v = f(p);
        if v < mn { mn = v; }
        if v > mx { mx = v; }
    }
    if mn > mx { (0.0, 1.0) } else { (mn, mx) }
}

fn bpm_to_microseconds(bpm: f64) -> u32 {
    if bpm <= 0.0 { return 500_000; }
    ((60.0 / bpm) * 1_000_000.0).round() as u32
}

fn write_var_len(mut value: u32, out: &mut Vec<u8>) {
    let mut buf = [0u8; 4];
    let mut n = 0usize;
    loop {
        buf[n] = (value & 0x7F) as u8;
        n += 1;
        value >>= 7;
        if value == 0 { break; }
    }
    for i in (0..n).rev() {
        let byte = if i == 0 { buf[i] } else { buf[i] | 0x80 };
        out.push(byte);
    }
}

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.push(((v >> 24) & 0xFF) as u8);
    out.push(((v >> 16) & 0xFF) as u8);
    out.push(((v >>  8) & 0xFF) as u8);
    out.push( (v        & 0xFF) as u8);
}

fn push_u16(out: &mut Vec<u8>, v: u16) {
    out.push(((v >> 8) & 0xFF) as u8);
    out.push( (v       & 0xFF) as u8);
}

fn denom_to_power(denom: u8) -> u8 {
    match denom { 1 => 0, 2 => 1, 4 => 2, 8 => 3, 16 => 4, 32 => 5, _ => 2 }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // MidiRecorder

    #[test]
    fn recorder_merges_equal_pitch() {
        let mut rec = MidiRecorder::new(100);
        rec.push(60, 80);
        rec.push(60, 80);
        assert_eq!(rec.len(), 1);
        assert_eq!(rec.frames[0].ticks, 2);
    }

    #[test]
    fn recorder_respects_max_frames() {
        let mut rec = MidiRecorder::new(2);
        rec.push(60, 80);
        rec.push(62, 80);
        assert!(!rec.push(64, 80));
        assert!(rec.is_full());
    }

    #[test]
    fn recorder_is_empty_on_new() {
        assert!(MidiRecorder::new(100).is_empty());
    }

    // coords_to_midi

    #[test]
    fn coords_to_midi_clamps_pitch() {
        let (p, _) = coords_to_midi(2.0, 0.5, 48, 36, &[0, 2, 4, 7, 9]);
        assert!(p <= 127);
    }

    #[test]
    fn coords_to_midi_velocity_range() {
        for yn in [0.0f64, 0.5, 1.0] {
            let (_, v) = coords_to_midi(0.0, yn, 48, 24, &[0, 2, 4, 7, 9]);
            assert!((1..=127).contains(&v));
        }
    }

    // scale_offsets

    #[test]
    fn scale_offsets_all_known() {
        for name in &["pentatonic","major","minor","chromatic","blues","whole_tone",
                      "diminished","dorian","phrygian","lydian","mixolydian",
                      "harmonic_minor","hungarian_minor","hirajoshi"] {
            assert!(!scale_offsets(name).is_empty());
        }
    }

    // MidiExporter helpers

    #[test]
    fn pitch_from_x_stays_in_scale() {
        let ex = MidiExporter::new();
        for x in [-20.0f64, -1.0, 0.0, 1.0, 20.0] {
            let p = ex.pitch_from_x(x, -20.0, 20.0, SCALE_PENTATONIC_C4);
            assert!(SCALE_PENTATONIC_C4.contains(&p));
        }
    }

    #[test]
    fn velocity_from_y_range() {
        let ex = MidiExporter::new();
        for y in [-10.0f64, 0.0, 10.0] {
            let v = ex.velocity_from_y(y, -10.0, 10.0);
            assert!((64..=127).contains(&v));
        }
    }

    #[test]
    fn duration_from_z_bounds() {
        let ex  = MidiExporter::new();
        let tpq = 480u16;
        let min = u32::from(tpq) / 4;
        let max = u32::from(tpq) * 4;
        for z in [-5.0f64, 0.0, 5.0] {
            let d = ex.duration_from_z(z, -5.0, 5.0, tpq);
            assert!(d >= min && d <= max, "duration {d} outside [{min},{max}]");
        }
    }

    // trajectory_to_track

    #[test]
    fn trajectory_to_track_empty() {
        let ex = MidiExporter::new();
        let track = ex.trajectory_to_track("empty", &[], SCALE_PENTATONIC_C4, 120.0);
        assert!(track.notes.is_empty());
    }

    #[test]
    fn trajectory_to_track_note_count() {
        let ex   = MidiExporter::new();
        let traj: Vec<(f64,f64,f64)> = (0..32).map(|i| (i as f64*0.1, (i as f64*0.07).sin(), (i as f64*0.13).cos())).collect();
        let track = ex.trajectory_to_track("test", &traj, SCALE_PENTATONIC_C4, 120.0);
        assert_eq!(track.notes.len(), 32);
    }

    #[test]
    fn trajectory_to_track_start_ticks_monotone() {
        let ex   = MidiExporter::new();
        let traj: Vec<(f64,f64,f64)> = (0..16).map(|i| (i as f64, i as f64, i as f64)).collect();
        let track = ex.trajectory_to_track("mono", &traj, SCALE_MINOR_A3, 90.0);
        let mut prev = 0u32;
        for note in &track.notes {
            assert!(note.start_tick >= prev);
            prev = note.start_tick;
        }
    }

    // export_smf

    #[test]
    fn export_smf_header_magic() {
        let ex   = MidiExporter::new();
        let traj  = vec![(1.0f64, 2.0, 3.0), (-1.0, -2.0, -3.0)];
        let track = ex.trajectory_to_track("hdr", &traj, SCALE_PENTATONIC_C4, 120.0);
        let smf   = ex.export_smf(&[track]);
        assert_eq!(&smf[0..4],   b"MThd");
        assert_eq!(&smf[14..18], b"MTrk");
    }

    #[test]
    fn export_smf_format_0() {
        let ex   = MidiExporter::new();
        let traj  = vec![(0.0f64, 0.0, 0.0)];
        let t1    = ex.trajectory_to_track("A", &traj, SCALE_PENTATONIC_C4, 120.0);
        let t2    = ex.trajectory_to_track("B", &traj, SCALE_MINOR_A3,      120.0);
        let smf   = ex.export_smf(&[t1, t2]);
        assert_eq!(smf[8],  0x00, "format high byte");
        assert_eq!(smf[9],  0x00, "format must be 0");
        assert_eq!(smf[10], 0x00, "ntracks high byte");
        assert_eq!(smf[11], 0x01, "ntracks must be 1 for format-0");
    }

    #[test]
    fn export_smf_tpq_written() {
        let mut ex = MidiExporter::new();
        ex.ticks_per_quarter = 960;
        let track = ex.trajectory_to_track("tpq", &[(0.0,0.0,0.0)], SCALE_PENTATONIC_C4, 120.0);
        let smf   = ex.export_smf(&[track]);
        assert_eq!(u16::from_be_bytes([smf[12], smf[13]]), 960);
    }

    // write_var_len

    #[test]
    fn var_len_single_byte() {
        let mut buf = Vec::new();
        write_var_len(0x40, &mut buf);
        assert_eq!(buf, vec![0x40]);
    }

    #[test]
    fn var_len_two_bytes() {
        let mut buf = Vec::new();
        write_var_len(0x80, &mut buf);
        assert_eq!(buf, vec![0x81, 0x00]);
    }

    #[test]
    fn var_len_3fff() {
        let mut buf = Vec::new();
        write_var_len(0x3FFF, &mut buf);
        assert_eq!(buf, vec![0xFF, 0x7F]);
    }

    // bpm_to_microseconds

    #[test]
    fn bpm_120() { assert_eq!(bpm_to_microseconds(120.0), 500_000); }
    #[test]
    fn bpm_60()  { assert_eq!(bpm_to_microseconds(60.0), 1_000_000); }
    #[test]
    fn bpm_zero_guard() { assert_eq!(bpm_to_microseconds(0.0), 500_000); }

    // export_to_file

    #[test]
    fn export_to_file_round_trip() {
        use std::io::Read;
        let ex    = MidiExporter::new();
        let traj  = vec![(1.0f64, 0.5, 0.3), (-1.0, 0.8, 0.1)];
        let track = ex.trajectory_to_track("rt", &traj, SCALE_PENTATONIC_C4, 120.0);
        let tmp   = std::env::temp_dir().join("math_sonify_test_midi.mid");
        ex.export_to_file(&[track], tmp.to_str().unwrap()).unwrap();
        let mut file = std::fs::File::open(&tmp).unwrap();
        let mut buf  = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(&buf[0..4], b"MThd");
        let _ = std::fs::remove_file(&tmp);
    }
}
