use rodio::Source;
use std::time::Duration;

// ─── Xorshift PRNG ────────────────────────────────────────────────────────────
// As specified in research: 3 bitwise ops, ~3 clock cycles per sample
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

// ─── Direct Form II Transposed (TDF-II) Biquad ───────────────────────────────
// Research confirms: optimal for floating-point audio. 2 state vars only.
// Equations: y[n] = b0*x[n] + s1
//            s1   = b1*x[n] - a1*y[n] + s2
//            s2   = b2*x[n] - a2*y[n]
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
}

// ─── Padé tanh(x) approximation ──────────────────────────────────────────────
// From research: f(x) = x*(27+x²)/(27+9x²)
// Applied to OUTPUT signal ONLY (not inside filter states, which causes aliasing)
#[inline]
fn pade_tanh(x: f32) -> f32 {
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

// ─── Config ───────────────────────────────────────────────────────────────────
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ProceduralConfig {
    pub switch_type: String,    // "cream"|"linear"|"red"|"black"|"tactile"|"brown"|"clicky"|"blue"|"box_jade"
    pub spring_weight: f32,     // 35–80g
    pub lube_amount: f32,       // 0.0–1.0
    pub pitch: f32,             // 0.5–2.0 (scales f₀)
    pub keycap_material: String,// "abs"|"pbt"
    pub plate_material: String, // "fr4"|"brass"|"aluminum"|"nylon"|"pom"|"abs"|"polycarbonate"
    pub mounting_style: String, // "tray"|"gasket"|"top"
    pub foam_mod: f32,          // 0.0–1.0 (PE foam / butter paper)
    pub case_material: String,  // "plastic"|"aluminum"|"polycarbonate"
    pub o_rings: f32,           // 0.0–1.0
}

impl Default for ProceduralConfig {
    fn default() -> Self {
        // NK Cream: POM housing & stem, 45g spring, gasket mount, PE foam, heavily lubed
        Self {
            switch_type: "cream".to_string(),
            spring_weight: 45.0,
            lube_amount: 0.85,
            pitch: 1.0,
            keycap_material: "pbt".to_string(),
            plate_material: "pom".to_string(),
            mounting_style: "gasket".to_string(),
            foam_mod: 0.60,
            case_material: "plastic".to_string(),
            o_rings: 0.0,
        }
    }
}

// ─── Modal Oscillator (Coupled-Form Recursive Sine) ──────────────────────────
// Research confirms: coupled-form is optimal. Drift is 0.0012 dB over 120ms.
// No renormalization needed. DO NOT change this structure.
// Formula: y(t) = A * exp(-t/tau) * sin(2π*f*t)
struct Modal {
    amp: f32,
    t_e: f32,   // raised-cosine attack envelope end time
    u: f32,     // cos component
    v: f32,     // sin component (this is the output)
    cos_w: f32,
    sin_w: f32,
    decay_coeff: f32,
}

impl Modal {
    fn new(freq: f32, decay_secs: f32, amp: f32, t_e: f32, sr: f32) -> Self {
        // tau = decay / ln(1000) ≈ decay / 6.908 for -60dB
        let tau = decay_secs / 6.908;
        let w = 2.0 * std::f32::consts::PI * freq / sr;
        Self {
            amp, t_e,
            u: 1.0, v: 0.0,
            cos_w: w.cos(),
            sin_w: w.sin(),
            decay_coeff: (-1.0 / (sr * tau)).exp(),
        }
    }

    #[inline]
    fn sample(&mut self, t: f32) -> f32 {
        // sin² raised-cosine attack — instantaneous start, smooth onset
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
    dt: f32,    // pre-computed 1/sr — avoids division per sample
    t: f32,
    prng: PRNG,

    modes: [Modal; 4],

    // Friction: BPF per research (Van den Doel et al.)
    friction_bpf: Biquad,
    friction_vol: f32,
    noise_env: f32,
    noise_decay_coeff: f32,

    // Output shaping
    output_lpf: Biquad,
    dc_hpf: Biquad,

    is_keyup: bool,
    is_clicky: bool,
    is_tactile: bool,
    foam_sat: f32,

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
        let vel = velocity.clamp(0.5, 1.8);

        // ── f₀: Material resonant frequency ──────────────────────────────────
        // Source: research table — f0 ∝ sqrt(E/ρ)
        // POM:  E=2.8GPa, ρ=1.42 → Z=3.45 MRayls. Dense, high-impedance.
        // PC:   E=2.3GPa, ρ=1.22 → Z=2.77 MRayls. Deep thock.
        // FR4:  E=8GPa,   ρ=1.8  → Z=4.5  MRayls. High-pitch clack.
        // ABS:  E=2.0GPa, ρ=1.03 → Z=2.31 MRayls. Rapid decay, dull clack.
        // Nylon:E=2.2GPa, ρ=1.12 → Z=2.70 MRayls. Muted, soft.
        let base_f0: f32 = match config.plate_material.as_str() {
            "fr4"                  => 3200.0, // highest stiffness-to-density ratio
            "brass"                => 2600.0,
            "aluminum"             => 2200.0,
            "nylon"                => 1600.0, // flexible, muted
            "pom"                  => 1400.0, // POM: E=2.8, dense (1.42 g/cm³) — thick clack
            "abs"                  => 1100.0, // ABS: low density, rapid decay
            "polycarbonate" | "pc" => 800.0,  // PC: deep resonant "thock" 500-1200Hz range
            _                      => 1400.0,
        };

        // Top-out hits the thinner top housing → higher pitch
        let f0 = base_f0 * config.pitch * if is_keyup { 1.30 } else { 1.0 };

        // ── Attack: time for raised-cosine onset ──────────────────────────────
        // POM is stiff — very fast attack (<0.4ms). O-rings slow it down.
        let base_te: f32 = match config.plate_material.as_str() {
            "brass"                => 0.00008,
            "aluminum"             => 0.00015,
            "fr4"                  => 0.00010,
            "polycarbonate" | "pc" => 0.00025,
            "pom"                  => 0.00020, // snappy POM impact
            _                      => 0.00020,
        };
        let t_e = base_te * (1.0 + config.o_rings * 2.0);

        // ── Amplitude: G = k1 * V^γ (γ≈1.5 is perceptually balanced) ─────────
        // Research says γ=2.0 (kinetic energy). But γ=2.0 makes quiet keys
        // nearly silent and loud keys feel "explosive". γ=1.5 balances well.
        let spring_scale = (config.spring_weight / 55.0).clamp(0.5, 1.5);
        let base_amp = spring_scale * vel.powf(1.5) * if is_keyup { 0.65 } else { 1.0 };

        // ── Decay: tau values per material (research table) ───────────────────
        // FR4: low internal damping → long ring. ABS: high damping → fast decay.
        // POM: moderate-high density, self-lubricating → short-medium decay.
        let d1_base: f32 = match config.plate_material.as_str() {
            "fr4"                  => 0.060, // long ring (glass composite, low damping)
            "brass"                => 0.045,
            "aluminum"             => 0.028,
            "polycarbonate" | "pc" => 0.022, // medium-long smooth decay (thock)
            "nylon"                => 0.014, // flexible → moderate damping
            "pom"                  => 0.012, // POM: short, punchy
            "abs"                  => 0.007, // ABS: rapid acoustic decay
            _                      => 0.014,
        };

        // Case material modifies the resonant chamber's acoustic damping
        let case_mult: f32 = match config.case_material.as_str() {
            "aluminum"             => 0.70, // rigid metal case cuts resonance short
            "polycarbonate" | "pc" => 1.25, // PC case extends the ring slightly
            _                      => 1.00,
        };

        // Gasket absorbs vibration from the plate → softer, shorter decay
        let gasket_k: f32 = if config.mounting_style == "gasket" { 0.75 } else { 1.0 };

        // Foam damps the high-frequency modes most aggressively (research-confirmed)
        let foam = config.foam_mod;
        let foam_k1 = 1.0 - foam * 0.10; // barely touches fundamental
        let foam_k2 = 1.0 - foam * 0.35;
        let foam_k3 = 1.0 - foam * 0.60;
        let foam_k4 = 1.0 - foam * 0.85; // crushes highest mode

        let pbt_k: f32 = if config.keycap_material == "pbt" { 0.75 } else { 1.0 };

        let d1 = d1_base * case_mult * gasket_k * foam_k1;
        let d2 = d1_base * case_mult * gasket_k * foam_k2 * 0.75;
        let d3 = d1_base * case_mult * gasket_k * foam_k3 * 0.50;
        let d4 = d1_base * case_mult * gasket_k * foam_k4 * pbt_k * 0.30;

        // ── Modal gains ───────────────────────────────────────────────────────
        // Research: "85% of energy in fundamental+second mode" for a thick switch.
        // Modes use NEAR-HARMONIC ratios. A POM switch cavity is close to a
        // rectangular box resonator. Harmonics: 1x, ~2x, ~3x, ~4.5x
        // (slight inharmonicity due to material stiffness)
        let a1 = base_amp * 0.80;  // fundamental — bulk of the sound
        let a2 = base_amp * 0.28;  // 2nd harmonic — adds body
        let a3 = base_amp * 0.07;  // 3rd harmonic — subtle texture
        let a4 = base_amp * 0.02 * (if config.keycap_material == "pbt" { pbt_k } else { 1.0 });

        // Modal frequency ratios: nearly harmonic (avoids metallic inharmonic harshness)
        // Ratio 2.0 = clean octave. 2.05 adds slight inharmonicity (physical realism).
        let modes = [
            Modal::new(f0,         d1, a1, t_e,         srf),
            Modal::new(f0 * 2.05,  d2, a2, t_e * 0.60,  srf),
            Modal::new(f0 * 3.10,  d3, a3, t_e * 0.35,  srf),
            Modal::new(f0 * 4.55,  d4, a4, t_e * 0.20,  srf),
        ];

        // ── Friction noise: BPF (research: Van den Doel et al.) ──────────────
        // BPF centered at stem-material scrape frequency.
        // POM-on-Nylon scraping: fundamental ~400-800 Hz.
        // Lube shifts this down and reduces variance.
        // Noise volume is tiny — it adds texture, NOT the main sound.
        let roughness = (1.0 - config.lube_amount).max(0.0);
        // BPF center: unlubed ~600Hz, fully lubed ~200Hz
        let friction_fc = 200.0 + roughness * 400.0;
        let friction_q = 0.8; // moderate width
        let friction_bpf = Biquad::bandpass(srf, friction_fc, friction_q);
        // Friction vol: very small, just texture
        let friction_vol = roughness * roughness * 0.08; // quadratic: lubed=nearly zero
        // Noise envelope decays in ~5ms (stem travel time)
        let noise_decay_coeff = (-200.0 / srf).exp();

        // ── Output LPF ────────────────────────────────────────────────────────
        // Mechanical switches: energy up to ~6-8kHz. Above that is mostly air/noise.
        // Lube smooths the high end. Foam drops it further.
        // We use a lower cutoff than before to prevent the "TV static" harshness.
        let lp_fc = 5500.0
            - config.lube_amount * 2000.0  // lube smooths HF
            - config.foam_mod * 1500.0;    // foam dampens HF further
        let output_lpf = Biquad::lowpass(srf, lp_fc.max(1500.0), 0.65);

        // ── DC removal HPF ────────────────────────────────────────────────────
        // Remove DC offset and sub-bass rumble. Simple 30Hz HPF.
        let dc_hpf = Biquad::highpass(srf, 30.0, 0.707);

        // ── Foam saturation drive ─────────────────────────────────────────────
        // Low drive so it adds warmth, not distortion
        let foam_sat = foam * 0.20;

        // ── Duration ──────────────────────────────────────────────────────────
        let tail_ms: f32 = match config.switch_type.as_str() {
            "clicky" | "blue" | "box_jade"     => 110.0,
            "tactile" | "brown" | "holy_panda" => 85.0,
            "cream"                             => 55.0, // POM: tight, punchy
            _                                  => 70.0,
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
        let click_w = 2.0 * pi * 3500.0 / srf;

        Self {
            sr, cursor: 0, total, dt: 1.0 / srf, t: 0.0,
            prng: PRNG::new(seed),
            modes,
            friction_bpf, friction_vol,
            noise_env: 1.0,
            noise_decay_coeff,
            output_lpf, dc_hpf,
            is_keyup, is_clicky, is_tactile, foam_sat,
            tactile_env: 1.0,
            tactile_decay_coeff,
            click_u: 1.0, click_v: 0.0,
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

        // ── Modal body (4 damped sinusoids) ───────────────────────────────────
        let body = self.modes[0].sample(t)
                 + self.modes[1].sample(t)
                 + self.modes[2].sample(t)
                 + self.modes[3].sample(t);

        // ── Friction texture (BPF noise, decays in ~5ms) ─────────────────────
        // Research: friction = LPF(PRNG) * roughness * velocity
        // We use BPF (Van den Doel) centered at stem scrape frequency
        let friction = self.friction_bpf.process(self.prng.next_f32())
            * self.noise_env
            * self.friction_vol;
        self.noise_env *= self.noise_decay_coeff;

        // ── Switch-type transients ─────────────────────────────────────────────
        let mut extra = 0.0f32;
        if !self.is_keyup {
            if self.is_clicky {
                let ct = t - 0.003;
                if ct > -0.0005 && t < 0.018 {
                    let next_u = self.click_u * self.click_cos_w - self.click_v * self.click_sin_w;
                    let next_v = self.click_u * self.click_sin_w + self.click_v * self.click_cos_w;
                    self.click_u = next_u;
                    self.click_v = next_v;
                    let env = (1.0 - (ct.abs() * 60.0)).max(0.0);
                    extra = self.click_v * env * 0.50;
                }
            } else if self.is_tactile {
                if t < 0.012 {
                    extra = self.prng.next_f32() * self.tactile_env * 0.10;
                    self.tactile_env *= self.tactile_decay_coeff;
                }
            }
        }

        let mut out = body + friction + extra;

        // ── Output LPF (linear — no nonlinear states, prevents aliasing) ──────
        out = self.output_lpf.process(out);

        // ── DC removal ────────────────────────────────────────────────────────
        out = self.dc_hpf.process(out);

        // ── Foam soft saturation: Padé tanh on OUTPUT only (not filter states) ─
        // Research: saturation must go on output signal, NOT TDF-II internal states.
        // Embedding in filter states causes inharmonic aliasing artifacts.
        if self.foam_sat > 0.02 {
            let drive = 1.0 + self.foam_sat * 1.5;
            out = pade_tanh(out * drive) / drive;
        }

        Some(out * 0.82)
    }
}

impl Source for ProceduralSwitch {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.sr }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_millis(
            (self.total * 1000 / self.sr as u64) as u64
        ))
    }
}
