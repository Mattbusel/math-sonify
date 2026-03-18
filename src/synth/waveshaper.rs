/// Waveshaper — soft saturation with tube-style harmonic content.
///
/// The original used a symmetric `tanh` curve which adds only odd harmonics
/// (3rd, 5th, 7th...) — the transistor or diode character.
///
/// Real vacuum tubes are slightly asymmetric because the triode's transfer curve
/// is not exactly symmetric around zero.  Asymmetry introduces *even* harmonics
/// (2nd, 4th...) which sit at the octave and two-octave positions — they blend
/// naturally with the fundamental and make the saturation sound warm rather than
/// harsh.
///
/// Implementation: blend `tanh(x)` with `tanh(x + k·x²)` where k controls the
/// asymmetry amount.  At k=0 this reduces to pure symmetric tanh.
pub struct Waveshaper {
    pub drive: f32,
    pub mix: f32,
    /// Tube asymmetry coefficient (0 = symmetric transistor, 0.3 = mild tube warmth).
    /// Values above 0.5 become noticeably biased and start to sound like an overdrive pedal.
    pub asymmetry: f32,
}

impl Waveshaper {
    /// Create a new waveshaper with unity drive, zero mix (bypass), and mild tube asymmetry.
    pub fn new() -> Self {
        Self { drive: 1.0, mix: 0.0, asymmetry: 0.22 }
    }

    /// Process one audio sample through the waveshaper and return the distorted output.
    ///
    /// Returns the input unchanged when `self.mix < 0.001`.
    pub fn process(&self, x: f32) -> f32 {
        if self.mix < 0.001 { return x; }

        let driven = x * self.drive;

        // Symmetric path: odd harmonics (transistor / diode character)
        let sym = driven.tanh();

        // Asymmetric path: even + odd harmonics (tube character)
        // The x² term breaks symmetry; tanh keeps it bounded.
        let k = self.asymmetry.clamp(0.0, 0.6);
        let asym = (driven + k * driven.abs()).tanh();

        // Blend: asymmetry=0 → pure sym, asymmetry=0.22 → warm tube mix
        let shaped = sym * (1.0 - k) + asym * k;

        // Compensate for gain reduction at high drive
        let comp = 1.0 / self.drive.max(1.0).sqrt();
        x * (1.0 - self.mix) + shaped * comp * self.mix
    }
}
