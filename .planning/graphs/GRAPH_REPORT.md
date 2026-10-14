# Graph Report - KeySound  (2026-10-14)

## Corpus Check
- 23 files · ~58,809 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 160 nodes · 308 edges · 20 communities (12 shown, 8 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 4 edges (avg confidence: 0.9)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- Audio Decoding & Buffer Pipeline
- Streaming Sources & Iterators
- Interactive CLI & Pack Parsing
- Acoustic Specifications & System Planning
- Digital Signal Processing Biquads
- Procedural Droplet Physics Engine
- Procedural Mechanical Switch Synthesis
- Resonant Modal Synthesis & Chladni Modes
- Pseudo-Random Number Generation
- Zero-Latency Synthesis Foundations
- macOS Keystroke Mapping Subsystem
- Direct Or Speed Source Combinator
- Audio Verification & Diagnostic Suite
- macOS Application Packaging Script
- System Installation Automation
- Desktop GUI Transition Framework
- Thock Project Identity & Core

## God Nodes (most connected - your core abstractions)
1. `ProceduralRain` - 14 edges
2. `ProceduralSwitch` - 13 edges
3. `SeamlessLoopSource` - 13 edges
4. `Biquad` - 10 edges
5. `run()` - 10 edges
6. `ThockHelper` - 9 edges
7. `ZeroCopySource` - 9 edges
8. `load_pack()` - 9 edges
9. `AsmrBuffer` - 8 edges
10. `decode_audio()` - 8 edges

## Surprising Connections (you probably didn't know these)
- `Zero Latency Constraint` --rationale_for--> `Procedural Audio Synthesis`  [INFERRED]
  .planning/PROJECT.md → DSP_ENGINE_RESEARCH.md
- `Thock Web Dashboard Controller` --conceptually_related_to--> `Thock Native macOS Glass GUI App`  [INFERRED]
  ui.html → README.md
- `Knowledge Graph Analysis Report` --references--> `Thock Project`  [INFERRED]
  .planning/graphs/GRAPH_REPORT.md → .planning/PROJECT.md
- `Thock Architecture Overview` --conceptually_related_to--> `Thock Project`  [EXTRACTED]
  README.md → .planning/PROJECT.md
- `SeamlessLoopSource` --references--> `Biquad`  [EXTRACTED]
  src/lib.rs → src/dsp.rs

## Import Cycles
- None detected.

## Communities (20 total, 8 thin omitted)

### Community 0 - "Audio Decoding & Buffer Pipeline"
Cohesion: 0.17
Nodes (21): Path, AsmrBuffer, decode_audio(), decode_thunder(), get_rain_buffer(), get_thunder_buffer(), get_wind_buffer(), IpcMessage (+13 more)

### Community 1 - "Streaming Sources & Iterators"
Cohesion: 0.10
Nodes (13): Arc, Item, AsmrLoopKind, DirectOrSpeed<A, B>, PredecodedAudio, rain_density_gain(), Duration, Iterator (+5 more)

### Community 2 - "Interactive CLI & Pack Parsing"
Cohesion: 0.12
Nodes (18): Completer, Context, Default, HashMap, Highlighter, Hinter, Pair, ReadlineError (+10 more)

### Community 3 - "Acoustic Specifications & System Planning"
Cohesion: 0.14
Nodes (14): Knowledge Graph Interactive Visualizer, Knowledge Graph Analysis Report, GSD Inbox Triage Report, ISO 226 Fletcher-Munson Psychoacoustic Calibration, Procedural Rain Acoustic Engine, Thunder Acoustic Rolling Reverberation, Thock Project, Phase 1: Core Audio & Tray (+6 more)

### Community 5 - "Procedural Droplet Physics Engine"
Cohesion: 0.28
Nodes (5): DropletVoice, generate_procedural_rain_buffer(), ProceduralRain, Iterator, Source

### Community 6 - "Procedural Mechanical Switch Synthesis"
Cohesion: 0.36
Nodes (3): ProceduralSwitch, Duration, Option

### Community 9 - "Zero-Latency Synthesis Foundations"
Cohesion: 0.50
Nodes (4): Zero Latency Constraint, Procedural Audio Synthesis, TDF-II Biquad Filter, Xorshift PRNG

### Community 10 - "macOS Keystroke Mapping Subsystem"
Cohesion: 0.50
Nodes (3): OnceLock, get_key_map(), Option

### Community 11 - "Direct Or Speed Source Combinator"
Cohesion: 0.67
Nodes (3): A, B, DirectOrSpeed

## Knowledge Gaps
- **13 isolated node(s):** `thock`, `build_mac_app.sh script`, `install.sh script`, `GSD Inbox Triage Report`, `Phase 1: Core Audio & Tray` (+8 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **8 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `SeamlessLoopSource` connect `Streaming Sources & Iterators` to `Audio Decoding & Buffer Pipeline`, `Digital Signal Processing Biquads`?**
  _High betweenness centrality (0.191) - this node is a cross-community bridge._
- **Why does `Biquad` connect `Digital Signal Processing Biquads` to `Streaming Sources & Iterators`, `Procedural Droplet Physics Engine`, `Procedural Mechanical Switch Synthesis`, `Resonant Modal Synthesis & Chladni Modes`?**
  _High betweenness centrality (0.168) - this node is a cross-community bridge._
- **Why does `ProceduralConfig` connect `Interactive CLI & Pack Parsing` to `Digital Signal Processing Biquads`, `Procedural Droplet Physics Engine`?**
  _High betweenness centrality (0.081) - this node is a cross-community bridge._
- **What connects `thock`, `build_mac_app.sh script`, `install.sh script` to the rest of the system?**
  _13 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Streaming Sources & Iterators` be split into smaller, more focused modules?**
  _Cohesion score 0.09852216748768473 - nodes in this community are weakly interconnected._
- **Should `Interactive CLI & Pack Parsing` be split into smaller, more focused modules?**
  _Cohesion score 0.12380952380952381 - nodes in this community are weakly interconnected._
- **Should `Acoustic Specifications & System Planning` be split into smaller, more focused modules?**
  _Cohesion score 0.14285714285714285 - nodes in this community are weakly interconnected._