# Graph Report - KeySound  (2026-09-27)

## Corpus Check
- 40 files · ~71,381 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 155 nodes · 217 edges · 20 communities (17 shown, 3 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `31c6a309`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- ProceduralSwitch
- lib.rs
- ProceduralConfig
- Graph Report - KeySound  (2026-09-24)
- Thock for macOS
- Option
- Phase 2: Desktop GUI Transition
- Project: Thock (GUI Upgrade)
- Roadmap
- keymap.rs
- build_mac_app.sh
- thock

## God Nodes (most connected - your core abstractions)
1. `ProceduralSwitch` - 12 edges
2. `LoopingDecoder` - 11 edges
3. `Graph Report - KeySound  (2026-09-24)` - 11 edges
4. `ArcPcmSource` - 10 edges
5. `ThockHelper` - 9 edges
6. `Communities (21 total, 3 thin omitted)` - 9 edges
7. `Thock for macOS` - 9 edges
8. `load_audio_file()` - 8 edges
9. `ProceduralConfig` - 7 edges
10. `ArcPcm` - 7 edges

## Surprising Connections (you probably didn't know these)
- `LoadedPack` --references--> `ProceduralConfig`  [EXTRACTED]
  src/lib.rs → src/dsp.rs
- `PackConfig` --references--> `ProceduralConfig`  [EXTRACTED]
  src/lib.rs → src/dsp.rs

## Import Cycles
- None detected.

## Communities (20 total, 3 thin omitted)

### Community 0 - "ProceduralSwitch"
Cohesion: 0.14
Nodes (10): Biquad, Modal, PRNG, ProceduralSwitch, Duration, Iterator, Option, Self (+2 more)

### Community 1 - "lib.rs"
Cohesion: 0.16
Nodes (14): Completer, Context, Highlighter, Hinter, Pair, ReadlineError, Result, AppSettings (+6 more)

### Community 2 - "ProceduralConfig"
Cohesion: 0.32
Nodes (7): Default, ProceduralConfig, String, LoadedPack, PackConfig, HashMap, Vec

### Community 3 - "Graph Report - KeySound  (2026-09-24)"
Cohesion: 0.10
Nodes (19): Communities (21 total, 3 thin omitted), Community 0 - "ProceduralSwitch", Community 1 - "lib.rs", Community 2 - "Self", Community 3 - "Graph Report - KeySound  (2026-09-22)", Community 4 - "Thock for macOS", Community 6 - "Phase 2: Desktop GUI Transition", Community 7 - "Project: Thock (GUI Upgrade)" (+11 more)

### Community 4 - "Thock for macOS"
Cohesion: 0.09
Nodes (22): 📦 16 Built-in Sound Packs, Adding Custom Sound Packs, Architecture, Build a native `.app` bundle and install to Applications, CLI Commands, 💻 CLI Tab-Autocomplete, Clone & Build, Direct Download (Easiest) (+14 more)

### Community 5 - "Option"
Cohesion: 0.11
Nodes (17): Arc, Cursor, Decoder, DecoderError, Path, ArcPcm, ArcPcmSource, load_audio_file() (+9 more)

### Community 6 - "Phase 2: Desktop GUI Transition"
Cohesion: 0.40
Nodes (4): Goal, Implementation Details, Phase 2: Desktop GUI Transition, Verification

### Community 7 - "Project: Thock (GUI Upgrade)"
Cohesion: 0.50
Nodes (3): Core Constraints, Overview, Project: Thock (GUI Upgrade)

### Community 8 - "Roadmap"
Cohesion: 0.50
Nodes (3): Phase 1: Core Audio & Tray (Completed), Phase 2: Desktop GUI Transition (MVP), Roadmap

## Knowledge Gaps
- **44 isolated node(s):** `thock`, `build_mac_app.sh script`, `Overview`, `Core Constraints`, `Phase 1: Core Audio & Tray (Completed)` (+39 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **3 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ProceduralConfig` connect `ProceduralConfig` to `ProceduralSwitch`?**
  _High betweenness centrality (0.134) - this node is a cross-community bridge._
- **Why does `LoadedPack` connect `ProceduralConfig` to `lib.rs`, `Option`?**
  _High betweenness centrality (0.077) - this node is a cross-community bridge._
- **Why does `PackConfig` connect `ProceduralConfig` to `lib.rs`, `Option`?**
  _High betweenness centrality (0.058) - this node is a cross-community bridge._
- **What connects `thock`, `build_mac_app.sh script`, `Overview` to the rest of the system?**
  _44 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `ProceduralSwitch` be split into smaller, more focused modules?**
  _Cohesion score 0.14333333333333334 - nodes in this community are weakly interconnected._
- **Should `Graph Report - KeySound  (2026-09-24)` be split into smaller, more focused modules?**
  _Cohesion score 0.1 - nodes in this community are weakly interconnected._
- **Should `Thock for macOS` be split into smaller, more focused modules?**
  _Cohesion score 0.08695652173913043 - nodes in this community are weakly interconnected._