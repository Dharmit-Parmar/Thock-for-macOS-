<p align="center">
  <img src="https://github.com/Dharmit-Parmar/Thock-for-macOS-/raw/main/thock_app_icon_1789644171932.jpg" alt="Thock Logo" width="200" style="border-radius: 20px;"/>
</p>

<h1 align="center">Thock for macOS 🎧</h1>

Bring the satisfying, premium sounds of custom mechanical keyboards directly to your Mac. Thock is a hyper-optimized, zero-latency desktop application built in Rust. It captures your keystrokes universally and plays back gorgeous keyboard sound profiles.

## 🚀 Two Ways to Use Thock

### 1. Download the Mac App (Easiest - No Coding Required)
If you just want to use the app with the beautiful glass-pane interface, you don't need to touch the terminal or clone anything!

1. **[Click here to download Thock-macOS.zip](https://github.com/Dharmit-Parmar/Thock-for-macOS-/raw/main/Thock-macOS.zip)**
2. Unzip it and drag `Thock.app` to your `Applications` folder.
3. Open the app. 

### ⚙️ macOS Accessibility Permissions (Required)
Because Thock needs to listen for your keystrokes globally to play sounds, macOS requires you to grant it Accessibility permissions.

1. Open **System Settings** -> **Privacy & Security** -> **Accessibility**.
2. Toggle the switch ON for **Thock**.
3. *(If you don't see it, click the `+` button at the bottom, navigate to your Applications folder, and select Thock.app).*
4. **Restart the app** if needed!

### 2. Lightweight Terminal Mode (For Developers)
Want to run the app headless with absolute minimum overhead? You can run it directly from your terminal!

```bash
# Clone the repository
git clone https://github.com/Dharmit-Parmar/Thock-for-macOS-.git
cd Thock-for-macOS-

# Run the app in ultra-lightweight CLI mode
cargo run --release -- --cli
```
*In CLI mode, the app uses virtually zero memory as the GUI is entirely disabled. Just hit `Ctrl+C` to quit.*

## 🛠 Features
- **Zero Latency**: Powered by macOS CoreGraphics and the `rodio` Rust audio engine.
- **Beautiful UI**: Designed with TailwindCSS and Mac glassmorphism.
- **Background Mode**: Just hit the Red Close button, and the app retreats silently to your Dock.
- **Dynamic Volumes**: Full granular control over your switch volumes.
- **Favorites**: Click the star on any sound pack to pin it to the top of your list forever.

## 🗂 Custom Sound Packs
To add your own switch sounds, simply create a folder in your `packs/` directory with a `config.json` that maps your files, and Thock will automatically load them.
