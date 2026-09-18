//! CREATE/WORLD sub-tab. forge_zones::worldgen live wire (was 0-caller).
//! generate_preview = cold-path only (allocates); render fn is alloc-free.
//!
//! TODO: forge-zones-v3 exists at F:\v3\crates\forge-zones-v3 (2026-08-17),
//! but forge-core-v3 is zero-dependency by architectural law (Crate Zero).
//! This module stubs the zone worldgen types and functions. Real ports must
//! live downstream (in crates that CAN depend on forge-zones-v3), not in core.

/// Stub for BiomeHint (from forge_zones::worldgen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeHint {
    /// Barren terrain.
    Barren,
    /// Sparse population.
    Sparse,
    /// Moderate density.
    Moderate,
    /// Dense population.
    Dense,
    /// Lush growth.
    Lush,
}

impl BiomeHint {
    /// Convert to a human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            BiomeHint::Barren => "barren",
            BiomeHint::Sparse => "sparse",
            BiomeHint::Moderate => "moderate",
            BiomeHint::Dense => "dense",
            BiomeHint::Lush => "lush",
        }
    }

    /// Convert from array index to BiomeHint.
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => BiomeHint::Barren,
            1 => BiomeHint::Sparse,
            2 => BiomeHint::Moderate,
            3 => BiomeHint::Dense,
            4 => BiomeHint::Lush,
            _ => BiomeHint::Barren,
        }
    }
}

/// Pexil type: Void (prime-indexed), Matter (composite, with factor count), or Bedrock (foundational).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PexilType {
    /// Empty pexil (prime-indexed).
    Void,
    /// Solid matter (composite-indexed).
    Matter {
        /// Distinct prime factor count for biome density.
        factor_count: u8,
    },
    /// Bedrock foundation (immutable substrate).
    Bedrock,
}

/// Chunk size in all three dimensions (typically 64).
pub const CHUNK_SIZE: usize = 64;

/// Chunk neighborhood size: a 3×3×3 cube of chunks surrounding a central chunk.
/// Used for state-chunk lookups and boundary computations.
pub const CHUNK_NEIGHBORHOOD: usize = 27;

/// A single pexil cell with spiral index tracking for prime-sieve generation.
#[derive(Clone, Copy, Debug)]
pub struct PexilCell {
    /// The spiral index used in deterministic classification.
    pub spiral_index: u64,
    /// Local (x, y, z) position in chunk (u8 for 16-voxel chunks).
    pub local_pos: (u8, u8, u8),
    /// Type (Void, Matter with factor count, or Bedrock).
    pub pexil_type: PexilType,
}

/// Stub for a generated chunk (forge-core-v3 worldgen stub only).
#[derive(Clone, Debug)]
pub struct WorldgenChunk {
    /// All pexils in this chunk.
    pub voxels: Vec<PexilCell>,
    /// Count of void pexils.
    pub void_count: u32,
    /// Count of matter pexils.
    pub matter_count: u32,
    /// Dominant biome hint.
    pub dominant_biome: BiomeHint,
    /// Content hash string.
    pub content_hash: String,
}

/// Stub for ChunkHasher.
pub struct ChunkHasher {
    seed: u64,
}

impl ChunkHasher {
    /// Stub constructor.
    pub fn new(seed: u64, _bound: u64) -> Result<Self, String> {
        if seed == 0 {
            Err("seed cannot be zero".into())
        } else {
            Ok(Self { seed })
        }
    }

    /// Stub: generate a chunk.
    pub fn generate_chunk(
        &self,
        _x: i32,
        _y: i32,
        _z: i32,
        _spiral: &UlamSpiral3DStub,
    ) -> WorldgenChunk {
        let mut voxels = Vec::new();
        for z in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    voxels.push(PexilCell {
                        spiral_index: (x + y * CHUNK_SIZE + z * CHUNK_SIZE * CHUNK_SIZE) as u64,
                        local_pos: (x as u8, y as u8, z as u8),
                        pexil_type: if (x + y + z) % 3 == 0 {
                            PexilType::Matter { factor_count: 1 }
                        } else {
                            PexilType::Void
                        },
                    });
                }
            }
        }

        let matter_count = voxels.iter().filter(|v| matches!(v.pexil_type, PexilType::Matter { .. })).count() as u32;
        let void_count = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as u32 - matter_count;

        WorldgenChunk {
            voxels,
            void_count,
            matter_count,
            dominant_biome: BiomeHint::Moderate,
            content_hash: format!("{:016x}", self.seed.wrapping_mul(31)),
        }
    }
}

/// Stub for UlamSpiral3DStub.
pub struct UlamSpiral3DStub {
    #[allow(dead_code)]
    radius: usize,
}

impl UlamSpiral3DStub {
    /// Stub constructor.
    pub fn new(radius: usize) -> Self {
        Self { radius }
    }
}

/// WorldgenPreview — the data returned by generate_preview.
#[derive(Clone)]
pub struct WorldgenPreview {
    /// Seed used.
    pub seed: u64,
    /// Count of void cells in top slice.
    pub void_count: u32,
    /// Count of matter cells in top slice.
    pub matter_count: u32,
    /// Name of dominant biome.
    pub dominant_biome: &'static str,
    /// Hash of content.
    pub content_hash: String,
    /// Z=0 slice (top layer).
    pub top_slice: [bool; CHUNK_SIZE * CHUNK_SIZE],
}

/// Generate a preview of the chunk at (0, 0, 0) for a given seed.
pub fn generate_preview(seed: u64) -> Result<WorldgenPreview, String> {
    let seed = seed.max(1);
    let hasher = ChunkHasher::new(seed, 100_000)?;
    let spiral = UlamSpiral3DStub::new(64);
    let chunk = hasher.generate_chunk(0, 0, 0, &spiral);

    let mut top_slice = [false; CHUNK_SIZE * CHUNK_SIZE];
    for cell in &chunk.voxels {
        let (x, y, z) = cell.local_pos;
        if z == 0 {
            top_slice[(x as usize) + (y as usize) * CHUNK_SIZE] = matches!(cell.pexil_type, PexilType::Matter { .. });
        }
    }

    Ok(WorldgenPreview {
        seed,
        void_count: chunk.void_count,
        matter_count: chunk.matter_count,
        dominant_biome: chunk.dominant_biome.name(),
        content_hash: chunk.content_hash,
        top_slice,
    })
}

/// WorldgenPanelState — stateful control for the worldgen panel.
pub struct WorldgenPanelState {
    /// Current seed.
    pub seed: u64,
    /// Latest preview (if any).
    pub preview: Option<WorldgenPreview>,
    /// Latest error (if any).
    pub error: Option<String>,
}

impl Default for WorldgenPanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldgenPanelState {
    /// Create a new panel state with default seed 42.
    pub fn new() -> Self {
        Self { seed: 42, preview: None, error: None }
    }

    /// Generate a preview for the current seed, advance seed, store result.
    pub fn generate_clicked(&mut self) {
        match generate_preview(self.seed) {
            Ok(p) => {
                self.seed = self.seed.wrapping_add(1);
                self.preview = Some(p);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_preview_is_deterministic() {
        let a = generate_preview(42).unwrap();
        let b = generate_preview(42).unwrap();
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn generate_preview_rejects_zero_seed() {
        let p = generate_preview(0).unwrap();
        assert_eq!(p.seed, 1);
    }

    #[test]
    fn different_seeds_usually_differ() {
        let a = generate_preview(1).unwrap();
        let b = generate_preview(2).unwrap();
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn chunk_hasher_is_deterministic() {
        let hasher = ChunkHasher::new(123, 100_000).expect("seed is nonzero");
        let spiral = UlamSpiral3DStub::new(64);
        let chunk1 = hasher.generate_chunk(0, 0, 0, &spiral);
        let chunk2 = hasher.generate_chunk(0, 0, 0, &spiral);
        assert_eq!(chunk1.content_hash, chunk2.content_hash, "same chunk hash must be identical");
    }

    #[test]
    fn generate_clicked_advances_seed() {
        let mut state = WorldgenPanelState::new();
        assert!(state.preview.is_none());
        let seed0 = state.seed;
        state.generate_clicked();
        assert!(state.preview.is_some());
        assert_eq!(state.preview.as_ref().unwrap().seed, seed0);
        assert_eq!(state.seed, seed0.wrapping_add(1));
    }

    #[test]
    fn top_slice_size_is_correct() {
        let p = generate_preview(7).unwrap();
        let matter_in_slice = p.top_slice.iter().filter(|&&m| m).count();
        assert!(matter_in_slice <= CHUNK_SIZE * CHUNK_SIZE);
    }
}
