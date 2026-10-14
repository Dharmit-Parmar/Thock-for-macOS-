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
- Rain starts from the licensed field recording with a short volume ramp. A 120 Hz high-pass reduces recording rumble; playback caches 90 seconds and loops through the later half with an equal-power crossfade.
- Wind uses block-rate filter updates and a continuous gust LFO.
- Rain, wind, and thunder buffers are decoded once and cached; rain uses about 7.6 MiB of decoded PCM.
- **Recorded Rain Texture**: The four-minute source recording has 18.4 dB less sub-80Hz RMS and 7.5 dB less energy above 4kHz than the old rain asset; 300Hz–4kHz rain energy is 2.2 dB higher.
- **Rain Onset**: Sparse drops fade out as the real recording fades in over four seconds. The recording begins at its natural opening and uses a short click-prevention fade.
- **Rolling Thunderclap**: Plays a 41.2s lightning-to-rumble event independently of rain, with a 40ms attack and 300ms tail fade. Re-enabling resumes the sink; event timing varies with the frequency control.
- Release build and asset verifier pass. Listening validation of the refreshed app remains pending; unit tests have not been rerun after the latest changes.


