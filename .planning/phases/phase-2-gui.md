# Phase 2: Desktop GUI Transition

## Goal
Replace the unstable macOS menu bar (`tray_icon`) with a lightweight, beautiful desktop window using `egui` (via `eframe`).

## Implementation Details
1. **Remove Old Dependencies**: Strip out `tao` and `tray-icon`.
2. **Add GUI Dependencies**: Add `eframe` (egui) and `image` (for the logo).
3. **App State**: Define a `ThockApp` struct containing `volume`, `current_pack`, `available_packs`.
4. **Threading**:
   - Thread 1: `eframe` main UI window (blocks main thread).
   - Thread 2: `CGEventTap` global keyboard hook (RunLoop).
   - Thread 3: Audio mixer thread (Rodio).
5. **UI Design**:
   - Dark mode default.
   - Large, clear volume slider.
   - Visual list of sound packs with "Favorite" and "Delete" actions.
   - Status warning if Accessibility permissions are blocked.

## Verification
- App compiles and opens a native macOS window.
- RAM usage remains under 30MB.
- Keystrokes are still intercepted with zero noticeable latency.
