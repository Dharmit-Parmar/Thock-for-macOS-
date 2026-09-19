# Thock for macOS 🎧

Bring the satisfying, premium sounds of custom mechanical keyboards directly to your Mac. Thock is a hyper-optimized, zero-latency desktop application built in Rust. It captures your keystrokes universally and plays back gorgeous keyboard sound profiles.

![Thock UI](https://github.com/Dharmit-Parmar/Thock-for-macOS-/assets/placeholder-screenshot.png)

## 🚀 Two Ways to Use Thock

### 1. Download the Mac App (Easiest - No Coding Required)
If you just want to use the app with the beautiful glass-pane interface, you don't need to touch the terminal or clone anything!

1. **[Click here to download Thock-macOS.zip](https://github.com/Dharmit-Parmar/Thock-for-macOS-/raw/main/Thock-macOS.zip)**
2. Unzip it and drag `Thock.app` to your `Applications` folder.
3. Open the app and grant Accessibility permissions when prompted.

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
