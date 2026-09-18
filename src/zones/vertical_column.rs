//! Sparse vertical Y-chunking: stacks fixed-edge `PexilChunk`s along Y,
//! allocating each chunk only when a cell inside it is actually touched.
//! Proves a `Y=500` (1.6km at 1 voxel = 1 al-Kāshī module = 3.20m)
//! address is reachable without a ~1.07GB monolithic 512-edge dense
//! chunk — `TritCell5D` cannot hold this at all (one packed trit per
//! axis, `-1/0/+1` only); `PexilChunk`'s `(x,y,z): usize` can hold it
//! trivially, the real constraint is dense-allocation cost, not bits.

use std::collections::BTreeMap;

use crate::atom::Pexil;
use crate::zones::storage::PexilChunk;

/// A sparse column of `chunk_edge`-cubed `PexilChunk`s stacked along Y,
/// each allocated lazily on first write.
pub struct VerticalColumn {
    /// Edge length of each stacked chunk (cells per axis).
    pub chunk_edge: usize,
    /// Sparse chunks, keyed by vertical chunk index (`y / chunk_edge`).
    pub chunks: BTreeMap<i64, PexilChunk>,
}

impl VerticalColumn {
    /// A fresh, empty column — no chunks allocated until touched.
    pub fn new(chunk_edge: usize) -> Self {
        Self { chunk_edge, chunks: BTreeMap::new() }
    }

    /// Split a global `(x,y,z)` into `(vertical_chunk_index, local_x,
    /// local_y, local_z)`.
    fn split(&self, x: usize, y: usize, z: usize) -> (i64, usize, usize, usize) {
        ((y / self.chunk_edge) as i64, x, y % self.chunk_edge, z)
    }

    /// Cell at global `(x,y,z)`. `None` if its vertical chunk was never
    /// touched (read-only — does not allocate) or the local coordinate
    /// is out of that chunk's edge.
    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<&Pexil> {
        let (idx, lx, ly, lz) = self.split(x, y, z);
        self.chunks.get(&idx)?.get(lx, ly, lz)
    }

    /// Mutable cell at global `(x,y,z)`, allocating its vertical chunk on
    /// first touch.
    pub fn get_mut(&mut self, x: usize, y: usize, z: usize) -> Option<&mut Pexil> {
        let (idx, lx, ly, lz) = self.split(x, y, z);
        let edge = self.chunk_edge;
        self.chunks.entry(idx).or_insert_with(|| PexilChunk::new(edge)).get_mut(lx, ly, lz)
    }

    /// How many vertical chunks are actually allocated right now — the
    /// real receipt for "sparse," not a claim.
    pub fn allocated_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Real byte footprint: only allocated chunks count, never the full
    /// theoretical column height.
    pub fn byte_footprint(&self) -> usize {
        self.chunks.values().map(PexilChunk::byte_footprint).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_mut_allocates_only_the_touched_chunk() {
        let mut col = VerticalColumn::new(32);
        assert_eq!(col.allocated_chunk_count(), 0);
        col.get_mut(0, 5, 0).unwrap().payload[0] = 1;
        assert_eq!(col.allocated_chunk_count(), 1);
    }

    #[test]
    fn get_never_allocates() {
        let col = VerticalColumn::new(32);
        assert!(col.get(0, 500, 0).is_none());
        assert_eq!(col.allocated_chunk_count(), 0);
    }

    /// **Audit answer, as a test, not a claim.** Ground plinth at Y=0
    /// (0m), cross finial at Y=500 (500 * 3.20m = 1600m exactly). Both
    /// resolve without panic or truncation — `usize` has no trouble with
    /// 500 — and only the two touched vertical chunks are ever
    /// allocated, not all 16 a naive dense 512-tall column would need.
    #[test]
    fn test_1600m_sky_ceiling() {
        const MODULE_M_X100: i64 = 320; // 3.20m, fixed-point x100 (no float)
        const SKY_CEILING_Y: usize = 500;

        let mut column = VerticalColumn::new(32);

        column.get_mut(10, 0, 10).unwrap().payload[0] = 8; // ground plinth
        column.get_mut(10, SKY_CEILING_Y, 10).unwrap().payload[0] = 3; // cross finial

        assert_eq!(column.get(10, 0, 10).unwrap().payload[0], 8);
        assert_eq!(column.get(10, SKY_CEILING_Y, 10).unwrap().payload[0], 3);

        // 500 modules * 3.20m = 1600.00m exactly, integer fixed-point.
        assert_eq!(SKY_CEILING_Y as i64 * MODULE_M_X100, 160_000); // 1600.00m x100

        // The real claim: sparse. Only chunk 0 (Y=0) and chunk 15
        // (500/32=15) exist — not all 16 a dense 0..512 column would need.
        assert_eq!(SKY_CEILING_Y / 32, 15);
        assert_eq!(column.allocated_chunk_count(), 2);
        assert!(column.chunks.contains_key(&0));
        assert!(column.chunks.contains_key(&15));

        // Real footprint vs. the monolithic-dense alternative.
        let dense_512_bytes = 512usize.pow(3) * 8;
        assert!(
            column.byte_footprint() * 1000 < dense_512_bytes,
            "2 sparse 32^3 chunks ({} bytes) must cost a tiny fraction of a dense 512-edge chunk ({} bytes)",
            column.byte_footprint(),
            dense_512_bytes
        );
    }
}
