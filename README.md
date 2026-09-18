## 📐 Core Architecture Overview

| Subsystem Domain | Primary Focus & Key Primitives | Documentation Spec |
# Domain 01 Technical Specification: Core Architecture & State Management

## 1. System Overview & Architectural Invariants

The Core & State domain governs the deterministic execution pipeline, thread synchronization, state history, and lock-free memory boundaries of **Forge Core V3**. To maintain execution stability and bit-exact replayability across hosts, the architecture enforces three fundamental invariants:

* **Zero Steady-State Heap Allocation**: All memory allocations occur during system startup. Runtime thread communication and rollback buffers utilize pre-allocated ring buffers and lock-free state bridges.
* **Deterministic Dual Timekeeping**: Simulation history and visual presentation are driven by two segregated clocks (`DetClock` and `CreativeClock`), isolating 120Hz physics calculations from asynchronous render frame rates.
* **Lock-Free Concurrency**: Cross-thread communication across the Logic, Raster, and Presentation pipelines relies exclusively on non-blocking atomics and triple-buffering mechanisms.

---

## 2. Multi-Thread Compositor & Concurrency Engine

### 2.1 Triple Loop Engine (`triple_loop.rs`)
The `triple_loop.rs` module manages the execution spine across three isolated, concurrent execution threads [22]:
1. **Logic Thread**: Drives Tier-1 synchronous simulation ticks (`SimTick`) at 120Hz.
2. **Raster Thread**: Handles visual spatial organization, voxel grid updates, and intermediate buffer synthesis.
3. **GPU / Present Thread**: Consumes visual state and handles final presentation output.

Communication between these threads is mediated by lock-free `TripleBuffer` bridges [22]. Visual updates and frame states are transmitted using `ClockPlane` payloads, which overwrite existing pre-allocated memory buffers to eliminate steady-state heap traffic [22].

### 2.2 Telemetry Overlay (`telemetry_kit.rs`)
The `telemetry_kit.rs` module provides a lock-free hardware and audio telemetry overlay [22]. It uses relaxed atomic operations (`TelemetryShared`) to stream integer-only performance metrics (`TelemetryView`) to the presentation thread [22]. This non-blocking design ensures telemetry monitoring does not introduce stalls into the 120Hz deterministic execution loop [22].

---

## 3. State Management & Phase Transitions

### 3.1 State Loop (`state_loop.rs`)
The `state_loop.rs` module implements a lock-free, dual-loop state machine powered by a `VestedTripleBuffer` bridge [1]. This structure isolates high-frequency 120Hz deterministic decay and vesting write cycles from asynchronous read requests [1]. 

The state loop evaluates `TriDualityState` across three system axes [1]:
* **Energy Flow**: Managing dynamic state propagation across active subsystems.
* **Boundary**: Enforcing spatial and memory containment limits.
* **Distribution**: Balancing resource allocation and system equilibria during state phase transitions [1].

### 3.2 Spine & Provenance Engine (`spine.rs`)
The `spine.rs` module acts as the backbone of the engine's deterministic fabric, tracking system artifacts through `AuthorityTicket` structures [1]. It enforces three core control criteria [1]:
* **Urgency Lanes (`Lane`)**: Assigns execution priority tiers to incoming commands.
* **Source Origin (`SourceKind`)**: Identifies the originating subsystem or network layer for state mutations.
* **Lock Verdict (`Trit`)**: A 3-state POSIX-aligned verdict representing state locks:
  * `-1`: **Fault** (lock rejected/invalid) [1]
  * `0`: **Intent** (lock pending validation) [1]
  * `+1`: **Sealed** (lock finalized and committed) [1]

### 3.3 Vested-Leaky Integrator (`vested_leaky.rs`)
The `vested_leaky.rs` module manages 32 independent processing channels designed to control field values over time [1]. Each channel applies:
* **Exponential Decay**: Smoothly reducing value intensity across ticks.
* **Vesting Floor**: Enforcing a monotonic lower bound beneath which field values cannot fall.
* **Hard Ceiling**: Enforcing an absolute upper threshold to prevent value runaway and keep continuous fields strictly bounded across ticks [1].

---

## 4. Deterministic Rollback, Checksumming, & History

### 4.1 Diff Pool & Rollback Engine (`diff_pool.rs`)
The `diff_pool.rs` module provides deterministic state rollback and prediction correction for multi-threaded state synchronization [18].
* **Voxel Mutations (`VixelDiff`)**: Individual voxel modifications are serialized into compact 18-byte `VixelDiff` structs [18].
* **Ring Buffer (`DiffPool`)**: Diffs are written sequentially to a 1.1MB pre-allocated `DiffPool` ring buffer [18].
* **Metadata Windows (`RollbackBuffer`)**: Diffs are grouped into 120-frame metadata windows (`RollbackBuffer`) to support frame prediction, error correction, and state rewind [18].

### 4.2 Allocation-Free Checksumming (`checksum.rs`, `checksum_2.rs`)
State integrity verification across frames is handled by a zero-dependency 64-bit FNV-1a checksum implementation [18, 23]. It uses a word-by-word streaming fold algorithm to process state memory directly without allocating buffer memory [18, 23].

---

## 5. Clock Domains & Deterministic Ordering

### 5.1 Segregated Clocks (`arch.rs`)
To prevent simulation drift and maintain replay accuracy, timekeeping is split into two explicit struct representations in `arch.rs` [23]:

| Clock Type | Memory Layout | Tracked Fields | System Purpose |
| :--- | :--- | :--- | :--- |
| **`DetClock`** | 16 bytes | `epoch`, `tick`, `authority` | Deterministic replay index used to establish a total ordering over state history [23]. |
| **`CreativeClock`** | 12 bytes | `beat`, `seed`, `phase` | Free-running visual clock driving presentation, animations, and UI state [23]. |

### 5.2 Primitive Time Metronomes
* **`SimTick`**: A `u64` transparent type wrapper serving as the metronome for Tier-1 synchronous physics updates [21].
* **`AudioFrame`**: A `u64` transparent type wrapper tracking Tier-2 variable-rate audio frames [21].

---

## 6. System Verification & Execution Order

### 6.1 Proof Stack (`stack.rs`)
The `stack.rs` module organizes system claims into a 4-state proof ladder [1]:
1. **Unproven**: Initial claim state.
2. **Estimate**: Heuristic or approximated value.
3. **Proven**: Mathematically verified state claim.
4. **Authored**: Fixed system doctrine.

Claims are indexed using 56-byte `StackRow` pointers [1]. `Authored` doctrine is specifically exempted from requiring machine anchors (`file:line`), granting foundational system rules fixed authority [1].

### 6.2 Dependency Graph (`dag.rs`)
The `dag.rs` module provides a directed acyclic graph structure that computes valid execution schedules for system tasks [18]. It uses Kahn's topological sorting algorithm to resolve node execution order and detect circular dependencies prior to execution [18].

### 6.3 Atomic Voxel Units (`atom.rs`)
At the lowest level of grid state, `atom.rs` defines `Pexil`—an 8-byte atomic structure representing a 5D lattice cell [23]. It packs five balanced trits into a 1-byte `TritCell15D` ($3^5 = 243$ states), paired with a Kleene three-valued validity mask, a 16-bit cell ordinal, and a 4-byte data payload [23].


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
