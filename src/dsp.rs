use rodio::Source;
use std::time::Duration;

// ─── Xorshift PRNG ────────────────────────────────────────────────────────────
pub struct PRNG { seed: u32 }
impl PRNG {
    pub fn new(seed: u32) -> Self { Self { seed: seed | 1 } }
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed as i32) as f32 / i32::MAX as f32
    }
}

// ─── Direct Form II Transposed Biquad ─────────────────────────────────────────
#[derive(Clone)]
pub struct Biquad {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    s1: f32, s2: f32,
}

impl Biquad {
    pub fn lowpass(sr: f32, fc: f32, q: f32) -> Self {
        let w = 2.0 * std::f32::consts::PI * (fc / sr).min(0.499);
        let cw = w.cos(); let alpha = w.sin() / (2.0 * q);
        let a0r = 1.0 / (1.0 + alpha);
        let b = (1.0 - cw) / 2.0;
        Self { b0: b*a0r, b1: (1.0-cw)*a0r, b2: b*a0r, a1: -2.0*cw*a0r, a2: (1.0-alpha)*a0r, s1:0.0, s2:0.0 }
    }
    pub fn highpass(sr: f32, fc: f32, q: f32) -> Self {
        let w = 2.0 * std::f32::consts::PI * (fc / sr).min(0.499);
        let cw = w.cos(); let alpha = w.sin() / (2.0 * q);
        let a0r = 1.0 / (1.0 + alpha);
        let b = (1.0 + cw) / 2.0;
        Self { b0: b*a0r, b1: -(1.0+cw)*a0r, b2: b*a0r, a1: -2.0*cw*a0r, a2: (1.0-alpha)*a0r, s1:0.0, s2:0.0 }
    }
    pub fn bandpass(sr: f32, fc: f32, q: f32) -> Self {
        let w = 2.0 * std::f32::consts::PI * (fc / sr).min(0.499);
        let cw = w.cos(); let alpha = w.sin() / (2.0 * q);
        let a0r = 1.0 / (1.0 + alpha);
        Self { b0: alpha*a0r, b1: 0.0, b2: -alpha*a0r, a1: -2.0*cw*a0r, a2: (1.0-alpha)*a0r, s1:0.0, s2:0.0 }
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }

    #[inline]
    pub fn process_nonlinear(&mut self, x: f32, drive: f32) -> f32 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        
        // Padé soft clip embedded directly into the TDF-II states
        let s1d = self.s1 * drive;
        let s2d = self.s2 * drive;
        let s12 = s1d * s1d;
        let s22 = s2d * s2d;
        self.s1 = (s1d * (27.0 + s12) / (27.0 + 9.0 * s12)) / drive;
        self.s2 = (s2d * (27.0 + s22) / (27.0 + 9.0 * s22)) / drive;
        
        y
    }
}

// ─── Config ───────────────────────────────────────────────────────────────────
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ProceduralConfig {
    pub switch_type: String,    // "cream"|"linear"|"red"|"black"|"tactile"|"brown"|"clicky"|"blue"|"box_jade"
    pub spring_weight: f32,     // 35–80g
    pub lube_amount: f32,       // 0.0–1.0
    pub pitch: f32,             // 0.5–2.0 (scales f₀)
    pub keycap_material: String,// "abs"|"pbt"
    pub plate_material: String, // "brass"|"aluminum"|"pom"|"polycarbonate"
    pub mounting_style: String, // "tray"|"gasket"|"top"
    pub foam_mod: f32,          // 0.0–1.0 (PE foam / butter paper)
    pub case_material: String,  // "plastic"|"aluminum"|"polycarbonate"
    pub o_rings: f32,           // 0.0–1.0
}

impl Default for ProceduralConfig {
    fn default() -> Self {
        // NK Cream: POM housing, 45g, gasket, PE foam, heavily lubed
        Self {
            switch_type: "cream".to_string(),
            spring_weight: 45.0,
            lube_amount: 0.85,
            pitch: 0.85,
            keycap_material: "pbt".to_string(),
            plate_material: "pom".to_string(),
            mounting_style: "gasket".to_string(),
            foam_mod: 0.80,
            case_material: "plastic".to_string(),
            o_rings: 0.0,
        }
    }
}

// ─── A single damped modal oscillator ────────────────────────────────────────
// Directly synthesizes an exponentially-damped sinusoid.
// This is mathematically equivalent to a high-Q resonator excited by an impulse,
// but avoids the gain normalization problem (high-Q biquads have near-zero gain
// for transient inputs → silence). Instead we compute it analytically:
//   y(t) = amplitude × sin(2π × freq × t) × exp(−t / tau)
//   where tau = decay_time_seconds / ln(1000) ≈ decay_secs / 6.91
// The sin² raised-cosine shapes the attack so it doesn't start abruptly.
struct Modal {
    amp: f32,
    t_e: f32,
    u: f32,
    v: f32,
    cos_w: f32,
    sin_w: f32,
    decay_coeff: f32,
}

impl Modal {
    fn new(freq: f32, decay_secs: f32, amp: f32, t_e: f32, sr: f32) -> Self {
        let tau = decay_secs / 6.908; // e^(-6.908) ≈ 0.001 = -60dB decay
        let w = 2.0 * std::f32::consts::PI * freq / sr;
        Self {
            amp, t_e,
            u: 1.0,
            v: 0.0,
            cos_w: w.cos(),
            sin_w: w.sin(),
            decay_coeff: (-1.0 / (sr * tau)).exp(),
        }
    }

    #[inline]
    fn sample(&mut self, t: f32) -> f32 {
        let attack = if t < self.t_e {
            (std::f32::consts::PI * t / self.t_e).sin().powi(2)
        } else {
            1.0
        };
        
        let next_u = self.u * self.cos_w - self.v * self.sin_w;
        let next_v = self.u * self.sin_w + self.v * self.cos_w;
        self.u = next_u * self.decay_coeff;
        self.v = next_v * self.decay_coeff;
        
        self.amp * self.v * attack
    }
}

// ─── Engine ───────────────────────────────────────────────────────────────────
pub struct ProceduralSwitch {
    sr: u32,
    cursor: u64,
    total: u64,
    // Pre-computed time step and current time — avoids integer division per sample
    dt: f32,
    t: f32,
    prng: PRNG,

    // 4 modal oscillators (computed via fast recursive complex multiply)
    modes: [Modal; 4],

    // Friction noise shaping
    friction_bpf: Biquad,
    friction_vol: f32,
    noise_env: f32,
    noise_decay_coeff: f32,

    // Output shaping filters
    output_lpf: Biquad,
    mount_hpf: Biquad,

    // State
    is_keyup: bool,
    is_clicky: bool,
    is_tactile: bool,
    foam_sat: f32,
    
    // Tactile/Clicky state
    tactile_env: f32,
    tactile_decay_coeff: f32,
    click_u: f32,
    click_v: f32,
    click_cos_w: f32,
    click_sin_w: f32,
}

impl ProceduralSwitch {
    pub fn new(config: ProceduralConfig, is_keyup: bool, velocity: f32) -> Self {
        let sr = 44100u32;
        let srf = sr as f32;
        let pi = std::f32::consts::PI;

        // ── f₀: body resonant frequency of plate/switch material ─────────────
        // A mechanical switch is a tiny plastic cavity (14mm).
        // Resonance is high-frequency clack (1200 Hz - 3000 Hz).
        let base_f0: f32 = match config.plate_material.as_str() {
            "fr4"                  => 3500.0, // High-pitch clack
            "brass"                => 2800.0,
            "aluminum"             => 2400.0,
            "nylon"                => 1800.0, // Muted transient
            "pom"                  => 1650.0, // NK Cream is a bit higher-pitched and clackier
            "abs"                  => 1200.0, // Low-pitch transient
            "polycarbonate" | "pc" => 900.0,  // Deep resonant thock
            _                      => 1450.0,
        };
        // Top-out (keyup) hits the thinner top housing, producing a higher pitch
        let f0 = base_f0 * config.pitch * if is_keyup { 1.35 } else { 1.0 };

        // ── T_e: raised-cosine attack width (stem hardness) ───────────────────
        // Hard plastic collisions are instantaneous (<0.5ms).
        // A slow attack (1.8ms) rounds off the wavefront and muffles it.
        let base_te: f32 = match config.plate_material.as_str() {
            "brass"                => 0.0001,
            "aluminum"             => 0.0002,
            "polycarbonate" | "pc" => 0.0003,
            "pom"                  => 0.0004, // Snappy POM clack
            _                      => 0.0003,
        };
        let t_e = base_te * (1.0 + config.o_rings * 0.9) / velocity.clamp(0.5, 2.0);

        // ── Amplitude: spring weight × kinetic energy (v^2) ───────────────────
        let spring_scale = config.spring_weight / 60.0;
        let vel = velocity.clamp(0.5, 2.0);
        let kinetic_energy = vel * vel;
        let base_amp = spring_scale * kinetic_energy * if is_keyup { 0.70 } else { 1.0 };

        // ── Decay: plate → case → gasket/foam modifications ───────────────────
        // High frequencies decay extremely fast in plastic, but we need them to 
        // form the acoustic body of the clack. 
        // -60dB tau values for each mode.
        let d1_base: f32 = match config.plate_material.as_str() {
            "fr4"                  => 0.055, // Low damping, extended ring
            "brass"                => 0.040,
            "aluminum"             => 0.025,
            "polycarbonate" | "pc" => 0.018, // Smooth medium-length decay
            "nylon"                => 0.012, // Moderate-fast decay
            "pom"                  => 0.008, // Very short, snappy transient for POM clack
            "abs"                  => 0.007, // High internal friction, rapid acoustic decay
            _                      => 0.012,
        };
        let case_mult: f32 = match config.case_material.as_str() {
            "aluminum"             => 0.65,
            "polycarbonate" | "pc" => 1.30,
            _                      => 1.00,
        };
        let gasket_k: f32 = if config.mounting_style == "gasket" { 0.70 } else { 1.0 };

        // Foam dampens the longest-ringing modes
        let foam = config.foam_mod;
        let foam_k1 = 1.0 - foam * 0.15;
        let foam_k2 = 1.0 - foam * 0.40;
        let foam_k3 = 1.0 - foam * 0.60;
        let foam_k4 = 1.0 - foam * 0.85;

        // PBT keycaps: denser than ABS → dampens high frequencies slightly faster
        let pbt_k: f32 = if config.keycap_material == "pbt" { 0.70 } else { 1.0 };

        // Authentic resonant duration tuning
        let d1 = d1_base * case_mult * gasket_k * foam_k1;
        let d2 = d1_base * case_mult * gasket_k * foam_k2 * 0.8;
        let d3 = d1_base * case_mult * gasket_k * foam_k3 * 0.6;
        let d4 = d1_base * case_mult * gasket_k * foam_k4 * pbt_k * 0.4;

        // ── Modal oscillator gains ───────────────────────────────────────────
        // In real switches, the fundamental (1.5kHz) and second mode (2.7kHz) hold 
        // the vast majority of the clack energy.
        // Distribute more energy to high frequencies for that authentic "clack"
        let a1 = base_amp * 0.65; 
        let a2 = base_amp * 0.60;
        let a3 = base_amp * 0.35;
        let a4 = base_amp * 0.15 * (if config.keycap_material == "pbt" { pbt_k } else { 1.0 });

        // Authentic modal structure of a rectangular POM keycap/switch cavity
        let modes = [
            Modal::new(f0,         d1, a1, t_e, srf),          // e.g. 1450 Hz
            Modal::new(f0 * 1.88,  d2, a2, t_e * 0.8, srf),    // e.g. 2726 Hz
            Modal::new(f0 * 2.34,  d3, a3, t_e * 0.5, srf),    // e.g. 3393 Hz
            Modal::new(f0 * 3.12,  d4, a4, t_e * 0.3, srf),    // e.g. 4524 Hz
        ];

        // ── Friction noise: LPF mapped to Roughness & Velocity ─────────────────
        // Roughness maps to noise variance. Fricton scales strictly with velocity.
        let roughness = 1.0 - (config.lube_amount * 0.8);
        let friction_vol = roughness * vel * 0.8;
        let friction_fc = 8000.0 - (config.lube_amount * 6000.0);
        let friction_bpf = Biquad::lowpass(srf, friction_fc.min(8000.0), 0.707);
        
        // Very fast decay for the initial slap noise (2-3ms)
        let noise_decay_coeff = (-1500.0 / srf).exp();

        // ── Output LPF (lube + foam smoothing) ────────────────────────────────
        // Real mechanical sound goes up to 8-12kHz.
        let lp_fc = (12000.0 - config.lube_amount * 3000.0) * (1.0 - config.foam_mod * 0.15);
        let output_lpf = Biquad::lowpass(srf, lp_fc.max(3000.0), 0.707);

        // ── Gasket HPF (sub-bass isolation) ───────────────────────────────────
        let hpf_fc: f32 = match config.mounting_style.as_str() {
            "gasket" => 250.0, // Cuts muddiness
            "top"    => 100.0,
            _        => 50.0,
        };
        let mount_hpf = Biquad::highpass(srf, hpf_fc, 0.707);

        // ── Foam soft-clip drive (creamy warmth without buzz) ─────────────────
        let foam_sat = foam * 0.30; 

        // ── Duration ──────────────────────────────────────────────────────────
        let tail_ms: f32 = match config.switch_type.as_str() {
            "clicky" | "blue" | "box_jade"        => 120.0,
            "tactile" | "brown" | "holy_panda"    => 95.0,
            "cream"                               => 60.0, // Very tight POM impact
            _                                     => 80.0,
        };
        let total = (srf * tail_ms / 1000.0) as u64;

        let is_clicky  = matches!(config.switch_type.as_str(), "clicky"|"blue"|"white"|"box_jade");
        let is_tactile = matches!(config.switch_type.as_str(), "tactile"|"brown"|"holy_panda");

        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
            .wrapping_add((f0 * 173.0) as u32);
            
        let tactile_decay_coeff = (-250.0 / srf).exp();
        
        // Clicky fast oscillator setup (3500 Hz)
        let click_w = 2.0 * pi * 3500.0 / srf;

        let _ = pi; // suppress unused import warning
        Self {
            sr, cursor: 0, total, dt: 1.0 / srf, t: 0.0, prng: PRNG::new(seed),
            modes,
            friction_bpf, friction_vol,
            noise_env: 1.0,
            noise_decay_coeff,
            output_lpf, mount_hpf,
            is_keyup, is_clicky, is_tactile, foam_sat,
            tactile_env: 1.0,
            tactile_decay_coeff,
            click_u: 1.0,
            click_v: 0.0,
            click_cos_w: click_w.cos(),
            click_sin_w: click_w.sin(),
        }
    }
}

impl Iterator for ProceduralSwitch {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        if self.cursor >= self.total { return None; }
        let t = self.t;
        self.cursor += 1;
        self.t += self.dt;

        // ── Modal body (4 directly-computed damped sinusoids) ─────────────────
        let body = self.modes[0].sample(t)
                 + self.modes[1].sample(t)
                 + self.modes[2].sample(t)
                 + self.modes[3].sample(t);

        // ── Friction noise (first ~10ms, killed by lube) ──────────────────────
        let friction = self.friction_bpf.process(self.prng.next_f32()) * self.noise_env * self.friction_vol;
        self.noise_env *= self.noise_decay_coeff;

        // ── Switch-type specific transients ───────────────────────────────────
        let mut extra = 0.0f32;
        if !self.is_keyup {
            if self.is_clicky {
                let ct = t - 0.003;
                if ct > -0.0005 && t < 0.018 {
                    // Update click oscillator
                    let next_u = self.click_u * self.click_cos_w - self.click_v * self.click_sin_w;
                    let next_v = self.click_u * self.click_sin_w + self.click_v * self.click_cos_w;
                    self.click_u = next_u;
                    self.click_v = next_v;
                    
                    // Simple linear decay approximation instead of exp() for 15ms burst
                    let env = (1.0 - (ct.abs() * 60.0)).max(0.0);
                    extra = self.click_v * env * 0.55;
                }
            } else if self.is_tactile {
                if t < 0.012 {
                    extra = self.prng.next_f32() * self.tactile_env * 0.12;
                    self.tactile_env *= self.tactile_decay_coeff;
                }
            }
        }

        let mut out = body + friction + extra;

        // ── Output LPF (lube + foam smoothing with embedded Padé Saturation) ───
        if self.foam_sat > 0.05 {
            let drive = 1.0 + self.foam_sat * 2.0;
            out = self.output_lpf.process_nonlinear(out, drive);
        } else {
            out = self.output_lpf.process(out);
        }

        // ── Gasket HPF (sub-bass isolation) ───────────────────────────────────
        out = self.mount_hpf.process(out);

        Some(out * 0.88)
    }
}

impl Source for ProceduralSwitch {
    fn current_frame_len(&self) -> Option<usize> { Some((self.total - self.cursor) as usize) }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.sr }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.total as f32 / self.sr as f32))
    }
}
