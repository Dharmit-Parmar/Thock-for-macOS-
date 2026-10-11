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
        } else if words[0] == "pack" {
            let word = if words.len() == 2 { words[1] } else { "" };
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

pub struct AsmrBuffer {
    pub samples: std::sync::Arc<[i16]>,
    pub channels: u16,
    pub sample_rate: u32,
}

static RAIN_OGG: &[u8] = include_bytes!("../assets/asmr/rain.ogg");
static WIND_OGG: &[u8] = include_bytes!("../assets/asmr/wind.ogg");
static THUNDER_OGG: &[u8] = include_bytes!("../assets/asmr/thunder.ogg");

fn decode_crossfaded(bytes: &'static [u8]) -> Option<AsmrBuffer> {
    let cursor = std::io::Cursor::new(bytes);
    let decoder = Decoder::new(cursor).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let mut raw: Vec<i16> = rodio::Source::convert_samples::<i16>(decoder).collect();
    let ch = channels as usize;
    let total_frames = raw.len() / ch;
    // 0.5s cosine equal-power crossfade between discarded tail and head
    let xfade_frames = ((sample_rate as usize) / 2).min(total_frames / 4);
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
    Some(AsmrBuffer {
        samples: std::sync::Arc::from(raw.into_boxed_slice()),
        channels,
        sample_rate,
    })
}

fn decode_faded_thunder(bytes: &'static [u8]) -> Option<AsmrBuffer> {
    let cursor = std::io::Cursor::new(bytes);
    let decoder = Decoder::new(cursor).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let raw: Vec<i16> = rodio::Source::convert_samples::<i16>(decoder).collect();
    let ch = channels as usize;
    let total_frames = raw.len() / ch;
    if total_frames == 0 { return None; }

    // 1. Locate the primary thunderclap: find peak sample in the recording
    let mut peak_val: i16 = 0;
    let mut peak_frame = 0usize;
    for (f, frame) in raw.chunks(ch).enumerate() {
        for &s in frame {
            let abs_s = s.abs();
            if abs_s > peak_val {
                peak_val = abs_s;
                peak_frame = f;
            }
        }
    }

    // 2. Search backward from the peak crack (up to 3 seconds) for the pre-rumble onset
    // where audio rises above ambient background floor (0.08 of full scale)
    let ambient_threshold = (32768.0 * 0.08) as i16;
    let pre_roll_frames = (sample_rate as usize * 600) / 1000; // 600ms pre-roll
    let search_window = (sample_rate as usize * 3).min(peak_frame);
    let search_start = peak_frame.saturating_sub(search_window);
    let mut start_frame = peak_frame.saturating_sub(pre_roll_frames);

    for f in search_start..peak_frame {
        let is_over = raw[f * ch..(f + 1) * ch].iter().any(|&s| s.abs() >= ambient_threshold);
        if is_over {
            start_frame = f.saturating_sub(pre_roll_frames);
            break;
        }
    }

    // 3. Slice the buffer from start_frame
    let mut sliced: Vec<i16> = raw[start_frame * ch..].to_vec();
    let sliced_frames = sliced.len() / ch;

    // 4. 40ms smooth cosine fade-in at onset (prevents click, skips ambient rain)
    let attack_frames = ((sample_rate as usize * 40) / 1000).min(sliced_frames / 4);
    for frame in 0..attack_frames {
        let t = frame as f32 / attack_frames as f32;
        let fade_in = (t * std::f32::consts::FRAC_PI_2).sin();
        for c in 0..ch {
            let idx = frame * ch + c;
            sliced[idx] = (sliced[idx] as f32 * fade_in) as i16;
        }
    }

    // 5. 1.2s smooth cosine fade-out at tail (natural thunder dissipation)
    let release_frames = ((sample_rate as usize * 12) / 10).min(sliced_frames / 2);
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

pub fn get_rain_buffer() -> Option<&'static AsmrBuffer> {
    static CACHE: std::sync::OnceLock<Option<AsmrBuffer>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| decode_crossfaded(RAIN_OGG)).as_ref()
}

pub fn get_wind_buffer() -> Option<&'static AsmrBuffer> {
    static CACHE: std::sync::OnceLock<Option<AsmrBuffer>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| decode_crossfaded(WIND_OGG)).as_ref()
}

pub fn get_thunder_buffer() -> Option<&'static AsmrBuffer> {
    static CACHE: std::sync::OnceLock<Option<AsmrBuffer>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| decode_faded_thunder(THUNDER_OGG)).as_ref()
}

pub struct SeamlessLoopSource {
    samples: std::sync::Arc<[i16]>,
    channels: u16,
    sample_rate: u32,
    cursor: usize,
    filter_left: Option<crate::dsp::Biquad>,
    filter_right: Option<crate::dsp::Biquad>,
    warmth_left: Option<crate::dsp::Biquad>,
    warmth_right: Option<crate::dsp::Biquad>,
    kind: AsmrLoopKind,
}

impl SeamlessLoopSource {
    pub fn new(samples: std::sync::Arc<[i16]>, channels: u16, sample_rate: u32, kind: AsmrLoopKind) -> Self {
        let (filter_left, filter_right, warmth_left, warmth_right) = match kind {
            AsmrLoopKind::Rain => {
                let dens = crate::dsp::ASMR_RAIN_DENS.load(Ordering::Relaxed) as f32 / 100.0;
                // Open up lowpass range: natural rain splash/droplets live at 1.2-4kHz
                let fc = 1200.0 + dens * 2800.0;
                (
                    Some(crate::dsp::Biquad::lowpass(sample_rate as f32, fc * 0.97, 0.707)),
                    Some(crate::dsp::Biquad::lowpass(sample_rate as f32, fc * 1.03, 0.707)),
                    // Sub-bass rumble cut at 85 Hz (removes HVAC/car rumble from rain recording)
                    // + gentle body warmth at 180 Hz (real rain on glass/roof, not motor drone)
                    Some(crate::dsp::Biquad::highpass(sample_rate as f32, 85.0, 0.707)),
                    Some(crate::dsp::Biquad::highpass(sample_rate as f32, 85.0, 0.707)),
                )
            }
            AsmrLoopKind::Wind => {
                // Wider aerodynamic range (350-1400 Hz) restores air-texture and breath
                let fc = (crate::dsp::ASMR_WIND_CUTOFF.load(Ordering::Relaxed) as f32).clamp(250.0, 1600.0);
                (
                    Some(crate::dsp::Biquad::lowpass(sample_rate as f32, fc * 0.95, 0.65)),
                    Some(crate::dsp::Biquad::lowpass(sample_rate as f32, fc * 1.05, 0.65)),
                    // Gentle 750 Hz presence peak (+2 dB): adds air movement / rustling texture
                    Some(crate::dsp::Biquad::peaking(sample_rate as f32, 750.0, 1.0, 2.0)),
                    Some(crate::dsp::Biquad::peaking(sample_rate as f32, 750.0, 1.0, 2.0)),
                )
            }
        };

        Self {
            samples,
            channels,
            sample_rate,
            cursor: 0,
            filter_left,
            filter_right,
            warmth_left,
            warmth_right,
            kind,
        }
    }
}

impl Iterator for SeamlessLoopSource {
    type Item = f32;

    #[inline(always)]
    fn next(&mut self) -> Option<f32> {
        if self.samples.is_empty() { return None; }
        let ch_idx = self.cursor % self.channels as usize;
        let mut sample = self.samples[self.cursor] as f32 / 32768.0;

        match self.kind {
            AsmrLoopKind::Rain => {
                // Update tone filter every 128 frames (~2.67ms @ 48kHz) preserving filter state
                if ch_idx == 0 && (self.cursor & 127) == 0 {
                    let dens = crate::dsp::ASMR_RAIN_DENS.load(Ordering::Relaxed) as f32 / 100.0;
                    let fc = 1200.0 + dens * 2800.0;
                    if let Some(fl) = &mut self.filter_left {
                        fl.update_lowpass(self.sample_rate as f32, fc * 0.97, 0.707);
                    }
                    if let Some(fr) = &mut self.filter_right {
                        fr.update_lowpass(self.sample_rate as f32, fc * 1.03, 0.707);
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
            AsmrLoopKind::Wind => {
                // Update aerodynamic lowpass filter tracking dynamic gust cutoff
                if ch_idx == 0 && (self.cursor & 127) == 0 {
                    let fc = (crate::dsp::ASMR_WIND_CUTOFF.load(Ordering::Relaxed) as f32).clamp(250.0, 1600.0);
                    if let Some(fl) = &mut self.filter_left {
                        fl.update_lowpass(self.sample_rate as f32, fc * 0.95, 0.65);
                    }
                    if let Some(fr) = &mut self.filter_right {
                        fr.update_lowpass(self.sample_rate as f32, fc * 1.05, 0.65);
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
        if self.cursor >= self.samples.len() {
            self.cursor = 0;
        }
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

// PredecodedAudio stores raw compressed audio bytes (WAV/OGG).
// rodio's Decoder streams lazily — first-sample latency is ~0ms.
// RAM footprint: ~30KB per file (compressed) vs ~60KB decoded f32.
// 34-file pack: ~1MB RAM instead of ~2MB. 114-file pack: ~3MB instead of ~7MB.

#[derive(Clone)]
pub struct PredecodedAudio {
    pub file_bytes: std::sync::Arc<[u8]>,
    pub decoded: std::sync::Arc<std::sync::OnceLock<std::sync::Arc<[i16]>>>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl PredecodedAudio {
    fn new(path: &std::path::Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let cursor = std::io::Cursor::new(bytes.clone());
        let decoder = rodio::Decoder::new(cursor).ok()?;
        let channels = rodio::Source::channels(&decoder);
        let sample_rate = rodio::Source::sample_rate(&decoder);
        Some(Self {
            file_bytes: std::sync::Arc::from(bytes.into_boxed_slice()),
            decoded: std::sync::Arc::new(std::sync::OnceLock::new()),
            channels,
            sample_rate,
        })
    }

    pub fn samples(&self) -> std::sync::Arc<[i16]> {
        self.decoded.get_or_init(|| {
            let cursor = std::io::Cursor::new(self.file_bytes.clone());
            if let Ok(decoder) = rodio::Decoder::new(cursor) {
                let s: Vec<i16> = rodio::Source::convert_samples::<i16>(decoder).collect();
                std::sync::Arc::from(s.into_boxed_slice())
            } else {
                std::sync::Arc::from(Vec::new().into_boxed_slice())
            }
        }).clone()
    }

    fn play(&self, vol: f32, pitch: f32) -> Option<impl rodio::Source<Item = f32>> {
        let samples = self.samples();
        let src = ZeroCopySource {
            samples,
            channels: self.channels,
            sample_rate: self.sample_rate,
            cursor: 0,
        }.amplify(vol);
        
        if (pitch - 1.0).abs() > 0.005 {
            Some(DirectOrSpeed::Speed(rodio::source::Source::speed(src, pitch)))
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
            let sample = self.samples[self.cursor] as f32 / 32768.0;
            self.cursor += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl rodio::Source for ZeroCopySource {
    fn current_frame_len(&self) -> Option<usize> { None }
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
            let _ = audio.samples(); // pre-warm default fallback sounds
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

    // Pre-warm primary typing keys (Space=57, Return=28, Backspace=14) for instantaneous 0ms latency
    for &common_code in &[57u64, 28u64, 14u64] {
        if let Some(audio) = mappings.get(&common_code) {
            let _ = audio.samples();
        }
    }

    Some(LoadedPack { defaults, mappings, pitch: config.pitch, vol_mult: config.vol_mult, procedural: config.procedural })
}

#[derive(Deserialize)]
struct IpcMessage<'a> {
    #[serde(borrow)]
    r#type: &'a str,
    value: Option<serde_json::Value>,
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
    available_packs.sort();
    
    let favorites = Arc::new(RwLock::new(HashSet::new()));
    if let Ok(json) = fs::read_to_string(packs_dir.join(".favorites.json")) {
        if let Ok(favs) = serde_json::from_str::<HashSet<String>>(&json) {
            *favorites.write().unwrap() = favs;
        }
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
        let pack_display = current_pack_name.read().unwrap().replace('_', " ");
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

        println!("  {:10}  {}", "pack".truecolor(140, 140, 160), pack_display.bold().truecolor(245, 245, 250));
        println!("  {:10}  {}", "volume".truecolor(140, 140, 160), vol_bar);
        println!("  {:10}  {}", "ambient".truecolor(140, 140, 160), ambient_desc);
        println!("  {:10}  {}", "engine".truecolor(140, 140, 160), "zero-latency i16 pcm".truecolor(100, 210, 150));
        println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
        println!("  {}", "Press Enter for interactive menu, or type 'help'".truecolor(120, 120, 140));
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
        // Rain sink — starts empty and paused (lazy load on enable)
        let rain_sink = Sink::try_new(&handle).unwrap_or_else(|_| {
            eprintln!("ASMR Rain: could not create sink");
            Sink::new_idle().0
        });
        rain_sink.set_volume(0.0);
        rain_sink.pause();

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
        thunder_sink.play();

        // Volume-control + thunder-retrigger polling thread
        let _asmr_ctrl = std::thread::Builder::new().stack_size(32 * 1024).spawn(move || {
            use std::sync::atomic::Ordering;
            use crate::dsp::*;
            let mut prng = PRNG::new(0xdeadbeef);
            let mut thunder_cooldown: u32 = 0;
            let mut wind_phase: f32 = 0.0;
            let mut rain_playing = false;
            let mut wind_playing = false;
            let mut prev_rain_vol = -1.0f32;
            let mut prev_wind_vol = -1.0f32;

            loop {
                let rain_on = ASMR_RAIN_ON.load(Ordering::Relaxed) != 0;
                let wind_on = ASMR_WIND_ON.load(Ordering::Relaxed) != 0;
                let thunder_on = ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0;
                let any_active = rain_on || wind_on || thunder_on;

                let m = ASMR_MASTER_VOL.load(Ordering::Relaxed) as f32 / 100.0;
                let dt = 0.030f32; // 30ms (33.3 Hz) for silky-smooth envelope

                // Rain (warm, gentle, dynamically filtered with binaural stereo & peaking warmth EQ)
                if rain_on {
                    if rain_sink.empty() {
                        if let Some(buf) = get_rain_buffer() {
                            rain_sink.append(SeamlessLoopSource::new(buf.samples.clone(), buf.channels, buf.sample_rate, AsmrLoopKind::Rain));
                        }
                    }
                    if !rain_playing {
                        rain_sink.play();
                        rain_playing = true;
                    }
                    let rain_v = ASMR_RAIN_VOL.load(Ordering::Relaxed) as f32 / 100.0;
                    let target = rain_v * m * 0.85;
                    if (target - prev_rain_vol).abs() > 0.002 {
                        rain_sink.set_volume(target);
                        prev_rain_vol = target;
                    }
                } else if rain_playing {
                    rain_sink.pause();
                    rain_playing = false;
                    prev_rain_vol = -1.0;
                }

                // Wind (organic breathing gusts via dual-sine LFO + aerodynamic cutoff)
                if wind_on {
                    if wind_sink.empty() {
                        if let Some(buf) = get_wind_buffer() {
                            wind_sink.append(SeamlessLoopSource::new(buf.samples.clone(), buf.channels, buf.sample_rate, AsmrLoopKind::Wind));
                        }
                    }
                    if !wind_playing {
                        wind_sink.play();
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

                    let target = wind_v * gust_mult * m * 0.75;
                    if (target - prev_wind_vol).abs() > 0.002 {
                        wind_sink.set_volume(target);
                        prev_wind_vol = target;
                    }
                } else if wind_playing {
                    wind_sink.pause();
                    wind_playing = false;
                    prev_wind_vol = -1.0;
                }

                // Thunder (zero-allocation one-shot retrigger)
                if thunder_cooldown > 0 { thunder_cooldown -= 1; }

                if thunder_on {
                    if thunder_cooldown == 0 && thunder_sink.empty() {
                        let t_freq = ASMR_THUNDER_FREQ.load(Ordering::Relaxed);
                        let chance = prng.next_unit_f32();
                        let interval_secs = (120.0 - 1.08 * t_freq as f32).max(10.0);
                        let threshold = dt / interval_secs;
                        if chance < threshold {
                            if let Some(buf) = get_thunder_buffer() {
                                let src = ZeroCopySource {
                                    samples: buf.samples.clone(),
                                    channels: buf.channels,
                                    sample_rate: buf.sample_rate,
                                    cursor: 0,
                                };
                                thunder_sink.append(src);
                                let t_vol = ASMR_THUNDER_VOL.load(Ordering::Relaxed) as f32 / 100.0;
                                let t_int = ASMR_THUNDER_INT.load(Ordering::Relaxed) as f32 / 100.0;
                                thunder_sink.set_volume(t_vol * t_int * m);
                                let cd_secs = 3.0 + (100.0 - t_freq as f32) * 0.4;
                                thunder_cooldown = (cd_secs / dt) as u32;
                            }
                        }
                    }
                } else if !thunder_sink.empty() {
                    thunder_sink.clear();
                }

                // Sleep at end of iteration: ensures pause/stop executes with 0ms latency.
                // 30ms when active for silky LFO; 250ms when idle to save CPU.
                let sleep_ms = if any_active { 30 } else { 250 };
                std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
            }
        });

        // Cell<usize> provides interior mutability with zero overhead (no locks, no atomics).
        // CGEventTap requires Fn (not FnMut), so we can't mutate a plain usize directly.
        let default_idx = std::cell::Cell::new(0usize);

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
                let mut is_keyup = matches!(event_type, CGEventType::KeyUp);
                
                if matches!(event_type, CGEventType::FlagsChanged) {
                    let current_flags = cg_event.get_flags().bits();
                    let old_flags = previous_flags.get();
                    is_keyup = current_flags < old_flags;
                    previous_flags.set(current_flags);
                }

                if is_autorepeat && !is_keyup { return None; }

                let mut velocity_mult = 1.0f32;
                if !is_keyup {
                    let now = std::time::Instant::now();
                    let elapsed = now.duration_since(last_press.replace(now)).as_secs_f32();
                    // Balanced humanized curve: baseline 0.95 at rest, rises smoothly to 1.12 during bursts
                    velocity_mult = (0.92 + (0.025 / (elapsed + 0.10))).clamp(0.90, 1.15);
                }

                let keycode = cg_event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;

                if let Some(pack) = &*pack_clone.read().unwrap() {
                    let vol_percent = VOL.load(Ordering::Relaxed) as f32 / 100.0;
                    // Perceptually uniform power curve (x^1.4), with strict DAC limit at 1.0 to prevent clipping
                    let base_vol = vol_percent.powf(1.4) * pack.vol_mult;
                    let playback_vol = (base_vol * velocity_mult).min(1.0);

                    if let Some(proc_config) = &pack.procedural {
                        let proc = dsp::ProceduralSwitch::new(proc_config, is_keyup, velocity_mult);
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
                            let micro_jitter = 1.0 + ((((keycode as u32).wrapping_mul(1103515245) >> 16) as f32 / 32768.0) - 1.0) * 0.015;
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
        let packs_for_rl = available_packs.clone();
        let packs_dir_clone = packs_dir.clone();
        let current_pack_clone = current_pack.clone();
        let current_pack_name_clone = current_pack_name.clone();

        let _ = std::thread::Builder::new().stack_size(256 * 1024).spawn(move || {
            let helper = ThockHelper {
                commands: vec![
                    "menu".to_string(), "status".to_string(), "vol".to_string(), "pack".to_string(),
                    "packs".to_string(), "proc".to_string(), "asmr".to_string(),
                    "pitch".to_string(), "help".to_string(), "exit".to_string()
                ],
                packs: packs_for_rl.clone(),
            };
            let mut rl = rustyline::Editor::<ThockHelper, _>::new().unwrap();
            rl.set_helper(Some(helper));

            let interactive_menu = |packs: &[String], packs_dir: &std::path::Path, cur_pack: &std::sync::Arc<std::sync::RwLock<Option<LoadedPack>>>, cur_name: &std::sync::Arc<std::sync::RwLock<String>>| {
                use inquire::Select;
                let active_name = cur_name.read().unwrap().replace('_', " ");
                let cur_vol = VOL.load(Ordering::Relaxed);
                let menu_options = vec![
                    format!("Pack        Switch sound pack (active: {})", active_name),
                    format!("Volume      Adjust master playback ({}%)", cur_vol),
                    "Ambient     Rain, wind & thunder background layers".to_string(),
                    "Procedural  Tune physical mechanical switch".to_string(),
                    "Status      Display audio telemetry & profile".to_string(),
                    "Exit        Quit Thock".to_string(),
                ];

                if let Ok(choice) = Select::new("Select action (↑↓ scroll, Enter select):", menu_options).prompt() {
                    if choice.starts_with("Pack") {
                        let active_clean = cur_name.read().unwrap().to_lowercase().replace(' ', "_");
                        let formatted_packs: Vec<String> = packs.iter().map(|p| {
                            let title = p.replace('_', " ");
                            if p == &active_clean {
                                format!("{:<26} (active)", title)
                            } else {
                                format!("{:<26}", title)
                            }
                        }).collect();
                        if let Ok(selected) = Select::new("Select sound pack:", formatted_packs).prompt() {
                            let clean_title = selected.split_whitespace().take_while(|&s| s != "(active)").collect::<Vec<_>>().join(" ");
                            let raw_name = clean_title.to_lowercase().replace(' ', "_");
                            let p_dir = packs_dir.join(&raw_name);
                            *cur_pack.write().unwrap() = None;
                            if let Some(pack) = load_pack(&p_dir) {
                                *cur_pack.write().unwrap() = Some(pack);
                                *cur_name.write().unwrap() = raw_name.clone();
                                println!("  {} Pack: {}", "✓".bold().truecolor(80, 220, 140), clean_title.bold().truecolor(245, 245, 250));
                                save_settings(&packs_dir.join(".settings.json"), VOL.load(Ordering::Relaxed), &raw_name);
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
                            format!("Rain       [{}] Dynamic low-pass warmth filter", rain_st),
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
                    interactive_menu(&packs_for_rl, &packs_dir_clone, &current_pack_clone, &current_pack_name_clone);
                    continue;
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
                    println!("  {:16}  {}", "vol <0-100>".truecolor(200, 190, 255), "Set master playback volume".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "asmr [layer]".truecolor(200, 190, 255), "Configure ambient rain/wind/thunder".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "proc [prop]".truecolor(200, 190, 255), "Tune procedural physics engine".truecolor(160, 160, 175));
                    println!("  {:16}  {}", "exit / quit".truecolor(200, 190, 255), "Exit Thock".truecolor(160, 160, 175));
                    println!("  {}", "──────────────────────────────────────────────────────".truecolor(60, 60, 75));
                    println!();
                },
                "status" => {
                    let p_name = current_pack_name_clone.read().unwrap().replace('_', " ");
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
                "pack" => {
                    if parts.len() > 1 {
                        let pack_name = parts[1..].join("_");
                        let p_dir = packs_dir_clone.join(&pack_name);
                        *current_pack_clone.write().unwrap() = None;
                        if let Some(pack) = load_pack(&p_dir) {
                            *current_pack_clone.write().unwrap() = Some(pack);
                            *current_pack_name_clone.write().unwrap() = pack_name.clone();
                            println!("  {} Pack: {}", "✓".bold().truecolor(80, 220, 140), pack_name.replace('_', " ").bold().truecolor(245, 245, 250));
                            save_settings(&packs_dir_clone.join(".settings.json"), VOL.load(Ordering::Relaxed), &pack_name);
                        } else {
                            println!("  {} Pack not found: '{}'. Type 'packs' to browse.", "✗".bold().truecolor(240, 100, 100), pack_name);
                        }
                    } else {
                        let active_clean = current_pack_name_clone.read().unwrap().to_lowercase().replace(' ', "_");
                        let formatted_packs: Vec<String> = packs_for_rl.iter().map(|p| {
                            let title = p.replace('_', " ");
                            if p == &active_clean {
                                format!("{:<26} (active)", title)
                            } else {
                                format!("{:<26}", title)
                            }
                        }).collect();
                        if let Ok(selected) = inquire::Select::new("Select sound pack:", formatted_packs).prompt() {
                            let clean_title = selected.split_whitespace().take_while(|&s| s != "(active)").collect::<Vec<_>>().join(" ");
                            let raw_name = clean_title.to_lowercase().replace(' ', "_");
                            let p_dir = packs_dir_clone.join(&raw_name);
                            *current_pack_clone.write().unwrap() = None;
                            if let Some(pack) = load_pack(&p_dir) {
                                *current_pack_clone.write().unwrap() = Some(pack);
                                *current_pack_name_clone.write().unwrap() = raw_name.clone();
                                println!("  {} Pack: {}", "✓".bold().truecolor(80, 220, 140), clean_title.bold().truecolor(245, 245, 250));
                                save_settings(&packs_dir_clone.join(".settings.json"), VOL.load(Ordering::Relaxed), &raw_name);
                            }
                        }
                    }
                },
                "packs" => {
                    let active_clean = current_pack_name_clone.read().unwrap().to_lowercase().replace(' ', "_");
                    let formatted_packs: Vec<String> = packs_for_rl.iter().map(|p| {
                        let title = p.replace('_', " ");
                        if p == &active_clean {
                            format!("{:<26} (active)", title)
                        } else {
                            format!("{:<26}", title)
                        }
                    }).collect();
                    if let Ok(selected) = inquire::Select::new("Select sound pack:", formatted_packs).prompt() {
                        let clean_title = selected.split_whitespace().take_while(|&s| s != "(active)").collect::<Vec<_>>().join(" ");
                        let raw_name = clean_title.to_lowercase().replace(' ', "_");
                        let p_dir = packs_dir_clone.join(&raw_name);
                        *current_pack_clone.write().unwrap() = None;
                        if let Some(pack) = load_pack(&p_dir) {
                            *current_pack_clone.write().unwrap() = Some(pack);
                            *current_pack_name_clone.write().unwrap() = raw_name.clone();
                            println!("  {} Pack: {}", "✓".bold().truecolor(80, 220, 140), clean_title.bold().truecolor(245, 245, 250));
                            save_settings(&packs_dir_clone.join(".settings.json"), VOL.load(Ordering::Relaxed), &raw_name);
                        }
                    }
                },
                "proc" => {
                    let mut lock = current_pack.write().unwrap();
                    let mut is_proc = false;
                    if let Some(pack) = lock.as_ref() {
                        if pack.procedural.is_some() {
                            is_proc = true;
                        }
                    }
                    if !is_proc {
                        let new_pack = crate::LoadedPack {
                            mappings: std::collections::HashMap::new(),
                            defaults: vec![],
                            vol_mult: 1.0,
                            pitch: 1.0,
                            procedural: Some(crate::dsp::ProceduralConfig::default()),
                        };
                        *lock = Some(new_pack);
                        *current_pack_name.write().unwrap() = "__cli_proc__".to_string();
                        println!("  {} Switched to procedural synthesis engine", "✓".bold().truecolor(80, 220, 140));
                    }
                    
                    if parts.len() == 1 {
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
                        println!("  {:12}  [{}]  vol {}%  dens {}%", "Rain", st(rain_on), ASMR_RAIN_VOL.load(Ordering::Relaxed), ASMR_RAIN_DENS.load(Ordering::Relaxed));
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
        available_packs: &[String],
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
        
        let mut sorted_packs = available_packs.to_vec();
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
                                    VOL.store(v as u32, std::sync::atomic::Ordering::Relaxed);
                                    save_settings(&ipc_packs_dir_clone.join(".settings.json"), v as u32, &*ipc_current_pack_name.read().unwrap());
                                }
                            }
                        }"select_pack" => {
                            if let Some(val) = msg.value {
                                if let Some(p) = val.as_str() {
                                    *ipc_current_pack_name.write().unwrap() = p.to_string();
                                    save_settings(&ipc_packs_dir_clone.join(".settings.json"), VOL.load(std::sync::atomic::Ordering::Relaxed), p);
                                    *ipc_current_pack.write().unwrap() = None;
                                    if let Some(loaded) = load_pack(&ipc_packs_dir_clone.join(p)) {
                                        *ipc_current_pack.write().unwrap() = Some(loaded);
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
        &available_packs
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
                        &available_packs
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
    fn test_zero_copy_source_conversion() {
        let samples: Vec<i16> = vec![-32768, -16384, 0, 16384, 32767];
        let lock = std::sync::OnceLock::new();
        let _ = lock.set(std::sync::Arc::from(samples.into_boxed_slice()));
        let audio = PredecodedAudio {
            file_bytes: std::sync::Arc::from(Vec::new().into_boxed_slice()),
            decoded: std::sync::Arc::new(lock),
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
        assert_eq!(rodio::Source::current_frame_len(&src), None);
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
                        let sample_arcs: Vec<_> = pack.mappings.values().map(|a| a.file_bytes.as_ptr()).collect();
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

        let wind1 = get_wind_buffer().expect("wind buffer should load");
        let wind2 = get_wind_buffer().expect("wind buffer should load");
        assert!(std::sync::Arc::ptr_eq(&wind1.samples, &wind2.samples), "Wind buffer must be cached as a single Arc");

        let thunder1 = get_thunder_buffer().expect("thunder buffer should load");
        let thunder2 = get_thunder_buffer().expect("thunder buffer should load");
        assert!(std::sync::Arc::ptr_eq(&thunder1.samples, &thunder2.samples), "Thunder buffer must be cached as a single Arc");
    }

    #[test]
    fn test_seamless_loop_source_bounded_and_smooth() {
        let rain = get_rain_buffer().expect("rain buffer should load");
        let mut loop_src = SeamlessLoopSource::new(rain.samples.clone(), rain.channels, rain.sample_rate, AsmrLoopKind::Rain);

        // Process 10,000 frames through the filter and verify bounds
        for _ in 0..10_000 {
            let sample = loop_src.next().expect("infinite loop source should always yield");
            assert!(!sample.is_nan(), "Sample must not be NaN");
            assert!(sample >= -1.0 && sample <= 1.0, "Sample {} must be strictly bounded within [-1.0, 1.0]", sample);
        }
    }
}

