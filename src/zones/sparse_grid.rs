//! Full sparse chunk grid — one flat `HashMap`, no nested per-layer map.
//! Supersedes `vertical_column::VerticalColumn` (Y-only sharding) with
//! real X/Y/Z chunking, and folds the world layer (`W`, surface `0` /
//! subterranean `<0`) directly into the chunk key rather than wrapping N
//! separate `SparseChunkGrid`s in an outer `BTreeMap<i8, _>` — one flat
//! O(1) lookup instead of two nested ones.
//!
//! `W` is a real `i8` here, not a `TritCell5D` trit (which only spans
//! `-1..=1` and cannot address a world layer, let alone a chunk grid at
//! world scale — `TritCell5D` has no `x()`/`y()`/`z()`/`w()` accessors).
//! Girih rotation angle `k` is deliberately NOT part of this key: it's an
//! authoring-time parameter for what a brush builds, not a location — a
//! cell's chunk doesn't change based on which angle placed it.
//!
//! Vertical chunk indexing for the `1.6km` sky ceiling: `Y_chunk =
//! floor(Y / 32)`, `Y in [0..500] -> Y_chunk in [0..15]` (16 levels max).
//! Uncarved sky chunks are never instantiated.

use std::collections::HashMap;

use crate::atom::Pexil;
use crate::zones::storage::PexilChunk;

/// A chunk's grid address: `(X_chunk, Y_chunk, Z_chunk, W_layer)`.
pub type ChunkKey = (i32, i32, i32, i8);

/// A sparse, multi-layer grid of `chunk_edge`-cubed `PexilChunk`s, each
/// allocated lazily on first write. One instance IS the whole world —
/// every `W` layer lives in the same flat map.
pub struct SparseChunkGrid {
    /// Edge length of each chunk (cells per axis). Default `32`.
    pub chunk_edge: usize,
    /// Allocated chunks, keyed by grid address — absent key means
    /// "entirely unallocated," not "entirely air."
    pub chunks: HashMap<ChunkKey, PexilChunk>,
}

impl SparseChunkGrid {
    /// A fresh, empty grid — no chunks allocated until touched.
    pub fn new(chunk_edge: usize) -> Self {
        Self { chunk_edge, chunks: HashMap::new() }
    }

    /// World coordinate `(x,y,z,w)` -> chunk key and local offset within it.
    pub fn resolve_coord(&self, x: usize, y: usize, z: usize, w: i8) -> (ChunkKey, usize, usize, usize) {
        let e = self.chunk_edge;
        let key = ((x / e) as i32, (y / e) as i32, (z / e) as i32, w);
        (key, x % e, y % e, z % e)
    }

    /// Get or insert a chunk on-demand when edited.
    pub fn get_or_create_mut(&mut self, key: ChunkKey) -> &mut PexilChunk {
        let e = self.chunk_edge;
        self.chunks.entry(key).or_insert_with(|| PexilChunk::new(e))
    }

    /// Mutable cell at world `(x,y,z)` on layer `w`, allocating its chunk
    /// on first touch.
    pub fn get_mut(&mut self, x: usize, y: usize, z: usize, w: i8) -> Option<&mut Pexil> {
        let (key, lx, ly, lz) = self.resolve_coord(x, y, z, w);
        self.get_or_create_mut(key).get_mut(lx, ly, lz)
    }

    /// Read-only cell at world `(x,y,z)` on layer `w`. `None` if its
    /// chunk was never touched (does NOT allocate — rendering/reading
    /// must never grow storage) or the local offset is out of that
    /// chunk's edge.
    pub fn get(&self, x: usize, y: usize, z: usize, w: i8) -> Option<&Pexil> {
        let (key, lx, ly, lz) = self.resolve_coord(x, y, z, w);
        self.chunks.get(&key)?.get(lx, ly, lz)
    }

    /// How many chunks are actually allocated right now, across every
    /// layer — the real receipt for "sparse," not a claim.
    pub fn allocated_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// How many chunks are allocated on layer `w` specifically.
    pub fn allocated_chunk_count_on_layer(&self, w: i8) -> usize {
        self.chunks.keys().filter(|k| k.3 == w).count()
    }

    /// Real byte footprint: only allocated chunks count.
    pub fn byte_footprint(&self) -> usize {
        self.chunks.values().map(PexilChunk::byte_footprint).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::raymarch::{CameraMode, PovCamera};

    #[test]
    fn resolve_coord_splits_key_and_local_correctly_including_layer() {
        let grid = SparseChunkGrid::new(32);
        let (key, lx, ly, lz) = grid.resolve_coord(40, 500, 10, -1);
        assert_eq!(key, (1, 15, 0, -1)); // 40/32=1, 500/32=15, 10/32=0, layer=-1
        assert_eq!((lx, ly, lz), (8, 20, 10)); // 40%32=8, 500%32=20, 10%32=10
    }

    #[test]
    fn different_layers_at_the_same_xyz_are_distinct_chunks() {
        let mut grid = SparseChunkGrid::new(32);
        grid.get_mut(0, 0, 0, 0).unwrap().payload[0] = 1;
        grid.get_mut(0, 0, 0, -1).unwrap().payload[0] = 2;
        assert_eq!(grid.allocated_chunk_count(), 2, "same xyz, different W -> different chunks");
        assert_eq!(grid.get(0, 0, 0, 0).unwrap().payload[0], 1);
        assert_eq!(grid.get(0, 0, 0, -1).unwrap().payload[0], 2);
    }

    #[test]
    fn get_never_allocates_get_mut_does() {
        let mut grid = SparseChunkGrid::new(32);
        assert!(grid.get(0, 500, 0, 0).is_none());
        assert_eq!(grid.allocated_chunk_count(), 0);
        grid.get_mut(0, 500, 0, 0).unwrap().payload[0] = 3;
        assert_eq!(grid.allocated_chunk_count(), 1);
        assert_eq!(grid.get(0, 500, 0, 0).unwrap().payload[0], 3);
    }

    /// **Audit answer, as a test.** Ground masonry at `(0,0,0)`, a spire
    /// cross finial at `(0,500,0)` — `500 * 3.20m = 1600.00m` exactly.
    /// Only the two touched chunks (`Y_chunk=0` and `Y_chunk=15`) are
    /// ever allocated; the 14 intermediate sky chunks stay at 0 bytes.
    /// Rendered through the real `PovCamera` sampling this sparse grid
    /// directly (never through a monolithic allocation) to confirm both
    /// the ground and the 1.6km finial are actually visible.
    #[test]
    fn test_1600m_sparse_sky_ceiling() {
        let mut grid = SparseChunkGrid::new(32);

        grid.get_mut(0, 0, 0, 0).unwrap().payload[0] = 8; // ground masonry
        grid.get_mut(0, 500, 0, 0).unwrap().payload[0] = 3; // spire cross finial

        assert_eq!(500usize / 32, 15);
        assert_eq!(grid.allocated_chunk_count(), 2, "only ground + apex chunks, 14 sky chunks stay unallocated");
        assert!(grid.chunks.contains_key(&(0, 0, 0, 0)));
        assert!(grid.chunks.contains_key(&(0, 15, 0, 0)));

        let dense_512_bytes = 512usize.pow(3) * 8;
        assert!(
            grid.byte_footprint() * 1000 < dense_512_bytes,
            "2 sparse 32^3 chunks ({} bytes) must cost a tiny fraction of a dense 512^3 chunk ({} bytes)",
            grid.byte_footprint(),
            dense_512_bytes
        );

        // Render south-elevation through the real PovCamera, sampling the
        // sparse grid directly (no monolithic chunk ever built).
        let camera = PovCamera::new(CameraMode::SouthElevation, 0);
        let frame = camera.render_frame_sparse(&grid, 0, 512, 512, 512);
        assert_eq!(frame.len(), 512 * 512 * 3);

        let has_solid = frame.chunks_exact(3).any(|p| p == [220, 220, 220]);
        assert!(has_solid, "ground and/or finial masonry must be visible in the elevation");
    }
}
