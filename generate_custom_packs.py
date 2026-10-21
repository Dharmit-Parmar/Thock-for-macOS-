#!/usr/bin/env python3
"""
KeySound Custom Sound Pack Synthesizer: Ultra-Buttery & Creamy Edition
Generates 100% authentic, physical-recording-based sound packs from nk_cream:
1. creamy_butter     - Ultra-buttery, deeply cushioned, zero-clack linear switches
2. creamy_lite_calm  - Whisper-soft, peaceful, pillowy cream switches

Uses 100% Python standard library: wave, struct, math, os, json, shutil.
Zero external dependencies.
"""

import os
import sys
import wave
import struct
import math
import json
import shutil
import glob

SOURCE_DIR = "packs/nk_cream"

def read_wav(filepath):
    with wave.open(filepath, "rb") as w:
        nchannels = w.getnchannels()
        sampwidth = w.getsampwidth()
        framerate = w.getframerate()
        nframes = w.getnframes()
        data = w.readframes(nframes)
        
    num_samples = nframes * nchannels
    int_samples = struct.unpack(f"<{num_samples}h", data)
    
    if nchannels == 1:
        left = [s / 32768.0 for s in int_samples]
        right = list(left)
    else:
        left = [int_samples[i] / 32768.0 for i in range(0, num_samples, 2)]
        right = [int_samples[i + 1] / 32768.0 for i in range(0, num_samples, 2)]
        
    return left, right, framerate

def write_wav(filepath, left, right, framerate):
    n = min(len(left), len(right))
    out_samples = []
    for i in range(n):
        vl = max(-32768, min(32767, int(round(left[i] * 32767.0))))
        vr = max(-32768, min(32767, int(round(right[i] * 32767.0))))
        out_samples.append(vl)
        out_samples.append(vr)
        
    packed = struct.pack(f"<{len(out_samples)}h", *out_samples)
    with wave.open(filepath, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(framerate)
        w.writeframes(packed)

def zero_phase_lpf(signal, cutoff, fs):
    """Bidirectional single-pole low-pass filter (0 phase distortion)."""
    if cutoff >= fs * 0.49:
        return list(signal)
    rc = 1.0 / (cutoff * 2.0 * math.pi)
    dt = 1.0 / fs
    alpha = dt / (rc + dt)
    
    fwd = []
    prev = signal[0] if signal else 0.0
    for s in signal:
        prev += alpha * (s - prev)
        fwd.append(prev)
        
    bwd = []
    prev = fwd[-1] if fwd else 0.0
    for s in reversed(fwd):
        prev += alpha * (s - prev)
        bwd.append(prev)
        
    return bwd[::-1]

def find_strike_onset(left, right, fs, thresh_ratio=0.08):
    """Detects mechanical impact onset above background noise, backtracks to zero crossing."""
    n = min(len(left), len(right))
    peak = max(max(abs(s) for s in left), max(abs(s) for s in right))
    if peak < 1e-4:
        return 0
    thresh = peak * thresh_ratio
    strike = 0
    for i in range(n - 8):
        rms = math.sqrt(sum(left[i+j]**2 + right[i+j]**2 for j in range(8)) / 16.0)
        if rms >= thresh:
            strike = i
            break
            
    zc = strike
    for k in range(strike, max(0, strike - 64), -1):
        if (left[k] * left[k-1] <= 0.0) or (right[k] * right[k-1] <= 0.0):
            zc = k
            break
    return zc

def soft_limit(x):
    """Smooth algebraic soft saturation."""
    if abs(x) < 0.75:
        return x
    return math.copysign(0.75 + 0.25 * math.tanh((abs(x) - 0.75) / 0.25), x)

# -------------------------------------------------------------
# Pack 1: ULTRA BUTTERY SMOOTH & FEATHER LIGHT (creamy_butter)
# -------------------------------------------------------------
def ultra_butter_channel(ch, fs):
    # 1. Low cut below 130 Hz (removes muddy subwoofer thump)
    lpf_sub = zero_phase_lpf(ch, 130.0, fs)
    ch_clean = [s - b for s, b in zip(ch, lpf_sub)]
    
    # 2. Extract deep buttery cushion band (220 Hz - 420 Hz)
    lpf_warm = zero_phase_lpf(ch_clean, 420.0, fs)
    lpf_low = zero_phase_lpf(ch_clean, 220.0, fs)
    cushion = [w - l for w, l in zip(lpf_warm, lpf_low)]
    
    # 3. Steep dual-stage low-pass (780 Hz + 900 Hz): 100% rolls off plastic clack & click
    lpf_top1 = zero_phase_lpf(ch_clean, 780.0, fs)
    lpf_top2 = zero_phase_lpf(lpf_top1, 900.0, fs)
    
    # 4. Enriched blend: silky body + warm buttery cushion
    blended = [0.50 * t + 1.50 * c for t, c in zip(lpf_top2, cushion)]
    
    # 5. Viscous Hydrodynamic Deceleration Envelope (2.8ms smooth raised-cosine from 0.0)
    attack_len = int(fs * 0.0028)
    for i in range(min(attack_len, len(blended))):
        env = 0.5 * (1.0 - math.cos(math.pi * i / attack_len))
        blended[i] *= env
        
    # 6. Smooth tail fade (10ms) to prevent ending DC pop
    tail_len = int(fs * 0.010)
    for i in range(min(tail_len, len(blended))):
        idx = len(blended) - 1 - i
        env = 0.5 * (1.0 - math.cos(math.pi * i / tail_len))
        blended[idx] *= env
        
    return blended

def transform_ultra_butter(left, right, fs):
    zc = find_strike_onset(left, right, fs)
    tl = left[zc:]
    tr = right[zc:]
    ol = ultra_butter_channel(tl, fs)
    or_ = ultra_butter_channel(tr, fs)
    return ol, or_

# -------------------------------------------------------------
# Pack 2: Creamy Lite Calm (creamy_lite_calm)
# -------------------------------------------------------------
def calm_channel(ch, fs):
    lpf_mid = zero_phase_lpf(ch, 420.0, fs)
    lpf_sub = zero_phase_lpf(ch, 140.0, fs)
    cushion = [m - s for m, s in zip(lpf_mid, lpf_sub)]
    
    lpf_top = zero_phase_lpf(ch, 1100.0, fs)
    blended = [0.65 * t + 0.85 * c for t, c in zip(lpf_top, cushion)]
    
    attack_len = int(fs * 0.0024)
    for i in range(min(attack_len, len(blended))):
        env = 0.5 * (1.0 - math.cos(math.pi * i / attack_len))
        blended[i] *= env
        
    tail_len = int(fs * 0.010)
    for i in range(min(tail_len, len(blended))):
        idx = len(blended) - 1 - i
        env = 0.5 * (1.0 - math.cos(math.pi * i / tail_len))
        blended[idx] *= env
        
    return blended

def transform_calm(left, right, fs):
    zc = find_strike_onset(left, right, fs)
    tl = left[zc:]
    tr = right[zc:]
    ol = calm_channel(tl, fs)
    or_ = calm_channel(tr, fs)
    return ol, or_

PACKS_CONFIG = {
    "creamy_butter": {
        "fn": transform_ultra_butter,
        "name": "Creamy & Buttery Smooth",
        "target_peak_db": -3.5,
    },
    "creamy_lite_calm": {
        "fn": transform_calm,
        "name": "Creamy Lite Calm",
        "target_peak_db": -4.0,
    }
}

def generate_all():
    wav_files = sorted(glob.glob(os.path.join(SOURCE_DIR, "*.wav")))
    if not wav_files:
        print(f"Error: No WAV files found in {SOURCE_DIR}")
        return
        
    print(f"Found {len(wav_files)} source switch recordings in {SOURCE_DIR}.")
    
    # Preload all source WAVs
    source_cache = {}
    for fp in wav_files:
        fn = os.path.basename(fp)
        source_cache[fn] = read_wav(fp)
        
    for pack_id, cfg in PACKS_CONFIG.items():
        dst_dir = os.path.join("packs", pack_id)
        os.makedirs(dst_dir, exist_ok=True)
        print(f"\nSynthesizing sound pack: '{cfg['name']}' ({pack_id})...")
        
        # 1. Generate config.json
        with open(os.path.join(SOURCE_DIR, "config.json"), "r") as f:
            pack_json = json.load(f)
        pack_json["id"] = pack_id
        pack_json["name"] = cfg["name"]
        with open(os.path.join(dst_dir, "config.json"), "w") as f:
            json.dump(pack_json, f, indent=2)
            
        # 2. Process all WAV files
        processed = {}
        max_peak = 0.0
        transform_fn = cfg["fn"]
        
        for fn, (left, right, fs) in source_cache.items():
            tl, tr = transform_fn(left, right, fs)
            file_peak = max(max(abs(s) for s in tl), max(abs(s) for s in tr))
            if file_peak > max_peak:
                max_peak = file_peak
            processed[fn] = (tl, tr, fs)
            
        # 3. Master Pack Normalization
        target_amp = 10.0 ** (cfg["target_peak_db"] / 20.0)
        gain = target_amp / max(max_peak, 1e-4)
        print(f"  Max unprocessed peak: {max_peak:.4f} -> Applying calibrated gain {gain:.3f}x")
        
        for fn, (tl, tr, fs) in processed.items():
            final_l = [soft_limit(s * gain) for s in tl]
            final_r = [soft_limit(s * gain) for s in tr]
            out_path = os.path.join(dst_dir, fn)
            write_wav(out_path, final_l, final_r, fs)
            
        print(f"  ✓ Successfully wrote {len(processed)} WAV files + config.json to '{dst_dir}'")

    print("\nAll custom packs successfully synthesized!")

if __name__ == "__main__":
    generate_all()
