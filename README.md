<div align="center">
  <img src="https://github.com/Dharmit-Parmar/Thock-for-macOS-/assets/your-banner-image-or-logo" width="150" alt="Thock Logo">
  <h1>⌨️ Thock</h1>
  <p><strong>A zero-latency, highly optimized mechanical keyboard sound simulator for macOS.</strong></p>
</div>

---

**Thock** intercepts your keystrokes globally on macOS and plays ultra-realistic, zero-latency mechanical keyboard sounds. It is built entirely in Rust, featuring extreme micro-optimizations (zero-copy audio loops, hardware Fused Multiply-Add DSP filters, and zero background RAM bloat) to ensure it runs completely invisibly on your machine.

It features both a **Command Line Interface (CLI)** with a beautiful interactive terminal menu, and a **GUI App** version. 

## 🚀 Recommended Installation (Terminal Setup)

The recommended way to install and run Thock is via the terminal. This ensures you always compile the freshest version tailored to your Mac's hardware and immediately installs the global `thock` command.

Simply open your terminal and run the following command:

```bash
curl -fsSL https://raw.githubusercontent.com/Dharmit-Parmar/Thock-for-macOS-/main/install.sh | bash
```

Once installed, you can type **`thock`** from anywhere in your terminal! 

It will instantly launch an interactive menu:
```text
⌨️  Thock Menu
==================
1) CLI Mode (thock-cli) [Recommended - Lightweight]
2) GUI App (thock-app)  [Visual]

Enter choice [1 or 2]: 
```

*Note: The CLI mode is highly recommended. Choosing `1` will drop you into a beautiful, arrow-key navigable menu to swap sound packs, adjust volume, and configure ASMR layers using exactly zero extra background RAM.*

## 📦 Direct App Download

If you prefer not to use the terminal installer, you can download the pre-compiled `.app` bundle.

1. Download the latest App Bundle: [**Thock-macOS.zip**](Thock-macOS.zip) (included in the repository)
2. Double-click to extract the `.zip`.
3. Drag **`Thock.app`** into your `Applications` folder.
4. *Important: You will need to grant Accessibility permissions in System Settings > Privacy & Security > Accessibility so Thock can hear your keystrokes.*

---

### ✨ Features
- **Zero Latency:** Pre-decoded OGG buffers loaded directly into RAM for instant audio response.
- **Micro-Optimized:** Lock-free atomic velocity tracking and macOS CoreGraphics event hooks tailored to prevent any memory leaks over time.
- **Procedural ASMR:** Generate rain, wind, and thunder ambient layers directly on your CPU using Fused Multiply-Add (FMA) math optimizations to save battery.
- **Interactive TUI:** Use your arrow keys right in the terminal to configure everything instantly.
