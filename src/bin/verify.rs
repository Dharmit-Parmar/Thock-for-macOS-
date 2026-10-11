use std::io::Cursor;
use rodio::{Decoder, Source};

fn analyze(name: &str, bytes: &'static [u8]) {
    let cursor = Cursor::new(bytes);
    let decoder = Decoder::new(cursor).unwrap();
    let channels = decoder.channels();
    let sr = decoder.sample_rate();
    let samples: Vec<f32> = decoder.convert_samples::<f32>().collect();
    let total_frames = samples.len() / channels as usize;
    
    let mut min = 0.0f32;
    let mut max = 0.0f32;
    let mut sum_sq = 0.0f64;
    for &s in &samples {
        if s < min { min = s; }
        if s > max { max = s; }
        sum_sq += (s * s) as f64;
    }
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;

    // Check loop discontinuity between last frame and first frame
    let first_l = samples[0];
    let first_r = if channels > 1 { samples[1] } else { samples[0] };
    let last_l = samples[samples.len() - (channels as usize)];
    let last_r = samples[samples.len() - 1];
    let jump_l = (last_l - first_l).abs();
    let jump_r = (last_r - first_r).abs();

    // Measure high-frequency vs low-frequency energy
    let mut low_energy = 0.0f64;
    let mut high_energy = 0.0f64;
    let mut prev_s = 0.0f32;
    for &s in &samples {
        let diff = s - prev_s; // High-pass differentiator
        high_energy += (diff * diff) as f64;
        low_energy += (s * s) as f64;
        prev_s = s;
    }
    let harshness_ratio = (high_energy / (low_energy + 1e-9)).sqrt();

    // Now test with gentle acoustic low-pass filter (3400 Hz)
    let w = 2.0 * std::f32::consts::PI * (3400.0 / sr as f32).min(0.499);
    let cw = w.cos();
    let alpha = w.sin() / (2.0 * 0.707);
    let a0r = 1.0 / (1.0 + alpha);
    let b = (1.0 - cw) / 2.0;
    let b0 = b * a0r;
    let b1 = (1.0 - cw) * a0r;
    let b2 = b * a0r;
    let a1 = -2.0 * cw * a0r;
    let a2 = (1.0 - alpha) * a0r;
    let mut s1 = 0.0f32;
    let mut s2 = 0.0f32;
    let mut filt_low = 0.0f64;
    let mut filt_high = 0.0f64;
    let mut filt_prev = 0.0f32;
    for &s in &samples {
        let y = b0 * s + s1;
        s1 = b1 * s - a1 * y + s2;
        s2 = b2 * s - a2 * y;
        let diff = y - filt_prev;
        filt_high += (diff * diff) as f64;
        filt_low += (y * y) as f64;
        filt_prev = y;
    }
    let filt_harshness = (filt_high / (filt_low + 1e-9)).sqrt();

    println!("{}: sr={}, ch={}, frames={}, duration={:.2}s", name, sr, channels, total_frames, total_frames as f32 / sr as f32);
    println!("  min={:.3}, max={:.3}, rms={:.3} ({:.1} dBFS)", min, max, rms, 20.0 * rms.log10());
    println!("  loop jump: L={:.3}, R={:.3}, raw harshness={:.3} -> filtered harshness={:.3}", jump_l, jump_r, harshness_ratio, filt_harshness);
}

fn main() {
    analyze("rain.ogg (old file)", include_bytes!("../../assets/asmr/rain.ogg"));
    analyze("wind.ogg", include_bytes!("../../assets/asmr/wind.ogg"));
    analyze("thunder.ogg", include_bytes!("../../assets/asmr/thunder.ogg"));

    if let Some(rain_buf) = thock::get_rain_buffer() {
        let frames = rain_buf.samples.len() / rain_buf.channels as usize;
        let dur = frames as f32 / rain_buf.sample_rate as f32;
        let max_val = rain_buf.samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        let sum_sq: f64 = rain_buf.samples.iter().map(|&s| {
            let f = s as f32 / 32768.0;
            (f * f) as f64
        }).sum();
        let rms = (sum_sq / rain_buf.samples.len() as f64).sqrt() as f32;
        println!("\nProcedural Rain Buffer (in-memory, 0% motor drone):");
        println!("  channels={}, sample_rate={}, frames={}, duration={:.2}s", rain_buf.channels, rain_buf.sample_rate, frames, dur);
        println!("  peak={:.3}, rms={:.3} ({:.1} dBFS)", max_val as f32 / 32768.0, rms, 20.0 * rms.log10());
    }

    if let Some(thunder_buf) = thock::get_thunder_buffer() {
        let frames = thunder_buf.samples.len() / thunder_buf.channels as usize;
        let dur = frames as f32 / thunder_buf.sample_rate as f32;
        let max_val = thunder_buf.samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        let sum_sq: f64 = thunder_buf.samples.iter().map(|&s| {
            let f = s as f32 / 32768.0;
            (f * f) as f64
        }).sum();
        let rms = (sum_sq / thunder_buf.samples.len() as f64).sqrt() as f32;
        println!("\nTrimmed Thunder Buffer (in-memory, isolated strike):");
        println!("  channels={}, sample_rate={}, frames={}, duration={:.2}s", thunder_buf.channels, thunder_buf.sample_rate, frames, dur);
        println!("  peak={:.3}, rms={:.3} ({:.1} dBFS)", max_val as f32 / 32768.0, rms, 20.0 * rms.log10());
    }
}

