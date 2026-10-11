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
use std::sync::atomic::AtomicU32;

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

// ─── Procedural Natural Rain Synthesis (Zero-Drone Acoustics) ─────────────────
/// Synthesizes a pristine, non-mechanical procedural rain buffer with 0.0% engine noise.
/// Built with 100% Rust stdlib operations: pink-bed diffusion + stochastic Poisson droplet impacts.
pub fn generate_procedural_rain_buffer(sample_rate: u32, duration_secs: f32) -> crate::AsmrBuffer {
    let ch = 2usize;
    let total_frames = (sample_rate as f32 * duration_secs) as usize;
    let mut raw = vec![0i16; total_frames * ch];
    let sr = sample_rate as f32;

    let mut prng_l = PRNG::new(0x1337_c001);
    let mut prng_r = PRNG::new(0x7001_d00d);

    let mut pink_l = 0.0f32;
    let mut pink_r = 0.0f32;

    // Highpass at 420 Hz: completely cuts any sub-bass and lower-mid drone
    let mut hpf_l = Biquad::highpass(sr, 420.0, 0.707);
    let mut hpf_r = Biquad::highpass(sr, 420.0, 0.707);

    // Lowpass at 7.0 kHz: prevents harsh high-frequency static
    let mut lpf_l = Biquad::lowpass(sr, 7000.0, 0.707);
    let mut lpf_r = Biquad::lowpass(sr, 7000.0, 0.707);

    // Foliage & window pane acoustic resonance (+2.5dB at ~2kHz)
    let mut peak_l = Biquad::peaking(sr, 1850.0, 1.2, 2.5);
    let mut peak_r = Biquad::peaking(sr, 2100.0, 1.2, 2.5);

    // Active droplet resonance oscillators (pool of 8 concurrent drops)
    #[derive(Clone, Copy)]
    struct Drop {
        phase: f32,
        phase_inc: f32,
        decay: f32,
        amp: f32,
        pan: f32,
    }
    let mut drops = [Drop { phase: 0.0, phase_inc: 0.0, decay: 0.0, amp: 0.0, pan: 0.5 }; 8];
    let lambda_drop = 160.0 / sr; // ~160 natural droplet impacts per second

    for f in 0..total_frames {
        // --- Layer 1: Ambient Rain Bed ---
        let white_l = prng_l.next_f32();
        let white_r = prng_r.next_f32();

        pink_l = 0.88 * pink_l + 0.12 * white_l;
        pink_r = 0.88 * pink_r + 0.12 * white_r;

        let mut bed_l = lpf_l.process(hpf_l.process(pink_l));
        let mut bed_r = lpf_r.process(hpf_r.process(pink_r));

        bed_l = peak_l.process(bed_l);
        bed_r = peak_r.process(bed_r);

        // --- Layer 2: Poisson Droplets ---
        if prng_l.next_unit_f32() < lambda_drop {
            if let Some(d) = drops.iter_mut().min_by(|a, b| a.amp.partial_cmp(&b.amp).unwrap_or(std::cmp::Ordering::Equal)) {
                let drop_freq = 1600.0 + prng_l.next_unit_f32() * 2200.0; // 1.6kHz - 3.8kHz
                let tau_ms = 0.002 + prng_l.next_unit_f32() * 0.003; // 2 - 5ms
                d.phase = 0.0;
                d.phase_inc = 2.0 * std::f32::consts::PI * drop_freq / sr;
                d.decay = (-1.0 / (sr * tau_ms)).exp();
                let u = prng_l.next_unit_f32();
                d.amp = (u * u) * 0.22;
                d.pan = prng_r.next_unit_f32();
            }
        }

        let mut drop_sum_l = 0.0f32;
        let mut drop_sum_r = 0.0f32;

        for d in &mut drops {
            if d.amp > 0.0001 {
                let s = d.phase.sin() * d.amp;
                drop_sum_l += s * (1.0 - d.pan);
                drop_sum_r += s * d.pan;
                d.phase += d.phase_inc;
                d.amp *= d.decay;
            }
        }

        // Mix layers: 65% diffuse bed + 35% droplet impacts, calibrated for natural ambient balance (-25 dBFS)
        let out_l = (bed_l * 0.65 + drop_sum_l * 0.35) * 0.95;
        let out_r = (bed_r * 0.65 + drop_sum_r * 0.35) * 0.95;

        let limited_l = soft_limit(out_l);
        let limited_r = soft_limit(out_r);

        raw[f * ch] = (limited_l * 12000.0).clamp(-32768.0, 32767.0) as i16;
        raw[f * ch + 1] = (limited_r * 12000.0).clamp(-32768.0, 32767.0) as i16;
    }

    // 0.6s equal-power cosine crossfade for seamless infinite looping
    let xfade_frames = (sr * 0.6) as usize;
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

