mod keymap;


use crossbeam_channel::unbounded;
use core_graphics::event::{CGEventTap, CGEventTapLocation, CGEventTapPlacement, CGEventTapOptions, CGEventType, EventField, CGEventFlags};
use core_foundation::runloop::CFRunLoop;
use rodio::{Decoder, OutputStream, Source};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::Path,
    process,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, RwLock,
    },
    thread,
};
use tao::event_loop::{ControlFlow, EventLoop};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    TrayIconBuilder,
};

#[cfg(target_os = "macos")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

static VOL: AtomicU32 = AtomicU32::new(100);

enum HotkeyAction {
    VolUp,
    VolDown,
    NextPack,
    PrevPack,
    ToggleFav,
}

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


#[derive(serde::Serialize, serde::Deserialize)]
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

fn get_friendly_name(dir: &str) -> String {
    match dir {
        "cherry_mx_brown_pbt" => "Cherry MX Brown PBT (Tactile / Thocky)".to_string(),
        "cherry_mx_brown_abs" => "Cherry MX Brown ABS (Tactile / Clack)".to_string(),
        "cherry_mx_red_abs" => "Cherry MX Red ABS (Linear / Clacky)".to_string(),
        "cherry_mx_red_pbt" => "Cherry MX Red PBT (Linear / Thocky)".to_string(),
        "cherry_mx_black_abs" => "Cherry MX Black ABS (Linear / Heavy)".to_string(),
        "cherry_mx_black_pbt" => "Cherry MX Black PBT (Linear / Heavy Thock)".to_string(),
        "nk_cream" => "NK Cream (Linear / Buttery)".to_string(),
        "nk_cream_loud" => "NK Cream (Loud 150%)".to_string(),
        "topre_purple" => "Topre Purple (Deep Marbly Thock)".to_string(),
        "pe_foam_creamy" => "PE Foam Custom (Ultra Smooth & Creamy)".to_string(),
        "tape_mod_custom" => "Tape Mod Custom (Marbly & Poppy)".to_string(),
        "overlubed_custom" => "Overlubed Custom (Deep & Muted)".to_string(),
        "glassy_custom" => "Glassy Custom (Crisp & Clacky)".to_string(),
        "tealios_v2" => "Tealios V2 (Linear / Smooth)".to_string(),
        "glorious_panda" => "Glorious Panda (Tactile / Snappy)".to_string(),
        "kailh_box_white" => "Kailh Box White (Clicky / Sharp)".to_string(),
        "eg_crystal_purple" => "EG Crystal Purple (Tactile / Crisp)".to_string(),
        "steelseries_apex_pro_v2" => "SteelSeries Apex Pro (Linear / Magnetic)".to_string(),
        "unicomp_classic" => "Unicomp Classic / IBM M (Buckling Spring / Loud)".to_string(),
        "animal_crossing_nl" => "Animal Crossing (Gaming / Fun)".to_string(),
        "osu" => "Osu! (Gaming / Tap)".to_string(),
        "minimal_tick" => "Minimal Tick (Quiet / Subtle)".to_string(),
        _ => dir.to_string(),
    }
}

fn format_volume_slider(vol_percent: u32) -> String {
    let filled = (vol_percent / 20).min(10) as usize;
    let empty = 10 - filled;
    let bar = "▰".repeat(filled) + &"▱".repeat(empty);
    format!("🔉 {}  {}%", bar, vol_percent)
}

fn load_favorites() -> HashSet<String> {
    let mut favs = HashSet::new();
    if let Ok(file) = File::open("packs/.favorites") {
        for line in BufReader::new(file).lines().flatten() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                favs.insert(trimmed.to_string());
            }
        }
    }
    favs
}

fn save_favorites(favs: &HashSet<String>) {
    if let Ok(mut file) = File::create("packs/.favorites") {
        for f in favs {
            let _ = writeln!(file, "{}", f);
        }
    }
}

fn main() {

    #[cfg(target_os = "macos")]
    if unsafe { !AXIsProcessTrusted() } {
        process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .unwrap();
        eprintln!("Enable Accessibility permissions and restart.");
        return;
    }

    let event_loop = EventLoop::new();
    let key_map = keymap::get_key_map();
    let (hotkey_tx, hotkey_rx) = unbounded::<HotkeyAction>();
    
    let m = Menu::new();
    let packs_menu = Submenu::with_id("packs", "Sound Packs", true);
    
    let packs_dir = Path::new("packs");
    let mut available_packs = Vec::new();
    let mut pack_items = HashMap::new();
    
    let mut favorites = load_favorites();

    if let Ok(entries) = fs::read_dir(packs_dir) {
        let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());
        
        for entry in dirs {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            available_packs.push(dir_name.clone());
            
            let friendly = get_friendly_name(&dir_name);
            let star = if favorites.contains(&dir_name) { "⭐ " } else { "" };
            let item = CheckMenuItem::with_id(format!("pack_{}", dir_name), &format!("{}{}", star, friendly), true, false, None);
            packs_menu.append(&item).unwrap();
            pack_items.insert(dir_name, item);
        }
    }
    
    m.append_items(&[&packs_menu, &PredefinedMenuItem::separator()]).unwrap();

    let toggle_fav_item = MenuItem::with_id("toggle_fav", "⭐ Mark as Favorite", true, None);
    let vol_display = MenuItem::with_id("vol_display", &format_volume_slider(100), false, None);
    let vol_up_i = MenuItem::with_id("up", "Increase Volume (+10%)", true, None);
    let vol_down_i = MenuItem::with_id("dn", "Decrease Volume (-10%)", true, None);
    let hotkey_info = MenuItem::with_id("info", "Shortcuts: Ctrl+Option+Arrows, Fav: F", false, None);
    
    m.append_items(&[
        &toggle_fav_item,
        &PredefinedMenuItem::separator(),
        &vol_display,
        &vol_up_i,
        &vol_down_i,
        &PredefinedMenuItem::separator(),
        &hotkey_info,
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id("quit", "Quit Thock", true, None),
    ]).unwrap();

    let _tray = TrayIconBuilder::new()
        .with_title("🎧 Thock")
        .with_menu(Box::new(m))
        .build()
        .unwrap();

    let current_pack = Arc::new(RwLock::new(None::<LoadedPack>));
    let current_pack_name = Arc::new(RwLock::new(String::new()));
    
    // Sort available_packs to push favorites to the top logically, but the menu is already built.
    // Instead of sorting the array which breaks indices, we'll just keep the menu alphabetical 
    // with stars making them visually stand out.

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
        if let Some(item) = pack_items.get(&first) {
            item.set_checked(true);
        }
        if favorites.contains(&first) {
            toggle_fav_item.set_text("❌ Remove from Favorites");
        } else {
            toggle_fav_item.set_text("⭐ Mark as Favorite");
        }
    }

    let pack_clone = current_pack.clone();

    thread::spawn(move || {
        let (_s, handle) = match OutputStream::try_default() {
            Ok(res) => res,
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
                let is_autorepeat = cg_event.get_integer_value_field(8) != 0; // 8 is kCGKeyboardEventAutorepeat
                if is_autorepeat {
                    return None;
                }

                let keycode = cg_event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                let flags = cg_event.get_flags();
                let ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
                let alt = flags.contains(CGEventFlags::CGEventFlagAlternate);

                if ctrl && alt {
                    match keycode {
                        126 /* Up */ => { let _ = hotkey_tx.send(HotkeyAction::VolUp); return None; }
                        125 /* Down */ => { let _ = hotkey_tx.send(HotkeyAction::VolDown); return None; }
                        124 /* Right */ => { let _ = hotkey_tx.send(HotkeyAction::NextPack); return None; }
                        123 /* Left */ => { let _ = hotkey_tx.send(HotkeyAction::PrevPack); return None; }
                        3 /* F */ => { let _ = hotkey_tx.send(HotkeyAction::ToggleFav); return None; }
                        _ => {}
                    }
                }

                if let Some(pack) = pack_clone.read().unwrap().as_ref() {
                    let audio = if let Some(code) = key_map.get(&keycode) {
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
                        let vol = vol_percent * vol_percent * vol_percent; // Cubic scaling for natural human hearing
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
            Err(e) => {
                eprintln!("Event tap error: {:?}", e);
                let _ = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg("display alert \"Accessibility Blocked\" message \"macOS silently blocked the keyboard listener because the app was updated. Please go to System Settings > Privacy & Security > Accessibility, select Thock, click the MINUS (-) button to completely remove it, then restart Thock.\"")
                    .spawn();
                std::process::exit(1);
            }
        }
    });

    let menu_channel = MenuEvent::receiver();
    let packs_dir = packs_dir.to_path_buf();
    
    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(std::time::Instant::now() + std::time::Duration::from_millis(50));
        
        let mut handle_action = |action: HotkeyAction| {
            match action {
                HotkeyAction::VolUp => {
                    let _ = VOL.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some((v + 10).min(200)));
                    vol_display.set_text(format_volume_slider(VOL.load(Ordering::Relaxed)));
                    save_settings(&packs_dir.join(".settings.json"), VOL.load(Ordering::Relaxed), &*current_pack_name.read().unwrap());
                }
                HotkeyAction::VolDown => {
                    let _ = VOL.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some(v.saturating_sub(10)));
                    vol_display.set_text(format_volume_slider(VOL.load(Ordering::Relaxed)));
                    save_settings(&packs_dir.join(".settings.json"), VOL.load(Ordering::Relaxed), &*current_pack_name.read().unwrap());
                }
                HotkeyAction::NextPack | HotkeyAction::PrevPack => {
                    if available_packs.is_empty() { return; }
                    let current = current_pack_name.read().unwrap().clone();
                    let current_idx = available_packs.iter().position(|p| p == &current).unwrap_or(0);
                    let new_idx = match action {
                        HotkeyAction::NextPack => (current_idx + 1) % available_packs.len(),
                        HotkeyAction::PrevPack => (current_idx + available_packs.len() - 1) % available_packs.len(),
                        _ => 0,
                    };
                    let pack_name = available_packs[new_idx].clone();
                    
                    *current_pack_name.write().unwrap() = pack_name.clone();
                    save_settings(&packs_dir.join(".settings.json"), VOL.load(Ordering::Relaxed), &pack_name);
                    for (name, item) in &pack_items {
                        item.set_checked(name == &pack_name);
                    }
                    
                    if favorites.contains(&pack_name) {
                        toggle_fav_item.set_text("❌ Remove from Favorites");
                    } else {
                        toggle_fav_item.set_text("⭐ Mark as Favorite");
                    }
                    
                    let cp = current_pack.clone();
                    let pd = packs_dir.clone();
                    thread::spawn(move || {
                        if let Some(loaded) = load_pack(&pd.join(&pack_name)) {
                            *cp.write().unwrap() = Some(loaded);
                        }
                    });
                }
                HotkeyAction::ToggleFav => {
                    let current = current_pack_name.read().unwrap().clone();
                    if current.is_empty() { return; }
                    
                    let is_fav = favorites.contains(&current);
                    if is_fav {
                        favorites.remove(&current);
                        toggle_fav_item.set_text("⭐ Mark as Favorite");
                    } else {
                        favorites.insert(current.clone());
                        toggle_fav_item.set_text("❌ Remove from Favorites");
                    }
                    save_favorites(&favorites);
                    
                    if let Some(item) = pack_items.get(&current) {
                        let star = if favorites.contains(&current) { "⭐ " } else { "" };
                        item.set_text(format!("{}{}", star, get_friendly_name(&current)));
                    }
                }
            }
        };

        if let Ok(action) = hotkey_rx.try_recv() {
            handle_action(action);
        }

        if let Ok(e) = menu_channel.try_recv() {
            let id = e.id.as_ref();
            if id == "quit" {
                process::exit(0);
            } else if id == "up" {
                handle_action(HotkeyAction::VolUp);
            } else if id == "dn" {
                handle_action(HotkeyAction::VolDown);
            } else if id == "toggle_fav" {
                handle_action(HotkeyAction::ToggleFav);
            } else if id.starts_with("pack_") {
                let pack_name = id[5..].to_string();
                *current_pack_name.write().unwrap() = pack_name.clone();
                for (name, item) in &pack_items {
                    item.set_checked(name == &pack_name);
                }
                
                if favorites.contains(&pack_name) {
                    toggle_fav_item.set_text("❌ Remove from Favorites");
                } else {
                    toggle_fav_item.set_text("⭐ Mark as Favorite");
                }
                
                let cp = current_pack.clone();
                let pd = packs_dir.clone();
                thread::spawn(move || {
                    if let Some(loaded) = load_pack(&pd.join(&pack_name)) {
                        *cp.write().unwrap() = Some(loaded);
                    }
                });
            }
        }
    });
}
