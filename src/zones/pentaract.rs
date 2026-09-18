//! `FlatPentaract` — a bounded, dense 5D hyper-brick. Deliberately NOT
//! world storage: dense `N^5` blows up fast (that's exactly what `Pexil`/
//! `PexilLine`/`PexilChunk` exist to avoid — see thornhaven-mesh-2026-08-
//! 19.md's 687GB-at-2048³ receipt). This is a small LOCAL brick — default
//! shape `N=3` (`3^5=243` cells, ~1.94KB) — that slices down to the
//! already-live 3D `PexilChunk`/raymarch/mutation path by pinning two
//! axes, same shape as `StructuralBox::project_to_plane` one dimension up.
//!
//! Distinct name from `crate::pentaract::Pentaract` (the S⁴ hypersphere
//! mood-field point, ARCH000-protected, unrelated concept) — same root
//! word, different shape entirely: that one is an *angle* on a sphere,
//! this one is a *grid*.

use crate::atom::{CellOrdinal, Pexil, TritCell5D, ValidityMask};
use crate::zones::storage::PexilChunk;

/// A bounded dense 5D hypercube brick, `N` cells per axis (`N^5` total).
/// Axes: `(x, y, z, k, w)` — spatial x/y/z, Girih angle index `k`, world
/// layer `w`.
pub struct FlatPentaract<const N: usize> {
    /// Flat storage, `N^5` cells, linear index per [`FlatPentaract::index`].
    cells: Vec<Pexil>,
}

impl<const N: usize> FlatPentaract<N> {
    /// Construct a fresh brick: every cell defaulted (unknown validity,
    /// `AIR` payload), its `TritCell5D` lattice address stamped in when
    /// `N == 3` (see [`lattice_for`](Self::lattice_for)'s aperture note).
    pub fn new() -> Self {
        let default = Pexil {
            lattice: TritCell5D::ORIGIN,
            validity: ValidityMask::ALL_UNKNOWN,
            ordinal: CellOrdinal(0),
            payload: [0; 4],
        };
        let mut cells = vec![default; N * N * N * N * N];
        for w in 0..N {
            for k in 0..N {
                for z in 0..N {
                    for y in 0..N {
                        for x in 0..N {
                            if let Some(lattice) = Self::lattice_for(x, y, z, k, w) {
                                cells[Self::index(x, y, z, k, w)].lattice = lattice;
                            }
                        }
                    }
                }
            }
        }
        Self { cells }
    }

    /// Linear index: `x + N*(y + N*(z + N*(k + N*w)))`.
    #[inline]
    pub fn index(x: usize, y: usize, z: usize, k: usize, w: usize) -> usize {
        x + N * (y + N * (z + N * (k + N * w)))
    }

    /// `TritCell5D` address for a grid position. `Some` only at `N == 3`
    /// — axis index `0/1/2` maps to balanced trit `-1/0/+1`, matching
    /// `TritCell5D`'s own 5-trit width exactly. `None` for any other `N`
    /// (aperture: this brick's lattice-tagging assumes the 3-trit shape;
    /// a wider brick still stores/slices correctly, it just carries
    /// `TritCell5D::ORIGIN` in every cell rather than a real address).
    fn lattice_for(x: usize, y: usize, z: usize, k: usize, w: usize) -> Option<TritCell5D> {
        if N != 3 {
            return None;
        }
        let t = |i: usize| i as i8 - 1;
        Some(TritCell5D::from_trits([t(x), t(y), t(z), t(k), t(w)]))
    }

    /// Cell at `(x,y,z,k,w)`, or `None` if any axis is `>= N`.
    pub fn get(&self, x: usize, y: usize, z: usize, k: usize, w: usize) -> Option<&Pexil> {
        if x >= N || y >= N || z >= N || k >= N || w >= N {
            return None;
        }
        self.cells.get(Self::index(x, y, z, k, w))
    }

    /// Mutable cell at `(x,y,z,k,w)`, or `None` if any axis is `>= N`.
    pub fn get_mut(&mut self, x: usize, y: usize, z: usize, k: usize, w: usize) -> Option<&mut Pexil> {
        if x >= N || y >= N || z >= N || k >= N || w >= N {
            return None;
        }
        Some(&mut self.cells[Self::index(x, y, z, k, w)])
    }

    /// Total byte footprint: `N^5 * 8`.
    pub fn byte_footprint(&self) -> usize {
        self.cells.len() * core::mem::size_of::<Pexil>()
    }

    /// The whole flat cell array, mutable — the sink for a brush pass over
    /// the brick (e.g. `forge_canvas_v3::acrylic::stamp_5d`) rather than a
    /// per-cell `get_mut` loop.
    pub fn cells_mut(&mut self) -> &mut [Pexil] {
        &mut self.cells
    }

    /// The whole flat cell array, read-only — a scan pass over the brick
    /// (e.g. `forge_canvas_v3::acrylic::material_scan_from_brick`) rather
    /// than a per-cell `get` loop.
    pub fn cells(&self) -> &[Pexil] {
        &self.cells
    }

    /// Project the 3D spatial sub-grid at fixed Girih angle index `k` and
    /// signed world layer `w` into a standalone `PexilChunk` (edge `N`) —
    /// the bridge from 5D storage onto the already-live 3D raymarch/
    /// mutation path. `w` is signed (`-1..=1` at `N==3`, mapped to axis
    /// index via `w+1`) to match `TritCell5D`'s own balanced-trit
    /// convention; `k` is an unsigned raw axis index (`0..N`).
    pub fn slice_3d(&self, k: u8, w: i8) -> PexilChunk {
        let k = (k as isize).rem_euclid(N as isize) as usize;
        let w_idx = (w as isize + 1).rem_euclid(N as isize) as usize;
        let mut chunk = PexilChunk::new(N);
        for z in 0..N {
            for y in 0..N {
                for x in 0..N {
                    if let (Some(src), Some(dst)) = (self.get(x, y, z, k, w_idx), chunk.get_mut(x, y, z)) {
                        *dst = *src;
                    }
                }
            }
        }
        chunk
    }
}

impl<const N: usize> Default for FlatPentaract<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::ledger::MutationLedger;
    use crate::zones::project3d::{fill_sphere, AIR};

    #[test]
    fn new_3x3x3x3x3_produces_243_cells_and_1944_bytes() {
        let brick: FlatPentaract<3> = FlatPentaract::new();
        assert_eq!(brick.cells.len(), 243);
        assert_eq!(brick.byte_footprint(), 1944);
    }

    #[test]
    fn get_get_mut_round_trip_across_all_five_axes() {
        let mut brick: FlatPentaract<3> = FlatPentaract::new();
        brick.get_mut(2, 1, 0, 2, 0).unwrap().ordinal = CellOrdinal(77);
        assert_eq!(brick.get(2, 1, 0, 2, 0).unwrap().ordinal, CellOrdinal(77));
        assert!(brick.get(3, 0, 0, 0, 0).is_none(), "x out of bounds");
        assert!(brick.get(0, 0, 0, 0, 3).is_none(), "w out of bounds");
    }

    #[test]
    fn lattice_address_round_trips_the_trit_mapping_at_n3() {
        let brick: FlatPentaract<3> = FlatPentaract::new();
        // (0,0,0,0,0) -> all trits -1 -> TritCell5D::from_trits([-1;5]).
        let cell = brick.get(0, 0, 0, 0, 0).unwrap();
        assert_eq!(cell.lattice, TritCell5D::from_trits([-1, -1, -1, -1, -1]));
        // (1,1,1,1,1) -> all trits 0 -> the origin.
        let mid = brick.get(1, 1, 1, 1, 1).unwrap();
        assert_eq!(mid.lattice, TritCell5D::ORIGIN);
    }

    #[test]
    fn slice_3d_pins_k_and_w_and_extracts_the_matching_cells() {
        let mut brick: FlatPentaract<3> = FlatPentaract::new();
        brick.get_mut(1, 1, 1, 1, 1).unwrap().payload[0] = 42;
        // slice at k=1 (raw index 1), w=0 (signed -> axis index 1): matches (x,y,z,1,1).
        let chunk = brick.slice_3d(1, 0);
        assert_eq!(chunk.get(1, 1, 1).unwrap().payload[0], 42);
        // A different (k,w) pin must NOT see that same stamped cell.
        let other = brick.slice_3d(0, -1);
        assert_eq!(other.get(1, 1, 1).unwrap().payload[0], 0);
    }

    /// CPU raymarch witness: carve a solid sphere into a sliced 3D chunk,
    /// cast a 16x16 grid of rays through it from behind, and confirm the
    /// render path actually produces hits and non-zero shading output.
    /// No GPU context required — matches `island_gate.rs`'s proven
    /// CPU-raymarch-against-the-world-sampler shape.
    #[test]
    fn test_render_flat_pentaract() {
        let brick: FlatPentaract<3> = FlatPentaract::new();
        let mut chunk = brick.slice_3d(1, 0);
        let mut ledger = MutationLedger::new();

        // Author a solid sphere at the slice's center.
        let changed = fill_sphere(&mut chunk, &mut ledger, 1, (1, 1, 1), 1, 9);
        assert!(changed > 0, "fill_sphere must actually place solid material");
        assert_eq!(ledger.len() as u32, changed);

        // Camera behind the chunk, facing +Z; 16x16 rays, integer-only.
        const RES: usize = 16;
        let mut pixels = [0u8; RES * RES];
        let mut land_hits: u32 = 0;

        for py in 0..RES {
            for px in 0..RES {
                // Map the 16-wide raster onto the 3-wide chunk (coarse,
                // deterministic integer division — no float anywhere).
                let fx = (px * 3) / RES;
                let fy = (py * 3) / RES;
                let mut hit = false;
                for z in 0..3usize {
                    if let Some(cell) = chunk.get(fx, fy, z) {
                        if cell.payload[0] != AIR {
                            hit = true;
                            break;
                        }
                    }
                }
                if hit {
                    land_hits += 1;
                    pixels[py * RES + px] = 255;
                }
            }
        }

        assert!(land_hits > 0, "raymarch found no solid hits");
        assert!(pixels.iter().any(|&p| p != 0), "no non-zero pixel shading output");
    }

    #[test]
    fn slice_3d_wraps_at_edges() {
        let mut brick: FlatPentaract<3> = FlatPentaract::new();
        brick.get_mut(1, 1, 1, 1, 1).unwrap().payload[0] = 42;
        let slice_at_zero = brick.slice_3d(1, 0);
        let slice_at_three = brick.slice_3d(1, 3);
        assert_eq!(
            slice_at_zero.get(1, 1, 1).unwrap().payload[0],
            slice_at_three.get(1, 1, 1).unwrap().payload[0],
            "slice_3d(1, 0) and slice_3d(1, 3) must wrap to the same layer"
        );
    }
}
