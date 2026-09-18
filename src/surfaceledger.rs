//! Ported verbatim from F:\NewRepo\crates\forge-core\src\surfaceledger.rs (2026-08-17 truth-hunt lineage port).
//!
//! SurfaceLedger — reversible sovereign asset grammar.
//!
//! Packed deterministic ledger format for image → mesh → SDF → GLB → motion round-trip.
//! All types `#[repr(C)]` + `Pod` + `Zeroable` for memcpy/serialization without alloc.
//!
//! Per `.kiro/steering/surfaceledger-doctrine.md`:
//! - Axis contract: X = pixel_x, Z = pixel_y, Y = recovered_height
//! - Integer/quantized canonical; f32 only at GLB/GPU boundary
//! - Replaces FlatSurfaceCell / FlatSurfaceLedger / SurfaceCell terminology
//!
//! Phase 2 lands the types; Phase 3 lands the quantization helpers and local_hash.

use bytemuck::{Pod, Zeroable};
use core::mem::size_of;

// ── Magic + version ─────────────────────────────────────────────────────────

/// `b"SLDG"` little-endian. Identifies a SurfaceLedger blob.
pub const SURFACELEDGER_MAGIC: u32 = u32::from_le_bytes(*b"SLDG");

/// Schema version (bump when types change incompatibly).
pub const SURFACELEDGER_VERSION: u16 = 1;

/// Default axis contract: X = pixel_x, Z = pixel_y, Y = recovered_height.
pub const AXIS_CONTRACT_DEFAULT: u16 = 1;

/// Default quantization: y_q16 normalized height, q15 normals.
pub const QUANTIZATION_DEFAULT: u16 = 1;

// ── Header ──────────────────────────────────────────────────────────────────

/// Header for a SurfaceLedger blob — versioned, magic-tagged, deterministic.
///
/// Size = 32 bytes (multiple of 8, satisfies u64 alignment for `hash_seed`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct SurfaceLedgerHeader {
    /// Format identifier — always [`SURFACELEDGER_MAGIC`] (`'SLDG'`).
    pub magic: u32,
    /// Schema version — see [`SURFACELEDGER_VERSION`].
    pub version: u16,
    /// Ledger width in cells.
    pub width: u16,
    /// Ledger height in cells.
    pub height: u16,
    /// Which axis-contract convention this blob was written under.
    pub axis_contract_id: u16,
    /// Which quantization convention this blob was written under.
    pub quantization_id: u16,
    /// Alignment padding — always zero.
    pub _pad0: u16,
    /// Total cell count (`width * height`).
    pub cell_count: u32,
    /// Palette version this ledger's material/primitive IDs were baked against.
    pub palette_version: u32,
    /// Seed used to derive every cell's `local_hash`.
    pub hash_seed: u64,
}

const _: () = assert!(size_of::<SurfaceLedgerHeader>() == 32, "SurfaceLedgerHeader size locked at 32 bytes");

impl SurfaceLedgerHeader {
    /// Builds a header with default axis-contract/quantization IDs and a
    /// `cell_count` derived from `width * height`.
    pub fn new(width: u16, height: u16, palette_version: u32, hash_seed: u64) -> Self {
        Self {
            magic: SURFACELEDGER_MAGIC,
            version: SURFACELEDGER_VERSION,
            width,
            height,
            axis_contract_id: AXIS_CONTRACT_DEFAULT,
            quantization_id: QUANTIZATION_DEFAULT,
            _pad0: 0,
            cell_count: width as u32 * height as u32,
            palette_version,
            hash_seed,
        }
    }

    /// True when `magic`/`version` match this module's current format.
    pub fn is_valid(&self) -> bool {
        self.magic == SURFACELEDGER_MAGIC && self.version == SURFACELEDGER_VERSION
    }
}

// ── HeightLedgerCell (Phase A — height-only cell) ───────────────────────────

/// Minimal ledger cell — height + material + primitive + local hash.
/// Used for atlas height field round-trip diffs.
///
/// Size = 24 bytes (multiple of 8).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct HeightLedgerCell {
    /// Pixel X (atlas-local).
    pub x: u16,
    /// Pixel Y (atlas-local; the axis contract maps this to world Z).
    pub z: u16,
    /// Quantized recovered height — see [`f32_to_y_q16`]/[`y_q16_to_f32`].
    pub y_q16: i16,
    /// Material ID this cell was baked with.
    pub material_id: u16,
    /// Primitive-provenance ID — see [`PRIMITIVE_UNKNOWN`]/[`PRIMITIVE_DEFAULT_SOLID`].
    pub primitive_id: u16,
    /// Alignment padding — always zero.
    pub _pad: [u8; 6],
    /// Deterministic 3×3-neighborhood hash — see [`compute_local_hash`].
    pub local_hash: u64,
}

const _: () = assert!(size_of::<HeightLedgerCell>() == 24, "HeightLedgerCell size locked at 24 bytes");

// ── SurfaceLedgerCell (Phase B — full surface cell) ─────────────────────────

/// Full surface cell — height + normal + material + primitive + layer + local hash.
/// Canonical for SurfaceLedger reproject + residue + governance.
///
/// Size = 32 bytes (multiple of 8). Field order matches spec verbatim;
/// explicit `_pad` fields are present so Pod has no uninitialized holes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct SurfaceLedgerCell {
    /// Pixel X (atlas-local).
    pub x: u16,
    /// Pixel Y (atlas-local; the axis contract maps this to world Z).
    pub z: u16,
    /// Quantized recovered height — see [`f32_to_y_q16`]/[`y_q16_to_f32`].
    pub y_q16: i16,
    /// Quantized surface-normal X component — see [`f32_to_normal_i8`]/[`normal_i8_to_f32`].
    pub normal_qx: i8,
    /// Quantized surface-normal Y component.
    pub normal_qy: i8,
    /// Quantized surface-normal Z component.
    pub normal_qz: i8,
    /// Which reprojection/bake layer produced this cell.
    pub layer_id: u8,
    /// Per-cell status/provenance bitflags.
    pub flags: u8,
    /// Alignment padding — always zero.
    pub _pad0: u8,
    /// Material ID this cell was baked with.
    pub material_id: u16,
    /// Primitive-provenance ID — see [`PRIMITIVE_UNKNOWN`]/[`PRIMITIVE_DEFAULT_SOLID`].
    pub primitive_id: u16,
    /// Alignment padding — always zero.
    pub _pad1: [u8; 8],
    /// Deterministic 3×3-neighborhood hash — see [`compute_local_hash`]. Itself
    /// excluded from its own hash input (see [`cell_local_bits`]).
    pub local_hash: u64,
}

const _: () = assert!(size_of::<SurfaceLedgerCell>() == 32, "SurfaceLedgerCell size locked at 32 bytes");

// ── Primitive provenance markers ────────────────────────────────────────────

/// Primitive provenance — caller did not supply an ID. Reprojected cells
/// outside a triangle footprint, or cells produced without a primitive
/// table, are stamped UNKNOWN. Diff treats matching UNKNOWN on both sides
/// as "no signal" rather than a swap.
pub const PRIMITIVE_UNKNOWN: u16 = 0;

/// MVP source ingestion stamp — single solid region. Used by the bake
/// profiler's `build_initial_cells_from_rgba` for every opaque cell and by
/// the reprojection config until per-region primitive tables exist.
pub const PRIMITIVE_DEFAULT_SOLID: u16 = 1;

// ── SurfaceLedgerSplineKnot (Phase C — spline boundary knot) ────────────────

/// Catmull-Rom contour knot — atlas-local q16 coordinate + material-left/right + primitive.
///
/// Size = 24 bytes (multiple of 8).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct SurfaceLedgerSplineKnot {
    /// Atlas-local X, quantized — see [`f32_to_q16`]/[`q16_to_f32`].
    pub x_q16: i32,
    /// Atlas-local Z, quantized.
    pub z_q16: i32,
    /// Quantized recovered height at this knot.
    pub y_q16: i16,
    /// Alignment padding — always zero.
    pub _pad0: u16,
    /// Material ID on the left side of the contour at this knot.
    pub material_left: u16,
    /// Material ID on the right side of the contour at this knot.
    pub material_right: u16,
    /// Primitive-provenance ID — see [`PRIMITIVE_UNKNOWN`]/[`PRIMITIVE_DEFAULT_SOLID`].
    pub primitive_id: u16,
    /// Alignment padding — always zero.
    pub _pad1: [u8; 6],
}

const _: () = assert!(size_of::<SurfaceLedgerSplineKnot>() == 24, "SurfaceLedgerSplineKnot size locked at 24 bytes");

// ── SurfaceLedgerOrientedPoint (Phase D — oriented point cloud) ─────────────

/// Oriented point — quantized position + q15 normal + material + primitive.
/// Input to Poisson / SDF reconstruction.
///
/// Size = 24 bytes (multiple of 8).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct SurfaceLedgerOrientedPoint {
    /// Quantized spatial X — see [`f32_to_q16`]/[`q16_to_f32`].
    pub x_q16: i32,
    /// Quantized spatial Y.
    pub y_q16: i32,
    /// Quantized spatial Z.
    pub z_q16: i32,
    /// Quantized surface-normal X — see [`f32_to_q15`]/[`q15_to_f32`].
    pub nx_q15: i16,
    /// Quantized surface-normal Y.
    pub ny_q15: i16,
    /// Quantized surface-normal Z.
    pub nz_q15: i16,
    /// Material ID this point was sampled from.
    pub material_id: u16,
    /// Primitive-provenance ID — see [`PRIMITIVE_UNKNOWN`]/[`PRIMITIVE_DEFAULT_SOLID`].
    pub primitive_id: u16,
    /// Alignment padding — always zero.
    pub _pad: u16,
}

const _: () = assert!(size_of::<SurfaceLedgerOrientedPoint>() == 24, "SurfaceLedgerOrientedPoint size locked at 24 bytes");

// ── Quantization helpers (Phase 3) ──────────────────────────────────────────

/// Normalized height range — y_q16 = clamp(y_atlas_normalized * 32767, -32768, 32767).
pub const Y_Q16_SCALE: f32 = 32767.0;

/// Normal range — q15 stores [-32767, 32767] for [-1.0, 1.0].
pub const Q15_SCALE: f32 = 32767.0;

/// Q16 spatial range — q16 stores i32 covering [-2_147_483_648, 2_147_483_647] for [-65536.0, 65536.0).
pub const Q16_SCALE: f32 = 32768.0;

/// f32 → i16 q16-normalized height with clamp.
#[inline]
pub fn f32_to_y_q16(y_normalized: f32) -> i16 {
    let v = (y_normalized * Y_Q16_SCALE).round();
    v.clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

/// i16 q16-normalized height → f32 (boundary use only).
#[inline]
pub fn y_q16_to_f32(y_q16: i16) -> f32 {
    y_q16 as f32 / Y_Q16_SCALE
}

/// f32 in [-1.0, 1.0] → i8 normal component with clamp to [-127, 127].
#[inline]
pub fn f32_to_normal_i8(n: f32) -> i8 {
    let v = (n * 127.0).round();
    v.clamp(-127.0, 127.0) as i8
}

/// i8 normal component → f32 in [-1.0, 1.0].
#[inline]
pub fn normal_i8_to_f32(n: i8) -> f32 {
    n as f32 / 127.0
}

/// f32 in [-1.0, 1.0] → i16 q15 normal with clamp.
#[inline]
pub fn f32_to_q15(n: f32) -> i16 {
    let v = (n * Q15_SCALE).round();
    v.clamp(-Q15_SCALE, Q15_SCALE) as i16
}

/// i16 q15 normal → f32 in [-1.0, 1.0] (boundary use only).
#[inline]
pub fn q15_to_f32(n: i16) -> f32 {
    n as f32 / Q15_SCALE
}

/// f32 spatial coordinate → i32 q16 with clamp.
#[inline]
pub fn f32_to_q16(v: f32) -> i32 {
    let scaled = (v * Q16_SCALE).round();
    scaled.clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

/// i32 q16 spatial coordinate → f32 (boundary use only).
#[inline]
pub fn q16_to_f32(v: i32) -> f32 {
    v as f32 / Q16_SCALE
}

// ── local_hash computation (Phase 3) ────────────────────────────────────────

/// Compute the local hash for a single `SurfaceLedgerCell` by mixing in its
/// 3×3 row-major neighborhood. Missing neighbors are zero-filled.
///
/// Per spec `hashing.local_hash`:
/// - Includes quantized_height, quantized_normal, material_id, primitive_id, layer_id, flags
/// - Excludes paths, timestamps, raw floats, nondeterministic traversal
/// - Packing: little-endian, row-major 3×3, missing = 0
///
/// Uses splitmix64 mixing (deterministic, cache-friendly, no dependency).
pub fn compute_local_hash(
    cells: &[SurfaceLedgerCell],
    width: u16,
    cx: u16,
    cz: u16,
    seed: u64,
) -> u64 {
    let w = width as i32;
    let h = (cells.len() / width.max(1) as usize) as i32;
    let cx = cx as i32;
    let cz = cz as i32;

    let mut acc: u64 = seed;
    // Row-major 3×3 traversal: (dz, dx) from (-1,-1) to (+1,+1).
    for dz in -1i32..=1 {
        for dx in -1i32..=1 {
            let nx = cx + dx;
            let nz = cz + dz;
            let bits = if nx < 0 || nz < 0 || nx >= w || nz >= h {
                0u64
            } else {
                let idx = (nz * w + nx) as usize;
                cell_local_bits(&cells[idx])
            };
            acc = splitmix64(acc.wrapping_add(bits));
        }
    }
    acc
}

/// Pack the hashed fields of a cell into a u64 (little-endian semantic).
/// Excludes `local_hash` itself (avoids circular dependency) and `_pad`.
#[inline]
fn cell_local_bits(c: &SurfaceLedgerCell) -> u64 {
    let h = c.y_q16 as u16 as u64;            // 16 bits
    let n =  (c.normal_qx as u8 as u64)       // 8 bits
          | ((c.normal_qy as u8 as u64) << 8)  // 8 bits
          | ((c.normal_qz as u8 as u64) << 16) // 8 bits
          | ((c.layer_id as u64)        << 24) // 8 bits
          | ((c.flags    as u64)        << 32) // 8 bits
          | ((c.material_id as u64)     << 40) // 16 bits → fits in [40..56]
          ;
    // primitive_id (16 bits) folded into the top bits with a multiply mix.
    let p = (c.primitive_id as u64).wrapping_mul(0x9E3779B97F4A7C15);
    h ^ n ^ p
}

/// SplitMix64 — Marsaglia's splittable mixer. Deterministic, no allocation.
/// Takes and returns the mixer state (caller feeds each new input by adding
/// it in before calling).
#[inline]
fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E3779B97F4A7C15);
    let mut t = z;
    t = (t ^ (t >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    t = (t ^ (t >> 27)).wrapping_mul(0x94D049BB133111EB);
    t ^ (t >> 31)
}

/// Recompute local_hash for every cell in a SurfaceLedger.
/// Modifies cells in place (single pass; uses pre-computed `cell_local_bits`).
pub fn rehash_all(cells: &mut [SurfaceLedgerCell], width: u16, seed: u64) {
    let w = width.max(1) as usize;
    let h = cells.len() / w;
    // Compute hashes into a temporary buffer first (avoids using already-overwritten cells).
    let mut new_hashes = vec![0u64; cells.len()];
    for z in 0..h {
        for x in 0..w {
            new_hashes[z * w + x] = compute_local_hash(cells, width, x as u16, z as u16, seed);
        }
    }
    for (i, h) in new_hashes.into_iter().enumerate() {
        cells[i].local_hash = h;
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_is_sldg() {
        assert_eq!(SURFACELEDGER_MAGIC, u32::from_le_bytes(*b"SLDG"));
        // Confirm little-endian semantic by writing back to bytes.
        let bytes = SURFACELEDGER_MAGIC.to_le_bytes();
        assert_eq!(&bytes, b"SLDG");
    }

    #[test]
    fn header_size_is_32() {
        assert_eq!(size_of::<SurfaceLedgerHeader>(), 32);
    }

    #[test]
    fn cell_sizes_are_locked() {
        assert_eq!(size_of::<HeightLedgerCell>(), 24);
        assert_eq!(size_of::<SurfaceLedgerCell>(), 32);
        assert_eq!(size_of::<SurfaceLedgerSplineKnot>(), 24);
        assert_eq!(size_of::<SurfaceLedgerOrientedPoint>(), 24);
    }

    #[test]
    fn header_new_is_valid() {
        let h = SurfaceLedgerHeader::new(64, 64, 1, 0xDEAD_BEEF);
        assert!(h.is_valid());
        assert_eq!(h.cell_count, 64 * 64);
    }

    #[test]
    fn header_default_is_invalid() {
        let h = SurfaceLedgerHeader::default();
        assert!(!h.is_valid(), "default (zeroed) header should not pass magic check");
    }

    #[test]
    fn quantization_roundtrip_y_q16() {
        for v in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
            let q = f32_to_y_q16(v);
            let back = y_q16_to_f32(q);
            assert!((back - v).abs() < 1e-3, "y_q16 round-trip failed for {} → {} → {}", v, q, back);
        }
    }

    #[test]
    fn quantization_normal_i8_roundtrip() {
        for v in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
            let q = f32_to_normal_i8(v);
            let back = normal_i8_to_f32(q);
            assert!((back - v).abs() < 0.01, "normal_i8 round-trip failed for {} → {} → {}", v, q, back);
        }
    }

    #[test]
    fn quantization_q15_roundtrip() {
        for v in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
            let q = f32_to_q15(v);
            let back = q15_to_f32(q);
            assert!((back - v).abs() < 1e-4, "q15 round-trip failed for {} → {} → {}", v, q, back);
        }
    }

    #[test]
    fn quantization_clamps_out_of_range() {
        assert_eq!(f32_to_y_q16(10.0), i16::MAX);
        assert_eq!(f32_to_y_q16(-10.0), i16::MIN);
        assert_eq!(f32_to_normal_i8(2.0), 127);
        assert_eq!(f32_to_normal_i8(-2.0), -127);
    }

    fn make_grid(width: u16, height: u16) -> Vec<SurfaceLedgerCell> {
        let mut v = Vec::with_capacity(width as usize * height as usize);
        for z in 0..height {
            for x in 0..width {
                v.push(SurfaceLedgerCell {
                    x,
                    z,
                    y_q16: ((x as i32 + z as i32) * 100) as i16,
                    normal_qx: 0,
                    normal_qy: 0,
                    normal_qz: 127,
                    layer_id: 0,
                    flags: 0,
                    _pad0: 0,
                    material_id: x % 4,
                    primitive_id: z % 8,
                    _pad1: [0u8; 8],
                    local_hash: 0,
                });
            }
        }
        v
    }

    #[test]
    fn local_hash_deterministic() {
        let mut a = make_grid(8, 8);
        let mut b = make_grid(8, 8);
        rehash_all(&mut a, 8, 0xC0FFEE);
        rehash_all(&mut b, 8, 0xC0FFEE);
        for (ca, cb) in a.iter().zip(b.iter()) {
            assert_eq!(ca.local_hash, cb.local_hash, "local_hash must be deterministic");
        }
    }

    #[test]
    fn local_hash_seed_changes_output() {
        let mut a = make_grid(8, 8);
        let mut b = make_grid(8, 8);
        rehash_all(&mut a, 8, 0xC0FFEE);
        rehash_all(&mut b, 8, 0xDEAD_BEEF);
        // At least some hashes must differ.
        let diffs = a.iter().zip(b.iter()).filter(|(x, y)| x.local_hash != y.local_hash).count();
        assert!(diffs > 0, "different seeds must produce different hashes");
    }

    #[test]
    fn local_hash_field_change_propagates() {
        let mut a = make_grid(8, 8);
        let mut b = make_grid(8, 8);
        b[5 * 8 + 4].material_id = 99; // flip one cell in middle
        rehash_all(&mut a, 8, 0xC0FFEE);
        rehash_all(&mut b, 8, 0xC0FFEE);
        // Flipped cell + its 8 neighbors (9 total) must have changed hashes.
        let mut changed = 0;
        for (ca, cb) in a.iter().zip(b.iter()) {
            if ca.local_hash != cb.local_hash {
                changed += 1;
            }
        }
        assert!(changed >= 1, "at least the flipped cell must change");
        assert!(changed <= 9, "at most 3×3 neighborhood should propagate");
    }

    #[test]
    fn local_hash_excludes_self_local_hash_field() {
        // Two grids identical except local_hash already set differently
        // must still rehash to the same value.
        let mut a = make_grid(4, 4);
        let mut b = make_grid(4, 4);
        for c in a.iter_mut() { c.local_hash = 0xFFFF_FFFF_FFFF_FFFF; }
        for c in b.iter_mut() { c.local_hash = 0; }
        rehash_all(&mut a, 4, 0xC0FFEE);
        rehash_all(&mut b, 4, 0xC0FFEE);
        for (ca, cb) in a.iter().zip(b.iter()) {
            assert_eq!(ca.local_hash, cb.local_hash,
                "rehash must ignore the existing local_hash field");
        }
    }

    #[test]
    fn pod_zeroable_roundtrip_via_bytes() {
        let c = SurfaceLedgerCell {
            x: 7, z: 11, y_q16: -42, normal_qx: 10, normal_qy: -20, normal_qz: 100,
            layer_id: 3, flags: 0xAB, _pad0: 0,
            material_id: 0xBEEF, primitive_id: 0xCAFE,
            _pad1: [0u8; 8],
            local_hash: 0x1234_5678_9ABC_DEF0,
        };
        let bytes: &[u8] = bytemuck::bytes_of(&c);
        assert_eq!(bytes.len(), 32);
        let c2: &SurfaceLedgerCell = bytemuck::from_bytes(bytes);
        assert_eq!(&c, c2);
    }
}
