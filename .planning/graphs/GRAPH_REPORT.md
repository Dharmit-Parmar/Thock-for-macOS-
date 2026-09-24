# Graph Report - KeySound  (2026-09-24)

## Corpus Check
- 41 files · ~67,258 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 151 nodes · 214 edges · 21 communities (18 shown, 3 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `87db88f8`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- ProceduralSwitch
- lib.rs
- Self
- Graph Report - KeySound  (2026-09-22)
- Thock for macOS
- Phase 2: Desktop GUI Transition
- Project: Thock (GUI Upgrade)
- Roadmap
- keymap.rs
- build_mac_app.sh
- thock

## God Nodes (most connected - your core abstractions)
1. `ProceduralSwitch` - 12 edges
2. `AsmrSource` - 11 edges
3. `Graph Report - KeySound  (2026-09-22)` - 11 edges
4. `Communities (18 total, 3 thin omitted)` - 10 edges
5. `Biquad` - 9 edges
6. `ThockHelper` - 9 edges
7. `Thock for macOS` - 9 edges
8. `ArcBuffer` - 8 edges
9. `ProceduralConfig` - 7 edges
10. `LoadedPack` - 7 edges

## Surprising Connections (you probably didn't know these)
- `LoadedPack` --references--> `ProceduralConfig`  [EXTRACTED]
  src/lib.rs → src/dsp.rs
- `PackConfig` --references--> `ProceduralConfig`  [EXTRACTED]
  src/lib.rs → src/dsp.rs

## Import Cycles
- None detected.

## Communities (21 total, 3 thin omitted)

### Community 0 - "ProceduralSwitch"
Cohesion: 0.13
Nodes (10): Duration, Iterator, AsmrSource, Biquad, Modal, PRNG, ProceduralSwitch, Option (+2 more)

### Community 1 - "lib.rs"
Cohesion: 0.12
Nodes (29): Arc, Completer, Context, Highlighter, Hinter, Item, Pair, Path (+21 more)

### Community 2 - "Self"
Cohesion: 0.36
Nodes (4): Default, ProceduralConfig, Self, String

### Community 3 - "Graph Report - KeySound  (2026-09-22)"
Cohesion: 0.10
Nodes (20): Communities (18 total, 3 thin omitted), Community 0 - "ProceduralSwitch", Community 1 - "ArcBuffer", Community 2 - "Mathematical Keyboard Sound Generation (DSP) Research", Community 3 - "Graph Report - KeySound  (2026-09-22)", Community 4 - "README.md", Community 5 - "Communities (18 total, 3 thin omitted)", Community 6 - "Phase 2: Desktop GUI Transition" (+12 more)

### Community 4 - "Thock for macOS"
Cohesion: 0.09
Nodes (22): 📦 16 Built-in Sound Packs, Adding Custom Sound Packs, Architecture, Build a native `.app` bundle and install to Applications, CLI Commands, 💻 CLI Tab-Autocomplete, Clone & Build, Direct Download (Easiest) (+14 more)

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
- **45 isolated node(s):** `thock`, `build_mac_app.sh script`, `Overview`, `Core Constraints`, `Phase 1: Core Audio & Tray (Completed)` (+40 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **3 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ProceduralConfig` connect `Self` to `ProceduralSwitch`, `lib.rs`?**
  _High betweenness centrality (0.130) - this node is a cross-community bridge._
- **Why does `LoadedPack` connect `lib.rs` to `Self`?**
  _High betweenness centrality (0.071) - this node is a cross-community bridge._
- **Why does `PackConfig` connect `lib.rs` to `Self`?**
  _High betweenness centrality (0.053) - this node is a cross-community bridge._
- **What connects `thock`, `build_mac_app.sh script`, `Overview` to the rest of the system?**
  _45 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `ProceduralSwitch` be split into smaller, more focused modules?**
  _Cohesion score 0.13227513227513227 - nodes in this community are weakly interconnected._
- **Should `lib.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.11861861861861862 - nodes in this community are weakly interconnected._
- **Should `Graph Report - KeySound  (2026-09-22)` be split into smaller, more focused modules?**
  _Cohesion score 0.09523809523809523 - nodes in this community are weakly interconnected._