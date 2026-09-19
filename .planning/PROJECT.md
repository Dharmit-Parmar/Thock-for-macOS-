# Project: Thock (GUI Upgrade)

## Overview
A zero-latency mechanical keyboard sound simulator for macOS. Currently operates entirely in the background via a tray icon (navbar). We are now upgrading to a full lightweight Desktop UI while maintaining 0% idle CPU and minimal RAM footprint.

## Core Constraints
- **Absolute Minimum Latency:** Audio playback must not be blocked by the UI event loop.
- **Ultra-lightweight:** Minimal RAM and CPU footprint (e.g. `egui` or lightweight renderer instead of full Electron/Tauri if possible, or heavily optimized Tauri if requested).
- **macOS Native:** Must continue to bundle seamlessly into `Thock.app`.
