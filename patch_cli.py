with open("src/main.rs", "r") as f:
    content = f.read()

# Add std::env to imports if not there (usually available directly)
# we can just use std::env::args

# Find the start of the background thread logic
bg_thread_old = """    let ax_trusted = unsafe { AXIsProcessTrusted() };

    // BACKGROUND THREAD: Audio + CGEventTap
    thread::spawn(move || {"""

bg_thread_new = """    let ax_trusted = unsafe { AXIsProcessTrusted() };

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
    let audio_thread = move || {"""

content = content.replace(bg_thread_old, bg_thread_new)

# Find the end of the thread block
thread_end_old = """            Err(e) => eprintln!("Event tap error: {:?}", e),
        }
    });

    let event_loop = EventLoop::new();"""

thread_end_new = """            Err(e) => eprintln!("Event tap error: {:?}", e),
        }
    };

    if is_cli {
        audio_thread();
        return;
    } else {
        thread::spawn(audio_thread);
    }

    let event_loop = EventLoop::new();"""

content = content.replace(thread_end_old, thread_end_new)

with open("src/main.rs", "w") as f:
    f.write(content)

print("CLI Patch applied!")
