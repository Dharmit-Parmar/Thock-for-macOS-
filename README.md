<div align="center">
  <h1>🎧 Thock <br><sub>Mechanical Keyboard Simulator for macOS</sub></h1>
  
  <p>
    <b>A blazing-fast, ultra-lightweight typing sound simulator built natively in Rust.</b>
  </p>

  <p>
    <img src="https://img.shields.io/badge/macOS-000000?style=for-the-badge&logo=apple&logoColor=white" alt="macOS Badge" />
    <img src="https://img.shields.io/badge/Rust-B7410E?style=for-the-badge&logo=rust&logoColor=white" alt="Rust Badge" />
    <img src="https://img.shields.io/badge/License-MIT-blue?style=for-the-badge" alt="MIT License" />
  </p>
</div>

<br>

> ⚡ **Why Thock?** Unlike heavy Electron-based apps that drain your battery, **Thock** intercepts keystrokes directly at the macOS kernel level using `core-graphics`. The result is **zero perceivable latency** and virtually **0% CPU footprint**.

<br>

## ✨ Features

<table>
  <tr>
    <td>🚀 <b>Zero Input Lag</b></td>
    <td>Uses raw hardware keycodes for instantaneous audio playback perfectly synced to your fingers.</td>
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

Ensure you have **Rust** installed on your Mac, then simply clone the repository and run:

```bash
cargo run --release
```

> **Note:** On macOS, you will be prompted to grant Accessibility permissions. This is required so the app can detect your keystrokes globally outside of the terminal.

<br>

## 📜 Intellectual Property & Credits

*   **Codebase:** The application code is open-source and licensed under the [MIT License](LICENSE).
*   **Audio Assets:** The `.wav` and `.ogg` files located in the `packs/` directory are community-sourced (originally built for the Mechvibes ecosystem) and belong to their respective creators and artists.
