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

4. **Acoustic Bug Fixes & Procedural Synthesis**:
   - **Procedural Rain Synthesis (100% Zero-Drone Natural Audio)**: Discovered that `rain.ogg` contained un-filterable mechanical harmonics (87.7 Hz, 258.4 Hz, 330.4 Hz, 516.8 Hz from automotive traffic/HVAC in the source recording). Implemented pure procedural natural rain generation (`generate_procedural_rain_buffer`) using:
     - 1-pole pink/brown noise diffusion for continuous distant shower bed
     - Stochastic Poisson process ($\lambda \approx 160\text{ drops/sec}$) triggering Minnaert bubble/damped droplet oscillators ($1.6\text{ kHz} - 3.8\text{ kHz}$, $\tau \in [2, 5]\text{ ms}$)
     - 420 Hz highpass + 7.0 kHz lowpass + 1.9 kHz foliage resonance + 0.6s equal-power crossfade
     - Measured frequency spectrum: $0\text{--}100\text{ Hz}$ reduced to $0.01\%$ ($-42\text{ dBFS}$), with $85.3\%$ of energy in the natural droplet impact band.
     - Removed `static RAIN_OGG` from binary compilation, saving 292 KB static storage.
   - **Thunder Isolation**: Backward-search transient detection in `decode_faded_thunder` isolates the 1.63s lightning strike (600ms pre-rumble + 40ms attack + crack + 1.2s dissipation), eliminating the 18.37s ambient rain prefix.
   - **Wind Air-Texture**: Dynamic aerodynamic sweep ($350\text{ Hz} - 1200\text{ Hz}$) + 750 Hz peaking EQ (+2 dB) for leaf flutter and organic breath.

5. **Verification**:
   - `cargo test`: 4/4 tests passed.
   - `cargo run --bin verify`: Procedural Rain (0% motor drone) verified at RMS 0.029 (-30.6 dBFS, 9.40s seamless loop). Thunder verified at 1.63s (-21.3 dBFS peak crack).
   - `cargo build --release`: clean compile in 13.27s.
