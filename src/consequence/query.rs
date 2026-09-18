//! `InteractionQuery` (16 bytes) and `Consequence` (8 bytes).
//!
//! Ported 2026-08-13 from `F:\NewRepo\crates\forge-consequence\src\query.rs`
//! (v2, real, tested). Two adaptations forced by Crate Zero's empty
//! `[dependencies]` section (Firewall Law — see `forge-core-v3/Cargo.toml`):
//!
//! - The source derives `bytemuck::{Pod, Zeroable}` for MoE byte-vector I/O.
//!   Dropped here: no `bytemuck` dependency exists to derive against. The
//!   16/8-byte-and-alignment-≤2 contract that `Pod` would have checked is
//!   still enforced, just via the `const _: size_of`/`align_of` layout locks
//!   below instead of a derive macro.
//! - `ConsequenceKind` drops its source `serde::{Serialize, Deserialize}`
//!   derives by default for the same reason (`fixed_point.rs` sets this
//!   precedent). 2026-08-17: gated back on behind an optional, off-by-default
//!   `serde` feature (see `forge-core-v3/Cargo.toml`) — `forge-consequence-v3`
//!   enables it for `quest.rs`/`rule.rs`. Default build stays zero-dep.
//!
//! `position: [MilliUnit; 3]` uses this crate's own `fixed_point::MilliUnit`
//! (`i64`), not the source's `forge_physics::types::MilliUnit` — no v3
//! `forge-physics` equivalent has been ported yet; `PendingInteraction` is
//! wired to Crate Zero's own type so it does not block on that port.

use crate::fixed_point::MilliUnit;

use super::tags::{SRC_FAMILY_SOUND, SRC_SOUND, TGT_FAMILY_TERRAIN, TGT_STONE};

/// 16-byte flat physics interaction query.
///
/// Layout (alignment = 2; all little-endian on supported platforms):
///
/// | offset | size | field |
/// |--------|------|-------|
/// | 0      | 1    | `source_tag` — within-family u8 (e.g. `SRC_FIRE = 0`) |
/// | 1      | 1    | `source_family` — Tier-1 routing key, 0..=6 |
/// | 2      | 1    | `target_tag` — within-family u8 (e.g. `TGT_STONE = 0`) |
/// | 3      | 1    | `target_family` — Tier-2 routing key, 0..=6 |
/// | 4      | 2    | `intensity_pmy` — Permyriad of full intensity (0..=10_000) |
/// | 6      | 2    | `material_id` — handle into a not-yet-ported v3 material registry |
/// | 8      | 2    | `resonance_pmy` — frequency-match Permyriad (10_000 = perfect) |
/// | 10     | 1    | `target_state` — current degradation/growth stage (0..=255) |
/// | 11     | 1    | `chain_depth` — cascade hop count; energy decays per hop |
/// | 12     | 1    | `context_celestial` — packed: moon phase / season / day_night |
/// | 13     | 1    | `velocity_pmy` — periodic/rhythm rate (0..=255 → Permyriad-of-255) |
/// | 14     | 1    | `faction` — faction id, 0=none (parallel observer only) |
/// | 15     | 1    | `relationship` — blessed=0, neutral=128, cursed=255 |
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InteractionQuery {
    /// Within-family source tag (e.g. `SRC_FIRE`, `SRC_SOUND`).
    pub source_tag: u8,
    /// Source family (Tier-1 MoE routing key, 0..=6).
    pub source_family: u8,
    /// Within-family target tag (e.g. `TGT_STONE`, `TGT_WOOD`).
    pub target_tag: u8,
    /// Target family (Tier-2 MoE routing key, 0..=6).
    pub target_family: u8,
    /// Intensity in Permyriad (0..=10_000).
    pub intensity_pmy: u16,
    /// Handle into a not-yet-ported v3 material registry.
    pub material_id: u16,
    /// Resonance / frequency match Permyriad. 10_000 = perfect match.
    pub resonance_pmy: u16,
    /// Current degradation or growth stage (0..=255).
    pub target_state: u8,
    /// Cascade hop count. Energy decays per hop; gate at the dispatcher.
    pub chain_depth: u8,
    /// Packed celestial context. MSB nibble = moon phase index (0..=12 fits
    /// in 4 bits with one spare); LSB nibble: bit 0 = day/night, bits 1..=3
    /// reserved (season index lives in the moon_phase nibble via convention).
    pub context_celestial: u8,
    /// Periodic-rate Permyriad-of-255 (rhythm / sustained-source velocity).
    pub velocity_pmy: u8,
    /// Faction id (0 = none). Parallel observer only — never gates physics.
    pub faction: u8,
    /// blessed=0, neutral=128, cursed=255.
    pub relationship: u8,
}

/// Semantic-wire query tags carried in byte `[0]` of the 16-byte image a
/// semantic-layer sieve action hands across the semantic → consequence
/// boundary. Append-only. `0` is reserved for "no routable interaction" so a
/// zeroed / stub image can never promote a world consequence.
pub const QUERY_TAG_NONE: u8 = 0;
/// The `grave_bell` phrase's canonical world interaction (sem-δ stub binding).
pub const QUERY_TAG_GRAVE_BELL: u8 = 1;

impl InteractionQuery {
    /// Decode a 16-byte semantic-wire image into a routable `InteractionQuery`.
    ///
    /// Byte `[0]` is the [`QUERY_TAG_NONE`] / [`QUERY_TAG_GRAVE_BELL`]
    /// discriminant. An unrecognized or `QUERY_TAG_NONE` tag yields `None` —
    /// the consequence dispatcher then promotes no world-state pressure.
    ///
    /// sem-δ stub: the grave_bell tag maps to a fixed bard-resonance query
    /// (`SOUND → STONE`, the one curve a struck bell belongs to).
    pub fn from_semantic_wire(bytes: [u8; 16]) -> Option<Self> {
        match bytes[0] {
            QUERY_TAG_GRAVE_BELL => Some(Self::grave_bell()),
            _ => None,
        }
    }

    /// Canonical world interaction for the `grave_bell` phrase: a fully
    /// resonant sound striking stone. Routes to the existing
    /// `SOUND → STONE` resonance-shatter curve — no new curve invented.
    fn grave_bell() -> Self {
        Self {
            source_tag: SRC_SOUND,
            source_family: SRC_FAMILY_SOUND,
            target_tag: TGT_STONE,
            target_family: TGT_FAMILY_TERRAIN,
            intensity_pmy: 10_000,
            resonance_pmy: 10_000,
            material_id: 1,
            ..Self::default()
        }
    }
}

/// Discrete consequence kind. Maps to one or more downstream physics-effect
/// variants during dispatch (not yet ported).
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConsequenceKind {
    /// Nothing happens (threshold not crossed yet). Default for the zeroed layout.
    #[default]
    None = 0,
    /// Material reaches structural failure → shatter + structural collapse.
    Shatter = 1,
    /// Material ignites → fire-ignite.
    Ignite = 2,
    /// Active flame extinguished → fire-extinguish.
    Extinguish = 3,
    /// Voxel removed (erosion, splash, melt) → voxel-break.
    VoxelBreak = 4,
    /// Voxel deposited (sediment, growth, freeze) → voxel-place.
    VoxelPlace = 5,
    /// Sound emission → sound-event.
    Sound = 6,
    /// Smoke / dust / spore emission → plume-spawn.
    Plume = 7,
    /// Entity damage → damage.
    Damage = 8,
    /// Visibility reduction (plume reaches AI) → visibility-change.
    Visibility = 9,
    /// Semantic primitive resolved with reveal-direction effect (e.g. GraveBell + MinorThirdDescent).
    Reveal = 10,
    /// Memory anchor stabilized — locks an erased name back into ledger state.
    MemoryStabilize = 11,
    /// Witness emission — the world records a witnessed semantic event.
    WitnessTrace = 12,
}

/// 8-byte flat consequence descriptor returned by the WCE router.
///
/// Layout (alignment = 2):
///
/// | offset | size | field |
/// |--------|------|-------|
/// | 0      | 1    | `kind` — `ConsequenceKind` discriminant |
/// | 1      | 1    | `new_state` — target_state after transition |
/// | 2      | 2    | `catalytic_pmy` — Permyriad of source energy released |
/// | 4      | 1    | `debris_count` — secondary entities to spawn |
/// | 5      | 1    | `sound_db` — sound emission level (0..=255 ≈ 0..=127 dB) |
/// | 6      | 1    | `plume_kind` — 0=none, 1=smoke, 2=steam, 3=dust, 4=spore |
/// | 7      | 1    | `flags` — bit 0: terrain_mutation, bit 1: entity_damage, ... |
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Consequence {
    /// `ConsequenceKind` as raw u8 (no enum field — matches the flat wire layout).
    pub kind: u8,
    /// New degradation/growth stage after this consequence.
    pub new_state: u8,
    /// Permyriad of stored source energy released by this transition.
    pub catalytic_pmy: u16,
    /// Number of secondary entities (debris, splash droplets, embers).
    pub debris_count: u8,
    /// Sound emission level (0..=255 ≈ 0..=127 dB).
    pub sound_db: u8,
    /// Plume kind: 0=none, 1=smoke, 2=steam, 3=dust, 4=spore.
    pub plume_kind: u8,
    /// Bit-packed flags. Bit 0: terrain_mutation. Bit 1: entity_damage.
    pub flags: u8,
}

impl Consequence {
    /// Convert the raw `kind` byte to a typed `ConsequenceKind`.
    #[inline]
    pub fn kind(&self) -> ConsequenceKind {
        match self.kind {
            1 => ConsequenceKind::Shatter,
            2 => ConsequenceKind::Ignite,
            3 => ConsequenceKind::Extinguish,
            4 => ConsequenceKind::VoxelBreak,
            5 => ConsequenceKind::VoxelPlace,
            6 => ConsequenceKind::Sound,
            7 => ConsequenceKind::Plume,
            8 => ConsequenceKind::Damage,
            9 => ConsequenceKind::Visibility,
            10 => ConsequenceKind::Reveal,
            11 => ConsequenceKind::MemoryStabilize,
            12 => ConsequenceKind::WitnessTrace,
            _ => ConsequenceKind::None,
        }
    }
}

/// Dispatcher-side bundle: a 16-byte `InteractionQuery` plus the cell id
/// and emission position the dispatcher needs to route + emit a physics
/// effect. NOT 16 bytes itself — purely a wrapper.
#[derive(Clone, Copy, Debug)]
pub struct PendingInteraction {
    /// Stable cell id; dispatcher owns the per-cell counter/state map.
    pub cell_id: u32,
    /// World position the produced effect should emit at.
    pub position: [MilliUnit; 3],
    /// The 16-byte query (MoE input).
    pub query: InteractionQuery,
}

impl PendingInteraction {
    /// Construct a pending interaction.
    pub fn new(cell_id: u32, position: [MilliUnit; 3], query: InteractionQuery) -> Self {
        Self { cell_id, position, query }
    }

    /// Intensity (delegates to the query) — used by the budget for
    /// weakest-discard ordering.
    #[inline]
    pub fn intensity_pmy(&self) -> u16 {
        self.query.intensity_pmy
    }
}

// ---------------------------------------------------------------------------
// LAYOUT LOCKS. Measured with rustc, never hand-computed. These stand in for
// the source's `bytemuck::Pod` derive, which is unavailable in Crate Zero.
// ---------------------------------------------------------------------------
const _: () = assert!(core::mem::size_of::<InteractionQuery>() == 16);
const _: () = assert!(core::mem::align_of::<InteractionQuery>() <= 2);
const _: () = assert!(core::mem::size_of::<Consequence>() == 8);
const _: () = assert!(core::mem::align_of::<Consequence>() <= 2);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_query_is_all_zero_fields() {
        let q = InteractionQuery::default();
        assert_eq!(q.source_tag, 0);
        assert_eq!(q.source_family, 0);
        assert_eq!(q.target_tag, 0);
        assert_eq!(q.target_family, 0);
        assert_eq!(q.intensity_pmy, 0);
        assert_eq!(q.material_id, 0);
        assert_eq!(q.resonance_pmy, 0);
        assert_eq!(q.target_state, 0);
        assert_eq!(q.chain_depth, 0);
        assert_eq!(q.context_celestial, 0);
        assert_eq!(q.velocity_pmy, 0);
        assert_eq!(q.faction, 0);
        assert_eq!(q.relationship, 0);
    }

    #[test]
    fn default_consequence_is_all_zero_fields() {
        let c = Consequence::default();
        assert_eq!(c.kind, 0);
        assert_eq!(c.new_state, 0);
        assert_eq!(c.catalytic_pmy, 0);
        assert_eq!(c.debris_count, 0);
        assert_eq!(c.sound_db, 0);
        assert_eq!(c.plume_kind, 0);
        assert_eq!(c.flags, 0);
    }

    #[test]
    fn consequence_kind_round_trip() {
        for k in [
            ConsequenceKind::None,
            ConsequenceKind::Shatter,
            ConsequenceKind::Ignite,
            ConsequenceKind::Extinguish,
            ConsequenceKind::VoxelBreak,
            ConsequenceKind::VoxelPlace,
            ConsequenceKind::Sound,
            ConsequenceKind::Plume,
            ConsequenceKind::Damage,
            ConsequenceKind::Visibility,
            ConsequenceKind::Reveal,
            ConsequenceKind::MemoryStabilize,
            ConsequenceKind::WitnessTrace,
        ] {
            let c = Consequence { kind: k as u8, ..Consequence::default() };
            assert_eq!(c.kind(), k);
        }
    }

    #[test]
    fn copy_preserves_all_fields() {
        let q = InteractionQuery {
            source_tag: 1,
            source_family: 3,
            target_tag: 0,
            target_family: 0,
            intensity_pmy: 8000,
            material_id: 42,
            resonance_pmy: 5000,
            target_state: 7,
            chain_depth: 2,
            context_celestial: 0x42,
            velocity_pmy: 128,
            faction: 5,
            relationship: 128,
        };
        let q2 = q;
        assert_eq!(q, q2);
    }

    #[test]
    fn grave_bell_wire_decode() {
        let mut bytes = [0u8; 16];
        bytes[0] = QUERY_TAG_GRAVE_BELL;
        let q = InteractionQuery::from_semantic_wire(bytes).expect("grave_bell should decode");
        assert_eq!(q.source_tag, SRC_SOUND);
        assert_eq!(q.source_family, SRC_FAMILY_SOUND);
        assert_eq!(q.target_tag, TGT_STONE);
        assert_eq!(q.target_family, TGT_FAMILY_TERRAIN);
        assert_eq!(q.intensity_pmy, 10_000);
        assert_eq!(q.resonance_pmy, 10_000);
    }

    #[test]
    fn unrecognized_wire_tag_decodes_to_none() {
        assert!(InteractionQuery::from_semantic_wire([0u8; 16]).is_none());
        let mut bytes = [0u8; 16];
        bytes[0] = 0xFF;
        assert!(InteractionQuery::from_semantic_wire(bytes).is_none());
    }

    #[test]
    fn consequence_kind_reveal_round_trip() {
        let k = ConsequenceKind::Reveal;
        let n: u8 = k as u8;
        assert_eq!(n, 10);
        let c = Consequence { kind: n, ..Consequence::default() };
        assert_eq!(c.kind(), ConsequenceKind::Reveal);
    }

    #[test]
    fn consequence_kind_memory_stabilize_round_trip() {
        let k = ConsequenceKind::MemoryStabilize;
        let n: u8 = k as u8;
        assert_eq!(n, 11);
        let c = Consequence { kind: n, ..Consequence::default() };
        assert_eq!(c.kind(), ConsequenceKind::MemoryStabilize);
    }

    #[test]
    fn consequence_kind_witness_trace_round_trip() {
        let k = ConsequenceKind::WitnessTrace;
        let n: u8 = k as u8;
        assert_eq!(n, 12);
        let c = Consequence { kind: n, ..Consequence::default() };
        assert_eq!(c.kind(), ConsequenceKind::WitnessTrace);
    }

    #[test]
    fn pending_interaction_delegates_intensity() {
        let q = InteractionQuery { intensity_pmy: 7777, ..InteractionQuery::default() };
        let p = PendingInteraction::new(42, [MilliUnit(0); 3], q);
        assert_eq!(p.intensity_pmy(), 7777);
    }
}
