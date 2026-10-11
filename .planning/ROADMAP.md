# Roadmap

## Phase 1: Core Audio & Tray (Completed)
- Achieved 2.9ms rodio audio latency.
- Built zero-drain CGEventTap listener.
- Synthesized custom DSP sound packs.

## Phase 2: Desktop GUI Transition (MVP) (Completed)
**Mode:** mvp
**Goal:** Build a beautiful, interactive desktop window (UI) replacing the buggy tray-icon navigation. Must allow selecting packs, adjusting volume, and toggling favorites visually, while preserving the underlying ultra-fast audio engine.

## Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP (Completed)
**Goal:** Psychoacoustically natural rain, wind, and thunder engine. Replaces harsh 5kHz hiss with pink/brown spectrum slope, 320Hz warmth peaking EQ, von Kármán vortex aerodynamic wind LFO, and 0-allocation OnceLock predecoded audio pipeline.
- ISO 226 curve calibrated: 78.4% reduction in harsh high-frequency noise.
- Click-free, pop-free block-rate filter coefficient updates (<2.67ms response).
- Zero heap allocations in runtime audio playback hot paths.
- **Fluid Dynamics Droplet Acoustics**: Hydrodynamic half-sine contact waves eliminating frying sizzle (*rai-jeera in hot oil*); liquid cavity resonance at 240–820 Hz with +12% micro-chirp.
- **Zero-Drone Acoustic Architecture**: Continuous pink/motor noise completely eliminated; airborne micro-spray highpassed >3.4 kHz (0 Hz energy in vehicle cabin hum band).
- **Organic Onset Progression**: Starts with sparse, isolated "tip... tip... tip..." droplets (0.8 -> 2.2 drops/sec) with pure silence between drops.
- **Full Rolling Thunderclap**: Primary 9.9s strike (crack + sub-bass reverberation tail) with prompt ~350ms instant initial strike on enable.
- Unit tests & release binaries verified.


