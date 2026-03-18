/// Digital waveguide string — two traveling-wave delay lines.
///
/// Improvements over the original:
///
/// 1. **Fractional-delay read** via linear interpolation: the delay length in
///    samples is rarely an integer, so integer rounding causes pitch quantization
///    errors (audible beating at low frequencies).  Interpolation gives true
///    intonation.
///
/// 2. **First-order IIR loop filter** (tunable brightness): replaces the naive
///    `(a + b) * 0.5` average.  The filter coefficient `b` trades off between a
///    bright metallic string (b ≈ 0) and a dark muffled one (b ≈ 0.9), with
///    independent control from the overall damping envelope.
///
/// 3. **Allpass dispersion filter** in the loop: a first-order allpass shifts
///    the phase of higher partials, causing the upper harmonics to arrive
///    slightly later than the fundamental — the defining "stiff string" quality
///    of piano and other struck/plucked strings.  Without this every partial
///    decays in exact lockstep and the result sounds like a pure electronic tone.

pub struct WaveguideString {
    delay_fwd: Vec<f32>,
    delay_bck: Vec<f32>,
    write_fwd: usize,
    write_bck: usize,
    sample_rate: f32,
    pub tension:    f32,  // 0..1 → frequency scaling
    pub damping:    f32,  // feedback coefficient (e.g. 0.995 = slow decay)
    pub brightness: f32,  // IIR loop filter coefficient: 0 = bright, 0.5 = balanced, 0.9 = dark
    pub dispersion: f32,  // allpass coefficient: 0 = none, 0.5 = moderate stiffness
    pub length: f32,      // delay line length in samples
    pub excite: bool,
    pub excite_pos: f32,
    noise_seed: u64,
    // Loop filter state
    lp_state: f32,
    // Allpass state
    ap_state: f32,
}

const MAX_DELAY: usize = 4096;

impl WaveguideString {
    /// Create a new waveguide string model tuned to the given sample rate.
    ///
    /// The string is initialized at A3 (220 Hz) with balanced brightness and
    /// mild dispersion.  Call [`WaveguideString::set_freq`] before the first
    /// excitation to tune it to the desired fundamental.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            delay_fwd: vec![0.0; MAX_DELAY],
            delay_bck: vec![0.0; MAX_DELAY],
            write_fwd: 0,
            write_bck: 0,
            sample_rate,
            tension:    0.5,
            damping:    0.995,  // raised from 0.98 — slower, more natural decay
            brightness: 0.35,   // balanced; user can push toward dark (0.7) or bright (0.0)
            dispersion: 0.12,   // mild stiffness; 0 = ideal string, 0.3 = piano-like
            length: (sample_rate / 220.0).clamp(2.0, MAX_DELAY as f32 - 2.0),
            excite: false,
            excite_pos: 0.5,
            noise_seed: 1234567,
            lp_state: 0.0,
            ap_state: 0.0,
        }
    }

    /// Set the string fundamental frequency in Hz.
    ///
    /// The actual pitch is further modulated by `self.tension` (exponential mapping
    /// around unity at 0.5).
    pub fn set_freq(&mut self, hz: f32) {
        let hz = hz.max(10.0);
        // Exponential mapping: tension [0,1] → frequency [0.25x, 4.0x], unity at 0.5
        let scaled = hz * 2.0f32.powf((self.tension - 0.5) * 2.0);
        self.length = (self.sample_rate / scaled).clamp(2.0, MAX_DELAY as f32 - 2.0);
    }

    fn lcg_noise(&mut self) -> f32 {
        self.noise_seed = self.noise_seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // >> 33 yields 31 bits; divide by 2^31 for unbiased [-1, 1) range.
        (self.noise_seed >> 33) as f32 / (1u64 << 31) as f32 * 2.0 - 1.0
    }

    /// Read from a circular delay buffer at a fractional position behind `write`.
    #[inline(always)]
    fn read_frac(buf: &[f32], write: usize, delay: f32) -> f32 {
        let len = buf.len();
        let d0 = delay as usize;
        let frac = delay - d0 as f32;
        let i0 = (write + len - d0.min(len - 1)) % len;
        let i1 = (write + len - (d0 + 1).min(len - 1)) % len;
        buf[i0] * (1.0 - frac) + buf[i1] * frac
    }

    pub fn next_sample(&mut self) -> f32 {
        let len_f = self.length.max(2.0).min(MAX_DELAY as f32 - 2.0);

        // Excite: fill both delay lines with a noise burst at excite_pos
        if self.excite {
            let len = len_f as usize;
            let inject = ((self.excite_pos * len as f32) as usize).min(len - 1);
            for k in 0..len {
                let n = self.lcg_noise();
                self.delay_fwd[(inject + k) % len] = n;
                self.delay_bck[(inject + k) % len] = n;
            }
            self.write_fwd = inject;
            self.write_bck = inject;
            self.lp_state = 0.0;
            self.ap_state = 0.0;
            self.excite = false;
        }

        // Read both traveling waves with fractional interpolation
        let half = len_f * 0.5;
        let fwd_read = Self::read_frac(&self.delay_fwd, self.write_fwd, half);
        let bck_read = Self::read_frac(&self.delay_bck, self.write_bck, half);

        // --- Loop filter chain applied at the bridge reflection ---
        // 1. Mix traveling waves at reflection boundary
        let at_bridge = fwd_read + bck_read;

        // 2. IIR lowpass (brightness control): y[n] = (1-b)*x[n] + b*y[n-1]
        let b = self.brightness;
        self.lp_state = (1.0 - b) * at_bridge + b * self.lp_state;
        let after_lp = self.lp_state;

        // 3. Allpass dispersion filter: y[n] = c*(x[n] - y[n-1]) + x[n-1]
        //    Shifts phase of high frequencies → slight inharmonicity (stiffness)
        let c = self.dispersion;
        let ap_in = after_lp;
        let ap_out = c * (ap_in - self.ap_state) + self.ap_state;
        self.ap_state = ap_in;

        // 4. Apply global damping coefficient
        let fed_back = ap_out * self.damping;

        // Write back into both delay lines (reflection inverts phase on backward wave)
        self.delay_fwd[self.write_fwd] = fed_back;
        self.delay_bck[self.write_bck] = -fed_back;

        // Advance write pointers using active loop length to prevent overrun on freq change
        let active_len = len_f as usize;
        self.write_fwd = (self.write_fwd + 1) % active_len.max(4);
        self.write_bck = (self.write_bck + 1) % active_len.max(4);

        // Output is the forward-traveling wave at the nut end
        fwd_read
    }
}
