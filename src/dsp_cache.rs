
use crate::dsp::*;

#[derive(Clone)]
pub struct ProceduralTemplate {
    pub impact_lpf: Biquad,
    pub impact_decay_ms: f32,
    pub impact_vol_base: f32,
    pub modes: [Modal; 3],
    pub modal_vol_base: f32,
    pub friction_bpf: Biquad,
    pub friction_decay: f32,
    pub friction_vol_base: f32,
    pub click_bpf: Biquad,
    pub click_decay: f32,
    pub click_vol_base: f32,
    pub output_lpf: Biquad,
    pub dc_hpf: Biquad,
    pub foam_sat: f32,
    pub foam_recip: f32,
    pub total: u64,
}

impl ProceduralTemplate {
    pub fn new(config: &ProceduralConfig, is_keyup: bool) -> Self {
        let sr = 44100u32;
        let srf = sr as f32;

        let impact_fc: f32 = match config.plate_material.as_str() {
            "fr4"                  => 3500.0,
            "brass"                => 2800.0,
            "aluminum"             => 2400.0,
            "nylon"                => 1400.0,
            "pom"                  => 1200.0,
            "abs"                  => 1000.0,
            "polycarbonate" | "pc" =>  700.0,
            _                      => 1200.0,
        };

        let impact_fc = impact_fc * config.pitch * (1.0 - config.lube_amount * 0.30);
        let impact_fc = impact_fc * if is_keyup { 1.25 } else { 1.0 };
        let impact_fc = impact_fc * (1.0 - config.o_rings * 0.20) * (1.0 - config.foam_mod * 0.15);

        let impact_q = 0.5 + config.lube_amount * 0.5;
        let impact_lpf = Biquad::lowpass(srf, impact_fc.min(8000.0), impact_q);

        let spring_scale = (config.spring_weight / 55.0).clamp(0.6, 1.4);
        let keyup_scale = if is_keyup { 0.55 } else { 1.0 };
        let impact_vol_base = spring_scale * keyup_scale;

        let base_impact_decay_ms: f32 = match config.switch_type.as_str() {
            "cream"                             => 4.5,
            "linear" | "red" | "black"         => 3.5,
            "tactile" | "brown" | "holy_panda" => 5.0,
            "clicky" | "blue" | "box_jade"     => 3.0,
            _                                  => 4.0,
        };
        let impact_decay_ms = base_impact_decay_ms * (1.0 + config.foam_mod * 0.5);

        let modal_f0: f32 = match config.plate_material.as_str() {
            "fr4"                  => 2800.0,
            "brass"                => 2200.0,
            "aluminum"             => 1800.0,
            "nylon"                => 1200.0,
            "pom"                  =>  900.0,
            "abs"                  =>  750.0,
            "polycarbonate" | "pc" =>  550.0,
            _                      =>  900.0,
        } * config.pitch * if is_keyup { 1.25 } else { 1.0 };

        let modal_decay: f32 = match config.plate_material.as_str() {
            "fr4"                  => 0.020,
            "brass"                => 0.018,
            "aluminum"             => 0.014,
            "polycarbonate" | "pc" => 0.010,
            "pom"                  => 0.007,
            "abs"                  => 0.005,
            _                      => 0.008,
        } * (1.0 - config.foam_mod * 0.50);

        let modal_vol_base = match config.plate_material.as_str() {
            "aluminum" | "brass" => 0.008,
            "fr4"                => 0.005,
            "pom" | "nylon"      => 0.003,
            "polycarbonate" | "pc"=> 0.002,
            _                    => 0.004,
        } * spring_scale * (if is_keyup { 0.4 } else { 1.0 });

        let modal_amp1 = 1.0;
        let modal_amp2 = 0.3 * (1.0 - config.lube_amount).max(0.2);
        let modal_amp3 = 0.1 * (1.0 - config.lube_amount).max(0.1);

        let modes = [
            Modal::new(modal_f0,        modal_decay,        modal_amp1, srf, 0.0),
            Modal::new(modal_f0 * 2.0,  modal_decay * 0.4,  modal_amp2, srf, 45.0),
            Modal::new(modal_f0 * 3.0,  modal_decay * 0.15, modal_amp3, srf, 89.0),
        ];

        let roughness = (1.0 - config.lube_amount).max(0.0).powi(2);
        let friction_fc = 300.0 + roughness * 500.0;
        let friction_bpf = Biquad::bandpass(srf, friction_fc, 0.7);
        let friction_vol_base = roughness * 0.06;
        let friction_decay = (-120.0 / srf).exp();

        let click_fc = 2800.0 * config.pitch;
        let click_bpf = Biquad::bandpass(srf, click_fc.min(5000.0), 1.2);
        let is_clicky = matches!(config.switch_type.as_str(), "clicky"|"blue"|"white"|"box_jade");
        let click_vol_base = if is_clicky && !is_keyup { spring_scale * 0.35 } else { 0.0 };
        let click_decay = (-800.0 / srf).exp();

        let lp_fc = 5000.0 - config.lube_amount * 1500.0 - config.foam_mod * 1000.0;
        let output_lpf = Biquad::lowpass(srf, lp_fc.max(1200.0), 0.60);
        let dc_hpf = Biquad::highpass(srf, 30.0, 0.707);
        let foam_sat = config.foam_mod * 0.15;
        let foam_recip = 0.85 / (1.0 + foam_sat);

        let tail_ms: f32 = match config.switch_type.as_str() {
            "clicky" | "blue" | "box_jade"     => 80.0,
            "tactile" | "brown" | "holy_panda" => 65.0,
            "cream"                             => 45.0,
            _                                  => 55.0,
        };
        let total = (srf * tail_ms / 1000.0) as u64;

        Self {
            impact_lpf, impact_decay_ms, impact_vol_base,
            modes, modal_vol_base,
            friction_bpf, friction_decay, friction_vol_base,
            click_bpf, click_decay, click_vol_base,
            output_lpf, dc_hpf, foam_sat, foam_recip, total,
        }
    }
}

pub struct ProceduralCache {
    pub down: ProceduralTemplate,
    pub up: ProceduralTemplate,
}

impl ProceduralCache {
    pub fn new(config: &ProceduralConfig) -> Self {
        Self {
            down: ProceduralTemplate::new(config, false),
            up: ProceduralTemplate::new(config, true),
        }
    }
}
