<div align="center">

# thock

**Zero-latency, hardware-optimized mechanical keyboard acoustic engine for macOS.**

[![Platform](https://img.shields.io/badge/platform-macOS%2012%2B-000000.svg?style=flat-square&logo=apple&logoColor=white)](https://github.com/Dharmit-Parmar/Thock-for-macOS-)
[![Language](https://img.shields.io/badge/language-Rust%202021-000000.svg?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Audio Engine](https://img.shields.io/badge/dsp-zero--copy%20i16%20PCM-000000.svg?style=flat-square)](https://github.com/Dharmit-Parmar/Thock-for-macOS-)
[![Memory Footprint](https://img.shields.io/badge/memory-%3C10MB%20RSS-000000.svg?style=flat-square)](https://github.com/Dharmit-Parmar/Thock-for-macOS-)
[![License](https://img.shields.io/badge/license-MIT-000000.svg?style=flat-square)](LICENSE)

<br />

```
thock  v0.1.0 · mechanical keyboard acoustic engine
──────────────────────────────────────────────────────
pack        cherry mx black pbt
volume      [████████░░] 80%
ambient     rain · wind
engine      zero-latency i16 pcm
──────────────────────────────────────────────────────
Press Enter for interactive menu, or type 'help'
```

</div>

---

## Overview

**Thock** intercepts keystrokes globally on macOS and synthesizes authentic, zero-latency mechanical switch acoustics in real-time. Built entirely in Rust, it couples low-level `CGEventTap` hooks directly with lock-free atomic velocity tracking and hardware-accelerated Fused Multiply-Add (FMA) digital signal processing.

Unlike heavyweight Electron-based sound wrappers that consume 150MB+ of RAM and suffer from audio buffer latency, Thock maintains an ultra-lean **~7–10MB resident memory footprint**, processes key events with **0ms perceptible latency**, and produces zero garbage collection or background audio buffer leaks.

Thock includes two seamless run modes:
- **CLI Mode (`thock-cli`)**: An interactive, arrow-key navigable terminal dashboard featuring real-time telemetry, volume meters, and instant acoustic switching.
- **GUI App (`thock-app`)**: A floating native glass window with real-time switch sliders and ambient sound toggles.

---

## Installation

### Method 1: One-Line Terminal Setup (Recommended)

The recommended installation compiles Thock directly for your Mac's CPU architecture (Apple Silicon M1/M2/M3/M4 or Intel) and configures the global `thock` command:

```bash
curl -fsSL https://raw.githubusercontent.com/Dharmit-Parmar/Thock-for-macOS-/main/install.sh | bash
```

Once installed, simply run `thock` from anywhere in your terminal:

```bash
thock
```

This launches the quick selection menu:

```text
thock · mechanical keyboard acoustic engine
────────────────────────────────────────────
1  CLI Mode  (terminal interactive)
2  GUI App   (desktop floating app)

choice [1/2]: 
```

> [!TIP]
> You can also launch either mode directly without prompts by passing arguments:
> ```bash
> thock cli    # Launches terminal interactive mode directly
> thock app    # Launches native desktop app directly
> ```

---

### Method 2: Direct App Download

If you prefer not to use the terminal installer, download the pre-compiled application bundle:

1. Download the latest release: [**Thock-macOS.zip**](Thock-macOS.zip) (bundled directly in this repository).
2. Double-click `Thock-macOS.zip` to extract `Thock.app`.
3. Drag **`Thock.app`** into your `/Applications` folder.
4. Launch `Thock.app` from Spotlight or Launchpad.

---

## macOS Accessibility Permissions

Because Thock listens to keyboard events globally across all applications, macOS requires explicit Accessibility permissions.

1. Open **System Settings** → **Privacy & Security** → **Accessibility**.
2. Click the **+** button (or toggle the switch).
3. Add your terminal application (e.g., **Terminal**, **iTerm2**, **Ghostty**, or **Alacritty**) if using the CLI, or **`Thock.app`** if using the GUI.
4. If permissions were previously granted but events are not registering, reset the permission cache via Terminal:
   ```bash
   tccutil reset Accessibility
   ```

---

## Interactive CLI Interface

The terminal interface provides keyboard-driven controls with zero CPU overhead. Hitting `Enter` at the prompt opens the interactive menu:

```text
? Action ›
❯ Pack         Switch sound pack
  Volume       Set master volume (0–100%)
  Ambient      Configure rain, wind & thunder
  Procedural   Tune physics synthesis parameters
  Status       Display engine diagnostics
  Exit         Quit thock
```

### REPL Commands

When not in the interactive menu, Thock accepts direct commands at the `thock › ` prompt:

| Command | Arguments | Description | Example |
|---|---|---|---|
| `pack` | `<name>` | Switch active sound pack | `pack cherry_mx_black_pbt` |
| `packs` | *none* | Open interactive pack picker | `packs` |
| `vol` | `<0–100>` | Adjust master playback volume | `vol 85` |
| `asmr` | *none* | Display ambient layers dashboard | `asmr` |
| `asmr` | `<layer> [on\|off]` | Toggle ambient sound layer | `asmr rain on` |
| `asmr` | `<prop> <0–100>` | Adjust layer parameter | `asmr rain_dens 75` |
| `proc` | *none* | Show procedural switch parameters | `proc` |
| `proc` | `<param> <val>` | Tune acoustic physical model | `proc lube 0.9` |
| `status` | *none* | Print full engine diagnostics | `status` |
| `help` | *none* | List all available commands | `help` |
| `exit` / `quit` | *none* | Stop audio engine and quit | `exit` |

---

## Sound Pack Library

Thock includes 16 curated acoustic profiles covering linear, tactile, clicky, modded, and procedural switches:

| Pack Identifier | Switch Type | Keycaps / Material | Acoustic Character |
|---|---|---|---|
| `cherry_mx_black_pbt` | Heavy Linear | Thick PBT | Deep, grounded bottom-out thock |
| `cherry_mx_brown_pbt` | Light Tactile | PBT | Muted tactile bump, soft return |
| `cherry_mx_brown_abs` | Light Tactile | ABS | Crisp, vintage tactile clack |
| `cherry_mx_red_abs` | Light Linear | ABS | Classic bright and snappy clack |
| `creamy_thock_v2` | Custom Linear | Hand-Lubed POM | Ultra-deep acoustic profile with dampening foam |
| `creamy_marbly` | Modded Linear | Tape Mod | High-pitched marbly pop with sharp resonance |
| `creamy_heavy` | Heavy Linear | Factory Lubed | Dense acoustic mass, deep low-end punch |
| `nk_cream` | Linear | Full POM Housing | Smooth self-lubricating POM slide and bottom-out |
| `nk_cream_loud` | Linear | POM Amplified | High-gain recording of NovelKeys Cream |
| `nk_cream_procedural` | Pure DSP | Mathematical Model | Real-time physical modal synthesis (zero audio files) |
| `pe_foam_creamy` | Modded Linear | PE Foam Sheet | Distinct poppy sound signature |
| `overlubed_custom` | Custom Linear | 205g0 Overlubed | Extremely muted, pillowy bottom-out |
| `glassy_custom` | Custom Tactile | Polycarbonate | High-frequency crystal clack |
| `eg_crystal_purple` | Clicky / Tactile | Polycarbonate | Sharp tactile snap with crystal clarity |
| `steelseries_apex_pro_v2`| Hall Effect | Dual Magnetic | Smooth magnetic actuation and steady rebound |
| `animal_crossing_nl` | Novelty | Synthesized | Playful Animal Crossing typewriter speech tone |

---

## Procedural Ambient & ASMR Engine

Thock features an integrated ambient sound engine designed to blend focus-enhancing environmental acoustics beneath your typing cadence:

- **Rain Layer**: A natural stereo field recording starts immediately with a short volume ramp. A 120 Hz high-pass filter reduces recording rumble, and bounded soft compression makes the intensity control audible without clipping. The first 90 seconds are cached and crossfaded for looping to keep decoded audio memory below 8 MB. Source credit and processing details are in `assets/asmr/SOURCES.md`.
- **Wind Layer**: Low-frequency resonant noise generator with dynamic gust oscillation.
- **Thunder Layer**: A 43.5-second field recording containing the lightning cue, two thunderclaps, and the full rumble decay. It is cached on first use, separate from rain, and credited in `assets/asmr/SOURCES.md`.

### Ambient Configuration Parameters

```bash
asmr rain on            # Activate rain layer
asmr rain_vol 70        # Set rain volume to 70%
asmr rain_dens 80       # Adjust perceived rain intensity
asmr wind on            # Activate wind layer
asmr wind_gust 60       # Adjust wind gust dynamics
asmr thunder on         # Activate distant thunder
asmr thunder_freq 40    # Set thunder occurrence frequency
asmr thunder_int 85     # Set thunder rumble intensity
asmr none               # Disable all ambient layers
```

---

## Procedural Switch Modeling (Pure DSP)

For zero-disk usage and infinite acoustic variation, the `nk_cream_procedural` engine uses real-time modal synthesis:

$$\text{Output}(t) = \sum_{k=1}^{M} A_k \cdot e^{-\alpha_k t} \cdot \sin(\omega_k t + \phi_k) + \text{Noise}_{\text{shaped}}(t)$$

- **Coupled-Form Modal Resonators**: Synthesizes stem impact, housing vibration, and plate reverberation.
- **Transposed Direct Form II (TDF-II) Biquads**: Stable recursive filtering with IEEE 754 32-bit floating-point headroom.
- **Continuous Velocity Scaling**: Keypress velocity dynamically modifies impact attack times, cutoffs, and resonant harmonic balance.

Adjust parameters live:
```bash
proc lube 0.95          # Dampen high frequencies and shorten decay
proc bottom_out 1.2     # Increase stem bottom-out impact energy
proc cutoff 3200        # Adjust low-pass filter cutoff (Hz)
proc resonance 0.85     # Adjust housing acoustic resonance
```

---

## Technical Architecture & Benchmarks

```
   [ macOS Hardware ]
           │
           ▼
   [ CGEventTap ] ──► CoreGraphics Global Hook (ListenOnly)
           │
           ▼
   [ Lock-Free Queue ] ──► Atomic Velocity & Timing Estimator
           │
   ┌───────┴──────────────────────────────┐
   ▼                                      ▼
[ PredecodedAudio ]              [ ProceduralSwitch ]
• Arc<[i16]> PCM Samples         • Coupled-Form Modal Bank
• Zero-Copy Audio Slices         • Hardware FMA (mul_add)
• Deduplicated Path Cache        • TDF-II Biquad Filters
   │                                      │
   └───────────────┬──────────────────────┘
                   ▼
         [ Rodio Audio Engine ]
         • Hardware Sample Rate Matching
         • Zero-Latency Mixing Bus
                   │
                   ▼
         [ CoreAudio Output ]
```

### Performance Comparison

| Metric | Thock (Rust) | Klack / Mechvibes (Electron) | Web-Based Simulators |
|---|---|---|---|
| **Audio Latency** | **< 1 ms (Imperceptible)** | 25–60 ms | 40–120 ms |
| **RAM Usage** | **~7–10 MB RSS** | 160–320 MB | 200–500 MB |
| **CPU Usage** | **< 0.2% on typing** | 3.5–8.0% | 5.0–15.0% |
| **Binary Footprint** | **~12 MB compiled** | 180+ MB | N/A |
| **Memory Retention** | **Deterministic drop** | Leaks over long sessions | Browser GC stalls |

---

## Building from Source

### Prerequisites

- macOS 12.0 (Monterey) or later
- [Rust & Cargo](https://rustup.rs/) (1.70+)
- Xcode Command Line Tools (`xcode-select --install`)

### Compilation

```bash
# Clone the repository
git clone https://github.com/Dharmit-Parmar/Thock-for-macOS-.git
cd Thock-for-macOS-

# Build the release binaries
cargo build --release

# Run CLI mode
cargo run --release --bin thock-cli

# Run GUI mode
cargo run --release --bin thock-app
```

---

## License

This project is licensed under the [MIT License](LICENSE).
Mechanical switch audio recordings belong to their respective sound designers and communities.
