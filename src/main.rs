mod keymap;

use core_graphics::event::{CGEventTap, CGEventTapLocation, CGEventTapPlacement, CGEventTapOptions, CGEventType, EventField};
use core_foundation::runloop::CFRunLoop;
use rodio::{Decoder, OutputStream, Source};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::BufReader,
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, RwLock,
    },
    thread,
};

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;

use wry::WebViewBuilder;

#[cfg(target_os = "macos")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

static VOL: AtomicU32 = AtomicU32::new(100);

#[derive(Clone)]
struct ArcBuffer {
    data: Arc<Vec<f32>>,
    channels: u16,
    sample_rate: u32,
    cursor: usize,
    volume: f32,
}

impl ArcBuffer {
    fn new(data: Arc<Vec<f32>>, channels: u16, sample_rate: u32) -> Self {
        Self { data, channels, sample_rate, cursor: 0, volume: 1.0 }
    }

    fn with_volume(mut self, vol: f32) -> Self {
        self.volume = vol;
        self
    }
}

impl Iterator for ArcBuffer {
    type Item = f32;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor < self.data.len() {
            let val = self.data[self.cursor] * self.volume;
            self.cursor += 1;
            Some(val)
        } else {
            None
        }
    }
}

impl Source for ArcBuffer {
    fn current_frame_len(&self) -> Option<usize> { Some(self.data.len() - self.cursor) }
    fn channels(&self) -> u16 { self.channels }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<std::time::Duration> { None }
}

#[derive(Deserialize)]
struct PackConfig {
    defaults: Vec<String>,
    #[serde(default)]
    mappings: HashMap<String, String>,
}

struct LoadedPack {
    defaults: Vec<ArcBuffer>,
    mappings: HashMap<u64, ArcBuffer>,
}

fn load_audio_file(path: &Path) -> Option<ArcBuffer> {
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples: Vec<f32> = decoder.convert_samples().collect();
    Some(ArcBuffer::new(Arc::new(samples), channels, sample_rate))
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

    Some(LoadedPack { defaults, mappings })
}

#[derive(Deserialize)]
struct IpcMessage {
    r#type: String,
    value: Option<serde_json::Value>,
}

fn main() {
    let current_exe = std::env::current_exe().unwrap();
    let exe_dir = current_exe.parent().unwrap();
    
    let packs_dir = if exe_dir.ends_with("MacOS") {
        exe_dir.parent().unwrap().join("Resources").join("packs")
    } else {
        std::env::current_dir().unwrap().join("packs")
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

    let is_cli = std::env::args().any(|arg| arg == "--cli");
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
        let default_idx = AtomicU32::new(0);

        let tap_res = CGEventTap::new(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![CGEventType::KeyDown],
            move |_proxy, _type, cg_event| {
                let is_autorepeat = cg_event.get_integer_value_field(8) != 0;
                if is_autorepeat { return None; }

                let keycode = cg_event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;

                if let Some(pack) = &*pack_clone.read().unwrap() {
                    let audio = if let Some(code) = keymap::get_key_map().get(&keycode) {
                        if let Some(mapped) = pack.mappings.get(code) {
                            Some(mapped)
                        } else if !pack.defaults.is_empty() {
                            let idx = default_idx.fetch_add(1, Ordering::Relaxed) as usize;
                            Some(&pack.defaults[idx % pack.defaults.len()])
                        } else {
                            None
                        }
                    } else if !pack.defaults.is_empty() {
                        let idx = default_idx.fetch_add(1, Ordering::Relaxed) as usize;
                        Some(&pack.defaults[idx % pack.defaults.len()])
                    } else {
                        None
                    };

                    if let Some(a) = audio {
                        let vol_percent = VOL.load(Ordering::Relaxed) as f32 / 100.0;
                        let vol = vol_percent * vol_percent * vol_percent;
                        let buf = a.clone().with_volume(vol);
                        let _ = handle.play_raw(buf.convert_samples());
                    }
                }
                None
            }
        );

        match tap_res {
            Ok(tap) => {
                let source = tap.mach_port.create_runloop_source(0).unwrap();
                CFRunLoop::get_current().add_source(&source, unsafe { core_foundation::runloop::kCFRunLoopCommonModes });
                tap.enable();
                unsafe { core_foundation::runloop::CFRunLoopRun() };
            }
            Err(e) => eprintln!("Event tap error: {:?}", e),
        }
    };

    if is_cli {
        audio_thread();
        return;
    } else {
        thread::spawn(audio_thread);
    }

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Thock")
        .with_inner_size(tao::dpi::LogicalSize::new(800.0, 600.0))
        .build(&event_loop)
        .unwrap();

    let ipc_current_pack = current_pack.clone();
    let ipc_current_pack_name = current_pack_name.clone();
    let ipc_packs_dir = packs_dir.clone();
    let ipc_favorites = favorites.clone();

    let html_template = include_str!("../ui.html");
    
    let mut packs_html = String::new();
    let current_pack_read = ipc_current_pack_name.read().unwrap().clone();
    let favs = favorites.read().unwrap().clone();
    
    let mut sorted_packs = available_packs.clone();
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
    
    let current_vol = VOL.load(Ordering::Relaxed).to_string();
    let final_html = html_template
        .replace("<!-- PACKS_LIST -->", &packs_html)
        .replace("<!-- VOL_VALUE -->", &current_vol);

    let _webview = WebViewBuilder::new(&window)
        .with_devtools(true)
        .with_html(final_html)
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            if let Ok(msg) = serde_json::from_str::<IpcMessage>(req.body()) {
                match msg.r#type.as_str() {
                    "set_volume" => {
                        if let Some(val) = msg.value {
                            if let Some(v) = val.as_u64() {
                                VOL.store(v as u32, Ordering::Relaxed);
                                save_settings(&ipc_packs_dir.join(".settings.json"), v as u32, &*ipc_current_pack_name.read().unwrap());
                            }
                        }
                    }"select_pack" => {
                        if let Some(val) = msg.value {
                            if let Some(p) = val.as_str() {
                                *ipc_current_pack_name.write().unwrap() = p.to_string();
                                save_settings(&ipc_packs_dir.join(".settings.json"), VOL.load(Ordering::Relaxed), p);
                                if let Some(loaded) = load_pack(&ipc_packs_dir.join(p)) {
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
                                    let _ = fs::write(ipc_packs_dir.join(".favorites.json"), json);
                                }
                            }
                        }
                    }
                    "quit" => {
                        std::process::exit(0);
                    }
                    _ => {}
                    _ => {}
                }
            }
        })
        .build()
        .unwrap();

    let mut modifiers = tao::keyboard::ModifiersState::empty();

    event_loop.run(move |event, event_loop_target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                #[cfg(target_os = "macos")]
                {
                    use tao::platform::macos::EventLoopWindowTargetExtMacOS;
                    let _ = event_loop_target.hide_application();
                }
            },
            Event::WindowEvent { event: WindowEvent::ModifiersChanged(state), .. } => {
                modifiers = state;
            },
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput {
                    event: tao::event::KeyEvent {
                        logical_key: tao::keyboard::Key::Character(c),
                        ..
                    },
                    ..
                },
                ..
            } if c == "q" && modifiers.contains(tao::keyboard::ModifiersState::SUPER) => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
                    _ => {}
        }
    });
}
