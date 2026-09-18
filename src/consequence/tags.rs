//! Source and target tag taxonomy.
//!
//! Ported 2026-08-13 from `F:\NewRepo\crates\forge-consequence\src\tags.rs`
//! (v2, real, tested 4/4) — verbatim, zero new dependencies. The 2026-08-13
//! `/aspire` WCE-CONSEQUENCE run (glyph `\u{1483}`, row `wce-tags-port`)
//! named this the first weld: everything else in the WCE port depends on it.
//!
//! Two-level: **families** (7 of each — matches Invention #169 HierarchicalMoE
//! 7×7=49 cells) and **specific tags** (per family, u8 distinct).
//!
//! Families address the routing cell (which sub-expert handles this pair).
//! Tags address the specific physics within the cell (fire vs heat, water-flow
//! vs water-pressure). Both fit in `u8`, so 49 × 256² variants are addressable
//! without growing the 16-byte `InteractionQuery` (ported separately).
//!
//! Game-side custom tags (e.g. ironroot's fae content) use unused values
//! within an existing family (`SRC_FAMILY_SUPERNATURAL`) so the engine
//! doesn't need to know about them — registration is implicit via the curve
//! table. Ironroot's real reserved range is `100..=110` (v2 doc comment,
//! carried forward below) — this is the fae ethics overlay's actual tag home.

// ── Source families (Tier-1 routing, 1-of-7 MoE) ────────────────────────────

/// Heat, flame, combustion.
pub const SRC_FAMILY_FIRE: u8 = 0;
/// Liquid flow, pressure, precipitation.
pub const SRC_FAMILY_FLUID: u8 = 1;
/// Gravity, impact, explosion, falling debris, lightning.
pub const SRC_FAMILY_GRAVITY: u8 = 2;
/// Acoustic energy, rhythm, silence-as-force.
pub const SRC_FAMILY_SOUND: u8 = 3;
/// Blessed, cursed, spirit, dream, ironroot — anything outside physical
/// sources. Game-side custom tags register inside this family.
pub const SRC_FAMILY_SUPERNATURAL: u8 = 4;
/// Decay, season, moon phase, aging, wind direction.
pub const SRC_FAMILY_TIME: u8 = 5;
/// Player, fauna, NPC, group movement.
pub const SRC_FAMILY_ENTITY: u8 = 6;

// ── Target families (Tier-2 routing, 1-of-7 MoE) ────────────────────────────

/// Voxel terrain — stone, dirt, ore, wood (as voxel material).
pub const TGT_FAMILY_TERRAIN: u8 = 0;
/// Water cells, fluid bodies.
pub const TGT_FAMILY_FLUID: u8 = 1;
/// Buildings, walls, connected voxel structures.
pub const TGT_FAMILY_STRUCTURE: u8 = 2;
/// Creatures, players — anything with entity physics.
pub const TGT_FAMILY_ENTITY: u8 = 3;
/// Smoke, dust, spores, gas — plume state.
pub const TGT_FAMILY_PLUME: u8 = 4;
/// Dropped items, ore, crafting materials.
pub const TGT_FAMILY_ITEM: u8 = 5;
/// Air, light, sound waves — non-voxel field state. Projectile medium.
pub const TGT_FAMILY_ATMOSPHERE: u8 = 6;

// ── Specific source tags ────────────────────────────────────────────────────

// SRC_FAMILY_FIRE
/// Active combustion source.
pub const SRC_FIRE: u8 = 0;
/// Radiant heat without flame (forge, sun, lava).
pub const SRC_HEAT: u8 = 1;

// SRC_FAMILY_FLUID
/// Moving water (rivers, runoff, springs).
pub const SRC_WATER_FLOW: u8 = 0;
/// Confined-water pressure (pipes, hydrostatic, dams).
pub const SRC_WATER_PRESSURE: u8 = 1;
/// Falling precipitation.
pub const SRC_RAIN: u8 = 2;

// SRC_FAMILY_GRAVITY
/// Falling debris / freefall.
pub const SRC_GRAVITY_FALL: u8 = 0;
/// Detonation / blast wave.
pub const SRC_EXPLOSION: u8 = 1;
/// Direct kinetic impact.
pub const SRC_IMPACT: u8 = 2;
/// Lightning strike (atmospheric → ground). Damage + ignition.
pub const SRC_LIGHTNING: u8 = 3;

// SRC_FAMILY_SOUND
/// Single acoustic source.
pub const SRC_SOUND: u8 = 0;
/// Sustained periodic source (drum, chant).
pub const SRC_RHYTHM: u8 = 1;
/// Active silence — sound-consuming zone.
pub const SRC_SILENCE: u8 = 2;

// SRC_FAMILY_SUPERNATURAL
/// Blessed (favorable) supernatural force.
pub const SRC_BLESSED: u8 = 0;
/// Cursed (hostile) supernatural force.
pub const SRC_CURSED: u8 = 1;
/// Generic spirit interaction.
pub const SRC_SPIRIT: u8 = 2;
// 3..=255 reserved for game-side registration (Ironroot uses 100..=110
// by convention — engine never sees these symbolically; this is the fae
// ethics overlay's real tag home, named not-yet-authored per the
// 2026-08-13 aspire run).
const _: () = assert!(SRC_BLESSED < 3);
const _: () = assert!(SRC_CURSED < 3);
const _: () = assert!(SRC_SPIRIT < 3);

// SRC_FAMILY_TIME
/// Wear / corrosion / weathering.
pub const SRC_TIME_DECAY: u8 = 0;
/// Seasonal cycle tick (winter freeze, spring growth, autumn fall).
pub const SRC_SEASON: u8 = 1;
/// Moon phase change (modulates supernatural and materials).
pub const SRC_MOON: u8 = 2;
/// Wind vector (direction + speed encoded via intensity + velocity bytes).
pub const SRC_WIND: u8 = 3;

// SRC_FAMILY_ENTITY
/// Player action / movement.
pub const SRC_PLAYER: u8 = 0;
/// Fauna action / movement.
pub const SRC_FAUNA: u8 = 1;
/// NPC action.
pub const SRC_NPC: u8 = 2;

// ── Specific target tags ────────────────────────────────────────────────────

// TGT_FAMILY_TERRAIN
/// Stone voxel.
pub const TGT_STONE: u8 = 0;
/// Wood voxel.
pub const TGT_WOOD: u8 = 1;
/// Dirt / soil voxel.
pub const TGT_DIRT: u8 = 2;
/// Ore vein voxel (mineral activation by moonlight, lightning conductivity).
pub const TGT_ORE: u8 = 3;

// TGT_FAMILY_FLUID
/// Active water cell with depth > 0.
pub const TGT_VOXEL_FLUID: u8 = 0;
/// Ice (frozen fluid).
pub const TGT_ICE: u8 = 1;
/// Steam / vapor field.
pub const TGT_STEAM: u8 = 2;

// TGT_FAMILY_STRUCTURE
/// Building / wall / connected-voxel assembly.
pub const TGT_BUILDING: u8 = 0;
/// Bridge / span — load propagates across multiple voxels.
pub const TGT_BRIDGE: u8 = 1;

// TGT_FAMILY_ENTITY
/// Any living entity (player, fauna, NPC).
pub const TGT_ENTITY_ALIVE: u8 = 0;
/// Corpse / dead entity (decay target).
pub const TGT_CORPSE: u8 = 1;

// TGT_FAMILY_PLUME
/// Smoke plume.
pub const TGT_PLUME_SMOKE: u8 = 0;
/// Dust plume.
pub const TGT_PLUME_DUST: u8 = 1;
/// Spore plume.
pub const TGT_PLUME_SPORE: u8 = 2;

// TGT_FAMILY_ITEM
/// Dropped item / crafting material.
pub const TGT_ITEM: u8 = 0;

// TGT_FAMILY_ATMOSPHERE
/// Generic air / atmosphere cell.
pub const TGT_AIR: u8 = 0;
/// In-flight projectile (medium = air/water determines drag).
pub const TGT_PROJECTILE: u8 = 1;
/// Sound wave (target for silence-as-force, interference).
pub const TGT_SOUND_WAVE: u8 = 2;
/// Active fire cell. Targets like "water flowing onto this fire" or "wind
/// blowing on this fire".
pub const TGT_FIRE_CELL: u8 = 3;

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Verify a source family fits the 7-family taxonomy.
#[inline]
pub const fn src_family(value: u8) -> Option<u8> {
    if value <= SRC_FAMILY_ENTITY { Some(value) } else { None }
}

/// Verify a target family fits the 7-family taxonomy.
#[inline]
pub const fn tgt_family(value: u8) -> Option<u8> {
    if value <= TGT_FAMILY_ATMOSPHERE { Some(value) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_source_families_in_range() {
        for f in 0..=6u8 {
            assert_eq!(src_family(f), Some(f), "family {f} should be valid");
        }
        assert_eq!(src_family(7), None);
    }

    #[test]
    fn seven_target_families_in_range() {
        for f in 0..=6u8 {
            assert_eq!(tgt_family(f), Some(f));
        }
        assert_eq!(tgt_family(7), None);
    }

    #[test]
    fn lightning_tag_distinct_from_gravity_subtags() {
        assert_ne!(SRC_LIGHTNING, SRC_GRAVITY_FALL);
        assert_ne!(SRC_LIGHTNING, SRC_EXPLOSION);
        assert_ne!(SRC_LIGHTNING, SRC_IMPACT);
    }

    // supernatural_reserves_slots_for_game_tags: enforced at compile time via
    // `const _: () = assert!(...)` next to SRC_BLESSED/SRC_CURSED/SRC_SPIRIT
    // definitions above. No runtime test needed.
}
