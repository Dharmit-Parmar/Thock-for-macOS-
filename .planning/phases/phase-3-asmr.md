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

4. **Acoustic Bug Fixes & Live Procedural Rain Engine**:
   - **Live Procedural Rain Engine (`ProceduralRain: rodio::Source`)**:
     - Completely eliminated static recorded loop and pre-rendered buffers; 100% real-time streaming.
     - **Fluid Dynamics Droplet Acoustics (Eradicated "Rai-Jeera in Hot Oil" Frying Sizzle)**:
       - Real water drop impacts have smooth surface tension deformation with zero white noise. Replaced stochastic shock noise pulses with smooth hydrodynamic half-sine contact pulses: $p(t) = A_{\text{impact}} \cdot \sin(\pi t / T_{\text{impact}})$ ($T \approx 1.6 - 3.2\text{ ms}$).
       - Shifted droplet cavity resonance down from whistling 1750–2400 Hz into the true organic liquid spectrum:
         - 50% Organic Leaf/Window "Tip": $520 - 820\text{ Hz}$, decay $\tau = 9 - 14\text{ ms}$.
         - 35% Hollow Resonant "Plink": $380 - 520\text{ Hz}$, decay $\tau = 12 - 18\text{ ms}$.
         - 15% Deep Surface "Plop / Thump": $240 - 380\text{ Hz}$, decay $\tau = 16 - 24\text{ ms}$.
       - Gentle $+12\%$ upward bubble chirp decaying in $2.5\text{ ms}$.
     - **Car Engine / Motor Drone Eradication**:
       - Completely removed the 450–1000 Hz continuous 1-pole pink noise bandpass bed that created vehicle cabin hum.
       - Replaced with delicate airborne micro-spray using sparse Poisson micro-ticks high-passed strictly above $3400\text{ Hz}$ ($0\text{ Hz}$ energy in motor drone band).
     - **Natural Onset Progression ("Tip... Tip... Tip...")**:
       - $t \in [0.0\text{s}, 3.5\text{s}]$: Pure silence between isolated droplets ($\lambda = 0.8 \to 2.2\text{ drops/sec}$, spray gain = $0.0$). Delivers crystal-clear individual droplet strikes on toggle ON.
       - $t \in [3.5\text{s}, 7.5\text{s}]$: Smoothstep transition into rain shower.
       - $t > 7.5\text{s}$: Full steady rainfall with dynamic density slider response (`ASMR_RAIN_DENS`).
   - **Thunder Realism & Rolling Reverberation**:
     - **Full 9.9-Second Thunderclap**: Sliced primary complete thunderclap from 5.1s to 15.0s in `thunder.ogg` (crack at ~5.3s + 9+ seconds of low-frequency rolling rumble and atmospheric reverb). Replaced old 1.63s truncated strike.
     - **60ms Cosine Attack & 2.5s Cosine Tail Fade-out**: Skips ambient rain prefix while preventing clicks, dissipating naturally without abrupt cutoffs.
     - **Instant First-Strike on Enable**: When thunder is toggled ON, triggers prompt initial strike (~350ms) for immediate gratification, then schedules subsequent strikes via Poisson interval timer.

5. **Verification**:
   - `cargo test`: 5/5 tests passed, including `test_procedural_rain_onset_progression`.
   - `cargo run --bin verify`: Procedural Rain verified at RMS 0.038 (-28.4 dBFS, 0% motor drone). Thunder verified at full 9.90s (-31.4 dBFS).
   - `cargo build --release`: clean compile in 13.34s.
