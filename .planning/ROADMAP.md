# Roadmap

## Phase 1: Core Audio & Tray (Completed)
- Achieved 2.9ms rodio audio latency.
- Built zero-drain CGEventTap listener.
- Synthesized custom DSP sound packs.

## Phase 2: Desktop GUI Transition (MVP) (Completed)
**Mode:** mvp
**Goal:** Build a beautiful, interactive desktop window (UI) replacing the buggy tray-icon navigation. Must allow selecting packs, adjusting volume, and toggling favorites visually, while preserving the underlying ultra-fast audio engine.

## Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP (In Progress)
**Goal:** Natural rain, wind, and thunder with low-allocation streaming playback.
- Rain uses one natural field recording that grows from sparse drops into steady rain, eliminating the cross-recording splice. A 120 Hz high-pass reduces low rumble; linear density gain stays within measured peak headroom to preserve transients. Playback caches 60 seconds (about 5 MiB PCM) and loops the steadier final 30 seconds with an equal-power crossfade.
- Wind uses block-rate filter updates and a continuous gust LFO.
- Rain, wind, and thunder buffers are decoded once and cached; rain uses about 5 MiB of decoded PCM.
- **Rain Onset**: The sparse-drop intro plays once before the steady recording loops.
- **Rolling Thunderclap**: A separate recording starts 150ms before the first strike and retains both claps and the full rumble tail, with a 20ms attack and 300ms release.
- Listening validation of the refreshed app remains pending.


