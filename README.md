# 🎧 Thock (for macOS)

A blazing-fast, ultra-lightweight mechanical keyboard sound simulator built natively for macOS in **Rust**. 

Unlike other Electron-based sound simulators that consume a massive amount of RAM and CPU, Thock intercepts keystrokes directly at the macOS kernel level using `core-graphics`, resulting in **zero perceivable latency** and virtually **0% CPU usage**.

### ✨ Features
* **Zero Input Lag:** Uses raw hardware keycodes for instantaneous audio playback.
* **DSP Custom Sounds:** Includes mathematically generated sound profiles (Zero-phase Low-Pass filters, Comb filters) to perfectly simulate physical keyboard mods (PE Foam, Tape Mod, Overlubed).
* **Smart Volume:** Applies a true cubic scaling algorithm so the volume slider actually matches human hearing.
* **Persistent Settings:** Automatically remembers your exact pack and volume when you restart.
* **Drop-in Support:** Compatible with standard Mechvibes sound packs.

### 🚀 How to Run
Make sure you have Rust installed, then simply run:
```bash
cargo run --release
```
_Note: On macOS, you will be prompted to grant Accessibility permissions so the app can detect your keystrokes globally._

### 📜 Sound Pack Credits
The code for this application is licensed under the MIT License. The audio files located in the `packs/` directory are community-sourced (originally built for Mechvibes) and belong to their respective creators and artists.
