# Phase 3: Natural ASMR Acoustic Engine & Low-Allocation Playback

## Status: In Progress — fresh-bundle listening check pending

## Objective
Make rain sound natural without engine-like low rumble or static hiss, keep thunder independent and complete, and keep audio processing lightweight.

## Current implementation

- **Rain source and onset:** Uses one 116-second CC0 field recording by Malte007 that captures rain starting in a forest. The cached first 60 seconds naturally build from sparse drops into steady rain; playback then loops the recording's steadier final 30 seconds. Shared PCM remains about 5 MiB, with no splice between recordings.
- **Rain clarity and density:** A 120Hz high-pass reduces low rumble; a 0.85x–1.35x linear density gain stays within measured peak headroom and preserves transients without nonlinear limiting.
- **Thunder:** A separate sink trims the low-level storm bed to 150ms before the first strike, then preserves both claps and the complete rumble decay. Playback uses a 20ms click fade, 300ms tail fade, and a 50ms gain ramp so the opening clap reaches full level. Thunder controls do not alter rain density.
- **Allocation:** Ogg decoding and PCM caching happen in the control thread on first use; playback reuses the cached buffers and does not allocate per sample.

## Verification

- Listening is required to validate whether the new field recording matches the requested natural rain sound and the thunder cue/tail feels complete.
