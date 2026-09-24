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

// ─── TDF-II Biquad ────────────────────────────────────────────────────────────
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
    
    pub fn set_lowpass(&mut self, sr: f32, fc: f32, q: f32) {
        let w = 2.0 * std::f32::consts::PI * (fc / sr).min(0.499);
        let cw = w.cos(); let alpha = w.sin() / (2.0 * q);
        let a0r = 1.0 / (1.0 + alpha);
        let b = (1.0 - cw) / 2.0;
        self.b0 = b*a0r;
        self.b1 = (1.0-cw)*a0r;
        self.b2 = b*a0r;
        self.a1 = -2.0*cw*a0r;
        self.a2 = (1.0-alpha)*a0r;
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

// ─── Fast Padé tanh (output only, never inside filter states) ─────────────────
#[inline]
fn soft_clip(x: f32) -> f32 {
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
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

// ─── Modal oscillator ─────────────────────────────────────────────────────────
// Generates ONE damped sinusoid. Used for the RESONANT TAIL only,
// NOT the main sound. The main sound is the transient noise burst.
struct Modal {
    amp: f32,
    t_start: f32,  // delay before this mode begins (in seconds)
    u: f32, v: f32,
    cos_w: f32, sin_w: f32,
    decay_coeff: f32,
}
impl Modal {
    fn new(freq: f32, decay_secs: f32, amp: f32, t_start: f32, sr: f32) -> Self {
        let tau = decay_secs / 6.908;
        let w = 2.0 * std::f32::consts::PI * freq / sr;
        Self { amp, t_start, u: 1.0, v: 0.0, cos_w: w.cos(), sin_w: w.sin(),
               decay_coeff: (-1.0 / (sr * tau)).exp() }
    }
    #[inline]
    fn sample(&mut self, t: f32) -> f32 {
        if t < self.t_start { return 0.0; }
        let next_u = self.u * self.cos_w - self.v * self.sin_w;
        let next_v = self.u * self.sin_w + self.v * self.cos_w;
        self.u = next_u * self.decay_coeff;
        self.v = next_v * self.decay_coeff;
        self.amp * self.v
    }
}

// ─── Engine ───────────────────────────────────────────────────────────────────
pub struct ProceduralSwitch {
    sr: u32,
    cursor: u64,
    total: u64,
    dt: f32,
    t: f32,
    prng: PRNG,

    // THE MAIN SOUND: filtered broadband transient (the plastic impact)
    impact_lpf: Biquad,     // shapes the "thack" noise into the right frequency
    impact_hpf: Biquad,     // removes sub-bass from impact
    impact_env: f32,        // envelope for the initial impact burst
    impact_decay: f32,      // how fast impact decays (very fast, <5ms)
    impact_vol: f32,        // amplitude of the impact

    // SECONDARY: modal resonance tail (subtle tonal body after the impact)
    modes: [Modal; 3],
    modal_vol: f32,         // kept LOW so it doesn't sound like a bell

    // TERTIARY: stem travel friction (very quiet, just texture)
    friction_bpf: Biquad,
    friction_env: f32,
    friction_decay: f32,
    friction_vol: f32,

    // Switch-type click mechanism
    click_env: f32,
    click_decay: f32,
    click_bpf: Biquad,
    click_vol: f32,

    // Output shaping
    output_lpf: Biquad,
    dc_hpf: Biquad,
    foam_sat: f32,

}

impl ProceduralSwitch {
    pub fn new(config: ProceduralConfig, is_keyup: bool, velocity: f32) -> Self {
        let sr = 44100u32;
        let srf = sr as f32;
        let vel = velocity.clamp(0.5, 1.8);

        // ── Material-dependent impact frequency ──────────────────────────────
        // This shapes the NOISE BURST, not a sine wave.
        // Stiffer/denser material = higher-frequency transient.
        // POM: dense (1.42 g/cm³), moderate stiffness → mid-range "thock"
        let impact_fc: f32 = match config.plate_material.as_str() {
            "fr4"                  => 3500.0,
            "brass"                => 2800.0,
            "aluminum"             => 2400.0,
            "nylon"                => 1400.0,
            "pom"                  => 1200.0, // POM: thocky, not clacky
            "abs"                  => 1000.0,
            "polycarbonate" | "pc" =>  700.0, // PC: deep thock
            _                      => 1200.0,
        };

        // Lube lowers the effective cutoff (smooths the hit)
        let impact_fc = impact_fc * config.pitch * (1.0 - config.lube_amount * 0.30);
        // Top-out is higher pitched (thinner housing)
        let impact_fc = impact_fc * if is_keyup { 1.25 } else { 1.0 };

        // O-rings and foam also reduce the high-frequency content of the impact
        let impact_fc = impact_fc * (1.0 - config.o_rings * 0.20) * (1.0 - config.foam_mod * 0.15);

        // ── Impact noise filter (BPF not LPF — shapes the "thwack") ──────────
        // We want a bandpass centered at impact_fc with moderate Q.
        // This turns white noise into a "click" or "thock" sound.
        let impact_q = 0.5 + config.lube_amount * 0.5; // lube = wider, smoother
        let impact_lpf = Biquad::lowpass(srf, impact_fc.min(8000.0), impact_q);
        let impact_hpf = Biquad::highpass(srf, 80.0, 0.707); // remove sub-bass

        // ── Impact amplitude ─────────────────────────────────────────────────
        // Spring weight affects how hard the stem lands.
        let spring_scale = (config.spring_weight / 55.0).clamp(0.6, 1.4);
        // Keyup is quieter (less force than a deliberate press)
        let keyup_scale = if is_keyup { 0.55 } else { 1.0 };
        let impact_vol = spring_scale * vel.powf(1.3) * keyup_scale;

        // ── Impact envelope: very fast decay (2-8ms) ─────────────────────────
        // This is KEY. A keyboard switch transient is VERY short. 2-5ms.
        // If it decays slowly, it sounds like a synth pad or instrument.
        let base_impact_decay_ms: f32 = match config.switch_type.as_str() {
            "cream"                             => 4.5,  // POM: punchy and short
            "linear" | "red" | "black"         => 3.5,  // linears: clean, fast
            "tactile" | "brown" | "holy_panda" => 5.0,  // tactiles: slightly longer body
            "clicky" | "blue" | "box_jade"     => 3.0,  // clickies: very sharp
            _                                  => 4.0,
        };
        // Foam mod makes the impact last slightly longer (dampens = slower release)
        let impact_decay_ms = base_impact_decay_ms * (1.0 + config.foam_mod * 0.5);
        // Convert ms decay to per-sample decay coefficient
        let impact_decay = (-1000.0 / (impact_decay_ms * srf)).exp();

        // ── Modal resonance tail (QUIET — just the body of the material) ─────
        // This is the subtle tonal "ring" after the impact.
        // Keep it VERY quiet or it becomes a musical instrument sound.
        // Modal freq = material resonance. Short decay (≤15ms).
        let modal_f0: f32 = match config.plate_material.as_str() {
            "fr4"                  => 2800.0,
            "brass"                => 2200.0,
            "aluminum"             => 1800.0,
            "nylon"                => 1200.0,
            "pom"                  =>  900.0, // POM: deep resonance
            "abs"                  =>  750.0,
            "polycarbonate" | "pc" =>  550.0, // PC: lowest, deepest thock
            _                      =>  900.0,
        } * config.pitch * if is_keyup { 1.25 } else { 1.0 };

        let modal_decay: f32 = match config.plate_material.as_str() {
            "fr4"                  => 0.020, // FR4 rings longer
            "brass"                => 0.018,
            "aluminum"             => 0.014,
            "polycarbonate" | "pc" => 0.010,
            "pom"                  => 0.007, // POM: very short tonal body
            "abs"                  => 0.005,
            _                      => 0.008,
        } * (1.0 - config.foam_mod * 0.50) // foam kills the ring
          * (1.0 + config.o_rings * 0.30); // o-rings extend it slightly

        // Modal volume: LOW. This adds body, not pitch.
        // If lube_amount is high, even less (smoother = less tonal ring)
        let modal_vol = 0.18 * (1.0 - config.lube_amount * 0.30) * spring_scale * keyup_scale;

        // 3 modes — fundamental, 2nd harmonic (slight), 3rd harmonic (tiny)
        let modal_amp1 = modal_vol;
        let modal_amp2 = modal_vol * 0.15; // barely audible second harmonic
        let modal_amp3 = modal_vol * 0.04; // nearly inaudible third
        let modes = [
            Modal::new(modal_f0,        modal_decay,        modal_amp1, 0.0,    srf),
            Modal::new(modal_f0 * 2.0,  modal_decay * 0.4,  modal_amp2, 0.001,  srf),
            Modal::new(modal_f0 * 3.0,  modal_decay * 0.15, modal_amp3, 0.002,  srf),
        ];

        // ── Friction: stem travel noise (very subtle) ─────────────────────────
        // Only present when not heavily lubed. Decays in ~8ms (stem travel time).
        let roughness = (1.0 - config.lube_amount).max(0.0).powi(2);
        let friction_fc = 300.0 + roughness * 500.0; // 300-800 Hz
        let friction_bpf = Biquad::bandpass(srf, friction_fc, 0.7);
        let friction_vol = roughness * 0.06 * vel; // very quiet
        let friction_decay = (-120.0 / srf).exp(); // decays in ~8ms

        // ── Click mechanism (clicky switches only) ────────────────────────────
        // A sharp pop/click at t≈3ms, separate from the main impact.
        let click_fc = 2800.0 * config.pitch;
        let click_bpf = Biquad::bandpass(srf, click_fc.min(5000.0), 1.2);
        let is_clicky = matches!(config.switch_type.as_str(), "clicky"|"blue"|"white"|"box_jade");
        let click_vol = if is_clicky && !is_keyup { spring_scale * vel * 0.35 } else { 0.0 };
        let click_decay = (-800.0 / srf).exp(); // ~5ms click burst

        // ── Tactile bump (tactile switches only) ──────────────────────────────
        // Tactile switches have a bump around 2-4ms into the press.
        // Simulated as a second, slightly-delayed smaller impact.
        // (No separate variable needed — reuse click infrastructure)

        // ── Output LPF ────────────────────────────────────────────────────────
        // Limit the harsh high end. Real switches top out around 5-7kHz.
        let lp_fc = 5000.0
            - config.lube_amount * 1500.0
            - config.foam_mod * 1000.0;
        let output_lpf = Biquad::lowpass(srf, lp_fc.max(1200.0), 0.60);

        // Remove DC offset
        let dc_hpf = Biquad::highpass(srf, 30.0, 0.707);

        // Foam saturation — subtle warmth
        let foam_sat = config.foam_mod * 0.15;

        // Duration: short for linears, longer for clickies
        let tail_ms: f32 = match config.switch_type.as_str() {
            "clicky" | "blue" | "box_jade"     => 80.0,
            "tactile" | "brown" | "holy_panda" => 65.0,
            "cream"                             => 45.0,
            _                                  => 55.0,
        };
        let total = (srf * tail_ms / 1000.0) as u64;

        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
            .wrapping_add((impact_fc * 173.0) as u32);

        Self {
            sr, cursor: 0, total, dt: 1.0 / srf, t: 0.0,
            prng: PRNG::new(seed),
            impact_lpf, impact_hpf,
            impact_env: 1.0,
            impact_decay,
            impact_vol,
            modes,
            modal_vol,
            friction_bpf,
            friction_env: 1.0,
            friction_decay,
            friction_vol,
            click_env: 1.0,
            click_decay,
            click_bpf,
            click_vol,
            output_lpf, dc_hpf,
            foam_sat,

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

        let noise = self.prng.next_f32();

        // ── PRIMARY: Impact transient (shaped noise burst) ────────────────────
        // This IS the keyboard sound. Noise filtered through a BPF centered
        // at the material's resonant frequency. Very fast decay (2-8ms).
        let impact_noise = self.impact_hpf.process(
            self.impact_lpf.process(noise)
        );
        let impact = impact_noise * self.impact_env * self.impact_vol;
        self.impact_env *= self.impact_decay;

        // ── SECONDARY: Modal resonance tail (subtle tonal body) ───────────────
        // Very quiet. Just adds the "body" of the material, not musicality.
        let modal = (self.modes[0].sample(t)
                  + self.modes[1].sample(t)
                  + self.modes[2].sample(t)) * self.modal_vol;

        // ── TERTIARY: Friction texture (stem travel, very subtle) ─────────────
        let friction = self.friction_bpf.process(noise) * self.friction_env * self.friction_vol;
        self.friction_env *= self.friction_decay;

        // ── Click mechanism burst (clicky switches only, at t≈3ms) ───────────
        let click = if t > 0.002 && t < 0.015 && self.click_vol > 0.0 {
            let c = self.click_bpf.process(noise) * self.click_env * self.click_vol;
            self.click_env *= self.click_decay;
            c
        } else { 0.0 };

        // ── Tactile bump (tactile: second filtered burst at t≈3ms) ────────────
        // For tactile switches the modal resonance IS the bump feel in sound

        let mut out = impact + modal + friction + click;

        // ── Output LPF: removes harshness above ~5kHz ─────────────────────────
        out = self.output_lpf.process(out);

        // ── DC removal ────────────────────────────────────────────────────────
        out = self.dc_hpf.process(out);

        // ── Foam saturation: subtle warmth, very low drive ────────────────────
        if self.foam_sat > 0.01 {
            out = soft_clip(out * (1.0 + self.foam_sat)) / (1.0 + self.foam_sat);
        }

        Some(out * 0.85)
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

pub static ASMR_MODE: AtomicU32 = AtomicU32::new(0); // 0=none, 1=rain, 2=thunder
pub static ASMR_MASTER_VOL: AtomicU32 = AtomicU32::new(50);
pub static ASMR_RAIN_DENS: AtomicU32 = AtomicU32::new(50);
pub static ASMR_WIND_DENS: AtomicU32 = AtomicU32::new(30);
pub static ASMR_RAIN_VOL: AtomicU32 = AtomicU32::new(70);
pub static ASMR_WIND_VOL: AtomicU32 = AtomicU32::new(40);
pub static ASMR_THUNDER_DELAY: AtomicU32 = AtomicU32::new(50);
pub static ASMR_THUNDER_INT: AtomicU32 = AtomicU32::new(70);
pub static ASMR_THUNDER_VOL: AtomicU32 = AtomicU32::new(80);

pub struct AsmrSource {
    prng: PRNG,
    
    // Pink noise state for Rain
    b0: f32, b1: f32, b2: f32, b3: f32, b4: f32, b5: f32, b6: f32,
    
    // Brown noise state for Wind/Thunder
    brown: f32,
    
    wind_lpf: Biquad,
    rain_hpf: Biquad,
    thunder_lpf: Biquad,
    
    thunder_env: f32,
    thunder_timer: u32,
    phase: f32,
}

impl AsmrSource {
    pub fn new(sr: u32) -> Self {
        Self {
            prng: PRNG::new(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u32),
            b0: 0.0, b1: 0.0, b2: 0.0, b3: 0.0, b4: 0.0, b5: 0.0, b6: 0.0,
            brown: 0.0,
            wind_lpf: Biquad::lowpass(sr as f32, 400.0, 0.5),
            rain_hpf: Biquad::highpass(sr as f32, 1200.0, 0.7),
            thunder_lpf: Biquad::lowpass(sr as f32, 200.0, 0.5),
            thunder_env: 0.0,
            thunder_timer: sr * 2,
            phase: 0.0,
        }
    }
}

impl Iterator for AsmrSource {
    type Item = f32;
    #[inline]
    fn next(&mut self) -> Option<f32> {
        let mode = ASMR_MODE.load(Ordering::Relaxed);
        if mode == 0 {
            return Some(0.0);
        }

        let m_vol = ASMR_MASTER_VOL.load(Ordering::Relaxed) as f32 / 100.0;
        let white = self.prng.next_f32(); // -1 to 1
        
        // Generate Pink Noise (Paul Kellet method)
        self.b0 = 0.99886 * self.b0 + white * 0.0555179;
        self.b1 = 0.99332 * self.b1 + white * 0.0750759;
        self.b2 = 0.96900 * self.b2 + white * 0.1538520;
        self.b3 = 0.86650 * self.b3 + white * 0.3104856;
        self.b4 = 0.55000 * self.b4 + white * 0.5329522;
        self.b5 = -0.7616 * self.b5 - white * 0.0168980;
        let pink_raw = self.b0 + self.b1 + self.b2 + self.b3 + self.b4 + self.b5 + self.b6 + white * 0.5362;
        self.b6 = white * 0.115926;
        let pink = pink_raw * 0.15; // Normalize roughly
        
        // Generate Brown Noise
        self.brown = (self.brown + 0.02 * white) / 1.02;
        let brown = self.brown * 4.0; // Normalize

        let mut out = 0.0;
        
        // --- WIND ---
        let wind_dens = ASMR_WIND_DENS.load(Ordering::Relaxed) as f32 / 100.0;
        let wind_v = ASMR_WIND_VOL.load(Ordering::Relaxed) as f32 / 100.0;
        
        // Modulate wind cutoff frequency slowly
        self.phase += 0.00005 * (0.5 + wind_dens);
        if self.phase > std::f32::consts::PI * 2.0 { self.phase -= std::f32::consts::PI * 2.0; }
        
        let lfo = (self.phase.sin() + (self.phase * 2.3).cos() * 0.5) * 0.5 + 0.5;
        let wind_cutoff = 100.0 + lfo * 800.0 * wind_dens;
        self.wind_lpf.set_lowpass(44100.0, wind_cutoff, 0.5);
        
        // Wind is brown noise swept by a lowpass filter
        let wind = self.wind_lpf.process(brown) * wind_v * 0.6;
        out += wind;

        // --- RAIN ---
        let rain_dens = ASMR_RAIN_DENS.load(Ordering::Relaxed) as f32 / 100.0;
        let rain_v = ASMR_RAIN_VOL.load(Ordering::Relaxed) as f32 / 100.0;
        
        // Rain is highpassed pink noise + sporadic intense crackles (drops)
        let mut drop = 0.0;
        if self.prng.next_f32().abs() < (0.002 * rain_dens) {
            drop = white * 1.5;
        }
        let rain_base = self.rain_hpf.process(pink + drop);
        let rain = rain_base * rain_v * (0.3 + rain_dens * 0.7) * 0.35;
        out += rain;

        // --- THUNDER ---
        if mode == 2 {
            let t_delay = ASMR_THUNDER_DELAY.load(Ordering::Relaxed);
            let t_int = ASMR_THUNDER_INT.load(Ordering::Relaxed) as f32 / 100.0;
            let t_vol = ASMR_THUNDER_VOL.load(Ordering::Relaxed) as f32 / 100.0;

            if self.thunder_timer > 0 {
                self.thunder_timer -= 1;
            } else {
                if self.prng.next_f32().abs() < 0.01 {
                    self.thunder_env = 1.0;
                    let next_base = 44100 * 5; 
                    let next_var = 44100 * 15;
                    let freq_factor = 1.0 - (t_delay as f32 / 100.0); 
                    self.thunder_timer = next_base + (self.prng.next_f32().abs() * next_var as f32 * freq_factor) as u32;
                }
            }

            if self.thunder_env > 0.0 {
                // Thunder is violently low-passed brown noise with a long decaying envelope
                self.thunder_lpf.set_lowpass(44100.0, 80.0 + self.thunder_env * 300.0 * t_int, 0.4 + self.thunder_env * 0.4);
                let strike = self.thunder_lpf.process(brown * 3.5);
                out += strike * self.thunder_env * t_vol * 1.5;
                
                // Extremely slow decay for natural rumble
                self.thunder_env -= 0.000005;
                if self.thunder_env < 0.0 {
                    self.thunder_env = 0.0;
                }
            }
        }

        Some(out * m_vol)
    }
}

impl Source for AsmrSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { 44100 }
    fn total_duration(&self) -> Option<std::time::Duration> { None }
}

