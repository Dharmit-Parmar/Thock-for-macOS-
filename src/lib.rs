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

pub struct LoopingDecoder {
    bytes: &'static [u8],
    decoder: Decoder<std::io::Cursor<&'static [u8]>>,
}

impl LoopingDecoder {
    pub fn new(bytes: &'static [u8]) -> Result<Self, rodio::decoder::DecoderError> {
        let decoder = Decoder::new(std::io::Cursor::new(bytes))?;
        Ok(Self { bytes, decoder })
    }
}

impl Iterator for LoopingDecoder {
    type Item = i16;
    #[inline]
    fn next(&mut self) -> Option<i16> {
        if let Some(sample) = self.decoder.next() {
            Some(sample)
        } else {
            if let Ok(new_decoder) = Decoder::new(std::io::Cursor::new(self.bytes)) {
                self.decoder = new_decoder;
                self.decoder.next()
            } else {
                None
            }
        }
    }
}

impl Source for LoopingDecoder {
    fn current_frame_len(&self) -> Option<usize> { self.decoder.current_frame_len() }
    fn channels(&self) -> u16 { self.decoder.channels() }
    fn sample_rate(&self) -> u32 { self.decoder.sample_rate() }
    fn total_duration(&self) -> Option<std::time::Duration> { None } // Infinite
}

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc, RwLock,
    },
    thread,
    time::Instant,
};



#[cfg(target_os = "macos")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

static VOL: AtomicU32 = AtomicU32::new(100);

// ArcBuffer stores the raw compressed file bytes (WAV/OGG).
// Decoding to f32 PCM happens lazily when Iterator::next() is first called,
// inside rodio's internal stream thread — so it never blocks the CGEventTap.
// RAM footprint per pack: ~300KB (bytes) instead of ~9MB (decoded f32 PCM).
#[derive(Clone)]
pub struct ArcPcm {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Arc<Vec<f32>>,
}

impl ArcPcm {
    fn with_volume(self, vol: f32) -> ArcPcmSource {
        ArcPcmSource {
            pcm: self,
            cursor: 0,
            volume: vol,
        }
    }
}

#[derive(Clone)]
pub struct ArcPcmSource {
    pcm: ArcPcm,
    cursor: usize,
    volume: f32,
}

impl Iterator for ArcPcmSource {
    type Item = f32;
    #[inline]
    fn next(&mut self) -> Option<f32> {
        if self.cursor < self.pcm.samples.len() {
            let sample = self.pcm.samples[self.cursor] * self.volume;
            self.cursor += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl Source for ArcPcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.pcm.samples.len() - self.cursor)
    }
    fn channels(&self) -> u16 { self.pcm.channels }
    fn sample_rate(&self) -> u32 { self.pcm.sample_rate }
    fn total_duration(&self) -> Option<std::time::Duration> {
        let frames = self.pcm.samples.len() as u64 / self.pcm.channels as u64;
        Some(std::time::Duration::from_nanos(frames * 1_000_000_000 / self.pcm.sample_rate as u64))
    }
}

fn load_audio_file(path: &Path) -> Option<ArcPcm> {
    use rodio::Decoder;
    use std::io::{BufReader, Cursor};
    let bytes = fs::read(path).ok()?;
    let decoder = Decoder::new(BufReader::new(Cursor::new(bytes))).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples: Vec<f32> = decoder.convert_samples::<f32>().collect();
    Some(ArcPcm { channels, sample_rate, samples: Arc::new(samples) })
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

struct LoadedPack {
    defaults: Vec<ArcPcm>,
    mappings: HashMap<u64, ArcPcm>,
    pitch: f32,
    vol_mult: f32,
    procedural: Option<dsp::ProceduralConfig>,
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

fn load_pack(pack_dir: &Path) -> Option<LoadedPack> {
    let config_str = fs::read_to_string(pack_dir.join("config.json")).ok()?;
    let config: PackConfig = serde_json::from_str(&config_str).ok()?;

    let mut defaults = Vec::new();
    for def in config.defaults {
        if let Some(audio) = load_audio_file(&pack_dir.join(def)) {
            defaults.push(audio);
        }
    }

    let mut mappings = HashMap::new();
    for (key_str, filename) in config.mappings {
        if let Ok(key_code) = key_str.parse::<u64>() {
            if let Some(audio) = load_audio_file(&pack_dir.join(filename)) {
                mappings.insert(key_code, audio);
            }
        }
    }

    Some(LoadedPack { defaults, mappings, pitch: config.pitch, vol_mult: config.vol_mult, procedural: config.procedural })
}

#[derive(Deserialize)]
struct IpcMessage {
    r#type: String,
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
    
    let pack_clone = current_pack.clone();
    let ax_trusted = unsafe { AXIsProcessTrusted() };

        if is_cli {
        println!("🎧 Thock is running in Lightweight CLI mode...");
        if !ax_trusted {
            println!("⚠️ WARNING: Accessibility permissions not granted. Keypresses may not be detected.");
        }
        println!("Current Pack: {}", *current_pack_name.read().unwrap());
        println!("Press Ctrl+C to quit.");
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
        // Embedded files compiled into the binary.
        static RAIN_OGG: &[u8] = include_bytes!("../assets/asmr/rain.ogg");
        static WIND_OGG: &[u8] = include_bytes!("../assets/asmr/wind.ogg");
        static THUNDER_OGG: &[u8] = include_bytes!("../assets/asmr/thunder.ogg");

        // Rain sink — looping
        let rain_sink = Sink::try_new(&handle).unwrap_or_else(|_| {
            eprintln!("ASMR Rain: could not create sink");
            Sink::new_idle().0
        });
        rain_sink.set_volume(0.0);
        match LoopingDecoder::new(RAIN_OGG) {
            Ok(decoder) => {
                let src = decoder.convert_samples::<f32>();
                rain_sink.append(src);
            },
            Err(e) => {
                eprintln!("ASMR Rain: failed to decode audio asset: {}", e);
                crate::dsp::ASMR_RAIN_ON.store(0, std::sync::atomic::Ordering::Relaxed);
            }
        }
        rain_sink.play();

        // Wind sink — looping
        let wind_sink = Sink::try_new(&handle).unwrap_or_else(|_| {
            eprintln!("ASMR Wind: could not create sink");
            Sink::new_idle().0
        });
        wind_sink.set_volume(0.0);
        match LoopingDecoder::new(WIND_OGG) {
            Ok(decoder) => {
                let src = decoder.convert_samples::<f32>();
                wind_sink.append(src);
            },
            Err(e) => {
                eprintln!("ASMR Wind: failed to decode audio asset: {}", e);
                crate::dsp::ASMR_WIND_ON.store(0, std::sync::atomic::Ordering::Relaxed);
            }
        }
        wind_sink.play();

        // Thunder sink — one-shot, re-triggered by a timer thread
        let thunder_sink = Sink::try_new(&handle).unwrap_or_else(|_| {
            eprintln!("ASMR Thunder: could not create sink");
            Sink::new_idle().0
        });
        thunder_sink.set_volume(0.0);
        thunder_sink.play();

        // Volume-control + thunder-retrigger polling thread
        let _asmr_ctrl = std::thread::spawn(move || {
            use std::sync::atomic::Ordering;
            use crate::dsp::*;
            let mut rng_seed: u32 = 0xdeadbeef;
            let mut thunder_cooldown: u32 = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(50));

                let m = ASMR_MASTER_VOL.load(Ordering::Relaxed) as f32 / 100.0;

                // Rain
                let rain_on = ASMR_RAIN_ON.load(Ordering::Relaxed) != 0;
                let rain_v = ASMR_RAIN_VOL.load(Ordering::Relaxed) as f32 / 100.0;
                let rain_dens = ASMR_RAIN_DENS.load(Ordering::Relaxed) as f32 / 100.0;
                // Density acts as an intensity volume scaler (0.3x to 1.0x) to avoid pitch-shifting
                let rain_intensity = 0.3 + (rain_dens * 0.7);
                rain_sink.set_volume(if rain_on { rain_v * rain_intensity * m } else { 0.0 });

                // Wind
                let wind_on = ASMR_WIND_ON.load(Ordering::Relaxed) != 0;
                let wind_v = ASMR_WIND_VOL.load(Ordering::Relaxed) as f32 / 100.0;
                let wind_gust = ASMR_WIND_GUST.load(Ordering::Relaxed) as f32 / 100.0;
                let wind_intensity = 0.3 + (wind_gust * 0.7);
                wind_sink.set_volume(if wind_on { wind_v * wind_intensity * m } else { 0.0 });

                // Thunder — retrigger one-shot randomly
                let thunder_on = ASMR_THUNDER_ON.load(Ordering::Relaxed) != 0;
                let t_vol = ASMR_THUNDER_VOL.load(Ordering::Relaxed) as f32 / 100.0;
                let t_freq = ASMR_THUNDER_FREQ.load(Ordering::Relaxed); // 0-100: 0=rare, 100=frequent

                if thunder_cooldown > 0 { thunder_cooldown -= 1; }

                if thunder_on && thunder_cooldown == 0 && thunder_sink.empty() {
                    // Random probability scaled by frequency slider
                    rng_seed ^= rng_seed << 13;
                    rng_seed ^= rng_seed >> 17;
                    rng_seed ^= rng_seed << 5;
                    let chance = (rng_seed as f32 / u32::MAX as f32).abs();
                    let threshold = 0.003 + (t_freq as f32 / 100.0) * 0.03; // 0.3%-3.3% per tick
                    if chance < threshold {
                        match Decoder::new(std::io::Cursor::new(THUNDER_OGG)) {
                            Ok(decoder) => {
                                let src = decoder.convert_samples::<f32>();
                                thunder_sink.append(src);
                                thunder_sink.set_volume(t_vol * m);
                                // Minimum cooldown between strikes (20 ticks = 1 second baseline)
                                let min_cd = 40u32 + ((100 - t_freq) as u32 * 8);
                                thunder_cooldown = min_cd;
                            },
                            Err(e) => {
                                eprintln!("ASMR Thunder: failed to decode audio asset: {}", e);
                                ASMR_THUNDER_ON.store(0, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                }
                if !thunder_on {
                    // Clear pending thunder if layer turned off
                    if !thunder_sink.empty() {
                        thunder_sink.clear();
                    }
                }
            }
        });

        // Cell<usize> provides interior mutability with zero overhead (no locks, no atomics).
        // CGEventTap requires Fn (not FnMut), so we can't mutate a plain usize directly.
        let default_idx = std::cell::Cell::new(0usize);

        // Lock-free velocity tracking: store nanoseconds since UNIX epoch as AtomicU64.
        // Avoids a write-lock acquisition on every single KeyDown event.
        let epoch = Instant::now();
        let last_press_ns = Arc::new(AtomicU64::new(0));
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
                    let now_ns = epoch.elapsed().as_nanos() as u64;
                    let prev_ns = last_press_ns.swap(now_ns, Ordering::Relaxed);
                    let elapsed = (now_ns.saturating_sub(prev_ns)) as f32 / 1_000_000_000.0;
                    velocity_mult = (0.5 + (0.1 / (elapsed + 0.01))).clamp(0.8, 1.5);
                }

                let keycode = cg_event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;

                if let Some(pack) = &*pack_clone.read().unwrap() {
                    let vol_percent = VOL.load(Ordering::Relaxed) as f32 / 100.0;
                    // Squared curve: at vol=80% → 64% amplitude (perceptually natural)
                    let vol = vol_percent * vol_percent * pack.vol_mult;

                    if let Some(proc_config) = &pack.procedural {
                        let proc = dsp::ProceduralSwitch::new(proc_config.clone(), is_keyup, velocity_mult);
                        // Procedural audio plays both keydown and keyup
                        let _ = handle.play_raw(proc.amplify(vol).convert_samples());
                    } else if !is_keyup {
                        let audio = if let Some(code) = keymap::get_key_map().get(&keycode) {
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
                            let src = a.clone().with_volume(vol);
                            if (pack.pitch - 1.0).abs() > 0.01 {
                                let _ = handle.play_raw(src.speed(pack.pitch).convert_samples());
                            } else {
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
        println!("\n🎧 Thock Headless CLI Mode Active");
        println!("Type 'help' to see available commands.\n");
        
        // Run CLI REPL in a background thread so the Main Thread is dedicated
        // purely to the macOS CGEventTap. MacOS heavily throttles background
        // threads, which causes input latency. Running the event tap on the
        // main thread fixes the latency instantly.
        let packs_for_rl = available_packs.clone();
        thread::spawn(move || {
            let helper = ThockHelper {
                commands: vec!["vol".to_string(), "pack".to_string(), "proc".to_string(), "pitch".to_string(), "help".to_string(), "exit".to_string()],
                packs: packs_for_rl,
            };
            let mut rl = rustyline::Editor::<ThockHelper, _>::new().unwrap();
            rl.set_helper(Some(helper));
            
            loop {
                let readline = rl.readline("> ");
                let input = match readline {
                    Ok(line) => {
                        let _ = rl.add_history_entry(line.as_str());
                        line
                    },
                    Err(_) => break,
                };
                let parts: Vec<&str> = input.trim().split_whitespace().collect();
            if parts.is_empty() { continue; }
            
            match parts[0] {
                "help" => {
                    println!("Available Commands:");
                    println!("  vol <0-100>    - Set master volume (e.g. vol 80)");
                    println!("  pack <name>    - Change the active sound pack (e.g. pack creamy)");
                    println!("  packs          - List all available sound packs");
                    println!("  proc           - View/Edit mathematical procedural settings");
                    println!("  asmr           - View/Edit ASMR background audio settings (e.g. asmr rain, asmr vol 80)");
                    println!("  quit/exit      - Close the application");
                },
                "vol" => {
                    if parts.len() > 1 {
                        if let Ok(v) = parts[1].parse::<u32>() {
                            let v = v.clamp(0, 100);
                            VOL.store(v, Ordering::Relaxed);
                            println!("🔊 Volume set to {}%", v);
                            save_settings(&packs_dir.join(".settings.json"), v, &*current_pack_name.read().unwrap());
                        } else {
                            println!("Invalid volume. Use a number between 0 and 100.");
                        }
                    }
                },
                "pack" => {
                    if parts.len() > 1 {
                        let pack_name = parts[1..].join("_");
                        let p_dir = packs_dir.join(&pack_name);
                        if let Some(pack) = load_pack(&p_dir) {
                            *current_pack.write().unwrap() = Some(pack);
                            *current_pack_name.write().unwrap() = pack_name.clone();
                            println!("✅ Switched to sound pack: {}", pack_name);
                            save_settings(&packs_dir.join(".settings.json"), VOL.load(Ordering::Relaxed), &pack_name);
                        } else {
                            println!("❌ Pack not found: {}", pack_name);
                        }
                    }
                },
                "packs" => {
                    println!("📦 Available Packs:");
                    for p in &available_packs {
                        let active = if *current_pack_name.read().unwrap() == *p { " (Active)" } else { "" };
                        println!("  - {}{}", p, active);
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
                        println!("⚙️ Switched to Procedural Engine.");
                    }
                    
                    if parts.len() == 1 {
                        if let Some(pack) = lock.as_ref() {
                            if let Some(proc) = &pack.procedural {
                                println!("⚙️ Current Procedural Settings:");
                                println!("  switch: {} (red|black|brown|blue|white)", proc.switch_type);
                                println!("  weight: {}g (40-90)", proc.spring_weight);
                                println!("  lube:   {:.2} (0.0-1.0)", proc.lube_amount);
                                println!("  foam:   {:.2} (0.0-1.0)", proc.foam_mod);
                                println!("  orings: {:.2} (0.0-1.0)", proc.o_rings);
                                println!("  pitch:  {:.2} (0.5-2.0)", proc.pitch);
                                println!("  keycap: {} (pbt|abs)", proc.keycap_material);
                                println!("  plate:  {} (brass|pom)", proc.plate_material);
                                println!("  case:   {} (aluminum|plastic)", proc.case_material);
                                println!("  mount:  {} (tray|gasket)", proc.mounting_style);
                                println!("
Change a setting: proc <setting> <value> (e.g. proc lube 0.9)");
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
                                    "switch" => { proc.switch_type = val.to_string(); println!("Set switch to {}", val); },
                                    "weight" => if let Ok(v) = val.parse::<f32>() { proc.spring_weight = v; println!("Set weight to {}", v); },
                                    "lube" => if let Ok(v) = val.parse::<f32>() { proc.lube_amount = v; println!("Set lube to {}", v); },
                                    "foam" => if let Ok(v) = val.parse::<f32>() { proc.foam_mod = v; println!("Set foam to {}", v); },
                                    "orings" => if let Ok(v) = val.parse::<f32>() { proc.o_rings = v; println!("Set orings to {}", v); },
                                    "pitch" => if let Ok(v) = val.parse::<f32>() { proc.pitch = v; println!("Set pitch to {}", v); },
                                    "keycap" => { proc.keycap_material = val.to_string(); println!("Set keycap to {}", val); },
                                    "plate" => { proc.plate_material = val.to_string(); println!("Set plate to {}", val); },
                                    "case" => { proc.case_material = val.to_string(); println!("Set case to {}", val); },
                                    "mount" => { proc.mounting_style = val.to_string(); println!("Set mount to {}", val); },
                                    _ => println!("Unknown procedural property: {}", prop),
                                }
                            }
                        }
                    } else {
                        println!("Usage: proc <property> <value>");
                    }
                },
                "asmr" => {
                    if parts.len() == 1 {
                        let mode = 0;
                        println!("ASMR Status:");
                        println!("  Mode: {}", match mode { 1 => "rain", 2 => "thunder", _ => "none" });
                        println!("  Master Vol: {}%", crate::dsp::ASMR_MASTER_VOL.load(Ordering::Relaxed));
                        if mode >= 1 {
                            println!("  Rain Dens: {}%, Vol: {}%", crate::dsp::ASMR_RAIN_DENS.load(Ordering::Relaxed), crate::dsp::ASMR_RAIN_VOL.load(Ordering::Relaxed));
                            println!("  Wind Gust: {}%, Vol: {}%", crate::dsp::ASMR_WIND_GUST.load(Ordering::Relaxed), crate::dsp::ASMR_WIND_VOL.load(Ordering::Relaxed));
                        }
                        if mode == 2 {
                            println!("  Thunder Freq: {}%, Int: {}%, Vol: {}%", crate::dsp::ASMR_THUNDER_FREQ.load(Ordering::Relaxed), crate::dsp::ASMR_THUNDER_INT.load(Ordering::Relaxed), crate::dsp::ASMR_THUNDER_VOL.load(Ordering::Relaxed));
                        }
                        println!("\nUsage:");
                        println!("  asmr none|rain|thunder");
                        println!("  asmr <vol|rain_dens|wind_gust|rain_vol|wind_vol|thunder_freq|thunder_int|thunder_vol> <0-100>");
                        continue;
                    }
                    
                    let prop = parts[1];
                    match prop {
                        "none" => { crate::dsp::ASMR_RAIN_ON.store(0, Ordering::Relaxed); crate::dsp::ASMR_WIND_ON.store(0, Ordering::Relaxed); crate::dsp::ASMR_THUNDER_ON.store(0, Ordering::Relaxed); println!("✅ ASMR disabled"); },
                        "rain" => { crate::dsp::ASMR_RAIN_ON.store(1, Ordering::Relaxed); println!("🌧️ ASMR Rain on"); },
                        "thunder" => { crate::dsp::ASMR_THUNDER_ON.store(1, Ordering::Relaxed); println!("⛈️ ASMR Thunder on"); },
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
                                        _ => { println!("Unknown ASMR property: {}", prop); continue; }
                                    }
                                    println!("✅ Set ASMR {} to {}", prop, v);
                                } else {
                                    println!("❌ Value must be a number from 0 to 100");
                                }
                            } else {
                                println!("❌ Missing value. Usage: asmr {} <0-100>", prop);
                            }
                        }
                    }
                },
                "quit" | "exit" => {
                    println!("Goodbye! 👋");
                    std::process::exit(0);
                },
                _ => println!("Unknown command. Type 'help' for options."),
            }
        }
        });
        // Block the main thread with the low-latency audio event tap
        audio_thread();
        return;
    } else {
        thread::spawn(audio_thread);
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
            r#"<div class="bg-red-500/90 text-white p-3 rounded-xl mb-4 text-center font-semibold shadow-lg border border-red-400">
                ⚠️ Accessibility permissions not granted. Keypresses will not be detected. Enable in System Settings &gt; Privacy &amp; Security &gt; Accessibility.
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
            .with_devtools(true)
            .with_html(final_html)
            .with_ipc_handler(move |req: wry::http::Request<String>| {
                if let Ok(msg) = serde_json::from_str::<IpcMessage>(req.body()) {
                    match msg.r#type.as_str() {
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
                                if let Some(v) = val.get("thunder_intensity").and_then(|v| v.as_u64()) {
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
