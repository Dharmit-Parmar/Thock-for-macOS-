<h1 align="center">Thock for macOS 🎧</h1>

<p align="center">
  Bring the satisfying, premium sounds of custom mechanical keyboards directly to your Mac. Thock is a hyper-optimized, zero-latency desktop application built in Rust. It captures your keystrokes universally and plays back gorgeous keyboard sound profiles.
</p>

## ✨ New in v2: Extreme Memory Optimization & Procedural Audio

Thock has been entirely re-architected to be a perfect macOS citizen:
* **Background Memory Drop:** When you close the main settings window, Thock drops its WebKit instance, plunging its memory usage from ~200MB down to a barely noticeable **~15MB**. It continues running perfectly in the background. Clicking the Dock icon instantly brings the window back.
* **Procedural Sound Engine:** Don't want to use `.wav` files? Thock now includes a real-time mathematical DSP engine. You can dynamically tweak spring weight, case material, o-rings, and foam modifications in the CLI to generate infinite switch profiles dynamically.
* **Separated Binaries:** Thock is now split into two clean targets: a gorgeous Glassmorphism GUI (`thock-app`) and an ultra-minimal headless terminal daemon (`thock-cli`).

---

## 🚀 Installation & Usage

### Option 1: Building the Mac App Bundle (Recommended)
You can easily build a native `Thock.app` bundle that you can place in your `/Applications` folder.

```bash
# 1. Clone the repository
git clone https://github.com/Dharmit-Parmar/Thock-for-macOS-.git
cd Thock-for-macOS-

# 2. Build the app bundle
./build_mac_app.sh

# 3. Move the generated Thock.app to your Applications folder
mv Thock.app /Applications/

# 4. Open the App!
open /Applications/Thock.app
```

### Option 2: Running the GUI directly via Cargo
```bash
cargo run --release --bin thock-app
```

### Option 3: Lightweight CLI Mode (Headless)
If you want zero GUI overhead from the start, run the interactive terminal interface:
```bash
cargo run --release --bin thock-cli
```
*Type `help` in the CLI to see volume and procedural sound commands.*

---

## ⚙️ macOS Accessibility Permissions (Required)
Because Thock needs to listen for your keystrokes globally in order to play the corresponding sounds, macOS strictly requires you to grant it Accessibility permissions.

1. Open **System Settings** -> **Privacy & Security** -> **Accessibility**.
2. Toggle the switch ON for **Thock** (or your Terminal emulator if running via Cargo).
3. *(If you don't see it, click the `+` button at the bottom, navigate to your Applications folder, and select Thock.app).*
4. **Restart the app** after granting permissions!

*(Note: Thock does not log, save, or transmit your keystrokes. The event tap purely triggers localized audio playback).*

---

## 🛠 Features

- **Zero Latency**: Powered by macOS CoreGraphics native event taps and the `rodio` Rust audio engine.
- **Smart Window Lifecycle**: Hitting the Red Close button fully tears down the WebKit renderer, freeing hundreds of megabytes of RAM while your sounds continue playing seamlessly in the background.
- **Dynamic Volumes**: Granular slider control over your master switch volumes.
- **Favorites**: Click the star on any sound pack to pin it to the top of your list forever.
- **Custom Sound Packs**: Add your own switch sounds! Simply create a folder in the `packs/` directory with a `config.json` mapping your `.wav` files, and Thock will automatically load them.

