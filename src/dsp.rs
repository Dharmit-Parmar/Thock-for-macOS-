use rodio::Source;
use std::time::Duration;

pub struct PRNG {
    seed: u32,
}

impl PRNG {
    pub fn new(seed: u32) -> Self {
        Self { seed: if seed == 0 { 1 } else { seed } }
    }
    
    pub fn next_f32(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        // Map to -1.0 .. 1.0
        (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

pub struct Biquad {
    a0: f32, a1: f32, a2: f32,
    b1: f32, b2: f32,
    z1: f32, z2: f32,
}

impl Biquad {
    pub fn lowpass(sample_rate: f32, cutoff: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * cutoff / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        
        let a0 = 1.0 + alpha;
        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a1_c = -2.0 * cos_w0;
        let a2_c = 1.0 - alpha;
        
        Self {
            a0: b0 / a0, a1: b1 / a0, a2: b2 / a0,
            b1: a1_c / a0, b2: a2_c / a0,
            z1: 0.0, z2: 0.0,
        }
    }
    
    pub fn highpass(sample_rate: f32, cutoff: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * cutoff / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        
        let a0 = 1.0 + alpha;
        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a1_c = -2.0 * cos_w0;
        let a2_c = 1.0 - alpha;
        
        Self {
            a0: b0 / a0, a1: b1 / a0, a2: b2 / a0,
            b1: a1_c / a0, b2: a2_c / a0,
            z1: 0.0, z2: 0.0,
        }
    }

    pub fn bandpass(sample_rate: f32, center: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * center / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        
        let a0 = 1.0 + alpha;
        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a1_c = -2.0 * w0.cos();
        let a2_c = 1.0 - alpha;
        
        Self {
            a0: b0 / a0, a1: b1 / a0, a2: b2 / a0,
            b1: a1_c / a0, b2: a2_c / a0,
            z1: 0.0, z2: 0.0,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let output = input * self.a0 + self.z1;
        self.z1 = input * self.a1 - output * self.b1 + self.z2;
        self.z2 = input * self.a2 - output * self.b2;
        output
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ProceduralConfig {
    pub switch_type: String,     // "linear", "tactile", "clicky"
    pub spring_weight: f32,      // 45.0 - 80.0
    pub lube_amount: f32,        // 0.0 - 1.0
    pub pitch: f32,              // 0.5 - 2.0
    pub keycap_material: String, // "abs", "pbt"
    pub plate_material: String,  // "metal", "plastic"
    pub mounting_style: String,  // "tray", "gasket"
    pub foam_mod: f32,           // 0.0 - 1.0 (creamy/marbly)
    pub case_material: String,   // "plastic", "aluminum"
    pub o_rings: f32,            // 0.0 - 1.0 (Dampening)
}

impl Default for ProceduralConfig {
    fn default() -> Self {
        Self {
            switch_type: "black".to_string(),
            spring_weight: 70.0,
            lube_amount: 0.8,
            pitch: 0.9,
            keycap_material: "pbt".to_string(),
            plate_material: "pom".to_string(),
            mounting_style: "gasket".to_string(),
            foam_mod: 0.7,
            case_material: "aluminum".to_string(),
            o_rings: 0.0,
        }
    }
}

pub struct ProceduralSwitch {
    sample_rate: u32,
    cursor: u64,
    total_samples: u64,
    prng: PRNG,
    
    // Filters
    friction_lpf: Biquad,
    impact_bpf: Biquad,
    case_resonance: Biquad,
    gasket_hpf: Biquad,
    
    // State
    is_keyup: bool,
    velocity_mult: f32,
    config: ProceduralConfig,
}

impl ProceduralSwitch {
    pub fn new(config: ProceduralConfig, is_keyup: bool, velocity_mult: f32) -> Self {
        let sample_rate = 44100;
        let total_samples = (sample_rate as f32 * 0.15) as u64; // 150ms tail
        
        let lube_cutoff = 8000.0 - (config.lube_amount * 6000.0); // More lube = lower cutoff
        let friction_lpf = Biquad::lowpass(sample_rate as f32, lube_cutoff, 0.7);
        
        // Base frequency shifts with pitch
        let f0 = 300.0 * config.pitch; 
        
        // Foam mod drastically lowers Q (dampens ring) and attenuates highs
        let q_impact = 2.0 - (config.foam_mod * 1.5);
        let impact_bpf = Biquad::bandpass(sample_rate as f32, f0, q_impact);
        
        let case_q = if config.case_material == "aluminum" { 5.0 } else { 1.5 };
        let case_resonance = Biquad::bandpass(sample_rate as f32, f0 * 1.5, case_q);
        
        // Gasket mount isolates the plate from the case, filtering out low rumble
        let gasket_cutoff = if config.mounting_style == "gasket" { 250.0 } else { 20.0 };
        let gasket_hpf = Biquad::highpass(sample_rate as f32, gasket_cutoff, 0.707);

        Self {
            sample_rate,
            cursor: 0,
            total_samples,
            prng: PRNG::new(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()),
            friction_lpf,
            impact_bpf,
            case_resonance,
            gasket_hpf,
            is_keyup,
            velocity_mult,
            config,
        }
    }
}

impl Iterator for ProceduralSwitch {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.total_samples {
            return None;
        }

        let t = self.cursor as f32 / self.sample_rate as f32;
        self.cursor += 1;

        // 1. Friction Noise (Scratchiness)
        // Mutes heavily with lube.
        let noise = self.prng.next_f32();
        let friction_vol = (1.0 - self.config.lube_amount) * 0.05;
        let friction_env = (-t * 30.0).exp(); // Decays fast
        let friction_signal = self.friction_lpf.process(noise) * friction_env * friction_vol;

        // 2. Impact Excitation (Raised-Cosine / Transient)
        // Spring weight scales base amplitude. Velocity multiplies it.
        let mut spring_factor = self.config.spring_weight / 60.0;
        if self.is_keyup { spring_factor *= 0.6; } // Keyup is softer
        
        let mut impact_vol = spring_factor * self.velocity_mult;
        
        // O-Rings heavily dampen the impact volume and slow the attack
        impact_vol *= 1.0 - (self.config.o_rings * 0.6);
        let attack_time = 0.001 + (self.config.o_rings * 0.005); 
        
        // Plate material controls sustain / decay. Brass rings, POM/PC thuds.
        let decay_time = if self.config.plate_material == "pom" { 40.0 } else { 10.0 };
        
        let impact_env = if t < attack_time {
            t / attack_time
        } else {
            (-(t - attack_time) * decay_time).exp()
        };

        // Base oscillator (triangle/sine burst)
        let f0 = 300.0 * self.config.pitch;
        let osc = (t * f0 * 2.0 * std::f32::consts::PI).sin();
        let mut impact_signal = self.impact_bpf.process(osc * impact_env * impact_vol);

        // Clicky switch simulation (Blue / White)
        if (self.config.switch_type == "clicky" || self.config.switch_type == "blue" || self.config.switch_type == "white") && t < 0.02 && !self.is_keyup {
            let click_env = (-(t - 0.005).abs() * 500.0).exp();
            // White switches have a slightly higher, thinner click than blue
            let click_freq = if self.config.switch_type == "white" { 3800.0 } else { 3000.0 };
            let click_osc = (t * click_freq * 2.0 * std::f32::consts::PI).sin();
            impact_signal += click_osc * click_env * 0.5;
        }

        // Tactile bump simulation (Brown)
        if (self.config.switch_type == "tactile" || self.config.switch_type == "brown") && t < 0.015 && !self.is_keyup {
            let bump_env = (-(t - 0.002).abs() * 300.0).exp();
            let bump_osc = (t * 150.0 * 2.0 * std::f32::consts::PI).sin();
            impact_signal += bump_osc * bump_env * 0.3;
        }

        // 3. Case Resonance
        // Combines everything and filters it
        let mut mixed = friction_signal + impact_signal;
        
        let case_verb = self.case_resonance.process(mixed) * 0.3;
        mixed += case_verb;
        
        // Apply Gasket Isolation (High-Pass)
        mixed = self.gasket_hpf.process(mixed);

        // Foam Mod (Butter/Creamy): Soft clips the transient and boosts mid-bass
        if self.config.foam_mod > 0.0 {
            // Soft clipping
            mixed = mixed.tanh();
        }

        Some(mixed * 0.5) // Master headroom
    }
}

impl Source for ProceduralSwitch {
    fn current_frame_len(&self) -> Option<usize> {
        Some((self.total_samples - self.cursor) as usize)
    }
    fn channels(&self) -> u16 {
        1 // Mono
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.total_samples as f32 / self.sample_rate as f32))
    }
}
