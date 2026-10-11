use rodio::Source;
use std::time::Duration;

// ─── Xorshift PRNG ────────────────────────────────────────────────────────────
pub struct PRNG { seed: u32 }
impl PRNG {
    #[inline(always)]
    pub fn new(seed: u32) -> Self { Self { seed: seed | 1 } }
    #[inline(always)]
    pub fn next_f32(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed as i32) as f32 / i32::MAX as f32
    }
    #[inline(always)]
    pub fn next_unit_f32(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed as f32) / (u32::MAX as f32)
    }
}

// ─── TDF-II Biquad Filter ─────────────────────────────────────────────────────
#[derive(Clone, Copy)]
pub struct Biquad {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    s1: f32, s2: f32,
}

impl Biquad {
    pub fn lowpass(sr: f32, fc: f32, q: f32) -> Self {
        let w = 2.0 * std::f32::consts::PI * (fc / sr).clamp(0.001, 0.495);
        let cw = w.cos();
        let alpha = w.sin() / (2.0 * q.max(0.1));
        let a0r = 1.0 / (1.0 + alpha);
        let b = (1.0 - cw) * 0.5;
        Self { b0: b * a0r, b1: (1.0 - cw) * a0r, b2: b * a0r,
               a1: -2.0 * cw * a0r, a2: (1.0 - alpha) * a0r, s1: 0.0, s2: 0.0 }
    }

    pub fn bandpass(sr: f32, fc: f32, q: f32) -> Self {
        let w = 2.0 * std::f32::consts::PI * (fc / sr).clamp(0.001, 0.495);
        let cw = w.cos();
        let alpha = w.sin() / (2.0 * q.max(0.1));
        let a0r = 1.0 / (1.0 + alpha);
        Self { b0: alpha * a0r, b1: 0.0, b2: -alpha * a0r,
               a1: -2.0 * cw * a0r, a2: (1.0 - alpha) * a0r, s1: 0.0, s2: 0.0 }
    }

    pub fn highpass(sr: f32, fc: f32, q: f32) -> Self {
        let w = 2.0 * std::f32::consts::PI * (fc / sr).clamp(0.001, 0.495);
        let cw = w.cos();
        let alpha = w.sin() / (2.0 * q.max(0.1));
        let a0r = 1.0 / (1.0 + alpha);
        let b = (1.0 + cw) * 0.5;
        Self { b0: b * a0r, b1: -(1.0 + cw) * a0r, b2: b * a0r,
               a1: -2.0 * cw * a0r, a2: (1.0 - alpha) * a0r, s1: 0.0, s2: 0.0 }
    }

    pub fn peaking(sr: f32, fc: f32, q: f32, gain_db: f32) -> Self {
        let a = 10.0f32.powf(gain_db / 40.0);
        let w = 2.0 * std::f32::consts::PI * (fc / sr).clamp(0.001, 0.495);
        let cw = w.cos();
        let sw = w.sin();
        let alpha = sw / (2.0 * q.max(0.1));
        let a0r = 1.0 / (1.0 + alpha / a);
        Self {
            b0: (1.0 + alpha * a) * a0r,
            b1: (-2.0 * cw) * a0r,
            b2: (1.0 - alpha * a) * a0r,
            a1: (-2.0 * cw) * a0r,
            a2: (1.0 - alpha / a) * a0r,
            s1: 0.0,
            s2: 0.0,
        }
    }

    #[inline(always)]
    pub fn update_lowpass(&mut self, sr: f32, fc: f32, q: f32) {
        let w = 2.0 * std::f32::consts::PI * (fc / sr).clamp(0.001, 0.495);
        let cw = w.cos();
        let alpha = w.sin() / (2.0 * q.max(0.1));
        let a0r = 1.0 / (1.0 + alpha);
        let b = (1.0 - cw) * 0.5;
        self.b0 = b * a0r;
        self.b1 = (1.0 - cw) * a0r;
        self.b2 = b * a0r;
        self.a1 = -2.0 * cw * a0r;
        self.a2 = (1.0 - alpha) * a0r;
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }
}

// ─── Fast Algebraic Soft Limiter (Strictly bounded to [-1.0, 1.0]) ───────────
#[inline(always)]
pub fn soft_limit(x: f32) -> f32 {
    x / (1.0 + x * x).sqrt()
}

// ─── Config ───────────────────────────────────────────────────────────────────
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ProceduralConfig {
    pub switch_type: String,     // "cream"|"linear"|"red"|"black"|"tactile"|"brown"|"clicky"|"blue"|"box_jade"
    pub spring_weight: f32,      // 35–80g
    pub lube_amount: f32,        // 0.0–1.0
    pub pitch: f32,              // 0.5–2.0
    pub keycap_material: String, // "abs"|"pbt"
    pub plate_material: String,  // "fr4"|"brass"|"aluminum"|"nylon"|"pom"|"abs"|"polycarbonate"
    pub mounting_style: String,  // "tray"|"gasket"|"top"
    pub foam_mod: f32,           // 0.0–1.0
    pub case_material: String,   // "plastic"|"aluminum"|"polycarbonate"
    pub o_rings: f32,            // 0.0–1.0
}

impl Default for ProceduralConfig {
    fn default() -> Self {
        Self {
            switch_type: "cream".to_string(),
            spring_weight: 45.0,
            lube_amount: 0.30,
            pitch: 1.0,
            keycap_material: "abs".to_string(),
            plate_material: "pom".to_string(),
            mounting_style: "top".to_string(),
            foam_mod: 0.05,
            case_material: "plastic".to_string(),
            o_rings: 0.0,
        }
    }
}

// ─── Resonant Modal Structural Ping (Inharmonic Chladni Plate Modes) ─────────
#[derive(Clone, Copy)]
struct ModalMode {
    u: f32, v: f32,
    cos_w: f32, sin_w: f32,
    decay: f32,
    amp: f32,
}

impl ModalMode {
    fn new(freq: f32, decay_secs: f32, amp: f32, sr: f32) -> Self {
        let tau = decay_secs.max(0.001) / 6.908;
        let w = 2.0 * std::f32::consts::PI * freq.clamp(20.0, sr * 0.48) / sr;
        Self {
            u: 0.0, v: 0.0, // starts at 0 -> zero step discontinuity clicks
            cos_w: w.cos(), sin_w: w.sin(),
            decay: (-1.0 / (sr * tau)).exp(),
            amp,
        }
    }

    #[inline(always)]
    fn excite(&mut self, impulse: f32) {
        self.v += impulse * self.amp;
    }

    #[inline(always)]
    fn step(&mut self) -> f32 {
        let next_u = (self.u * self.cos_w - self.v * self.sin_w) * self.decay;
        let next_v = (self.u * self.sin_w + self.v * self.cos_w) * self.decay;
        self.u = next_u;
        self.v = next_v;
        self.v
    }
}

// ─── Procedural Switch Synthesizer ────────────────────────────────────────────
pub struct ProceduralSwitch {
    sr: u32,
    cursor: u64,
    total: u64,
    dt: f32,
    t: f32,
    prng: PRNG,

    // Contact mechanics (smooth physical rise + decay)
    t_rise: f32,
    tau_decay: f32,
    impact_vol: f32,

    // Acoustic Formants: cavity "thock" & switch housing "clack"
    thock_cavity: Biquad,
    thock_vol: f32,
    clack_housing: Biquad,
    clack_vol: f32,
    presence_lpf: Biquad,
    topout_hpf: Option<Biquad>,

    // Inharmonic structural modes (excited naturally by impulse)
    modes: [ModalMode; 3],

    // Click / Tactile mechanism
    is_clicky: bool,
    is_tactile: bool,
    click_bpf: Biquad,
    click_t0: f32,
    click_vol: f32,

    // Anti-click release fadeout
    fade_len: u64,
}

impl ProceduralSwitch {
    pub fn new(config: &ProceduralConfig, is_keyup: bool, velocity: f32) -> Self {
        let sr = 44100u32;
        let srf = sr as f32;
        let vel = velocity.clamp(0.85, 1.20);
        let spring_scale = (config.spring_weight / 55.0).clamp(0.7, 1.3);

        // ── 1. Top-out (Keyup) vs. Bottom-out (Keydown) ────────────────────────
        let (impact_vol, keyup_pitch_mult, topout_hpf) = if is_keyup {
            // Natural top-out: -15 dB quieter, high-passed (no heavy plate thock)
            let vol = spring_scale * 0.18;
            let hpf = Biquad::highpass(srf, 550.0, 0.707);
            (vol, 1.85f32, Some(hpf))
        } else {
            // Natural bottom-out: full finger kinetic energy
            let vol = spring_scale * vel * 0.85;
            (vol, 1.0f32, None)
        };

        // ── 2. Contact Mechanics (Smooth Rise + Exponential Fall) ─────────────
        // Lubing softens the impact edge (longer rise, smoother impact)
        let t_rise = (0.0003 + config.lube_amount * 0.0004) * (if is_keyup { 0.6 } else { 1.0 });
        let base_decay_ms: f32 = match config.switch_type.as_str() {
            "cream"                             => 3.5,
            "linear" | "red" | "black"         => 3.0,
            "tactile" | "brown" | "holy_panda" => 4.2,
            "clicky" | "blue" | "box_jade"     => 2.5,
            _                                  => 3.2,
        };
        let tau_decay = (base_decay_ms / 1000.0) * (1.0 + config.foam_mod * 0.35);

        // ── 3. Dual Formant Cavity Tuning (The Physical "Thock" & "Clack") ────
        // Formant 1: Deep cavity resonance (Keycap + Case air volume)
        let f_cavity: f32 = match config.plate_material.as_str() {
            "polycarbonate" | "pc" => 360.0,
            "pom"                  => 420.0,
            "nylon"                => 460.0,
            "abs"                  => 520.0,
            "fr4"                  => 560.0,
            "aluminum"             => 620.0,
            "brass"                => 680.0,
            _                      => 450.0,
        } * config.pitch * keyup_pitch_mult * (1.0 - config.foam_mod * 0.20);

        // Formant 2: Switch housing impact clack (stiff plastic contact)
        let f_clack: f32 = match config.plate_material.as_str() {
            "polycarbonate" | "pc" => 1100.0,
            "pom"                  => 1350.0,
            "nylon"                => 1500.0,
            "abs"                  => 1750.0,
            "fr4"                  => 2200.0,
            "aluminum"             => 2600.0,
            "brass"                => 3000.0,
            _                      => 1500.0,
        } * config.pitch * keyup_pitch_mult * (1.0 - config.lube_amount * 0.25);

        let q_cavity = 2.2 + config.foam_mod * 1.5;
        let q_clack = 3.0 + config.lube_amount * 1.0;

        let thock_cavity = Biquad::bandpass(srf, f_cavity, q_cavity);
        let clack_housing = Biquad::bandpass(srf, f_clack, q_clack);

        // Relative balance between deep thock and high clack
        let (thock_vol, clack_vol) = if is_keyup {
            (0.15, 0.85) // keyup is mostly high-frequency clack
        } else {
            let tv = 0.70 + config.foam_mod * 0.25;
            let cv = (0.35 - config.foam_mod * 0.15).max(0.10);
            (tv, cv)
        };

        // Output presence filter (removes unnatural hiss above 5.5 kHz)
        let lp_fc = (5800.0 - config.lube_amount * 1500.0 - config.foam_mod * 1200.0).max(2200.0);
        let presence_lpf = Biquad::lowpass(srf, lp_fc, 0.707);

        // ── 4. Inharmonic Plate Modes (Continuous, Impulse-Excited) ───────────
        // Chladni plate mode ratios (1.0 : 1.48 : 2.14)
        let plate_f0 = f_cavity * 1.5;
        let plate_decay = if is_keyup { 0.003 } else { 0.008 * (1.0 - config.foam_mod * 0.6) };
        let mode_vol = if is_keyup { 0.015 } else { 0.045 * (1.0 - config.lube_amount * 0.4) };

        let modes = [
            ModalMode::new(plate_f0,        plate_decay,        mode_vol,        srf),
            ModalMode::new(plate_f0 * 1.48, plate_decay * 0.65, mode_vol * 0.35, srf),
            ModalMode::new(plate_f0 * 2.14, plate_decay * 0.35, mode_vol * 0.15, srf),
        ];

        // ── 5. Clicky / Tactile Mechanism ─────────────────────────────────────
        let is_clicky = !is_keyup && matches!(config.switch_type.as_str(), "clicky"|"blue"|"white"|"box_jade");
        let is_tactile = !is_keyup && matches!(config.switch_type.as_str(), "tactile"|"brown"|"holy_panda");

        let click_bpf = if is_clicky {
            Biquad::bandpass(srf, 3200.0 * config.pitch, 3.5)
        } else if is_tactile {
            Biquad::bandpass(srf, 650.0 * config.pitch, 1.8) // warm tactile bump
        } else {
            Biquad::bandpass(srf, 1000.0, 1.0)
        };
        let click_t0 = 0.0022; // snaps 2.2 ms into press
        let click_vol = if is_clicky { 0.38 * spring_scale * vel }
                        else if is_tactile { 0.18 * spring_scale * vel }
                        else { 0.0 };

        // ── 6. Duration & Safe Anti-Click Fade ─────────────────────────────────
        let tail_ms: f32 = if is_keyup { 24.0 } else { 45.0 };
        let total = (srf * tail_ms / 1000.0) as u64;
        let fade_len = (srf * 0.005) as u64; // smooth 5 ms fade-out at end

        static SEED_CTR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        let seed = SEED_CTR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .wrapping_add((f_cavity * 97.0) as u32)
            .wrapping_mul(2654435761);

        Self {
            sr, cursor: 0, total, dt: 1.0 / srf, t: 0.0,
            prng: PRNG::new(seed),
            t_rise, tau_decay, impact_vol,
            thock_cavity, thock_vol,
            clack_housing, clack_vol,
            presence_lpf, topout_hpf,
            modes,
            is_clicky, is_tactile, click_bpf, click_t0, click_vol,
            fade_len,
        }
    }
}

impl Iterator for ProceduralSwitch {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        if self.cursor >= self.total { return None; }
        let t = self.t;
        let cur = self.cursor;
        self.cursor += 1;
        self.t += self.dt;

        // 1. Smooth physical contact pulse (no sample-0 click)
        let pulse = if t < self.t_rise {
            (std::f32::consts::PI * 0.5 * (t / self.t_rise)).sin()
        } else {
            (-(t - self.t_rise) / self.tau_decay).exp()
        };

        // 2. High-frequency micro-roughness noise (attenuated with pulse)
        let noise = self.prng.next_f32() * pulse * 0.20;
        let excitation = (pulse + noise) * self.impact_vol;

        // 3. Excite inharmonic plate modes with contact impulse
        if cur < 5 {
            let imp = excitation / 5.0;
            self.modes[0].excite(imp);
            self.modes[1].excite(imp);
            self.modes[2].excite(imp);
        }
        let modal_tail = self.modes[0].step() + self.modes[1].step() + self.modes[2].step();

        // 4. Excite acoustic cavity & housing clack formants
        let thock = self.thock_cavity.process(excitation) * self.thock_vol;
        let clack = self.clack_housing.process(excitation) * self.clack_vol;

        // 5. Clicky snap / Tactile bump (Hann-windowed burst, zero edge clicks)
        let click = if (self.is_clicky || self.is_tactile) && t > self.click_t0 {
            let dt = t - self.click_t0;
            let width = if self.is_clicky { 0.0018 } else { 0.0035 };
            if dt < width {
                let env = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * dt / width).cos());
                let n = self.prng.next_f32();
                self.click_bpf.process(n) * env * self.click_vol
            } else { 0.0 }
        } else { 0.0 };

        let mut out = thock + clack + modal_tail + click;

        // Filter presence and apply keyup highpass if applicable
        out = self.presence_lpf.process(out);
        if let Some(hpf) = &mut self.topout_hpf {
            out = hpf.process(out);
        }

        // 6. Anti-pop release fade-out at end of buffer
        let rem = self.total - cur;
        if rem < self.fade_len {
            out *= rem as f32 / self.fade_len as f32;
        }

        // 7. Bounded algebraic soft limiter
        Some(soft_limit(out * 1.25))
    }
}

impl Source for ProceduralSwitch {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.sr }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_millis((self.total * 1000 / self.sr as u64) as u64))
    }
}

// ─── ASMR Engine ──────────────────────────────────────────────────────────────
use std::sync::atomic::{AtomicU32, Ordering};

pub static ASMR_RAIN_ON: AtomicU32 = AtomicU32::new(0);
pub static ASMR_WIND_ON: AtomicU32 = AtomicU32::new(0);
pub static ASMR_THUNDER_ON: AtomicU32 = AtomicU32::new(0);
pub static ASMR_MASTER_VOL: AtomicU32 = AtomicU32::new(50);
pub static ASMR_RAIN_DENS: AtomicU32 = AtomicU32::new(50);
pub static ASMR_WIND_GUST: AtomicU32 = AtomicU32::new(30);
pub static ASMR_RAIN_VOL: AtomicU32 = AtomicU32::new(70);
pub static ASMR_WIND_VOL: AtomicU32 = AtomicU32::new(40);
pub static ASMR_THUNDER_FREQ: AtomicU32 = AtomicU32::new(50);
pub static ASMR_THUNDER_INT: AtomicU32 = AtomicU32::new(70);
pub static ASMR_THUNDER_VOL: AtomicU32 = AtomicU32::new(80);
pub static ASMR_WIND_CUTOFF: AtomicU32 = AtomicU32::new(500);

// ─── Procedural Natural Rain Synthesis (Acoustic Droplet Physics) ─────────────

/// Individual physical water droplet voice.
/// Synthesizes acoustic bubble/cavity resonance with upward micro-chirp and shock excitation.
#[derive(Clone, Copy)]
pub struct DropletVoice {
    pub active: bool,
    pub phase: f32,
    pub base_freq: f32,
    pub chirp_amp: f32,
    pub chirp_decay: f32,
    pub amp: f32,
    pub amp_decay: f32,
    pub shock_amp: f32,
    pub shock_decay: f32,
    pub pan: f32,
}

impl Default for DropletVoice {
    fn default() -> Self {
        Self {
            active: false,
            phase: 0.0,
            base_freq: 0.0,
            chirp_amp: 0.0,
            chirp_decay: 0.0,
            amp: 0.0,
            amp_decay: 0.0,
            shock_amp: 0.0,
            shock_decay: 0.0,
            pan: 0.5,
        }
    }
}

/// Real-time procedural rain engine implementing `rodio::Source`.
/// Generates authentic, non-mechanical water droplets modeled from physical
/// bubble/cavity resonance with upward micro-chirp and shock excitation.
///
/// Features an organic temporal onset progression:
/// - t in [0.0s, 2.0s]: pure sparse isolated droplets ("tip... tip... tip...") with zero background noise.
/// - t in [2.0s, 5.5s]: smooth acceleration and gentle mist fade-in.
/// - t > 5.5s: full steady rainfall responding dynamically to `ASMR_RAIN_DENS`.
pub struct ProceduralRain {
    sample_rate: f32,
    frame_count: u64,
    prng: PRNG,
    voices: [DropletVoice; 16],
    pink_l: f32,
    pink_r: f32,
    hpf_l: Biquad,
    hpf_r: Biquad,
    lpf_l: Biquad,
    lpf_r: Biquad,
    staged_right: f32,
    is_right: bool,
}

impl ProceduralRain {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        Self {
            sample_rate: sr,
            frame_count: 0,
            prng: PRNG::new(0x2026_c001),
            voices: [DropletVoice::default(); 16],
            pink_l: 0.0,
            pink_r: 0.0,
            hpf_l: Biquad::highpass(sr, 450.0, 0.707),
            hpf_r: Biquad::highpass(sr, 450.0, 0.707),
            lpf_l: Biquad::lowpass(sr, 3200.0, 0.707),
            lpf_r: Biquad::lowpass(sr, 3200.0, 0.707),
            staged_right: 0.0,
            is_right: false,
        }
    }

    /// Spawns a physical water droplet in the voice pool.
    fn trigger_droplet(&mut self, sr: f32) {
        let mut best_idx = 0;
        let mut min_energy = f32::MAX;
        for (i, v) in self.voices.iter().enumerate() {
            if !v.active {
                best_idx = i;
                break;
            }
            let energy = v.amp + v.shock_amp;
            if energy < min_energy {
                min_energy = energy;
                best_idx = i;
            }
        }

        let roll = self.prng.next_unit_f32();
        let (base_f, tau, shock_init, amp_init) = if roll < 0.60 {
            // 60% Crisp small droplet ("tip") - 1750 to 2400 Hz
            let f = 1750.0 + self.prng.next_unit_f32() * 650.0;
            let t = 0.004 + self.prng.next_unit_f32() * 0.003; // 4 - 7 ms
            let s = 0.42;
            let a = 0.32 + self.prng.next_unit_f32() * 0.20;
            (f, t, s, a)
        } else if roll < 0.90 {
            // 30% Medium hollow droplet ("plink") - 1100 to 1600 Hz
            let f = 1100.0 + self.prng.next_unit_f32() * 500.0;
            let t = 0.007 + self.prng.next_unit_f32() * 0.005; // 7 - 12 ms
            let s = 0.28;
            let a = 0.28 + self.prng.next_unit_f32() * 0.16;
            (f, t, s, a)
        } else {
            // 10% Deep fat droplet ("plop") - 650 to 970 Hz
            let f = 650.0 + self.prng.next_unit_f32() * 320.0;
            let t = 0.012 + self.prng.next_unit_f32() * 0.008; // 12 - 20 ms
            let s = 0.18;
            let a = 0.22 + self.prng.next_unit_f32() * 0.14;
            (f, t, s, a)
        };

        let voice = &mut self.voices[best_idx];
        voice.active = true;
        voice.phase = 0.0;
        voice.base_freq = base_f;
        voice.chirp_amp = 0.15; // +15% upward pitch chirp on impact
        voice.chirp_decay = (-1.0 / (sr * 0.0015)).exp(); // decays in 1.5ms
        voice.amp = amp_init;
        voice.amp_decay = (-1.0 / (sr * tau)).exp();
        voice.shock_amp = shock_init;
        voice.shock_decay = (-1.0 / (sr * 0.00035)).exp(); // shock transient lasts ~0.35ms
        voice.pan = 0.15 + self.prng.next_unit_f32() * 0.70; // 0.15 to 0.85 stereo spread
    }

    /// Computes the next stereo frame (left, right).
    #[inline]
    pub fn render_frame(&mut self) -> (f32, f32) {
        let sr = self.sample_rate;
        let t = self.frame_count as f32 / sr;

        // Current user density setting [0.0, 1.0]
        let dens = (ASMR_RAIN_DENS.load(Ordering::Relaxed) as f32 / 100.0).clamp(0.0, 1.0);
        let steady_lambda = 65.0 + dens * 175.0; // 65 - 240 drops/sec at steady state

        // Three-stage onset timeline progression
        let (lambda, mist_gain) = if t < 2.0 {
            // Stage 1: Sparse isolated drops ("tip... tip... tip...")
            // Pure silence between individual droplet strikes.
            let sparse_rate = 2.4 + (t / 2.0) * 1.6; // 2.4 -> 4.0 drops/s
            (sparse_rate, 0.0f32)
        } else if t < 5.5 {
            // Stage 2: Smootherstep acceleration and gentle mist fade-in
            let u = (t - 2.0) / 3.5;
            let s = u * u * u * (u * (u * 6.0 - 15.0) + 10.0);
            let rate = 4.0 + s * (steady_lambda - 4.0);
            let mist = s * (0.04 + dens * 0.05); // 0.0 -> max 0.09
            (rate, mist)
        } else {
            // Stage 3: Steady-state authentic rain shower
            (steady_lambda, 0.04 + dens * 0.05)
        };

        // Stochastic Poisson arrival check
        let p_arrival = lambda / sr;
        if self.prng.next_unit_f32() < p_arrival {
            self.trigger_droplet(sr);
        }

        // Render active physical droplet voices
        let mut drop_l = 0.0f32;
        let mut drop_r = 0.0f32;

        for v in &mut self.voices {
            if !v.active {
                continue;
            }

            // Impact shock transient (short high-frequency click)
            let shock = if v.shock_amp > 0.001 {
                let s = v.shock_amp * (self.prng.next_f32() * 0.6);
                v.shock_amp *= v.shock_decay;
                s
            } else {
                0.0
            };

            // Cavity/bubble resonance ringdown
            let cur_freq = v.base_freq * (1.0 + v.chirp_amp);
            v.chirp_amp *= v.chirp_decay;

            let ring = v.phase.sin() * v.amp;
            v.phase += 2.0 * std::f32::consts::PI * cur_freq / sr;
            if v.phase > 2.0 * std::f32::consts::PI {
                v.phase -= 2.0 * std::f32::consts::PI;
            }
            v.amp *= v.amp_decay;

            let sample = shock + ring;
            drop_l += sample * (1.0 - v.pan);
            drop_r += sample * v.pan;

            if v.amp < 0.0002 && v.shock_amp < 0.001 {
                v.active = false;
            }
        }

        // Render background mist bed (only if mist_gain > 0)
        let (bed_l, bed_r) = if mist_gain > 0.001 {
            let white_l = self.prng.next_f32();
            let white_r = self.prng.next_f32();
            self.pink_l = 0.88 * self.pink_l + 0.12 * white_l;
            self.pink_r = 0.88 * self.pink_r + 0.12 * white_r;

            let filtered_l = self.lpf_l.process(self.hpf_l.process(self.pink_l));
            let filtered_r = self.lpf_r.process(self.hpf_r.process(self.pink_r));
            (filtered_l * mist_gain * 0.40, filtered_r * mist_gain * 0.40)
        } else {
            (0.0, 0.0)
        };

        // Combine: Droplets dominate (>85%), mist is subtle background
        let out_l = soft_limit(drop_l * 0.85 + bed_l);
        let out_r = soft_limit(drop_r * 0.85 + bed_r);

        self.frame_count += 1;
        (out_l, out_r)
    }
}

impl Iterator for ProceduralRain {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        if self.is_right {
            self.is_right = false;
            Some(self.staged_right)
        } else {
            let (out_l, out_r) = self.render_frame();
            self.staged_right = out_r;
            self.is_right = true;
            Some(out_l)
        }
    }
}

impl rodio::Source for ProceduralRain {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn channels(&self) -> u16 {
        2
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.sample_rate as u32
    }

    #[inline]
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// Synthesizes a steady-state procedural rain buffer for cached zero-copy testing.
pub fn generate_procedural_rain_buffer(sample_rate: u32, duration_secs: f32) -> crate::AsmrBuffer {
    let ch = 2usize;
    let total_frames = (sample_rate as f32 * duration_secs) as usize;
    let mut raw = Vec::with_capacity(total_frames * ch);
    let mut rain = ProceduralRain::new(sample_rate);
    // Pre-warm to steady-state so the pre-rendered buffer loops smoothly
    rain.frame_count = (sample_rate as f32 * 6.0) as u64;

    for _ in 0..total_frames {
        let (out_l, out_r) = rain.render_frame();
        raw.push((out_l * 12000.0).clamp(-32768.0, 32767.0) as i16);
        raw.push((out_r * 12000.0).clamp(-32768.0, 32767.0) as i16);
    }

    // 0.6s equal-power cosine crossfade for seamless infinite looping
    let xfade_frames = (sample_rate as f32 * 0.6) as usize;
    if total_frames > xfade_frames * 2 && xfade_frames > 0 {
        let loop_frames = total_frames - xfade_frames;
        for frame in 0..xfade_frames {
            let tail_frame = loop_frames + frame;
            let t = frame as f32 / xfade_frames as f32;
            let fade_out = (t * std::f32::consts::FRAC_PI_2).cos();
            let fade_in = (t * std::f32::consts::FRAC_PI_2).sin();
            for c in 0..ch {
                let tail_val = raw[tail_frame * ch + c] as f32;
                let head_val = raw[frame * ch + c] as f32;
                let blended = (fade_out * tail_val + fade_in * head_val).clamp(-32768.0, 32767.0) as i16;
                raw[frame * ch + c] = blended;
            }
        }
        raw.truncate(loop_frames * ch);
    }

    crate::AsmrBuffer {
        samples: std::sync::Arc::from(raw.into_boxed_slice()),
        channels: ch as u16,
        sample_rate,
    }
}

