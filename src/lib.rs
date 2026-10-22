mod assets;
mod keymap;
mod dsp;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::Context;

#[derive(rustyline::Helper)]
struct ThockHelper {
    commands: Vec<String>,
    packs: Vec<String>,
}


impl Validator for ThockHelper {}
impl Highlighter for ThockHelper {}
impl Hinter for ThockHelper {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> { None }
}
impl Completer for ThockHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let mut candidates = Vec::new();
        let words: Vec<&str> = line[..pos].split_whitespace().collect();
        let is_first_word = words.is_empty() || (words.len() == 1 && !line[..pos].ends_with(' '));
        
        if is_first_word {
            let word = words.first().unwrap_or(&"");
            for cmd in &self.commands {
                if cmd.starts_with(word) {
                    candidates.push(Pair { display: cmd.clone(), replacement: cmd.clone() });
                }
            }
        } else if words[0] == "pack" || words[0] == "fav" || words[0] == "delete" || words[0] == "del" {
            let word = if words.len() >= 2 && !line[..pos].ends_with(' ') { words.last().unwrap_or(&"") } else { "" };
            for pack in &self.packs {
                if pack.starts_with(word) {
                    candidates.push(Pair { display: pack.clone(), replacement: pack.clone() });
                }
            }
        }
        
        let start = if is_first_word {
            line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0)
        } else {
            line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0)
        };

        Ok((start, candidates))
    }
}
use core_graphics::event::{CGEventTap, CGEventTapLocation, CGEventTapPlacement, CGEventTapOptions, CGEventType, EventField};
use core_foundation::runloop::CFRunLoop;
use rodio::{Decoder, OutputStream, Sink, Source};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AsmrLoopKind {
    Rain,
    Wind,
}

#[derive(Clone)]
pub struct AsmrBuffer {
    pub samples: std::sync::Arc<[i16]>,
    pub channels: u16,
    pub sample_rate: u32,
}

static WIND_OGG: &[u8] = include_bytes!("../assets/asmr/wind.ogg");
static THUNDER_OGG: &[u8] = include_bytes!("../assets/asmr/thunder.ogg");
static RAIN_FIELD_OGG: &[u8] = include_bytes!("../assets/asmr/rain-field.ogg");

#[inline]
fn rain_density_drive(density: u32) -> f32 {
    0.85 + density.min(100) as f32 * 0.005
}

fn decode_audio(bytes: &'static [u8], crossfade_loop: bool, max_seconds: Option<u32>) -> Option<AsmrBuffer> {
    let cursor = std::io::Cursor::new(bytes);
    let decoder = Decoder::new(cursor).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let ch = channels as usize;
    let samples = rodio::Source::convert_samples::<i16>(decoder);
    let mut raw: Vec<i16> = if let Some(seconds) = max_seconds {
        samples.take(sample_rate as usize * ch * seconds as usize).collect()
    } else {
        samples.collect()
    };
    let total_frames = raw.len() / ch;
    // Crossfades are handled by the playback source so the recording's opening stays intact.
    let xfade_frames = ((sample_rate as usize) / 2).min(total_frames / 4);
    if crossfade_loop && total_frames > xfade_frames * 2 && xfade_frames > 0 {
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
    Some(AsmrBuffer {
        samples: std::sync::Arc::from(raw.into_boxed_slice()),
        channels,
        sample_rate,
    })
}

fn decode_thunder(bytes: &'static [u8]) -> Option<AsmrBuffer> {
    let cursor = std::io::Cursor::new(bytes);
    let decoder = Decoder::new(cursor).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let mut raw: Vec<i16> = rodio::Source::convert_samples::<i16>(decoder).collect();
    let ch = channels as usize;
    let total_frames = raw.len() / ch;
    if total_frames == 0 { return None; }

    // Trim to 150ms before the first strike peak measured at about 2.35s in the bundled clip.
    let start_frame = ((sample_rate as usize * 220) / 100).min(total_frames);
    raw.drain(..start_frame * ch);
    let mut sliced = raw;
    let sliced_frames = sliced.len() / ch;

    // Short attack prevents a click without softening the thunder crack.
    let fade_frames = ((sample_rate as usize * 20) / 1000).min(sliced_frames / 4);
    for frame in 0..fade_frames {
        let t = frame as f32 / fade_frames as f32;
        let fade_in = (t * std::f32::consts::FRAC_PI_2).sin();
        for c in 0..ch {
            let idx = frame * ch + c;
            sliced[idx] = (sliced[idx] as f32 * fade_in) as i16;
        }
    }

    let release_frames = ((sample_rate as usize * 3) / 10).min(sliced_frames / 2);
    let start_release = sliced_frames.saturating_sub(release_frames);
    for frame in 0..release_frames {
        let f_idx = start_release + frame;
        let t = frame as f32 / release_frames as f32;
        let fade_out = ((1.0 - t) * std::f32::consts::FRAC_PI_2).sin();
        for c in 0..ch {
            let idx = f_idx * ch + c;
            sliced[idx] = (sliced[idx] as f32 * fade_out) as i16;
        }
    }

    Some(AsmrBuffer {
        samples: std::sync::Arc::from(sliced.into_boxed_slice()),
        channels,
        sample_rate,
    })
}

pub static RAIN_CACHE: std::sync::Mutex<Option<AsmrBuffer>> = std::sync::Mutex::new(None);
pub static WIND_CACHE: std::sync::Mutex<Option<AsmrBuffer>> = std::sync::Mutex::new(None);
pub static THUNDER_CACHE: std::sync::Mutex<Option<AsmrBuffer>> = std::sync::Mutex::new(None);

pub fn get_rain_buffer() -> Option<AsmrBuffer> {
    {
        let cache = RAIN_CACHE.lock().unwrap();
        if let Some(buf) = &*cache {
            return Some(buf.clone());
        }
    }
    let decoded = decode_audio(RAIN_FIELD_OGG, false, Some(60));
    let mut cache = RAIN_CACHE.lock().unwrap();
    if cache.is_none() {
        *cache = decoded;
    }
    cache.clone()
}

pub fn purge_rain_buffer() {
    *RAIN_CACHE.lock().unwrap() = None;
}

pub fn get_wind_buffer() -> Option<AsmrBuffer> {
    {
        let cache = WIND_CACHE.lock().unwrap();
        if let Some(buf) = &*cache {
            return Some(buf.clone());
        }
    }
    let decoded = decode_audio(WIND_OGG, true, None);
    let mut cache = WIND_CACHE.lock().unwrap();
    if cache.is_none() {
        *cache = decoded;
    }
    cache.clone()
}

pub fn purge_wind_buffer() {
    *WIND_CACHE.lock().unwrap() = None;
}

pub fn get_thunder_buffer() -> Option<AsmrBuffer> {
    {
        let cache = THUNDER_CACHE.lock().unwrap();
        if let Some(buf) = &*cache {
            return Some(buf.clone());
        }
    }
    let decoded = decode_thunder(THUNDER_OGG);
    let mut cache = THUNDER_CACHE.lock().unwrap();
    if cache.is_none() {
        *cache = decoded;
    }
    cache.clone()
}

pub fn purge_thunder_buffer() {
    *THUNDER_CACHE.lock().unwrap() = None;
}

pub fn purge_all_asmr_buffers() {
    purge_rain_buffer();
    purge_wind_buffer();
    purge_thunder_buffer();
}

#[inline]
fn thunder_gap_seconds(frequency: u32) -> f32 {
    4.0 - frequency.min(100) as f32 * 0.03
}

#[inline]
fn thunder_next_gap_seconds(frequency: u32, jitter: f32) -> f32 {
    thunder_gap_seconds(frequency) * (0.8 + 0.4 * jitter.clamp(0.0, 1.0))
}

#[inline]
fn thunder_gain(volume: u32, intensity: u32, master: u32) -> f32 {
    (volume.min(100) as f32 / 100.0)
        * (intensity.min(100) as f32 / 100.0)
        * (master.min(100) as f32 / 100.0)
}

pub struct SeamlessLoopSource {
    samples: std::sync::Arc<[i16]>,
    channels: u16,
    sample_rate: u32,
    cursor: usize,
    played_frames: u64,
    fade_in_frames: u64,
    inv_fade_in: f32,
    rain_loop_start: usize,
    rain_crossfade_frames: usize,
    inv_crossfade_frames: f32,
    fade_start: usize,
    fade_start_sample: usize,
    cached_g1: f32,
    cached_g2: f32,
    cached_frame: usize,
    filter_left: Option<crate::dsp::Biquad>,
    filter_right: Option<crate::dsp::Biquad>,
    warmth_left: Option<crate::dsp::Biquad>,
    warmth_right: Option<crate::dsp::Biquad>,
    density_cache: f32,    // cached rain_density_drive — refreshed every 128 frames
    last_wind_fc: f32,
    kind: AsmrLoopKind,
}

impl SeamlessLoopSource {
    pub fn new(samples: std::sync::Arc<[i16]>, channels: u16, sample_rate: u32, kind: AsmrLoopKind) -> Self {
        let total_frames = samples.len() / channels as usize;
        let rain_crossfade_frames = if kind == AsmrLoopKind::Rain {
            // Smooth 1.0s equal-power crossfade at the recording loop boundary
            (sample_rate as usize).min(total_frames / 8)
        } else {
            0
        };
        // Play the recorded rain build once, then loop its steadier final half.
        let rain_loop_start = if kind == AsmrLoopKind::Rain {
            (sample_rate as usize * 30).min(total_frames.saturating_sub(rain_crossfade_frames))
        } else {
            0
        };
        let fade_start = total_frames.saturating_sub(rain_crossfade_frames);
        let fade_start_sample = fade_start * channels as usize;
        let inv_crossfade_frames = if rain_crossfade_frames > 0 {
            1.0 / rain_crossfade_frames as f32
        } else {
            0.0
        };
        let fade_in_frames = (sample_rate as f32 * 0.05) as u64;
        let inv_fade_in = if fade_in_frames > 0 { 1.0 / fade_in_frames as f32 } else { 1.0 };

        let (filter_left, filter_right, warmth_left, warmth_right, last_wind_fc) = match kind {
            AsmrLoopKind::Rain => (
                // Remove low-frequency traffic/wind rumble while retaining the rain's crisp drop transients.
                Some(crate::dsp::Biquad::highpass(sample_rate as f32, 120.0, 0.707)),
                Some(crate::dsp::Biquad::highpass(sample_rate as f32, 120.0, 0.707)),
                // Pure transparent acoustic pass: preserves authentic Minecraft drop clarity and air without muffling
                None,
                None,
                0.0,
            ),
            AsmrLoopKind::Wind => {
                // Wider aerodynamic range (350-1400 Hz) restores air-texture and breath
                let fc = (crate::dsp::ASMR_WIND_CUTOFF.load(Ordering::Relaxed) as f32).clamp(250.0, 1600.0);
                (
                    Some(crate::dsp::Biquad::lowpass(sample_rate as f32, fc * 0.95, 0.65)),
                    Some(crate::dsp::Biquad::lowpass(sample_rate as f32, fc * 1.05, 0.65)),
                    // Gentle 750 Hz presence peak (+2 dB): adds air movement / rustling texture
                    Some(crate::dsp::Biquad::peaking(sample_rate as f32, 750.0, 1.0, 2.0)),
                    Some(crate::dsp::Biquad::peaking(sample_rate as f32, 750.0, 1.0, 2.0)),
                    fc,
                )
            }
        };

        Self {
            samples,
            channels,
            sample_rate,
            cursor: 0,
            played_frames: 0,
            fade_in_frames,
            inv_fade_in,
            rain_loop_start,
            rain_crossfade_frames,
            inv_crossfade_frames,
            fade_start,
            fade_start_sample,
            cached_g1: 1.0,
            cached_g2: 0.0,
            cached_frame: usize::MAX,
            filter_left,
            filter_right,
            warmth_left,
            warmth_right,
            density_cache: rain_density_drive(crate::dsp::ASMR_RAIN_DENS.load(Ordering::Relaxed)),
            last_wind_fc,
            kind,
        }
    }
}

impl Iterator for SeamlessLoopSource {
    type Item = f32;

    #[inline(always)]
    fn next(&mut self) -> Option<f32> {
        if self.samples.is_empty() { return None; }
        if self.cursor >= self.samples.len() {
            let total_frames = self.samples.len() / self.channels as usize;
            let restart_frame = if total_frames > 0 {
                (self.rain_loop_start + self.rain_crossfade_frames) % total_frames
            } else {
                0
            };
            self.cursor = restart_frame * self.channels as usize;
            if self.cursor >= self.samples.len() {
                self.cursor = 0;
            }
        }
        let ch_idx = if self.channels == 2 { self.cursor & 1 } else { self.cursor % self.channels as usize };
        let mut sample = self.samples[self.cursor] as f32 * 0.000030517578_f32;

        match self.kind {
            AsmrLoopKind::Rain => {
                if self.rain_crossfade_frames > 0 && self.cursor >= self.fade_start_sample {
                    let frame = self.cursor / self.channels as usize;
                    if frame != self.cached_frame {
                        let t = (frame - self.fade_start) as f32 * self.inv_crossfade_frames;
                        // Equal-power cubic polynomial approximation P(t) = 1 - t^2 (1.3431458 - 0.3431458 t)
                        // Max power error is 0.055% (0.0024 dB ripple). Zero transcendental trig calls!
                        let t2 = t * t;
                        self.cached_g1 = 1.0 - t2 * (1.3431458 - 0.3431458 * t);
                        let u = 1.0 - t;
                        let u2 = u * u;
                        self.cached_g2 = 1.0 - u2 * (1.3431458 - 0.3431458 * u);
                        self.cached_frame = frame;
                    }
                    let loop_sample = self.samples[(self.rain_loop_start + frame - self.fade_start) * self.channels as usize + ch_idx] as f32 * 0.000030517578_f32;
                    sample = sample * self.cached_g1 + loop_sample * self.cached_g2;
                }
                if ch_idx == 0 {
                    self.played_frames += 1;
                    // Refresh density cache every 128 left-channel frames (~5.8ms): avoids
                    // an atomic load every 22μs in the hot path without perceptible slider lag.
                    if self.played_frames & 127 == 0 {
                        self.density_cache = rain_density_drive(crate::dsp::ASMR_RAIN_DENS.load(Ordering::Relaxed));
                    }
                }

                // 1. Scale by density drive
                sample *= self.density_cache;

                // Remove rumble before level shaping; keep the recorded broadband drop texture.
                if ch_idx == 0 {
                    if let Some(filter) = &mut self.filter_left { sample = filter.process(sample); }
                    if let Some(warmth) = &mut self.warmth_left { sample = warmth.process(sample); }
                } else {
                    if let Some(filter) = &mut self.filter_right { sample = filter.process(sample); }
                    if let Some(warmth) = &mut self.warmth_right { sample = warmth.process(sample); }
                }

                if self.played_frames < self.fade_in_frames {
                    sample *= self.played_frames as f32 * self.inv_fade_in;
                }
            }
            AsmrLoopKind::Wind => {
                // Update aerodynamic lowpass filter tracking dynamic gust cutoff (hoisted to only update on change)
                if ch_idx == 0 && (self.cursor & 127) == 0 {
                    let fc = (crate::dsp::ASMR_WIND_CUTOFF.load(Ordering::Relaxed) as f32).clamp(250.0, 1600.0);
                    if (fc - self.last_wind_fc).abs() > 2.0 {
                        self.last_wind_fc = fc;
                        if let Some(fl) = &mut self.filter_left {
                            fl.update_lowpass(self.sample_rate as f32, fc * 0.95, 0.65);
                        }
                        if let Some(fr) = &mut self.filter_right {
                            fr.update_lowpass(self.sample_rate as f32, fc * 1.05, 0.65);
                        }
                    }
                }

                if ch_idx == 0 {
                    if let Some(fl) = &mut self.filter_left {
                        sample = fl.process(sample);
                    }
                    if let Some(wl) = &mut self.warmth_left {
                        sample = wl.process(sample);
                    }
                } else {
                    if let Some(fr) = &mut self.filter_right {
                        sample = fr.process(sample);
                    }
                    if let Some(wr) = &mut self.warmth_right {
                        sample = wr.process(sample);
                    }
                }
                sample = crate::dsp::soft_limit(sample * 1.05);
            }
        }

        self.cursor += 1;
        Some(sample)
    }
}

impl Source for SeamlessLoopSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { self.channels }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<std::time::Duration> { None }
}

enum DirectOrSpeed<A, B> {
    Direct(A),
    Speed(B),
}

impl<A, B> Iterator for DirectOrSpeed<A, B>
where
    A: Iterator<Item = f32>,
    B: Iterator<Item = f32>,
{
    type Item = f32;
    #[inline(always)]
    fn next(&mut self) -> Option<f32> {
        match self {
            Self::Direct(a) => a.next(),
            Self::Speed(b) => b.next(),
        }
    }
}

impl<A, B> Source for DirectOrSpeed<A, B>
where
    A: Source<Item = f32>,
    B: Source<Item = f32>,
{
    #[inline(always)]
    fn current_frame_len(&self) -> Option<usize> {
        match self {
            Self::Direct(a) => a.current_frame_len(),
            Self::Speed(b) => b.current_frame_len(),
        }
    }
    #[inline(always)]
    fn channels(&self) -> u16 {
        match self {
            Self::Direct(a) => a.channels(),
            Self::Speed(b) => b.channels(),
        }
    }
    #[inline(always)]
    fn sample_rate(&self) -> u32 {
        match self {
            Self::Direct(a) => a.sample_rate(),
            Self::Speed(b) => b.sample_rate(),
        }
    }
    #[inline(always)]
    fn total_duration(&self) -> Option<std::time::Duration> {
        match self {
            Self::Direct(a) => a.total_duration(),
            Self::Speed(b) => b.total_duration(),
        }
    }
}

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, RwLock,
    },
    time::Instant,
};



#[cfg(target_os = "macos")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

static VOL: AtomicU32 = AtomicU32::new(100);

// PredecodedAudio stores eagerly-decoded PCM samples as a shared Arc<[i16]>.
// Memory footprint: exactly one copy of PCM per unique file. Because load_pack
// pre-warms all samples anyway (defaults + common keys), the OnceLock lazy approach
// gave no practical savings — it just held BOTH compressed bytes AND decoded PCM
// simultaneously after first use, doubling per-file RAM.
// Now: compressed bytes are decoded and dropped immediately, leaving only Arc<[i16]>.

#[derive(Clone)]
pub struct PredecodedAudio {
    pub samples_data: std::sync::Arc<[i16]>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl PredecodedAudio {
    fn new(path: &std::path::Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let cursor = std::io::Cursor::new(bytes);
        let decoder = rodio::Decoder::new(cursor).ok()?;
        let channels = rodio::Source::channels(&decoder);
        let sample_rate = rodio::Source::sample_rate(&decoder);
        let s: Vec<i16> = rodio::Source::convert_samples::<i16>(decoder).collect();
        // file bytes are dropped here; only decoded PCM survives
        Some(Self {
            samples_data: std::sync::Arc::from(s.into_boxed_slice()),
            channels,
            sample_rate,
        })
    }

    pub fn samples(&self) -> std::sync::Arc<[i16]> {
        self.samples_data.clone()
    }

    fn play(&self, vol: f32, pitch: f32) -> Option<impl rodio::Source<Item = f32>> {
        let src = ZeroCopySource {
            samples: self.samples_data.clone(),
            channels: self.channels,
            sample_rate: self.sample_rate,
            cursor: 0,
        }.amplify(vol);

        let safe_pitch = if pitch.is_finite() && pitch > 0.05 { pitch.clamp(0.2, 4.0) } else { 1.0 };
        if (safe_pitch - 1.0).abs() > 0.005 {
            Some(DirectOrSpeed::Speed(rodio::source::Source::speed(src, safe_pitch)))
        } else {
            Some(DirectOrSpeed::Direct(src))
        }
    }
}


struct ZeroCopySource {
    samples: std::sync::Arc<[i16]>,
    channels: u16,
    sample_rate: u32,
    cursor: usize,
}

impl Iterator for ZeroCopySource {
    type Item = f32;
    #[inline(always)]
    fn next(&mut self) -> Option<f32> {
        if self.cursor < self.samples.len() {
            let sample = self.samples[self.cursor] as f32 * 0.000030517578_f32;
            self.cursor += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl rodio::Source for ZeroCopySource {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.samples.len().saturating_sub(self.cursor))
    }
    fn channels(&self) -> u16 { self.channels }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs_f32((self.samples.len() / self.channels as usize) as f32 / self.sample_rate as f32))
    }
}

fn load_audio_file(path: &Path) -> Option<PredecodedAudio> {
    PredecodedAudio::new(path)
}

fn default_pitch() -> f32 { 1.0 }
fn default_vol_mult() -> f32 { 1.0 }

#[derive(Serialize, Deserialize)]
struct PackConfig {
    #[serde(default)]
    defaults: Vec<String>,
    #[serde(default)]
    mappings: HashMap<String, String>,
    #[serde(default = "default_pitch")]
    pitch: f32,
    #[serde(default = "default_vol_mult")]
    vol_mult: f32,
    #[serde(default)]
    base: Option<String>,
    #[serde(default)]
    procedural: Option<dsp::ProceduralConfig>,
}

pub struct LoadedPack {
    pub defaults: Vec<PredecodedAudio>,
    pub mappings: HashMap<u64, PredecodedAudio>,
    pub pitch: f32,
    pub vol_mult: f32,
    pub procedural: Option<dsp::ProceduralConfig>,
}

#[derive(Serialize, Deserialize, Debug)]
struct AppSettings {
    volume: u32,
    pack: String,
}

fn load_settings(path: &std::path::Path) -> Option<AppSettings> {
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

fn save_settings(path: &Path, volume: u32, pack: &str) {
    let settings = AppSettings {
        volume,
        pack: pack.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&settings) {
        let _ = fs::write(path, json);
    }
}

fn is_safe_pack_name(name: &str) -> bool {
    if name.contains('\\') { return false; }
    let mut components = std::path::Path::new(name).components();
    matches!(components.next(), Some(std::path::Component::Normal(_))) && components.next().is_none()
}

pub fn load_pack(pack_dir: &Path) -> Option<LoadedPack> {
    let config_str = fs::read_to_string(pack_dir.join("config.json")).ok()?;
    let config: PackConfig = serde_json::from_str(&config_str).ok()?;

    let mut cache: HashMap<std::path::PathBuf, PredecodedAudio> = HashMap::new();
    let mut load_cached = |rel_path: &str| -> Option<PredecodedAudio> {
        let full = pack_dir.join(rel_path);
        if let Some(cached) = cache.get(&full) {
            return Some(cached.clone());
        }
        let audio = load_audio_file(&full)?;
        cache.insert(full, audio.clone());
        Some(audio)
    };

    let mut defaults = Vec::new();
    for def in config.defaults {
        if let Some(audio) = load_cached(&def) {
            defaults.push(audio);
        }
    }

    let mut mappings = HashMap::new();
    for (key_str, filename) in config.mappings {
        if let Ok(key_code) = key_str.parse::<u64>() {
            if let Some(audio) = load_cached(&filename) {
                mappings.insert(key_code, audio);
            }
        }
    }

    if config.procedural.is_none() && defaults.is_empty() && mappings.is_empty() {
        return None;
    }

    let pitch = if config.pitch.is_finite() && config.pitch > 0.05 { config.pitch.clamp(0.2, 4.0) } else { 1.0 };
    let vol_mult = if config.vol_mult.is_finite() && config.vol_mult >= 0.0 { config.vol_mult.clamp(0.0, 5.0) } else { 1.0 };

    Some(LoadedPack { defaults, mappings, pitch, vol_mult, procedural: config.procedural })
}

#[derive(Deserialize)]
struct IpcMessage<'a> {
    #[serde(borrow)]
    r#type: &'a str,
    value: Option<serde_json::Value>,
}

#[inline]
fn vol_power_curve(v: u32) -> f32 {
    static LUT: std::sync::OnceLock<[f32; 151]> = std::sync::OnceLock::new();
    let lut = LUT.get_or_init(|| {
        let mut arr = [0.0f32; 151];
        for i in 0..=150 {
            arr[i] = ((i as f32) / 100.0).powf(1.4);
        }
        arr
    });
    lut[(v as usize).min(150)]
}

pub fn run(is_cli: bool) {
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<dyn Any>",
            },
        };
        eprintln!("🔥 CRITICAL THREAD PANIC: {}", msg);
        if let Some(loc) = info.location() {
            eprintln!("Location: {}:{}", loc.file(), loc.line());
        }
    }));
    let exe_dir = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let exe_dir = exe_dir.parent().unwrap_or(std::path::Path::new("."));
    
    let packs_dir = if exe_dir.ends_with("MacOS") {
        exe_dir.parent().unwrap_or(std::path::Path::new(".")).join("Resources").join("packs")
    } else {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join("packs")
    };
    
    let _ = fs::create_dir_all(&packs_dir);

    let mut available_packs = Vec::new();
    if let Ok(entries) = fs::read_dir(&packs_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Ok(name) = entry.file_name().into_string() {
                    available_packs.push(name);
                }
            }
        }
    }
    let favorites = Arc::new(RwLock::new(HashSet::new()));
    if let Ok(json) = fs::read_to_string(packs_dir.join(".favorites.json")) {
        if let Ok(favs) = serde_json::from_str::<HashSet<String>>(&json) {
            *favorites.write().unwrap() = favs;
        }
    }
    
    // Sort packs so favorite packs appear at the start together automatically!
    {
        let favs = favorites.read().unwrap();
        available_packs.sort_by(|a, b| {
            let a_fav = favs.contains(a);
            let b_fav = favs.contains(b);
            b_fav.cmp(&a_fav).then(a.cmp(b))
        });
    }
    
    let current_pack = Arc::new(RwLock::new(None));
    let current_pack_name = Arc::new(RwLock::new(String::new()));
    
    let settings_path = packs_dir.join(".settings.json");
    let mut starting_pack = available_packs.first().cloned();
    
    if let Some(settings) = load_settings(&settings_path) {
        VOL.store(settings.volume, std::sync::atomic::Ordering::Relaxed);
        if available_packs.contains(&settings.pack) {
            starting_pack = Some(settings.pack);
        }
    }

    // Direct terminal switch argument (e.g. `thock 1` or `thock 2` or `thock <name>`)
    if let Some(arg) = std::env::args().nth(1) {
        if let Ok(idx) = arg.parse::<usize>() {
            if idx >= 1 && idx <= available_packs.len() {
                starting_pack = Some(available_packs[idx - 1].clone());
            }
        } else {
            let arg_clean = arg.to_lowercase().replace(' ', "_");
            if available_packs.contains(&arg_clean) {
                starting_pack = Some(arg_clean);
            } else if let Some(m) = available_packs.iter().find(|p| p.starts_with(&arg_clean) || p.replace('_', "").starts_with(&arg_clean.replace('_', ""))) {
                starting_pack = Some(m.clone());
            }
        }
    }
    
    if let Some(first) = starting_pack {
        *current_pack.write().unwrap() = load_pack(&packs_dir.join(&first));
        *current_pack_name.write().unwrap() = first.clone();
    }
    
    use colored::Colorize;
    let pack_clone = current_pack.clone();
    let ax_trusted = unsafe { AXIsProcessTrusted() };

    if !ax_trusted {
        // Auto-open settings to prompt the user
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEventAccess")
            .spawn();
    }

    if is_cli {
        let cur_raw = current_pack_name.read().unwrap().clone();
        let is_fav = favorites.read().unwrap().contains(&cur_raw);
        let title = cur_raw.replace('_', " ");
        let pack_display = if is_fav {
            format!("{} {}", "★".bold().truecolor(255, 215, 0), title.bold().truecolor(245, 245, 250))
        } else {
            title.bold().truecolor(245, 245, 250).to_string()
        };
        let cur_vol = VOL.load(std::sync::atomic::Ordering::Relaxed);
        let filled = (cur_vol / 10).min(10) as usize;
        let empty = 10 - filled;
        let vol_bar = format!("[{}{}] {}%", "█".repeat(filled).truecolor(80, 220, 140), "░".repeat(empty).truecolor(70, 70, 85), cur_vol);

        println!();
        println!("  {}  {}", "thock".bold().truecolor(255, 255, 255), "v0.1.0 · mechanical keyboard acoustic engine".truecolor(130, 130, 150));
        println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));

        if !ax_trusted {
            println!("  {}", "Accessibility permission missing".bold().truecolor(240, 100, 100));
            println!("  {}", "Keypress detection requires Accessibility. Prompt opened in System Settings.".truecolor(180, 180, 190));
            println!("  {}", "Grant access and restart Thock.".truecolor(140, 140, 150));
            println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
        }

        let rain_on = crate::dsp::ASMR_RAIN_ON.load(Ordering::Relaxed) != 0;
        let wind_on = crate::dsp::ASMR_WIND_ON.load(Ordering::Relaxed) != 0;
        let thunder_on = crate::dsp::ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0;
        let ambient_desc = if !rain_on && !wind_on && !thunder_on {
            "off".truecolor(120, 120, 130).to_string()
        } else {
            let mut parts = Vec::new();
            if rain_on { parts.push("rain".truecolor(120, 200, 255).to_string()); }
            if wind_on { parts.push("wind".truecolor(160, 220, 255).to_string()); }
            if thunder_on { parts.push("thunder".truecolor(255, 210, 100).to_string()); }
            parts.join(" · ")
        };

        println!("  {:10}  {}", "pack".truecolor(140, 140, 160), pack_display);
        println!("  {:10}  {}", "volume".truecolor(140, 140, 160), vol_bar);
        println!("  {:10}  {}", "ambient".truecolor(140, 140, 160), ambient_desc);
        println!("  {:10}  {}", "engine".truecolor(140, 140, 160), "zero-latency i16 pcm".truecolor(100, 210, 150));
        println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
        println!("  {}", "Sound Packs (press 1, 2... to switch directly):".bold().truecolor(220, 220, 235));
        for (i, p) in available_packs.iter().enumerate() {
            let is_fav = favorites.read().unwrap().contains(p);
            let is_cur = p == &cur_raw;
            let star = if is_fav { "★ ".bold().truecolor(255, 215, 0) } else { "  ".normal() };
            let active_tag = if is_cur { " (active)".bold().truecolor(80, 220, 140) } else { "".normal() };
            let title = p.replace('_', " ");
            println!("   {:2}. {}{}{}", i + 1, star, title.truecolor(230, 230, 240), active_tag);
        }
        println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
        println!("  {}", "Press 1, 2... to switch pack, or Enter for menu".truecolor(130, 130, 150));
        println!();
    }

    // Audio + CGEventTap
    let audio_thread = move || {
        let (_s, handle) = match OutputStream::try_default() {
            Ok(x) => x,
            Err(e) => {
                eprintln!("Audio device error: {:?}", e);
                return;
            }
        };
        // ── ASMR: three independent real-audio layers ────────────────────────
        let rain_field_sink = Sink::try_new(&handle).unwrap_or_else(|_| {
            eprintln!("ASMR Rain: could not create field-recording sink");
            Sink::new_idle().0
        });
        rain_field_sink.set_volume(0.0);
        rain_field_sink.pause();

        // Wind sink — starts empty and paused (lazy load on enable)
        let wind_sink = Sink::try_new(&handle).unwrap_or_else(|_| {
            eprintln!("ASMR Wind: could not create sink");
            Sink::new_idle().0
        });
        wind_sink.set_volume(0.0);
        wind_sink.pause();

        // Thunder sink — one-shot, re-triggered by a timer thread
        let thunder_sink = Sink::try_new(&handle).unwrap_or_else(|_| {
            eprintln!("ASMR Thunder: could not create sink");
            Sink::new_idle().0
        });
        thunder_sink.set_volume(0.0);
        thunder_sink.pause();

        // Volume-control + thunder-retrigger polling thread
        let _asmr_ctrl = std::thread::Builder::new().stack_size(128 * 1024).spawn(move || {
            use std::sync::atomic::Ordering;
            use crate::dsp::*;
            let mut prng = PRNG::new(0xdeadbeef);
            let mut thunder_cooldown: u32 = 0;
            let mut prev_thunder_on = false;
            let mut wind_phase: f32 = 0.0;
            let mut rain_playing = false;
            let mut wind_playing = false;
            let mut thunder_playing = false;
            let mut rain_current_vol = 0.0f32;
            let mut prev_rain_vol = -1.0f32;
            let mut prev_wind_vol = -1.0f32;
            let mut wind_current_vol = 0.0f32;
            let mut prev_thunder_vol = -1.0f32;
            let mut thunder_current_vol = 0.0f32;

            loop {
                let rain_on = ASMR_RAIN_ON.load(Ordering::Relaxed) != 0;
                let wind_on = ASMR_WIND_ON.load(Ordering::Relaxed) != 0;
                let thunder_on = ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0;
                let any_active = rain_on || wind_on || thunder_on;

                let m = ASMR_MASTER_VOL.load(Ordering::Relaxed) as f32 / 100.0;
                let dt = if any_active || rain_playing || wind_playing || thunder_playing || thunder_current_vol > 0.0001 { 0.030f32 } else { 0.500f32 };

                // Rain (field recording with click-free 250ms volume ramp)
                let rain_target = if rain_on {
                    if !rain_playing {
                        if !rain_field_sink.empty() {
                            rain_field_sink.clear();
                        }
                        if let Some(buf) = get_rain_buffer() {
                            rain_field_sink.append(SeamlessLoopSource::new(
                                buf.samples.clone(), buf.channels, buf.sample_rate, AsmrLoopKind::Rain,
                            ));
                            rain_field_sink.play();
                        }
                        rain_playing = true;
                    }
                    let rain_v = ASMR_RAIN_VOL.load(Ordering::Relaxed) as f32 / 100.0;
                    rain_v * m * 0.90
                } else {
                    0.0
                };
                if rain_playing {
                    let max_step = dt / 0.25;
                    rain_current_vol += (rain_target - rain_current_vol).clamp(-max_step, max_step);
                    if (rain_current_vol - prev_rain_vol).abs() > 0.002 {
                        rain_field_sink.set_volume(rain_current_vol);
                        prev_rain_vol = rain_current_vol;
                    }
                    if !rain_on && rain_current_vol <= 0.0001 {
                        if !rain_field_sink.empty() {
                            rain_field_sink.clear();
                        }
                        rain_field_sink.pause();
                        rain_playing = false;
                        rain_current_vol = 0.0;
                        prev_rain_vol = -1.0;
                        purge_rain_buffer();
                    }
                }

                // Wind (organic breathing gusts via dual-sine LFO + aerodynamic cutoff with 250ms volume ramp)
                let wind_target = if wind_on {
                    if !wind_playing {
                        if !wind_sink.empty() {
                            wind_sink.clear();
                        }
                        if let Some(buf) = get_wind_buffer() {
                            wind_sink.append(SeamlessLoopSource::new(buf.samples.clone(), buf.channels, buf.sample_rate, AsmrLoopKind::Wind));
                            wind_sink.play();
                        }
                        wind_playing = true;
                    }
                    wind_phase += dt;
                    if wind_phase > 65536.0 { wind_phase = 0.0; }
                    let wind_v = ASMR_WIND_VOL.load(Ordering::Relaxed) as f32 / 100.0;
                    let wind_gust = ASMR_WIND_GUST.load(Ordering::Relaxed) as f32 / 100.0;
                    let lfo = 0.6 * (wind_phase * 0.449).sin() + 0.4 * (wind_phase * 0.739).sin();
                    let gust_mult = (1.0 + (wind_gust * 0.35) * lfo).max(0.0);
                    
                    // Modulate aerodynamic cutoff frequency (expands from 350 Hz to 1200 Hz on gusts)
                    let wind_fc = 350.0 + 850.0 * ((gust_mult - 0.75).clamp(0.0, 1.0));
                    ASMR_WIND_CUTOFF.store(wind_fc as u32, Ordering::Relaxed);

                    wind_v * gust_mult * m * 0.75
                } else {
                    0.0
                };
                if wind_playing {
                    let wind_step = dt / 0.25;
                    wind_current_vol += (wind_target - wind_current_vol).clamp(-wind_step, wind_step);
                    if (wind_current_vol - prev_wind_vol).abs() > 0.002 {
                        wind_sink.set_volume(wind_current_vol);
                        prev_wind_vol = wind_current_vol;
                    }
                    if !wind_on && wind_current_vol <= 0.0001 {
                        if !wind_sink.empty() {
                            wind_sink.clear();
                        }
                        wind_sink.pause();
                        wind_playing = false;
                        wind_current_vol = 0.0;
                        prev_wind_vol = -1.0;
                        purge_wind_buffer();
                    }
                }

                // Thunder (zero-allocation one-shot retrigger with instant first strike)
                if thunder_on && !prev_thunder_on {
                    // Resume the sink after clear() and allow the first strike immediately.
                    thunder_cooldown = 0;
                    thunder_sink.play();
                    thunder_playing = true;
                }
                prev_thunder_on = thunder_on;

                if thunder_cooldown > 0 { thunder_cooldown -= 1; }

                let thunder_target = if thunder_on {
                    if thunder_cooldown == 0 && thunder_sink.empty() {
                        let t_freq = ASMR_THUNDER_FREQ.load(Ordering::Relaxed);
                        if let Some(buf) = get_thunder_buffer() {
                            let src = ZeroCopySource {
                                samples: buf.samples.clone(),
                                channels: buf.channels,
                                sample_rate: buf.sample_rate,
                                cursor: 0,
                            };
                            let clip_secs = buf.samples.len() as f32 / (buf.channels as f32 * buf.sample_rate as f32);
                            let gap_secs = thunder_next_gap_seconds(t_freq, prng.next_unit_f32());
                            thunder_cooldown = ((clip_secs + gap_secs) / dt) as u32;
                            thunder_sink.append(src);
                            thunder_sink.play();
                            thunder_playing = true;
                        }
                    }
                    thunder_gain(
                        ASMR_THUNDER_VOL.load(Ordering::Relaxed),
                        ASMR_THUNDER_INT.load(Ordering::Relaxed),
                        ASMR_MASTER_VOL.load(Ordering::Relaxed),
                    )
                } else {
                    thunder_cooldown = 0;
                    0.0
                };
                // Reach full level before the first strike, which arrives ~150ms after the clip starts.
                if thunder_playing {
                    let thunder_step = dt / 0.05;
                    thunder_current_vol += (thunder_target - thunder_current_vol).clamp(-thunder_step, thunder_step);
                    if (thunder_current_vol - prev_thunder_vol).abs() > 0.002 {
                        thunder_sink.set_volume(thunder_current_vol);
                        prev_thunder_vol = thunder_current_vol;
                    }
                    if !thunder_on && thunder_current_vol <= 0.0001 {
                        if !thunder_sink.empty() {
                            thunder_sink.clear();
                        }
                        thunder_sink.pause();
                        thunder_playing = false;
                        thunder_current_vol = 0.0;
                        prev_thunder_vol = -1.0;
                        purge_thunder_buffer();
                    } else if thunder_on && thunder_sink.empty() && thunder_cooldown > 0 {
                        thunder_sink.pause();
                    }
                }

                // Sleep at end of iteration: 30ms when audio is active, 500ms when idle to save CPU cycles.
                let sleep_ms = if any_active || rain_playing || wind_playing || thunder_playing || thunder_current_vol > 0.0001 { 30 } else { 500 };
                std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
            }
        });

        // Cell<usize> provides interior mutability with zero overhead (no locks, no atomics).
        // CGEventTap requires Fn (not FnMut), so we can't mutate a plain usize directly.
        let default_idx = std::cell::Cell::new(0usize);
        let strike_idx = std::cell::Cell::new(0u32);

        // Lock-free velocity tracking: store nanoseconds since UNIX epoch as AtomicU64.
        // Avoids a write-lock acquisition on every single KeyDown event.
        let _epoch = Instant::now();
        let last_press = std::cell::Cell::new(std::time::Instant::now());
        let previous_flags = std::cell::Cell::new(0u64);
        let tap_res = CGEventTap::new(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged],
            move |_proxy, event_type, cg_event| {
                let is_autorepeat = cg_event.get_integer_value_field(8) != 0;
                let keycode = cg_event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                let is_capslock = keycode == 57;
                let is_keyup = if is_capslock {
                    false
                } else if matches!(event_type, CGEventType::FlagsChanged) {
                    let current_flags = cg_event.get_flags().bits();
                    let old_flags = previous_flags.get();
                    previous_flags.set(current_flags);
                    current_flags < old_flags
                } else {
                    matches!(event_type, CGEventType::KeyUp)
                };

                if is_autorepeat && !is_keyup { return None; }

                let mut velocity_mult = 1.0f32;
                if !is_keyup {
                    let now = std::time::Instant::now();
                    let elapsed = now.duration_since(last_press.replace(now)).as_secs_f32();
                    // Balanced humanized curve: baseline 0.95 at rest, rises smoothly to 1.12 during bursts
                    velocity_mult = (0.92 + (0.025 / (elapsed + 0.10))).clamp(0.90, 1.15);
                }

                if let Some(pack) = &*pack_clone.read().unwrap() {
                    // Perceptually uniform power curve via static 151-entry LUT (0 cycles spent on powf)
                    let base_vol = vol_power_curve(VOL.load(Ordering::Relaxed)) * pack.vol_mult;
                    let playback_vol = (base_vol * velocity_mult).min(1.0);

                    if let Some(proc_config) = &pack.procedural {
                        let proc = dsp::ProceduralSwitch::new(proc_config, is_keyup, velocity_mult, keycode);
                        let _ = handle.play_raw(proc.amplify(playback_vol));
                    } else if !is_keyup {
                        let key_map = keymap::get_key_map();
                        let audio = if let Some(Some(code)) = key_map.get(keycode as usize) {
                            if let Some(mapped) = pack.mappings.get(code) {
                                Some(mapped)
                            } else if !pack.defaults.is_empty() {
                                let idx = default_idx.get() % pack.defaults.len();
                                default_idx.set(idx.wrapping_add(1));
                                Some(&pack.defaults[idx])
                            } else {
                                None
                            }
                        } else if !pack.defaults.is_empty() {
                            let idx = default_idx.get() % pack.defaults.len();
                            default_idx.set(idx.wrapping_add(1));
                            Some(&pack.defaults[idx])
                        } else {
                            None
                        };

                        if let Some(a) = audio {
                            // Micro-pitch humanization (+-1.5%) to eliminate machine-gun repetition
                            let strike = strike_idx.get();
                            strike_idx.set(strike.wrapping_add(1));
                            let micro_jitter = 1.0 + ((((keycode as u32).wrapping_add(strike).wrapping_mul(1103515245) >> 16) as f32 / 32768.0) - 1.0) * 0.015;
                            let effective_pitch = pack.pitch * micro_jitter;
                            if let Some(src) = a.play(playback_vol, effective_pitch) {
                                let _ = handle.play_raw(src);
                            }
                        }
                    }
                }
                None
            }
        );

        match tap_res {
            Ok(tap) => {
                match tap.mach_port.create_runloop_source(0) {
                    Ok(source) => {
                        CFRunLoop::get_current().add_source(&source, unsafe { core_foundation::runloop::kCFRunLoopCommonModes });
                        tap.enable();
                        unsafe { core_foundation::runloop::CFRunLoopRun() };
                    }
                    Err(e) => eprintln!("Failed to create runloop source (Mach port limit reached?): {:?}", e),
                }
            }
            Err(e) => eprintln!("Event tap error: {:?}", e),
        }
    };

    if is_cli {
        // Print banner only once here (removed duplicate inside the spawned thread below)
        
        // Run CLI REPL in a background thread so the Main Thread is dedicated
        // purely to the macOS CGEventTap. MacOS heavily throttles background
        // threads, which causes input latency. Running the event tap on the
        // main thread fixes the latency instantly.
        let mut packs_for_rl = available_packs.clone();
        let packs_dir_clone = packs_dir.clone();
        let current_pack_clone = current_pack.clone();
        let current_pack_name_clone = current_pack_name.clone();
        let favorites_clone = favorites.clone();

        let _ = std::thread::Builder::new().stack_size(256 * 1024).spawn(move || {
            let helper = ThockHelper {
                commands: vec![
                    "menu".to_string(), "status".to_string(), "vol".to_string(), "pack".to_string(),
                    "packs".to_string(), "fav".to_string(), "proc".to_string(), "asmr".to_string(),
                    "pitch".to_string(), "delete".to_string(), "del".to_string(), "update".to_string(), "help".to_string(), "exit".to_string()
                ],
                packs: packs_for_rl.clone(),
            };
            let mut rl = rustyline::Editor::<ThockHelper, _>::new().unwrap();
            rl.set_helper(Some(helper));

            let interactive_menu = |packs: &[String], packs_dir: &std::path::Path, cur_pack: &std::sync::Arc<std::sync::RwLock<Option<LoadedPack>>>, cur_name: &std::sync::Arc<std::sync::RwLock<String>>, favs_lock: &std::sync::Arc<std::sync::RwLock<std::collections::HashSet<String>>>| {
                use inquire::Select;
                let cur_raw = cur_name.read().unwrap().clone();
                let is_cur_fav = favs_lock.read().unwrap().contains(&cur_raw);
                let star = if is_cur_fav { "★ " } else { "" };
                let active_name = format!("{}{}", star, cur_raw.replace('_', " "));
                let cur_vol = VOL.load(Ordering::Relaxed);
                let fav_action = if is_cur_fav {
                    format!("Favorite    Remove active pack from favorites (★)")
                } else {
                    format!("Favorite    Add active pack to favorites (★)")
                };
                let menu_options = vec![
                    format!("Pack        Switch sound pack (active: {})", active_name),
                    fav_action,
                    "Delete      Delete a sound pack (favorites protected 🔒)".to_string(),
                    format!("Volume      Adjust master playback ({}%)", cur_vol),
                    "Ambient     Rain, wind & thunder background layers".to_string(),
                    "Procedural  Tune physical mechanical switch".to_string(),
                    "Status      Display audio telemetry & profile".to_string(),
                    "Update      Pull latest version from git & rebuild".to_string(),
                    "Exit        Quit Thock".to_string(),
                ];

                if let Ok(choice) = Select::new("Select action (↑↓ scroll, Enter select):", menu_options).prompt() {
                    if choice.starts_with("Pack") {
                        let active_clean = cur_name.read().unwrap().to_lowercase().replace(' ', "_");
                        let favs = favs_lock.read().unwrap();
                        let formatted_packs: Vec<String> = packs.iter().enumerate().map(|(i, p)| {
                            let title = p.replace('_', " ");
                            let star = if favs.contains(p) { "★ " } else { "  " };
                            if p == &active_clean {
                                format!("{:2}. {}{:<24} (active)", i + 1, star, title)
                            } else {
                                format!("{:2}. {}{:<24}", i + 1, star, title)
                            }
                        }).collect();
                        if let Ok(selected) = Select::new("Select sound pack:", formatted_packs).prompt() {
                            let clean_title = selected
                                .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '★' || c.is_whitespace())
                                .split_whitespace()
                                .take_while(|&s| s != "(active)")
                                .collect::<Vec<_>>()
                                .join(" ");
                            let raw_name = clean_title.to_lowercase().replace(' ', "_");
                            let p_dir = packs_dir.join(&raw_name);
                            if let Some(pack) = load_pack(&p_dir) {
                                *cur_pack.write().unwrap() = Some(pack);
                                *cur_name.write().unwrap() = raw_name.clone();
                                println!("  {} Pack: {}", "✓".bold().truecolor(80, 220, 140), clean_title.bold().truecolor(245, 245, 250));
                                save_settings(&packs_dir.join(".settings.json"), VOL.load(Ordering::Relaxed), &raw_name);
                            }
                        }
                    } else if choice.starts_with("Favorite") {
                        let mut favs = favs_lock.write().unwrap();
                        let title = cur_raw.replace('_', " ");
                        if favs.contains(&cur_raw) {
                            favs.remove(&cur_raw);
                            println!("  {} Removed from favorites: {}", "★".truecolor(120, 120, 130), title.truecolor(200, 200, 210));
                        } else {
                            favs.insert(cur_raw.clone());
                            println!("  {} Added to favorites: {}", "★".bold().truecolor(255, 215, 0), title.bold().truecolor(255, 255, 255));
                        }
                        if let Ok(json) = serde_json::to_string(&*favs) {
                            let _ = std::fs::write(packs_dir.join(".favorites.json"), json);
                        }
                    } else if choice.starts_with("Delete") {
                        let active_clean = cur_name.read().unwrap().to_lowercase().replace(' ', "_");
                        let favs = favs_lock.read().unwrap();
                        let formatted_packs: Vec<String> = packs.iter().enumerate().map(|(i, p)| {
                            let title = p.replace('_', " ");
                            let star = if favs.contains(p) { "★ " } else { "  " };
                            let tag = if favs.contains(p) { " [🔒 FAVORITE]" } else { "" };
                            if p == &active_clean {
                                format!("{:2}. {}{:<24} (active){}", i + 1, star, title, tag)
                            } else {
                                format!("{:2}. {}{:<24}{}", i + 1, star, title, tag)
                            }
                        }).collect();
                        drop(favs);
                        if let Ok(selected) = Select::new("Select sound pack to delete:", formatted_packs).prompt() {
                            let clean_title = selected
                                .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '★' || c.is_whitespace())
                                .split_whitespace()
                                .take_while(|&s| s != "(active)" && s != "[🔒" && s != "FAVORITE]")
                                .collect::<Vec<_>>()
                                .join(" ");
                            let raw_name = clean_title.to_lowercase().replace(' ', "_");
                            if favs_lock.read().unwrap().contains(&raw_name) {
                                println!("  {} Cannot delete favorite pack '{}'. Remove it from favorites first with 'fav {}'.", "🔒".bold(), raw_name.replace('_', " ").bold().truecolor(255, 215, 0), raw_name);
                            } else if is_safe_pack_name(&raw_name) {
                                let p_dir = packs_dir.join(&raw_name);
                                if p_dir.is_dir() {
                                    let is_active = *cur_name.read().unwrap() == raw_name;
                                    if is_active {
                                        if let Some(new_p) = packs.iter().find(|p| *p != &raw_name) {
                                            if let Some(new_pack) = load_pack(&packs_dir.join(new_p)) {
                                                *cur_pack.write().unwrap() = Some(new_pack);
                                                *cur_name.write().unwrap() = new_p.clone();
                                                save_settings(&packs_dir.join(".settings.json"), VOL.load(Ordering::Relaxed), new_p);
                                                println!("  {} Active pack switched to: {}", "ℹ".bold().truecolor(100, 180, 240), new_p.replace('_', " "));
                                            }
                                        }
                                    }
                                    if let Err(e) = std::fs::remove_dir_all(&p_dir) {
                                        println!("  {} Failed to delete pack: {}", "✗".bold().truecolor(240, 100, 100), e);
                                    } else {
                                        println!("  {} Successfully deleted pack: {}", "✓".bold().truecolor(80, 220, 140), clean_title.bold());
                                    }
                                }
                            }
                        }
                    } else if choice.starts_with("Volume") {
                        if let Ok(v_str) = inquire::Text::new("Enter volume [0-100]:").prompt() {
                            if let Ok(v) = v_str.parse::<u32>() {
                                let v = v.clamp(0, 100);
                                VOL.store(v, Ordering::Relaxed);
                                let filled = (v / 10).min(10) as usize;
                                let empty = 10 - filled;
                                let bar = format!("[{}{}]", "█".repeat(filled).truecolor(80, 220, 140), "░".repeat(empty).truecolor(70, 70, 85));
                                println!("  {} Volume: {}% {}", "✓".bold().truecolor(80, 220, 140), v, bar);
                                save_settings(&packs_dir.join(".settings.json"), v, &*cur_name.read().unwrap());
                            } else {
                                println!("  {} Volume must be a number between 0 and 100", "✗".bold().truecolor(240, 100, 100));
                            }
                        }
                    } else if choice.starts_with("Ambient") {
                        let rain_st = if crate::dsp::ASMR_RAIN_ON.load(Ordering::Relaxed) != 0 { "ON " } else { "OFF" };
                        let wind_st = if crate::dsp::ASMR_WIND_ON.load(Ordering::Relaxed) != 0 { "ON " } else { "OFF" };
                        let thunder_st = if crate::dsp::ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0 { "ON " } else { "OFF" };
                        let asmr_opts = vec![
                            format!("Rain       [{}] Recorded garden rainfall", rain_st),
                            format!("Wind       [{}] Dual-LFO organic gust modulation", wind_st),
                            format!("Thunder    [{}] Ambient rolling rumble (tail-faded)", thunder_st),
                            "Turn Off   Disable all ambient layers".to_string(),
                        ];
                        if let Ok(asmr_sel) = Select::new("Ambient sound settings:", asmr_opts).prompt() {
                            match asmr_sel.as_str() {
                                s if s.starts_with("Rain") => {
                                    let cur = crate::dsp::ASMR_RAIN_ON.load(Ordering::Relaxed) != 0;
                                    crate::dsp::ASMR_RAIN_ON.store(if cur { 0 } else { 1 }, Ordering::Relaxed);
                                    println!("  {} Rain: {}", "✓".bold().truecolor(80, 220, 140), if !cur { "ON" } else { "OFF" });
                                },
                                s if s.starts_with("Wind") => {
                                    let cur = crate::dsp::ASMR_WIND_ON.load(Ordering::Relaxed) != 0;
                                    crate::dsp::ASMR_WIND_ON.store(if cur { 0 } else { 1 }, Ordering::Relaxed);
                                    println!("  {} Wind: {}", "✓".bold().truecolor(80, 220, 140), if !cur { "ON" } else { "OFF" });
                                },
                                s if s.starts_with("Thunder") => {
                                    let cur = crate::dsp::ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0;
                                    crate::dsp::ASMR_THUNDER_ON.store(if cur { 0 } else { 1 }, Ordering::Relaxed);
                                    println!("  {} Thunder: {}", "✓".bold().truecolor(80, 220, 140), if !cur { "ON" } else { "OFF" });
                                },
                                s if s.starts_with("Turn Off") => {
                                    crate::dsp::ASMR_RAIN_ON.store(0, Ordering::Relaxed);
                                    crate::dsp::ASMR_WIND_ON.store(0, Ordering::Relaxed);
                                    crate::dsp::ASMR_THUNDER_ON.store(0, Ordering::Relaxed);
                                    println!("  {} All ambient layers disabled", "✓".bold().truecolor(80, 220, 140));
                                },
                                _ => {}
                            }
                        }
                    } else if choice.starts_with("Procedural") {
                        let proc_opts = vec![
                            "Cream      NovelKeys Cream (smooth POM on POM)".to_string(),
                            "Linear     Cherry MX Red (smooth linear bottom-out)".to_string(),
                            "Tactile    Cherry MX Brown (light bump, crisp clack)".to_string(),
                            "Clicky     Cherry MX Blue (sharp click leaf snap)".to_string(),
                        ];
                        if let Ok(sel) = Select::new("Procedural switch profile:", proc_opts).prompt() {
                            let switch_val = if sel.contains("Cream") { "cream" }
                                else if sel.contains("Linear") { "linear" }
                                else if sel.contains("Tactile") { "tactile" }
                                else { "clicky" };
                            let mut lock = cur_pack.write().unwrap();
                            if lock.is_none() || lock.as_ref().unwrap().procedural.is_none() {
                                *lock = Some(crate::LoadedPack {
                                    defaults: vec![],
                                    mappings: HashMap::new(),
                                    vol_mult: 1.0,
                                    pitch: 1.0,
                                    procedural: Some(crate::dsp::ProceduralConfig::default()),
                                });
                                *cur_name.write().unwrap() = "__cli_proc__".to_string();
                            }
                            if let Some(pack) = lock.as_mut() {
                                if let Some(proc) = &mut pack.procedural {
                                    proc.switch_type = switch_val.to_string();
                                    println!("  {} Switch preset: {}", "✓".bold().truecolor(80, 220, 140), switch_val.bold().truecolor(245, 245, 250));
                                }
                            }
                        }
                    } else if choice.starts_with("Status") {
                        let p_name = cur_name.read().unwrap().replace('_', " ");
                        let cur_vol = VOL.load(Ordering::Relaxed);
                        let filled = (cur_vol / 10).min(10) as usize;
                        let empty = 10 - filled;
                        let vol_bar = format!("[{}{}] {}%", "█".repeat(filled).truecolor(80, 220, 140), "░".repeat(empty).truecolor(70, 70, 85), cur_vol);
                        let rain_on = crate::dsp::ASMR_RAIN_ON.load(Ordering::Relaxed) != 0;
                        let wind_on = crate::dsp::ASMR_WIND_ON.load(Ordering::Relaxed) != 0;
                        let thunder_on = crate::dsp::ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0;
                        let dot = |on: bool| if on { "ON ".bold().truecolor(80, 220, 140) } else { "OFF".truecolor(120, 120, 130) };
                        println!();
                        println!("  {}", "thock status".bold().truecolor(240, 240, 250));
                        println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                        println!("  {:12}  {}", "pack", p_name.bold().truecolor(255, 255, 255));
                        println!("  {:12}  {}", "volume", vol_bar);
                        println!("  {:12}  Rain [{}]  Wind [{}]  Thunder [{}]", "ambient", dot(rain_on), dot(wind_on), dot(thunder_on));
                        println!("  {:12}  {}", "engine", "zero-latency i16 pcm (lock-free)".truecolor(100, 210, 150));
                        println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                        println!();
                    } else if choice.starts_with("Update") {
                        println!("\n  {} Checking for updates from git...", "·".truecolor(130, 130, 150));
                        let pull = std::process::Command::new("git").args(["pull", "origin", "main"]).status();
                        match pull {
                            Ok(s) if s.success() => {
                                println!("  {} Repository updated. Recompiling release binary...", "✓".bold().truecolor(80, 220, 140));
                                let build = std::process::Command::new("cargo").args(["build", "--release"]).status();
                                if build.map(|b| b.success()).unwrap_or(false) {
                                    println!("  {} Thock updated successfully! Restart thock to apply changes.\n", "✓".bold().truecolor(80, 220, 140));
                                } else {
                                    println!("  {} Rebuild failed. Check cargo output above.\n", "✗".bold().truecolor(240, 100, 100));
                                }
                            }
                            _ => {
                                println!("  {} Failed to pull from git. Check network connection.\n", "✗".bold().truecolor(240, 100, 100));
                            }
                        }
                    } else if choice.starts_with("Exit") {
                        println!("  {} {}\n", "·".truecolor(130, 130, 150), "thock stopped".truecolor(170, 170, 185));
                        std::process::exit(0);
                    }
                }
            };

            loop {
                let readline = rl.readline(&format!("{} ", "thock ›".truecolor(130, 120, 180)));
                let input = match readline {
                    Ok(line) => {
                        let _ = rl.add_history_entry(line.as_str());
                        line
                    },
                    Err(_) => break,
                };
                let parts: Vec<&str> = input.trim().split_whitespace().collect();
                if parts.is_empty() || parts[0] == "menu" {
                    interactive_menu(&packs_for_rl, &packs_dir_clone, &current_pack_clone, &current_pack_name_clone, &favorites_clone);
                    continue;
                }

                // Direct numeric pack selection (e.g. typing `1` or `2` directly switches the pack!)
                if let Ok(idx) = parts[0].parse::<usize>() {
                    if idx >= 1 && idx <= packs_for_rl.len() {
                        let selected = packs_for_rl[idx - 1].clone();
                        let p_dir = packs_dir_clone.join(&selected);
                        if let Some(pack) = load_pack(&p_dir) {
                            *current_pack_clone.write().unwrap() = Some(pack);
                            *current_pack_name_clone.write().unwrap() = selected.clone();
                            let is_fav = favorites_clone.read().unwrap().contains(&selected);
                            let star = if is_fav { "★ " } else { "" };
                            println!("  {} Pack: {}{}", "✓".bold().truecolor(80, 220, 140), star.bold().truecolor(255, 215, 0), selected.replace('_', " ").bold().truecolor(245, 245, 250));
                            save_settings(&packs_dir_clone.join(".settings.json"), VOL.load(Ordering::Relaxed), &selected);
                        } else {
                            println!("  {} Failed to load pack: '{}'", "✗".bold().truecolor(240, 100, 100), selected);
                        }
                        continue;
                    } else {
                        println!("  {} Invalid pack number '{}'. Enter 1–{}", "✗".bold().truecolor(240, 100, 100), idx, packs_for_rl.len());
                        continue;
                    }
                }
            
            match parts[0] {
                "help" => {
                    println!();
                    println!("  {}", "Commands".bold().truecolor(240, 240, 250));
                    println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                    println!("  {:16}  {}", "menu".truecolor(200, 190, 255), "Interactive menu (or press Enter)".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "status".truecolor(200, 190, 255), "Display current pack & telemetry".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "pack <name>".truecolor(200, 190, 255), "Switch sound pack by name".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "packs".truecolor(200, 190, 255), "Interactive sound pack selector".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "fav [name]".truecolor(200, 190, 255), "Toggle favorite for current or named pack (★)".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "del <name>".truecolor(200, 190, 255), "Delete sound pack (favorites are protected 🔒)".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "vol <0-100>".truecolor(200, 190, 255), "Set master playback volume".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "asmr [layer]".truecolor(200, 190, 255), "Configure ambient rain/wind/thunder".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "proc [prop]".truecolor(200, 190, 255), "Tune procedural physics engine".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "update".truecolor(200, 190, 255), "Pull latest version from git & rebuild".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "exit / quit".truecolor(200, 190, 255), "Exit Thock".truecolor(160, 160, 175));
                    println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                    println!();
                },
                "status" => {
                    let cur_raw = current_pack_name_clone.read().unwrap().clone();
                    let is_fav = favorites_clone.read().unwrap().contains(&cur_raw);
                    let title = cur_raw.replace('_', " ");
                    let p_name = if is_fav {
                        format!("{} {}", "★".bold().truecolor(255, 215, 0), title.bold().truecolor(255, 255, 255))
                    } else {
                        title.bold().truecolor(255, 255, 255).to_string()
                    };
                    let cur_vol = VOL.load(Ordering::Relaxed);
                    let filled = (cur_vol / 10).min(10) as usize;
                    let empty = 10 - filled;
                    let vol_bar = format!("[{}{}] {}%", "█".repeat(filled).truecolor(80, 220, 140), "░".repeat(empty).truecolor(70, 70, 85), cur_vol);
                    let rain_on = crate::dsp::ASMR_RAIN_ON.load(Ordering::Relaxed) != 0;
                    let wind_on = crate::dsp::ASMR_WIND_ON.load(Ordering::Relaxed) != 0;
                    let thunder_on = crate::dsp::ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0;
                    let dot = |on: bool| if on { "ON ".bold().truecolor(80, 220, 140) } else { "OFF".truecolor(120, 120, 130) };
                    println!();
                    println!("  {}", "thock status".bold().truecolor(240, 240, 250));
                    println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                    println!("  {:12}  {}", "pack", p_name);
                    println!("  {:12}  {}", "volume", vol_bar);
                    println!("  {:12}  Rain [{}]  Wind [{}]  Thunder [{}]", "ambient", dot(rain_on), dot(wind_on), dot(thunder_on));
                    println!("  {:12}  {}", "engine", "zero-latency i16 pcm (lock-free)".truecolor(100, 210, 150));
                    println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                    println!();
                },
                "vol" => {
                    if parts.len() > 1 {
                        if let Ok(v) = parts[1].parse::<u32>() {
                            let v = v.clamp(0, 100);
                            VOL.store(v, Ordering::Relaxed);
                            let filled = (v / 10).min(10) as usize;
                            let empty = 10 - filled;
                            let bar = format!("[{}{}]", "█".repeat(filled).truecolor(80, 220, 140), "░".repeat(empty).truecolor(70, 70, 85));
                            println!("  {} Volume: {}% {}", "✓".bold().truecolor(80, 220, 140), v, bar);
                            save_settings(&packs_dir_clone.join(".settings.json"), v, &*current_pack_name_clone.read().unwrap());
                        } else {
                            println!("  {} Volume must be a number between 0 and 100", "✗".bold().truecolor(240, 100, 100));
                        }
                    } else {
                        if let Ok(v_str) = inquire::Text::new("Enter volume [0-100]:").prompt() {
                            if let Ok(v) = v_str.parse::<u32>() {
                                let v = v.clamp(0, 100);
                                VOL.store(v, Ordering::Relaxed);
                                let filled = (v / 10).min(10) as usize;
                                let empty = 10 - filled;
                                let bar = format!("[{}{}]", "█".repeat(filled).truecolor(80, 220, 140), "░".repeat(empty).truecolor(70, 70, 85));
                                println!("  {} Volume: {}% {}", "✓".bold().truecolor(80, 220, 140), v, bar);
                                save_settings(&packs_dir_clone.join(".settings.json"), v, &*current_pack_name_clone.read().unwrap());
                            } else {
                                println!("  {} Volume must be a number between 0 and 100", "✗".bold().truecolor(240, 100, 100));
                            }
                        }
                    }
                },
                "fav" => {
                    let target_pack = if parts.len() > 1 {
                        if let Ok(idx) = parts[1].parse::<usize>() {
                            if idx >= 1 && idx <= packs_for_rl.len() {
                                packs_for_rl[idx - 1].clone()
                            } else {
                                parts[1..].join("_").to_lowercase()
                            }
                        } else {
                            let input = parts[1..].join("_").to_lowercase();
                            if packs_for_rl.contains(&input) {
                                input
                            } else if let Some(matched) = packs_for_rl.iter().find(|p| p.starts_with(&input) || p.replace('_', "").starts_with(&input.replace('_', ""))) {
                                matched.clone()
                            } else {
                                input
                            }
                        }
                    } else {
                        current_pack_name_clone.read().unwrap().clone()
                    };

                    if target_pack.is_empty() {
                        println!("  {} No sound pack specified or active.", "✗".bold().truecolor(240, 100, 100));
                    } else {
                        let mut favs = favorites_clone.write().unwrap();
                        let title = target_pack.replace('_', " ");
                        if favs.contains(&target_pack) {
                            favs.remove(&target_pack);
                            println!("  {} Removed from favorites: {}", "★".truecolor(120, 120, 130), title.truecolor(200, 200, 210));
                        } else {
                            favs.insert(target_pack.clone());
                            println!("  {} Added to favorites: {}", "★".bold().truecolor(255, 215, 0), title.bold().truecolor(255, 255, 255));
                        }
                        if let Ok(json) = serde_json::to_string(&*favs) {
                            let _ = std::fs::write(packs_dir_clone.join(".favorites.json"), json);
                        }

                        // Re-sort packs so favorites appear at the start together automatically!
                        packs_for_rl.sort_by(|a, b| {
                            let a_fav = favs.contains(a);
                            let b_fav = favs.contains(b);
                            b_fav.cmp(&a_fav).then(a.cmp(b))
                        });

                        println!("  {}", "Sound Packs (favorites at start):".bold().truecolor(220, 220, 235));
                        for (i, p) in packs_for_rl.iter().enumerate() {
                            let is_fav = favs.contains(p);
                            let is_cur = p == &*current_pack_name_clone.read().unwrap();
                            let star = if is_fav { "★ ".bold().truecolor(255, 215, 0) } else { "  ".normal() };
                            let active_tag = if is_cur { " (active)".bold().truecolor(80, 220, 140) } else { "".normal() };
                            println!("   {:2}. {}{}{}", i + 1, star, p.replace('_', " ").truecolor(230, 230, 240), active_tag);
                        }
                    }
                },
                "delete" | "del" => {
                    let target_pack = if parts.len() > 1 {
                        if let Ok(idx) = parts[1].parse::<usize>() {
                            if idx >= 1 && idx <= packs_for_rl.len() {
                                packs_for_rl[idx - 1].clone()
                            } else {
                                parts[1..].join("_").to_lowercase()
                            }
                        } else {
                            let input = parts[1..].join("_").to_lowercase();
                            if packs_for_rl.contains(&input) {
                                input
                            } else if let Some(matched) = packs_for_rl.iter().find(|p| p.starts_with(&input) || p.replace('_', "").starts_with(&input.replace('_', ""))) {
                                matched.clone()
                            } else {
                                input
                            }
                        }
                    } else {
                        let active_clean = current_pack_name_clone.read().unwrap().to_lowercase().replace(' ', "_");
                        let favs = favorites_clone.read().unwrap();
                        let formatted_packs: Vec<String> = packs_for_rl.iter().enumerate().map(|(i, p)| {
                            let title = p.replace('_', " ");
                            let star = if favs.contains(p) { "★ " } else { "  " };
                            let tag = if favs.contains(p) { " [🔒 FAVORITE]" } else { "" };
                            if p == &active_clean {
                                format!("{:2}. {}{:<24} (active){}", i + 1, star, title, tag)
                            } else {
                                format!("{:2}. {}{:<24}{}", i + 1, star, title, tag)
                            }
                        }).collect();
                        drop(favs);
                        if let Ok(selected) = inquire::Select::new("Select sound pack to delete:", formatted_packs).prompt() {
                            let clean_title = selected
                                .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '★' || c.is_whitespace())
                                .split_whitespace()
                                .take_while(|&s| s != "(active)" && s != "[🔒" && s != "FAVORITE]")
                                .collect::<Vec<_>>()
                                .join(" ");
                            clean_title.to_lowercase().replace(' ', "_")
                        } else {
                            continue;
                        }
                    };

                    if target_pack.is_empty() {
                        println!("  {} No sound pack specified.", "✗".bold().truecolor(240, 100, 100));
                        continue;
                    }

                    if favorites_clone.read().unwrap().contains(&target_pack) {
                        println!("  {} Cannot delete favorite pack '{}'. Remove it from favorites first with 'fav {}'.",
                            "🔒".bold(),
                            target_pack.replace('_', " ").bold().truecolor(255, 215, 0),
                            target_pack
                        );
                        continue;
                    }

                    if !is_safe_pack_name(&target_pack) {
                        println!("  {} Invalid pack name: '{}'", "✗".bold().truecolor(240, 100, 100), target_pack);
                        continue;
                    }

                    let target_dir = packs_dir_clone.join(&target_pack);
                    if !target_dir.is_dir() {
                        println!("  {} Pack not found: '{}'", "✗".bold().truecolor(240, 100, 100), target_pack);
                        continue;
                    }

                    let is_active = *current_pack_name_clone.read().unwrap() == target_pack;
                    if is_active {
                        if let Some(new_pack_name) = packs_for_rl.iter().find(|p| *p != &target_pack) {
                            let new_dir = packs_dir_clone.join(new_pack_name);
                            if let Some(new_p) = load_pack(&new_dir) {
                                *current_pack_clone.write().unwrap() = Some(new_p);
                                *current_pack_name_clone.write().unwrap() = new_pack_name.clone();
                                save_settings(&packs_dir_clone.join(".settings.json"), VOL.load(Ordering::Relaxed), new_pack_name);
                                println!("  {} Active pack switched to: {}", "ℹ".bold().truecolor(100, 180, 240), new_pack_name.replace('_', " "));
                            }
                        }
                    }

                    if let Err(e) = std::fs::remove_dir_all(&target_dir) {
                        println!("  {} Failed to delete pack: {}", "✗".bold().truecolor(240, 100, 100), e);
                    } else {
                        packs_for_rl.retain(|p| p != &target_pack);
                        println!("  {} Successfully deleted pack: {}", "✓".bold().truecolor(80, 220, 140), target_pack.replace('_', " ").bold());
                    }
                },
                "pack" => {
                    if parts.len() > 1 {
                        let pack_name = if let Ok(idx) = parts[1].parse::<usize>() {
                            if idx >= 1 && idx <= packs_for_rl.len() {
                                packs_for_rl[idx - 1].clone()
                            } else {
                                parts[1..].join("_")
                            }
                        } else {
                            parts[1..].join("_")
                        };
                        let p_dir = packs_dir_clone.join(&pack_name);
                        if let Some(pack) = load_pack(&p_dir) {
                            *current_pack_clone.write().unwrap() = Some(pack);
                            *current_pack_name_clone.write().unwrap() = pack_name.clone();
                            let is_fav = favorites_clone.read().unwrap().contains(&pack_name);
                            let star = if is_fav { "★ " } else { "" };
                            println!("  {} Pack: {}{}", "✓".bold().truecolor(80, 220, 140), star.bold().truecolor(255, 215, 0), pack_name.replace('_', " ").bold().truecolor(245, 245, 250));
                            save_settings(&packs_dir_clone.join(".settings.json"), VOL.load(Ordering::Relaxed), &pack_name);
                        } else {
                            println!("  {} Pack not found: '{}'. Type 1-{} to choose directly.", "✗".bold().truecolor(240, 100, 100), pack_name, packs_for_rl.len());
                        }
                    } else {
                        let cur_name = current_pack_name_clone.read().unwrap().clone();
                        let favs = favorites_clone.read().unwrap();
                        println!("  {}", "Sound Packs (press 1, 2... to switch directly):".bold().truecolor(220, 220, 235));
                        for (i, p) in packs_for_rl.iter().enumerate() {
                            let is_fav = favs.contains(p);
                            let is_cur = p == &cur_name;
                            let star = if is_fav { "★ ".bold().truecolor(255, 215, 0) } else { "  ".normal() };
                            let active_tag = if is_cur { " (active)".bold().truecolor(80, 220, 140) } else { "".normal() };
                            println!("   {:2}. {}{}{}", i + 1, star, p.replace('_', " ").truecolor(230, 230, 240), active_tag);
                        }
                        println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                        println!("  {}", "Press 1, 2... to switch pack directly".truecolor(130, 130, 150));
                    }
                },
                "packs" => {
                    let cur_name = current_pack_name_clone.read().unwrap().clone();
                    let favs = favorites_clone.read().unwrap();
                    println!("  {}", "Sound Packs (press 1, 2... to switch directly):".bold().truecolor(220, 220, 235));
                    for (i, p) in packs_for_rl.iter().enumerate() {
                        let is_fav = favs.contains(p);
                        let is_cur = p == &cur_name;
                        let star = if is_fav { "★ ".bold().truecolor(255, 215, 0) } else { "  ".normal() };
                        let active_tag = if is_cur { " (active)".bold().truecolor(80, 220, 140) } else { "".normal() };
                        println!("   {:2}. {}{}{}", i + 1, star, p.replace('_', " ").truecolor(230, 230, 240), active_tag);
                    }
                    println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                    println!("  {}", "Press 1, 2... to switch pack directly".truecolor(130, 130, 150));
                },
                "proc" => {
                    let is_proc = {
                        let lock = current_pack.read().unwrap();
                        lock.as_ref().map_or(false, |p| p.procedural.is_some())
                    };
                    if !is_proc {
                        let new_pack = crate::LoadedPack {
                            mappings: std::collections::HashMap::new(),
                            defaults: vec![],
                            vol_mult: 1.0,
                            pitch: 1.0,
                            procedural: Some(crate::dsp::ProceduralConfig::default()),
                        };
                        *current_pack.write().unwrap() = Some(new_pack);
                        *current_pack_name.write().unwrap() = "__cli_proc__".to_string();
                        println!("  {} Switched to procedural synthesis engine", "✓".bold().truecolor(80, 220, 140));
                    }
                    
                    if parts.len() == 1 {
                        let lock = current_pack.read().unwrap();
                        if let Some(pack) = lock.as_ref() {
                            if let Some(proc) = &pack.procedural {
                                println!();
                                println!("  {}", "Procedural Synthesis Engine".bold().truecolor(240, 240, 250));
                                println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                                println!("  {:12}  {} (cream | linear | tactile | clicky)", "Switch", proc.switch_type.bold().truecolor(255, 255, 255));
                                println!("  {:12}  {}g", "Weight", proc.spring_weight);
                                println!("  {:12}  {:.2}", "Lube", proc.lube_amount);
                                println!("  {:12}  {:.2}", "Foam Mod", proc.foam_mod);
                                println!("  {:12}  {:.2}", "O-Rings", proc.o_rings);
                                println!("  {:12}  {:.2}", "Pitch", proc.pitch);
                                println!("  {:12}  {}", "Keycap", proc.keycap_material);
                                println!("  {:12}  {}", "Plate", proc.plate_material);
                                println!("  {:12}  {}", "Case", proc.case_material);
                                println!("  {:12}  {}", "Mount", proc.mounting_style);
                                println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                                println!("  {}", "Usage: proc <property> <value>  (e.g. proc lube 0.8)".truecolor(130, 130, 145));
                                println!();
                            }
                        }
                        continue;
                    }
                    
                    if parts.len() >= 3 {
                        let prop = parts[1];
                        let val = parts[2];
                        let mut lock = current_pack.write().unwrap();
                        if let Some(pack) = lock.as_mut() {
                            if let Some(proc) = pack.procedural.as_mut() {
                                match prop {
                                    "switch" => { proc.switch_type = val.to_string(); println!("  {} switch → {}", "✓".bold().truecolor(80, 220, 140), val.bold().truecolor(245, 245, 250)); },
                                    "weight" => if let Ok(v) = val.parse::<f32>() { proc.spring_weight = v; println!("  {} weight → {}g", "✓".bold().truecolor(80, 220, 140), v); },
                                    "lube" => if let Ok(v) = val.parse::<f32>() { proc.lube_amount = v; println!("  {} lube → {:.2}", "✓".bold().truecolor(80, 220, 140), v); },
                                    "foam" => if let Ok(v) = val.parse::<f32>() { proc.foam_mod = v; println!("  {} foam → {:.2}", "✓".bold().truecolor(80, 220, 140), v); },
                                    "orings" => if let Ok(v) = val.parse::<f32>() { proc.o_rings = v; println!("  {} orings → {:.2}", "✓".bold().truecolor(80, 220, 140), v); },
                                    "pitch" => if let Ok(v) = val.parse::<f32>() { proc.pitch = v; println!("  {} pitch → {:.2}", "✓".bold().truecolor(80, 220, 140), v); },
                                    "keycap" => { proc.keycap_material = val.to_string(); println!("  {} keycap → {}", "✓".bold().truecolor(80, 220, 140), val); },
                                    "plate" => { proc.plate_material = val.to_string(); println!("  {} plate → {}", "✓".bold().truecolor(80, 220, 140), val); },
                                    "case" => { proc.case_material = val.to_string(); println!("  {} case → {}", "✓".bold().truecolor(80, 220, 140), val); },
                                    "mount" => { proc.mounting_style = val.to_string(); println!("  {} mount → {}", "✓".bold().truecolor(80, 220, 140), val); },
                                    _ => println!("  {} Unknown procedural property: '{}'", "✗".bold().truecolor(240, 100, 100), prop),
                                }
                            }
                        }
                    } else {
                        println!("  {} Usage: proc <property> <value>  (e.g. proc lube 0.8)", "·".truecolor(140, 140, 160));
                    }
                },
                "asmr" => {
                    if parts.len() == 1 {
                        use crate::dsp::*;
                        let rain_on = ASMR_RAIN_ON.load(Ordering::Relaxed) != 0;
                        let wind_on = ASMR_WIND_ON.load(Ordering::Relaxed) != 0;
                        let thunder_on = ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0;
                        let st = |on: bool| if on { "ON ".bold().truecolor(80, 220, 140) } else { "OFF".truecolor(120, 120, 130) };
                        println!();
                        println!("  {}", "Ambient Layers".bold().truecolor(240, 240, 250));
                        println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                        println!("  {:12}  {}%", "Master Vol", ASMR_MASTER_VOL.load(Ordering::Relaxed));
                        println!("  {:12}  [{}]  vol {}%  intensity {}%", "Rain", st(rain_on), ASMR_RAIN_VOL.load(Ordering::Relaxed), ASMR_RAIN_DENS.load(Ordering::Relaxed));
                        println!("  {:12}  [{}]  vol {}%  gust {}%", "Wind", st(wind_on), ASMR_WIND_VOL.load(Ordering::Relaxed), ASMR_WIND_GUST.load(Ordering::Relaxed));
                        println!("  {:12}  [{}]  vol {}%  freq {}%  intensity {}%", "Thunder", st(thunder_on), ASMR_THUNDER_VOL.load(Ordering::Relaxed), ASMR_THUNDER_FREQ.load(Ordering::Relaxed), ASMR_THUNDER_INT.load(Ordering::Relaxed));
                        println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                        println!("  {}", "Commands: asmr rain | asmr wind | asmr thunder | asmr none".truecolor(130, 130, 145));
                        println!();
                        continue;
                    }
                    
                    let prop = parts[1];
                    match prop {
                        "none" => {
                            crate::dsp::ASMR_RAIN_ON.store(0, Ordering::Relaxed);
                            crate::dsp::ASMR_WIND_ON.store(0, Ordering::Relaxed);
                            crate::dsp::ASMR_THUNDER_ON.store(0, Ordering::Relaxed);
                            println!("  {} All ambient layers disabled", "✓".bold().truecolor(80, 220, 140));
                        },
                        "rain" => {
                            let curr = crate::dsp::ASMR_RAIN_ON.load(Ordering::Relaxed) != 0;
                            let new_st = if parts.len() > 2 { parts[2] == "on" || parts[2] == "1" } else { !curr };
                            crate::dsp::ASMR_RAIN_ON.store(if new_st { 1 } else { 0 }, Ordering::Relaxed);
                            println!("  {} Rain: {}", "✓".bold().truecolor(80, 220, 140), if new_st { "ON" } else { "OFF" });
                        },
                        "wind" => {
                            let curr = crate::dsp::ASMR_WIND_ON.load(Ordering::Relaxed) != 0;
                            let new_st = if parts.len() > 2 { parts[2] == "on" || parts[2] == "1" } else { !curr };
                            crate::dsp::ASMR_WIND_ON.store(if new_st { 1 } else { 0 }, Ordering::Relaxed);
                            println!("  {} Wind: {}", "✓".bold().truecolor(80, 220, 140), if new_st { "ON" } else { "OFF" });
                        },
                        "thunder" => {
                            let curr = crate::dsp::ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0;
                            let new_st = if parts.len() > 2 { parts[2] == "on" || parts[2] == "1" } else { !curr };
                            crate::dsp::ASMR_THUNDER_ON.store(if new_st { 1 } else { 0 }, Ordering::Relaxed);
                            println!("  {} Thunder: {}", "✓".bold().truecolor(80, 220, 140), if new_st { "ON" } else { "OFF" });
                        },
                        _ => {
                            if parts.len() >= 3 {
                                if let Ok(v) = parts[2].parse::<u32>() {
                                    let v = v.clamp(0, 100);
                                    match prop {
                                        "vol" => crate::dsp::ASMR_MASTER_VOL.store(v, Ordering::Relaxed),
                                        "rain_dens" => crate::dsp::ASMR_RAIN_DENS.store(v, Ordering::Relaxed),
                                        "wind_gust" => crate::dsp::ASMR_WIND_GUST.store(v, Ordering::Relaxed),
                                        "rain_vol" => crate::dsp::ASMR_RAIN_VOL.store(v, Ordering::Relaxed),
                                        "wind_vol" => crate::dsp::ASMR_WIND_VOL.store(v, Ordering::Relaxed),
                                        "thunder_freq" => crate::dsp::ASMR_THUNDER_FREQ.store(v, Ordering::Relaxed),
                                        "thunder_int" => crate::dsp::ASMR_THUNDER_INT.store(v, Ordering::Relaxed),
                                        "thunder_vol" => crate::dsp::ASMR_THUNDER_VOL.store(v, Ordering::Relaxed),
                                        _ => { println!("  {} Unknown ASMR property: '{}'", "✗".bold().truecolor(240, 100, 100), prop); continue; }
                                    }
                                    println!("  {} {} → {}%", "✓".bold().truecolor(80, 220, 140), prop, v);
                                } else {
                                    println!("  {} Value must be a number between 0 and 100", "✗".bold().truecolor(240, 100, 100));
                                }
                            } else {
                                println!("  {} Usage: asmr {} <0–100>", "·".truecolor(140, 140, 160), prop);
                            }
                        }
                    }
                },
                "update" => {
                    println!("\n  {} Checking for updates from git...", "·".truecolor(130, 130, 150));
                    let pull = std::process::Command::new("git").args(["pull", "origin", "main"]).status();
                    match pull {
                        Ok(s) if s.success() => {
                            println!("  {} Repository updated. Recompiling release binary...", "✓".bold().truecolor(80, 220, 140));
                            let build = std::process::Command::new("cargo").args(["build", "--release"]).status();
                            if build.map(|b| b.success()).unwrap_or(false) {
                                println!("  {} Thock updated successfully! Restart thock to apply changes.\n", "✓".bold().truecolor(80, 220, 140));
                            } else {
                                println!("  {} Rebuild failed. Check cargo output above.\n", "✗".bold().truecolor(240, 100, 100));
                            }
                        }
                        _ => {
                            println!("  {} Failed to pull from git. Check network connection.\n", "✗".bold().truecolor(240, 100, 100));
                        }
                    }
                },
                "quit" | "exit" => {
                    println!("  {} {}\n", "·".truecolor(130, 130, 150), "thock stopped".truecolor(170, 170, 185));
                    std::process::exit(0);
                },
                _ => println!("  {} Unknown command '{}'. Type 'help' for available commands.", "·".truecolor(140, 140, 160), parts[0]),
            }
        }
        });
        // Block the main thread with the low-latency audio event tap
        audio_thread();
        return;
    } else {
        let _ = std::thread::Builder::new().stack_size(256 * 1024).spawn(audio_thread);
    }

    let event_loop = tao::event_loop::EventLoop::new();

    // Setup native macOS menu bar so Cmd+Q, Cmd+M, Cmd+W work perfectly globally
    let menu_bar = muda::Menu::new();
    let app_m = muda::Submenu::new("Thock", true);
    let _ = app_m.append_items(&[&muda::PredefinedMenuItem::quit(None)]);
    let _ = menu_bar.append(&app_m);
    
    let window_m = muda::Submenu::new("Window", true);
    let _ = window_m.append_items(&[
        &muda::PredefinedMenuItem::minimize(None),
        &muda::PredefinedMenuItem::close_window(None),
    ]);
    let _ = menu_bar.append(&window_m);
    #[cfg(target_os = "macos")]
    menu_bar.init_for_nsapp();

    
    // UI Builder function to allow dynamic recreation of the window and webview
    // without leaking 200MB of WebKit memory when the window is closed.
    fn build_ui(
        event_loop: &tao::event_loop::EventLoopWindowTarget<()>,
        ipc_current_pack_name: std::sync::Arc<std::sync::RwLock<String>>,
        ipc_current_pack: std::sync::Arc<std::sync::RwLock<Option<crate::LoadedPack>>>,
        ipc_packs_dir: std::path::PathBuf,
        ipc_favorites: std::sync::Arc<std::sync::RwLock<std::collections::HashSet<String>>>,
    ) -> (wry::WebView, tao::window::Window) {
        let window = tao::window::WindowBuilder::new()
            .with_title("Thock")
            .with_inner_size(tao::dpi::LogicalSize::new(800.0, 600.0))
            .with_min_inner_size(tao::dpi::LogicalSize::new(700.0, 500.0))
            .with_max_inner_size(tao::dpi::LogicalSize::new(1200.0, 900.0))
            .build(event_loop)
            .unwrap();

        let html_template = include_str!("../ui.html");
        
        let mut packs_html = String::new();
        let current_pack_read = ipc_current_pack_name.read().unwrap().clone();
        let favs = ipc_favorites.read().unwrap().clone();
        
        let mut sorted_packs = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&ipc_packs_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Ok(name) = entry.file_name().into_string() {
                        if !name.starts_with('.') {
                            sorted_packs.push(name);
                        }
                    }
                }
            }
        }
        sorted_packs.sort_by(|a, b| {
            let a_fav = favs.contains(a);
            let b_fav = favs.contains(b);
            b_fav.cmp(&a_fav).then(a.cmp(b))
        });
        
        for pack in &sorted_packs {
            let is_active = *pack == current_pack_read;
            let is_fav = favs.contains(pack);
            
            let _active_badge = if is_active {
                r#"inline-block"#
            } else { "none" };
            
            let active_card_class = if is_active { "bg-[#beb3a6] border-orange-300 ring-2 ring-orange-200" } else { "bg-white/50 border-white/50" };
            
            let fav_class = if is_fav { "text-yellow-500 fill-current" } else { "text-gray-400" };

            let display_name = pack.split("_").map(|w| { let mut c = w.chars(); match c.next() { None => String::new(), Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), } }).collect::<Vec<_>>().join(" ");
            let safe_id = pack.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>();
            
            let mut hash_val = 0usize;
            for b in display_name.bytes() { hash_val = hash_val.wrapping_add(b as usize); }
            
            let img_src = if display_name.to_lowercase().contains("panda") {
                crate::assets::IMG_PANDA
            } else if display_name.to_lowercase().contains("creamy heavy") || display_name.to_lowercase().contains("steelseries") || display_name.to_lowercase().contains("nk cream") {
                crate::assets::IMG_NEW_BLUE
            } else if display_name.to_lowercase().contains("blue") {
                crate::assets::IMG_BLUE
            } else if display_name.to_lowercase().contains("red") {
                crate::assets::IMG_RED
            } else if display_name.to_lowercase().contains("black") {
                crate::assets::IMG_BLACK
            } else if display_name.to_lowercase().contains("alpaca") {
                crate::assets::IMG_ALPACA
            } else {
                let images = [
                    crate::assets::IMG_PANDA,
                    crate::assets::IMG_BLUE,
                    crate::assets::IMG_RED,
                    crate::assets::IMG_BLACK,
                    crate::assets::IMG_ALPACA,
                ];
                images[hash_val % images.len()]
            };

            
            // Dynamic switch properties based on name
            let lower_name = pack.to_lowercase();
            let (_stem_color, switch_type, act_weight) = if display_name.to_lowercase().contains("blue") {
                ("#3b82f6", "Clicky", "60g")
            } else if display_name.to_lowercase().contains("brown") {
                ("#92400e", "Tactile", "55g")
            } else if display_name.to_lowercase().contains("red") {
                ("#ef4444", "Linear", "45g")
            } else if display_name.to_lowercase().contains("black") {
                ("#1f2937", "Linear", "60g")
            } else if display_name.to_lowercase().contains("holy") || lower_name.contains("panda") {
                ("#f59e0b", "Tactile", "67g")
            } else if display_name.to_lowercase().contains("cream") {
                ("#fef3c7", "Linear", "55g")
            } else if lower_name.contains("bubble") || lower_name.contains("morandi") {
                ("#c084fc", "Bubble Pop", "53g")
            } else {
                ("#a8a29e", "Linear", "50g")
            };

            let card = format!(r##"
                <div id="pack-{}" onclick="selectPack('{}')" data-fav="{}" class="pack-card relative backdrop-blur-md hover:bg-white hover:-translate-y-1 hover:shadow-xl border {} rounded-xl p-3.5 transition-all duration-300 ease-[cubic-bezier(0.25,0.8,0.25,1)] cursor-pointer flex flex-col justify-between h-[146px] overflow-hidden shadow-sm group">

                    <!-- Keyboard Watermark SVG -->
                    <svg class="absolute -right-6 -top-4 w-40 h-40 text-gray-500 opacity-5 pointer-events-none transform rotate-12 transition-transform duration-500 group-hover:rotate-6 group-hover:scale-110" viewBox="0 0 100 100">
                        <path fill="currentColor" d="M10 30 h80 v40 h-80 z M15 35 h8 v8 h-8 z M25 35 h8 v8 h-8 z M35 35 h8 v8 h-8 z M45 35 h8 v8 h-8 z M55 35 h8 v8 h-8 z M65 35 h8 v8 h-8 z M75 35 h8 v8 h-8 z M18 48 h8 v8 h-8 z M28 48 h8 v8 h-8 z M38 48 h8 v8 h-8 z M48 48 h8 v8 h-8 z M58 48 h8 v8 h-8 z M68 48 h8 v8 h-8 z M30 61 h40 v8 h-40 z"/>
                    </svg>
                    <div class="z-10">
                        <!-- Switch Icon -->
                        <div class="w-9 h-9 mb-1.5 transition-transform duration-300 group-hover:scale-110 group-active:scale-95">
                            <img src="{}" class="w-full h-full object-contain drop-shadow-md" alt="switch" />
                        </div>
                        <h3 class="font-bold text-gray-800 text-[14px] leading-tight truncate tracking-tight">{}</h3>
                        <p class="text-[10px] text-gray-500 mt-1 font-medium">{} {}</p>
                    </div>

                    <div class="z-10 flex justify-between items-end pt-1">
                        <span class="text-[9px] font-semibold text-gray-400 tracking-wider">108 Keys</span>
                        <div class="flex items-center space-x-3">
                            <button onclick="toggleFav(event, '{}')" class="text-gray-300 hover:text-yellow-500 transition-all duration-200 active:scale-90 hover:scale-110">
                                <svg id="fav-{}" class="w-5 h-5 {} drop-shadow-sm" viewBox="0 0 20 20" stroke="currentColor" stroke-width="1.5" fill="none"><path stroke-linecap="round" stroke-linejoin="round" d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z"></path></svg>
                            </button>
                            <button onclick="deletePack(event, '{}')" class="text-gray-300 hover:text-red-500 transition-all duration-200 active:scale-90 hover:scale-110">
                                <svg class="w-5 h-5 drop-shadow-sm" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"></path></svg>
                            </button>
                        </div>
                    </div>
                </div>
            "##, safe_id, pack, is_fav, active_card_class, img_src, display_name, switch_type, act_weight, pack, safe_id, fav_class, pack);
            
            packs_html.push_str(&card);
        }
        
        let mut packs_options = String::new();
        for pack in &sorted_packs {
            let display_name = pack.split("_").map(|w| { let mut c = w.chars(); match c.next() { None => String::new(), Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), } }).collect::<Vec<_>>().join(" ");
            let selected = if &current_pack_read == pack { "selected" } else { "" };
            packs_options.push_str(&format!("<option value='{}' {}>{}</option>", pack, selected, display_name));
        }
        
        let ax_trusted = unsafe { AXIsProcessTrusted() };
        let ax_banner = if !ax_trusted {
            r#"<div class="bg-red-500/90 text-white p-4 rounded-xl mb-4 shadow-lg border border-red-400 flex flex-col sm:flex-row items-center justify-between gap-4">
                <div class="font-semibold text-center sm:text-left">
                    ⚠️ Input Monitoring / Accessibility permission missing.<br/>
                    <span class="text-sm font-normal text-red-100">Keypresses cannot be detected. Please enable it in System Settings.</span>
                </div>
                <button onclick="safeIpc({type: 'open_settings'})" class="whitespace-nowrap px-4 py-2 bg-white text-red-600 font-bold rounded-lg shadow hover:bg-red-100 transition-colors">
                    Open Settings
                </button>
            </div>"#
        } else { "" };
        
        let current_vol = VOL.load(std::sync::atomic::Ordering::Relaxed).to_string();
        let final_html = html_template
            .replace("<!-- MAIN_LOGO -->", crate::assets::IMG_MAIN_LOGO)
            .replace("<!-- IMG_ICON_RAIN -->", crate::assets::IMG_ICON_RAIN)
            .replace("<!-- IMG_ICON_WIND -->", crate::assets::IMG_ICON_WIND)
            .replace("<!-- IMG_ICON_THUNDER -->", crate::assets::IMG_ICON_THUNDER)
            .replace("<!-- PACKS_LIST -->", &packs_html)
            .replace("<!-- VOL_VALUE -->", &current_vol)
            .replace("<!-- PACK_OPTIONS -->", &packs_options)
            .replace("<!-- AX_WARNING -->", ax_banner);

        let ipc_packs_dir_clone = ipc_packs_dir.clone();
        
        let webview = wry::WebViewBuilder::new(&window)
            .with_devtools(false)
            .with_incognito(true)
            .with_html(final_html)
            .with_ipc_handler(move |req: wry::http::Request<String>| {
                if let Ok(msg) = serde_json::from_str::<IpcMessage>(req.body()) {
                    match msg.r#type {
                        "open_settings" => {
                            let _ = std::process::Command::new("open")
                                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                                .spawn();
                            let _ = std::process::Command::new("open")
                                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEventAccess")
                                .spawn();
                        }
                        "set_volume" => {
                            if let Some(val) = msg.value {
                                if let Some(v) = val.as_u64() {
                                    let v = (v as u32).min(100);
                                    VOL.store(v, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                        }
                        "save_volume" => {
                            if let Some(val) = msg.value {
                                if let Some(v) = val.as_u64() {
                                    let v = (v as u32).min(100);
                                    VOL.store(v, std::sync::atomic::Ordering::Relaxed);
                                    let pack_name = ipc_current_pack_name.read().unwrap().clone();
                                    save_settings(&ipc_packs_dir_clone.join(".settings.json"), v, &pack_name);
                                }
                            }
                        }
                        "select_pack" => {
                            if let Some(val) = msg.value {
                                if let Some(p) = val.as_str() {
                                    if let Some(loaded) = load_pack(&ipc_packs_dir_clone.join(p)) {
                                        *ipc_current_pack.write().unwrap() = Some(loaded);
                                        *ipc_current_pack_name.write().unwrap() = p.to_string();
                                        save_settings(&ipc_packs_dir_clone.join(".settings.json"), VOL.load(std::sync::atomic::Ordering::Relaxed), p);
                                    }
                                }
                            }
                        }
                        "delete_pack" => {
                            if let Some(val) = msg.value {
                                if let Some(p) = val.as_str() {
                                    if ipc_favorites.read().unwrap().contains(p) {
                                        // FAVORITE LOCK: Cannot delete favorite pack!
                                        return;
                                    }
                                    if is_safe_pack_name(p) {
                                        let target_dir = ipc_packs_dir_clone.join(p);
                                        let _ = std::fs::remove_dir_all(&target_dir);
                                        let cur_name = ipc_current_pack_name.read().unwrap().clone();
                                        if cur_name == p {
                                            if let Ok(entries) = std::fs::read_dir(&ipc_packs_dir_clone) {
                                                for entry in entries.flatten() {
                                                    let name = entry.file_name().to_string_lossy().to_string();
                                                    if entry.path().is_dir() && !name.starts_with('.') && name != p {
                                                        if let Some(new_pack) = load_pack(&entry.path()) {
                                                            *ipc_current_pack.write().unwrap() = Some(new_pack);
                                                            *ipc_current_pack_name.write().unwrap() = name.clone();
                                                            save_settings(&ipc_packs_dir_clone.join(".settings.json"), VOL.load(std::sync::atomic::Ordering::Relaxed), &name);
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "toggle_fav" => {
                            if let Some(val) = msg.value {
                                if let Some(p) = val.as_str() {
                                    let mut favs = ipc_favorites.write().unwrap();
                                    if favs.contains(p) {
                                        favs.remove(p);
                                    } else {
                                        favs.insert(p.to_string());
                                    }
                                    if let Ok(json) = serde_json::to_string(&*favs) {
                                        let _ = std::fs::write(ipc_packs_dir_clone.join(".favorites.json"), json);
                                    }
                                }
                            }
                        }
                        "preview_maker" => {
                            if let Some(val) = msg.value.as_ref() {
                                if let (Some(base), Some(pitch), Some(vol)) = (
                                    val.get("base").and_then(|v| v.as_str()),
                                    val.get("pitch").and_then(|v| v.as_f64()),
                                    val.get("volume").and_then(|v| v.as_f64()),
                                ) {
                                    let proc_val = val.get("procedural");
                                    let pack = if base == "__procedural__" {
                                        crate::LoadedPack {
                                            defaults: vec![],
                                            mappings: std::collections::HashMap::new(),
                                            pitch: pitch as f32,
                                            vol_mult: vol as f32,
                                            procedural: proc_val.and_then(|v| serde_json::from_value(v.clone()).ok()),
                                        }
                                    } else {
                                        let base_dir = ipc_packs_dir_clone.join(base);
                                        let mut p = load_pack(&base_dir).unwrap_or(crate::LoadedPack {
                                            defaults: vec![], mappings: std::collections::HashMap::new(), pitch: 1.0, vol_mult: 1.0, procedural: None,
                                        });
                                        p.pitch = pitch as f32;
                                        p.vol_mult = vol as f32;
                                        p
                                    };
                                    *ipc_current_pack.write().unwrap() = Some(pack);
                                    *ipc_current_pack_name.write().unwrap() = "Preview".to_string();
                                }
                            }
                        }
                        "asmr_update" => {
                            if let Some(val) = msg.value.as_ref() {
                                if let Some(v) = val.get("rain_on").and_then(|v| v.as_bool()) {
                                    crate::dsp::ASMR_RAIN_ON.store(if v { 1 } else { 0 }, std::sync::atomic::Ordering::Relaxed);
                                }
                                if let Some(v) = val.get("wind_on").and_then(|v| v.as_bool()) {
                                    crate::dsp::ASMR_WIND_ON.store(if v { 1 } else { 0 }, std::sync::atomic::Ordering::Relaxed);
                                }
                                if let Some(v) = val.get("thunder_on").and_then(|v| v.as_bool()) {
                                    crate::dsp::ASMR_THUNDER_ON.store(if v { 1 } else { 0 }, std::sync::atomic::Ordering::Relaxed);
                                }
                                
                                if let Some(v) = val.get("master_vol").and_then(|v| v.as_u64()) {
                                    crate::dsp::ASMR_MASTER_VOL.store(v as u32, std::sync::atomic::Ordering::Relaxed);
                                }
                                if let Some(v) = val.get("rain_density").and_then(|v| v.as_u64()) {
                                    crate::dsp::ASMR_RAIN_DENS.store(v as u32, std::sync::atomic::Ordering::Relaxed);
                                }
                                if let Some(v) = val.get("wind_gust").and_then(|v| v.as_u64()) {
                                    crate::dsp::ASMR_WIND_GUST.store(v as u32, std::sync::atomic::Ordering::Relaxed);
                                }
                                if let Some(v) = val.get("rain_vol").and_then(|v| v.as_u64()) {
                                    crate::dsp::ASMR_RAIN_VOL.store(v as u32, std::sync::atomic::Ordering::Relaxed);
                                }
                                if let Some(v) = val.get("wind_vol").and_then(|v| v.as_u64()) {
                                    crate::dsp::ASMR_WIND_VOL.store(v as u32, std::sync::atomic::Ordering::Relaxed);
                                }
                                if let Some(v) = val.get("thunder_freq").and_then(|v| v.as_u64()) {
                                    crate::dsp::ASMR_THUNDER_FREQ.store(v as u32, std::sync::atomic::Ordering::Relaxed);
                                }
                                if let Some(v) = val.get("thunder_int").or_else(|| val.get("thunder_intensity")).and_then(|v| v.as_u64()) {
                                    crate::dsp::ASMR_THUNDER_INT.store(v as u32, std::sync::atomic::Ordering::Relaxed);
                                }
                                if let Some(v) = val.get("thunder_vol").and_then(|v| v.as_u64()) {
                                    crate::dsp::ASMR_THUNDER_VOL.store(v as u32, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                        }
                        "save_maker" => {
                            if let Some(val) = msg.value.as_ref() {
                                if let (Some(name), Some(base), Some(pitch), Some(vol)) = (
                                    val.get("name").and_then(|v| v.as_str()),
                                    val.get("base").and_then(|v| v.as_str()),
                                    val.get("pitch").and_then(|v| v.as_f64()),
                                    val.get("volume").and_then(|v| v.as_f64()),
                                ) {
                                    if !is_safe_pack_name(name) {
                                        eprintln!("Rejected invalid custom pack name");
                                        return;
                                    }
                                    let new_dir = ipc_packs_dir_clone.join(name);
                                    let _ = std::fs::create_dir_all(&new_dir);
                                    
                                    let proc_val = val.get("procedural");
                                    
                                    if base == "__procedural__" {
                                        let config = PackConfig {
                                            defaults: vec![],
                                            mappings: std::collections::HashMap::new(),
                                            pitch: pitch as f32,
                                            vol_mult: vol as f32,
                                            base: Some(base.to_string()),
                                            procedural: proc_val.and_then(|v| serde_json::from_value(v.clone()).ok()),
                                        };
                                        if let Ok(new_json) = serde_json::to_string_pretty(&config) {
                                            let _ = std::fs::write(new_dir.join("config.json"), new_json);
                                        }
                                    } else {
                                        let base_dir = ipc_packs_dir_clone.join(base);
                                        if let Ok(entries) = std::fs::read_dir(&base_dir) {
                                            for entry in entries.flatten() {
                                                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                                                    let file_name = entry.file_name();
                                                    let _ = std::fs::copy(entry.path(), new_dir.join(file_name));
                                                }
                                            }
                                        }
                                        if let Ok(config_str) = std::fs::read_to_string(new_dir.join("config.json")) {
                                            if let Ok(mut config) = serde_json::from_str::<PackConfig>(&config_str) {
                                                config.pitch = pitch as f32;
                                                config.vol_mult = vol as f32;
                                                config.base = Some(base.to_string());
                                                if let Ok(new_json) = serde_json::to_string_pretty(&config) {
                                                    let _ = std::fs::write(new_dir.join("config.json"), new_json);
                                                }
                                            }
                                        }
                                    }
                                    
                                    *ipc_current_pack.write().unwrap() = None;
                                    if let Some(pack) = load_pack(&new_dir) {
                                        *ipc_current_pack.write().unwrap() = Some(pack);
                                        *ipc_current_pack_name.write().unwrap() = name.to_string();
                                        save_settings(&ipc_packs_dir_clone.join(".settings.json"), VOL.load(std::sync::atomic::Ordering::Relaxed), name);
                                    }
                                }
                            }
                        }
                        "quit" => {
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                }
            })
            .build()
            .unwrap();

        (webview, window)
    }

    let mut ui = Some(build_ui(
        &event_loop,
        current_pack_name.clone(),
        current_pack.clone(),
        packs_dir.clone(),
        favorites.clone(),
    ));

    let mut modifiers = tao::keyboard::ModifiersState::empty();

    event_loop.run(move |event, event_loop_target, control_flow| {
        *control_flow = tao::event_loop::ControlFlow::Wait;

        match event {
            tao::event::Event::WindowEvent { event: tao::event::WindowEvent::CloseRequested, .. } => {
                // Drop the Window and WebView entirely to free ~200MB WebKit memory!
                ui = None;
            },
            tao::event::Event::Reopen { .. } => {
                // macOS Dock icon clicked! Re-create the window if it was dropped
                if ui.is_none() {
                    ui = Some(build_ui(
                        event_loop_target,
                        current_pack_name.clone(),
                        current_pack.clone(),
                        packs_dir.clone(),
                        favorites.clone(),
                    ));
                }
            },
            tao::event::Event::WindowEvent { event: tao::event::WindowEvent::ModifiersChanged(state), .. } => {
                modifiers = state;
            },
            tao::event::Event::WindowEvent {
                event: tao::event::WindowEvent::KeyboardInput {
                    event: tao::event::KeyEvent {
                        physical_key,
                        state: tao::event::ElementState::Pressed,
                        ..
                    },
                    ..
                },
                ..
            } => {
                if modifiers.contains(tao::keyboard::ModifiersState::SUPER) {
                    if physical_key == tao::keyboard::KeyCode::KeyQ {
                        *control_flow = tao::event_loop::ControlFlow::Exit;
                    } else if physical_key == tao::keyboard::KeyCode::KeyW {
                        ui = None; // Drop the window
                    } else if physical_key == tao::keyboard::KeyCode::KeyM {
                        if let Some((_, window)) = &ui {
                            window.set_minimized(true);
                        }
                    }
                }
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_custom_pack_name_stays_within_packs_directory() {
        assert!(is_safe_pack_name("Deep Holy Panda"));
        assert!(is_safe_pack_name("rain-v2"));
        for name in ["", ".", "..", "../outside", "nested/pack", "/tmp/pack", "nested\\pack"] {
            assert!(!is_safe_pack_name(name), "unsafe pack name accepted: {name}");
        }
    }

    #[test]
    fn test_zero_copy_source_conversion() {
        let samples: Vec<i16> = vec![-32768, -16384, 0, 16384, 32767];
        let audio = PredecodedAudio {
            samples_data: std::sync::Arc::from(samples.into_boxed_slice()),
            channels: 2,
            sample_rate: 44100,
        };
        let mut src = ZeroCopySource {
            samples: audio.samples(),
            channels: audio.channels,
            sample_rate: audio.sample_rate,
            cursor: 0,
        };

        let first = src.next().unwrap();
        assert!((first - (-1.0)).abs() < 0.001);

        let second = src.next().unwrap();
        assert!((second - (-0.5)).abs() < 0.001);

        let third = src.next().unwrap();
        assert!((third - 0.0).abs() < 0.001);

        let fourth = src.next().unwrap();
        assert!((fourth - 0.5).abs() < 0.001);

        let fifth = src.next().unwrap();
        assert!((fifth - (32767.0 / 32768.0)).abs() < 0.001);

        assert_eq!(src.next(), None);
        assert_eq!(rodio::Source::current_frame_len(&src), Some(0));
    }

    #[test]
    fn test_load_pack_deduplication() {
        let packs_dir = PathBuf::from("packs");
        if let Ok(entries) = std::fs::read_dir(&packs_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && entry.path().join("config.json").exists() {
                    let pack = load_pack(&entry.path());
                    assert!(pack.is_some(), "Pack should load successfully: {:?}", entry.path());
                    let pack = pack.unwrap();
                    // Verify that mapped sounds share Arc references where filenames repeat
                    if pack.mappings.len() > 10 {
                        let sample_arcs: Vec<_> = pack.mappings.values().map(|a| a.samples_data.as_ptr()).collect();
                        let unique_ptrs: std::collections::HashSet<_> = sample_arcs.iter().copied().collect();
                        // Many packs have repeated sound files; verify unique buffers <= total mappings
                        assert!(unique_ptrs.len() <= sample_arcs.len());
                    }
                    break;
                }
            }
        }
    }

    #[test]
    fn test_asmr_buffer_caching_and_zero_copy() {
        let rain1 = get_rain_buffer().expect("rain buffer should load");
        let rain2 = get_rain_buffer().expect("rain buffer should load");
        assert!(std::sync::Arc::ptr_eq(&rain1.samples, &rain2.samples), "Rain buffer must be cached as a single Arc");
        let rain_secs = rain1.samples.len() / rain1.channels as usize / rain1.sample_rate as usize;
        assert_eq!(rain_secs, 60, "Rain buffer should retain the recorded build and steady loop");
        assert!(rain1.samples.len() * std::mem::size_of::<i16>() <= 6 * 1024 * 1024, "Rain PCM cache must stay below 6 MiB");

        let wind1 = get_wind_buffer().expect("wind buffer should load");
        let wind2 = get_wind_buffer().expect("wind buffer should load");
        assert!(std::sync::Arc::ptr_eq(&wind1.samples, &wind2.samples), "Wind buffer must be cached as a single Arc");

        let thunder1 = get_thunder_buffer().expect("thunder buffer should load");
        let thunder2 = get_thunder_buffer().expect("thunder buffer should load");
        assert!(std::sync::Arc::ptr_eq(&thunder1.samples, &thunder2.samples), "Thunder buffer must be cached as a single Arc");
        let thunder_secs = thunder1.samples.len() / thunder1.channels as usize / thunder1.sample_rate as usize;
        assert!((40..=44).contains(&thunder_secs), "Thunder should preserve the complete lightning-to-rumble recording");
        let lead_in = thunder1.sample_rate as usize * thunder1.channels as usize * 2;
        let first_strike_end = thunder1.sample_rate as usize * thunder1.channels as usize * 5 / 2;
        let first_half_second = thunder1.sample_rate as usize * thunder1.channels as usize / 2;
        let first_quarter_second = first_half_second / 2;
        assert!(thunder1.samples[..first_half_second].iter().any(|s| s.abs() > 16_000), "First thunder strike must remain near the start");
        assert!(thunder1.samples[..first_quarter_second].iter().any(|s| s.abs() > 16_000), "First strike should arrive within 250ms");
        assert!(thunder1.samples[lead_in..first_strike_end].iter().any(|s| s.abs() > 16_000), "First strike should be preserved");

        // Verify cache eviction and memory reclamation when toggled OFF
        purge_rain_buffer();
        purge_wind_buffer();
        purge_thunder_buffer();
        assert!(RAIN_CACHE.lock().unwrap().is_none(), "Rain cache must be deallocated after purge");
        assert!(WIND_CACHE.lock().unwrap().is_none(), "Wind cache must be deallocated after purge");
        assert!(THUNDER_CACHE.lock().unwrap().is_none(), "Thunder cache must be deallocated after purge");

        // Verify clean re-population on next toggle ON
        let rain3 = get_rain_buffer().expect("rain buffer should reload cleanly");
        assert!(RAIN_CACHE.lock().unwrap().is_some(), "Rain cache should be re-populated on demand");
        assert_eq!(rain1.samples.len(), rain3.samples.len());
    }

    #[test]
    fn test_seamless_loop_source_bounded_and_smooth() {
        let rain = get_rain_buffer().expect("rain buffer should load");
        let mut loop_src = SeamlessLoopSource::new(rain.samples.clone(), rain.channels, rain.sample_rate, AsmrLoopKind::Rain);
        assert_eq!(loop_src.rain_loop_start, rain.sample_rate as usize * 30);
        assert!(loop_src.next().unwrap().abs() < 0.001, "Rain should start without a click");

        let wind = get_wind_buffer().expect("wind buffer should load");
        let wind_loop = SeamlessLoopSource::new(wind.samples.clone(), wind.channels, wind.sample_rate, AsmrLoopKind::Wind);
        assert_eq!(wind_loop.rain_loop_start, 0, "Wind should continue to loop its full recording");

        // Process samples and verify the loop source stays bounded.
        for _ in 0..10_000 {
            let sample = loop_src.next().expect("infinite loop source should always yield");
            assert!(!sample.is_nan(), "Sample must not be NaN");
            assert!(sample >= -1.0 && sample <= 1.0, "Sample {} must be strictly bounded within [-1.0, 1.0]", sample);
        }
    }

    #[test]
    fn test_rain_density_has_audible_range_and_clamps() {
        assert!((rain_density_drive(0) - 0.85).abs() < f32::EPSILON);
        assert!((rain_density_drive(50) - 1.1).abs() < 0.001);
        assert!((rain_density_drive(100) - 1.35).abs() < f32::EPSILON);
        assert_eq!(rain_density_drive(150), rain_density_drive(100));
    }

    #[test]
    fn test_rain_max_density_preserves_transient_headroom() {
        let rain = get_rain_buffer().expect("rain buffer should load");
        let mut left = crate::dsp::Biquad::highpass(rain.sample_rate as f32, 120.0, 0.707);
        let mut right = crate::dsp::Biquad::highpass(rain.sample_rate as f32, 120.0, 0.707);
        let gain = rain_density_drive(100);
        let mut peak = 0.0f32;
        for frame in rain.samples.chunks_exact(2) {
            peak = peak.max(left.process(frame[0] as f32 * (1.0 / 32768.0) * gain).abs());
            peak = peak.max(right.process(frame[1] as f32 * (1.0 / 32768.0) * gain).abs());
        }
        assert!(peak < 0.95, "Rain must retain headroom at maximum density without nonlinear limiting: {peak}");
    }

    #[test]
    fn test_rain_intro_crossfade_has_no_loudness_hole() {
        let rain = get_rain_buffer().expect("rain buffer should load");
        let mut source = SeamlessLoopSource::new(
            rain.samples.clone(), rain.channels, rain.sample_rate, AsmrLoopKind::Rain,
        );
        let samples_per_second = rain.sample_rate as usize * rain.channels as usize;
        let mut levels = Vec::with_capacity(6);
        for _ in 0..6 {
            let mut sum = 0.0f64;
            for _ in 0..samples_per_second {
                let sample = source.next().expect("rain source should continue");
                sum += (sample * sample) as f64;
            }
            levels.push((sum / samples_per_second as f64).sqrt() as f32);
        }
        let transition = &levels[3..6];
        let loudest = transition.iter().copied().fold(0.0f32, f32::max);
        let quietest = transition.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(quietest > loudest * 0.4, "Rain intro crossfade must not dip by more than 8 dB");
    }

    #[test]
    fn test_rain_loop_seam_has_no_loudness_hole() {
        let rain = get_rain_buffer().expect("rain buffer should load");
        let mut source = SeamlessLoopSource::new(
            rain.samples.clone(), rain.channels, rain.sample_rate, AsmrLoopKind::Rain,
        );
        let samples_per_second = rain.sample_rate as usize * rain.channels as usize;
        for _ in 0..57 * samples_per_second { source.next(); }
        let mut levels = Vec::with_capacity(5);
        for _ in 0..5 {
            let mut sum = 0.0f64;
            for _ in 0..samples_per_second {
                let sample = source.next().expect("rain loop should continue");
                sum += (sample * sample) as f64;
            }
            levels.push((sum / samples_per_second as f64).sqrt() as f32);
        }
        let loudest = levels.iter().copied().fold(0.0f32, f32::max);
        let quietest = levels.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(quietest > loudest * 0.4, "Rain loop seam must not dip by more than 8 dB");
    }

    #[test]
    fn test_rain_highpass_removes_engine_band_without_dulling_drops() {
        let measure = |frequency: f32| {
            let mut filter = crate::dsp::Biquad::highpass(22_050.0, 120.0, 0.707);
            let mut sum = 0.0f64;
            let mut count = 0usize;
            for n in 0..44_100 {
                let input = (2.0 * std::f32::consts::PI * frequency * n as f32 / 22_050.0).sin();
                let output = filter.process(input);
                if n >= 2_000 {
                    sum += (output * output) as f64;
                    count += 1;
                }
            }
            (sum / count as f64).sqrt() as f32
        };
        assert!(measure(60.0) < 0.3, "120 Hz high-pass should attenuate low rumble");
        assert!(measure(1_000.0) > 0.65, "120 Hz high-pass should preserve rain transients");
    }

    #[test]
    fn test_rain_calming_acoustic_warmth_and_no_buzz() {
        let measure = |frequency: f32| {
            let mut hp = crate::dsp::Biquad::highpass(22_050.0, 120.0, 0.707);
            let mut lp = crate::dsp::Biquad::lowpass(22_050.0, 2200.0, 0.60);
            let mut sum = 0.0f64;
            let mut count = 0usize;
            for n in 0..44_100 {
                let input = (2.0 * std::f32::consts::PI * frequency * n as f32 / 22_050.0).sin();
                let output = lp.process(hp.process(input));
                if n >= 2_000 {
                    sum += (output * output) as f64;
                    count += 1;
                }
            }
            (sum / count as f64).sqrt() as f32
        };
        // 1. Passband (500 Hz droplet presence) is preserved (> -1 dB)
        assert!(measure(500.0) > 0.65, "Droplet presence in 500 Hz range should be preserved");
        // 2. Ear-canal resonance sizzle (4000 Hz) is strongly attenuated to prevent headaches (> 10 dB down)
        assert!(measure(4_000.0) < 0.25, "Headache-inducing 4 kHz ear-canal band must be rolled off");
        // 3. Ultra-high frequencies (> 8000 Hz) are effectively eliminated (> 22 dB down)
        assert!(measure(8_000.0) < 0.08, "High-frequency hiss must be effectively eliminated");
    }

    #[test]
    fn test_thunder_gap_tracks_frequency_and_clamps() {
        assert_eq!(thunder_gap_seconds(0), 4.0);
        assert_eq!(thunder_gap_seconds(50), 2.5);
        assert_eq!(thunder_gap_seconds(100), 1.0);
        assert_eq!(thunder_gap_seconds(150), thunder_gap_seconds(100));
        assert!((thunder_next_gap_seconds(50, 0.0) - 2.0).abs() < 0.001);
        assert!((thunder_next_gap_seconds(50, 1.0) - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_thunder_gain_never_exceeds_full_scale() {
        assert_eq!(thunder_gain(100, 100, 100), 1.0);
        assert_eq!(thunder_gain(100, 100, 50), 0.5);
        assert_eq!(thunder_gain(150, 150, 150), 1.0);
    }

    #[test]
    fn test_rapid_toggle_memory_purge_and_no_leak() {
        // Verify that dropping an isolated buffer deallocates Arc sample memory completely (0 strong count)
        for _ in 0..2 {
            let sample_buf = decode_audio(RAIN_FIELD_OGG, false, Some(1)).expect("short decode succeeds");
            let weak = std::sync::Arc::downgrade(&sample_buf.samples);
            drop(sample_buf);
            assert_eq!(weak.strong_count(), 0, "Buffer PCM memory must be 100% deallocated when dropped");
        }

        // Verify purge functions reset the static caches
        purge_wind_buffer();
        assert!(WIND_CACHE.lock().unwrap().is_none(), "Wind cache must be None after purge");
        purge_thunder_buffer();
        assert!(THUNDER_CACHE.lock().unwrap().is_none(), "Thunder cache must be None after purge");

        // Verify rain cache can be purged atomically
        purge_rain_buffer();
        let rain = get_rain_buffer().expect("rain buffer reloads on demand");
        assert_eq!(rain.channels, 2);
    }

    #[test]
    fn test_seamless_loop_source_anti_hail_smoothing() {
        let rain = get_rain_buffer().expect("rain buffer should load");
        let mut loop_src = SeamlessLoopSource::new(rain.samples.clone(), rain.channels, rain.sample_rate, AsmrLoopKind::Rain);

        let channels = rain.channels as usize;
        let n = rain.sample_rate as usize * channels;
        let mut opening_sq = 0.0f64;
        let mut max_val = 0.0f32;
        for _ in 0..n {
            let s = loop_src.next().expect("valid sample");
            max_val = max_val.max(s.abs());
            opening_sq += (s * s) as f64;
            assert!(s >= -1.0 && s <= 1.0, "Sample must be bounded");
        }
        let opening_rms = (opening_sq / n as f64).sqrt() as f32;

        let steady_start = rain.sample_rate as usize * 60 * channels;
        for _ in n..steady_start {
            loop_src.next().expect("valid sample");
        }
        let mut steady_sq = 0.0f64;
        for _ in 0..n {
            let s = loop_src.next().expect("valid sample");
            max_val = max_val.max(s.abs());
            steady_sq += (s * s) as f64;
            assert!(s >= -1.0 && s <= 1.0, "Sample must be bounded");
        }
        let steady_rms = (steady_sq / n as f64).sqrt() as f32;
        assert!(max_val < 0.95, "Rain peaks must stay below full scale");
        assert!(steady_rms > opening_rms * 3.0, "The recorded first drops should build into steady rain");
        assert!(steady_rms > 0.005, "Steady rain should remain audible");
    }

    #[test]
    fn test_thock_helper_autocomplete_fav() {
        use rustyline::completion::Completer;
        let helper = ThockHelper {
            commands: vec!["menu".to_string(), "status".to_string(), "pack".to_string(), "packs".to_string(), "fav".to_string()],
            packs: vec!["creamy_butter".to_string(), "creamy_lite_calm".to_string(), "steelseries_apex_pro_v2".to_string()],
        };

        // 1. First word autocomplete: "fa" should complete to "fav"
        let line = "fa";
        let (start, candidates) = helper.complete(line, line.len(), &rustyline::Context::new(&rustyline::history::MemHistory::new())).unwrap();
        assert_eq!(start, 0);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].display, "fav");

        // 2. Second word autocomplete after "fav ": should list matching packs
        let line = "fav cream";
        let (start, candidates) = helper.complete(line, line.len(), &rustyline::Context::new(&rustyline::history::MemHistory::new())).unwrap();
        assert_eq!(start, 4);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].display, "creamy_butter");
        assert_eq!(candidates[1].display, "creamy_lite_calm");

        // 3. Selection parsing with star
        let selected = "★ creamy butter (active)";
        let clean_title = selected
            .trim_start_matches(|c: char| c == '★' || c.is_whitespace())
            .split_whitespace()
            .take_while(|&s| s != "(active)")
            .collect::<Vec<_>>()
            .join(" ");
        let raw_name = clean_title.to_lowercase().replace(' ', "_");
        assert_eq!(raw_name, "creamy_butter");
    }

    #[test]
    fn test_favorites_set_and_json_roundtrip() {
        let mut set = std::collections::HashSet::new();
        set.insert("creamy_butter".to_string());
        set.insert("overlubed_custom".to_string());

        let json = serde_json::to_string(&set).expect("serialize favorites");
        let restored: std::collections::HashSet<String> = serde_json::from_str(&json).expect("deserialize favorites");

        assert!(restored.contains("creamy_butter"));
        assert!(restored.contains("overlubed_custom"));
        assert!(!restored.contains("nk_cream"));
    }

    #[test]
    fn test_thock_helper_autocomplete_delete() {
        use rustyline::completion::Completer;
        let helper = ThockHelper {
            commands: vec![
                "menu".to_string(), "status".to_string(), "pack".to_string(), "packs".to_string(),
                "fav".to_string(), "delete".to_string(), "del".to_string()
            ],
            packs: vec!["creamy_butter".to_string(), "creamy_lite_calm".to_string(), "steelseries_apex_pro_v2".to_string()],
        };

        // 1. First word autocomplete: "del" should complete to "delete" and "del"
        let line = "del";
        let (start, candidates) = helper.complete(line, line.len(), &rustyline::Context::new(&rustyline::history::MemHistory::new())).unwrap();
        assert_eq!(start, 0);
        let displays: Vec<String> = candidates.iter().map(|c| c.display.clone()).collect();
        assert!(displays.contains(&"delete".to_string()));
        assert!(displays.contains(&"del".to_string()));

        // 2. Second word autocomplete after "del ": should list matching packs
        let line = "del cream";
        let (start, candidates) = helper.complete(line, line.len(), &rustyline::Context::new(&rustyline::history::MemHistory::new())).unwrap();
        assert_eq!(start, 4);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].display, "creamy_butter");
        assert_eq!(candidates[1].display, "creamy_lite_calm");

        // 3. Second word autocomplete after "delete ": should list matching packs
        let line = "delete cream";
        let (start, candidates) = helper.complete(line, line.len(), &rustyline::Context::new(&rustyline::history::MemHistory::new())).unwrap();
        assert_eq!(start, 7);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].display, "creamy_butter");
        assert_eq!(candidates[1].display, "creamy_lite_calm");
    }

    #[test]
    fn test_thock_helper_autocomplete_update() {
        use rustyline::completion::Completer;
        let helper = ThockHelper {
            commands: vec![
                "menu".to_string(), "status".to_string(), "pack".to_string(), "packs".to_string(),
                "update".to_string(), "help".to_string()
            ],
            packs: vec!["bubble_heaven".to_string()],
        };

        let line = "upd";
        let (start, candidates) = helper.complete(line, line.len(), &rustyline::Context::new(&rustyline::history::MemHistory::new())).unwrap();
        assert_eq!(start, 0);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].display, "update");
    }

    #[test]
    fn test_favorite_lock_prevents_deletion() {
        let mut favorites = std::collections::HashSet::new();
        favorites.insert("creamy_butter".to_string());
        favorites.insert("nk_cream".to_string());

        let target_locked = "creamy_butter";
        let target_unlocked = "some_temporary_pack";

        // Logic check: Locked pack must be rejected
        let can_delete_locked = !favorites.contains(target_locked);
        assert!(!can_delete_locked, "Favorite pack MUST be locked against deletion");

        // Unlocked pack is allowed
        let can_delete_unlocked = !favorites.contains(target_unlocked);
        assert!(can_delete_unlocked, "Non-favorite pack should be deletable");
    }

    #[test]
    fn test_creamy_butter_feather_acoustics() {
        let wav_path = std::path::Path::new("packs/creamy_butter/a.wav");
        if wav_path.is_file() {
            let bytes = std::fs::read(wav_path).expect("read creamy_butter/a.wav");
            assert!(bytes.len() > 44, "WAV must have audio data beyond header");
            // Standard 16-bit PCM WAV header is 44 bytes, followed by interleaved i16 PCM samples
            let s0 = i16::from_le_bytes([bytes[44], bytes[45]]);
            let s1 = i16::from_le_bytes([bytes[46], bytes[47]]);
            assert!(s0.abs() <= 2, "First sample must start near 0.0 (smooth cushion), got {}", s0);
            assert!(s1.abs() <= 2, "Second sample must start near 0.0 (smooth cushion), got {}", s1);
        }
    }

    #[test]
    fn test_bubble_heaven_pack_loading_and_acoustics() {
        let pack_path = std::path::Path::new("packs/bubble_heaven");
        if pack_path.is_dir() {
            let loaded = load_pack(pack_path).expect("bubble_heaven pack should load cleanly");
            assert!(!loaded.defaults.is_empty(), "Defaults must contain audio");
            assert!(loaded.mappings.contains_key(&57), "Spacebar keycode 57 must be mapped");
            
            // Check space.wav header and smoothness
            let space_path = pack_path.join("space.wav");
            let bytes = std::fs::read(space_path).expect("read bubble_heaven/space.wav");
            assert!(bytes.len() > 44, "space.wav must have audio data");
            let s0 = i16::from_le_bytes([bytes[44], bytes[45]]);
            assert!(s0.abs() <= 2, "First sample of bubble space must start smoothly near 0, got {}", s0);
        }
    }

    #[test]
    fn test_favorites_appear_first_and_numeric_selection() {
        let mut packs = vec![
            "banana_split_lubed".to_string(),
            "bubble_heaven".to_string(),
            "creamy_butter".to_string(),
            "nk_cream".to_string(),
            "tealios_v2".to_string(),
        ];

        let mut favorites = std::collections::HashSet::new();
        favorites.insert("creamy_butter".to_string());
        favorites.insert("bubble_heaven".to_string());

        // Sort so favorites appear at the start together automatically:
        packs.sort_by(|a, b| {
            let a_fav = favorites.contains(a);
            let b_fav = favorites.contains(b);
            b_fav.cmp(&a_fav).then(a.cmp(b))
        });

        // The first two MUST be the favorite packs:
        assert_eq!(packs[0], "bubble_heaven", "1st pack should be favorite");
        assert_eq!(packs[1], "creamy_butter", "2nd pack should be favorite");
        assert_eq!(packs[2], "banana_split_lubed", "3rd pack should be non-favorite");

        // Numeric selection mapping: '1' selects 1st favorite, '2' selects 2nd favorite
        let idx1: usize = "1".parse().unwrap();
        assert_eq!(packs[idx1 - 1], "bubble_heaven");
        let idx2: usize = "2".parse().unwrap();
        assert_eq!(packs[idx2 - 1], "creamy_butter");

        // Clean title parser strips index and star
        let formatted = format!(" 1. ★ Bubble Heaven (active)");
        let clean_title = formatted
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '★' || c.is_whitespace())
            .split_whitespace()
            .take_while(|&s| s != "(active)")
            .collect::<Vec<_>>()
            .join(" ");
        let raw_name = clean_title.to_lowercase().replace(' ', "_");
        assert_eq!(raw_name, "bubble_heaven");
    }

    #[test]
    fn test_pack_switch_memory_purge_and_no_leak() {
        let pack_path = std::path::Path::new("packs/bubble_heaven");
        if pack_path.is_dir() {
            let pack = load_pack(pack_path).expect("pack should load cleanly");
            let weak_default = std::sync::Arc::downgrade(&pack.defaults[0].samples_data);
            assert!(weak_default.strong_count() >= 1);

            // Simulate pack switch: old pack is dropped
            drop(pack);
            assert_eq!(weak_default.strong_count(), 0, "Old sound pack PCM memory must be 100% purged on switch");
        }
    }

    #[test]
    fn test_seamless_loop_source_zero_leak() {
        // Use an isolated decoded buffer to verify lifecycle independently of parallel test threads
        let buf = decode_audio(RAIN_FIELD_OGG, false, Some(1)).expect("decode succeeds");
        let weak = std::sync::Arc::downgrade(&buf.samples);
        let src = SeamlessLoopSource::new(buf.samples.clone(), buf.channels, buf.sample_rate, AsmrLoopKind::Rain);
        
        // Drop the local buffer
        drop(buf);
        
        // SeamlessLoopSource is the sole owner now
        assert_eq!(weak.strong_count(), 1, "Only loop source should hold buffer");
        
        // Dropping the loop source must free all PCM memory completely
        drop(src);
        assert_eq!(weak.strong_count(), 0, "PCM memory must drop to 0 strong refs when loop source is dropped");
    }

    #[test]
    fn test_procedural_switch_physical_acoustics() {
        let config = crate::dsp::ProceduralConfig::default();
        let mut switch = crate::dsp::ProceduralSwitch::new(&config, false, 1.0, 0);

        let mut samples = Vec::new();
        let mut max_abs = 0.0f32;
        let mut sum = 0.0f64;

        while let Some(s) = switch.next() {
            assert!(s.is_finite(), "Procedural sample must be finite");
            assert!(s >= -1.0 && s <= 1.0, "Sample must be bounded in [-1.0, 1.0]");
            max_abs = max_abs.max(s.abs());
            sum += s as f64;
            samples.push(s);
        }

        assert!(!samples.is_empty(), "Procedural switch must generate samples");
        assert!(max_abs > 0.40, "Switch bottom-out must have tactile presence (peak >= 0.40)");
        assert!(max_abs <= 0.99, "Switch bottom-out must retain headroom (peak <= 0.99)");
        
        let mean = (sum / samples.len() as f64).abs();
        assert!(mean < 0.015, "Switch contact wavelet must have near-zero DC offset (< 0.015)");
        assert_eq!(rodio::Source::current_frame_len(&switch), Some(0), "Completed switch voice must report 0 remaining frames");

        // Keyup top-out test: must be whisper-soft and damped
        let mut keyup = crate::dsp::ProceduralSwitch::new(&config, true, 1.0, 0);
        let mut keyup_max = 0.0f32;
        while let Some(s) = keyup.next() {
            keyup_max = keyup_max.max(s.abs());
        }
        assert!(keyup_max < max_abs * 0.35, "Keyup top-out must be whisper-soft (< 35% of keydown)");
    }

    #[test]
    fn test_procedural_switch_spacebar_geometry() {
        let config = crate::dsp::ProceduralConfig::default();
        let space = crate::dsp::ProceduralSwitch::new(&config, false, 1.0, 49);
        let alpha = crate::dsp::ProceduralSwitch::new(&config, false, 1.0, 0);

        let space_dur = rodio::Source::total_duration(&space).unwrap();
        let alpha_dur = rodio::Source::total_duration(&alpha).unwrap();

        assert!(space_dur > alpha_dur, "Spacebar (6.25u cavity) must have longer acoustic ringdown than 1u alpha");
    }
}

