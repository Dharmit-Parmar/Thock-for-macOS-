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
use rodio::{Decoder, OutputStream, Source};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::BufReader,
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
struct ArcBuffer {
    bytes: Arc<Vec<u8>>,
    volume: f32,
}

impl ArcBuffer {
    fn new(bytes: Arc<Vec<u8>>) -> Self {
        Self { bytes, volume: 1.0 }
    }

    fn with_volume(mut self, vol: f32) -> Self {
        self.volume = vol;
        self
    }

    /// Decode to a live rodio Source. Called on the audio output thread, never on the event tap.
    fn into_source(self) -> Option<impl Source<Item = f32>> {
        use std::io::Cursor;
        let cursor = Cursor::new((*self.bytes).clone());
        let decoder = Decoder::new(BufReader::new(cursor)).ok()?;
        Some(decoder.convert_samples::<f32>().amplify(self.volume))
    }
}

fn load_audio_file(path: &Path) -> Option<ArcBuffer> {
    let bytes = fs::read(path).ok()?;
    Some(ArcBuffer::new(Arc::new(bytes)))
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
    defaults: Vec<ArcBuffer>,
    mappings: HashMap<u64, ArcBuffer>,
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
                            // Lazily decode bytes to PCM on the audio thread via into_source()
                            if let Some(src) = a.clone().with_volume(vol).into_source() {
                                if (pack.pitch - 1.0).abs() > 0.01 {
                                    let _ = handle.play_raw(src.speed(pack.pitch).convert_samples());
                                } else {
                                    let _ = handle.play_raw(src.convert_samples());
                                }
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
    
    // UI Builder function to allow dynamic recreation of the window and webview
    // without leaking 200MB of WebKit memory when the window is closed.
    fn build_ui(
        event_loop: &tao::event_loop::EventLoopWindowTarget<()>,
        ipc_current_pack_name: std::sync::Arc<std::sync::RwLock<String>>,
        ipc_current_pack: std::sync::Arc<std::sync::RwLock<Option<crate::LoadedPack>>>,
        ipc_packs_dir: std::path::PathBuf,
        ipc_favorites: std::sync::Arc<std::sync::RwLock<std::collections::HashSet<String>>>,
        available_packs: &[String],
    ) -> (tao::window::Window, wry::WebView) {
        let window = tao::window::WindowBuilder::new()
            .with_title("Thock")
            .with_inner_size(tao::dpi::LogicalSize::new(800.0, 600.0))
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
            
            let active_badge = if is_active {
                r#"inline-block"#
            } else { "none" };
            
            let active_card_class = if is_active { "border-pink-400 bg-white/70 shadow-md" } else { "bg-white/40" };
            let active_anim = if is_active { "flex" } else { "none" };
            
            let fav_class = if is_fav { "text-yellow-400 fill-current" } else { "text-gray-400" };

            let display_name = pack.replace("_", " ").to_uppercase();
            let safe_id = pack.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>();

            let card = format!(r#"
                <div id="pack-{}" onclick="selectPack('{}')" class="pack-card glass-card py-3.5 px-5 rounded-2xl cursor-pointer flex items-center justify-between group {} transition-all hover:bg-white/60 mb-2.5 border border-white/20">
                    <div class="flex items-center space-x-3.5">
                        <div class="w-10 h-10 rounded-full bg-pink-50 flex items-center justify-center shadow-sm text-pink-500">
                            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"></path></svg>
                        </div>
                        <div>
                            <h3 class="font-bold text-gray-800 text-base flex items-center">
                                {}
                                <span class="active-badge ml-2 px-2 py-0.5 rounded-md bg-pink-200 text-pink-700 text-xs font-semibold tracking-wide uppercase" style="display: {}">Active</span>
                            </h3>
                        </div>
                    </div>
                    <div class="flex items-center space-x-3">
                        <button onclick="toggleFav(event, '{}')" class="w-8 h-8 rounded-full bg-white/50 flex items-center justify-center hover:bg-white shadow-sm transition-all">
                            <svg id="fav-{}" class="w-5 h-5 {}" viewBox="0 0 20 20" stroke="currentColor" stroke-width="1.5" fill="none"><path stroke-linecap="round" stroke-linejoin="round" d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z"></path></svg>
                        </button>
                        <div class="active-anim space-x-1" style="display: {};">
                            <div class="w-1.5 h-4 bg-pink-400 rounded-full animate-bounce" style="animation-delay: 0s;"></div>
                            <div class="w-1.5 h-5 bg-pink-400 rounded-full animate-bounce" style="animation-delay: 0.1s;"></div>
                            <div class="w-1.5 h-3 bg-pink-400 rounded-full animate-bounce" style="animation-delay: 0.2s;"></div>
                        </div>
                    </div>
                </div>
            "#, safe_id, pack, active_card_class, display_name, active_badge, pack, safe_id, fav_class, active_anim);
            
            packs_html.push_str(&card);
        }
        
        let mut packs_options = String::new();
        for pack in &sorted_packs {
            let display_name = pack.replace("_", " ").to_uppercase();
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

        (window, webview)
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
                        logical_key: tao::keyboard::Key::Character(c),
                        ..
                    },
                    ..
                },
                ..
            } if c == "q" && modifiers.contains(tao::keyboard::ModifiersState::SUPER) => {
                *control_flow = tao::event_loop::ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
