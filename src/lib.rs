//! Crate Zero — the zero-dependency floor. Every primitive carries a `const _` layout
//! lock, so a changed field type fails `cargo check` rather than review.
//! Doctrine: `forge_book::arch_tablets` ARCH-020. Prose cannot enforce; this can.

/// Ability Scaling — codifies which stats abilities draw from. AbilityStat types
/// are distinct from gem-socketing StatType (forge-book-v3).
pub mod ability_scaling;
pub mod arch;
pub mod astrolabe;
pub mod atom;
pub mod buff_registry;
pub mod cdk;
pub mod checksum;
pub mod chladni;
pub mod colour;
/// Integer sRGB8 <-> OklchColor bridge, ported from the v2 quarry codec
/// (Q32 fixed point, Newton cube root, CORDIC atan2/sin/cos).
pub mod colour_hub;
/// The monochrome law — a drained era renders achromatic and colour returns
/// one earned thing at a time. A world state, never a vision profile.
pub mod monochrome;
pub mod consequence;
/// Unified Canadian Aboriginal Syllabics (UCAS) — codepoint table and lookup.
pub mod cree_syllabics;
/// The climb — integer XP curve, level cap 99, unbounded ascension (v2 forge-insights
/// rpg/curve.rs, ported 2026-09-13). The ONE home for XP standing; game crates read it.
pub mod curve;
pub mod dag;
pub mod decay;
pub mod dialogue;
/// 8 Disciplines progression and 9-chord 120Hz fixed-point combat foundations.
pub mod discipline_progression;
pub mod diff_pool;
pub mod entity_memory;
pub mod expert_cache;
/// Unified expert framework: inference-time binding for all domain experts.
pub mod expert_framework;
pub mod fixed_point;
pub mod ghostmoon;
pub mod glossary;
pub mod grid;
pub mod hierarchical_moe;
/// Per-tick peer input barrier + hash chain + desync verdict + rollback. Fourth generation
/// of a lineage that began as `ak_lockstep_coordinator.gd` (see the module doc).
pub mod liminal;
pub mod lockstep;
/// Directed traversal of the 5-lane lattice: a `TritCell5D` read as a direction, one
/// integer DDA, predicate supplied by the caller (geometric / semantic / syntactic).
pub mod march;
pub mod mersenne;
pub mod metarouter;
pub mod music_sieve;
/// The 1-API registry for ported studio organs (Sean 2026-08-17: registry-first).
/// Zero-dep organs live here as mods; dep-heavy organs register downstream.
pub mod organs;
pub mod palette;
pub mod pentaract;
pub mod pentaract_field;
pub mod trit_bijection;
pub mod poisson_disk;
pub mod pose5d;
/// Balanced-primality proof (`{0,1}` as the fixed point, Composite/Prime as the 2-orbit).
/// Its sibling `anomaly_fold` proof scaffold retired 2026-08-14 — superseded by
/// `forge-sieve-v3::resonance::AnomalyType`, the real home the scaffold's own doc
/// comment always pointed at (L05 one-home).
pub mod primality_fold;
pub mod proof_surface;
pub mod pvp_seam;
pub mod ramus_prime;
pub mod resolvent;
pub mod river;
pub mod river_cst;
pub mod river_dsl;
pub mod s13;
/// ScalingProfile — unifies class+role stat distributions across three schemas.
/// Bridges CoreStats, RpgStats, and ability scaling coefficients.
pub mod scaling_profile;
/// Mulberry32 PRNG for combat/gameplay randomness (`serde` derives dropped —
/// Crate Zero has no deps).
pub mod seed;
pub mod sentinel;
/// Real sidereal time (Meeus GMST) — pure; faces supply the Julian Date.
pub mod sidereal;
/// 5D world-storage organs (forge-zone-v3-5D wave 1). Landed: `zones::spatial`.
pub mod zones;
/// The 13moons star-lore catalogue (16 named lights, v2-verbatim magnitudes).
/// Self-contained by contract: xtask mounts this same file via `#[path]`.
pub mod aspire;
pub mod sky;
pub mod soul;
/// Soliton-Phase Context Collapse — the interference kernel handed to `Field5D`.
pub mod spcc;
pub mod spine;
/// Anti-Shannon Spectral — 32-channel concentration via pure quadratic form (SPATIAL_TEMPORAL_ISOMORPHISM.md:50-62).
pub mod spectral;
/// Dual-loop state machine with VestedTripleBuffer lock-free architecture.
/// Decay → vesting → snapshot read → spectral → trit transition (no locks, O(1) swap).
/// 6-primitive (3-duality) closed-loop system: energy flow, boundary, distribution axes.
pub mod state_loop;
pub use state_loop::TriDualityState;
/// Sealed sprite-animation binary blob — the SoA/AoS boundary (ANIM-domain fold).
pub mod sprite_blob;
pub mod stack;
pub mod surfaceledger;
/// Single-word 13-trit balanced-ternary packer, sibling to `atom::TritCell5D`'s
/// 5-trit byte — one radix, one law, a wider word (`3^13` fits a `u32`).
pub mod trit13;
/// The 16-byte MoM (Mixture of Musicians) routing word — distinct from
/// `spine::packet::Ump`, the MIDI 2.0 wire packet (same width, different face).
pub mod ump_word;
/// Vixel automata cellular rules — fire, gravity, fluid flow, sand emergence/collapse.
pub mod vixel_automata;
pub mod vested_leaky;
pub mod weighted_reservoir;

pub use ability_scaling::{
    AbilityDef, AbilityRegistry, AbilityStat, CharacterStats, ScalingPair, compute_ability_power,
};
pub use arch::{ArchRole, CreativeClock, DetClock};
pub use atom::{CellOrdinal, Pexil, PexilLine, TritCell5D, ValidityMask, RADIX, TRITS_PER_BYTE};
pub use buff_registry::{apply_modifier, decay_buffs, BuffEffect, BuffRegistry, StackingPolicy, StatTarget};
pub use checksum::hash_bytes_fnv1a;
pub use colour::{ColorBlindMode, OklchColor, CHROMA_CEILING_PERMYRIAD, TURN, VISION_PROFILES};
pub use colour_hub::{oklch_to_rgb8, rgb8_to_oklch};
pub use fixed_point::{
    cartesian_to_hex_prism, isqrt_i128, isqrt_i64, isqrt_u64, log10_permyriad, AudioFrame,
    MilliUnit, Permyriad, SimTick, Vec2Milli, Vec3Milli,
};
pub use decay::{LeakyPermyriad, PMY};
pub use discipline_progression::{
    ChordAffinity, ChordKind, DisciplineKind, DisciplineProgression, PoiseState,
};
pub use expert_cache::{CacheStats, ExpertCache, ExpertId, Tier};
pub use ghostmoon::Ghostmoon;
pub use grid::{pixels_to_point, point_to_pixels, GridPixelBuffer, PackedPoint105, CELLS, DEPTH, LANES};
pub use hierarchical_moe::{
    DomainSpecialist, HierarchicalMoe, SubExpertSlice, SubRouter, NUM_DOMAINS, SUB_EXPERTS_PER_DOMAIN,
    TOTAL_EXPERTS,
};
pub use mersenne::{reduce_m61, EXPONENTS, M13, M2, M3, M31, M5, M61, M7};
pub use metarouter::MetaRouter;
pub use palette::{MachineColor, SENTINEL_PALETTE, TRIT_PALETTE};
pub use proof_surface::{
    ProofSurface, MIP_LEVEL_COUNT, READBACK_COMPRESSION, READBACK_FILTER, READBACK_FORMAT,
    SAMPLE_COUNT, SRGB_TRANSFORM, SURFACE_BYTES, SURFACE_HEIGHT, SURFACE_WIDTH,
};
pub use ramus_prime::{
    edge_is_forward, key_to_point, mersenne_dot, point_to_key, Box5D, HypersphereVector5D,
    MersenneScalar, MortonKey5D, RamusPrimeNode, AXES, AXIS_BITS, AXIS_MASK, AXIS_S, AXIS_T,
    AXIS_X, AXIS_Y, AXIS_Z, KEY_TRIT_DIGITS,
};
pub use resolvent::Field5D;
pub use trit_bijection::{TritArray32, TritStateMachine};
pub use vested_leaky::VestedLeaky32;
pub use spectral::AntiShannonSpectral;
pub use river::{ingest, parse_line, render_line, IngestReport, RiverLine, RiverRefusal};
pub use s13::S13;
pub use scaling_profile::{
    CharacterClass, CharacterRole, CoreStat, CoreStatValues, ScalingProfile, StatAssignment,
};
pub use seed::Mulberry32;
pub use sentinel::{breach, Sentinel, MAX_PACKED, SENTINEL_COUNT};
pub use soul::{EssenceId, SoulId, SoulIdentity, PILLARS};
pub use spcc::{
    collapse, interaction, is_orphan, phase_inverse, CollapseReceipt, ContextRow, Interaction,
    COUPLING_CEILING_PMY, HOLON, PHASE_EPS_TRITS, PROXIMITY_MAX_TRITS, WEIGHT_FLOOR_PMY,
};
pub use spine::{
    AuthorityTicket, BrutalHash, CarrierHeader, CarrierHeaderError, CarrierKind, Lane, ReceiptKind,
    SourceKind, Trit,
};
pub use stack::{tally, ProofState, StackRow, PROOF_STATES};
pub use trit13::{pack13, unpack13, TRITS_PER_WORD};
pub use astrolabe::{Astrolabe, StarPointer, CATALOG_16};

/// The three forcings that land on 13, stated as machine-checkable facts.
/// ARCH-020 §1 — two are forced by nature, one by construction. Only the second
/// is checkable here; the astronomical one is `Proof::Authored`, the architectural
/// one is a design outcome and is deliberately not asserted.
pub mod thirteen {
    use super::{MAX_PACKED, RADIX, SENTINEL_COUNT, TRITS_PER_BYTE};

    /// A 5-trit cell in an 8-bit byte leaves exactly 13 states. Forced arithmetic.
    pub const ARITHMETIC_FORCING: usize = 256 - (RADIX as usize).pow(TRITS_PER_BYTE as u32);

    const _: () = assert!(ARITHMETIC_FORCING == 13);
    const _: () = assert!(ARITHMETIC_FORCING == SENTINEL_COUNT);
    const _: () = assert!(MAX_PACKED as usize + SENTINEL_COUNT == 256);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_arithmetic_forcing_is_thirteen() {
        assert_eq!(crate::thirteen::ARITHMETIC_FORCING, 13);
    }
}
