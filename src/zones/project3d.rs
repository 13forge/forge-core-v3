//! Sphere carve/fill mutation over `PexilChunk`, audited into
//! `MutationLedger`. Algorithm ported from `forge-canvas-v3/src/
//! sphere_brush.rs` (integer sphere-membership test, no sqrt, no float) —
//! applied fresh to `PexilChunk` rather than `StructuralBox`. Every
//! changed cell is captured before/after into the ledger; unchanged cells
//! (already the target material) cost nothing.

use crate::atom::{CellOrdinal, Pexil, ValidityMask};
use crate::zones::ledger::MutationLedger;
use crate::zones::storage::PexilChunk;

/// Empty-cell sentinel, matching `sphere_brush.rs`'s `AIR` convention.
pub const AIR: u8 = 0;

/// True if cell `(x,y,z)` lies within the integer sphere centred at `c`
/// with `radius` (inclusive). Pure integer — `dx²+dy²+dz²≤r²`, no `sqrt`.
#[inline]
pub fn in_sphere(c: (i64, i64, i64), radius: i64, x: i64, y: i64, z: i64) -> bool {
    let (dx, dy, dz) = (x - c.0, y - c.1, z - c.2);
    dx * dx + dy * dy + dz * dz <= radius * radius
}

/// Carve (set to [`AIR`]) every in-sphere cell of `chunk` centred at
/// `center` with the given `radius`, sealing each real change into
/// `ledger` at `tick`. Returns the number of cells actually changed.
pub fn carve_sphere(
    chunk: &mut PexilChunk,
    ledger: &mut MutationLedger,
    tick: u64,
    center: (usize, usize, usize),
    radius: usize,
) -> u32 {
    sphere_set(chunk, ledger, tick, center, radius, AIR)
}

/// Fill every in-sphere cell of `chunk` centred at `center` with `radius`
/// to `material` (should be nonzero), sealing each real change into
/// `ledger` at `tick`. Returns the number of cells actually changed.
pub fn fill_sphere(
    chunk: &mut PexilChunk,
    ledger: &mut MutationLedger,
    tick: u64,
    center: (usize, usize, usize),
    radius: usize,
    material: u8,
) -> u32 {
    sphere_set(chunk, ledger, tick, center, radius, material)
}

/// Shared carve/fill walk: bounded to the sphere's own AABB (clipped to
/// the chunk), in place, zero extra allocation beyond the ledger's own
/// row storage.
fn sphere_set(
    chunk: &mut PexilChunk,
    ledger: &mut MutationLedger,
    tick: u64,
    center: (usize, usize, usize),
    radius: usize,
    material: u8,
) -> u32 {
    let edge = chunk.edge();
    let (cx, cy, cz) = (center.0 as i64, center.1 as i64, center.2 as i64);
    let r = radius as i64;

    let lo = |c: i64| (c - r).max(0) as usize;
    let hi = |c: i64| ((c + r).max(0) as usize + 1).min(edge);

    let mut changed = 0u32;
    for z in lo(cz)..hi(cz) {
        for y in lo(cy)..hi(cy) {
            for x in lo(cx)..hi(cx) {
                if !in_sphere((cx, cy, cz), r, x as i64, y as i64, z as i64) {
                    continue;
                }
                let Some(before) = chunk.get(x, y, z).copied() else { continue };
                if before.payload[0] == material {
                    continue;
                }
                if let Some(cell) = chunk.get_mut(x, y, z) {
                    cell.payload[0] = material;
                    cell.validity = ValidityMask::ALL_KNOWN;
                    let after: Pexil = *cell;
                    let ordinal: CellOrdinal = before.ordinal;
                    // No world-layer concept at this single-chunk level (that's
                    // `SparseChunkGrid`'s job) — `w=0` is the only sensible value.
                    ledger.append(ordinal, tick, before, after, (x, y, z, 0));
                    changed += 1;
                }
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_sphere_boundary_inclusive() {
        assert!(in_sphere((0, 0, 0), 5, 5, 0, 0));
        assert!(!in_sphere((0, 0, 0), 5, 6, 0, 0));
    }

    #[test]
    fn carve_sphere_changes_only_in_sphere_cells_and_ledgers_each() {
        let mut chunk = PexilChunk::new(9);
        for z in 0..9 {
            for y in 0..9 {
                for x in 0..9 {
                    chunk.get_mut(x, y, z).unwrap().payload[0] = 7;
                }
            }
        }
        let mut ledger = MutationLedger::new();
        let changed = carve_sphere(&mut chunk, &mut ledger, 100, (4, 4, 4), 2);

        assert!(changed > 0);
        assert_eq!(ledger.len() as u32, changed);
        assert_eq!(chunk.get(4, 4, 4).unwrap().payload[0], AIR);
        // A far corner, well outside radius 2 of the center, stays untouched.
        assert_eq!(chunk.get(0, 0, 0).unwrap().payload[0], 7);
    }

    #[test]
    fn re_carving_the_same_sphere_changes_nothing_further() {
        let mut chunk = PexilChunk::new(9);
        for z in 0..9 {
            for y in 0..9 {
                for x in 0..9 {
                    chunk.get_mut(x, y, z).unwrap().payload[0] = 3;
                }
            }
        }
        let mut ledger = MutationLedger::new();
        let first = carve_sphere(&mut chunk, &mut ledger, 1, (4, 4, 4), 2);
        let second = carve_sphere(&mut chunk, &mut ledger, 2, (4, 4, 4), 2);
        assert!(first > 0);
        assert_eq!(second, 0, "already-AIR cells are not re-mutated");
        assert_eq!(ledger.len() as u32, first);
    }

    #[test]
    fn fill_then_carve_round_trips_ledger_before_after() {
        let mut chunk = PexilChunk::new(5);
        let mut ledger = MutationLedger::new();
        fill_sphere(&mut chunk, &mut ledger, 10, (2, 2, 2), 1, 9);
        carve_sphere(&mut chunk, &mut ledger, 11, (2, 2, 2), 1);
        assert_eq!(chunk.get(2, 2, 2).unwrap().payload[0], AIR);
        // Two mutations at the center cell: fill then carve.
        let rows: Vec<_> = ledger.rows().iter().filter(|r| r.tick == 10 || r.tick == 11).collect();
        assert!(rows.len() >= 2);
    }
}
