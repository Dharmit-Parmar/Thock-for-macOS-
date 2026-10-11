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
- Unit tests & release binaries verified.

