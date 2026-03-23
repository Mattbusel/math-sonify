//! Scale mapper v2 — maps numeric data series to musical notes and events.
//!
//! Provides a standalone data-to-MIDI mapping pipeline independent of the
//! attractor-based `scale_mapper` module. Suitable for driving live input
//! sonification from arbitrary numeric streams.

// ---------------------------------------------------------------------------
// MusicalScale enum
// ---------------------------------------------------------------------------

/// A musical scale variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicalScale {
    Major,
    Minor,
    Pentatonic,
    Blues,
    WholeTone,
    Chromatic,
    Dorian,
    Phrygian,
}

/// Return the semitone intervals (from root) that define each scale.
pub fn scale_intervals(scale: &MusicalScale) -> &'static [u8] {
    match scale {
        MusicalScale::Major      => &[0, 2, 4, 5, 7, 9, 11],
        MusicalScale::Minor      => &[0, 2, 3, 5, 7, 8, 10],
        MusicalScale::Pentatonic => &[0, 2, 4, 7, 9],
        MusicalScale::Blues      => &[0, 3, 5, 6, 7, 10],
        MusicalScale::WholeTone  => &[0, 2, 4, 6, 8, 10],
        MusicalScale::Chromatic  => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        MusicalScale::Dorian     => &[0, 2, 3, 5, 7, 9, 10],
        MusicalScale::Phrygian   => &[0, 1, 3, 5, 7, 8, 10],
    }
}

// ---------------------------------------------------------------------------
// ScaleMapper
// ---------------------------------------------------------------------------

/// Maps a stream of numeric values to MIDI notes in a given musical scale.
#[derive(Debug, Clone)]
pub struct ScaleMapper {
    pub scale: MusicalScale,
    /// MIDI root note (0–127).
    pub root_midi: u8,
    /// Number of octaves to span.
    pub octave_range: u8,
}

impl ScaleMapper {
    pub fn new(scale: MusicalScale, root_midi: u8, octave_range: u8) -> Self {
        Self { scale, root_midi, octave_range }
    }

    /// Map `value` in `[min, max]` to a MIDI note in the configured scale.
    pub fn map_to_note(&self, value: f64, min: f64, max: f64) -> u8 {
        let intervals = scale_intervals(&self.scale);
        let n_degrees = intervals.len() as u8;
        let total_notes = n_degrees * self.octave_range.max(1);

        let t = normalize(value, min, max);
        let idx = (t * (total_notes as f64 - 1.0)).round() as u8;
        let idx = idx.min(total_notes - 1);

        let octave = idx / n_degrees;
        let degree = idx % n_degrees;
        let semitone = intervals[degree as usize];

        let midi = self.root_midi as u16 + octave as u16 * 12 + semitone as u16;
        midi.min(127) as u8
    }

    /// Map `value` in `[min, max]` to a MIDI velocity in [20, 127].
    pub fn map_to_velocity(&self, value: f64, min: f64, max: f64) -> u8 {
        let t = normalize(value, min, max);
        (20.0 + t * (127.0 - 20.0)).round().clamp(20.0, 127.0) as u8
    }

    /// Map `value` in `[min, max]` to a duration in milliseconds within
    /// `[min_ms, max_ms]`.
    pub fn map_to_duration(&self, value: f64, min: f64, max: f64, min_ms: u32, max_ms: u32) -> u32 {
        let t = normalize(value, min, max);
        let dur = min_ms as f64 + t * (max_ms as f64 - min_ms as f64);
        dur.round() as u32
    }
}

/// Clamp-normalise `value` ∈ [min, max] → [0.0, 1.0].
fn normalize(value: f64, min: f64, max: f64) -> f64 {
    if (max - min).abs() < 1e-12 {
        return 0.0;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// DataSonificationEvent
// ---------------------------------------------------------------------------

/// A single MIDI-like event produced by the sonification pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSonificationEvent {
    pub midi_note: u8,
    pub velocity: u8,
    pub duration_ms: u32,
    pub channel: u8,
}

// ---------------------------------------------------------------------------
// map_series
// ---------------------------------------------------------------------------

/// Convert a series of data values to sonification events using `config`.
///
/// The global min/max of the series is used for normalisation so that the
/// full scale range is utilised regardless of the data magnitude.
pub fn map_series(data: &[f64], config: &ScaleMapper) -> Vec<DataSonificationEvent> {
    if data.is_empty() {
        return Vec::new();
    }
    let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    data.iter()
        .map(|&v| DataSonificationEvent {
            midi_note: config.map_to_note(v, min, max),
            velocity: config.map_to_velocity(v, min, max),
            duration_ms: config.map_to_duration(v, min, max, 100, 500),
            channel: 0,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// chord_from_value
// ---------------------------------------------------------------------------

/// Return a 3-note chord (root, third, fifth of the scale degree chosen for
/// `value`) as MIDI note numbers.
pub fn chord_from_value(
    value: f64,
    min: f64,
    max: f64,
    scale: &MusicalScale,
    root: u8,
) -> Vec<u8> {
    let intervals = scale_intervals(scale);
    let n = intervals.len();
    if n == 0 {
        return vec![];
    }
    let t = normalize(value, min, max);
    let degree = (t * (n as f64 - 1.0)).round() as usize;
    let degree = degree.min(n - 1);

    // Third is 2 degrees up, fifth is 4 degrees up (wrapping into next octave).
    let third_degree = (degree + 2) % n;
    let fifth_degree = (degree + 4) % n;

    let third_octave  = if (degree + 2) >= n { 12u8 } else { 0 };
    let fifth_octave  = if (degree + 4) >= n { 12u8 } else { 0 };

    let root_note  = (root as u16 + intervals[degree] as u16).min(127) as u8;
    let third_note = (root as u16 + intervals[third_degree] as u16 + third_octave as u16).min(127) as u8;
    let fifth_note = (root as u16 + intervals[fifth_degree] as u16 + fifth_octave as u16).min(127) as u8;

    vec![root_note, third_note, fifth_note]
}

// ---------------------------------------------------------------------------
// midi_note_to_freq
// ---------------------------------------------------------------------------

/// Convert a MIDI note number to its frequency in Hz.
/// A4 (MIDI 69) = 440 Hz.
pub fn midi_note_to_freq(midi: u8) -> f64 {
    440.0 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn major_scale_intervals_correct() {
        let intervals = scale_intervals(&MusicalScale::Major);
        assert_eq!(intervals, &[0, 2, 4, 5, 7, 9, 11]);
    }

    #[test]
    fn value_mapping_clamps_to_range() {
        let mapper = ScaleMapper::new(MusicalScale::Major, 60, 2);
        // Values outside the range should be clamped.
        let note_low  = mapper.map_to_note(-100.0, 0.0, 10.0);
        let note_high = mapper.map_to_note(999.0,  0.0, 10.0);
        // Root at minimum, max note at maximum.
        assert_eq!(note_low, 60); // root MIDI note
        // High value maps to last degree of 2 octaves: 60 + 12*2 - 1 interval offset
        let intervals = scale_intervals(&MusicalScale::Major);
        let expected_high = (60u16 + 12 + intervals[intervals.len() - 1] as u16).min(127) as u8;
        assert_eq!(note_high, expected_high);
    }

    #[test]
    fn chord_has_three_notes() {
        let chord = chord_from_value(5.0, 0.0, 10.0, &MusicalScale::Major, 60);
        assert_eq!(chord.len(), 3);
    }

    #[test]
    fn a4_is_440_hz() {
        let freq = midi_note_to_freq(69);
        assert!((freq - 440.0).abs() < 1e-6, "freq was {}", freq);
    }

    #[test]
    fn map_series_returns_same_length() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mapper = ScaleMapper::new(MusicalScale::Pentatonic, 60, 2);
        let events = map_series(&data, &mapper);
        assert_eq!(events.len(), data.len());
    }

    #[test]
    fn velocity_range() {
        let mapper = ScaleMapper::new(MusicalScale::Minor, 48, 1);
        assert_eq!(mapper.map_to_velocity(0.0, 0.0, 1.0), 20);
        assert_eq!(mapper.map_to_velocity(1.0, 0.0, 1.0), 127);
    }
}
