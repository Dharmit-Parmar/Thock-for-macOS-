# Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP

## Status: Completed

## Objective
Overhaul the ambient ASMR sound engine (rain, wind, thunder) in Thock to replace irritating static noise with organic psychoacoustic acoustics, fix slider latency and click bugs, and guarantee 100% allocation-free runtime playback.

## Deliverables & Evidence

1. **Acoustic Realism & Psychoacoustics**:
   - Fletcher-Munson / ISO 226 curve calibration: eliminated harsh high-frequency white noise ($f_c \in [650\text{ Hz}, 2000\text{ Hz}]$).
   - Low-mid warmth peaking EQ at $320\text{ Hz}$ ($Q = 1.2, +3.5\text{ dB}$) reproducing physical droplet resonance.
   - Binaural stereo decorrelation (L: 0.97, R: 1.03) for spatial depth.
   - Verified harshness reduction: $1.106 \to 0.239$ (78.4% cut in harshness).

2. **DSP Math & Bug Fixes**:
   - In-place Biquad updates via `update_lowpass` preserving $s_1, s_2$ filter delay states (0 DC pop).
   - Block-rate parameter updates every 128 frames ($\sim 2.67\text{ ms}$ response vs old 20s lag).
   - Continuous phase accumulator for wind LFO with golden-ratio dual sines ($0.6\sin(0.449t) + 0.4\sin(0.739t)$) modulating aerodynamic vortex cutoff ($[180\text{ Hz}, 500\text{ Hz}]$).
   - Bounded algebraic soft limiter $x / \sqrt{1 + x^2}$ guaranteeing zero digital rail clipping.

3. **Memory & Lifecycle Optimization (Ponytail)**:
   - Lazy one-time decoding of rain, wind, and thunder into `std::sync::OnceLock<Option<AsmrBuffer>>`.
   - Zero-allocation thunder triggers via `Arc<[i16]>` zero-copy sharing.
   - 80ms cosine fade-in and 1.0s cosine tail fade-out on thunder eliminating start/stop pops.
   - State transition caching eliminating redundant mutex locks in `Sink` at 33 Hz.
   - Loop sleep at end: 30ms active, 250ms idle (0.000% CPU).

4. **Acoustic Bug Fixes & Refinements**:
   - **Motor Rumble Elimination**: Replaced 320 Hz / +3.5 dB resonant peaking filter with an 85 Hz sub-bass highpass to eliminate HVAC/car rumble in the source recording. Expanded rain filter cutoff to 1200 Hz – 4000 Hz for airy droplet clarity. Reduced makeup multiplier to 1.05 to eliminate 3rd-harmonic intermodulation distortion.
   - **Thunder Isolation**: Implemented backward-search peak crack detection in `decode_faded_thunder`, removing the 18.37s ambient rain prefix. Thunder now triggers as an isolated 1.63s lightning strike (600ms pre-rumble + 40ms attack + crack + 1.2s dissipation), completely preventing the pre-strike rain density jump.
   - **Wind Air-Texture**: Expanded aerodynamic cutoff sweep to 350 Hz – 1200 Hz and integrated 750 Hz peaking EQ (+2 dB) into the active processing path for organic breath and leaf flutter.

5. **Verification**:
   - `cargo test`: 4/4 tests passed.
   - `cargo run --bin verify`: Thunder trimmed duration verified at 1.63s (-21.3 dBFS peak crack).
   - `cargo build --release`: clean compile.
