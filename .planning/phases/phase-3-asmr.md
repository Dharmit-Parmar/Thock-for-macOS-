# Phase 3: Natural ASMR Acoustic Engine & Low-Allocation Playback

## Status: In Progress — fresh-bundle listening check pending

## Objective
Make rain sound natural without engine-like low rumble or static hiss, keep thunder independent and complete, and keep audio processing lightweight.

## Current implementation

- **Rain source:** The first 90 seconds of the licensed four-minute stereo recording are decoded once into shared `Arc<[i16]>` PCM (about 7.6 MiB). Playback preserves the recorded opening, crossfades at the end into its later half, and then loops that 45-second section.
- **Rain onset:** The natural field recording starts immediately with a 250ms volume ramp. Synthetic resonant drops were removed because they sounded tonal and artificial.
- **Rain clarity and density:** A 120Hz high-pass filter reduces low recording rumble. The density control drives a bounded soft limiter from 2x to 4x, making the quiet recording denser while keeping peaks below full scale.
- **Recording measurements:** Against the old rain asset, the field recording has 18.4 dB less RMS below 80 Hz, 7.5 dB less above 4 kHz, and 2.2 dB more in the 300 Hz–4 kHz band. This is source-file analysis, not a subjective listening result.
- **Thunder:** A separate sink plays the full 41-second lightning-to-rumble clip. Playback trims to 150ms before the first strike (about 2.25s into the source), uses a 20ms attack and 300ms tail fade, and has a 250ms volume ramp. The sink resumes after being cleared.
- **Control timing:** The controller uses a 30ms active interval and 50ms idle interval, with envelope steps matched to the actual interval. Thunder controls do not write the rain-density setting.
- **Allocation:** Ogg decoding and the bounded rain PCM cache allocate in the control thread on first use. The sample iterator reuses cached buffers and does not allocate per sample.

## Verification

- `cargo build --release` succeeds.
- `cargo run --release --bin verify` reports the expected 90-second rain cache and 41.2-second decoded thunder event, plus asset peaks and RMS.
- Installed `/Applications/Thock.app` is signed and its executable hash matches the release bundle.
- Unit tests have not been rerun after the latest changes.
- Listening validation of rain onset, loop seam, density range, and thunder re-enable/tail remains pending.
