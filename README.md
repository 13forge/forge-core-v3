forge-core-v3

```
# Forge Core V3

**Forge Core V3** is a deterministic, zero-float systems engineering engine built for high-frequency simulation, structural mechanics, and multi-dimensional grid state.

## 🛠 Architectural Invariants
1. **Zero Steady-State Heap Traffic**: The core 120Hz execution pipeline utilizes pre-allocated ring buffers and `TripleBuffer` state bridges to eliminate runtime allocations.
2. **Absolute Determinism**: Zero `f32`/`f64` floats. All spatial coordinates, colors, and audio waveforms use fixed-point `Permyriad` (1/10,000) or `MilliUnit` arithmetic.
3. **Lock-Free Concurrency**: Segregated Logic, Raster, and Present threads communicating over non-blocking state rings.

## 📐 Core Architecture Overview

| Subsystem Domain | Primary Focus &amp; Key Primitives | Documentation Spec |
| :--- | :--- | :--- |
| **Core &amp; State** | Concurrency, `TripleLoop`, `Spine`, Rollback Ring Buffers | [`/docs/01_core_and_state.md`](docs/01_core_and_state.md) |
| **Math &amp; 5D Lattices** | `Permyriad`, `RamusPrime` 5D Morton keys, Ternary Math | [`/docs/02_math_and_geometry.md`](docs/02_math_and_geometry.md) |
| **Structural Engineering** | Ad Quadratum/Triangulum solvers, Buttress thrust checks | [`/docs/03_structural_solvers.md`](docs/03_structural_solvers.md) |
| **Simulation &amp; Rules** | `VixelAtom` cellular automata, 14-family MoE tag routing | [`/docs/04_simulation_and_rules.md`](docs/04_simulation_and_rules.md) |
| **Data &amp; Memory** | `River` wire grammar, L1-L3 Cache Words (`SoulWord`) | [`/docs/05_data_and_memory.md`](docs/05_data_and_memory.md) |
| **Media &amp; Render** | OKLCH color spaces, Integer PCM `SongSynth`, SVG engine | [`/docs/06_media_and_presentation.md`](docs/06_media_and_presentation.md) |
| **Tooling &amp; Verification**| `DualOracle` consensus, `SilentDrops` QA, `TwinCull` | [`/docs/07_tooling_and_qa.md`](docs/07_tooling_and_qa.md) |

## 🚀 Quickstart &amp; Verification
# Run deterministic test harness
cargo test --all

# Run structural validation &amp; silent drop QA harness
cargo run --bin silent_drops

```

---