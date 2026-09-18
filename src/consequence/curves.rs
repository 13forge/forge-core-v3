//! Ported verbatim from F:\NewRepo\crates\forge-consequence\src\curves.rs (2026-08-17 truth-hunt lineage port, completing the 2026-08-13 wce-tags-port).
//!
//! Precompiled degradation curves keyed by `(source_family, source_tag,
//! target_family, target_tag)`.
//!
//! Generalizes the proof-of-concept pattern from `forge-physics/src/water/erosion.rs`
//! (cell-local, integer-only, one op per cell per tick). Baked at boot from
//! pp-math equations; runtime is pure integer comparisons.
//!
//! 12 curves cover the MVP-plus-bridges set:
//!
//! | id | source              | target                | semantic |
//! |----|---------------------|-----------------------|----------|
//! | 0  | WATER_FLOW          | TGT_STONE             | erosion → `VoxelBreak` |
//! | 1  | FIRE                | TGT_WOOD              | ignite → consume |
//! | 2  | SOUND               | TGT_STONE             | resonance shatter (bard) |
//! | 3  | WATER_FLOW          | TGT_FIRE_CELL         | bridge: extinguish |
//! | 4  | FIRE                | TGT_VOXEL_FLUID       | bridge: evaporation |
//! | 5  | WIND                | TGT_FIRE_CELL         | bridge: directional spread |
//! | 6  | LIGHTNING           | TGT_WOOD              | instant ignite |
//! | 7  | LIGHTNING           | TGT_ENTITY_ALIVE      | instant damage |
//! | 8  | LIGHTNING           | TGT_VOXEL_FLUID       | electrocution (sound+damage) |
//! | 9  | FIRE                | TGT_ENTITY_ALIVE      | bridge: plume visibility |
//! | 10 | GRAVITY_FALL        | TGT_ENTITY_ALIVE      | bridge: collapse damage |
//! | 11 | MOON                | TGT_ORE               | bridge: ore activation |
//! | 12 | SUPERNATURAL(tag 100) | TGT_ENTITY_ALIVE    | Ironroot strike/hit/attack → damage |
//! | 13 | SUPERNATURAL(tag 101) | TGT_ENTITY_ALIVE    | Ironroot craft/forge/brew → sound |
//! | 14 | SUPERNATURAL(tag 102) | TGT_ENTITY_ALIVE    | Ironroot gather/mine/fish → plume |
//! | 15 | SUPERNATURAL(tag 103) | TGT_ENTITY_ALIVE    | Ironroot speak/talk/ask → sound |
//! | 16 | SUPERNATURAL(tag 104) | TGT_ENTITY_ALIVE    | Ironroot purify/tend/heal → reveal (stub, no effect yet) |
//! | 17 | SUPERNATURAL(tag 105) | TGT_ENTITY_ALIVE    | Ironroot read/decode/witness → witnessed (stub, no effect yet) |

use super::query::{Consequence, ConsequenceKind};
use super::tags::*;

/// Stable index into the precompiled curve table.
pub type CurveId = u16;

/// Sentinel for "no curve applies to this (src, tgt, mat) triple".
pub const NO_CURVE: CurveId = u16::MAX;

/// One precompiled degradation curve.
#[derive(Clone, Debug)]
pub struct DegradationCurve {
    /// Display tag (debug only).
    pub label: &'static str,
    /// Source family the curve fires for.
    pub src_family: u8,
    /// Source tag within family.
    pub src_tag: u8,
    /// Target family the curve fires against.
    pub tgt_family: u8,
    /// Target tag within family.
    pub tgt_tag: u8,
    /// Counter thresholds for each state transition (0→1, 1→2, …). Max 8 stages.
    pub thresholds: [u32; 8],
    /// Number of valid thresholds (1..=8).
    pub stage_count: u8,
    /// Base rate added to the counter per tick at intensity=10_000.
    pub base_rate: u32,
    /// Minimum resonance Permyriad required to fire at all (0 = no gate).
    pub resonance_floor_pmy: u16,
    /// Consequences emitted as `target_state` crosses into each stage.
    pub consequence_on_stage: [ConsequenceKind; 8],
    /// Catalytic release Permyriad per stage.
    pub catalytic_on_stage_pmy: [u16; 8],
    /// Per-stage sound emission (0..=255, packed as sound_db in Consequence).
    pub sound_on_stage_db: [u8; 8],
    /// Per-stage plume kind (0=none, 1=smoke, 2=steam, 3=dust, 4=spore).
    pub plume_on_stage: [u8; 8],
}

impl DegradationCurve {
    /// Step the counter and state. Returns `Some(Consequence)` on threshold
    /// crossing, `None` otherwise.
    pub fn step(
        &self,
        counter: &mut u32,
        state: &mut u8,
        intensity_pmy: u16,
        resonance_pmy: u16,
    ) -> Option<Consequence> {
        if resonance_pmy < self.resonance_floor_pmy {
            return None;
        }
        if *state >= self.stage_count {
            return None;
        }

        let delta = ((self.base_rate as u64) * (intensity_pmy as u64) / 10_000) as u32;
        let res_bonus = ((self.base_rate as u64) * (resonance_pmy as u64) / 10_000) as u32;
        *counter = counter.saturating_add(delta).saturating_add(res_bonus);

        let stage = *state as usize;
        let threshold = self.thresholds[stage];
        if *counter >= threshold {
            *state = state.saturating_add(1);
            let kind = self.consequence_on_stage[stage];
            let catalytic = self.catalytic_on_stage_pmy[stage];
            let sound_db = self.sound_on_stage_db[stage];
            let plume_kind = self.plume_on_stage[stage];
            let mut flags = 0u8;
            if matches!(kind, ConsequenceKind::VoxelBreak | ConsequenceKind::VoxelPlace | ConsequenceKind::Shatter) {
                flags |= 0b0000_0001; // terrain mutation
            }
            if matches!(kind, ConsequenceKind::Damage) {
                flags |= 0b0000_0010; // entity damage
            }
            return Some(Consequence {
                kind: kind as u8,
                new_state: *state,
                catalytic_pmy: catalytic,
                debris_count: 0,
                sound_db,
                plume_kind,
                flags,
            });
        }
        None
    }
}

/// Catalog of precompiled curves. Lookup is O(N) scan in MVP; P2 wires the
/// HierarchicalMoE 7×7 router for sub-100ns routing.
#[derive(Clone)]
pub struct CurveTable {
    curves: Vec<DegradationCurve>,
}

impl CurveTable {
    /// Construct the full curve table (3 core + 9 bridges including
    /// lightning). Use this for production / dispatchers that want every
    /// bridge wired.
    pub fn full() -> Self {
        Self { curves: build_full_curves() }
    }

    /// Construct the 3-core MVP table only (water-on-stone, fire-on-wood,
    /// sound-on-stone). Useful for tests that need a minimal surface.
    pub fn mvp() -> Self {
        Self { curves: build_full_curves().into_iter().take(3).collect() }
    }

    /// Lookup by source family+tag and target family+tag. `NO_CURVE` if no
    /// match.
    pub fn lookup(
        &self,
        src_family: u8,
        src_tag: u8,
        tgt_family: u8,
        tgt_tag: u8,
    ) -> CurveId {
        for (i, c) in self.curves.iter().enumerate() {
            if c.src_family == src_family
                && c.src_tag == src_tag
                && c.tgt_family == tgt_family
                && c.tgt_tag == tgt_tag
            {
                return i as CurveId;
            }
        }
        NO_CURVE
    }

    /// Borrow the curve at `id`. Returns `None` for `NO_CURVE` or out-of-range.
    pub fn get(&self, id: CurveId) -> Option<&DegradationCurve> {
        if id == NO_CURVE { return None; }
        self.curves.get(id as usize)
    }

    /// Number of curves in the table.
    #[inline]
    pub fn len(&self) -> usize { self.curves.len() }

    /// Whether the table is empty.
    #[inline]
    pub fn is_empty(&self) -> bool { self.curves.is_empty() }
}

// ── Curve table construction ────────────────────────────────────────────────

fn build_full_curves() -> Vec<DegradationCurve> {
    use ConsequenceKind::*;
    vec![
        // 0: water-on-stone → erosion (mirrors water/erosion.rs pacing)
        DegradationCurve {
            label: "water_on_stone",
            src_family: SRC_FAMILY_FLUID, src_tag: SRC_WATER_FLOW,
            tgt_family: TGT_FAMILY_TERRAIN, tgt_tag: TGT_STONE,
            thresholds: [800, 1_600, 2_400, 3_200, 4_000, 0, 0, 0],
            stage_count: 5, base_rate: 1, resonance_floor_pmy: 0,
            consequence_on_stage: [None, None, None, None, VoxelBreak, None, None, None],
            catalytic_on_stage_pmy: [0, 0, 0, 0, 1_000, 0, 0, 0],
            sound_on_stage_db: [0, 0, 0, 0, 40, 0, 0, 0],
            plume_on_stage: [0; 8],
        },
        // 1: fire-on-wood → ignite then consume (Arrhenius pacing)
        DegradationCurve {
            label: "fire_on_wood",
            src_family: SRC_FAMILY_FIRE, src_tag: SRC_FIRE,
            tgt_family: TGT_FAMILY_TERRAIN, tgt_tag: TGT_WOOD,
            thresholds: [30, 200, 400, 600, 800, 1_000, 1_200, 1_400],
            stage_count: 8, base_rate: 10, resonance_floor_pmy: 0,
            consequence_on_stage: [Ignite, Plume, Plume, Plume, Plume, Plume, VoxelBreak, Extinguish],
            catalytic_on_stage_pmy: [500, 100, 100, 100, 100, 100, 2_000, 500],
            sound_on_stage_db: [50, 30, 30, 30, 30, 30, 60, 20],
            plume_on_stage: [0, 1, 1, 1, 1, 1, 0, 0],
        },
        // 2: sound-on-stone → resonance shatter (bard scenario)
        DegradationCurve {
            label: "sound_on_stone",
            src_family: SRC_FAMILY_SOUND, src_tag: SRC_SOUND,
            tgt_family: TGT_FAMILY_TERRAIN, tgt_tag: TGT_STONE,
            thresholds: [100, 200, 400, 800, 1_600, 3_200, 6_400, 12_800],
            stage_count: 8, base_rate: 1, resonance_floor_pmy: 8_000,
            consequence_on_stage: [None, None, None, None, None, None, None, Shatter],
            catalytic_on_stage_pmy: [0, 0, 0, 0, 0, 0, 0, 4_000],
            sound_on_stage_db: [0; 8],
            plume_on_stage: [0; 8],
        },
        // 3: water-on-fire → extinguish (bridge)
        DegradationCurve {
            label: "water_on_fire",
            src_family: SRC_FAMILY_FLUID, src_tag: SRC_WATER_FLOW,
            tgt_family: TGT_FAMILY_ATMOSPHERE, tgt_tag: TGT_FIRE_CELL,
            thresholds: [20, 0, 0, 0, 0, 0, 0, 0],
            stage_count: 1, base_rate: 10, resonance_floor_pmy: 0,
            consequence_on_stage: [Extinguish, None, None, None, None, None, None, None],
            catalytic_on_stage_pmy: [200, 0, 0, 0, 0, 0, 0, 0],
            sound_on_stage_db: [30, 0, 0, 0, 0, 0, 0, 0],
            // Plume kind 2 = steam (existing PlumeSource enum lacks Steam;
            // mapped to PlumeSource::Fire as MVP placeholder in dispatch.rs).
            plume_on_stage: [2, 0, 0, 0, 0, 0, 0, 0],
        },
        // 4: fire-on-water → evaporation (bridge)
        DegradationCurve {
            label: "fire_on_water",
            src_family: SRC_FAMILY_FIRE, src_tag: SRC_FIRE,
            tgt_family: TGT_FAMILY_FLUID, tgt_tag: TGT_VOXEL_FLUID,
            thresholds: [50, 100, 0, 0, 0, 0, 0, 0],
            stage_count: 2, base_rate: 5, resonance_floor_pmy: 0,
            consequence_on_stage: [Plume, VoxelBreak, None, None, None, None, None, None],
            catalytic_on_stage_pmy: [50, 100, 0, 0, 0, 0, 0, 0],
            sound_on_stage_db: [25, 35, 0, 0, 0, 0, 0, 0],
            plume_on_stage: [2, 2, 0, 0, 0, 0, 0, 0], // steam
        },
        // 5: wind-on-fire → directional spread ignition (bridge)
        DegradationCurve {
            label: "wind_on_fire",
            src_family: SRC_FAMILY_TIME, src_tag: SRC_WIND,
            tgt_family: TGT_FAMILY_ATMOSPHERE, tgt_tag: TGT_FIRE_CELL,
            thresholds: [40, 0, 0, 0, 0, 0, 0, 0],
            stage_count: 1, base_rate: 4, resonance_floor_pmy: 0,
            consequence_on_stage: [Ignite, None, None, None, None, None, None, None],
            catalytic_on_stage_pmy: [300, 0, 0, 0, 0, 0, 0, 0],
            sound_on_stage_db: [40, 0, 0, 0, 0, 0, 0, 0],
            plume_on_stage: [1, 0, 0, 0, 0, 0, 0, 0],
        },
        // 6: lightning-on-wood → instant ignite (very low threshold)
        DegradationCurve {
            label: "lightning_on_wood",
            src_family: SRC_FAMILY_GRAVITY, src_tag: SRC_LIGHTNING,
            tgt_family: TGT_FAMILY_TERRAIN, tgt_tag: TGT_WOOD,
            thresholds: [1, 0, 0, 0, 0, 0, 0, 0],
            stage_count: 1, base_rate: 10_000, resonance_floor_pmy: 0,
            consequence_on_stage: [Ignite, None, None, None, None, None, None, None],
            catalytic_on_stage_pmy: [10_000, 0, 0, 0, 0, 0, 0, 0],
            sound_on_stage_db: [120, 0, 0, 0, 0, 0, 0, 0],
            plume_on_stage: [1, 0, 0, 0, 0, 0, 0, 0],
        },
        // 7: lightning-on-entity → instant damage
        DegradationCurve {
            label: "lightning_on_entity",
            src_family: SRC_FAMILY_GRAVITY, src_tag: SRC_LIGHTNING,
            tgt_family: TGT_FAMILY_ENTITY, tgt_tag: TGT_ENTITY_ALIVE,
            thresholds: [1, 0, 0, 0, 0, 0, 0, 0],
            stage_count: 1, base_rate: 10_000, resonance_floor_pmy: 0,
            consequence_on_stage: [Damage, None, None, None, None, None, None, None],
            catalytic_on_stage_pmy: [10_000, 0, 0, 0, 0, 0, 0, 0],
            sound_on_stage_db: [120, 0, 0, 0, 0, 0, 0, 0],
            plume_on_stage: [0; 8],
        },
        // 8: lightning-on-water → electrocution (sound + damage flag)
        DegradationCurve {
            label: "lightning_on_water",
            src_family: SRC_FAMILY_GRAVITY, src_tag: SRC_LIGHTNING,
            tgt_family: TGT_FAMILY_FLUID, tgt_tag: TGT_VOXEL_FLUID,
            thresholds: [1, 0, 0, 0, 0, 0, 0, 0],
            stage_count: 1, base_rate: 10_000, resonance_floor_pmy: 0,
            consequence_on_stage: [Damage, None, None, None, None, None, None, None],
            catalytic_on_stage_pmy: [10_000, 0, 0, 0, 0, 0, 0, 0],
            sound_on_stage_db: [110, 0, 0, 0, 0, 0, 0, 0],
            plume_on_stage: [2, 0, 0, 0, 0, 0, 0, 0], // steam from electrolysis
        },
        // 9: fire-on-entity → plume visibility reduction (bridge)
        DegradationCurve {
            label: "fire_on_entity_vision",
            src_family: SRC_FAMILY_FIRE, src_tag: SRC_FIRE,
            tgt_family: TGT_FAMILY_ENTITY, tgt_tag: TGT_ENTITY_ALIVE,
            thresholds: [100, 0, 0, 0, 0, 0, 0, 0],
            stage_count: 1, base_rate: 10, resonance_floor_pmy: 0,
            consequence_on_stage: [Visibility, None, None, None, None, None, None, None],
            catalytic_on_stage_pmy: [0, 0, 0, 0, 0, 0, 0, 0],
            sound_on_stage_db: [0; 8],
            plume_on_stage: [0; 8],
        },
        // 10: gravity-on-entity → collapse damage (bridge: structural→entity)
        DegradationCurve {
            label: "gravity_on_entity_damage",
            src_family: SRC_FAMILY_GRAVITY, src_tag: SRC_GRAVITY_FALL,
            tgt_family: TGT_FAMILY_ENTITY, tgt_tag: TGT_ENTITY_ALIVE,
            thresholds: [50, 0, 0, 0, 0, 0, 0, 0],
            stage_count: 1, base_rate: 50, resonance_floor_pmy: 0,
            consequence_on_stage: [Damage, None, None, None, None, None, None, None],
            catalytic_on_stage_pmy: [2_000, 0, 0, 0, 0, 0, 0, 0],
            sound_on_stage_db: [80, 0, 0, 0, 0, 0, 0, 0],
            plume_on_stage: [3, 0, 0, 0, 0, 0, 0, 0], // dust
        },
        // 11: moon-on-ore → activation (bridge: celestial→material)
        DegradationCurve {
            label: "moon_on_ore",
            src_family: SRC_FAMILY_TIME, src_tag: SRC_MOON,
            tgt_family: TGT_FAMILY_TERRAIN, tgt_tag: TGT_ORE,
            thresholds: [200, 0, 0, 0, 0, 0, 0, 0],
            stage_count: 1, base_rate: 5, resonance_floor_pmy: 0,
            consequence_on_stage: [Sound, None, None, None, None, None, None, None],
            catalytic_on_stage_pmy: [5_000, 0, 0, 0, 0, 0, 0, 0],
            sound_on_stage_db: [60, 0, 0, 0, 0, 0, 0, 0],
            plume_on_stage: [0; 8],
        },
        // 12: Ironroot strike/hit/attack (skill_idx 0, tag 100 of the reserved 100..=110
        // SUPERNATURAL range — see tags.rs) on entity → escalating damage.
        DegradationCurve {
            label: "ironroot_strike_on_entity",
            src_family: SRC_FAMILY_SUPERNATURAL, src_tag: 100,
            tgt_family: TGT_FAMILY_ENTITY, tgt_tag: TGT_ENTITY_ALIVE,
            thresholds: [200, 500, 900, 1_500, 0, 0, 0, 0],
            stage_count: 4, base_rate: 50, resonance_floor_pmy: 0,
            consequence_on_stage: [Damage, Damage, Damage, Damage, None, None, None, None],
            catalytic_on_stage_pmy: [1_000, 2_500, 5_000, 10_000, 0, 0, 0, 0],
            sound_on_stage_db: [0; 8],
            plume_on_stage: [0; 8],
        },
        // 13: Ironroot craft/forge/brew (skill_idx 1, tag 101) on entity → forge clang.
        DegradationCurve {
            label: "ironroot_craft_on_entity",
            src_family: SRC_FAMILY_SUPERNATURAL, src_tag: 101,
            tgt_family: TGT_FAMILY_ENTITY, tgt_tag: TGT_ENTITY_ALIVE,
            thresholds: [200, 500, 900, 1_500, 0, 0, 0, 0],
            stage_count: 4, base_rate: 50, resonance_floor_pmy: 0,
            consequence_on_stage: [Sound, Sound, Sound, Sound, None, None, None, None],
            catalytic_on_stage_pmy: [1_000, 2_500, 5_000, 10_000, 0, 0, 0, 0],
            sound_on_stage_db: [0; 8],
            plume_on_stage: [0; 8],
        },
        // 14: Ironroot gather/mine/fish (skill_idx 2, tag 102) on entity → kicked-up dust.
        DegradationCurve {
            label: "ironroot_gather_on_entity",
            src_family: SRC_FAMILY_SUPERNATURAL, src_tag: 102,
            tgt_family: TGT_FAMILY_ENTITY, tgt_tag: TGT_ENTITY_ALIVE,
            thresholds: [200, 500, 900, 1_500, 0, 0, 0, 0],
            stage_count: 4, base_rate: 50, resonance_floor_pmy: 0,
            consequence_on_stage: [Plume, Plume, Plume, Plume, None, None, None, None],
            catalytic_on_stage_pmy: [1_000, 2_500, 5_000, 10_000, 0, 0, 0, 0],
            sound_on_stage_db: [0; 8],
            plume_on_stage: [3, 3, 3, 3, 0, 0, 0, 0], // dust
        },
        // 15: Ironroot speak/talk/ask (skill_idx 3, tag 103) on entity → voice.
        DegradationCurve {
            label: "ironroot_speak_on_entity",
            src_family: SRC_FAMILY_SUPERNATURAL, src_tag: 103,
            tgt_family: TGT_FAMILY_ENTITY, tgt_tag: TGT_ENTITY_ALIVE,
            thresholds: [200, 500, 900, 1_500, 0, 0, 0, 0],
            stage_count: 4, base_rate: 50, resonance_floor_pmy: 0,
            consequence_on_stage: [Sound, Sound, Sound, Sound, None, None, None, None],
            catalytic_on_stage_pmy: [1_000, 2_500, 5_000, 10_000, 0, 0, 0, 0],
            sound_on_stage_db: [0; 8],
            plume_on_stage: [0; 8],
        },
        // 16: Ironroot purify/tend/heal (skill_idx 4, tag 104) on entity → reveal.
        // ConsequenceKind::Reveal is a documented existing stub (consequence_to_effect
        // maps it to None — "sem-δ will wire these"); accepted honest gap, not silently
        // dropped.
        DegradationCurve {
            label: "ironroot_purify_on_entity",
            src_family: SRC_FAMILY_SUPERNATURAL, src_tag: 104,
            tgt_family: TGT_FAMILY_ENTITY, tgt_tag: TGT_ENTITY_ALIVE,
            thresholds: [200, 500, 900, 1_500, 0, 0, 0, 0],
            stage_count: 4, base_rate: 50, resonance_floor_pmy: 0,
            consequence_on_stage: [Reveal, Reveal, Reveal, Reveal, None, None, None, None],
            catalytic_on_stage_pmy: [1_000, 2_500, 5_000, 10_000, 0, 0, 0, 0],
            sound_on_stage_db: [0; 8],
            plume_on_stage: [0; 8],
        },
        // 17: Ironroot read/decode/witness (skill_idx 5, tag 105) on entity → witnessed.
        // Same documented stub as curve 16 (WitnessTrace -> None today).
        DegradationCurve {
            label: "ironroot_witness_on_entity",
            src_family: SRC_FAMILY_SUPERNATURAL, src_tag: 105,
            tgt_family: TGT_FAMILY_ENTITY, tgt_tag: TGT_ENTITY_ALIVE,
            thresholds: [200, 500, 900, 1_500, 0, 0, 0, 0],
            stage_count: 4, base_rate: 50, resonance_floor_pmy: 0,
            consequence_on_stage: [WitnessTrace, WitnessTrace, WitnessTrace, WitnessTrace, None, None, None, None],
            catalytic_on_stage_pmy: [1_000, 2_500, 5_000, 10_000, 0, 0, 0, 0],
            sound_on_stage_db: [0; 8],
            plume_on_stage: [0; 8],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_table_has_eighteen_curves() {
        // 12 core + bridges, + 6 (2026-07-18): one Ironroot MUD curve per verb group,
        // tags 100-105 of the reserved 100..=110 SUPERNATURAL range (see tags.rs).
        let t = CurveTable::full();
        assert_eq!(t.len(), 18);
    }

    #[test]
    fn mvp_table_has_three_curves() {
        let t = CurveTable::mvp();
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn all_curves_have_unique_keys() {
        // No two curves should collide on (src_family, src_tag, tgt_family, tgt_tag).
        let t = CurveTable::full();
        for (i, a) in t.curves.iter().enumerate() {
            for (j, b) in t.curves.iter().enumerate() {
                if i == j { continue; }
                let same_key = a.src_family == b.src_family
                    && a.src_tag == b.src_tag
                    && a.tgt_family == b.tgt_family
                    && a.tgt_tag == b.tgt_tag;
                assert!(!same_key,
                    "curves {} ({}) and {} ({}) share the same (src, tgt) key",
                    i, a.label, j, b.label);
            }
        }
    }

    #[test]
    fn lookup_finds_water_on_stone() {
        let t = CurveTable::full();
        let id = t.lookup(SRC_FAMILY_FLUID, SRC_WATER_FLOW, TGT_FAMILY_TERRAIN, TGT_STONE);
        assert_eq!(id, 0);
        assert_eq!(t.get(id).unwrap().label, "water_on_stone");
    }

    #[test]
    fn lookup_finds_lightning_curves() {
        let t = CurveTable::full();
        assert_ne!(t.lookup(SRC_FAMILY_GRAVITY, SRC_LIGHTNING, TGT_FAMILY_TERRAIN, TGT_WOOD), NO_CURVE);
        assert_ne!(t.lookup(SRC_FAMILY_GRAVITY, SRC_LIGHTNING, TGT_FAMILY_ENTITY, TGT_ENTITY_ALIVE), NO_CURVE);
        assert_ne!(t.lookup(SRC_FAMILY_GRAVITY, SRC_LIGHTNING, TGT_FAMILY_FLUID, TGT_VOXEL_FLUID), NO_CURVE);
    }

    #[test]
    fn lookup_finds_all_eight_bridges() {
        let t = CurveTable::full();
        // Bridges per the design discussion.
        // (water-on-fire, fire-on-water, wind-on-fire, lightning, plume-visibility,
        //  structural-on-entity, moon-on-material; gravity-on-fluid not yet wired.)
        let bridges = [
            (SRC_FAMILY_FLUID, SRC_WATER_FLOW, TGT_FAMILY_ATMOSPHERE, TGT_FIRE_CELL),
            (SRC_FAMILY_FIRE, SRC_FIRE, TGT_FAMILY_FLUID, TGT_VOXEL_FLUID),
            (SRC_FAMILY_TIME, SRC_WIND, TGT_FAMILY_ATMOSPHERE, TGT_FIRE_CELL),
            (SRC_FAMILY_FIRE, SRC_FIRE, TGT_FAMILY_ENTITY, TGT_ENTITY_ALIVE),
            (SRC_FAMILY_GRAVITY, SRC_GRAVITY_FALL, TGT_FAMILY_ENTITY, TGT_ENTITY_ALIVE),
            (SRC_FAMILY_TIME, SRC_MOON, TGT_FAMILY_TERRAIN, TGT_ORE),
        ];
        for (sf, st, tf, tt) in bridges {
            let id = t.lookup(sf, st, tf, tt);
            assert_ne!(id, NO_CURVE,
                "bridge ({sf},{st}) → ({tf},{tt}) missing from table");
        }
    }

    #[test]
    fn lookup_returns_no_curve_for_unknown() {
        let t = CurveTable::full();
        let id = t.lookup(SRC_FAMILY_ENTITY, 99, TGT_FAMILY_ATMOSPHERE, 99);
        assert_eq!(id, NO_CURVE);
        assert!(t.get(id).is_none());
    }

    #[test]
    fn water_on_stone_accumulates_then_erodes() {
        let t = CurveTable::full();
        let c = t.get(0).unwrap();
        let (mut counter, mut state) = (0u32, 0u8);
        // Drive ~4_000 ticks of intensity=10_000 (delta=1 each, no resonance).
        for _ in 0..3_999 { let _ = c.step(&mut counter, &mut state, 10_000, 0); }
        let _ = c.step(&mut counter, &mut state, 10_000, 0);
        assert_eq!(state, 5, "VoxelBreak fires at stage 4 → 5");
    }

    #[test]
    fn fire_on_wood_ignites_then_extinguishes() {
        let t = CurveTable::full();
        let c = t.get(1).unwrap();
        let (mut counter, mut state) = (0u32, 0u8);
        let mut fired = vec![];
        for _ in 0..200 {
            if let Some(c) = c.step(&mut counter, &mut state, 10_000, 0) {
                fired.push(c.kind());
            }
        }
        assert!(fired.contains(&ConsequenceKind::Ignite));
        assert!(fired.contains(&ConsequenceKind::Plume));
    }

    #[test]
    fn sound_on_stone_requires_resonance_floor() {
        let t = CurveTable::full();
        let c = t.get(2).unwrap();
        let (mut counter, mut state) = (0u32, 0u8);
        for _ in 0..100 {
            let _ = c.step(&mut counter, &mut state, 10_000, 5_000);
        }
        assert_eq!(state, 0);
        for _ in 0..200 {
            let _ = c.step(&mut counter, &mut state, 10_000, 9_500);
        }
        assert!(state > 0);
    }

    #[test]
    fn sound_on_stone_bard_scenario_emits_exactly_one_shatter() {
        let t = CurveTable::full();
        let c = t.get(2).unwrap();
        let (mut counter, mut state) = (0u32, 0u8);
        let mut shatters = 0;
        for _ in 0..20_000 {
            if let Some(c) = c.step(&mut counter, &mut state, 10_000, 10_000) {
                if c.kind() == ConsequenceKind::Shatter { shatters += 1; }
            }
        }
        assert_eq!(shatters, 1);
        assert_eq!(state, 8);
    }

    #[test]
    fn lightning_on_wood_ignites_instantly() {
        let t = CurveTable::full();
        let id = t.lookup(SRC_FAMILY_GRAVITY, SRC_LIGHTNING, TGT_FAMILY_TERRAIN, TGT_WOOD);
        let c = t.get(id).unwrap();
        let (mut counter, mut state) = (0u32, 0u8);
        let consequence = c.step(&mut counter, &mut state, 10_000, 0);
        assert!(consequence.is_some(), "lightning should fire on the FIRST tick");
        assert_eq!(consequence.unwrap().kind(), ConsequenceKind::Ignite);
    }

    #[test]
    fn lightning_on_entity_damages_instantly() {
        let t = CurveTable::full();
        let id = t.lookup(SRC_FAMILY_GRAVITY, SRC_LIGHTNING, TGT_FAMILY_ENTITY, TGT_ENTITY_ALIVE);
        let c = t.get(id).unwrap();
        let (mut counter, mut state) = (0u32, 0u8);
        let consequence = c.step(&mut counter, &mut state, 10_000, 0).unwrap();
        assert_eq!(consequence.kind(), ConsequenceKind::Damage);
        // Entity damage flag must be set.
        assert!(consequence.flags & 0b0000_0010 != 0);
    }

    #[test]
    fn water_on_fire_extinguishes() {
        let t = CurveTable::full();
        let id = t.lookup(SRC_FAMILY_FLUID, SRC_WATER_FLOW, TGT_FAMILY_ATMOSPHERE, TGT_FIRE_CELL);
        let c = t.get(id).unwrap();
        let (mut counter, mut state) = (0u32, 0u8);
        let mut fired = None;
        for _ in 0..10 {
            if let Some(c) = c.step(&mut counter, &mut state, 10_000, 0) {
                fired = Some(c);
                break;
            }
        }
        assert!(fired.is_some());
        assert_eq!(fired.unwrap().kind(), ConsequenceKind::Extinguish);
    }

    #[test]
    fn fire_on_water_evaporates_with_steam() {
        let t = CurveTable::full();
        let id = t.lookup(SRC_FAMILY_FIRE, SRC_FIRE, TGT_FAMILY_FLUID, TGT_VOXEL_FLUID);
        let c = t.get(id).unwrap();
        let (mut counter, mut state) = (0u32, 0u8);
        let mut kinds = vec![];
        for _ in 0..40 {
            if let Some(c) = c.step(&mut counter, &mut state, 10_000, 0) {
                kinds.push(c.kind());
                // Steam plume kind = 2.
                assert!(c.plume_kind == 2 || c.plume_kind == 0);
            }
        }
        assert!(kinds.contains(&ConsequenceKind::Plume), "should emit steam plume");
        assert!(kinds.contains(&ConsequenceKind::VoxelBreak), "should remove water voxel");
    }

    #[test]
    fn moon_on_ore_activates() {
        let t = CurveTable::full();
        let id = t.lookup(SRC_FAMILY_TIME, SRC_MOON, TGT_FAMILY_TERRAIN, TGT_ORE);
        let c = t.get(id).unwrap();
        let (mut counter, mut state) = (0u32, 0u8);
        let mut activated = false;
        for _ in 0..200 {
            if let Some(c) = c.step(&mut counter, &mut state, 10_000, 0) {
                if c.kind() == ConsequenceKind::Sound {
                    activated = true;
                    assert!(c.catalytic_pmy >= 1_000, "activation releases stored celestial energy");
                    break;
                }
            }
        }
        assert!(activated);
    }
}
