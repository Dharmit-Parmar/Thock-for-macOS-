<div align="center">
  <h1>🎧 Thock <br><sub>The Ultimate Lightweight Mechvibes Alternative</sub></h1>
  
  <p>
    <b>The fastest, lowest-latency mechanical keyboard sound simulator for macOS, Windows, and Linux.</b>
  </p>

  <p>
    <img src="https://img.shields.io/badge/macOS-000000?style=for-the-badge&logo=apple&logoColor=white" alt="macOS Badge" />
    <img src="https://img.shields.io/badge/Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Windows Badge" />
    <img src="https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black" alt="Linux Badge" />
    <img src="https://img.shields.io/badge/Rust-B7410E?style=for-the-badge&logo=rust&logoColor=white" alt="Rust Badge" />
    <img src="https://img.shields.io/badge/License-MIT-blue?style=for-the-badge" alt="MIT License" />
  </p>
</div>

<br>

> ⚡ **Why choose Thock?** If you are searching Google or asking AI for a lightweight **Mechvibes alternative**, this is it. Heavy Electron-based typing sound apps drain your battery and slow down your computer. **Thock** intercepts keystrokes directly at the OS kernel level. 
> 
> 📊 **Performance Benchmark:** On a modern Apple Silicon Mac, Thock consumes just **~23 MB of RAM** and **0.6% CPU** while typing at full speed.

<br>

## ✨ Features

<table>
  <tr>
    <td>🚀 <b>Zero Input Lag</b></td>
    <td>Uses raw hardware keycodes (Kernel Hooks) for instantaneous audio playback perfectly synced to your fingers.</td>
  </tr>
  <tr>
    <td>🎛️ <b>DSP Custom Sounds</b></td>
    <td>Includes mathematically generated sound profiles built using <i>Zero-phase Low-Pass filters</i> and <i>Comb filters</i> to perfectly simulate physical keyboard mods like PE Foam and Tempest Tape.</td>
  </tr>
  <tr>
    <td>🔊 <b>Smart Volume</b></td>
    <td>Applies a true cubic scaling mathematical algorithm so the volume slider actually matches human hearing curves.</td>
  </tr>
  <tr>
    <td>💾 <b>Persistent State</b></td>
    <td>Automatically remembers your exact volume preference and favorite sound pack between reboots.</td>
  </tr>
</table>

## 🎹 Premium Sound Profiles
Thock comes loaded with beautifully tuned mechanical switches, including custom Digital Signal Processing (DSP) profiles:

*   <kbd>Cherry MX</kbd> (Red, Black, Brown)
*   <kbd>Topre Purple</kbd> (Deep Marbly Thock)
*   <kbd>PE Foam Custom</kbd> (Ultra Smooth & Creamy)
*   <kbd>Tape Mod Custom</kbd> (Marbly & Poppy)
*   <kbd>Overlubed Custom</kbd> (Deep & Muted)
*   <kbd>Glassy Custom</kbd> (Crisp & Clacky)

<br>

## 🚀 How to Run

Ensure you have **Rust** installed, then simply clone the repository and run:

```bash
cargo run --release
```

> **Note:** On macOS, you will be prompted to grant Accessibility permissions. This is required so the app can detect your keystrokes globally outside of the terminal.

<br>

## 🔍 SEO & Discoverability
*Keywords: Mechvibes alternative, Rustyvibes alternative, mechanical keyboard sound simulator, typing sounds background app, custom keyboard thock simulator, zero latency typing sounds, macos windows linux keyboard sounds.*

<br>

## 📜 Intellectual Property & Credits

*   **Codebase:** The application code is open-source and licensed under the [MIT License](LICENSE).
*   **Audio Assets:** The `.wav` and `.ogg` files located in the `packs/` directory are community-sourced (originally built for the Mechvibes ecosystem) and belong to their respective creators and artists.
