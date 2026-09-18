## 📐 Core Architecture Overview

| Subsystem Domain | Primary Focus & Key Primitives | Documentation Spec |
|---|---|---|
| **Core & State** | Concurrency, `TripleLoop`, `Spine`, Rollback Ring Buffers | [`/docs/01_core_and_state.md`](docs/01_core_and_state.md) |
| **Math & 5D Lattices** | `Permyriad`, `RamusPrime` 5D Morton keys, Ternary Math | [`/docs/02_math_and_geometry.md`](docs/02_math_and_geometry.md) |

... (rest of your table) ...

## 🚀 Quickstart & Verification

# Run deterministic test harness
```bash
cargo test --all

# Run structural validation & silent drop QA harness
cargo run --bin silent_drops

After two decades measuring, prepping, and coating physical surfaces and tolerances on job sites, I made the choice after 23yrs to learn a new trade because
the last one has broke my body. Forge Core V3 is a 38-subsystem deterministic runtime featuring 60-bit 5D Morton indexing, lock-free triple-buffering, and
fixed-point physics. Here is what I learned going from tradesman to systems programmer in 10 months.
People do not like change.
Machines are not always truthful.
New concepts are often looked at with fear.
What started with a python script, quickly turned into photometric stereo solves and all the way to concepts like At Rest Compute, and splitshaders.

This is the result of thousands of hours of self taught engineering so take it for what it is.
-dev
13forge
