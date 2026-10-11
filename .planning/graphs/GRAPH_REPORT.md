# Graph Report - KeySound  (2026-10-12)

## Corpus Check
- 21 files · ~62,689 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 178 nodes · 316 edges · 22 communities (13 shown, 9 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 1 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `c29afcdd`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- ProceduralRain
- .new
- Option
- OnceLock
- lib.rs
- Procedural Audio Synthesis
- Communities (22 total, 9 thin omitted)
- Eframe GUI Integration
- build_mac_app.sh
- Thock Project
- thock
- ASMR Engine
- Thock CLI Mode
- GSD Inbox Triage Report
- safeIpc
- Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP
- Biquad
- verify.rs
- install.sh

## God Nodes (most connected - your core abstractions)
1. `ProceduralRain` - 14 edges
2. `ProceduralSwitch` - 13 edges
3. `SeamlessLoopSource` - 13 edges
4. `Graph Report - KeySound  (2026-10-12)` - 11 edges
5. `Communities (22 total, 9 thin omitted)` - 11 edges
6. `Biquad` - 10 edges
7. `ThockHelper` - 9 edges
8. `ZeroCopySource` - 9 edges
9. `load_pack()` - 9 edges
10. `AsmrBuffer` - 8 edges

## Surprising Connections (you probably didn't know these)
- `Zero Latency Constraint` --rationale_for--> `Procedural Audio Synthesis`  [INFERRED]
  .planning/PROJECT.md → DSP_ENGINE_RESEARCH.md
- `SeamlessLoopSource` --references--> `Biquad`  [EXTRACTED]
  src/lib.rs → src/dsp.rs
- `LoadedPack` --references--> `ProceduralConfig`  [EXTRACTED]
  src/lib.rs → src/dsp.rs
- `PackConfig` --references--> `ProceduralConfig`  [EXTRACTED]
  src/lib.rs → src/dsp.rs
- `generate_procedural_rain_buffer()` --references--> `AsmrBuffer`  [EXTRACTED]
  src/dsp.rs → src/lib.rs

## Import Cycles
- None detected.

## Communities (22 total, 9 thin omitted)

### Community 0 - "ProceduralRain"
Cohesion: 0.13
Nodes (9): ModalMode, PRNG, ProceduralRain, ProceduralSwitch, Duration, Iterator, Option, Source (+1 more)

### Community 1 - ".new"
Cohesion: 0.18
Nodes (19): Arc, Path, generate_procedural_rain_buffer(), AsmrBuffer, decode_crossfaded(), decode_faded_thunder(), get_rain_buffer(), get_thunder_buffer() (+11 more)

### Community 2 - "Option"
Cohesion: 0.14
Nodes (10): Item, AsmrLoopKind, DirectOrSpeed<A, B>, Duration, Iterator, Option, Self, Source (+2 more)

### Community 3 - "OnceLock"
Cohesion: 0.50
Nodes (3): OnceLock, get_key_map(), Option

### Community 4 - "lib.rs"
Cohesion: 0.12
Nodes (21): A, B, Completer, Context, HashMap, Highlighter, Hinter, Pair (+13 more)

### Community 5 - "Procedural Audio Synthesis"
Cohesion: 0.50
Nodes (4): Zero Latency Constraint, Procedural Audio Synthesis, TDF-II Biquad Filter, Xorshift PRNG

### Community 6 - "Communities (22 total, 9 thin omitted)"
Cohesion: 0.09
Nodes (21): Communities (22 total, 9 thin omitted), Community 0 - "ProceduralRain", Community 16 - "GSD Inbox Triage Report", Community 18 - "Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP", Community 19 - "DirectOrSpeed", Community 1 - "lib.rs", Community 2 - "ZeroCopySource", Community 3 - "SeamlessLoopSource" (+13 more)

### Community 16 - "GSD Inbox Triage Report"
Cohesion: 0.33
Nodes (5): Gate Violations & Action Items, GSD Inbox Triage Report, Issues & PR Status, Recommendations, Summary

### Community 18 - "Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP"
Cohesion: 0.40
Nodes (4): Deliverables & Evidence, Objective, Phase 3: Natural ASMR Acoustic Engine & Zero-Allocation DSP, Status: Completed

### Community 19 - "Biquad"
Cohesion: 0.24
Nodes (6): Default, Biquad, DropletVoice, ProceduralConfig, Self, String

## Knowledge Gaps
- **37 isolated node(s):** `thock`, `build_mac_app.sh script`, `install.sh script`, `Summary`, `Gate Violations & Action Items` (+32 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **9 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `SeamlessLoopSource` connect `Option` to `.new`, `Biquad`, `lib.rs`?**
  _High betweenness centrality (0.141) - this node is a cross-community bridge._
- **Why does `Biquad` connect `Biquad` to `ProceduralRain`, `Option`?**
  _High betweenness centrality (0.126) - this node is a cross-community bridge._
- **Why does `ProceduralConfig` connect `Biquad` to `ProceduralRain`, `lib.rs`?**
  _High betweenness centrality (0.062) - this node is a cross-community bridge._
- **What connects `thock`, `build_mac_app.sh script`, `install.sh script` to the rest of the system?**
  _37 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `ProceduralRain` be split into smaller, more focused modules?**
  _Cohesion score 0.13054187192118227 - nodes in this community are weakly interconnected._
- **Should `Option` be split into smaller, more focused modules?**
  _Cohesion score 0.14492753623188406 - nodes in this community are weakly interconnected._
- **Should `lib.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.12 - nodes in this community are weakly interconnected._