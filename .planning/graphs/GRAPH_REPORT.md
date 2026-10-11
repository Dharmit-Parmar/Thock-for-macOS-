# Graph Report - KeySound  (2026-10-12)

## Corpus Check
- 21 files · ~55,069 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 177 nodes · 316 edges · 22 communities (13 shown, 9 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 1 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `15b17e40`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- ProceduralRain
- lib.rs
- ZeroCopySource
- SeamlessLoopSource
- ProceduralConfig
- Procedural Audio Synthesis
- Graph Report - KeySound  (2026-09-27)
- Eframe GUI Integration
- build_mac_app.sh
- Thock Project
- thock
- ASMR Engine
- Thock CLI Mode
- GSD Inbox Triage Report
- safeIpc
- Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP
- DirectOrSpeed
- verify.rs
- install.sh

## God Nodes (most connected - your core abstractions)
1. `ProceduralRain` - 14 edges
2. `ProceduralSwitch` - 13 edges
3. `SeamlessLoopSource` - 13 edges
4. `Graph Report - KeySound  (2026-09-27)` - 11 edges
5. `Biquad` - 10 edges
6. `Communities (20 total, 3 thin omitted)` - 10 edges
7. `ThockHelper` - 9 edges
8. `ZeroCopySource` - 9 edges
9. `load_pack()` - 9 edges
10. `AsmrBuffer` - 8 edges

## Surprising Connections (you probably didn't know these)
- `Zero Latency Constraint` --rationale_for--> `Procedural Audio Synthesis`  [INFERRED]
  .planning/PROJECT.md → DSP_ENGINE_RESEARCH.md
- `SeamlessLoopSource` --references--> `Biquad`  [EXTRACTED]
  src/lib.rs → src/dsp.rs
- `generate_procedural_rain_buffer()` --references--> `AsmrBuffer`  [EXTRACTED]
  src/dsp.rs → src/lib.rs
- `LoadedPack` --references--> `ProceduralConfig`  [EXTRACTED]
  src/lib.rs → src/dsp.rs
- `PackConfig` --references--> `ProceduralConfig`  [EXTRACTED]
  src/lib.rs → src/dsp.rs

## Import Cycles
- None detected.

## Communities (22 total, 9 thin omitted)

### Community 0 - "ProceduralRain"
Cohesion: 0.10
Nodes (14): Default, Biquad, DropletVoice, generate_procedural_rain_buffer(), ModalMode, PRNG, ProceduralRain, ProceduralSwitch (+6 more)

### Community 1 - "lib.rs"
Cohesion: 0.15
Nodes (27): Completer, Context, Highlighter, Hinter, Path, AppSettings, AsmrBuffer, decode_crossfaded() (+19 more)

### Community 2 - "ZeroCopySource"
Cohesion: 0.19
Nodes (4): DirectOrSpeed<A, B>, Duration, Iterator, ZeroCopySource

### Community 3 - "SeamlessLoopSource"
Cohesion: 0.13
Nodes (11): Arc, Item, OnceLock, get_key_map(), Option, AsmrLoopKind, PredecodedAudio, Self (+3 more)

### Community 4 - "ProceduralConfig"
Cohesion: 0.22
Nodes (9): HashMap, Pair, ReadlineError, Result, ProceduralConfig, String, LoadedPack, PackConfig (+1 more)

### Community 5 - "Procedural Audio Synthesis"
Cohesion: 0.50
Nodes (4): Zero Latency Constraint, Procedural Audio Synthesis, TDF-II Biquad Filter, Xorshift PRNG

### Community 6 - "Graph Report - KeySound  (2026-09-27)"
Cohesion: 0.10
Nodes (20): Communities (20 total, 3 thin omitted), Community 0 - "ProceduralSwitch", Community 1 - "lib.rs", Community 2 - "ProceduralConfig", Community 3 - "Graph Report - KeySound  (2026-09-24)", Community 4 - "Thock for macOS", Community 5 - "Option", Community 6 - "Phase 2: Desktop GUI Transition" (+12 more)

### Community 16 - "GSD Inbox Triage Report"
Cohesion: 0.33
Nodes (5): Gate Violations & Action Items, GSD Inbox Triage Report, Issues & PR Status, Recommendations, Summary

### Community 18 - "Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP"
Cohesion: 0.40
Nodes (4): Deliverables & Evidence, Objective, Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP, Status: Completed

### Community 19 - "DirectOrSpeed"
Cohesion: 0.67
Nodes (3): A, B, DirectOrSpeed

## Knowledge Gaps
- **36 isolated node(s):** `thock`, `build_mac_app.sh script`, `install.sh script`, `Summary`, `Gate Violations & Action Items` (+31 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **9 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `SeamlessLoopSource` connect `SeamlessLoopSource` to `ProceduralRain`, `lib.rs`, `ZeroCopySource`?**
  _High betweenness centrality (0.143) - this node is a cross-community bridge._
- **Why does `Biquad` connect `ProceduralRain` to `SeamlessLoopSource`?**
  _High betweenness centrality (0.127) - this node is a cross-community bridge._
- **Why does `ProceduralConfig` connect `ProceduralConfig` to `ProceduralRain`?**
  _High betweenness centrality (0.063) - this node is a cross-community bridge._
- **What connects `thock`, `build_mac_app.sh script`, `install.sh script` to the rest of the system?**
  _36 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `ProceduralRain` be split into smaller, more focused modules?**
  _Cohesion score 0.10104529616724739 - nodes in this community are weakly interconnected._
- **Should `SeamlessLoopSource` be split into smaller, more focused modules?**
  _Cohesion score 0.13450292397660818 - nodes in this community are weakly interconnected._
- **Should `Graph Report - KeySound  (2026-09-27)` be split into smaller, more focused modules?**
  _Cohesion score 0.09523809523809523 - nodes in this community are weakly interconnected._